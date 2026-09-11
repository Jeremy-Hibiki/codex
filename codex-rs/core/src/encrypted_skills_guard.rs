//! Thin host adapter around `fm_encrypted_skills::guard`.
//!
//! All interception and redaction policy lives in the `fm-encrypted-skills`
//! crate. This module maps core's tool identities and output types onto that
//! boundary, so upstream merges only need to keep this single adapter in sync.

use std::sync::Arc;

use codex_protocol::models::ResponseInputItem;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use fm_encrypted_skills::runtime::EncryptedSkillRuntime;
use serde_json::Value;

pub(crate) use fm_encrypted_skills::guard::GuardDecision;
pub(crate) use fm_encrypted_skills::guard::redact_storage_paths;

use crate::guardian::GuardianApprovalRequest;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;

/// Wraps a tool output so every surface it feeds (preview, response item,
/// telemetry, code-mode result) has decrypted paths rewritten away.
pub(crate) struct RedactingToolOutput {
    pub(crate) inner: Box<dyn ToolOutput>,
    pub(crate) runtime: Arc<EncryptedSkillRuntime>,
    pub(crate) session_id: String,
    pub(crate) engaged: bool,
}

/// Rewrites decrypted storage paths out of `text` for an engaged session.
///
/// Free function form so callers that only need the preview path (telemetry)
/// can redact without constructing a [`RedactingToolOutput`].
pub(crate) fn redact_text_for(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    text: &str,
) -> String {
    redact_storage_paths(runtime, session_id, text)
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
    fn log_output(&self) -> String {
        self.redact_text(&self.inner.log_output())
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
        GuardianApprovalRequest::ExecCommand {
            id,
            environment_id,
            command,
            cwd,
            guardian_cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
            tty,
        } => GuardianApprovalRequest::ExecCommand {
            id,
            environment_id,
            command: redact_args(command),
            cwd: redact_path_uri(runtime, session_id, cwd),
            guardian_cwd: redact_legacy_app_path(runtime, session_id, guardian_cwd),
            sandbox_permissions,
            additional_permissions,
            justification: justification.map(&redact),
            tty,
        },
        GuardianApprovalRequest::WriteStdin {
            id,
            approval_id,
            environment_id,
            process_id,
            input,
            cwd,
            tty,
            sandbox_permissions,
            additional_permissions,
        } => GuardianApprovalRequest::WriteStdin {
            id,
            approval_id,
            environment_id,
            process_id,
            input,
            cwd: redact_path_uri(runtime, session_id, cwd),
            tty,
            sandbox_permissions,
            additional_permissions,
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
            cwd: redact_absolute_path(runtime, session_id, cwd),
            additional_permissions,
        },
        GuardianApprovalRequest::ApplyPatch {
            id,
            cwd,
            files,
            patch,
        } => GuardianApprovalRequest::ApplyPatch {
            id,
            cwd: redact_path_uri(runtime, session_id, cwd),
            files: files
                .into_iter()
                .map(|file| redact_path_uri(runtime, session_id, file))
                .collect(),
            patch,
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
                trigger.cwd = redact_path_uri(runtime, session_id, trigger.cwd);
                trigger.justification = trigger
                    .justification
                    .map(|text| redact_storage_paths(runtime, session_id, &text));
                trigger
            }),
        },
        other => other,
    }
}

/// Rewrites a `PathUri`-typed cwd so decrypted skill locations never reach the
/// reviewer. Falls back to the original value (with a warning) when the
/// rewritten path is empty or cannot be represented as a `PathUri`.
fn redact_path_uri(runtime: &EncryptedSkillRuntime, session_id: &str, uri: PathUri) -> PathUri {
    let native = uri.inferred_native_path_string();
    let redacted = redact_storage_paths(runtime, session_id, &native);
    if redacted == native {
        return uri;
    }
    if redacted.is_empty() {
        tracing::warn!("guardian cwd redaction produced an empty path; keeping original");
        return uri;
    }
    match PathUri::from_host_native_path(&redacted) {
        Ok(redacted) => redacted,
        Err(error) => {
            tracing::warn!(
                %error,
                "guardian cwd redaction produced an unusable path; keeping original"
            );
            uri
        }
    }
}

/// Rewrites a legacy app-path string cwd; the rewritten spelling is always a
/// valid display string, so only an empty rewrite keeps the original.
fn redact_legacy_app_path(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    path: LegacyAppPathString,
) -> LegacyAppPathString {
    let redacted = redact_storage_paths(runtime, session_id, path.as_str());
    if redacted.is_empty() {
        tracing::warn!("guardian cwd redaction produced an empty path; keeping original");
        return path;
    }
    LegacyAppPathString::from_string(redacted)
}

