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
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
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
fn rewrites_paths_to_decrypted_storage() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool(
        &runtime,
        "t1",
        "exec_command",
        &json!({ "cmd": "bash /skills/run.sh" }),
    );
    let GuardDecision::Updated(updated) = decision else {
        panic!("expected Updated command without binds: {decision:?}");
    };
    let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();
    assert!(
        updated["cmd"]
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
        let decision =
            before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
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
        let decision =
            before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "listing command on decrypted storage must be blocked: {command}"
        );
    }
}

// ---- HIGH-1 ③ cd 链 cwd 状态（guard 层） ----

#[test]
fn blocks_cd_chain_relative_guarded_read() {
    let (runtime, tmp) = loaded_runtime();
    let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();
    let mem_root = runtime.mem_root();
    let rel = decrypted.strip_prefix(mem_root).unwrap();
    let command = format!(
        "cd {} && cd {} && cat {}",
        tmp.path().display(),
        mem_root.file_name().unwrap().to_string_lossy(),
        rel.to_string_lossy(),
    );
    let decision =
        before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "cd-chain relative guarded read must be blocked: {command} → {decision:?}"
    );
}

#[test]
fn blocks_relative_guarded_read_after_unresolvable_cd() {
    let (runtime, _tmp) = loaded_runtime();
    let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();
    let name = decrypted.file_name().unwrap().to_string_lossy();
    let command = format!("cd $WORK && cat {name}/SKILL.md");
    let decision =
        before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "relative guarded read after an unresolvable cd must be blocked: {command} → {decision:?}"
    );
}

#[test]
fn cd_chain_absolute_skill_script_execution_still_allowed() {
    let (runtime, _tmp) = loaded_runtime();
    let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();
    let command = format!(
        "cd /tmp && cd work && bash {}/scripts/run.sh",
        decrypted.to_string_lossy()
    );
    let decision =
        before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
    assert!(
        matches!(decision, GuardDecision::Allow),
        "cd chain followed by an absolute skill script execution must stay allowed: {decision:?}"
    );
}

#[test]
fn cd_chain_relative_skill_script_execution_still_allowed() {
    let (runtime, _tmp) = loaded_runtime();
    let decrypted = runtime.decrypted_dirs("t1").pop().unwrap();
    let name = decrypted.file_name().unwrap().to_string_lossy();
    let command = format!("cd $WORK && python {name}/scripts/run.py");
    let decision =
        before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
    assert!(
        matches!(decision, GuardDecision::Allow),
        "relative skill script execution after an unresolvable cd must stay allowed (D9): {decision:?}"
    );
}

#[test]
fn blocks_bash_c_inner_script_read() {
    let (runtime, _tmp) = loaded_runtime();
    let dirs = runtime.decrypted_dirs("t1");
    let command = format!("bash -c 'cat {}/scripts/run.sh'", dirs[0].to_string_lossy());
    let decision =
        before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
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
        let decision =
            before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
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
        "exec_command",
        &json!({ "cmd": "bash /skills/secret/scripts/build.sh" }),
    );
    match decision {
        GuardDecision::Updated(updated) => {
            let command = updated["cmd"].as_str().expect("rewritten command");
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
        let decision =
            before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "chain-smuggled read must be blocked: {command}"
        );
    }
}

#[test]
fn blocks_guarded_read_inside_subshell_compound() {
    // H1 PoC 1 at the guard layer: a guarded `cd` plus relative read inside a
    // subshell must not slip through the segment layer.
    let (runtime, _tmp) = loaded_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    let command = format!("(cd {dir} && cat scripts/run.py)");
    let decision =
        before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "guarded read inside a subshell must be blocked: {command}"
    );
}

#[test]
fn blocks_guarded_read_inside_if_compound() {
    // H1 PoC 2 at the guard layer: an `if` compound hiding `cd /dev/shm`,
    // chained into a relative read of a decrypted dir's tail component.
    let (runtime, _tmp) = loaded_runtime();
    let decrypted = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    let mem_root = runtime.mem_root().to_string_lossy().into_owned();
    let tail = decrypted
        .strip_prefix(&format!("{mem_root}/"))
        .unwrap_or(decrypted.as_str())
        .to_string();
    let command = format!("if true; then cd {mem_root}; fi && cat {tail}");
    let decision =
        before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
    assert!(
        matches!(decision, GuardDecision::Blocked { .. }),
        "guarded read hidden in an if compound must be blocked: {command}"
    );
}

