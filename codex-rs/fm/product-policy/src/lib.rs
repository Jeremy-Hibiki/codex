//! Product policy for this build.
//!
//! Product-policy error wording shared by the CLI and app-server. These
//! messages describe the specific policy that rejected a request.

pub const SANDBOX_BYPASS_DISABLED_MESSAGE: &str =
    "sandbox bypass is disabled by product policy; use a sandboxed permission profile";
pub const MANAGED_PLUGINS_ONLY_MESSAGE: &str = "only managed plugins are allowed by product policy";
pub const MANAGED_MARKETPLACES_ONLY_MESSAGE: &str =
    "only managed marketplaces are allowed by product policy";

pub fn sandbox_bypass_error() -> anyhow::Error {
    anyhow::anyhow!(SANDBOX_BYPASS_DISABLED_MESSAGE)
}

pub fn managed_plugins_only_error() -> anyhow::Error {
    anyhow::anyhow!(MANAGED_PLUGINS_ONLY_MESSAGE)
}

pub fn managed_marketplaces_only_error() -> anyhow::Error {
    anyhow::anyhow!(MANAGED_MARKETPLACES_ONLY_MESSAGE)
}

/// True when a request or command asks for danger-full-access sandboxing.
/// Full-access approvals are an approval-policy choice and are not a sandbox bypass.
pub fn full_access_requested(sandbox_danger: bool) -> bool {
    sandbox_danger
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_sandbox_danger_is_a_bypass() {
        assert!(super::full_access_requested(true));
        assert!(!super::full_access_requested(false));
    }
}
