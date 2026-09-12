use std::path::Path;
use std::sync::Arc;

use fm_encrypted_skills::registry::TtlConfig;
use fm_encrypted_skills::runtime::EncryptedSkillRuntime;
use fm_encrypted_skills::sdk::EnvelopeError;
use fm_encrypted_skills::sdk::EnvelopeSdk;
use fm_encrypted_skills::sdk::PackageEntry;

use super::AgentSecurityContext;

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
