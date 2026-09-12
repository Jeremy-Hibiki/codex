use pretty_assertions::assert_eq;

use super::Mode;
use super::detect_mode;

fn write_skill_md(dir: &std::path::Path, mode: Option<&str>) {
    let mode_block = match mode {
        Some(mode) => format!("  encryption:\n    mode: {mode}\n"),
        None => String::new(),
    };
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nmetadata:\n  encrypted: true\n{mode_block}---\n\n<!-- ENCRYPTED:SKILL -->\n"),
    )
    .unwrap();
}

#[test]
fn frontmatter_mode_wins() {
    let tmp = tempfile::tempdir().unwrap();
    for (raw, expected) in [
        ("mock", Mode::Mock),
        ("software", Mode::Software),
        ("ukey", Mode::Ukey),
        ("ukey_two_phase", Mode::UkeyTwoPhase),
        ("ukey-two-phase", Mode::UkeyTwoPhase),
    ] {
        let dir = tmp.path().join(raw.replace('-', "_"));
        std::fs::create_dir_all(&dir).unwrap();
        write_skill_md(&dir, Some(raw));
        assert_eq!(detect_mode(&dir), expected, "mode {raw}");
    }
}

#[test]
fn key_enc_presence_selects_two_phase() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skill");
    std::fs::create_dir_all(&dir).unwrap();
    write_skill_md(&dir, None);
    std::fs::write(dir.join("key.enc"), b"wrapped-key").unwrap();
    assert_eq!(detect_mode(&dir), Mode::UkeyTwoPhase);
}

#[test]
fn missing_mode_and_key_falls_back_to_ukey() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skill");
    std::fs::create_dir_all(&dir).unwrap();
    write_skill_md(&dir, None);
    assert_eq!(detect_mode(&dir), Mode::Ukey);
}

#[test]
fn unknown_mode_falls_back() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skill");
    std::fs::create_dir_all(&dir).unwrap();
    write_skill_md(&dir, Some("rot13"));
    assert_eq!(detect_mode(&dir), Mode::Ukey);
    std::fs::write(dir.join("key.enc"), b"wrapped-key").unwrap();
    assert_eq!(detect_mode(&dir), Mode::UkeyTwoPhase);
}
