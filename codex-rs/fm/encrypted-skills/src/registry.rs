//! Session-scoped decrypted skill registry with a skill-level idle TTL.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Injectable clock so TTL behavior is unit-testable without wall-clock waits.
pub trait Clock: Send + Sync {
    fn now_millis(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRecord {
    pub dir: PathBuf,
    /// The skill's original directory on disk (pre-decryption path).
    pub original_dir: PathBuf,
    pub loaded_at: u64,
    pub last_used_at: u64,
    /// Content-cache hex for the skill's `SKILL.md` plaintext.
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvictedSkill {
    pub session_id: String,
    pub skill_name: String,
    pub dir: PathBuf,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtlConfig {
    pub skill_idle: Duration,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            skill_idle: Duration::from_secs(600),
        }
    }
}

/// Tracks `sessionID -> skillName -> {dir, timestamps}` plus per-session
/// activity timestamps. Pure bookkeeping: filesystem cleanup is the caller's
/// job (see `mem_root::secure_wipe`).
pub struct Registry {
    skills: HashMap<String, HashMap<String, SkillRecord>>,
    clock: Arc<dyn Clock>,
    ttl: TtlConfig,
}

impl Registry {
    pub fn new(clock: Arc<dyn Clock>, ttl: TtlConfig) -> Self {
        Self {
            skills: HashMap::new(),
            clock,
            ttl,
        }
    }

    /// Registers (or replaces) the session/skill mapping and returns the
    /// previously registered record, if any, so the caller can wipe the stale
    /// decrypted directory it pointed at.
    pub fn register(
        &mut self,
        session_id: &str,
        skill_name: &str,
        dir: PathBuf,
        original_dir: PathBuf,
        token: String,
    ) -> Option<SkillRecord> {
        let now = self.clock.now_millis();
        let record = SkillRecord {
            dir,
            original_dir,
            loaded_at: now,
            last_used_at: now,
            token,
        };
        self.skills
            .entry(session_id.to_string())
            .or_default()
            .insert(skill_name.to_string(), record)
    }

    pub fn get(&self, session_id: &str, skill_name: &str) -> Option<&SkillRecord> {
        self.skills
            .get(session_id)
            .and_then(|bucket| bucket.get(skill_name))
    }

    pub fn skills_for_session(&self, session_id: &str) -> impl Iterator<Item = &SkillRecord> {
        self.skills
            .get(session_id)
            .into_iter()
            .flat_map(|bucket| bucket.values())
    }

    /// Number of live skill records in `session_id` that reference `token`.
    /// Content dedup can share one cache entry across skills, so an eviction
    /// must not drop the cache entry while another skill still references it.
    pub fn token_ref_count(&self, session_id: &str, token: &str) -> usize {
        self.skills
            .get(session_id)
            .map(|bucket| {
                bucket
                    .values()
                    .filter(|record| record.token == token)
                    .count()
            })
            .unwrap_or(0)
    }

    /// True when the skill is registered and its idle time is within the
    /// skill-level TTL (idempotent hit — no re-decryption needed).
    pub fn is_loaded(&self, session_id: &str, skill_name: &str) -> bool {
        let Some(record) = self.get(session_id, skill_name) else {
            return false;
        };
        let idle = self.clock.now_millis().saturating_sub(record.last_used_at);
        idle <= self.ttl.skill_idle.as_millis() as u64
    }

    /// Refreshes the skill's `last_used_at`.
    pub fn touch_skill(&mut self, session_id: &str, skill_name: &str) {
        let now = self.clock.now_millis();
        if let Some(record) = self
            .skills
            .get_mut(session_id)
            .and_then(|bucket| bucket.get_mut(skill_name))
        {
            record.last_used_at = now;
        }
    }

    /// Evicts skills whose idle time exceeds the skill-level TTL.
    pub fn sweep_expired_skills(&mut self) -> Vec<EvictedSkill> {
        let now = self.clock.now_millis();
        let ttl = self.ttl.skill_idle.as_millis() as u64;
        let mut evicted = Vec::new();
        for (session_id, bucket) in self.skills.iter_mut() {
            let mut expired = Vec::new();
            bucket.retain(|skill_name, record| {
                let idle = now.saturating_sub(record.last_used_at);
                if idle > ttl {
                    expired.push((skill_name.clone(), record.dir.clone(), record.token.clone()));
                    false
                } else {
                    true
                }
            });
            for (skill_name, dir, token) in expired {
                evicted.push(EvictedSkill {
                    session_id: session_id.clone(),
                    skill_name,
                    dir,
                    token,
                });
            }
        }
        evicted
    }

    /// Removes one thread and returns the decrypted dirs that must be wiped.
    pub fn clear_thread(&mut self, session_id: &str) -> Vec<PathBuf> {
        self.skills
            .remove(session_id)
            .into_iter()
            .flat_map(|bucket| bucket.into_values().map(|record| record.dir))
            .collect()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
