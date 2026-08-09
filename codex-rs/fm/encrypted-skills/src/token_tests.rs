use super::*;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;

#[test]
fn serializes_and_parses_round_trip() {
    let token = Token::new("thread-1", "a1b2c3d4e5f60718293a4b5c6d7e8f90");
    let serialized = token.serialize();
    assert_eq!(
        serialized,
        "[SENSITIVE_SKILL_TOKEN:thread-1:a1b2c3d4e5f60718293a4b5c6d7e8f90]"
    );
    assert_eq!(Token::parse(&serialized), Some(token));
}

#[test]
fn parses_token_inside_surrounding_text() {
    let hex = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
    let text = format!("prefix [SENSITIVE_SKILL_TOKEN:abc:{hex}] suffix");
    assert_eq!(Token::parse(&text), Some(Token::new("abc", hex)));
}

#[test]
fn parse_rejects_missing_prefix() {
    assert_eq!(Token::parse("SENSITIVE_SKILL_TOKEN:abc:ff00"), None);
}

#[test]
fn parse_rejects_invalid_hex() {
    assert_eq!(Token::parse("[SENSITIVE_SKILL_TOKEN:abc:xyz]"), None);
}

#[test]
fn parse_rejects_empty_session_id() {
    assert_eq!(
        Token::parse("[SENSITIVE_SKILL_TOKEN::a1b2c3d4e5f60718293a4b5c6d7e8f90]"),
        None
    );
}

#[test]
fn random_hex_produces_32_hex_chars_and_unique_values() {
    let a = random_hex();
    let b = random_hex();
    let a = Token::new("thread-1", a);
    let b = Token::new("thread-1", b);
    assert_eq!(a.session_id, "thread-1");
    assert_eq!(a.hex.len(), TOKEN_HEX_CHARS);
    assert!(is_hex(&a.hex));
    assert_ne!(a.hex, b.hex);
}

#[test]
fn parse_rejects_short_hex_and_ambiguous_session_ids() {
    assert_eq!(
        Token::parse("[SENSITIVE_SKILL_TOKEN:abc:ff00]"),
        None,
        "hex shorter than the token key length must be rejected"
    );
    assert_eq!(
        Token::parse("[SENSITIVE_SKILL_TOKEN:a:b:a1b2c3d4e5f60718293a4b5c6d7e8f90]"),
        None,
        "session ids containing ':' are ambiguous"
    );
    assert_eq!(
        Token::parse("[SENSITIVE_SKILL_TOKEN:a]b:a1b2c3d4e5f60718293a4b5c6d7e8f90]"),
        None,
        "session ids containing ']' are ambiguous"
    );
}

#[test]
fn is_hex_rejects_empty_and_non_hex() {
    assert!(!is_hex(""));
    assert!(!is_hex("zz"));
    assert!(is_hex("0123456789abcdef"));
}
#[test]
fn strip_tokens_replaces_sentinels_and_keeps_surrounding_text() {
    for (input, replacement, expected) in [
        (
            "do [SENSITIVE_SKILL_TOKEN:abc:ff00] then [SENSITIVE_SKILL_TOKEN:abc:00ff]",
            "[REMOVED]",
            "do [REMOVED] then [REMOVED]",
        ),
        (
            "prefix [SENSITIVE_SKILL_TOKEN:abc:ff00] suffix",
            "X",
            "prefix X suffix",
        ),
        ("token [SENSITIVE_SKILL_TOKEN:abc:ff00]", "X", "token X"),
        (
            "[SENSITIVE_SKILL_TOKEN:abc:ff00][SENSITIVE_SKILL_TOKEN:abc:00ff]",
            "X",
            "XX",
        ),
        (
            "unclosed [SENSITIVE_SKILL_TOKEN:abc:ff00",
            "X",
            "unclosed [SENSITIVE_SKILL_TOKEN:abc:ff00",
        ),
        ("nothing here", "X", "nothing here"),
    ] {
        assert_eq!(strip_tokens(input, replacement), expected);
    }
}

#[test]
fn rollout_items_contain_token_detects_sensitive_threads() {
    let message_with_token = RolloutItem::ResponseItem(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "use [SENSITIVE_SKILL_TOKEN:abc:ff00]".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    });
    let plain = RolloutItem::ResponseItem(ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::InputText {
            text: "plain".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    });

    assert!(rollout_items_contain_token(&[message_with_token]));
    assert!(!rollout_items_contain_token(&[plain]));
}
