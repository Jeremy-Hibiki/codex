//! Built-in tool-use interception for encrypted skill storage.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::TurnItem;
use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use fm_encrypted_skills::export_guard;
use fm_encrypted_skills::paths;
use fm_encrypted_skills::runtime::EncryptedSkillRuntime;
use serde_json::Value;

use crate::session::session::Session;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::hook_names::HookToolName;

pub(crate) const BLOCK_MESSAGE: &str = "Direct access to encrypted skill storage is not allowed";

#[derive(Debug)]
pub(crate) enum GuardDecision {
    Allow,
    Updated(Value),
    Blocked {
        message: String,
        reason: &'static str,
    },
}

pub(crate) fn before_tool(
    session: &Session,
    tool_name: &HookToolName,
    tool_input: &Value,
) -> GuardDecision {
    before_tool_with_runtime(
        &session.services.encrypted_skills_runtime,
        &session.thread_id.to_string(),
        tool_name,
        tool_input,
    )
}

pub(crate) fn before_tool_with_runtime(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    tool_name: &HookToolName,
    tool_input: &Value,
) -> GuardDecision {
    let decision = match tool_name {
        name if name == &HookToolName::bash() => guard_shell(runtime, session_id, tool_input),
        name if name == &HookToolName::view_image() => guard_read(runtime, session_id, tool_input),
        // Every other tool (file writes, web search, MCP, extension tools)
        // is an outbound-capable surface: block arguments containing known
        // skill plaintext. Shell remains the sole runtime channel for
        // legitimate in-session secret use, and its commands are path-guarded
        // and executed inside the sandbox.
        _ => guard_export(runtime, session_id, tool_input),
    };
    if let GuardDecision::Blocked { reason, .. } = &decision {
        runtime.record_blocked(session_id, tool_name.name(), reason);
    }
    decision
}

fn guard_shell(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    tool_input: &Value,
) -> GuardDecision {
    let Some(command) = tool_input.get("command").and_then(Value::as_str) else {
        return GuardDecision::Allow;
    };
    // Rewrite original skill directories to their decrypted `/dev/shm` paths
    // FIRST, so every downstream check operates on a single, canonical view
    // of what the shell will actually execute.
    let rewritten = runtime.rewrite_paths(session_id, command);
    let guarded_paths = runtime
        .decrypted_dirs(session_id)
        .into_iter()
        .map(|dir| dir.to_string_lossy().into_owned())
        .chain([runtime.mem_root().to_string_lossy().into_owned()])
        .collect::<Vec<_>>();
    // Split at unquoted chain operators (`;`, `|`, `&&`, `&`) and judge each
    // segment independently. This closes the smuggle vector where a forbidden
    // read hides after an allowed execution (`bash run.sh; cat SKILL.md`).
    for segment in paths::split_command_segments(&rewritten) {
        let references_dir = paths::command_references_dir(&segment, &guarded_paths);
        if references_dir {
            // Execution of a skill script is allowed (the runner receives the
            // rewritten decrypted path). Anything else that touches the
            // decrypted storage — read, copy, redirect, pipe, glob — is
            // blocked, including reads smuggled through redirections or
            // command substitutions inside an otherwise-allowed script
            // execution (`bash run.sh < SKILL.md`, `bash run.sh $(cat SKILL.md)`).
            let script_execution = paths::is_script_execution(&segment);
            if !script_execution
                || !paths::script_execution_avoids_guarded_io(&segment, &guarded_paths)
            {
                return GuardDecision::Blocked {
                    message: BLOCK_MESSAGE.to_string(),
                    reason: if script_execution {
                        "script_execution_io"
                    } else {
                        "non_execution_access"
                    },
                };
            }
        }
        // A segment that does NOT reference decrypted storage is always
        // allowed on its own.
    }
    updated_command(tool_input, rewritten)
}

fn updated_command(tool_input: &Value, command: String) -> GuardDecision {
    if tool_input.get("command").and_then(Value::as_str) == Some(command.as_str()) {
        return GuardDecision::Allow;
    }
    let mut updated = tool_input.clone();
    updated["command"] = Value::String(command);
    GuardDecision::Updated(updated)
}

fn guard_read(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    tool_input: &Value,
) -> GuardDecision {
    let Some(file_path) = tool_input.get("path").and_then(Value::as_str) else {
        return GuardDecision::Allow;
    };
    // Match guard_shell semantics: block any path under the session's
    // registered decrypted dirs or under the memory root (runtime root or the
    // default constant), so unknown/other-session subpaths are also covered.
    let mut guarded = runtime.decrypted_dirs(session_id);
    guarded.push(runtime.mem_root().to_path_buf());
    guarded.push(PathBuf::from(paths::MEM_ROOT));
    if path_under_dirs(file_path, &guarded) {
        GuardDecision::Blocked {
            message: BLOCK_MESSAGE.to_string(),
            reason: "file_view",
        }
    } else {
        GuardDecision::Allow
    }
}

