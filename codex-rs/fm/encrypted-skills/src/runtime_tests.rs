use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use super::*;
use crate::audit::AuditEvent;
use crate::audit::AuditSink;
use crate::mem_root::process_namespace;
use crate::registry::Clock;
use crate::registry::TtlConfig;
use crate::sdk::PackageEntry;

#[derive(Clone, Default)]
struct FakeClock(Arc<AtomicU64>);

impl FakeClock {
    fn new(start: u64) -> Self {
        Self(Arc::new(AtomicU64::new(start)))
    }

    fn advance(&self, millis: u64) {
        self.0.fetch_add(millis, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_millis(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct CountingSdk {
    calls: Arc<AtomicUsize>,
    fail: bool,
    content_for: fn(&Path) -> &'static str,
}

#[derive(Default)]
struct CollectingSink {
    events: std::sync::Mutex<Vec<AuditEvent>>,
}

impl AuditSink for CollectingSink {
    fn emit(&self, event: AuditEvent) {
        self.events.lock().unwrap().push(event);
    }
}

impl CollectingSink {
    fn events(&self) -> Vec<AuditEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl EnvelopeSdk for CountingSdk {
    fn decrypt_package(&self, package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(EnvelopeError::Decrypt("mock failure".into()))
        } else {
            Ok(vec![
                PackageEntry {
                    rel_path: PathBuf::from("SKILL.md"),
                    contents: (self.content_for)(package_path).as_bytes().to_vec(),
                },
                PackageEntry {
                    rel_path: PathBuf::from("scripts/build.sh"),
                    contents: b"#!/bin/sh".to_vec(),
                },
            ])
        }
    }
}

fn counting_sdk(calls: Arc<AtomicUsize>, content_for: fn(&Path) -> &'static str) -> CountingSdk {
    CountingSdk {
        calls,
        fail: false,
        content_for,
    }
}

const SKILL_MD: &str = "# Encrypted skill\nrun scripts/build.sh";

#[test]
fn load_or_register_returns_sentinel_token_and_writes_layout() {
    let calls = Arc::new(AtomicUsize::new(0));
    let sdk = Arc::new(counting_sdk(calls.clone(), |_| SKILL_MD));
    let (runtime, _tmp) = test_runtime(sdk);

    let token = runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();

    assert!(token.starts_with("[SENSITIVE_SKILL_TOKEN:t1:"));
    assert_eq!(token.len(), "[SENSITIVE_SKILL_TOKEN:t1:".len() + 32 + 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(runtime.mem_root().exists());
    assert!(
        runtime.mem_root().join(process_namespace()).exists(),
        "decrypted dirs must live under the per-process namespace"
    );
}

#[test]
fn second_load_within_ttl_reuses_without_redecrypting() {
    let calls = Arc::new(AtomicUsize::new(0));
    let sdk = Arc::new(counting_sdk(calls.clone(), |_| SKILL_MD));
    let (runtime, _tmp) = test_runtime(sdk);
    let package = Path::new("/skills/secret.zip.enc");

    let first = runtime.load_or_register("t1", "secret", package).unwrap();
    let second = runtime.load_or_register("t1", "secret", package).unwrap();

    assert_eq!(first, second);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn sdk_failure_propagates_and_nothing_is_registered() {
    let sdk = Arc::new(CountingSdk {
        calls: Arc::new(AtomicUsize::new(0)),
        fail: true,
        content_for: |_| "",
    });
    let (runtime, _tmp) = test_runtime(sdk);

    assert!(matches!(
        runtime.load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc")),
        Err(EnvelopeError::Decrypt(_))
    ));
}

#[test]
fn rehydrate_uses_cached_content_with_session_gate() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let (runtime, _tmp) = test_runtime(sdk);
    let token = runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();

    assert!(
        runtime
            .rehydrate_framed(Some("t1"), &token)
            .contains(SKILL_MD)
    );
    assert_eq!(runtime.rehydrate_framed(Some("t2"), &token), token);
}

#[test]
fn redact_uses_the_runtime_mem_root() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let (runtime, _tmp) = test_runtime(sdk);
    let text = format!(
        "script at {}/fm_skill_security_abc/scripts/run.py",
        runtime.mem_root().display()
    );
    let redacted = runtime.redact(&text);
    assert_eq!(redacted, "script at [REDACTED]");
    assert!(!redacted.contains("/dev/shm/fm-agent-security"));
}

#[test]
fn rehydrate_framed_wraps_content_with_skill_and_base_dir() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let (runtime, _tmp) = test_runtime(sdk);
    let token = runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();

    let framed = runtime.rehydrate_framed(Some("t1"), &token);

    assert!(framed.contains("<skill_name>secret</skill_name>"));
    assert!(framed.contains("<base_directory>/skills</base_directory>"));
    assert!(framed.contains(SKILL_MD));
    assert!(!framed.contains("/dev/shm/fm-agent-security"));
}

#[test]
fn missing_skill_md_entry_is_an_error() {
    struct NoSkillMd;
    impl EnvelopeSdk for NoSkillMd {
        fn decrypt_package(&self, _: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
            Ok(vec![PackageEntry {
                rel_path: PathBuf::from("scripts/run.py"),
                contents: b"x".to_vec(),
            }])
        }
    }
    let (runtime, _tmp) = test_runtime(Arc::new(NoSkillMd));
    assert!(matches!(
        runtime.load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc")),
        Err(EnvelopeError::Decrypt(_))
    ));
}

#[test]
fn clear_thread_and_unload_turn_wipe_dirs_and_cache() {
    for unload in [
        |runtime: &EncryptedSkillRuntime| runtime.clear_thread("t1"),
        |runtime: &EncryptedSkillRuntime| runtime.unload_turn("t1"),
    ] {
        let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
        let (runtime, _tmp) = test_runtime(sdk);
        let token = runtime
            .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
            .unwrap();
        let decrypted = runtime.decrypted_dirs("t1");
        assert_eq!(decrypted.len(), 1);

        unload(&runtime);

        assert_eq!(runtime.rehydrate_framed(Some("t1"), &token), token);
        assert!(runtime.known_plaintexts("t1").is_empty());
        assert!(runtime.decrypted_dirs("t1").is_empty());
        assert!(!decrypted[0].exists());
    }
}

#[test]
fn sweep_evicts_skills_after_ttl_and_keeps_fresh_ones() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |path| {
        if path.to_string_lossy().contains("stale") {
            "stale content"
        } else {
            "fresh content"
        }
    }));
    let clock = Arc::new(FakeClock::new(1000));
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new_with_clock(
        sdk,
        TtlConfig {
            skill_idle: Duration::from_secs(60),
        },
        tmp.path().join("mem-root"),
        clock.clone(),
    );

