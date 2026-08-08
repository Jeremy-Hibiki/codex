use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::audit::AuditEvent;
use crate::audit::AuditSink;
use crate::registry::TtlConfig;
use crate::runtime::EncryptedSkillRuntime;
use crate::sdk::EnvelopeError;
use crate::sdk::EnvelopeSdk;
use crate::sdk::PackageEntry;
use codex_protocol::ThreadId;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::CollabAgentTool;
use codex_protocol::items::CollabAgentToolCallItem;
use codex_protocol::items::CollabAgentToolCallStatus;
use codex_protocol::items::CommandExecutionItem;
use codex_protocol::items::CommandExecutionStatus;
use codex_protocol::items::DynamicToolCallItem;
use codex_protocol::items::DynamicToolCallStatus;
use codex_protocol::items::FileChangeItem;
use codex_protocol::items::McpToolCallError;
use codex_protocol::items::McpToolCallItem;
use codex_protocol::items::McpToolCallStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::items::WebSearchItem;
use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::WebSearchAction;
use codex_protocol::protocol::ExecCommandSource;
use codex_utils_path_uri::PathUri;
use serde_json::json;

use super::*;

#[derive(Default)]
struct GuardCollectingSink {
    events: std::sync::Mutex<Vec<AuditEvent>>,
}

impl AuditSink for GuardCollectingSink {
    fn emit(&self, event: AuditEvent) {
        self.events.lock().unwrap().push(event);
    }
}

impl GuardCollectingSink {
    fn events(&self) -> Vec<AuditEvent> {
        self.events.lock().unwrap().clone()
    }
}

struct GuardTestSdk;

impl EnvelopeSdk for GuardTestSdk {
    fn decrypt_package(&self, _package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        Ok(vec![PackageEntry {
            rel_path: PathBuf::from("SKILL.md"),
            contents: b"# Guarded content".to_vec(),
        }])
    }
}

struct GuardLongTestSdk;

impl EnvelopeSdk for GuardLongTestSdk {
    fn decrypt_package(&self, _package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        Ok(vec![PackageEntry {
            rel_path: PathBuf::from("SKILL.md"),
            contents: b"line one\nabcdefghijklmnopqrstuvwxyzABCDEFGHIJ\nline three".to_vec(),
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

fn empty_runtime() -> (EncryptedSkillRuntime, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    (
        EncryptedSkillRuntime::new(
            Arc::new(GuardTestSdk),
            TtlConfig::default(),
            tmp.path().join("mem-root"),
        ),
        tmp,
    )
}

fn loaded_long_runtime() -> (EncryptedSkillRuntime, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new(
        Arc::new(GuardLongTestSdk),
        TtlConfig::default(),
        tmp.path().join("mem-root"),
    );
    runtime
        .load_or_register(
            "t1",
            "long-secret",
            Path::new("/skills/long-secret.zip.enc"),
        )
        .unwrap();
    (runtime, tmp)
}

#[test]
fn binds_active_logical_script_execution_is_allowed() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool(
        &runtime,
        "t1",
        BASH_TOOL_NAME,
        &json!({ "command": "bash /skills/run.sh" }),
        /*binds_active*/ true,
    );
    assert!(
        matches!(decision, GuardDecision::Allow),
        "logical script execution must be allowed with binds active: {decision:?}"
    );
}

#[test]
fn binds_active_logical_read_is_blocked() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool(
        &runtime,
        "t1",
        BASH_TOOL_NAME,
        &json!({ "command": "cat /skills/SKILL.md" }),
        /*binds_active*/ true,
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "logical read must be blocked with binds active: {decision:?}"
    );
}

#[test]
fn legacy_rewrite_without_binds_rewrites_to_decrypted_path() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool(
        &runtime,
        "t1",
        BASH_TOOL_NAME,
        &json!({ "command": "bash /skills/run.sh" }),
        /*binds_active*/ false,
    );
    let GuardDecision::Updated(updated) = decision else {
        panic!("expected Updated command without binds: {decision:?}");
    };
    let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();
    assert!(
        updated["command"]
            .as_str()
            .is_some_and(|command| command.contains(&decrypted.to_string_lossy().to_string())),
        "legacy mode must rewrite to the decrypted path: {updated:?}"
    );
}

#[test]
fn stdin_input_guards_original_and_decrypted_paths() {
    let (runtime, _tmp) = loaded_runtime();
    let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();
    let cases = [
        ("cat /skills/SKILL.md".to_string(), true),
        (
            format!("cat {}/SKILL.md", decrypted.to_string_lossy()),
            true,
        ),
        ("ls /tmp".to_string(), false),
        ("bash /skills/run.sh".to_string(), false),
    ];
    for (chars, blocked) in cases {
        let decision = guard_stdin_input(&runtime, "t1", &chars);
        if blocked {
            assert!(
                matches!(
                    decision,
                    GuardDecision::Blocked {
                        reason: "non_execution_access",
                        ..
                    }
                ),
                "stdin path read must be blocked: {chars}"
            );
        } else {
            assert!(
                matches!(decision, GuardDecision::Allow),
                "harmless stdin input must be allowed: {chars}"
            );
        }
    }
}

#[test]
fn stdin_input_smuggled_read_after_script_is_blocked() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = guard_stdin_input(&runtime, "t1", "bash /skills/run.sh; cat /skills/SKILL.md");
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "smuggled read after a script must be blocked: {decision:?}"
    );
}

