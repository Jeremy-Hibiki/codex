//! Built-in tool-use interception and output redaction for encrypted skill
//! storage.
//!
//! This module is the policy core: it depends only on this crate and
//! `codex-protocol` value types. Host crates (codex-core) keep a thin adapter
//! that maps their tool identities into a plain tool-name string and wraps
//! their output types around the redaction helpers.

use std::borrow::Cow;
use std::path::PathBuf;

use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use serde_json::Value;

use crate::audit::AuditEvent;
use crate::export_guard;
use crate::paths;
use crate::runtime::EncryptedSkillRuntime;

/// Hook name for the exec tool that carries shell commands.
///
/// Upstream merged its standalone shell tool into unified exec, so
/// `exec_command` is the only exec-capable tool the guard sees. The legacy
/// spellings are kept because the hook name is a string at this boundary.
pub const BASH_TOOL_NAMES: &[&str] = &["exec_command", "shell", "Bash"];
/// Argument carrying the command line; the key differs per exec tool.
const COMMAND_ARG_KEYS: &[&str] = &["cmd", "command"];
/// Canonical hook name for the `view_image` tool.
pub const VIEW_IMAGE_TOOL_NAME: &str = "view_image";

pub const BLOCK_MESSAGE: &str = "Direct access to encrypted skill storage is not allowed";

#[cfg(test)]
pub fn before_tool_with_runtime(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    tool_name: &str,
    tool_input: &Value,
) -> GuardDecision {
    before_tool(runtime, session_id, tool_name, tool_input)
}

#[derive(Debug)]
#[must_use]
pub enum GuardDecision {
    Allow,
    Updated(Value),
    Blocked {
        message: String,
        reason: &'static str,
    },
}

pub fn before_tool(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    tool_name: &str,
    tool_input: &Value,
) -> GuardDecision {
    // Unengaged sessions have no plaintext and no decrypted paths: every
    // guard rule passes through unchanged so normal Codex behavior is
    // preserved exactly.
    if !runtime.is_engaged(session_id) {
        return GuardDecision::Allow;
    }
    let decision = match tool_name {
        name if BASH_TOOL_NAMES.contains(&name) => guard_shell(runtime, session_id, tool_input),
        VIEW_IMAGE_TOOL_NAME => guard_read(runtime, session_id, tool_input),
        // Every other tool (file writes, web search, MCP, extension tools)
        // is an outbound-capable surface: block arguments containing known
        // skill plaintext. Shell remains the sole runtime channel for
        // legitimate in-session secret use, and its commands are path-guarded
        // and executed inside the sandbox.
        _ => guard_export(runtime, session_id, tool_input),
    };
    if let GuardDecision::Blocked { reason, .. } = &decision {
        runtime.record_blocked(session_id, tool_name, reason);
    }
    decision
}

/// The guarded directory set used for command classification: the session's
/// decrypted directories, the runtime memory root, and the original skill
/// directories. Shell commands are rewritten to decrypted paths before this
/// set is consulted, while stdin input and script detection may see either
/// form — one shared definition keeps all three judges consistent.
fn guarded_paths_for_session(runtime: &EncryptedSkillRuntime, session_id: &str) -> Vec<String> {
    let mut guarded: Vec<String> = runtime
        .decrypted_dirs(session_id)
        .into_iter()
        .map(|dir| dir.to_string_lossy().into_owned())
        .collect();
    guarded.push(runtime.mem_root().to_string_lossy().into_owned());
    guarded.extend(
        runtime
            .path_mappings(session_id)
            .into_iter()
            .map(|(_, original)| original.to_string_lossy().into_owned()),
    );
    guarded
}

