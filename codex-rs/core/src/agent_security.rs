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