#[test]
fn stdin_input_unengaged_session_is_allowed() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = guard_stdin_input(&runtime, "other", "cat /skills/SKILL.md");
    assert!(
        matches!(decision, GuardDecision::Allow),
        "unengaged session stdin must pass through: {decision:?}"
    );
}

#[test]
fn is_skill_script_execution_detects_execute_only_commands() {
    let (runtime, _tmp) = loaded_runtime();
    assert!(crate::guard::is_skill_script_execution(
        &runtime,
        "t1",
        "bash /skills/run.sh"
    ));
    assert!(!crate::guard::is_skill_script_execution(
        &runtime,
        "t1",
        "cat /skills/SKILL.md"
    ));
    assert!(!crate::guard::is_skill_script_execution(
        &runtime,
        "t1",
        "git status"
    ));
}

#[test]
fn is_skill_script_execution_false_when_unengaged() {
    let (runtime, _tmp) = empty_runtime();
    assert!(!crate::guard::is_skill_script_execution(
        &runtime,
        "t1",
        "bash /skills/run.sh"
    ));
}

#[test]
fn blocks_reads_of_guarded_paths() {
    let (runtime, _tmp) = loaded_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    let mut cases: Vec<String> = vec![
        "cat /dev/shm/fm-agent-security/fm_skill_security_abc/scripts/run.sh",
        "cat /skills/secret/scripts/run.sh",
        "cat /skills/secret/SKILL.md",
        "grep -r secret /dev/shm/fm-agent-security/fm_skill_security_abc",
        "grep -r secret /skills/secret",
        "cat /dev/shm/fm-agent-security/fm_skill_security_abc/notes.md",
        "cat /dev/shm/fm-agent-security/fm_skill_security_abc/scripts/run.sh",
        "ls /dev/shm/fm-agent-security/fm_skill_security_abc",
        "find /dev/shm/fm-agent-security/fm_skill_security_abc -name '*.md'",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    cases.push(format!("grep secret {dir}/scripts/run.sh"));
    cases.push(format!("cat {dir}/notes.md"));
    cases.push(format!("cat {dir}/scripts/run.sh"));
    cases.push(format!("ls {dir}"));
    cases.push(format!("find {dir} -name '*.md'"));

    for command in cases {
        let decision = before_tool_with_runtime(
            &runtime,
            "t1",
            BASH_TOOL_NAME,
            &json!({ "command": command }),
        );
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "guarded path read must be blocked: {command}"
        );
    }
}

#[test]
fn blocks_modern_listing_commands() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    for command in [
        format!("lsd {}", dirs[0].to_string_lossy()),
        format!("eza {}", dirs[0].to_string_lossy()),
        format!("tree {}", dirs[0].to_string_lossy()),
        format!("fd . {}", dirs[0].to_string_lossy()),
        format!("ls {}", runtime.mem_root().to_string_lossy()),
    ] {
        let decision = before_tool_with_runtime(
            &runtime,
            "t1",
            BASH_TOOL_NAME,
            &json!({ "command": command }),
        );
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "listing command on decrypted storage must be blocked: {command}"
        );
    }
}

#[test]
fn blocks_bash_c_inner_script_read() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("bash -c 'cat {}/scripts/run.sh'", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        BASH_TOOL_NAME,
        &json!({ "command": command }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "bash -c wrapping a script read must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_exfil_and_modern_reader_commands() {
    let (runtime, _tmp) = loaded_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    for command in [
        format!("cp {dir}/scripts/run.sh /tmp/leak.sh"),
        format!("tar cf - {dir}/scripts/run.sh"),
        format!("cp {dir}/SKILL.md /tmp/leak.md"),
        format!("cat {dir}/SKILL.md > /tmp/leak.md"),
        format!("base64 {dir}/SKILL.md"),
        format!("bat {dir}/SKILL.md"),
    ] {
        let decision = before_tool_with_runtime(
            &runtime,
            "t1",
            BASH_TOOL_NAME,
            &json!({ "command": command }),
        );
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "exfil/modern-read command on decrypted storage must be blocked: {command}"
        );
    }
}

#[test]
fn rewrites_original_skill_path_in_execution_commands() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        BASH_TOOL_NAME,
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
fn blocks_chain_smuggled_reads() {
    let (runtime, _tmp) = loaded_runtime();
    for command in [
        "bash /skills/secret/scripts/build.sh; cat /skills/secret/SKILL.md",
        "bash /skills/secret/scripts/build.sh && cat /skills/secret/SKILL.md",
        "bash /skills/secret/scripts/build.sh | cat /skills/secret/SKILL.md",
    ] {
        let decision = before_tool_with_runtime(
            &runtime,
            "t1",
            BASH_TOOL_NAME,
            &json!({ "command": command }),
        );
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "chain-smuggled read must be blocked: {command}"
        );
    }
}