    let stale = runtime
        .load_or_register("t1", "stale", Path::new("/skills/stale.zip.enc"))
        .unwrap();
    let fresh = runtime
        .load_or_register("t1", "fresh", Path::new("/skills/fresh.zip.enc"))
        .unwrap();

    clock.advance(61_000);
    runtime.touch("t1", "fresh");
    runtime.sweep();

    assert_eq!(runtime.rehydrate_framed(Some("t1"), &stale), stale);
    assert!(
        runtime
            .rehydrate_framed(Some("t1"), &fresh)
            .contains("fresh content")
    );
}

#[test]
fn ttl_reload_wipes_the_replaced_directory() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let clock = Arc::new(FakeClock::new(1000));
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new_with_clock(
        sdk,
        TtlConfig {
            skill_idle: Duration::from_secs(60),
        },
        tmp.path().join("mem-root"),
        clock.clone(),
    );
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();
    let first_dir = runtime.decrypted_dirs("t1")[0].clone();
    assert!(first_dir.exists());

    // Expire the skill TTL, then load again before any sweep runs.
    clock.advance(61_000);
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();

    assert!(
        !first_dir.exists(),
        "the replaced decrypted directory must be wiped"
    );
    let dirs = runtime.decrypted_dirs("t1");
    assert_eq!(dirs.len(), 1);
    assert!(dirs[0].exists());
}

#[test]
fn concurrent_loads_do_not_leave_orphaned_directories() {
    let calls = Arc::new(AtomicUsize::new(0));
    let sdk = Arc::new(counting_sdk(Arc::clone(&calls), |_| SKILL_MD));
    let (runtime, _tmp) = test_runtime(sdk);
    let runtime = Arc::new(runtime);
    let first = Arc::clone(&runtime);
    let second = Arc::clone(&runtime);
    let first = std::thread::spawn(move || {
        first
            .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
            .unwrap()
    });
    let second = std::thread::spawn(move || {
        second
            .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
            .unwrap()
    });
    first.join().unwrap();
    second.join().unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "concurrent loads of the same skill must share one decryption"
    );
    let dirs = runtime.decrypted_dirs("t1");
    assert_eq!(dirs.len(), 1);
    assert!(
        dirs.iter().all(|dir| dir.exists()),
        "all registered directories must exist"
    );
    let namespace = runtime.mem_root().join(process_namespace());
    let remaining: Vec<_> = std::fs::read_dir(&namespace)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    assert_eq!(
        remaining.len(),
        1,
        "concurrent loads must not leave orphaned decrypted directories"
    );
}

