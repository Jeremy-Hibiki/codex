//! Per-session plaintext content cache.

use std::collections::HashMap;
use std::collections::VecDeque;

use crate::token::random_hex;
use zeroize::Zeroizing;

/// Cached plaintext plus the metadata needed to frame it during rehydration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedContent {
    pub plaintext: Zeroizing<String>,
    pub skill_name: String,
    pub base_dir: Option<String>,
}

/// Caches plaintext per session for rehydration and outbound detection.
/// Deduplicates by plaintext and caps entries per session with FIFO eviction.
pub struct ContentCache {
    by_session: HashMap<String, VecDeque<(String, CachedContent)>>,
    session_bytes: HashMap<String, usize>,
    cap: usize,
    max_bytes: usize,
}

impl ContentCache {
    pub fn new(cap: usize) -> Self {
        Self::new_with_limits(cap, Self::DEFAULT_MAX_BYTES)
    }

    pub const DEFAULT_MAX_BYTES: usize = 8 * 1024 * 1024;

    pub fn new_with_limits(cap: usize, max_bytes: usize) -> Self {
        Self {
            by_session: HashMap::new(),
            session_bytes: HashMap::new(),
            cap,
            max_bytes,
        }
    }

    /// Stores `content` and returns its token hex. Identical plaintext in the
    /// same session returns the existing token.
    ///
    /// `is_referenced` prevents evicting entries the session registry still
    /// points at: evicting a referenced token would leave the registry saying
    /// "loaded" while rehydration turns stale.
    pub fn store(
        &mut self,
        session_id: &str,
        content: CachedContent,
        is_referenced: impl Fn(&str) -> bool,
    ) -> String {
        let bucket = self.by_session.entry(session_id.to_string()).or_default();
        for (hex, existing) in bucket.iter() {
            if existing.plaintext == content.plaintext {
                return hex.clone();
            }
        }
        let bytes = content.plaintext.len();
        let hex = random_hex();
        bucket.push_back((hex.clone(), content));
        let bytes_for_session = self
            .session_bytes
            .entry(session_id.to_string())
            .or_default();
        *bytes_for_session += bytes;
        while bucket.len() > self.cap || *bytes_for_session > self.max_bytes {
            let Some((hex, _)) = bucket.front() else {
                break;
            };
            if is_referenced(hex) {
                // A live registry reference must stay resolvable; the entry is
                // bounded by the registry/thread lifecycle instead.
                break;
            }
            if let Some((_, evicted)) = bucket.pop_front() {
                *bytes_for_session = bytes_for_session.saturating_sub(evicted.plaintext.len());
            }
        }
        hex
    }

    pub fn lookup(&self, session_id: &str, hex: &str) -> Option<&CachedContent> {
        self.by_session.get(session_id).and_then(|bucket| {
            bucket
                .iter()
                .find(|(candidate, _)| candidate == hex)
                .map(|(_, content)| content)
        })
    }

    /// Resolves cached content for a session by skill name, used to rehydrate
    /// on-disk stub blocks that name the skill's encrypted package.
    pub fn lookup_by_skill_name(
        &self,
        session_id: &str,
        skill_name: &str,
    ) -> Option<&CachedContent> {
        self.by_session.get(session_id).and_then(|bucket| {
            bucket
                .iter()
                .find(|(_, content)| content.skill_name == skill_name)
                .map(|(_, content)| content)
        })
    }

    pub fn plaintexts(&self, session_id: &str) -> impl Iterator<Item = &str> {
        self.by_session
            .get(session_id)
            .into_iter()
            .flatten()
            .map(|(_, content)| content.plaintext.as_str())
    }

    /// Iterates `(skill_name, plaintext)` pairs for a session, used by
    /// redaction auditing to name affected skills without exposing content.
    pub fn skill_plaintexts(&self, session_id: &str) -> impl Iterator<Item = (&str, &str)> {
        self.by_session
            .get(session_id)
            .into_iter()
            .flatten()
            .map(|(_, content)| (content.skill_name.as_str(), content.plaintext.as_str()))
    }

    pub fn drop_session(&mut self, session_id: &str) {
        self.by_session.remove(session_id);
        self.session_bytes.remove(session_id);
    }

    /// Removes one cached entry, used when a single skill is unloaded.
    pub fn remove(&mut self, session_id: &str, hex: &str) {
        if let Some(bucket) = self.by_session.get_mut(session_id) {
            bucket.retain(|(candidate, content)| {
                if candidate == hex {
                    if let Some(bytes) = self.session_bytes.get_mut(session_id) {
                        *bytes = bytes.saturating_sub(content.plaintext.len());
                    }
                    false
                } else {
                    true
                }
            });
        }
    }

    pub fn clear(&mut self) {
        self.by_session.clear();
        self.session_bytes.clear();
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