#[test]
fn blocks_glob_probing_decrypted_storage() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        BASH_TOOL_NAME,
        &json!({ "command": "cat /dev/shm/fm-agent-security/p*/f*/SKILL.md" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "glob probing of decrypted storage must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_cross_session_directory_access() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        BASH_TOOL_NAME,
        &json!({ "command": "cat /dev/shm/fm-agent-security/p99999/fm_skill_security_deadbeef/SKILL.md" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "cross-session directory access must be blocked: {decision:?}"
    );
}

#[test]
fn allows_script_execution_in_decrypted_storage() {
    let (runtime, _tmp) = loaded_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    for command in [
        format!("bash {dir}/scripts/build.sh"),
        format!("bash {dir}/scripts/build.sh --input {dir}/resources/config.json"),
    ] {
        let decision = before_tool_with_runtime(
            &runtime,
            "t1",
            BASH_TOOL_NAME,
            &json!({ "command": command }),
        );
        assert!(
            matches!(decision, GuardDecision::Updated(_) | GuardDecision::Allow),
            "script execution with plain guarded arguments must stay allowed: {command}"
        );
    }
}

#[test]
fn blocks_script_execution_reading_through_io_channels() {
    let (runtime, _tmp) = loaded_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    let cases = [
        format!("bash {dir}/scripts/build.sh < {dir}/SKILL.md"),
        format!("bash {dir}/scripts/build.sh \"$(cat {dir}/SKILL.md)\""),
        format!("bash {dir}/scripts/build.sh `cat {dir}/SKILL.md`"),
        format!("bash {dir}/scripts/build.sh <(cat {dir}/SKILL.md)"),
        format!("bash {dir}/scripts/build.sh <<< \"$(cat {dir}/SKILL.md)\""),
    ];
    for command in cases {
        let decision = before_tool_with_runtime(
            &runtime,
            "t1",
            BASH_TOOL_NAME,
            &json!({ "command": command }),
        );
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "script execution with a guarded io channel must be blocked: {decision:?}"
        );
    }
}

#[test]
fn guards_view_image_paths() {
    let (runtime, _tmp) = loaded_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    let root = runtime.mem_root().to_string_lossy().into_owned();
    for path in [
        format!("{dir}/image.png"),
        format!("{root}/whatever/image.png"),
        "/dev/shm/fm-agent-security/other/image.png".to_string(),
    ] {
        let decision = before_tool_with_runtime(
            &runtime,
            "t1",
            VIEW_IMAGE_TOOL_NAME,
            &json!({ "path": path }),
        );
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "view_image under the mem root must be blocked: {path}"
        );
    }
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        VIEW_IMAGE_TOOL_NAME,
        &json!({ "path": "/tmp/workspace/image.png" }),
    );
    assert!(matches!(decision, GuardDecision::Allow));
}

#[test]
fn redacts_assistant_reply_plaintext_before_persistence() {
    let (runtime, _tmp) = loaded_runtime();
    let reply = ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![
            ContentItem::InputText {
                text: "reasoning about # Guarded content".to_string(),
            },
            ContentItem::OutputText {
                text: "the skill says: # Guarded content".to_string(),
            },
        ],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    let items = std::borrow::Cow::Owned(vec![reply]);

    let redacted = redact_assistant_reply_items(&runtime, "t1", items);

    let ResponseItem::Message { content, .. } = &redacted[0] else {
        panic!("expected message item");
    };
    assert_eq!(
        content,
        &[
            ContentItem::InputText {
                text: "reasoning about [REDACTED]".to_string(),
            },
            ContentItem::OutputText {
                text: "the skill says: [REDACTED]".to_string(),
            },
        ]
    );
}

#[test]
fn redacts_only_assistant_reply_items() {
    let (runtime, _tmp) = loaded_runtime();
    let user_item = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "user says: # Guarded content".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    let items = std::borrow::Cow::Owned(vec![user_item]);

    let redacted = redact_assistant_reply_items(&runtime, "t1", items);

    let ResponseItem::Message { content, .. } = &redacted[0] else {
        panic!("expected message item");
    };
    assert_eq!(
        content,
        &[ContentItem::InputText {
            text: "user says: # Guarded content".to_string(),
        }]
    );
}

#[test]
fn redacts_reasoning_response_item_text() {
    let (runtime, _tmp) = loaded_runtime();
    let reasoning = ResponseItem::Reasoning {
        id: None,
        summary: vec![ReasoningItemReasoningSummary::SummaryText {
            text: "summary: # Guarded content".to_string(),
        }],
        content: Some(vec![
            ReasoningItemContent::ReasoningText {
                text: "raw: # Guarded content".to_string(),
            },
            ReasoningItemContent::Text {
                text: "plain: # Guarded content".to_string(),
            },
        ]),
        encrypted_content: None,
        internal_chat_message_metadata_passthrough: None,
    };

    let redacted = redact_assistant_reply_item(&runtime, "t1", reasoning);

    let ResponseItem::Reasoning {
        summary, content, ..
    } = redacted
    else {
        panic!("expected reasoning item");
    };
    assert_eq!(
        summary,
        vec![ReasoningItemReasoningSummary::SummaryText {
            text: "summary: [REDACTED]".to_string(),
        }]
    );
    assert_eq!(
        content,
        Some(vec![
            ReasoningItemContent::ReasoningText {
                text: "raw: [REDACTED]".to_string(),
            },
            ReasoningItemContent::Text {
                text: "plain: [REDACTED]".to_string(),
            },
        ])
    );
}

