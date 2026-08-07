use std::path::Path;
use std::sync::Arc;

use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use fm_encrypted_skills::registry::TtlConfig;
use fm_encrypted_skills::runtime::EncryptedSkillRuntime;
use fm_encrypted_skills::sdk::EnvelopeError;
use fm_encrypted_skills::sdk::EnvelopeSdk;
use fm_encrypted_skills::sdk::PackageEntry;

use super::AgentSecurityContext;
use super::ensure_encrypted_skill_sandbox;
use super::sandbox_applies_binds;
use super::sandbox_policy_bypassed;
use super::sandbox_policy_bypassed_for;

struct TestSdk;

impl EnvelopeSdk for TestSdk {
    fn decrypt_package(&self, _package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        Ok(vec![PackageEntry {
            rel_path: std::path::PathBuf::from("SKILL.md"),
            contents: b"# Encrypted skill".to_vec(),
        }])
    }
}

#[test]
fn agent_security_context_reflects_live_engagement() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Arc::new(EncryptedSkillRuntime::new(
        Arc::new(TestSdk),
        TtlConfig::default(),
        tmp.path().join("mem-root"),
    ));
    let context = AgentSecurityContext::new(Arc::clone(&runtime), "t1");

    assert!(!context.engaged());
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();
    assert!(context.engaged());
}

#[test]
fn sandbox_applies_binds_under_bwrap_profiles() {
    let restricted = FileSystemSandboxPolicy::restricted(Vec::new());
    assert!(sandbox_applies_binds(
        &restricted,
        NetworkSandboxPolicy::Enabled,
        /*use_legacy_landlock*/ false,
        /*enforce_managed_network*/ false,
    ));
    assert!(sandbox_applies_binds(
        &restricted,
        NetworkSandboxPolicy::Restricted,
        /*use_legacy_landlock*/ false,
        /*enforce_managed_network*/ false,
    ));
}

#[test]
fn sandbox_applies_binds_false_without_bwrap() {
    let unrestricted = FileSystemSandboxPolicy::unrestricted();
    assert!(!sandbox_applies_binds(
        &unrestricted,
        NetworkSandboxPolicy::Enabled,
        /*use_legacy_landlock*/ false,
        /*enforce_managed_network*/ false,
    ));
    let restricted = FileSystemSandboxPolicy::restricted(Vec::new());
    assert!(!sandbox_applies_binds(
        &restricted,
        NetworkSandboxPolicy::Enabled,
        /*use_legacy_landlock*/ true,
        /*enforce_managed_network*/ false,
    ));
}

#[test]
fn ensure_encrypted_skill_sandbox_gates_engaged_execution() {
    assert_eq!(
        ensure_encrypted_skill_sandbox(true, false).is_err(),
        !sandbox_policy_bypassed()
    );
    assert!(ensure_encrypted_skill_sandbox(true, true).is_ok());
    assert!(ensure_encrypted_skill_sandbox(false, false).is_ok());
}

#[test]
fn sandbox_bypass_env_only_accepts_exact_one() {
    assert!(!sandbox_policy_bypassed_for(None));
    assert!(sandbox_policy_bypassed_for(Some("1")));
    assert!(!sandbox_policy_bypassed_for(Some("0")));
    assert!(!sandbox_policy_bypassed_for(Some("true")));
}