fn guard_export(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    tool_input: &Value,
) -> GuardDecision {
    let known = runtime.known_plaintexts(session_id);
    let mut values = Vec::new();
    collect_string_values(tool_input, &mut values);
    if !known.is_empty() {
        let known: Vec<&str> = known.iter().map(String::as_str).collect();
        if export_guard::args_contain_plaintext(&values, &known) {
            return GuardDecision::Blocked {
                message:
                    "Blocked by encrypted skill policy: tool arguments contain encrypted skill content"
                        .to_string(),
                reason: "export_plaintext",
            };
        }
    }
    // Path probes through MCP/extension tools are blocked the same way shell
    // read/search commands are: referencing the memory root or any decrypted
    // directory is never a legitimate argument for a non-shell tool.
    let mut guarded_paths: Vec<String> = runtime
        .decrypted_dirs(session_id)
        .into_iter()
        .map(|dir| dir.to_string_lossy().into_owned())
        .collect();
    guarded_paths.push(runtime.mem_root().to_string_lossy().into_owned());
    if values
        .iter()
        .any(|value| paths::command_references_dir(value, &guarded_paths))
    {
        return GuardDecision::Blocked {
            message: BLOCK_MESSAGE.to_string(),
            reason: "storage_probe",
        };
    }
    GuardDecision::Allow
}