#[test]
fn redacts_plaintext_in_agent_messages() {
    let (runtime, _tmp) = loaded_runtime();
    let agent_message = ResponseItem::AgentMessage {
        id: None,
        author: "worker".to_string(),
        recipient: "root".to_string(),
        content: vec![
            AgentMessageInputContent::InputText {
                text: "done with # Guarded content".to_string(),
            },
            AgentMessageInputContent::EncryptedContent {
                encrypted_content: "ciphertext".to_string(),
            },
        ],
        internal_chat_message_metadata_passthrough: None,
    };
    let items = std::borrow::Cow::Owned(vec![agent_message]);

    let redacted = redact_assistant_reply_items(&runtime, "t1", items);

    let ResponseItem::AgentMessage { content, .. } = &redacted[0] else {
        panic!("expected agent message item");
    };
    assert_eq!(
        content,
        &[
            AgentMessageInputContent::InputText {
                text: "done with [REDACTED]".to_string(),
            },
            AgentMessageInputContent::EncryptedContent {
                encrypted_content: "ciphertext".to_string(),
            },
        ]
    );
}

#[test]
fn redacts_turn_item_agent_message_text() {
    let (runtime, _tmp) = loaded_runtime();
    let item = TurnItem::AgentMessage(codex_protocol::items::AgentMessageItem {
        id: "msg-1".to_string(),
        content: vec![AgentMessageContent::Text {
            text: "the skill says: # Guarded content".to_string(),
        }],
        phase: None,
        memory_citation: None,
    });

    let redacted = redact_turn_item(&runtime, "t1", item);

    let TurnItem::AgentMessage(agent_message) = redacted else {
        panic!("expected agent message turn item");
    };
    let [AgentMessageContent::Text { text }] = agent_message.content.as_slice() else {
        panic!("expected one text content");
    };
    assert_eq!(text.as_str(), "the skill says: [REDACTED]");
}

#[test]
fn redacts_turn_item_reasoning_text() {
    let (runtime, _tmp) = loaded_runtime();
    let item = TurnItem::Reasoning(codex_protocol::items::ReasoningItem {
        id: "rsn-1".to_string(),
        summary_text: vec!["summary: # Guarded content".to_string()],
        raw_content: vec!["raw: # Guarded content".to_string()],
    });

    let redacted = redact_turn_item(&runtime, "t1", item);

    let TurnItem::Reasoning(reasoning) = redacted else {
        panic!("expected reasoning turn item");
    };
    assert_eq!(reasoning.summary_text, vec!["summary: [REDACTED]"]);
    assert_eq!(reasoning.raw_content, vec!["raw: [REDACTED]"]);
}

#[test]
fn redacts_turn_item_plan_text() {
    let (runtime, _tmp) = loaded_runtime();
    let item = TurnItem::Plan(codex_protocol::items::PlanItem {
        id: "plan-1".to_string(),
        text: "step: # Guarded content".to_string(),
    });

    let redacted = redact_turn_item(&runtime, "t1", item);

    let TurnItem::Plan(plan) = redacted else {
        panic!("expected plan turn item");
    };
    assert_eq!(plan.text.as_str(), "step: [REDACTED]");
}

#[test]
fn redacts_turn_item_command_execution_output() {
    let (runtime, _tmp) = loaded_runtime();
    let decrypted = runtime.mem_root().to_string_lossy();
    let item = TurnItem::CommandExecution(CommandExecutionItem {
        id: "exec-1".to_string(),
        plugin_id: None,
        script_path: None,
        process_id: None,
        command: vec!["bash".to_string(), "run.sh".to_string()],
        cwd: PathUri::from_host_native_path(PathBuf::from("/tmp")).unwrap(),
        parsed_cmd: Vec::new(),
        source: ExecCommandSource::Agent,
        interaction_input: Some("input: # Guarded content".to_string()),
        status: CommandExecutionStatus::Completed,
        stdout: Some("output: # Guarded content".to_string()),
        stderr: Some(format!("error: {decrypted}/p1/skill/SKILL.md")),
        aggregated_output: Some("aggregated: # Guarded content".to_string()),
        exit_code: Some(0),
        duration: None,
        formatted_output: Some(format!("formatted: {decrypted}/p1/skill/SKILL.md")),
    });

    let redacted = redact_turn_item(&runtime, "t1", item);

    let TurnItem::CommandExecution(command_execution) = redacted else {
        panic!("expected command execution turn item");
    };
    assert_eq!(
        command_execution.interaction_input.as_deref(),
        Some("input: [REDACTED]")
    );
    assert_eq!(
        command_execution.stdout.as_deref(),
        Some("output: [REDACTED]")
    );
    assert_eq!(
        command_execution.stderr.as_deref(),
        Some("error: [REDACTED]")
    );
    assert_eq!(
        command_execution.aggregated_output.as_deref(),
        Some("aggregated: [REDACTED]")
    );
    assert_eq!(
        command_execution.formatted_output.as_deref(),
        Some("formatted: [REDACTED]")
    );
}

