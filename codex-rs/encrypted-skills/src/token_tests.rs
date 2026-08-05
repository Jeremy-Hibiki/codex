use super::*;

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
    let text = "prefix [SENSITIVE_SKILL_TOKEN:abc:ff00] suffix";
    assert_eq!(Token::parse(text), Some(Token::new("abc", "ff00")));
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
    assert_eq!(Token::parse("[SENSITIVE_SKILL_TOKEN::ff00]"), None);
}

#[test]
fn generate_produces_32_hex_chars_and_unique_values() {
    let a = Token::generate("thread-1");
    let b = Token::generate("thread-1");
    assert_eq!(a.session_id, "thread-1");
    assert_eq!(a.hex.len(), TOKEN_HEX_CHARS);
    assert!(is_hex(&a.hex));
    assert_ne!(a.hex, b.hex);
}

#[test]
fn is_hex_rejects_empty_and_non_hex() {
    assert!(!is_hex(""));
    assert!(!is_hex("zz"));
    assert!(is_hex("0123456789abcdef"));
}
#[test]
fn strip_tokens_replaces_all_sentinels() {
    let text = "do [SENSITIVE_SKILL_TOKEN:abc:ff00] then [SENSITIVE_SKILL_TOKEN:abc:00ff]";
    assert_eq!(
        strip_tokens(text, "[REMOVED]"),
        "do [REMOVED] then [REMOVED]"
    );
}

#[test]
fn strip_tokens_keeps_surrounding_text() {
    let text = "prefix [SENSITIVE_SKILL_TOKEN:abc:ff00] suffix";
    assert_eq!(strip_tokens(text, "X"), "prefix X suffix");
}

#[test]
fn strip_tokens_without_sentinels_is_identity() {
    assert_eq!(strip_tokens("nothing here", "X"), "nothing here");
}
