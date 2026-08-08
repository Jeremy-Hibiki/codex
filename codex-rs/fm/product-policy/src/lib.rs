//! Product policy for this build.
//!
//! Full-access execution and plugin/marketplace management are disabled
//! product features, independent of encrypted-skill runtime state. Keeping
//! the wording here gives the CLI and app-server a single place to update
//! when these features are re-enabled upstream.

/// Debug-only escape hatch for the forced-sandbox product policy (I6).
///
/// Only honored in `debug_assertions` builds; release builds always return
/// `false` so the env var can never weaken production enforcement. Intended
/// for local development and CI harnesses that need full-access execution
/// without editing the product policy.
pub const SANDBOX_BYPASS_ENV_VAR: &str = "FMSH_CODEX_AGENT_SECURITY_SANDBOX_BYPASS";

static SANDBOX_BYPASS_WARNED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub const FULL_ACCESS_DISABLED_MESSAGE: &str =
    "full-access execution is disabled by product policy; use a sandboxed permission profile";
pub const PLUGIN_MANAGEMENT_DISABLED_MESSAGE: &str =
    "plugin and marketplace management is disabled by product policy";

pub fn full_access_error() -> anyhow::Error {
    anyhow::anyhow!(FULL_ACCESS_DISABLED_MESSAGE)
}

pub fn plugin_management_error() -> anyhow::Error {
    anyhow::anyhow!(PLUGIN_MANAGEMENT_DISABLED_MESSAGE)
}

/// True when a request or command asks for full-access execution: an explicit
/// danger-full-access sandbox mode or the dangerous approvals/sandbox bypass
/// flag. Hosts keep their own error types and apply their own bypass checks.
pub fn full_access_requested(sandbox_danger: bool, permissions_danger: bool) -> bool {
    sandbox_danger || permissions_danger
}

/// True when the debug-only sandbox bypass is active for this process.
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

/// True only when the env value is exactly `1`.
pub fn sandbox_policy_bypassed_for(value: Option<&str>) -> bool {
    matches!(value, Some("1"))
}