fn test_runtime(sdk: Arc<dyn EnvelopeSdk>) -> (EncryptedSkillRuntime, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    (
        EncryptedSkillRuntime::new(sdk, TtlConfig::default(), tmp.path().join("mem-root")),
        tmp,
    )
}

struct BlockingSdk {
    started: std::sync::Mutex<Option<mpsc::Sender<()>>>,
    release: std::sync::Mutex<Option<mpsc::Receiver<()>>>,
}

impl EnvelopeSdk for BlockingSdk {
    fn decrypt_package(&self, _package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        if let Some(tx) = self.started.lock().unwrap().take() {
            let _ = tx.send(());
        }
        if let Some(rx) = self.release.lock().unwrap().as_ref() {
            let _ = rx.recv();
        }
        Ok(vec![PackageEntry {
            rel_path: PathBuf::from("SKILL.md"),
            contents: b"# Encrypted skill".to_vec(),
        }])
    }
}

struct PartialWriteSdk;

impl EnvelopeSdk for PartialWriteSdk {
    fn decrypt_package(&self, _package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        Ok(vec![
            PackageEntry {
                rel_path: PathBuf::from("SKILL.md"),
                contents: b"# ok".to_vec(),
            },
            PackageEntry {
                rel_path: PathBuf::from("../escape"),
                contents: b"x".to_vec(),
            },
        ])
    }
}

#[test]
fn is_engaged_tracks_load_and_clear() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let (runtime, _tmp) = test_runtime(sdk);

    assert!(!runtime.is_engaged("t1"));
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();
    assert!(runtime.is_engaged("t1"));

    runtime.clear_thread("t1");
    assert!(!runtime.is_engaged("t1"));
}

#[test]
fn is_engaged_true_while_decryption_in_flight() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let sdk = Arc::new(BlockingSdk {
        started: std::sync::Mutex::new(Some(started_tx)),
        release: std::sync::Mutex::new(Some(release_rx)),
    });
    let runtime = Arc::new(test_runtime(sdk).0);

    assert!(!runtime.is_engaged("t1"));
    let rt = Arc::clone(&runtime);
    let handle = std::thread::spawn(move || {
        rt.load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
    });

    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("decryption should start");
    assert!(
        runtime.is_engaged("t1"),
        "engaged must be true while decryption is in flight"
    );

    release_tx.send(()).unwrap();
    let result = handle.join().expect("load thread should not panic");
    assert!(result.is_ok());
    assert!(runtime.is_engaged("t1"));
}

#[test]
fn failed_decryption_removes_in_flight_and_wipes_partial_dir() {
    let (runtime, _tmp) = test_runtime(Arc::new(PartialWriteSdk));

    assert!(
        runtime
            .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
            .is_err()
    );
    assert!(!runtime.is_engaged("t1"));
    assert!(runtime.decrypted_dirs("t1").is_empty());

    let namespace_dir = runtime.mem_root().join(process_namespace());
    let leftover = std::fs::read_dir(&namespace_dir)
        .expect("namespace dir should exist")
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(crate::mem_root::DECRYPTED_DIR_PREFIX)
        });
    assert!(
        !leftover,
        "partial decrypted dir must be wiped after failure"
    );
}

#[test]
fn path_mappings_expose_registered_skill_pairs() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let (runtime, _tmp) = test_runtime(sdk);
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();

    let mappings = runtime.path_mappings("t1");
    assert_eq!(mappings.len(), 1);
    let (decrypted, original) = &mappings[0];
    assert_eq!(*original, PathBuf::from("/skills"));
    assert_eq!(runtime.decrypted_dirs("t1"), vec![decrypted.clone()]);
    assert!(decrypted.starts_with(runtime.mem_root()));
}

#[test]
fn path_mappings_exclude_empty_original_dir_and_empty_session() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let (runtime, _tmp) = test_runtime(sdk);
    runtime
        .load_or_register("t1", "flat", Path::new("secret.zip.enc"))
        .unwrap();

    assert!(runtime.path_mappings("t1").is_empty());
    assert!(runtime.path_mappings("other").is_empty());
}

#[test]
fn process_registry_tracks_shared_runtime_and_guarded_paths() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new_shared_with_audit(
        sdk,
        TtlConfig::default(),
        tmp.path().join("mem-root"),
        None,
    );
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();

    assert!(any_engaged());
    let paths = engaged_guarded_paths();
    assert!(paths.contains(&runtime.mem_root().to_path_buf()));
    let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();
    assert!(paths.contains(&decrypted));

    drop(runtime);
    assert!(!engaged_guarded_paths().contains(&decrypted));
}

