use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use crate::registry::TtlConfig;
use crate::runtime::EncryptedSkillRuntime;
use crate::sdk::EnvelopeError;
use crate::sdk::EnvelopeSdk;
use crate::sdk::PackageEntry;

use super::rpc;

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
fn rpc_guard_blocks_guarded_paths_when_engaged() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Arc::new(EncryptedSkillRuntime::new_shared(
        Arc::new(TestSdk),
        TtlConfig::default(),
        tmp.path().join("mem-root"),
    ));
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();
    let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();

    assert!(rpc::is_guarded_path(&decrypted.join("SKILL.md")));
    assert!(rpc::is_guarded_path(runtime.mem_root()));
    assert!(rpc::command_references_guarded_path(&format!(
        "cat {}",
        decrypted.join("SKILL.md").to_string_lossy()
    )));
    assert!(rpc::command_references_guarded_path("find /dev/shm"));
    assert!(rpc::args_reference_guarded_path(&json!({
        "path": decrypted.join("SKILL.md").to_string_lossy()
    })));
    assert!(rpc::args_reference_guarded_path(&json!({
        "path": "/dev/shm"
    })));
    assert!(rpc::args_reference_guarded_path(&json!({
        "nested": [{"path": decrypted.join("SKILL.md").to_string_lossy()}]
    })));
    assert!(!rpc::args_reference_guarded_path(&json!({
        "path": "/dev/shmx/foo"
    })));

    drop(runtime);
    assert!(!rpc::is_guarded_path(&decrypted.join("SKILL.md")));
}

#[test]
fn rpc_guard_allows_unengaged_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Arc::new(EncryptedSkillRuntime::new_shared(
        Arc::new(TestSdk),
        TtlConfig::default(),
        tmp.path().join("mem-root"),
    ));
    let own_path = runtime.mem_root().join("own-file");

    assert!(!rpc::is_guarded_path(&own_path));
    assert!(!rpc::command_references_guarded_path(&format!(
        "cat {}",
        own_path.to_string_lossy()
    )));
    assert!(!rpc::command_references_guarded_path("find /dev/shm"));
    assert!(!rpc::args_reference_guarded_path(&json!({
        "path": own_path.to_string_lossy()
    })));
    assert!(!rpc::args_reference_guarded_path(&json!({
        "path": "/dev/shm"
    })));
}
