//! Per-turn agent-security context for encrypted-skill environments.

use std::sync::Arc;

use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use fm_encrypted_skills::runtime::EncryptedSkillRuntime;

/// Carries the encrypted-skill runtime and session identity for one turn.
///
/// The context is an informational handle assembled at turn construction;
/// security decisions always consult the live runtime state via [`Self::engaged`].
#[derive(Clone)]
pub struct AgentSecurityContext {
    runtime: Arc<EncryptedSkillRuntime>,
    session_id: String,
}

impl std::fmt::Debug for AgentSecurityContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentSecurityContext")
            .field("session_id", &self.session_id)
            .field("runtime", &"<opaque>")
            .finish()
    }
}

/// Returns true when the current sandbox will actually apply readonly binds
/// for decrypted skill directories (bubblewrap in effect on Linux).
///
/// Mirrors the bwrap skip condition: legacy landlock mode never uses binds,
/// and full-disk-write with unrestricted network (no managed proxy) skips
/// bubblewrap entirely. Unreadable-glob edge combinations are approximated:
/// a full-disk-write profile with globs still returns false, which preserves
/// the pre-change legacy rewrite behavior for that rare case.
pub(crate) fn sandbox_applies_binds(
    file_system_policy: &FileSystemSandboxPolicy,
    network_policy: NetworkSandboxPolicy,
    use_legacy_landlock: bool,
    enforce_managed_network: bool,
) -> bool {
    #[cfg(target_os = "linux")]
    {
        if use_legacy_landlock {
            return false;
        }
        let full_disk_skip = file_system_policy.has_full_disk_write_access()
            && network_policy == NetworkSandboxPolicy::Enabled
            && !enforce_managed_network;
        !full_disk_skip
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (
            file_system_policy,
            network_policy,
            use_legacy_landlock,
            enforce_managed_network,
        );
        false
    }
}

impl AgentSecurityContext {
    pub fn new(runtime: Arc<EncryptedSkillRuntime>, session_id: impl Into<String>) -> Self {
        Self {
            runtime,
            session_id: session_id.into(),
        }
    }

    /// Live engagement check for this session.
    pub fn engaged(&self) -> bool {
        self.runtime.is_engaged(&self.session_id)
    }
}

#[cfg(test)]
#[path = "agent_security_tests.rs"]
mod tests;
