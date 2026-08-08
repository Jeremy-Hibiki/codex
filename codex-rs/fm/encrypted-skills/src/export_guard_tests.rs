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
fn middle_fragment_matches() {
    let line = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ";
    let plaintext = format!("first line\n{line}\nlast line");
    let middle = &line[8..28]; // "ijklmnopqrstuvwxyzAB" — neither line start nor full line
    assert!(contains_known_plaintext(
        &format!("output: {middle} suffix"),
        &[plaintext.as_str()]
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
fn redact_middle_fragment() {
    let line = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ";
    let plaintext = format!("first line\n{line}\nlast line");
    let middle = &line[8..28];
    let out = redact_known_plaintext(&format!("output: {middle} suffix"), &[plaintext.as_str()]);
    assert!(!out.contains(middle));
    assert!(out.contains("[REDACTED]"));
}

#[test]
fn normalized_variants_match() {
    let plaintext = "The quick brown fox jumps over the lazy dog";
    let variants = [
        "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG",
        "The.quick.brown.fox.jumps.over.the.lazy.dog",
        "**The quick brown fox** jumps over the lazy dog",
        "[The quick brown fox jumps over the lazy dog](https://example.com)",
        "The  quick\tbrown fox jumps over the lazy dog!",
        "The-quick-brown-fox-jumps-over-the-lazy-dog",
    ];
    for variant in variants {
        assert!(
            contains_known_plaintext(variant, &[plaintext]),
            "normalized variant must match: {variant}"
        );
    }
}

#[test]
fn fullwidth_variants_match() {
    let plaintext = "skill content alpha";
    assert!(contains_known_plaintext(
        "ｓｋｉｌｌ ｃｏｎｔｅｎｔ ａｌｐｈａ",
        &[plaintext]
    ));
}

#[test]
fn redact_normalized_line_variant() {
    let plaintext = "The quick brown fox jumps over the lazy dog";
    assert_eq!(
        redact_known_plaintext("THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG", &[plaintext]),
        "[REDACTED]"
    );
    assert_eq!(
        redact_known_plaintext(
            "see The.quick.brown.fox.jumps.over.the.lazy.dog",
            &[plaintext]
        ),
        "[REDACTED]"
    );
}

#[test]
fn normalized_short_line_redacts_as_complete_line_only() {
    let known = ["api_key=abc"];
    assert_eq!(redact_known_plaintext("API_KEY=ABC", &known), "[REDACTED]");
    assert_eq!(
        redact_known_plaintext("value API_KEY=ABC here", &known),
        "value API_KEY=ABC here"
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
fn redaction_is_idempotent_across_fragment_kinds() {
    let long = "first line\nthis is a long sensitive line inside the skill\nlast line";
    let short = "line one\napi_key=abc\nline three";
    let line = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ";
    let middle_plaintext = format!("first line\n{line}\nlast line");
    let cases: [(&str, &str); 4] = [
        (
            "quote: this is a long sensitive line inside the skill",
            long,
        ),
        ("the key is:\napi_key=abc\n", short),
        ("output: ijklmnopqrstuvwxyzAB suffix", &middle_plaintext),
        ("模型回复「long sensitive line」", long),
    ];
    for (text, plaintext) in cases {
        let once = redact_known_plaintext(text, std::slice::from_ref(&plaintext));
        assert_eq!(
            redact_known_plaintext(&once, std::slice::from_ref(&plaintext)),
            once
        );
    }
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
fn fragment_length_boundaries() {
    let line = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ";
    let plaintext = format!("first line\n{line}\nlast line");
    // A 19-char fragment must not match; a 20-char fragment must.
    assert!(!contains_known_plaintext(
        &line[..19],
        &[plaintext.as_str()]
    ));
    assert!(contains_known_plaintext(&line[..20], &[plaintext.as_str()]));
    // Quoted fragments: 11 chars are kept, 12 chars (the quoted minimum) are
    // redacted.
    assert_eq!(
        redact_known_plaintext(
            &format!("quote \"{}\" more", &line[..11]),
            &[plaintext.as_str()]
        ),
        format!("quote \"{}\" more", &line[..11])
    );
    assert_eq!(
        redact_known_plaintext(
            &format!("quote \"{}\" more", &line[..12]),
            &[plaintext.as_str()]
        ),
        "quote \"[REDACTED]\" more"
    );
}

#[test]
fn empty_known_and_empty_text_are_safe() {
    assert!(!contains_known_plaintext("anything", &[]));
    assert_eq!(redact_known_plaintext("anything", &[]), "anything");
    assert!(!contains_known_plaintext("", &["known"]));
}

#[test]
fn redact_handles_crlf_and_leading_whitespace() {
    let short = "api_key=abc";
    assert_eq!(
        redact_known_plaintext("  api_key=abc\r\nnext\r\n", &[short]),
        "  [REDACTED]\r\nnext\r\n"
    );
    let long = "this is a long sensitive line inside the skill";
    assert_eq!(
        redact_known_plaintext(
            "  this is a long sensitive line inside the skill\r\n",
            &[long]
        ),
        "  [REDACTED]\r\n"
    );
}