#[test]
fn redacts_turn_item_file_change_output() {
    let (runtime, _tmp) = loaded_runtime();
    let item = TurnItem::FileChange(FileChangeItem {
        id: "file-1".to_string(),
        changes: HashMap::new(),
        status: None,
        auto_approved: None,
        stdout: Some("stdout: # Guarded content".to_string()),
        stderr: Some("stderr: # Guarded content".to_string()),
    });

    let redacted = redact_turn_item(&runtime, "t1", item);

    let TurnItem::FileChange(file_change) = redacted else {
        panic!("expected file change turn item");
    };
    assert_eq!(file_change.stdout.as_deref(), Some("stdout: [REDACTED]"));
    assert_eq!(file_change.stderr.as_deref(), Some("stderr: [REDACTED]"));
}

#[test]
fn redacts_turn_item_web_search_and_tool_call_text() {
    let (runtime, _tmp) = loaded_runtime();
    let item = TurnItem::WebSearch(WebSearchItem {
        id: "web-1".to_string(),
        query: "query: # Guarded content".to_string(),
        action: WebSearchAction::Search {
            query: None,
            queries: None,
        },
        results: None,
    });
    let redacted = redact_turn_item(&runtime, "t1", item);
    let TurnItem::WebSearch(web_search) = redacted else {
        panic!("expected web search turn item");
    };
    assert_eq!(web_search.query.as_str(), "query: [REDACTED]");

    let item = TurnItem::DynamicToolCall(DynamicToolCallItem {
        id: "dyn-1".to_string(),
        namespace: None,
        tool: "demo".to_string(),
        arguments: serde_json::json!({}),
        status: DynamicToolCallStatus::Failed,
        content_items: Some(vec![
            codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem::InputText {
                text: "content: # Guarded content".to_string(),
            },
        ]),
        success: None,
        error: Some("error: # Guarded content".to_string()),
        duration: None,
    });
    let redacted = redact_turn_item(&runtime, "t1", item);
    let TurnItem::DynamicToolCall(dynamic_tool_call) = redacted else {
        panic!("expected dynamic tool call turn item");
    };
    assert!(
        dynamic_tool_call
            .content_items
            .as_ref()
            .is_some_and(|items| items.iter().any(|item| matches!(
                item,
                codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem::InputText { text }
                    if text == "content: [REDACTED]"
            )))
    );
    assert_eq!(
        dynamic_tool_call.error.as_deref(),
        Some("error: [REDACTED]")
    );

    let item = TurnItem::McpToolCall(McpToolCallItem {
        id: "mcp-1".to_string(),
        server: "server".to_string(),
        tool: "tool".to_string(),
        arguments: serde_json::json!({}),
        connector_id: None,
        mcp_app_resource_uri: None,
        link_id: None,
        app_name: None,
        action_name: None,
        plugin_id: None,
        status: McpToolCallStatus::Failed,
        result: None,
        error: Some(McpToolCallError {
            message: "error: # Guarded content".to_string(),
        }),
        duration: None,
    });
    let redacted = redact_turn_item(&runtime, "t1", item);
    let TurnItem::McpToolCall(mcp_tool_call) = redacted else {
        panic!("expected mcp tool call turn item");
    };
    assert_eq!(
        mcp_tool_call
            .error
            .as_ref()
            .map(|error| error.message.as_str()),
        Some("error: [REDACTED]")
    );
}

#[test]
fn redacts_turn_item_collab_agent_tool_call_prompt() {
    let (runtime, _tmp) = loaded_runtime();
    let item = TurnItem::CollabAgentToolCall(CollabAgentToolCallItem {
        id: "collab-1".to_string(),
        tool: CollabAgentTool::SpawnAgent,
        status: CollabAgentToolCallStatus::InProgress,
        sender_thread_id: ThreadId::default(),
        receiver_thread_ids: Vec::new(),
        receiver_agents: Vec::new(),
        prompt: Some("prompt: # Guarded content".to_string()),
        model: None,
        reasoning_effort: None,
        agents_states: HashMap::new(),
    });

    let redacted = redact_turn_item(&runtime, "t1", item);

    let TurnItem::CollabAgentToolCall(collab) = redacted else {
        panic!("expected collab agent tool call turn item");
    };
    assert_eq!(collab.prompt.as_deref(), Some("prompt: [REDACTED]"));
}

#[test]
fn redacts_tool_output_plaintext_for_durable_surfaces() {
    let (runtime, _tmp) = loaded_runtime();
    let text_item = ResponseItem::FunctionCallOutput {
        id: None,
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::Text("the skill says: # Guarded content".to_string()),
            ..Default::default()
        },
        internal_chat_message_metadata_passthrough: None,
    };

    let redacted = redact_tool_output_plaintext_for_persistence(&runtime, "t1", text_item);

    let ResponseItem::FunctionCallOutput { output, .. } = redacted else {
        panic!("expected function call output");
    };
    assert_eq!(
        output.body,
        FunctionCallOutputBody::Text("the skill says: [REDACTED]".to_string())
    );

    let content_item = ResponseItem::CustomToolCallOutput {
        id: None,
        call_id: "call-1".to_string(),
        name: None,
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::ContentItems(vec![
                codex_protocol::models::FunctionCallOutputContentItem::InputText {
                    text: "notes: # Guarded content".to_string(),
                },
                codex_protocol::models::FunctionCallOutputContentItem::InputText {
                    text: "safe note".to_string(),
                },
            ]),
            ..Default::default()
        },
        internal_chat_message_metadata_passthrough: None,
    };

    let redacted = redact_tool_output_plaintext_for_persistence(&runtime, "t1", content_item);

    let ResponseItem::CustomToolCallOutput { output, .. } = redacted else {
        panic!("expected custom tool output");
    };
    let FunctionCallOutputBody::ContentItems(items) = &output.body else {
        panic!("expected content items");
    };
    assert_eq!(items.len(), 2);
    let codex_protocol::models::FunctionCallOutputContentItem::InputText { text: first } =
        &items[0]
    else {
        panic!("expected input text");
    };
    let codex_protocol::models::FunctionCallOutputContentItem::InputText { text: second } =
        &items[1]
    else {
        panic!("expected input text");
    };
    assert_eq!(first, "notes: [REDACTED]");
    assert_eq!(second, "safe note");
}

