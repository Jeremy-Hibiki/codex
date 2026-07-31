//! Built-in tool-use interception for encrypted skill storage.

use std::path::PathBuf;
use std::sync::Arc;

use codex_encrypted_skills::export_guard;
use codex_encrypted_skills::paths;
use codex_encrypted_skills::runtime::EncryptedSkillRuntime;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
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
        name if name == &HookToolName::apply_patch() => {
            guard_export(runtime, session_id, tool_input)
        }
        _ => GuardDecision::Allow,
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
    let dirs: Vec<String> = runtime
        .decrypted_dirs(session_id)
        .into_iter()
        .map(|dir| dir.to_string_lossy().into_owned())
        .collect();
    let mut guarded_paths = dirs;
    guarded_paths.push(runtime.mem_root().to_string_lossy().into_owned());
    if paths::command_references_dir(command, &guarded_paths)
        && (paths::is_read_command(command) || paths::is_search_command(command))
    {
        return GuardDecision::Blocked {
            message: BLOCK_MESSAGE.to_string(),
            reason: if paths::is_read_command(command) {
                "direct_read"
            } else {
                "search_probe"
            },
        };
    }
    let rewritten = runtime.rewrite_paths(session_id, command);
    if rewritten == command {
        return GuardDecision::Allow;
    }
    let mut updated = tool_input.clone();
    updated["command"] = Value::String(rewritten);
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
    if known.is_empty() {
        return GuardDecision::Allow;
    }
    let known: Vec<&str> = known.iter().map(String::as_str).collect();
    let mut values = Vec::new();
    collect_string_values(tool_input, &mut values);
    if export_guard::args_contain_plaintext(&values, &known) {
        GuardDecision::Blocked {
            message:
                "Blocked by encrypted skill policy: tool arguments contain encrypted skill content"
                    .to_string(),
            reason: "export_plaintext",
        }
    } else {
        GuardDecision::Allow
    }
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

/// Wraps a tool output so any decrypted storage path string is redacted before
/// it reaches the model or is persisted.
pub(crate) struct RedactingToolOutput {
    pub(crate) inner: Box<dyn ToolOutput>,
    pub(crate) runtime: Arc<EncryptedSkillRuntime>,
}

impl RedactingToolOutput {
    fn redact_text(&self, text: &str) -> String {
        self.runtime.redact(text)
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