/// Rewrites an absolute-path cwd (Execve); keeps the original (with a warning)
/// when the rewritten path is empty or no longer absolute.
fn redact_absolute_path(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    path: AbsolutePathBuf,
) -> AbsolutePathBuf {
    let native = path.as_path().to_string_lossy().into_owned();
    let redacted = redact_storage_paths(runtime, session_id, &native);
    if redacted == native {
        return path;
    }
    if redacted.is_empty() {
        tracing::warn!("guardian cwd redaction produced an empty path; keeping original");
        return path;
    }
    match AbsolutePathBuf::from_absolute_path_checked(&redacted) {
        Ok(redacted) => redacted,
        Err(error) => {
            tracing::warn!(
                %error,
                "guardian cwd redaction produced an unusable path; keeping original"
            );
            path
        }
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
    use crate::tools::context::ToolPayload;
    use crate::tools::hook_names::HookToolName;

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
    fn guardian_request_redacts_justification_and_cwd_paths() {
        let (runtime, _tmp) = loaded_runtime();
        let decrypted_dir = runtime
            .decrypted_dirs("t1")
            .pop()
            .expect("loaded skill should register a decrypted dir");
        let mem_root = runtime.mem_root().to_string_lossy().into_owned();

        let workspace = AbsolutePathBuf::try_from(decrypted_dir.join("scripts").as_path())
            .expect("absolute workspace");
        let request = GuardianApprovalRequest::ExecCommand {
            id: "approval-2".to_string(),
            environment_id: "local".to_string(),
            command: vec!["bash".to_string()],
            cwd: codex_utils_path_uri::PathUri::from_abs_path(&workspace),
            guardian_cwd: codex_utils_path_uri::LegacyAppPathString::from_abs_path(&workspace),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some(format!(
                "need to read {}",
                decrypted_dir.join("SKILL.md").display()
            )),
            tty: false,
        };

        let GuardianApprovalRequest::ExecCommand {
            cwd,
            guardian_cwd,
            justification,
            ..
        } = redact_guardian_request(&runtime, "t1", request)
        else {
            panic!("expected ExecCommand request after redaction");
        };

        let cwd_text = cwd.to_string();
        assert!(
            !cwd_text.contains(mem_root.as_str()),
            "cwd must not contain the memory root: {cwd_text}"
        );
        assert!(
            cwd_text.contains("/skills/scripts"),
            "decrypted cwd should be rewritten back to the original skill path: {cwd_text}"
        );
        let guardian_cwd = guardian_cwd.to_string();
        assert!(
            !guardian_cwd.contains(mem_root.as_str()),
            "guardian_cwd must not contain the memory root: {guardian_cwd}"
        );
        assert!(
            guardian_cwd.contains("/skills/scripts"),
            "decrypted guardian_cwd should be rewritten to the original skill path: {guardian_cwd}"
        );
        let justification = justification.expect("justification should be preserved");
        assert!(
            !justification.contains(mem_root.as_str()),
            "justification must not contain the memory root: {justification}"
        );
        assert!(
            justification.contains("/skills/SKILL.md"),
            "decrypted path in justification should be rewritten: {justification}"
        );
    }

    #[test]
    fn guardian_request_redacts_mem_root_paths() {
        let (runtime, _tmp) = loaded_runtime();
        let decrypted_dir = runtime
            .decrypted_dirs("t1")
            .pop()
            .expect("loaded skill should register a decrypted dir");
        let mem_root = runtime.mem_root().to_string_lossy().into_owned();

        let cwd = AbsolutePathBuf::try_from(Path::new("/tmp")).expect("absolute cwd");
        let request = GuardianApprovalRequest::ExecCommand {
            id: "approval-1".to_string(),
            environment_id: "local".to_string(),
            command: vec![
                "bash".to_string(),
                decrypted_dir
                    .join("script.sh")
                    .to_string_lossy()
                    .into_owned(),
            ],
            cwd: codex_utils_path_uri::PathUri::from_abs_path(&cwd),
            guardian_cwd: codex_utils_path_uri::LegacyAppPathString::from_abs_path(&cwd),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: None,
            tty: false,
        };

        let GuardianApprovalRequest::ExecCommand { command, .. } =
            redact_guardian_request(&runtime, "t1", request)
        else {
            panic!("expected ExecCommand request after redaction");
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
    fn redacting_tool_output_log_output_is_redacted_when_engaged() {
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
        let preview = output.log_output();
        assert!(
            !preview.contains(&runtime.mem_root().to_string_lossy().to_string()),
            "telemetry preview must not contain the memory root: {preview}"
        );
        assert!(
            preview.contains("/skills/run.sh"),
            "decrypted dir should unrewrite to the original skill path in preview: {preview}"
        );
    }

    #[test]
    fn redacting_tool_output_response_item_is_redacted_when_engaged() {
        let (runtime, _tmp) = loaded_runtime();
        let runtime = Arc::new(runtime);
        let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();
        let raw = format!(
            "script at {}/run.sh",
            decrypted.join("run.sh").to_string_lossy()
        );
        let output = RedactingToolOutput {
            inner: Box::new(FunctionToolOutput::from_text(raw, Some(true))),
            runtime: Arc::clone(&runtime),
            session_id: "t1".to_string(),
            engaged: true,
        };

        let item = output.to_response_item(
            "call-1",
            &ToolPayload::Function {
                arguments: "{}".to_string(),
            },
        );
        let ResponseInputItem::FunctionCallOutput { output, .. } = item else {
            panic!("expected function call output item");
        };
        let text = output.body.to_text().unwrap();
        assert!(
            !text.contains(&runtime.mem_root().to_string_lossy().to_string()),
            "response item must not contain the memory root: {text}"
        );
        assert!(
            text.contains("[REDACTED]"),
            "decrypted path should be redacted in the response item: {text}"
        );
    }

    #[test]
    fn before_tool_maps_hook_names_to_guarded_tools() {
        let (runtime, _tmp) = loaded_runtime();
        for tool_name in [HookToolName::bash().name(), "exec_command"] {
            let decision = fm_encrypted_skills::guard::before_tool(
                &runtime,
                "t1",
                tool_name,
                &serde_json::json!({ "command": "cat /skills/secret/SKILL.md" }),
            );
            assert!(
                matches!(decision, GuardDecision::Blocked { .. }),
                "shell read via {tool_name} must be blocked: {decision:?}"
            );
        }
    }
}