#[test]
fn redact_text_redacts_plaintext_and_decrypted_paths() {
    let (runtime, _tmp) = loaded_runtime();
    let mem_root = runtime.mem_root().display().to_string();
    let text =
        format!("hook context: # Guarded content at {mem_root}/fm_skill_security_abc/SKILL.md");
    let out = redact_text(&runtime, "t1", &text);
    assert!(!out.contains("# Guarded content"));
    assert!(!out.contains(&mem_root));
    assert!(out.contains("[REDACTED]"));
}

#[test]
fn persistence_redacts_developer_messages() {
    let (runtime, _tmp) = loaded_runtime();
    let mem_root = runtime.mem_root().display().to_string();
    let item = ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText {
            text: format!("# Guarded content at {mem_root}/fm_skill_security_abc/SKILL.md"),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    let out = redact_tool_output_plaintext_for_persistence(&runtime, "t1", item);
    let ResponseItem::Message { content, .. } = out else {
        panic!("expected message item");
    };
    let text = content
        .iter()
        .find_map(|item| match item {
            ContentItem::InputText { text } => Some(text.as_str()),
            _ => None,
        })
        .unwrap();
    assert!(!text.contains("# Guarded content"));
    assert!(!text.contains(&mem_root));
    assert!(text.contains("[REDACTED]"));
}

#[test]
fn redact_all_response_item_text_redacts_user_and_tool_output_items() {
    let (runtime, _tmp) = loaded_runtime();
    let mem_root = runtime.mem_root().display().to_string();
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "# Guarded content".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text(format!(
                    "# Guarded content at {mem_root}/fm_skill_security_abc"
                )),
                success: None,
            },
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    let out = redact_all_response_item_text(&runtime, "t1", &items);
    let mut texts = Vec::new();
    for item in out {
        match item {
            ResponseItem::Message { content, .. } => {
                for content_item in content {
                    if let ContentItem::InputText { text } = content_item {
                        texts.push(text);
                    }
                }
            }
            ResponseItem::FunctionCallOutput { output, .. } => {
                texts.push(output.body.to_text().unwrap());
            }
            _ => {}
        }
    }
    assert!(texts.iter().all(|text| !text.contains("# Guarded content")));
    assert!(texts.iter().all(|text| !text.contains(&mem_root)));
}

#[test]
fn blocks_shell_command_containing_known_plaintext() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        BASH_TOOL_NAME,
        &json!({ "command": "echo \"# Guarded content\" > /tmp/leak.txt" }),
    );
    assert!(
        matches!(
            decision,
            GuardDecision::Blocked {
                reason: "shell_plaintext",
                ..
            }
        ),
        "shell commands carrying skill plaintext must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_shell_command_with_normalized_plaintext_variant() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        BASH_TOOL_NAME,
        &json!({ "command": "echo '**# Guarded content**'" }),
    );
    assert!(
        matches!(
            decision,
            GuardDecision::Blocked {
                reason: "shell_plaintext",
                ..
            }
        ),
        "normalized variants of skill plaintext must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_sed_and_head_on_decrypted_storage() {
    let (runtime, _tmp) = loaded_long_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    for command in [
        format!("sed -n '1,20p' {dir}/SKILL.md"),
        format!("head -n 20 {dir}/SKILL.md"),
        format!("head -c 40 {dir}/SKILL.md"),
        format!("tail -c 100 {dir}/SKILL.md"),
    ] {
        let decision = before_tool_with_runtime(
            &runtime,
            "t1",
            BASH_TOOL_NAME,
            &json!({ "command": command }),
        );
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "sed/head/tail reads of decrypted storage must be blocked: {decision:?}"
        );
    }
}

#[test]
fn persistence_redacts_function_call_arguments() {
    let (runtime, _tmp) = loaded_runtime();
    let function_call = ResponseItem::FunctionCall {
        id: None,
        name: "shell".to_string(),
        namespace: None,
        arguments: json!({ "command": "echo \"# Guarded content\"" }).to_string(),
        call_id: "call-1".to_string(),
        internal_chat_message_metadata_passthrough: None,
    };
    let out = redact_tool_output_plaintext_for_persistence(&runtime, "t1", function_call);
    let ResponseItem::FunctionCall { arguments, .. } = out else {
        panic!("expected function call item");
    };
    assert!(!arguments.contains("# Guarded content"));
    assert!(arguments.contains("[REDACTED]"));
}

