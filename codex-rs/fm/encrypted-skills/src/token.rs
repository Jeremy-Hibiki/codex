//! Sentinel token serialization/parsing for encrypted skill placeholders.

use std::fmt::Write as _;

use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use rand::RngCore;

pub const TOKEN_PREFIX: &str = "[SENSITIVE_SKILL_TOKEN:";
pub const TOKEN_SUFFIX: &str = "]";
pub const TOKEN_HEX_BYTES: usize = 16;
pub const TOKEN_HEX_CHARS: usize = TOKEN_HEX_BYTES * 2;

/// A parsed `[SENSITIVE_SKILL_TOKEN:{session_id}:{hex}]` placeholder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub session_id: String,
    pub hex: String,
}

impl Token {
    pub fn new(session_id: impl Into<String>, hex: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            hex: hex.into(),
        }
    }

    pub fn serialize(&self) -> String {
        format!(
            "{TOKEN_PREFIX}{}:{}{TOKEN_SUFFIX}",
            self.session_id, self.hex
        )
    }

    /// Parses the first sentinel token found in `text`.
    pub fn parse(text: &str) -> Option<Token> {
        let start = text.find(TOKEN_PREFIX)?;
        let rest = &text[start + TOKEN_PREFIX.len()..];
        let (session_id, rest) = rest.split_once(':')?;
        let end = rest.find(TOKEN_SUFFIX)?;
        let hex = &rest[..end];
        if !valid_session_id(session_id) || hex.len() != TOKEN_HEX_CHARS || !is_hex(hex) {
            return None;
        }
        Some(Token::new(session_id, hex))
    }
}

/// Session ids are embedded in sentinel tokens, so `:` and `]` would make the
/// serialized form ambiguous to parse.
fn valid_session_id(session_id: &str) -> bool {
    !session_id.is_empty() && !session_id.contains([':', ']'])
}

pub fn random_hex() -> String {
    let mut bytes = [0u8; TOKEN_HEX_BYTES];
    rand::rng().fill_bytes(&mut bytes);
    hex_encode(&bytes)
}
/// Replaces every sentinel token in `text` with `replacement`. Used at fork
/// boundaries to neutralize the parent session's decryption handles without
/// discarding the surrounding user message that carries them.
pub fn strip_tokens(text: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(TOKEN_PREFIX) {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end_rel) = tail.find(TOKEN_SUFFIX) else {
            out.push_str(tail);
            return out;
        };
        out.push_str(replacement);
        rest = &tail[end_rel + TOKEN_SUFFIX.len()..];
    }
    out.push_str(rest);
    out
}

