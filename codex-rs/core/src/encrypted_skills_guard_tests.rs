use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_encrypted_skills::registry::TtlConfig;
use codex_encrypted_skills::runtime::EncryptedSkillRuntime;
use codex_encrypted_skills::sdk::EnvelopeError;
use codex_encrypted_skills::sdk::EnvelopeSdk;
use codex_encrypted_skills::sdk::PackageEntry;
use serde_json::json;

use super::*;
use crate::tools::hook_names::HookToolName;

struct GuardTestSdk;

impl EnvelopeSdk for GuardTestSdk {
    fn decrypt_package(&self, _package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        Ok(vec![PackageEntry {
            rel_path: PathBuf::from("SKILL.md"),
            contents: b"# Guarded content".to_vec(),
        }])
    }
}

fn loaded_runtime() -> (EncryptedSkillRuntime, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new(
        Arc::new(GuardTestSdk),
        TtlConfig::default(),
        tmp.path().join("mem-root"),
    );
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .expect("load skill");
    (runtime, tmp)
}

#[test]
fn blocks_read_commands_referencing_mem_root() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": "cat /dev/shm/fm-agent-security/fm_skill_security_abc/SKILL.md" }),
    );
    assert!(matches!(decision, GuardDecision::Blocked(_)));
}

#[test]
fn blocks_search_commands_referencing_decrypted_dirs() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("grep -r secret {}", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(matches!(decision, GuardDecision::Blocked(_)));
}

#[test]
fn rewrites_original_skill_path_in_execution_commands() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": "bash /skills/secret/scripts/build.sh" }),
    );
    match decision {
        GuardDecision::Updated(updated) => {
            let command = updated["command"].as_str().expect("rewritten command");
            assert!(command.contains("/mem-root/"));
            assert!(command.contains("fm_skill_security_"));
            assert!(!command.contains("/skills/secret"));
        }
        other => panic!("expected Updated, got {other:?}"),
    }
}

#[test]
fn blocks_view_image_on_decrypted_directory() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::view_image(),
        &json!({ "path": format!("{}/image.png", dirs[0].to_string_lossy()) }),
    );
    assert!(matches!(decision, GuardDecision::Blocked(_)));
}

#[test]
fn blocks_view_image_on_unknown_mem_root_subpath() {
    let (runtime, _tmp) = loaded_runtime();
    let root = runtime.mem_root().to_string_lossy();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::view_image(),
        &json!({ "path": format!("{root}/whatever/image.png") }),
    );
    assert!(matches!(decision, GuardDecision::Blocked(_)));
}

#[test]
fn blocks_view_image_on_default_mem_root_prefix() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::view_image(),
        &json!({ "path": "/dev/shm/fm-agent-security/other/image.png" }),
    );
    assert!(matches!(decision, GuardDecision::Blocked(_)));
}

#[test]
fn allows_view_image_outside_mem_root() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::view_image(),
        &json!({ "path": "/tmp/workspace/image.png" }),
    );
    assert!(matches!(decision, GuardDecision::Allow));
}

#[test]
fn unrelated_commands_pass_through() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": "echo hello" }),
    );
    assert!(matches!(decision, GuardDecision::Allow));
}

#[test]
fn mcp_tools_pass_through() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::new("mcp__server__tool"),
        &json!({ "command": "cat /dev/shm/fm-agent-security/whatever" }),
    );
    assert!(matches!(decision, GuardDecision::Allow));
}

#[test]
fn blocks_listing_the_runtime_mem_root() {
    let (runtime, _tmp) = loaded_runtime();
    let root = runtime.mem_root().to_string_lossy().to_string();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": format!("ls {root}") }),
    );
    assert!(matches!(decision, GuardDecision::Blocked(_)));
}

#[test]
fn blocks_export_tool_containing_known_plaintext() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::apply_patch(),
        &json!({ "command": "*** Begin Patch\n+ # Guarded content\n*** End Patch" }),
    );
    assert!(matches!(decision, GuardDecision::Blocked(_)));
}

#[test]
fn allows_export_tool_without_known_plaintext() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::apply_patch(),
        &json!({ "command": "*** Begin Patch\n+ normal user content\n*** End Patch" }),
    );
    assert!(matches!(decision, GuardDecision::Allow));
}