#[test]
fn persistence_redacts_middle_fragments_in_tool_output() {
    let (runtime, _tmp) = loaded_long_runtime();
    let line = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ";
    let middle = &line[8..28];
    let item = ResponseItem::FunctionCallOutput {
        id: None,
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::Text(format!("script printed: {middle}")),
            success: None,
        },
        internal_chat_message_metadata_passthrough: None,
    };
    let out = redact_tool_output_plaintext_for_persistence(&runtime, "t1", item);
    let ResponseItem::FunctionCallOutput { output, .. } = out else {
        panic!("expected function call output item");
    };
    let text = output.body.to_text().unwrap();
    assert!(!text.contains(middle));
    assert!(text.contains("[REDACTED]"));
}

#[test]
fn redact_turn_item_leaves_user_messages_untouched() {
    let (runtime, _tmp) = loaded_runtime();
    let item = TurnItem::UserMessage(codex_protocol::items::UserMessageItem {
        id: "user-1".to_string(),
        client_id: None,
        content: vec![codex_protocol::user_input::UserInput::Text {
            text: "user says: # Guarded content".to_string(),
            text_elements: Vec::new(),
        }],
    });

    let redacted = redact_turn_item(&runtime, "t1", item);

    let TurnItem::UserMessage(user_message) = redacted else {
        panic!("expected user message turn item");
    };
    let [codex_protocol::user_input::UserInput::Text { text, .. }] =
        user_message.content.as_slice()
    else {
        panic!("expected one text content");
    };
    assert_eq!(text.as_str(), "user says: # Guarded content");
}

#[test]
fn unrelated_commands_pass_through() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        BASH_TOOL_NAME,
        &json!({ "command": "echo hello" }),
    );
    assert!(matches!(decision, GuardDecision::Allow));
}

#[test]
fn guards_non_shell_tool_path_probes() {
    let (runtime, _tmp) = loaded_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    let blocked_cases = [
        (
            "mcp__server__tool",
            json!({ "path": "/dev/shm/fm-agent-security/whatever" }),
        ),
        ("some_extension_tool", json!({ "directory": dir })),
    ];
    for (tool_name, input) in blocked_cases {
        let decision = before_tool_with_runtime(&runtime, "t1", tool_name, &input);
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "non-shell tool probing guarded storage must be blocked: {tool_name}: {input}"
        );
    }
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        "mcp__server__tool",
        &json!({ "path": "/tmp/unrelated.txt" }),
    );
    assert!(matches!(decision, GuardDecision::Allow));
}

#[test]
fn blocked_actions_record_specific_audit_reasons() {
    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(GuardCollectingSink::default());
    let runtime = EncryptedSkillRuntime::new_with_audit(
        Arc::new(GuardTestSdk),
        TtlConfig::default(),
        tmp.path().join("mem-root"),
        Some(sink.clone()),
    );
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();

    let _ = before_tool_with_runtime(
        &runtime,
        "t1",
        "mcp__server__tool",
        &json!({ "path": "/dev/shm/fm-agent-security/whatever" }),
    );
    let _ = before_tool_with_runtime(
        &runtime,
        "t1",
        BASH_TOOL_NAME,
        &json!({ "command": "cat /dev/shm/fm-agent-security/x/scripts/run.sh" }),
    );
    let _ = before_tool_with_runtime(
        &runtime,
        "t1",
        "apply_patch",
        &json!({ "command": "patch with # Guarded content" }),
    );

    let events = sink.events();
    assert!(events.iter().any(|event| matches!(
        event,
        AuditEvent::Blocked { reason, .. } if reason == "storage_probe"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        AuditEvent::Blocked { reason, .. } if reason == "non_execution_access"
    )));
    assert!(
        events.iter().any(|event| matches!(
            event,
            AuditEvent::Blocked { reason, .. } if reason == "export_plaintext"
        )),
        "expected export_plaintext audit event, got {events:?}"
    );
}

#[test]
fn guards_export_surfaces_with_known_plaintext() {
    let (runtime, _tmp) = loaded_runtime();
    let blocked_cases = [
        (
            "apply_patch",
            json!({ "command": "*** Begin Patch\n+ # Guarded content\n*** End Patch" }),
        ),
        // Standalone web search extension: namespace `web` + tool `run`
        // flattens to the hook payload name `webrun`.
        ("webrun", json!({ "query": "# Guarded content" })),
        (
            "some_extension_tool",
            json!({ "payload": "prefix # Guarded content suffix" }),
        ),
    ];
    for (tool_name, input) in blocked_cases {
        let decision = before_tool_with_runtime(&runtime, "t1", tool_name, &input);
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "export surface with skill plaintext must be blocked: {tool_name}: {input}"
        );
    }
    let allowed_cases = [
        (
            "apply_patch",
            json!({ "command": "*** Begin Patch\n+ normal user content\n*** End Patch" }),
        ),
        ("webrun", json!({ "query": "rust async trait" })),
    ];
    for (tool_name, input) in allowed_cases {
        let decision = before_tool_with_runtime(&runtime, "t1", tool_name, &input);
        assert!(
            matches!(decision, GuardDecision::Allow),
            "export surface without skill plaintext must pass: {tool_name}: {input}"
        );
    }
}