fn collect_string_values<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::String(text) => out.push(text),
        Value::Array(items) => {
            for item in items {
                collect_string_values(item, out);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_string_values(item, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn path_under_dirs(path: &str, dirs: &[PathBuf]) -> bool {
    dirs.iter().any(|dir| {
        let dir = dir.to_string_lossy();
        path == dir.as_ref()
            || (path.starts_with(dir.as_ref()) && path.as_bytes().get(dir.len()) == Some(&b'/'))
    })
}

/// Redacts known skill plaintext from assistant replies and plaintext
/// inter-agent messages before they reach durable history, rollout, or the
/// client stream.
pub(crate) fn redact_assistant_reply_items<'a>(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    items: Cow<'a, [ResponseItem]>,
) -> Cow<'a, [ResponseItem]> {
    if !items.iter().any(contains_redactable_text) {
        return items;
    }
    let known = runtime.known_plaintexts(session_id);
    if known.is_empty() {
        return items;
    }
    let known: Vec<&str> = known.iter().map(String::as_str).collect();
    let mut items = items;
    for item in items.to_mut() {
        redact_response_item_text(item, &known);
    }
    items
}

/// Redacts one assistant reply item (used at the model stream intake so every
/// derived copy — response item, turn item, and `last_agent_message` — is
/// already clean before it can be persisted).
pub(crate) fn redact_assistant_reply_item(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    mut item: ResponseItem,
) -> ResponseItem {
    if !contains_redactable_text(&item) {
        return item;
    }
    let known = runtime.known_plaintexts(session_id);
    if known.is_empty() {
        return item;
    }
    let known: Vec<&str> = known.iter().map(String::as_str).collect();
    redact_response_item_text(&mut item, &known);
    item
}

fn redact_response_item_text(item: &mut ResponseItem, known: &[&str]) {
    match item {
        ResponseItem::Message { role, content, .. } if role == "assistant" => {
            for content_item in content {
                if let ContentItem::InputText { text } | ContentItem::OutputText { text } =
                    content_item
                {
                    *text = export_guard::redact_known_plaintext(text, known);
                }
            }
        }
        ResponseItem::AgentMessage { content, .. } => {
            for content_item in content {
                if let AgentMessageInputContent::InputText { text } = content_item {
                    *text = export_guard::redact_known_plaintext(text, known);
                }
            }
        }
        ResponseItem::Reasoning {
            summary, content, ..
        } => {
            for entry in summary {
                let ReasoningItemReasoningSummary::SummaryText { text } = entry;
                *text = export_guard::redact_known_plaintext(text, known);
            }
            if let Some(content) = content {
                for entry in content {
                    match entry {
                        ReasoningItemContent::ReasoningText { text }
                        | ReasoningItemContent::Text { text } => {
                            *text = export_guard::redact_known_plaintext(text, known);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Redacts known skill plaintext from tool-output text for durable surfaces
/// (rollout and the client stream). In-memory history keeps the original text
/// so the model can keep using skill-provided content across turns; the
/// persisted copy never contains plaintext.
pub(crate) fn redact_tool_output_plaintext_for_persistence(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    mut item: ResponseItem,
) -> ResponseItem {
    let known = runtime.known_plaintexts(session_id);
    if known.is_empty() {
        return item;
    }
    let known: Vec<&str> = known.iter().map(String::as_str).collect();
    match &mut item {
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => match &mut output.body {
            FunctionCallOutputBody::Text(text) => {
                *text = export_guard::redact_known_plaintext(text, &known);
            }
            FunctionCallOutputBody::ContentItems(items) => {
                for content in items {
                    if let FunctionCallOutputContentItem::InputText { text } = content {
                        *text = export_guard::redact_known_plaintext(text, &known);
                    }
                }
            }
        },
        _ => {}
    }
    item
}

fn contains_redactable_text(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { role, content, .. } => {
            role == "assistant"
                && content.iter().any(|content_item| {
                    matches!(
                        content_item,
                        ContentItem::InputText { .. } | ContentItem::OutputText { .. }
                    )
                })
        }
        ResponseItem::AgentMessage { content, .. } => content
            .iter()
            .any(|content_item| matches!(content_item, AgentMessageInputContent::InputText { .. })),
        ResponseItem::Reasoning {
            summary, content, ..
        } => !summary.is_empty() || content.as_ref().is_some_and(|content| !content.is_empty()),
        _ => false,
    }
}

/// Redacts known skill plaintext from assistant turn items (messages and
/// reasoning) before they are emitted or persisted, covering the
/// `ItemStarted`/`ItemCompleted` event surface that `record_conversation_items`
/// does not reach.
pub(crate) fn redact_turn_item(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    mut item: TurnItem,
) -> TurnItem {
    let known = runtime.known_plaintexts(session_id);
    if known.is_empty() {
        return item;
    }
    let known: Vec<&str> = known.iter().map(String::as_str).collect();
    match &mut item {
        TurnItem::AgentMessage(agent_message) => {
            for content in &mut agent_message.content {
                let AgentMessageContent::Text { text } = content;
                *text = export_guard::redact_known_plaintext(text, &known);
            }
        }
        TurnItem::Reasoning(reasoning) => {
            for text in &mut reasoning.summary_text {
                *text = export_guard::redact_known_plaintext(text, &known);
            }
            for text in &mut reasoning.raw_content {
                *text = export_guard::redact_known_plaintext(text, &known);
            }
        }
        _ => {}
    }
    item
}

/// Wraps a tool output so any decrypted storage path string is redacted before
/// it reaches the model or is persisted.
pub(crate) struct RedactingToolOutput {
    pub(crate) inner: Box<dyn ToolOutput>,
    pub(crate) runtime: Arc<EncryptedSkillRuntime>,
    pub(crate) session_id: String,
}

impl RedactingToolOutput {
    fn redact_text(&self, text: &str) -> String {
        // 解密目录路径改写回原目录（agent 可读脚本名/原路径，不暴露 /dev/shm）；
        // 未知 mem root 子路径仍以 [REDACTED] 兜底。
        let mut out = self.runtime.unrewrite_paths(&self.session_id, text);
        let root = self.runtime.mem_root().to_string_lossy();
        out = paths::redact_path_prefix(&out, root.as_ref());
        if root.as_ref() != paths::MEM_ROOT {
            out = paths::redact_path_prefix(&out, paths::MEM_ROOT);
        }
        out
    }

    fn redact_payload(&self, payload: &mut FunctionCallOutputPayload) {
        match &mut payload.body {
            FunctionCallOutputBody::Text(text) => *text = self.redact_text(text),
            FunctionCallOutputBody::ContentItems(items) => {
                for item in items {
                    if let FunctionCallOutputContentItem::InputText { text } = item {
                        *text = self.redact_text(text);
                    }
                }
            }
        }
    }

    fn redact_response_item(&self, item: &mut ResponseInputItem) {
        match item {
            ResponseInputItem::Message { content, .. } => {
                for content_item in content {
                    match content_item {
                        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                            *text = self.redact_text(text);
                        }
                        ContentItem::InputImage { .. } | ContentItem::InputAudio { .. } => {}
                    }
                }
            }
            ResponseInputItem::FunctionCallOutput { output, .. }
            | ResponseInputItem::CustomToolCallOutput { output, .. } => {
                self.redact_payload(output);
            }
            ResponseInputItem::McpToolCallOutput { .. }
            | ResponseInputItem::ToolSearchOutput { .. } => {}
        }
    }

    fn redact_json(&self, value: &mut Value) {
        match value {
            Value::String(text) => *text = self.redact_text(text),
            Value::Array(items) => {
                for item in items {
                    self.redact_json(item);
                }
            }
            Value::Object(map) => {
                for item in map.values_mut() {
                    self.redact_json(item);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
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

#[cfg(test)]
#[path = "encrypted_skills_guard_tests.rs"]
mod tests;
