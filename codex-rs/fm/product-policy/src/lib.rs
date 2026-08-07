//! Product policy for this build.
//!
//! Full-access execution and plugin/marketplace management are disabled
//! product features, independent of encrypted-skill runtime state. Keeping
//! the wording here gives the CLI and app-server a single place to update
//! when these features are re-enabled upstream.

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
