//! Sandbox policy predicates for encrypted-skill sessions.
//!
//! These decide whether the active sandbox will apply readonly binds for
//! decrypted skill directories, and whether an engaged session is running
//! under a sandbox at all. Kept separate from [`crate::guard`] (tool-call
//! interception) so neither module needs to grow with the other.

use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;

/// Returns true when the current sandbox will actually apply readonly binds
/// for decrypted skill directories (bubblewrap in effect on Linux).
///
/// Mirrors the bwrap skip condition: legacy landlock mode never uses binds,
/// and full-disk-write with unrestricted network (no managed proxy) skips
/// bubblewrap entirely. Unreadable-glob edge combinations are approximated:
/// a full-disk-write profile with globs still returns false, which preserves
/// the pre-change legacy rewrite behavior for that rare case.
pub fn sandbox_applies_binds(
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

/// Product policy (I6): engaged sessions must execute under an active
/// sandbox; full-access execution is rejected at runtime unless the
/// debug-only bypass is active.
pub fn ensure_encrypted_skill_sandbox(
    engaged: bool,
    sandbox_requested: bool,
) -> Result<(), &'static str> {
    if engaged && !sandbox_requested && !fm_product_policy::sandbox_policy_bypassed() {
        Err("encrypted skills require an active sandbox; full-access execution is disabled")
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "sandbox_policy_tests.rs"]
mod tests;