#[test]
fn process_registry_ignores_unengaged_shared_runtime() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let tmp = tempfile::tempdir().unwrap();
    let runtime =
        EncryptedSkillRuntime::new_shared(sdk, TtlConfig::default(), tmp.path().join("mem-root"));
    assert!(!engaged_guarded_paths().contains(&runtime.mem_root().to_path_buf()));
}

#[test]
fn audit_events_cover_decryption_and_tokenization() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let sink = Arc::new(CollectingSink::default());
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new_with_audit(
        sdk,
        TtlConfig::default(),
        tmp.path().join("mem-root"),
        Some(sink.clone()),
    );

    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();

    let events = sink.events();
    assert!(events.contains(&AuditEvent::Decryption {
        session_id: "t1".into(),
        skill_name: "secret".into(),
        cache_hit: false,
    }));
    assert!(events.contains(&AuditEvent::Decryption {
        session_id: "t1".into(),
        skill_name: "secret".into(),
        cache_hit: true,
    }));
    assert!(events.contains(&AuditEvent::Tokenization {
        session_id: "t1".into(),
        skill_name: "secret".into(),
    }));
}

#[test]
fn audit_events_cover_rehydration_and_cleanup() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let sink = Arc::new(CollectingSink::default());
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new_with_audit(
        sdk,
        TtlConfig::default(),
        tmp.path().join("mem-root"),
        Some(sink.clone()),
    );
    let token = runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();

    runtime.rehydrate_framed(Some("t1"), &token);
    runtime.record_blocked("t1", "shell", "direct read");
    runtime.clear_thread("t1");

    let events = sink.events();
    assert!(events.contains(&AuditEvent::Rehydration {
        session_id: "t1".into(),
        token_count: 1,
    }));
    assert!(events.contains(&AuditEvent::Blocked {
        session_id: "t1".into(),
        tool: "shell".into(),
        reason: "direct read".into(),
    }));
    assert!(events.contains(&AuditEvent::Cleanup {
        session_id: "t1".into(),
        dirs_removed: 1,
        reason: "thread_end".into(),
    }));
}

#[test]
fn audit_events_cover_skill_ttl_eviction() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |path| {
        if path.to_string_lossy().contains("stale") {
            "stale content"
        } else {
            "fresh content"
        }
    }));
    let sink = Arc::new(CollectingSink::default());
    let clock = Arc::new(FakeClock::new(1000));
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new_with_clock_and_audit(
        sdk,
        TtlConfig {
            skill_idle: Duration::from_secs(60),
        },
        tmp.path().join("mem-root"),
        clock.clone(),
        Some(sink.clone()),
    );
    runtime
        .load_or_register("t1", "stale", Path::new("/skills/stale.zip.enc"))
        .unwrap();

    clock.advance(61_000);
    runtime.sweep();

    let events = sink.events();
    assert!(events.contains(&AuditEvent::Cleanup {
        session_id: "t1".into(),
        dirs_removed: 1,
        reason: "skill_ttl_sweep".into(),
    }));
}

#[test]
fn rehydration_hit_refreshes_skill_ttl() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |path| {
        if path.to_string_lossy().contains("fresh") {
            "fresh content"
        } else {
            "stale content"
        }
    }));
    let clock = Arc::new(FakeClock::new(1000));
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new_with_clock(
        sdk,
        TtlConfig {
            skill_idle: Duration::from_secs(60),
        },
        tmp.path().join("mem-root"),
        clock.clone(),
    );
    let refreshed = runtime
        .load_or_register("t1", "fresh", Path::new("/skills/fresh.zip.enc"))
        .unwrap();
    let untouched = runtime
        .load_or_register("t1", "stale", Path::new("/skills/stale.zip.enc"))
        .unwrap();

    // 50s into the 60s TTL: a rehydration hit refreshes `last_used_at` for the
    // fresh skill, while the stale skill is left untouched.
    clock.advance(50_000);
    let _ = runtime.rehydrate_framed(Some("t1"), &refreshed);
    clock.advance(20_000);
    runtime.sweep();

    // The refreshed skill (touched at t=51s) survives the sweep at t=71s; the
    // untouched skill is evicted and its token goes stale.
    assert!(
        runtime
            .rehydrate_framed(Some("t1"), &refreshed)
            .contains("fresh content"),
        "refreshed content should still be available after the sweep"
    );
    assert_eq!(
        runtime.rehydrate_framed(Some("t1"), &untouched),
        untouched,
        "untouched skill should be evicted and its token should go stale"
    );
}

