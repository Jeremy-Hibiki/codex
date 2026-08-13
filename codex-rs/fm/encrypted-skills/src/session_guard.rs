//! Per-session facade over the encrypted-skill runtime and guard helpers.
//!
//! Hosts (codex-core) repeatedly call pairs like
//! `(&runtime, &session_id.to_string())`; this facade bundles the two so call
//! sites read `session.encrypted_skills_guard().redact_turn_item(item)` and
//! the session id is materialized once.

use std::borrow::Cow;
use std::path::PathBuf;

use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use serde_json::Value;

use crate::audit::AuditEvent;
use crate::guard::GuardDecision;
use crate::runtime::EncryptedSkillRuntime;

/// Session-scoped handle over an [`EncryptedSkillRuntime`].
///
/// The session id is owned so the guard can be constructed from a temporary
/// `thread_id.to_string()` without lifetime issues.
pub struct SessionGuard<'a> {
    runtime: &'a EncryptedSkillRuntime,
    session_id: String,
}

impl EncryptedSkillRuntime {
    /// Returns a session-scoped facade over this runtime.
    pub fn guard(&self, session_id: impl Into<String>) -> SessionGuard<'_> {
        SessionGuard {
            runtime: self,
            session_id: session_id.into(),
        }
    }
}

impl SessionGuard<'_> {
    fn session_id(&self) -> &str {
        &self.session_id
    }

    /// True when this session currently has decrypted skill content loaded.
    pub fn is_engaged(&self) -> bool {
        self.runtime.is_engaged(self.session_id())
    }

    /// Wipes this session's decrypted directories, cache, and registry.
    pub fn clear_thread(&self) {
        self.runtime.clear_thread(self.session_id());
    }

    /// Unloads this session's decrypted state at turn end.
    pub fn unload_turn(&self) {
        self.runtime.unload_turn(self.session_id());
    }

    /// Runs the TTL sweep over all sessions.
    pub fn sweep(&self) {
        self.runtime.sweep();
    }

    /// Records a blocked-access audit event for this session.
    pub fn record_blocked(&self, tool: &str, reason: &'static str) {
        self.runtime.record_blocked(self.session_id(), tool, reason);
    }

    /// Registered `(decrypted, original)` directory pairs for this session.
    pub fn path_mappings(&self) -> Vec<(PathBuf, PathBuf)> {
        self.runtime.path_mappings(self.session_id())
    }

    /// Guard one tool invocation for this session.
    pub fn before_tool(
        &self,
        tool_name: &str,
        tool_input: &Value,
        binds_active: bool,
    ) -> GuardDecision {
        crate::guard::before_tool(
            self.runtime,
            self.session_id(),
            tool_name,
            tool_input,
            binds_active,
        )
    }

    /// Guard stdin text written to a running shell for this session.
    pub fn guard_stdin_input(&self, chars: &str) -> GuardDecision {
        crate::guard::guard_stdin_input(self.runtime, self.session_id(), chars)
    }

    /// True when the command is an execute-only skill script execution for
    /// this session.
    pub fn is_skill_script_execution(&self, command: &str) -> bool {
        crate::guard::is_skill_script_execution(self.runtime, self.session_id(), command)
    }

    /// Redacts known plaintext and decrypted paths from arbitrary text.
    pub fn redact_text(&self, text: &str) -> String {
        crate::guard::redact_text(self.runtime, self.session_id(), text)
    }

    /// Redacts streaming text without emitting per-fragment audit events; the
    /// complete-item redaction records the `redaction` event once.
    pub fn redact_text_streaming(&self, text: &str) -> String {
        crate::guard::redact_text_quiet(self.runtime, self.session_id(), text)
    }

    /// Records that the external guardrail flagged a user input. `prompt` is
    /// the flagged input (truncate at the call site if needed).
    pub fn record_guardrail_blocked(&self, prompt: String) {
        self.runtime.emit(AuditEvent::GuardrailBlocked {
            session_id: self.session_id().to_string(),
            prompt,
        });
    }

    /// Redacts one assistant reply item at the model stream intake.
    pub fn redact_assistant_reply_item(&self, item: ResponseItem) -> ResponseItem {
        crate::guard::redact_assistant_reply_item(self.runtime, self.session_id(), item)
    }

    /// Redacts assistant reply items before durable persistence.
    pub fn redact_assistant_reply_items<'b>(
        &self,
        items: Cow<'b, [ResponseItem]>,
    ) -> Cow<'b, [ResponseItem]> {
        crate::guard::redact_assistant_reply_items(self.runtime, self.session_id(), items)
    }

    /// Redacts one turn item before it is emitted or persisted.
    pub fn redact_turn_item(&self, item: TurnItem) -> TurnItem {
        crate::guard::redact_turn_item(self.runtime, self.session_id(), item)
    }

    /// Redacts tool-output and developer text for durable surfaces.
    pub fn redact_tool_output_plaintext_for_persistence(&self, item: ResponseItem) -> ResponseItem {
        crate::guard::redact_tool_output_plaintext_for_persistence(
            self.runtime,
            self.session_id(),
            item,
        )
    }

    /// Redacts every text-bearing response item for trace payloads.
    pub fn redact_all_response_item_text(&self, items: &[ResponseItem]) -> Vec<ResponseItem> {
        crate::guard::redact_all_response_item_text(self.runtime, self.session_id(), items)
    }
}
