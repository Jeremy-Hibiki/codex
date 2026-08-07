//! Thin host adapter around `fm_encrypted_skills::guard`.
//!
//! All interception and redaction policy lives in the `fm-encrypted-skills`
//! crate. This module maps core's tool identities and output types onto that
//! boundary, so upstream merges only need to keep this single adapter in sync.

use std::sync::Arc;

use codex_protocol::models::ResponseInputItem;
use fm_encrypted_skills::runtime::EncryptedSkillRuntime;
use serde_json::Value;

pub(crate) use fm_encrypted_skills::guard::GuardDecision;
pub(crate) use fm_encrypted_skills::guard::guard_stdin_input;
pub(crate) use fm_encrypted_skills::guard::is_skill_script_execution;
pub(crate) use fm_encrypted_skills::guard::redact_all_response_item_text;
pub(crate) use fm_encrypted_skills::guard::redact_assistant_reply_item;
pub(crate) use fm_encrypted_skills::guard::redact_assistant_reply_items;
pub(crate) use fm_encrypted_skills::guard::redact_storage_paths;
pub(crate) use fm_encrypted_skills::guard::redact_text;
pub(crate) use fm_encrypted_skills::guard::redact_tool_output_plaintext_for_persistence;
pub(crate) use fm_encrypted_skills::guard::redact_turn_item;

use crate::guardian::GuardianApprovalRequest;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::hook_names::HookToolName;

/// Maps core's typed hook identity onto the fm guard's plain tool-name
/// boundary and records the block audit event inside the runtime.
pub(crate) fn before_tool_with_runtime_and_binds(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    tool_name: &HookToolName,
    tool_input: &Value,
    binds_active: bool,
) -> GuardDecision {
    fm_encrypted_skills::guard::before_tool(
        runtime,
        session_id,
        tool_name.name(),
        tool_input,
        binds_active,
    )
}

/// Wraps a tool output so every surface it feeds (preview, response item,
/// telemetry, code-mode result) has decrypted paths rewritten away.
pub(crate) struct RedactingToolOutput {
    pub(crate) inner: Box<dyn ToolOutput>,
    pub(crate) runtime: Arc<EncryptedSkillRuntime>,
    pub(crate) session_id: String,
    pub(crate) engaged: bool,
}

impl RedactingToolOutput {
    fn redact_text(&self, text: &str) -> String {
        if !self.engaged {
            return text.to_string();
        }
        redact_storage_paths(&self.runtime, &self.session_id, text)
    }

    fn redact_response_item(&self, item: &mut ResponseInputItem) {
        if !self.engaged {
            return;
        }
        fm_encrypted_skills::guard::redact_response_item(&self.runtime, &self.session_id, item);
    }

    fn redact_json(&self, value: &mut Value) {
        if !self.engaged {
            return;
        }
        fm_encrypted_skills::guard::redact_json(&self.runtime, &self.session_id, value);
    }
}

impl ToolOutput for RedactingToolOutput {
    fn log_preview(&self) -> String {
        self.redact_text(&self.inner.log_preview())
    }

    fn success_for_logging(&self) -> bool {
        self.inner.success_for_logging()
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        let mut item = self.inner.to_response_item(call_id, payload);
        self.redact_response_item(&mut item);
        item
    }

    fn post_tool_use_id(&self, call_id: &str) -> String {
        self.inner.post_tool_use_id(call_id)
    }

    fn post_tool_use_input(&self, payload: &ToolPayload) -> Option<Value> {
        self.inner.post_tool_use_input(payload)
    }

    fn post_tool_use_response(&self, call_id: &str, payload: &ToolPayload) -> Option<Value> {
        self.inner
            .post_tool_use_response(call_id, payload)
            .map(|mut value| {
                self.redact_json(&mut value);
                value
            })
    }

    fn code_mode_result(&self, payload: &ToolPayload) -> Value {
        let mut value = self.inner.code_mode_result(payload);
        self.redact_json(&mut value);
        value
    }
}

