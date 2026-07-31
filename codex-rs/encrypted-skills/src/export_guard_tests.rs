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
