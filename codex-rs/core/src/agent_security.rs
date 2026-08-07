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

/// Debug-only escape hatch for the forced-sandbox product policy (I6).
///
/// Only honored in `debug_assertions` builds; release builds always return
/// `false` so the env var can never weaken production enforcement. Intended
/// for local development and CI harnesses that need full-access execution
/// without editing the product policy.
pub const SANDBOX_BYPASS_ENV_VAR: &str = "FMSH_CODEX_AGENT_SECURITY_SANDBOX_BYPASS";

static SANDBOX_BYPASS_WARNED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn sandbox_policy_bypassed() -> bool {
    #[cfg(debug_assertions)]
    {
        let active =
            sandbox_policy_bypassed_for(std::env::var(SANDBOX_BYPASS_ENV_VAR).ok().as_deref());
        if active && !SANDBOX_BYPASS_WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!(
                "{SANDBOX_BYPASS_ENV_VAR}=1 is active: full-access execution is allowed in this debug build"
            );
        }
        active
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

pub fn sandbox_policy_bypassed_for(value: Option<&str>) -> bool {
    matches!(value, Some("1"))
}

/// Product policy (I6): engaged sessions must execute under an active
/// sandbox; full-access execution is rejected at runtime.
pub fn ensure_encrypted_skill_sandbox(
    engaged: bool,
    sandbox_requested: bool,
) -> Result<(), &'static str> {
    if engaged && !sandbox_requested && !sandbox_policy_bypassed() {
        Err("encrypted skills require an active sandbox; full-access execution is disabled")
    } else {
        Ok(())
    }
}

/// Pure helpers for thread-less surfaces (app-server RPC) that must block
/// guarded paths when any session in the process is engaged.
pub mod rpc {
    use serde_json::Value;
    use std::path::Path;

    pub fn any_engaged() -> bool {
        fm_encrypted_skills::runtime::any_engaged()
    }

    fn guarded_paths() -> Vec<std::path::PathBuf> {
        fm_encrypted_skills::runtime::engaged_guarded_paths()
    }

    fn guarded_strings() -> Vec<String> {
        guarded_paths()
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    }

    /// True when `path` lies under the mem root or an engaged session's
    /// decrypted directory.
    pub fn is_guarded_path(path: &Path) -> bool {
        if !any_engaged() {
            return false;
        }
        guarded_paths().iter().any(|guard| path.starts_with(guard))
    }

    /// True when the command string references a guarded path.
    pub fn command_references_guarded_path(command: &str) -> bool {
        if !any_engaged() {
            return false;
        }
        let guarded = guarded_strings();
        fm_encrypted_skills::paths::command_references_dir(command, &guarded)
    }

    /// True when any string value in `value` references a guarded path.
    pub fn args_reference_guarded_path(value: &Value) -> bool {
        if !any_engaged() {
            return false;
        }
        let guarded = guarded_strings();
        let mut hit = false;
        collect_string_values(value, &mut |text| {
            if fm_encrypted_skills::paths::command_references_dir(text, &guarded) {
                hit = true;
            }
        });
        hit
    }

    fn collect_string_values<'a>(value: &'a Value, out: &mut impl FnMut(&'a str)) {
        match value {
            Value::String(text) => out(text),
            Value::Array(items) => {
                for item in items {
                    collect_string_values(item, out);
                }
            }
            Value::Object(map) => {
                for item in map.values() {
                    collect_string_values(item, out);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
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