/// Shared traversal over every text-bearing field of a response item. The
/// surface list lives in exactly one place here; [`for_each_response_item_text`]
/// and [`for_each_response_item_text_ref`] instantiate it with `&mut` and `&`
/// access respectively, so the two mutation modes cannot drift apart.
macro_rules! for_each_response_item_text {
    ($item:expr, $text:ident, $visit:expr) => {
        match $item {
            ResponseItem::Message { content, .. } => {
                for content_item in content {
                    match content_item {
                        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                            let $text = text;
                            $visit;
                        }
                        ContentItem::InputImage { .. } | ContentItem::InputAudio { .. } => {}
                    }
                }
            }
            ResponseItem::AgentMessage { content, .. } => {
                for content_item in content {
                    if let AgentMessageInputContent::InputText { text } = content_item {
                        let $text = text;
                        $visit;
                    }
                }
            }
            ResponseItem::FunctionCall { arguments, .. } => {
                let $text = arguments;
                $visit;
            }
            ResponseItem::CustomToolCall { input, .. } => {
                let $text = input;
                $visit;
            }
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. } => match output {
                FunctionCallOutputPayload {
                    body: FunctionCallOutputBody::Text(text),
                    ..
                } => {
                    let $text = text;
                    $visit;
                }
                FunctionCallOutputPayload {
                    body: FunctionCallOutputBody::ContentItems(items),
                    ..
                } => {
                    for content_item in items {
                        if let FunctionCallOutputContentItem::InputText { text } = content_item {
                            let $text = text;
                            $visit;
                        }
                    }
                }
            },
            ResponseItem::Reasoning {
                summary, content, ..
            } => {
                for entry in summary {
                    let ReasoningItemReasoningSummary::SummaryText { text } = entry;
                    let $text = text;
                    $visit;
                }
                if let Some(content) = content {
                    for entry in content {
                        match entry {
                            ReasoningItemContent::ReasoningText { text }
                            | ReasoningItemContent::Text { text } => {
                                let $text = text;
                                $visit;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    };
}

/// Applies `f` to every text-bearing field of a response item. Shared by the
/// host adapters (fork token stripping, redaction) so the text-surface list
/// stays in one place.
pub fn for_each_response_item_text(item: &mut ResponseItem, f: &mut impl FnMut(&mut String)) {
    for_each_response_item_text!(item, text, f(text));
}

/// Read-only variant of [`for_each_response_item_text`] for checks that only
/// need to inspect text (token-presence detection), avoiding a full item
/// clone just to walk the surfaces.
pub fn for_each_response_item_text_ref(item: &ResponseItem, f: &mut impl FnMut(&str)) {
    for_each_response_item_text!(item, text, f(text));
}

fn strip_token_text(text: &mut String, replacement: &str) {
    if text.contains(TOKEN_PREFIX) {
        *text = strip_tokens(text, replacement);
    }
}

/// Strips sentinel tokens from every text-bearing field of a response item.
pub fn strip_tokens_from_response_item(item: &mut ResponseItem, replacement: &str) {
    for_each_response_item_text(item, &mut |text| strip_token_text(text, replacement));
}

/// Strips sentinel tokens from every text-bearing surface of a rollout item
/// (response items, inter-agent communication, compacted history).
pub fn strip_tokens_from_rollout_item(item: &mut RolloutItem, replacement: &str) {
    match item {
        RolloutItem::ResponseItem(response_item) => {
            strip_tokens_from_response_item(response_item, replacement);
        }
        RolloutItem::InterAgentCommunication(communication) => {
            strip_token_text(&mut communication.content, replacement);
            if let Some(encrypted) = &mut communication.encrypted_content {
                strip_token_text(encrypted, replacement);
            }
        }
        RolloutItem::Compacted(compacted) => {
            strip_token_text(&mut compacted.message, replacement);
            if let Some(history) = &mut compacted.replacement_history {
                for response_item in history {
                    strip_tokens_from_response_item(response_item, replacement);
                }
            }
        }
        _ => {}
    }
}

/// True when any text-bearing surface of the rollout contains the sentinel
/// token prefix, i.e. this thread engaged an encrypted skill at some point.
///
/// Used by host history builders to hide reasoning from client-facing
/// history while keeping it in the model context.
pub fn rollout_items_contain_token(items: &[RolloutItem]) -> bool {
    items.iter().any(rollout_item_contains_token)
}

fn rollout_item_contains_token(item: &RolloutItem) -> bool {
    let mut found = false;
    match item {
        RolloutItem::ResponseItem(response_item) => {
            for_each_response_item_text_ref(response_item, &mut |text| {
                if text.contains(TOKEN_PREFIX) {
                    found = true;
                }
            });
        }
        RolloutItem::InterAgentCommunication(communication) => {
            found = communication.content.contains(TOKEN_PREFIX)
                || communication
                    .encrypted_content
                    .as_deref()
                    .is_some_and(|text| text.contains(TOKEN_PREFIX));
        }
        RolloutItem::Compacted(compacted) => {
            found = compacted.message.contains(TOKEN_PREFIX);
            if let Some(history) = &compacted.replacement_history {
                for response_item in history {
                    for_each_response_item_text_ref(response_item, &mut |text| {
                        if text.contains(TOKEN_PREFIX) {
                            found = true;
                        }
                    });
                }
            }
        }
        _ => {}
    }
    found
}

pub fn is_hex(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