#[test]
fn blocks_glob_probing_decrypted_storage() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        "exec_command",
        &json!({ "cmd": "cat /dev/shm/fm-agent-security/p*/f*/SKILL.md" }),
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
        "exec_command",
        &json!({ "cmd": "cat /dev/shm/fm-agent-security/p99999/fm_skill_security_deadbeef/SKILL.md" }),
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
        let decision =
            before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
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
        let decision =
            before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
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
fn guards_view_image_mem_root_parent_paths() {
    let (runtime, _tmp) = loaded_runtime();
    for path in [
        "/dev/shm/other/image.png",
        "/dev/shm",
        // 点段别名不含字面 `/dev/shm`，词法归一后必须命中。
        "/dev/./shm/other/image.png",
    ] {
        let decision = before_tool_with_runtime(
            &runtime,
            "t1",
            VIEW_IMAGE_TOOL_NAME,
            &json!({ "path": path }),
        );
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "view_image under the mem-root parent must be blocked: {path}"
        );
    }
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
            call_id: Some("call-1".to_string()),
            name: None,
            namespace: None,
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
        "exec_command",
        &json!({ "cmd": "echo \"# Guarded content\" > /tmp/leak.txt" }),
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
        "exec_command",
        &json!({ "cmd": "echo '**# Guarded content**'" }),
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
        let decision =
            before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "sed/head/tail reads of decrypted storage must be blocked: {decision:?}"
        );
    }
}

