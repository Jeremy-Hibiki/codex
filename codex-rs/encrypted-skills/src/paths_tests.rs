use std::path::PathBuf;

use super::*;

fn dir(path: &str) -> PathBuf {
    PathBuf::from(path)
}

#[test]
fn rewrite_replaces_original_dir_with_decrypted_dir() {
    let text = "Run: bash ~/.codex/skills/foo/scripts/build.sh and read ~/.codex/skills/foo/resources/config.json";
    let out = rewrite_skill_paths(
        text,
        &[(
            dir("~/.codex/skills/foo"),
            dir("/dev/shm/fm-agent-security/fm_skill_security_abc"),
        )],
    );
    assert_eq!(
        out,
        "Run: bash /dev/shm/fm-agent-security/fm_skill_security_abc/scripts/build.sh and read /dev/shm/fm-agent-security/fm_skill_security_abc/resources/config.json"
    );
}

#[test]
fn rewrite_applies_longest_original_first() {
    let text = "run foo/scripts/run.py and foo/scripts/run.py.bak";
    let out = rewrite_skill_paths(
        text,
        &[
            (dir("foo"), dir("/mnt/dec")),
            (dir("foo/scripts/run.py"), dir("/mnt/dec/real.py")),
        ],
    );
    assert_eq!(out, "run /mnt/dec/real.py and /mnt/dec/real.py.bak");
}

#[test]
fn rewrite_leaves_unrelated_text_unchanged() {
    let text = "echo hello && pwd";
    let out = rewrite_skill_paths(text, &[(dir("/a"), dir("/b"))]);
    assert_eq!(out, text);
}

#[test]
fn redact_hides_root_and_subpaths() {
    let text = "script at /dev/shm/fm-agent-security/fm_skill_security_abc/scripts/run.py";
    let out = redact_decrypted_paths(text);
    assert_eq!(out, "script at [REDACTED]");
}

#[test]
fn redact_keeps_surrounding_text() {
    let text = "ok: /dev/shm/fm-agent-security done";
    let out = redact_decrypted_paths(text);
    assert_eq!(out, "ok: [REDACTED] done");
}

#[test]
fn redact_leaves_other_text_unchanged() {
    let text = "no secrets here";
    assert_eq!(redact_decrypted_paths(text), text);
}

#[test]
fn allowed_text_file_extensions() {
    assert!(is_allowed_text_file("SKILL.md"));
    assert!(is_allowed_text_file("resources/config.txt"));
    assert!(is_allowed_text_file("x.MARKDOWN"));
    assert!(!is_allowed_text_file("scripts/run.py"));
    assert!(!is_allowed_text_file("scripts/build.sh"));
}

#[test]
fn read_commands_detected() {
    assert!(is_read_command("cat /x/run.py"));
    assert!(is_read_command("head -5 /x/run.py"));
    assert!(is_read_command("base64 /x/config.json"));
    assert!(!is_read_command("python /x/run.py"));
}

#[test]
fn search_commands_detected() {
    assert!(is_search_command("grep -r secret /x"));
    assert!(is_search_command("find /x -name '*.py'"));
    assert!(is_search_command("ls /x"));
    assert!(!is_search_command("bash /x/run.sh"));
}

#[test]
fn redact_path_prefix_hides_custom_prefix() {
    let text = "dir at /tmp/mem-root/fm_skill_security_abc/SKILL.md end";
    let out = redact_path_prefix(text, "/tmp/mem-root");
    assert_eq!(out, "dir at [REDACTED] end");
}

#[test]
fn script_execution_detected() {
    assert!(is_script_execution("python /x/scripts/run.py"));
    assert!(is_script_execution("bash /x/scripts/build.sh"));
    assert!(!is_script_execution("cat /x/scripts/run.py"));
    assert!(!is_script_execution("python -m pytest"));
    assert!(!is_script_execution("bash -c 'cat /x/scripts/build.sh'"));
    assert!(is_script_execution("bash \"/x/scripts/build.sh\""));
}

#[test]
fn command_references_decrypted_dir() {
    let dirs = vec!["/dev/shm/fm-agent-security/fm_skill_security_abc".to_string()];
    assert!(command_references_dir(
        "ls /dev/shm/fm-agent-security",
        &dirs
    ));
    assert!(command_references_dir(
        "bash /dev/shm/fm-agent-security/fm_skill_security_abc/scripts/run.sh",
        &dirs
    ));
    assert!(!command_references_dir("bash /tmp/run.sh", &dirs));
}

#[test]
fn file_targets_skip_flags_and_redirections() {
    assert_eq!(
        file_targets("cat -n /x/a.md > /tmp/out.txt"),
        vec!["/x/a.md"]
    );
    assert_eq!(file_targets("head -5 /x/b.sh 2>/dev/null"), vec!["/x/b.sh"]);
}

#[test]
fn command_targets_script_detects_script_files() {
    assert!(command_targets_script("cat /x/scripts/run.py"));
    assert!(command_targets_script("head /x/scripts/build.sh"));
    assert!(!command_targets_script("cat /x/notes.md"));
    assert!(!command_targets_script("ls /x/scripts"));
    // Execution is excluded at the guard level, not at the token level.
    assert!(command_targets_script("bash /x/scripts/run.py"));
}

#[test]
fn recursive_search_detected_for_grep_and_rg() {
    assert!(is_recursive_search("grep -r secret /x"));
    assert!(is_recursive_search("grep -rn secret /x"));
    assert!(is_recursive_search("rg secret /x"));
    assert!(!is_recursive_search("grep secret /x/notes.md"));
    assert!(!is_recursive_search("ls /x"));
}

#[test]
fn script_exclusions_injected_for_recursive_search() {
    let out = inject_script_exclusions("grep -r secret /x");
    assert!(out.contains("--exclude='*.sh'"));
    assert!(out.contains("--exclude='*.py'"));
    assert!(out.starts_with("grep -r secret /x"));
}
