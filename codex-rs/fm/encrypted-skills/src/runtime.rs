//! Host-facing runtime that owns decryption state for encrypted skills.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::audit::AuditEvent;
use crate::audit::AuditSink;
use crate::cache::CachedContent;
use crate::cache::ContentCache;
use crate::mem_root::decrypted_dir_name;
use crate::mem_root::process_namespace;
use crate::mem_root::secure_wipe;
use crate::mem_root::write_package_entries;
use crate::paths::redact_path_prefix;
use crate::paths::rewrite_skill_paths;
use crate::registry::Clock;
use crate::registry::Registry;
use crate::registry::TtlConfig;
use crate::rehydrate::rehydrate_text_mapped;
use crate::rehydrate::wrap_with_framing;
use crate::sdk::EnvelopeError;
use crate::sdk::EnvelopeSdk;
use crate::sdk::PackageEntry;
use crate::token::Token;
use crate::token::random_hex;

/// Per-process encrypted-skill state: session registry, content cache, the
/// envelope SDK, and the memory root. Threads are isolated by session id.
pub struct EncryptedSkillRuntime {
    registry: Mutex<Registry>,
    cache: Mutex<ContentCache>,
    /// Serializes the "cache store + registry register" sequence against
    /// per-session teardown (`clear_session`), so a load cannot register a
    /// record pointing at a cache entry that teardown already dropped.
    state_lock: Mutex<()>,
    /// Per-(session, skill) gates so concurrent loads of the same skill share
    /// one decryption instead of racing to decrypt and replace each other.
    inflight: Mutex<HashMap<(String, String), Arc<Mutex<()>>>>,
    sdk: Arc<dyn EnvelopeSdk>,
    mem_root: PathBuf,
    namespace: String,
    min_free_bytes: AtomicU64,
    audit: Option<Arc<dyn AuditSink>>,
}

pub const DEFAULT_MIN_FREE_BYTES: u64 = 4 * 1024 * 1024;

impl EncryptedSkillRuntime {
    pub fn new(sdk: Arc<dyn EnvelopeSdk>, ttl: TtlConfig, mem_root: PathBuf) -> Self {
        Self::new_with_audit(sdk, ttl, mem_root, None)
    }

    pub fn new_with_audit(
        sdk: Arc<dyn EnvelopeSdk>,
        ttl: TtlConfig,
        mem_root: PathBuf,
        audit: Option<Arc<dyn AuditSink>>,
    ) -> Self {
        Self::new_with_clock_and_audit(
            sdk,
            ttl,
            mem_root,
            Arc::new(crate::registry::SystemClock),
            audit,
        )
    }

    pub fn new_with_clock(
        sdk: Arc<dyn EnvelopeSdk>,
        ttl: TtlConfig,
        mem_root: PathBuf,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self::new_with_clock_and_audit(sdk, ttl, mem_root, clock, None)
    }

    pub fn new_with_clock_and_audit(
        sdk: Arc<dyn EnvelopeSdk>,
        ttl: TtlConfig,
        mem_root: PathBuf,
        clock: Arc<dyn Clock>,
        audit: Option<Arc<dyn AuditSink>>,
    ) -> Self {
        Self {
            registry: Mutex::new(Registry::new(clock, ttl)),
            cache: Mutex::new(ContentCache::new(64)),
            state_lock: Mutex::new(()),
            inflight: Mutex::new(HashMap::new()),
            sdk,
            mem_root,
            namespace: process_namespace(),
            min_free_bytes: AtomicU64::new(DEFAULT_MIN_FREE_BYTES),
            audit,
        }
    }

