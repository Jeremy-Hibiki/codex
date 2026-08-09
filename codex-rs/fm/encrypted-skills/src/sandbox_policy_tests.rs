//! Tests for [`crate::sandbox_policy`].

use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;

use crate::sandbox_policy::ensure_encrypted_skill_sandbox;
use crate::sandbox_policy::sandbox_applies_binds;

#[test]
fn sandbox_applies_binds_only_under_bwrap_profiles() {
    let restricted = FileSystemSandboxPolicy::restricted(Vec::new());
    let unrestricted = FileSystemSandboxPolicy::unrestricted();
    for (policy, network, legacy_landlock, enforced_network, expected) in [
        (
            &restricted,
            NetworkSandboxPolicy::Enabled,
            false,
            false,
            #[cfg(target_os = "linux")]
            true,
            #[cfg(not(target_os = "linux"))]
            false,
        ),
        (
            &restricted,
            NetworkSandboxPolicy::Restricted,
            false,
            false,
            #[cfg(target_os = "linux")]
            true,
            #[cfg(not(target_os = "linux"))]
            false,
        ),
        (
            &unrestricted,
            NetworkSandboxPolicy::Enabled,
            false,
            false,
            false,
        ),
        (
            &restricted,
            NetworkSandboxPolicy::Enabled,
            true,
            false,
            false,
        ),
    ] {
        assert_eq!(
            sandbox_applies_binds(policy, network, legacy_landlock, enforced_network),
            expected
        );
    }
}

#[test]
fn ensure_encrypted_skill_sandbox_gates_engaged_execution() {
    // Without the debug bypass env, an engaged session without a sandbox is
    // rejected; CI never sets the bypass so this is a fixed expectation.
    assert_eq!(
        ensure_encrypted_skill_sandbox(true, false).is_ok(),
        fm_product_policy::sandbox_policy_bypassed()
    );
    assert!(ensure_encrypted_skill_sandbox(true, true).is_ok());
    assert!(ensure_encrypted_skill_sandbox(false, false).is_ok());
}
