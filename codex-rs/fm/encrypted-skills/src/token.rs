//! Sentinel token serialization/parsing for encrypted skill placeholders.

use std::fmt::Write as _;

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