    /// Decrypts the skill package for `session_id` and returns the sentinel
    /// token to inject into context. Idempotent: a load that is still within
    /// the skill TTL is reused without re-decrypting.
    pub fn load_or_register(
        &self,
        session_id: &str,
        skill_name: &str,
        package_path: &Path,
    ) -> Result<String, EnvelopeError> {
        let key = (session_id.to_string(), skill_name.to_string());
        let gate = {
            let mut inflight = self.inflight.lock().map_err(lock_error)?;
            inflight
                .entry(key.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        // Serialize concurrent loads for the same session+skill: the second
        // caller waits here and then hits the registry cache-hit path.
        let _gate = gate.lock().map_err(lock_error)?;
        let result = self.load_or_register_inner(session_id, skill_name, package_path);
        drop(_gate);
        if let Ok(mut inflight) = self.inflight.lock() {
            if inflight
                .get(&key)
                .is_some_and(|candidate| Arc::ptr_eq(candidate, &gate))
            {
                inflight.remove(&key);
            }
        }
        result
    }

    fn load_or_register_inner(
        &self,
        session_id: &str,
        skill_name: &str,
        package_path: &Path,
    ) -> Result<String, EnvelopeError> {
        {
            let mut registry = self.registry.lock().map_err(lock_error)?;
            if let Some(record) = registry.get(session_id, skill_name)
                && registry.is_loaded(session_id, skill_name)
            {
                let token = record.token.clone();
                // Refresh inside the same lock as the liveness check so a
                // concurrent sweep cannot evict the skill between the check
                // and the touch (TOCTOU).
                registry.touch_skill(session_id, skill_name);
                drop(registry);
                self.emit(AuditEvent::Decryption {
                    session_id: session_id.to_string(),
                    skill_name: skill_name.to_string(),
                    cache_hit: true,
                });
                return Ok(Token::new(session_id, token).serialize());
            }
        }

        self.check_capacity_for_package(package_path)?;
        let entries = self.sdk.decrypt_package(package_path)?;
        self.check_capacity(&entries)?;
        let plaintext = extract_skill_md(&entries)?;
        let original_dir = package_path.parent().map(Path::to_path_buf);
        let dir = self
            .mem_root
            .join(&self.namespace)
            .join(decrypted_dir_name(&random_hex()));
        if let Err(error) = write_package_entries(&entries, &dir) {
            // A partial write must not leave plaintext behind unregistered.
            let _ = secure_wipe(&dir);
            return Err(error);
        }
        let content = CachedContent {
            plaintext,
            skill_name: skill_name.to_string(),
            base_dir: package_path
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned()),
        };
        // Hold the lifecycle lock across store+register so teardown cannot
        // drop the cache between them and leave a dangling registry record.
        let _state = match self.state_lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let _ = secure_wipe(&dir);
                return Err(lock_error(poisoned));
            }
        };
        let hex = match self.cache.lock() {
            Ok(mut cache) => cache.store(session_id, content, |hex| {
                self.registry
                    .lock()
                    .map(|registry| registry.token_ref_count(session_id, hex) > 0)
                    .unwrap_or(true)
            }),
            Err(poisoned) => {
                let _ = secure_wipe(&dir);
                return Err(lock_error(poisoned));
            }
        };
        let replaced = match self.registry.lock() {
            Ok(mut registry) => registry.register(
                session_id,
                skill_name,
                dir,
                original_dir.unwrap_or_default(),
                hex.clone(),
            ),
            Err(poisoned) => {
                let _ = secure_wipe(&dir);
                return Err(lock_error(poisoned));
            }
        };
        // TTL re-load and concurrent loads replace a stale record; wipe the
        // directory the replaced record pointed at so no orphaned plaintext
        // survives in the memory root.
        if let Some(replaced) = replaced {
            let _ = secure_wipe(&replaced.dir);
        }
        self.emit(AuditEvent::Decryption {
            session_id: session_id.to_string(),
            skill_name: skill_name.to_string(),
            cache_hit: false,
        });
        self.emit(AuditEvent::Tokenization {
            session_id: session_id.to_string(),
            skill_name: skill_name.to_string(),
        });
        Ok(Token::new(session_id, hex).serialize())
    }

    /// Refreshes the skill's idle timestamp (mention or rehydration hit).
    pub fn touch(&self, session_id: &str, skill_name: &str) {
        if let Ok(mut registry) = self.registry.lock() {
            registry.touch_skill(session_id, skill_name);
        }
    }

    /// Rehydrates tokens with trust-tier framing, skill name, and the skill's
    /// original base directory anchor. Never emits the decrypted path.
    pub fn rehydrate_framed(&self, owner_session: Option<&str>, text: &str) -> String {
        // Request-level TTL sweep: any model request activity also enforces
        // the skill TTL, so idle skills on long-lived processes do not keep
        // decrypted content resident.
        self.sweep();
        let cache = match self.cache.lock() {
            Ok(cache) => cache,
            Err(_) => return text.to_string(),
        };
        let mut replaced = 0usize;
        let mut touched_skills: Vec<String> = Vec::new();
        let out = rehydrate_text_mapped(text, owner_session, &mut |token| {
            cache.lookup(&token.session_id, &token.hex).map(|content| {
                replaced += 1;
                if !touched_skills.contains(&content.skill_name) {
                    touched_skills.push(content.skill_name.clone());
                }
                wrap_with_framing(
                    &content.plaintext,
                    &content.skill_name,
                    content.base_dir.as_deref(),
                )
            })
        });
        drop(cache);
        if replaced > 0 {
            if let Some(session_id) = owner_session {
                for skill_name in &touched_skills {
                    self.touch(session_id, skill_name);
                }
            }
            self.emit(AuditEvent::Rehydration {
                session_id: owner_session.unwrap_or("").to_string(),
                token_count: replaced,
            });
        }
        out
    }

    /// Known plaintext fragments for a session (outbound export detection).
    pub fn known_plaintexts(&self, session_id: &str) -> Vec<String> {
        self.cache
            .lock()
            .map_err(lock_error)
            .map(|cache| {
                cache
                    .plaintexts(session_id)
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Runs the skill-level TTL sweep and wipes evicted directories.
    pub fn sweep(&self) {
        let skill_evictions = {
            let mut registry = match self.registry.lock() {
                Ok(registry) => registry,
                Err(_) => return,
            };
            registry.sweep_expired_skills()
        };
        // Only drop cache entries that no remaining skill references. Two
        // skills with identical content share one token via cache dedup, so
        // evicting one must not invalidate the other's cached plaintext.
        let remove_from_cache: Vec<(String, String)> = match self.registry.lock() {
            Ok(registry) => skill_evictions
                .iter()
                .filter(|evicted| {
                    registry.token_ref_count(&evicted.session_id, &evicted.token) == 0
                })
                .map(|evicted| (evicted.session_id.clone(), evicted.token.clone()))
                .collect(),
            Err(_) => Vec::new(),
        };
        let mut cache = match self.cache.lock() {
            Ok(cache) => cache,
            Err(_) => return,
        };
        for evicted in &skill_evictions {
            let _ = secure_wipe(&evicted.dir);
            self.emit(AuditEvent::Cleanup {
                session_id: evicted.session_id.clone(),
                dirs_removed: 1,
                reason: "skill_ttl_sweep".to_string(),
            });
        }
        for (session_id, token) in &remove_from_cache {
            cache.remove(session_id, token);
        }
    }

    /// Immediately clears one thread's decrypted state (thread end).
    pub fn clear_thread(&self, session_id: &str) {
        self.clear_session(session_id, "thread_end");
    }

    /// Unloads one thread's decrypted state at the end of a turn. Encrypted
    /// skills do not carry plaintext across turns; re-mentioning a skill in a
    /// later turn decrypts it again.
    pub fn unload_turn(&self, session_id: &str) {
        self.clear_session(session_id, "turn_end");
    }

    fn clear_session(&self, session_id: &str, reason: &str) {
        let Ok(_state) = self.state_lock.lock() else {
            tracing::error!("encrypted-skill lifecycle lock poisoned; session cleanup skipped");
            return;
        };
        let dirs = self
            .registry
            .lock()
            .map_err(lock_error)
            .map(|mut registry| registry.clear_thread(session_id))
            .unwrap_or_default();
        for dir in &dirs {
            let _ = secure_wipe(dir);
        }
        if let Ok(mut cache) = self.cache.lock() {
            cache.drop_session(session_id);
        }
        self.emit(AuditEvent::Cleanup {
            session_id: session_id.to_string(),
            dirs_removed: dirs.len(),
            reason: reason.to_string(),
        });
    }

    pub fn mem_root(&self) -> &Path {
        &self.mem_root
    }

    /// Sets the minimum free space required below the memory root before a new
    /// decryption is accepted (injectable for tests and deployment tuning).
    pub fn set_min_free_bytes(&self, bytes: u64) {
        self.min_free_bytes.store(bytes, Ordering::Relaxed);
    }

    /// All decrypted directories currently registered for a session.
    pub fn decrypted_dirs(&self, session_id: &str) -> Vec<PathBuf> {
        self.registry
            .lock()
            .map_err(lock_error)
            .map(|registry| {
                registry
                    .skills_for_session(session_id)
                    .map(|record| record.dir.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Rewrites references to skill original directories to the decrypted
    /// directories (longest original first).
    pub fn rewrite_paths(&self, session_id: &str, text: &str) -> String {
        let mappings: Vec<(PathBuf, PathBuf)> = self
            .registry
            .lock()
            .map_err(lock_error)
            .map(|registry| {
                registry
                    .skills_for_session(session_id)
                    .map(|record| (record.original_dir.clone(), record.dir.clone()))
                    .collect()
            })
            .unwrap_or_default();
        rewrite_skill_paths(text, &mappings)
    }

    /// Rewrites decrypted directories back to their original skill paths for
    /// tool output, so the model can see file names and reuse original paths
    /// without ever observing the /dev/shm location.
    pub fn unrewrite_paths(&self, session_id: &str, text: &str) -> String {
        let mappings: Vec<(PathBuf, PathBuf)> = self
            .registry
            .lock()
            .map_err(lock_error)
            .map(|registry| {
                registry
                    .skills_for_session(session_id)
                    .map(|record| (record.dir.clone(), record.original_dir.clone()))
                    .collect()
            })
            .unwrap_or_default();
        rewrite_skill_paths(text, &mappings)
    }

    /// Redacts decrypted storage paths from tool output text.
    pub fn redact(&self, text: &str) -> String {
        let root = self.mem_root.to_string_lossy();
        let mut out = redact_path_prefix(text, root.as_ref());
        if root.as_ref() != crate::paths::MEM_ROOT {
            out = redact_path_prefix(&out, crate::paths::MEM_ROOT);
        }
        out
    }

    /// Records a blocked tool invocation in the audit log.
    pub fn record_blocked(&self, session_id: &str, tool: &str, reason: &str) {
        self.emit(AuditEvent::Blocked {
            session_id: session_id.to_string(),
            tool: tool.to_string(),
            reason: reason.to_string(),
        });
    }

    fn emit(&self, event: AuditEvent) {
        if let Some(sink) = &self.audit {
            sink.emit(event);
        }
    }

    fn check_capacity(&self, entries: &[PackageEntry]) -> Result<(), EnvelopeError> {
        let Some(free) = crate::mem_root::available_bytes(&self.mem_root)
            .ok()
            .flatten()
        else {
            return Ok(());
        };
        let package_bytes: u64 = entries
            .iter()
            .map(|entry| entry.contents.len() as u64)
            .sum();
        let needed = package_bytes.saturating_add(self.min_free_bytes.load(Ordering::Relaxed));
        if free < needed {
            return Err(EnvelopeError::Internal(
                "memory root has insufficient free space".into(),
            ));
        }
        Ok(())
    }

    /// Pre-decryption capacity gate using the encrypted package file size, so
    /// an oversized package cannot exhaust `/dev/shm` or allocate its
    /// in-memory entries before the check runs.
    fn check_capacity_for_package(&self, package_path: &Path) -> Result<(), EnvelopeError> {
        let Some(free) = crate::mem_root::available_bytes(&self.mem_root)
            .ok()
            .flatten()
        else {
            return Ok(());
        };
        let package_len = std::fs::metadata(package_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let needed = package_len.saturating_add(self.min_free_bytes.load(Ordering::Relaxed));
        if free < needed {
            return Err(EnvelopeError::Internal(
                "memory root has insufficient free space for package".into(),
            ));
        }
        Ok(())
    }
}

fn extract_skill_md(entries: &[PackageEntry]) -> Result<String, EnvelopeError> {
    let skill_md = entries
        .iter()
        .find(|entry| entry.rel_path == Path::new("SKILL.md"))
        .ok_or_else(|| EnvelopeError::Decrypt("package has no SKILL.md entry".into()))?;
    String::from_utf8(skill_md.contents.clone())
        .map_err(|_| EnvelopeError::Decrypt("SKILL.md is not valid UTF-8".into()))
}

fn lock_error<T>(_: std::sync::PoisonError<T>) -> EnvelopeError {
    EnvelopeError::Internal("encrypted-skill state lock poisoned".into())
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