fn guard_shell(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    tool_input: &Value,
) -> GuardDecision {
    let Some((_, command)) = command_argument(tool_input) else {
        return GuardDecision::Allow;
    };
    // Rewrite original dirs to the decrypted `/dev/shm` paths so every
    // downstream check sees what the shell will actually execute. Upstream
    // dropped the sandbox bind that used to expose plaintext at the logical
    // path, so rewriting is the only mode left.
    let rewritten = runtime.rewrite_paths(session_id, command);
    // A shell command that carries known skill plaintext is an outbound channel
    // (echo/printf/heredoc writing to disk, pipes to other processes, ...).
    // Block it the same way non-shell tools are blocked, before the path rules
    // run, so partial fragments cannot be smuggled through command arguments.
    let known = runtime.known_plaintexts(session_id);
    if !known.is_empty() {
        let known: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
        if export_guard::args_contain_plaintext(&[&rewritten], &known) {
            return GuardDecision::Blocked {
                message:
                    "Blocked by encrypted skill policy: command contains encrypted skill content"
                        .to_string(),
                reason: "shell_plaintext",
            };
        }
    }
    let guarded_paths = guarded_paths_for_session(runtime, session_id);
    // Split at unquoted chain operators (`;`, `|`, `&&`, `&`) and judge each
    // segment independently. This closes the smuggle vector where a forbidden
    // read hides after an allowed execution (`bash run.sh; cat SKILL.md`).
    // The judgment tracks the virtual cwd across `cd`/`pushd` segments so a
    // relative read cannot hide behind chained `cd`s (`cd /dev && cd shm &&
    // cat fm-agent-security/...`).
    let segments = paths::split_command_segments(&rewritten);
    let flags = paths::segment_guarded_flags(&segments, &guarded_paths);
    for (segment, references_dir) in segments.iter().zip(flags) {
        if references_dir {
            // Execution of a skill script is allowed (the runner receives the
            // rewritten decrypted path). Anything else that touches the
            // decrypted storage — read, copy, redirect, pipe, glob — is
            // blocked, including reads smuggled through redirections or
            // command substitutions inside an otherwise-allowed script
            // execution (`bash run.sh < SKILL.md`, `bash run.sh $(cat SKILL.md)`).
            let script_execution = paths::is_script_execution(segment);
            if !script_execution
                || !paths::script_execution_avoids_guarded_io(segment, &guarded_paths)
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

/// Finds the command argument for an exec tool, returning its key and value.
///
/// Tools spell this argument differently (`cmd` for unified exec, `command`
/// for the legacy shell tool), so the guard must not hard-code one spelling.
fn command_argument(tool_input: &Value) -> Option<(&'static str, &str)> {
    COMMAND_ARG_KEYS.iter().find_map(|key| {
        tool_input
            .get(*key)
            .and_then(Value::as_str)
            .map(|v| (*key, v))
    })
}

fn updated_command(tool_input: &Value, command: String) -> GuardDecision {
    let Some((key, current)) = command_argument(tool_input) else {
        return GuardDecision::Allow;
    };
    if current == command.as_str() {
        return GuardDecision::Allow;
    }
    let mut updated = tool_input.clone();
    updated[key] = Value::String(command);
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
    // registered decrypted dirs, under the memory root, or under the memory
    // root's parent `/dev/shm` (any reference there can reach decrypted
    // storage — `find /dev/shm` lists the real plaintext tree).
    let mut guarded = runtime.decrypted_dirs(session_id);
    guarded.push(runtime.mem_root().to_path_buf());
    guarded.push(PathBuf::from(paths::MEM_ROOT_PARENT));
    // Alias shapes (`/dev/./shm`, `/run/shm`) hold no literal guarded prefix;
    // check the lexically normalized path as well. Images cannot be
    // text-redacted, so the path gate is the only line of defense here.
    let normalized = paths::normalize_path_token(file_path);
    if path_under_dirs(file_path, &guarded)
        || (normalized != file_path && path_under_dirs(&normalized, &guarded))
    {
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
        let known: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
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

/// Guards bytes about to be written to the stdin of an already-running shell
/// process. The spawn-time guard only inspected the command that *launched*
/// the shell; subsequent stdin writes are fresh shell input it never saw, so
/// they are checked the same way a new shell command is — with two
/// differences:
/// - Paths are never rewritten: the running shell's filesystem view was fixed
///   at spawn time, so rewriting the injected text has no effect on what the
///   shell can read.
/// - Both original and decrypted skill paths are checked: a shell spawned with
///   sandbox binds active sees the original path bound to the decrypted tree,
///   while one spawned without binds only sees the decrypted path.
pub fn guard_stdin_input(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    chars: &str,
) -> GuardDecision {
    if !runtime.is_engaged(session_id) {
        return GuardDecision::Allow;
    }
    // A stdin write carrying known skill plaintext is an outbound channel
    // (echo/printf/heredoc writing to disk, pipes to other processes, ...).
    let known = runtime.known_plaintexts(session_id);
    if !known.is_empty() {
        let known: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
        if export_guard::args_contain_plaintext(&[chars], &known) {
            return GuardDecision::Blocked {
                message:
                    "Blocked by encrypted skill policy: stdin input contains encrypted skill content"
                        .to_string(),
                reason: "shell_plaintext",
            };
        }
    }
    let guarded_paths = guarded_paths_for_session(runtime, session_id);
    let segments = paths::split_command_segments(chars);
    let flags = paths::segment_guarded_flags(&segments, &guarded_paths);
    for (segment, references_dir) in segments.iter().zip(flags) {
        if references_dir {
            let script_execution = paths::is_script_execution(segment);
            if !script_execution
                || !paths::script_execution_avoids_guarded_io(segment, &guarded_paths)
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
    }
    GuardDecision::Allow
}

/// True when `command` is an execute-only skill script execution for an
/// engaged session (a guarded path referenced as a script argument, without
/// guarded shell I/O channels). Used to auto-permit such executions (D9).
pub fn is_skill_script_execution(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    command: &str,
) -> bool {
    if !runtime.is_engaged(session_id) {
        return false;
    }
    let guarded_paths = guarded_paths_for_session(runtime, session_id);
    let rewritten = runtime.rewrite_paths(session_id, command);
    let segments = paths::split_command_segments(&rewritten);
    let flags = paths::segment_guarded_flags(&segments, &guarded_paths);
    segments.iter().zip(flags).any(|(segment, references_dir)| {
        references_dir
            && paths::is_script_execution(segment)
            && paths::script_execution_avoids_guarded_io(segment, &guarded_paths)
    })
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

/// Emits a `redaction` audit event when any cached skill plaintext appears in
/// `texts`. Names the affected skills; content is never included.
fn emit_redaction_if_matched(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    surface: &'static str,
    texts: &[&str],
) {
    let skills = runtime.matched_skills_in_texts(session_id, texts);
    if !skills.is_empty() {
        runtime.emit(AuditEvent::Redaction {
            session_id: session_id.to_string(),
            skills,
            surface,
        });
    }
}

/// Text fields redacted by [`redact_response_item_all_text`].
fn response_item_all_texts(item: &ResponseItem) -> Vec<&str> {
    let mut out = Vec::new();
    match item {
        ResponseItem::Message { content, .. } => {
            for content_item in content {
                match content_item {
                    ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                        out.push(text.as_str())
                    }
                    ContentItem::InputImage { .. } | ContentItem::InputAudio { .. } => {}
                }
            }
        }
        ResponseItem::AgentMessage { content, .. } => {
            for content_item in content {
                if let AgentMessageInputContent::InputText { text } = content_item {
                    out.push(text.as_str());
                }
            }
        }
        ResponseItem::FunctionCall { arguments, .. } => out.push(arguments.as_str()),
        ResponseItem::CustomToolCall { input, .. } => out.push(input.as_str()),
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => match &output.body {
            FunctionCallOutputBody::Text(text) => out.push(text.as_str()),
            FunctionCallOutputBody::ContentItems(items) => {
                for content in items {
                    if let FunctionCallOutputContentItem::InputText { text } = content {
                        out.push(text.as_str());
                    }
                }
            }
        },
        ResponseItem::Reasoning {
            summary, content, ..
        } => {
            for entry in summary {
                let ReasoningItemReasoningSummary::SummaryText { text } = entry;
                out.push(text.as_str());
            }
            if let Some(content) = content {
                for entry in content {
                    match entry {
                        ReasoningItemContent::ReasoningText { text }
                        | ReasoningItemContent::Text { text } => out.push(text.as_str()),
                    }
                }
            }
        }
        _ => {}
    }
    out
}

/// Text fields redacted by [`redact_response_item_text`] (assistant replies).
fn assistant_reply_texts(item: &ResponseItem) -> Vec<&str> {
    match item {
        ResponseItem::Message { role, content, .. } if role == "assistant" => content
            .iter()
            .filter_map(|content_item| match content_item {
                ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                    Some(text.as_str())
                }
                _ => None,
            })
            .collect(),
        ResponseItem::AgentMessage { content, .. } => content
            .iter()
            .filter_map(|content_item| {
                if let AgentMessageInputContent::InputText { text } = content_item {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect(),
        ResponseItem::Reasoning {
            summary, content, ..
        } => {
            let mut out = Vec::new();
            for entry in summary {
                let ReasoningItemReasoningSummary::SummaryText { text } = entry;
                out.push(text.as_str());
            }
            if let Some(content) = content {
                for entry in content {
                    match entry {
                        ReasoningItemContent::ReasoningText { text }
                        | ReasoningItemContent::Text { text } => out.push(text.as_str()),
                    }
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Redacts known skill plaintext from assistant replies and plaintext
/// inter-agent messages before they reach durable history, rollout, or the
/// client stream.
pub fn redact_assistant_reply_items<'a>(
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
    let texts: Vec<&str> = items.iter().flat_map(assistant_reply_texts).collect();
    emit_redaction_if_matched(runtime, session_id, "assistant_reply", &texts);
    let known: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
    let mut items = items;
    for item in items.to_mut() {
        redact_response_item_text(item, &known);
    }
    items
}

/// Redacts one assistant reply item (used at the model stream intake so every
/// derived copy — response item, turn item, and `last_agent_message` — is
/// already clean before it can be persisted).
pub fn redact_assistant_reply_item(
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
    let texts = assistant_reply_texts(&item);
    emit_redaction_if_matched(runtime, session_id, "assistant_reply", &texts);
    let known: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
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

/// Redacts known skill plaintext (and decrypted paths) from arbitrary text.
/// Used for hook-provided contexts and trace payloads, which do not pass
/// through the assistant-reply redaction path.
pub fn redact_text(runtime: &EncryptedSkillRuntime, session_id: &str, text: &str) -> String {
    emit_redaction_if_matched(runtime, session_id, "text", std::slice::from_ref(&text));
    redact_text_quiet(runtime, session_id, text)
}

/// Redacts without emitting a per-call audit event. Used for high-frequency
/// streaming deltas (every streamed fragment would otherwise flood the audit
/// log); the complete-item redaction still records one `redaction` event.
pub fn redact_text_quiet(runtime: &EncryptedSkillRuntime, session_id: &str, text: &str) -> String {
    let mut out = text.to_string();
    let known = runtime.known_plaintexts(session_id);
    if !known.is_empty() {
        let known: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
        out = export_guard::redact_known_plaintext(&out, &known);
    }
    runtime.redact(&out)
}

/// Rewrites decrypted storage paths back to their original skill paths and
/// redacts any remaining memory-root path segments. This is the model-facing
/// projection used for tool output and for approval/review payloads: reviewers
/// must never see the real `/dev/shm` location.
pub fn redact_storage_paths(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    text: &str,
) -> String {
    let mut out = runtime.unrewrite_paths(session_id, text);
    let root = runtime.mem_root().to_string_lossy();
    out = paths::redact_path_prefix(&out, root.as_ref());
    if root.as_ref() != paths::MEM_ROOT {
        out = paths::redact_path_prefix(&out, paths::MEM_ROOT);
    }
    out
}

/// Redacts decrypted storage paths from every command-bearing guardian
/// approval request before the request reaches a reviewer model.
/// Clones and redacts every text-bearing field of `items` (all roles), for
/// diagnostic surfaces such as compaction traces that may contain model output
/// derived from rehydrated skill content.
pub fn redact_all_response_item_text(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    items: &[ResponseItem],
) -> Vec<ResponseItem> {
    let known = runtime.known_plaintexts(session_id);
    let known: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
    let texts: Vec<&str> = items.iter().flat_map(response_item_all_texts).collect();
    emit_redaction_if_matched(runtime, session_id, "response_items", &texts);
    items
        .iter()
        .cloned()
        .map(|mut item| {
            redact_response_item_all_text(runtime, &mut item, &known);
            item
        })
        .collect()
}

fn redact_response_item_all_text(
    runtime: &EncryptedSkillRuntime,
    item: &mut ResponseItem,
    known: &[&str],
) {
    match item {
        ResponseItem::Message { content, .. } => {
            for content_item in content {
                match content_item {
                    ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                        redact_text_field(runtime, text, known);
                    }
                    ContentItem::InputImage { .. } | ContentItem::InputAudio { .. } => {}
                }
            }
        }
        ResponseItem::AgentMessage { content, .. } => {
            for content_item in content {
                if let AgentMessageInputContent::InputText { text } = content_item {
                    redact_text_field(runtime, text, known);
                }
            }
        }
        ResponseItem::FunctionCall { arguments, .. } => {
            redact_text_field(runtime, arguments, known)
        }
        ResponseItem::CustomToolCall { input, .. } => redact_text_field(runtime, input, known),
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => {
            redact_output_body(runtime, output, known);
        }
        ResponseItem::Reasoning {
            summary, content, ..
        } => {
            for entry in summary {
                let ReasoningItemReasoningSummary::SummaryText { text } = entry;
                redact_text_field(runtime, text, known);
            }
            if let Some(content) = content {
                for entry in content {
                    match entry {
                        ReasoningItemContent::ReasoningText { text }
                        | ReasoningItemContent::Text { text } => {
                            redact_text_field(runtime, text, known);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn redact_text_field(runtime: &EncryptedSkillRuntime, text: &mut String, known: &[&str]) {
    *text = export_guard::redact_known_plaintext(text, known);
    *text = runtime.redact(text);
}

fn redact_output_body(
    runtime: &EncryptedSkillRuntime,
    output: &mut FunctionCallOutputPayload,
    known: &[&str],
) {
    match &mut output.body {
        FunctionCallOutputBody::Text(text) => redact_text_field(runtime, text, known),
        FunctionCallOutputBody::ContentItems(items) => {
            for content in items {
                if let FunctionCallOutputContentItem::InputText { text } = content {
                    redact_text_field(runtime, text, known);
                }
            }
        }
    }
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

/// Wraps a tool output so any decrypted storage path string is redacted before
/// it reaches the model or is persisted.
/// Redacts text-bearing fields of a function-call output payload.
pub fn redact_payload(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    payload: &mut FunctionCallOutputPayload,
) {
    match &mut payload.body {
        FunctionCallOutputBody::Text(text) => *text = redact_text(runtime, session_id, text),
        FunctionCallOutputBody::ContentItems(items) => {
            for item in items {
                if let FunctionCallOutputContentItem::InputText { text } = item {
                    *text = redact_text(runtime, session_id, text);
                }
            }
        }
    }
}

/// Redacts text-bearing fields of a response input item.
pub fn redact_response_item(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    item: &mut ResponseInputItem,
) {
    match item {
        ResponseInputItem::Message { content, .. } => {
            for content_item in content {
                match content_item {
                    ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                        *text = redact_text(runtime, session_id, text);
                    }
                    ContentItem::InputImage { .. } | ContentItem::InputAudio { .. } => {}
                }
            }
        }
        ResponseInputItem::FunctionCallOutput { output, .. }
        | ResponseInputItem::CustomToolCallOutput { output, .. } => {
            redact_payload(runtime, session_id, output);
        }
        ResponseInputItem::McpToolCallOutput { output, .. } => {
            // Same treatment as FunctionCallOutput: every text-bearing field
            // (content items, structured content, metadata) is redacted.
            for value in &mut output.content {
                redact_json(runtime, session_id, value);
            }
            if let Some(structured_content) = &mut output.structured_content {
                redact_json(runtime, session_id, structured_content);
            }
            if let Some(meta) = &mut output.meta {
                redact_json(runtime, session_id, meta);
            }
        }
        ResponseInputItem::ToolSearchOutput { tools, .. } => {
            for tool in tools {
                redact_json(runtime, session_id, tool);
            }
        }
    }
}

/// Redacts every string in an arbitrary JSON value.
pub fn redact_json(runtime: &EncryptedSkillRuntime, session_id: &str, value: &mut Value) {
    match value {
        Value::String(text) => *text = redact_text(runtime, session_id, text),
        Value::Array(items) => {
            for item in items {
                redact_json(runtime, session_id, item);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                redact_json(runtime, session_id, item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
