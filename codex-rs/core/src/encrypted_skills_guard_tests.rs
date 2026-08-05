use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_encrypted_skills::audit::AuditEvent;
use codex_encrypted_skills::audit::AuditSink;
use codex_encrypted_skills::registry::TtlConfig;
use codex_encrypted_skills::runtime::EncryptedSkillRuntime;
use codex_encrypted_skills::sdk::EnvelopeError;
use codex_encrypted_skills::sdk::EnvelopeSdk;
use codex_encrypted_skills::sdk::PackageEntry;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::TurnItem;
use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use serde_json::json;

use super::*;
use crate::tools::hook_names::HookToolName;

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
fn blocks_cat_script_under_mem_root() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": "cat /dev/shm/fm-agent-security/fm_skill_security_abc/scripts/run.sh" }),
    );
    assert!(matches!(decision, GuardDecision::Blocked { .. }));
}

#[test]
fn blocks_cat_script_referencing_original_skill_paths() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": "cat /skills/secret/scripts/run.sh" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "original-path script read must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_cat_text_file_referencing_original_skill_paths() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": "cat /skills/secret/SKILL.md" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "reading SKILL.md from decrypted storage must be blocked: {decision:?}"
    );
}

#[test]
fn recursive_grep_on_decrypted_dir_is_blocked() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("grep -r secret {}", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "recursive grep into decrypted storage must be blocked: {decision:?}"
    );
}

#[test]
fn recursive_grep_on_original_dir_is_blocked() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": "grep -r secret /skills/secret" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "recursive grep referencing original skill dir must be blocked: {decision:?}"
    );
}

#[test]
fn grep_specific_script_file_is_blocked() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("grep secret {}/scripts/run.sh", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(matches!(decision, GuardDecision::Blocked { .. }));
}

#[test]
fn blocks_listing_decrypted_dir() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": format!("ls {}", dirs[0].to_string_lossy()) }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "listing the decrypted dir must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_find_in_decrypted_dir() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("find {} -name '*.md'", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "find in the decrypted dir must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_cat_text_file_in_decrypted_dir() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("cat {}/notes.md", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "text reads from decrypted storage must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_cat_script_in_decrypted_dir() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("cat {}/scripts/run.sh", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "script reads must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_copy_of_script_file() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!(
        "cp {}/scripts/run.sh /tmp/leak.sh",
        dirs[0].to_string_lossy()
    );
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "copying a script file out must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_archive_of_script_file() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("tar cf - {}/scripts/run.sh", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(matches!(decision, GuardDecision::Blocked { .. }));
}

#[test]
fn blocks_bash_c_inner_script_read() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("bash -c 'cat {}/scripts/run.sh'", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "bash -c wrapping a script read must be blocked: {decision:?}"
    );
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
    ] {
        let decision = before_tool_with_runtime(
            &runtime,
            "t1",
            &HookToolName::bash(),
            &json!({ "command": command }),
        );
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "modern listing command on decrypted storage must be blocked: {command}"
        );
    }
}

#[test]
fn blocks_modern_text_reader_on_decrypted_storage() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let text = format!("bat {}/SKILL.md", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": text }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "bat on decrypted storage must be blocked: {decision:?}"
    );
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
fn blocks_chain_smuggled_read_after_script_execution() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": "bash /skills/secret/scripts/build.sh; cat /skills/secret/SKILL.md" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "chain-smuggled read after execution must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_chain_smuggled_read_via_logical_and() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": "bash /skills/secret/scripts/build.sh && cat /skills/secret/SKILL.md" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "chain-smuggled read via && must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_pipe_smuggled_read() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": "bash /skills/secret/scripts/build.sh | cat /skills/secret/SKILL.md" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "pipe-smuggled read must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_glob_probing_decrypted_storage() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
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
        &HookToolName::bash(),
        &json!({ "command": "cat /dev/shm/fm-agent-security/p99999/fm_skill_security_deadbeef/SKILL.md" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "cross-session directory access must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_copy_escape_from_decrypted_storage() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("cp {}/SKILL.md /tmp/leak.md", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "copying plaintext out of decrypted storage must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_redirect_escape_from_decrypted_storage() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("cat {}/SKILL.md > /tmp/leak.md", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "redirecting plaintext out of decrypted storage must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_base64_escape_from_decrypted_storage() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("base64 {}/SKILL.md", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "base64-encoding plaintext out of decrypted storage must be blocked: {decision:?}"
    );
}

#[test]
fn allows_script_execution_in_decrypted_storage() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("bash {}/scripts/build.sh", dirs[0].to_string_lossy());
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": command }),
    );
    assert!(
        matches!(decision, GuardDecision::Updated(_) | GuardDecision::Allow),
        "script execution in decrypted storage must be allowed: {decision:?}"
    );
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
    assert!(matches!(decision, GuardDecision::Blocked { .. }));
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
    assert!(matches!(decision, GuardDecision::Blocked { .. }));
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
fn redacts_tool_output_text_plaintext_for_durable_surfaces() {
    let (runtime, _tmp) = loaded_runtime();
    let item = ResponseItem::FunctionCallOutput {
        id: None,
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::Text("the skill says: # Guarded content".to_string()),
            ..Default::default()
        },
        internal_chat_message_metadata_passthrough: None,
    };

    let redacted = redact_tool_output_plaintext_for_persistence(&runtime, "t1", item);

    let ResponseItem::FunctionCallOutput { output, .. } = redacted else {
        panic!("expected function call output");
    };
    assert_eq!(
        output.body,
        FunctionCallOutputBody::Text("the skill says: [REDACTED]".to_string())
    );
}