#[test]
fn unrelated_commands_pass_through() {
    let (runtime, _tmp) = loaded_runtime();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        "exec_command",
        &json!({ "cmd": "echo hello" }),
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
        "exec_command",
        &json!({ "cmd": "cat /dev/shm/fm-agent-security/x/scripts/run.sh" }),
    );
    let _ = before_tool_with_runtime(
        &runtime,
        "t1",
        "apply_patch",
        &json!({ "cmd": "patch with # Guarded content" }),
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
fn redaction_emits_audit_event_with_skill_names_but_never_content() {
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

    assert_eq!(
        redact_text(&runtime, "t1", "nothing sensitive"),
        "nothing sensitive"
    );
    assert!(
        !sink
            .events()
            .iter()
            .any(|event| matches!(event, AuditEvent::Redaction { .. }))
    );

    let redacted = redact_text(&runtime, "t1", "before # Guarded content after");
    assert!(!redacted.contains("# Guarded content"));
    let events = sink.events();
    assert!(events.iter().any(|event| matches!(
        event,
        AuditEvent::Redaction { skills, surface, .. }
            if skills == &vec!["secret".to_string()] && *surface == "text"
    )));
    assert!(
        !format!("{events:?}").contains("Guarded content"),
        "audit events must never carry skill plaintext, got {events:?}"
    );
}

#[test]
fn streaming_redaction_does_not_emit_per_fragment_audit_events() {
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

    let redacted = redact_text_quiet(&runtime, "t1", "before # Guarded content after");
    assert!(!redacted.contains("# Guarded content"));
    assert!(
        !sink
            .events()
            .iter()
            .any(|event| matches!(event, AuditEvent::Redaction { .. })),
        "streaming redaction must not flood the audit log"
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
fn missing_or_non_string_inputs_pass_through() {
    let (runtime, _tmp) = loaded_runtime();
    for input in [json!({}), json!({ "cmd": 42 }), json!({ "command": 42 })] {
        let decision = before_tool(&runtime, "t1", "exec_command", &input);
        assert!(
            matches!(decision, GuardDecision::Allow),
            "exec input without a string command must pass through: {input}"
        );
    }
    for input in [json!({}), json!({ "path": 42 })] {
        let decision = before_tool(&runtime, "t1", VIEW_IMAGE_TOOL_NAME, &input);
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
fn redacts_mcp_tool_call_and_tool_search_outputs() {
    let (runtime, _tmp) = loaded_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();

    let mut mcp_item = ResponseInputItem::McpToolCallOutput {
        call_id: "call-1".to_string(),
        output: CallToolResult {
            content: vec![json!({ "type": "text", "text": format!("script at {dir}/run.sh") })],
            structured_content: Some(json!({ "path": format!("{dir}/SKILL.md") })),
            is_error: None,
            meta: None,
        },
    };
    redact_response_item(&runtime, "t1", &mut mcp_item);
    let ResponseInputItem::McpToolCallOutput { output, .. } = mcp_item else {
        panic!("expected mcp tool call output");
    };
    let serialized = serde_json::to_string(&output).unwrap();
    assert!(
        !serialized.contains(&dir),
        "mcp tool call output must be redacted: {serialized}"
    );

    let mut search_item = ResponseInputItem::ToolSearchOutput {
        call_id: "search-1".to_string(),
        status: "completed".to_string(),
        execution: "client".to_string(),
        tools: vec![json!({ "description": format!("reads {dir}/SKILL.md") })],
    };
    redact_response_item(&runtime, "t1", &mut search_item);
    let ResponseInputItem::ToolSearchOutput { tools, .. } = search_item else {
        panic!("expected tool search output");
    };
    let serialized = serde_json::to_string(&tools).unwrap();
    assert!(
        !serialized.contains(&dir),
        "tool search output must be redacted: {serialized}"
    );
}

#[test]
fn blocks_bypass_shaped_commands() {
    // Corpus from the tree-sitter differential suite (`paths_tests.rs`):
    // shell shapes that attempt to hide a guarded path behind quoting,
    // substitution, variables, comments, or redirects. The tree-sitter
    // implementation blocks them via semantic literal-token matching.
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
        format!("cat {dir}/SKILL.md # trailing comment"),
        "cat /dev/shm/fm-agent-securit*/p*/fm_skill_security_abc/SKILL.md".to_string(),
        format!("cd {dir} && cat SKILL.md"),
        format!("sh -c 'cat {dir}/SKILL.md'"),
        format!("python3 -c 'open(\"{dir}/SKILL.md\")'"),
        format!("cat <<'EOF'\n{dir}/SKILL.md\nEOF"),
    ];

    for command in cases {
        let decision =
            before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "bypass-shaped command must be blocked: {command}"
        );
    }
}

#[test]
fn blocks_alias_normalized_guarded_reads() {
    let (runtime, _tmp) = loaded_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    // 点段别名使命令文本不含字面 decrypted 目录前缀。
    let dot_alias = dir.replace("/mem-root/", "/./mem-root/");
    let cases = [
        format!("cat {dot_alias}/SKILL.md"),
        // 点段别名引用 /dev/shm 父目录。
        "cat /dev/./shm/p1/fm_skill_x/SKILL.md".to_string(),
        // /run/shm 是 /dev/shm 的符号链接。
        "cat /run/shm/p1/fm_skill_x/SKILL.md".to_string(),
        // `$VAR` 开头的路径保守视为可能命中。
        "cat $ROOT/fm_skill_x/SKILL.md".to_string(),
    ];
    for command in cases {
        let decision =
            before_tool_with_runtime(&runtime, "t1", "exec_command", &json!({ "cmd": command }));
        assert!(
            matches!(decision, GuardDecision::Blocked { .. }),
            "alias-shaped guarded read must be blocked: {command}"
        );
    }
}

#[test]
fn allows_comment_only_path_references() {
    // tree-sitter classification: a path mentioned only inside a comment is
    // not an executing read, so the command passes through (legacy substring
    // matching would have blocked it).
    let (runtime, _tmp) = loaded_runtime();
    let dir = runtime.decrypted_dirs("t1")[0]
        .to_string_lossy()
        .into_owned();
    let decision = before_tool_with_runtime(
        &runtime,
        "t1",
        "exec_command",
        &json!({ "cmd": format!("# {dir}/SKILL.md") }),
    );
    assert!(
        matches!(decision, GuardDecision::Allow),
        "comment-only path mention must pass through: {decision:?}"
    );
}