#[test]
fn evicting_one_shared_content_skill_keeps_the_other_token_alive() {
    // Both skills decrypt to identical plaintext, so they share one cache
    // entry (and therefore one token) via dedup.
    let sdk = Arc::new(counting_sdk(
        Arc::new(AtomicUsize::new(0)),
        |_| "shared content",
    ));
    let clock = Arc::new(FakeClock::new(1000));
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new_with_clock(
        sdk,
        TtlConfig {
            skill_idle: Duration::from_secs(60),
        },
        tmp.path().join("mem-root"),
        clock.clone(),
    );
    let first = runtime
        .load_or_register("t1", "alpha", Path::new("/skills/alpha.zip.enc"))
        .unwrap();
    let second = runtime
        .load_or_register("t1", "beta", Path::new("/skills/beta.zip.enc"))
        .unwrap();
    assert_eq!(first, second, "identical plaintext must dedup to one token");

    // Let alpha expire but keep beta fresh, then sweep.
    clock.advance(61_000);
    runtime.touch("t1", "beta");
    runtime.sweep();

    // Beta still rehydrates because its shared cache entry survived.
    assert!(
        runtime
            .rehydrate_framed(Some("t1"), &second)
            .contains("shared content"),
        "the surviving skill must keep its token rehydratable after the sweep"
    );
}

#[test]
fn request_rehydration_enforces_ttl_for_idle_skills() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let clock = Arc::new(FakeClock::new(1000));
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new_with_clock(
        sdk,
        TtlConfig {
            skill_idle: Duration::from_secs(60),
        },
        tmp.path().join("mem-root"),
        clock.clone(),
    );
    let token = runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();

    // No activity for longer than the skill TTL: the request itself (the
    // rehydration call) must sweep the idle skill, leaving the token stale.
    clock.advance(61_000);
    assert_eq!(
        runtime.rehydrate_framed(Some("t1"), &token),
        token,
        "idle skill must be unloaded by the request-level sweep"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn load_or_register_fails_when_memory_root_is_full() {
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let (runtime, _tmp) = test_runtime(sdk);
    // Demand more free space than /dev/shm (or the temp dir) can ever offer.
    runtime.set_min_free_bytes(u64::MAX);

    assert!(matches!(
        runtime.load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc")),
        Err(EnvelopeError::Internal(_))
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn oversized_package_is_rejected_before_decryption() {
    let calls = Arc::new(AtomicUsize::new(0));
    let sdk = Arc::new(counting_sdk(calls.clone(), |_| SKILL_MD));
    let (runtime, tmp) = test_runtime(sdk);
    // A package larger than the enforced headroom.
    let package_path = tmp.path().join("big.zip.enc");
    std::fs::write(&package_path, vec![0u8; 1024 * 1024]).unwrap();
    runtime.set_min_free_bytes(u64::MAX);

    assert!(matches!(
        runtime.load_or_register("t1", "secret", &package_path),
        Err(EnvelopeError::Internal(_))
    ));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "decryption must not run when the capacity gate rejects the package"
    );
}

#[test]
fn read_only_queries_survive_registry_poisoning() {
    // Poison the registry mutex, then confirm the read-only security
    // queries recover and keep serving the in-memory state instead of
    // returning empty (which would silently disable the guard).
    let sdk = Arc::new(counting_sdk(Arc::new(AtomicUsize::new(0)), |_| SKILL_MD));
    let (runtime, _tmp) = test_runtime(sdk);
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();
    let before_dirs = runtime.decrypted_dirs("t1");
    assert!(!before_dirs.is_empty(), "precondition: dirs registered");

    // Poison the registry mutex by panicking on a thread while holding it.
    let poison_runtime = Arc::new(runtime);
    {
        let inner = Arc::clone(&poison_runtime);
        let handle = std::thread::spawn(move || {
            let _guard = inner.registry.lock().unwrap();
            panic!("intentional poisoning");
        });
        let _ = handle.join();
    }

    // After poisoning, the guard-critical read-only queries must still work
    // and return the same state — not the empty fail-open default.
    assert!(
        poison_runtime.is_engaged("t1"),
        "is_engaged must recover from poisoning, not fail open"
    );
    assert_eq!(
        poison_runtime.decrypted_dirs("t1"),
        before_dirs,
        "decrypted_dirs must recover from poisoning, not return empty"
    );
    assert!(
        !poison_runtime.known_plaintexts("t1").is_empty(),
        "known_plaintexts must recover from poisoning, not return empty"
    );
}