/// Rewrites decrypted `/dev/shm` paths inside a guardian approval request back
/// to original skill paths before the review prompt is built.
pub(crate) fn redact_guardian_request(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    request: GuardianApprovalRequest,
) -> GuardianApprovalRequest {
    let redact = |arg: String| redact_storage_paths(runtime, session_id, &arg);
    let redact_args = |args: Vec<String>| args.into_iter().map(redact).collect();
    match request {
        GuardianApprovalRequest::Shell {
            id,
            command,
            cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
        } => GuardianApprovalRequest::Shell {
            id,
            command: redact_args(command),
            cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
        },
        GuardianApprovalRequest::ExecCommand {
            id,
            command,
            cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
            tty,
        } => GuardianApprovalRequest::ExecCommand {
            id,
            command: redact_args(command),
            cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
            tty,
        },
        #[cfg(unix)]
        GuardianApprovalRequest::Execve {
            id,
            source,
            program,
            argv,
            cwd,
            additional_permissions,
        } => GuardianApprovalRequest::Execve {
            id,
            source,
            program: redact(program),
            argv: redact_args(argv),
            cwd,
            additional_permissions,
        },
        GuardianApprovalRequest::NetworkAccess {
            id,
            turn_id,
            target,
            host,
            protocol,
            port,
            trigger,
        } => GuardianApprovalRequest::NetworkAccess {
            id,
            turn_id,
            target,
            host,
            protocol,
            port,
            trigger: trigger.map(|mut trigger| {
                trigger.command = trigger
                    .command
                    .into_iter()
                    .map(|arg| redact_storage_paths(runtime, session_id, &arg))
                    .collect();
                trigger
            }),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use codex_utils_absolute_path::AbsolutePathBuf;
    use fm_encrypted_skills::registry::TtlConfig;
    use fm_encrypted_skills::runtime::EncryptedSkillRuntime;
    use fm_encrypted_skills::sdk::EnvelopeError;
    use fm_encrypted_skills::sdk::EnvelopeSdk;
    use fm_encrypted_skills::sdk::PackageEntry;

    use super::*;
    use crate::guardian::GuardianApprovalRequest;
    use crate::tools::context::FunctionToolOutput;

    struct TestSdk;

    impl EnvelopeSdk for TestSdk {
        fn decrypt_package(
            &self,
            _package_path: &Path,
        ) -> Result<Vec<PackageEntry>, EnvelopeError> {
            Ok(vec![PackageEntry {
                rel_path: std::path::PathBuf::from("SKILL.md"),
                contents: b"# Encrypted skill".to_vec(),
            }])
        }
    }

    fn loaded_runtime() -> (EncryptedSkillRuntime, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = EncryptedSkillRuntime::new(
            std::sync::Arc::new(TestSdk),
            TtlConfig::default(),
            tmp.path().join("mem-root"),
        );
        runtime
            .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
            .unwrap();
        (runtime, tmp)
    }

    fn empty_runtime() -> (EncryptedSkillRuntime, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = EncryptedSkillRuntime::new(
            std::sync::Arc::new(TestSdk),
            TtlConfig::default(),
            tmp.path().join("mem-root"),
        );
        (runtime, tmp)
    }

    #[test]
    fn guardian_request_redacts_mem_root_paths() {
        let (runtime, _tmp) = loaded_runtime();
        let decrypted_dir = runtime
            .decrypted_dirs("t1")
            .pop()
            .expect("loaded skill should register a decrypted dir");
        let mem_root = runtime.mem_root().to_string_lossy().into_owned();

        let request = GuardianApprovalRequest::Shell {
            id: "approval-1".to_string(),
            command: vec![
                "bash".to_string(),
                decrypted_dir
                    .join("script.sh")
                    .to_string_lossy()
                    .into_owned(),
            ],
            cwd: AbsolutePathBuf::try_from(Path::new("/tmp")).expect("absolute cwd"),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: None,
        };

        let GuardianApprovalRequest::Shell { command, .. } =
            redact_guardian_request(&runtime, "t1", request)
        else {
            panic!("expected Shell request after redaction");
        };
        let joined = command.join(" ");
        assert!(
            !joined.contains(&mem_root),
            "reviewer command must not contain the memory root: {joined}"
        );
        assert!(
            joined.contains("/skills/script.sh"),
            "decrypted dir should be rewritten back to the original skill path: {joined}"
        );
    }

    #[test]
    fn redacting_tool_output_passes_through_when_unengaged() {
        let (runtime, _tmp) = empty_runtime();
        let runtime = Arc::new(runtime);
        let output = RedactingToolOutput {
            inner: Box::new(FunctionToolOutput::from_text(
                "unused".to_string(),
                Some(true),
            )),
            runtime: Arc::clone(&runtime),
            session_id: "t1".to_string(),
            engaged: false,
        };
        let input = format!("script at {}", runtime.mem_root().to_string_lossy());
        assert_eq!(output.redact_text(&input), input);
    }

    #[test]
    fn redacting_tool_output_redacts_when_engaged() {
        let (runtime, _tmp) = loaded_runtime();
        let runtime = Arc::new(runtime);
        let output = RedactingToolOutput {
            inner: Box::new(FunctionToolOutput::from_text(
                "unused".to_string(),
                Some(true),
            )),
            runtime: Arc::clone(&runtime),
            session_id: "t1".to_string(),
            engaged: true,
        };
        let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();
        let input = format!("script at {}", decrypted.join("run.sh").to_string_lossy());
        let redacted = output.redact_text(&input);
        assert!(!redacted.contains(&runtime.mem_root().to_string_lossy().to_string()));
        assert!(
            redacted.contains("/skills/run.sh"),
            "decrypted dir should unrewrite to original skill path: {redacted}"
        );
    }

    #[test]
    fn redacting_tool_output_log_preview_is_redacted_when_engaged() {
        let (runtime, _tmp) = loaded_runtime();
        let runtime = Arc::new(runtime);
        let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();
        let raw_preview = format!("preview {}", decrypted.join("run.sh").to_string_lossy());
        let output = RedactingToolOutput {
            inner: Box::new(FunctionToolOutput::from_text(raw_preview, Some(true))),
            runtime: Arc::clone(&runtime),
            session_id: "t1".to_string(),
            engaged: true,
        };
        let preview = output.log_preview();
        assert!(
            !preview.contains(&runtime.mem_root().to_string_lossy().to_string()),
            "telemetry preview must not contain the memory root: {preview}"
        );
        assert!(
            preview.contains("/skills/run.sh"),
            "decrypted dir should unrewrite to the original skill path in preview: {preview}"
        );
    }
}