#[test]
fn binds_active_guards_logical_paths_for_read_and_export() {
    let (runtime, _tmp) = loaded_runtime();
    let logical_path = "/skills/secret/SKILL.md";
    let decision = before_tool(
        &runtime,
        "t1",
        VIEW_IMAGE_TOOL_NAME,
        &json!({ "path": logical_path }),
        /*binds_active*/ true,
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "view_image on the logical skill path must be blocked with binds active: {decision:?}"
    );

    let decision = before_tool(
        &runtime,
        "t1",
        "some_extension_tool",
        &json!({ "directory": "/skills/secret" }),
        /*binds_active*/ true,
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "non-shell tool probing the logical skill path must be blocked with binds active: {decision:?}"
    );
}

#[test]
fn missing_or_non_string_inputs_pass_through() {
    let (runtime, _tmp) = loaded_runtime();
    for input in [json!({}), json!({ "command": 42 })] {
        let decision = before_tool(&runtime, "t1", BASH_TOOL_NAME, &input, false);
        assert!(
            matches!(decision, GuardDecision::Allow),
            "shell input without a string command must pass through: {input}"
        );
    }
    for input in [json!({}), json!({ "path": 42 })] {
        let decision = before_tool(&runtime, "t1", VIEW_IMAGE_TOOL_NAME, &input, false);
        assert!(
            matches!(decision, GuardDecision::Allow),
            "view_image input without a string path must pass through: {input}"
        );
    }
}

#[test]
fn redaction_helpers_pass_through_when_known_plaintext_is_empty() {
    let (runtime, _tmp) = empty_runtime();
    let item = ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::InputText {
            text: "# Guarded content".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    let items = std::borrow::Cow::Owned(vec![item.clone()]);
    let redacted = redact_assistant_reply_items(&runtime, "t1", items);
    assert_eq!(redacted[0], item);

    let output = ResponseItem::FunctionCallOutput {
        id: None,
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::Text("# Guarded content".to_string()),
            ..Default::default()
        },
        internal_chat_message_metadata_passthrough: None,
    };
    assert_eq!(
        redact_tool_output_plaintext_for_persistence(&runtime, "t1", output.clone()),
        output
    );
}

#[test]
fn redact_payload_response_item_and_json_redact_paths() {
    let (runtime, _tmp) = loaded_runtime();
    let path = runtime.decrypted_dirs("t1")[0]
        .join("run.sh")
        .to_string_lossy()
        .into_owned();

    let mut payload = FunctionCallOutputPayload {
        body: FunctionCallOutputBody::Text(format!("script at {path}")),
        ..Default::default()
    };
    redact_payload(&runtime, "t1", &mut payload);
    assert!(!payload.body.to_text().unwrap().contains(&path));

    let mut item = ResponseInputItem::Message {
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: format!("script at {path}"),
        }],
        phase: None,
    };
    redact_response_item(&runtime, "t1", &mut item);
    let ResponseInputItem::Message { content, .. } = item else {
        panic!("expected message input item");
    };
    let ContentItem::InputText { text } = &content[0] else {
        panic!("expected input text");
    };
    assert!(!text.contains(&path));

    let mut value = json!({ "path": format!("script at {path}"), "nested": [path] });
    redact_json(&runtime, "t1", &mut value);
    assert!(!serde_json::to_string(&value).unwrap().contains(&path));
}

#[test]
fn blocks_bypass_shaped_commands() {
    // Corpus inherited from the removed tree-sitter prototype
    // (`ts_paths_tests.rs`): shell shapes that attempt to hide a guarded
    // path behind quoting, substitution, variables, comments, or redirects.
    // The legacy scanner blocks them all via substring/prefix matching.
    let (runtime, _tmp) = loaded_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    let cases = [
        format!("bash -lc \"cat {dir}/SKILL.md\""),
        format!("bash -lc 'cat {dir}/SKILL.md'"),
        format!("cat '{dir}/SKILL.md'"),
        format!("cat \"{dir}/SKILL.md\""),
        format!("cat {dir}/SKILL.md 2>&1"),
        format!("echo x > {dir}/out"),
        format!("cat $(echo {dir}/SKILL.md)"),
        format!("cat `echo {dir}/SKILL.md`"),
        format!("eval cat {dir}/SKILL.md"),
        format!("v={dir}; cat \"$v/SKILL.md\""),
        format!("printf '%s\\n' {dir}/SKILL.md"),
        format!("cat {dir}/SKI\"LL.md\""),
        format!("# {dir}/SKILL.md"),
        format!("cat {dir}/SKILL.md # trailing comment"),
        "cat /dev/shm/fm-agent-securit*/p*/fm_skill_security_abc/SKILL.md".to_string(),
        format!("cd {dir} && cat SKILL.md"),
        format!("sh -c 'cat {dir}/SKILL.md'"),
        format!("python3 -c 'open(\"{dir}/SKILL.md\")'"),
        format!("cat <<'EOF'\n{dir}/SKILL.md\nEOF"),
    ];

    for command in cases {
        let decision = before_tool_with_runtime(
            &runtime,
            "t1",
            BASH_TOOL_NAME,
            &json!({ "command": command }),
        );
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "bypass-shaped command must be blocked: {command}"
        );
    }
}