#[test]
fn redacts_tool_output_content_items_for_durable_surfaces() {
    let (runtime, _tmp) = loaded_runtime();
    let item = ResponseItem::CustomToolCallOutput {
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

    let redacted = redact_tool_output_plaintext_for_persistence(&runtime, "t1", item);

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
fn redact_turn_item_leaves_user_messages_untouched() {
    let (runtime, _tmp) = loaded_runtime();
    let item = TurnItem::UserMessage(codex_protocol::items::UserMessageItem {
        id: "user-1".to_string(),
        client_id: None,
        content: vec![],
    });

    let redacted = redact_turn_item(&runtime, "t1", item);

    assert!(matches!(redacted, TurnItem::UserMessage(_)));
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
    assert!(matches!(decision, GuardDecision::Blocked { .. }));
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
fn allows_mcp_tool_without_storage_references() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::new("mcp__server__tool"),
        &json!({ "path": "/tmp/unrelated.txt" }),
    );
    assert!(matches!(decision, GuardDecision::Allow));
}

#[test]
fn blocks_mcp_tool_probing_mem_root_path() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::new("mcp__server__tool"),
        &json!({ "path": "/dev/shm/fm-agent-security/whatever" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "MCP path probe of the memory root must be blocked: {decision:?}"
    );
}

#[test]
fn blocks_extension_tool_probing_decrypted_dir() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::new("some_extension_tool"),
        &json!({ "directory": dirs[0].to_string_lossy() }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "extension tool probing a decrypted dir must be blocked: {decision:?}"
    );
}

#[test]
fn blocked_path_probe_records_storage_probe_reason() {
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

    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::new("mcp__server__tool"),
        &json!({ "path": "/dev/shm/fm-agent-security/whatever" }),
    );
    assert!(matches!(decision, GuardDecision::Blocked { .. }));
    assert!(sink.events().iter().any(|event| matches!(
        event,
        AuditEvent::Blocked { reason, .. } if reason == "storage_probe"
    )));
}

#[test]
fn blocked_audit_records_specific_reason() {
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
        .expect("load skill");

    let _ = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::bash(),
        &json!({ "command": "cat /dev/shm/fm-agent-security/x/scripts/run.sh" }),
    );
    let _ = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::apply_patch(),
        &json!({ "command": "patch with # Guarded content" }),
    );

    let events = sink.events();
    assert!(events.iter().any(|event| matches!(
        event,
        AuditEvent::Blocked { reason, .. } if reason == "non_execution_access"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        AuditEvent::Blocked { reason, .. } if reason == "export_plaintext"
    )));
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
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "listing the memory root must be blocked: {decision:?}"
    );
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
    assert!(matches!(decision, GuardDecision::Blocked { .. }));
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

#[test]
fn blocks_web_search_tool_containing_known_plaintext() {
    let (runtime, _tmp) = loaded_runtime();
    // Standalone web search extension: namespace `web` + tool `run` flattens
    // to the hook payload name `webrun`.
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::new("webrun"),
        &json!({ "query": "# Guarded content" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "web search with skill plaintext must be blocked: {decision:?}"
    );
}

#[test]
fn allows_web_search_tool_without_known_plaintext() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::new("webrun"),
        &json!({ "query": "rust async trait" }),
    );
    assert!(matches!(decision, GuardDecision::Allow));
}

#[test]
fn blocks_extension_tool_args_containing_known_plaintext() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        &HookToolName::new("some_extension_tool"),
        &json!({ "payload": "prefix # Guarded content suffix" }),
    );
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "extension tool with skill plaintext must be blocked: {decision:?}"
    );
}
