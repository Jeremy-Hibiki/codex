use super::*;

#[test]
fn exact_plaintext_matches() {
    assert!(contains_known_plaintext(
        "prefix SECRET-CONTENT suffix",
        &["SECRET-CONTENT"]
    ));
}

#[test]
fn full_line_and_prefix_match() {
    let plaintext = "first line\nthis is a long sensitive line inside the skill\nlast line";
    assert!(contains_known_plaintext(
        "...this is a long sensitive line inside the skill...",
        &[plaintext]
    ));
    assert!(contains_known_plaintext(
        "prefix this is a long sensi",
        &[plaintext]
    ));
}

#[test]
fn short_fragments_are_not_matched() {
    // A short line inside a known plaintext does not match on its own.
    let plaintext = "line one\napi_key=abc\nline three";
    assert!(!contains_known_plaintext("api_key=abc", &[plaintext]));
}

#[test]
fn no_known_plaintext_passes() {
    assert!(!contains_known_plaintext(
        "hello world",
        &["totally different"]
    ));
}

#[test]
fn args_are_checked_individually() {
    let known = ["sensitive skill content here"];
    let args = [
        r#"{"filePath":"/tmp/out.txt","content":"sensitive skill content here"}"#,
        "/tmp/out.txt",
    ];
    assert!(args_contain_plaintext(&args, &known));
    assert!(!args_contain_plaintext(&["/tmp/out.txt", "normal"], &known));
}

#[test]
fn json_escaped_newline_does_not_bypass() {
    let plaintext = "line one\nline two";
    assert!(!contains_known_plaintext(
        r#"line one\nline two"#,
        &[plaintext]
    ));
}

#[test]
fn redact_exact_plaintext() {
    assert_eq!(
        redact_known_plaintext("prefix SECRET-CONTENT suffix", &["SECRET-CONTENT"]),
        "prefix [REDACTED] suffix"
    );
}

#[test]
fn redact_full_line_and_prefix() {
    let plaintext = "first line\nthis is a long sensitive line inside the skill\nlast line";
    assert_eq!(
        redact_known_plaintext(
            "quote: this is a long sensitive line inside the skill",
            &[plaintext]
        ),
        "quote: [REDACTED]"
    );
    assert_eq!(
        redact_known_plaintext("prefix this is a long sensi", &[plaintext]),
        "prefix [REDACTED]"
    );
}

#[test]
fn redact_short_fragments_are_not_redacted() {
    let plaintext = "line one\napi_key=abc\nline three";
    assert_eq!(
        redact_known_plaintext("prefix api_key=abc suffix", &[plaintext]),
        "prefix api_key=abc suffix"
    );
}

#[test]
fn redact_short_line_quoted_as_a_complete_line() {
    let plaintext = "line one\napi_key=abc\nline three";
    assert_eq!(
        redact_known_plaintext("the key is:\napi_key=abc\n", &[plaintext]),
        "the key is:\n[REDACTED]\n"
    );
    assert_eq!(
        redact_known_plaintext("api_key=abc", &[plaintext]),
        "[REDACTED]"
    );
}

#[test]
fn redact_short_line_embedded_in_a_sentence_is_kept() {
    let plaintext = "line one\napi_key=abc\nline three";
    assert_eq!(
        redact_known_plaintext("value api_key=abc here", &[plaintext]),
        "value api_key=abc here"
    );
}

#[test]
fn redact_unknown_text_is_unchanged() {
    assert_eq!(
        redact_known_plaintext("hello world", &["totally different"]),
        "hello world"
    );
}

#[test]
fn redact_multiple_known_plaintexts() {
    assert_eq!(
        redact_known_plaintext("alpha beta gamma", &["alpha", "gamma"]),
        "[REDACTED] beta [REDACTED]"
    );
}

#[test]
fn redact_is_idempotent() {
    let plaintext = "first line\nthis is a long sensitive line inside the skill\nlast line";
    let text = "quote: this is a long sensitive line inside the skill";
    let once = redact_known_plaintext(text, &[plaintext]);
    assert_eq!(redact_known_plaintext(&once, &[plaintext]), once);
}

#[test]
fn redact_short_line_is_idempotent() {
    let plaintext = "line one\napi_key=abc\nline three";
    let once = redact_known_plaintext("the key is:\napi_key=abc\n", &[plaintext]);
    assert_eq!(redact_known_plaintext(&once, &[plaintext]), once);
}

#[test]
fn redact_quoted_mid_line_fragment() {
    let plaintext = "first line\nthis is a long sensitive line inside the skill\nlast line";
    assert_eq!(
        redact_known_plaintext("模型回复「long sensitive line」", &[plaintext]),
        "模型回复「[REDACTED]」"
    );
}

#[test]
fn redact_quoted_line_prefix_fragment() {
    let plaintext = "first line\nthis is a long sensitive line inside the skill\nlast line";
    assert_eq!(
        redact_known_plaintext("quote \"this is a long sensi\" more", &[plaintext]),
        "quote \"[REDACTED]\" more"
    );
}

#[test]
fn short_quoted_fragment_is_kept() {
    let plaintext = "line one\napi_key=abc\nline three";
    assert_eq!(
        redact_known_plaintext("他说「abc」", &[plaintext]),
        "他说「abc」"
    );
}

#[test]
fn redact_quoted_fragment_is_idempotent() {
    let plaintext = "first line\nthis is a long sensitive line inside the skill\nlast line";
    let once = redact_known_plaintext("模型回复「long sensitive line」", &[plaintext]);
    assert_eq!(redact_known_plaintext(&once, &[plaintext]), once);
}
