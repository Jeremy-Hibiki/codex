//! Per-turn agent-security context for encrypted-skill environments.

use std::sync::Arc;

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
    file_system_policy: &codex_protocol::permissions::FileSystemSandboxPolicy,
    network_policy: codex_protocol::permissions::NetworkSandboxPolicy,
    use_legacy_landlock: bool,
    enforce_managed_network: bool,
) -> bool {
    fm_encrypted_skills::guard::sandbox_applies_binds(
        file_system_policy,
        network_policy,
        use_legacy_landlock,
        enforce_managed_network,
    )
}

/// Debug-only escape hatch for the forced-sandbox product policy (I6).
///
/// Only honored in `debug_assertions` builds; release builds always return
/// `false` so the env var can never weaken production enforcement. Intended
/// for local development and CI harnesses that need full-access execution
/// without editing the product policy.
/// Debug-only escape hatch for the forced-sandbox product policy (I6).
pub const SANDBOX_BYPASS_ENV_VAR: &str = fm_product_policy::SANDBOX_BYPASS_ENV_VAR;

/// Delegates to the fm product-policy implementation.
pub fn sandbox_policy_bypassed() -> bool {
    fm_product_policy::sandbox_policy_bypassed()
}

/// Delegates to the fm product-policy implementation.
pub fn sandbox_policy_bypassed_for(value: Option<&str>) -> bool {
    fm_product_policy::sandbox_policy_bypassed_for(value)
}

/// Product policy (I6): engaged sessions must execute under an active
/// sandbox; full-access execution is rejected at runtime.
pub fn ensure_encrypted_skill_sandbox(
    engaged: bool,
    sandbox_requested: bool,
) -> Result<(), &'static str> {
    fm_encrypted_skills::guard::ensure_encrypted_skill_sandbox(engaged, sandbox_requested)
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
