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
fn script_file_detection() {
    assert!(is_script_file("/x/scripts/run.py"));
    assert!(is_script_file("/x/scripts/build.sh"));
    assert!(!is_script_file("/x/SKILL.md"));
    assert!(!is_script_file("/x/notes.txt"));
}

#[test]
fn command_references_decrypted_dir() {
    let dirs = vec!["/dev/shm/fm-agent-security/fm_skill_security_abc".to_string()];
    // The shared root constant is always treated as a reference.
    assert!(command_references_dir(
        "ls /dev/shm/fm-agent-security",
        &dirs
    ));
    // A registered decrypted directory is a reference.
    assert!(command_references_dir(
        "bash /dev/shm/fm-agent-security/fm_skill_security_abc/scripts/run.sh",
        &dirs
    ));
    // Glob wildcards still match by substring, so `p*/f*/SKILL.md` hits.
    assert!(command_references_dir(
        "cat /dev/shm/fm-agent-security/p*/f*/SKILL.md",
        &dirs
    ));
    // Unrelated paths do not reference decrypted storage.
    assert!(!command_references_dir("bash /tmp/run.sh", &dirs));
}

#[test]
fn command_references_parent_directory() {
    let dirs = vec!["/dev/shm/fm-agent-security/fm_skill_security_abc".to_string()];
    // 父目录引用可以触达全部解密树：`find /dev/shm` 不拼出 MEM_ROOT 也能列出明文路径。
    for command in [
        "find /dev/shm",
        "ls /dev/shm",
        "du -sh /dev/shm/*",
        "find /dev/shm -exec cat {} \\;",
        "cat /dev/shm/fm-agent-security/p1/fm_skill_security_abc/SKILL.md",
    ] {
        assert!(
            command_references_dir(command, &dirs),
            "must flag parent-directory reference: {command}"
        );
    }
    // 边界：/dev/shmx 不是 /dev/shm 下的路径，不应误报。
    assert!(!command_references_dir("cat /dev/shmx/foo", &dirs));
    assert!(!command_references_dir("bash /tmp/run.sh", &dirs));
}

#[test]
fn split_at_semicolon() {
    assert_eq!(
        split_command_segments("cat /a; cat /b"),
        vec!["cat /a", "cat /b"]
    );
}

#[test]
fn split_at_pipe() {
    assert_eq!(
        split_command_segments("cat /a | grep x"),
        vec!["cat /a", "grep x"]
    );
}

#[test]
fn split_at_logical_and() {
    assert_eq!(
        split_command_segments("bash run.sh && cat /b"),
        vec!["bash run.sh", "cat /b"]
    );
}

#[test]
fn split_at_logical_or() {
    assert_eq!(
        split_command_segments("bash run.sh || cat /b"),
        vec!["bash run.sh", "cat /b"]
    );
}

#[test]
fn split_at_background_ampersand() {
    assert_eq!(
        split_command_segments("sleep 1 & cat /b"),
        vec!["sleep 1", "cat /b"]
    );
}

#[test]
fn split_respects_single_quotes() {
    // Operator inside single quotes is not a separator.
    assert_eq!(
        split_command_segments("bash -c 'cat /a; cat /b'"),
        vec!["bash -c 'cat /a; cat /b'"]
    );
}

#[test]
fn split_respects_double_quotes() {
    assert_eq!(
        split_command_segments("echo \"a; b\" && cat /c"),
        vec!["echo \"a; b\"", "cat /c"]
    );
}

#[test]
fn split_ignores_ampersand_in_redirection() {
    // `2>&1` must NOT be treated as a separator.
    assert_eq!(split_command_segments("cat /a 2>&1"), vec!["cat /a 2>&1"]);
}

#[test]
fn split_drops_empty_segments() {
    assert_eq!(split_command_segments("a ;; b"), vec!["a", "b"]);
}

#[test]
fn split_respects_backslash_escaped_pipe() {
    // `\|` is a literal pipe character, not a separator.
    assert_eq!(split_command_segments("cat a\\|b"), vec!["cat a\\|b"]);
}

#[test]
fn split_respects_backslash_escaped_semicolon() {
    // `\;` is literal, not a separator.
    assert_eq!(split_command_segments("echo a\\;b"), vec!["echo a\\;b"]);
}

#[test]
fn split_at_newline_separator() {
    // A bare newline is a command separator like `;`.
    assert_eq!(
        split_command_segments("cat /a\ncat /b"),
        vec!["cat /a", "cat /b"]
    );
}

#[test]
fn split_at_stderr_pipe() {
    // `|&` (bash stderr pipe) is a distinct two-char separator.
    assert_eq!(
        split_command_segments("bash run.sh |& cat /b"),
        vec!["bash run.sh", "cat /b"]
    );
}

#[test]
fn split_adjacent_pipe_and_ampersand() {
    // `;&` — `;` splits, `&` is background operator on the empty right
    // segment (dropped). No panic, no mis-split.
    assert_eq!(
        split_command_segments("cat /a;& cat /b"),
        vec!["cat /a", "cat /b"]
    );
}

#[test]
fn split_adjacent_semicolon_and_pipe() {
    // `;|` — `;` then `|` both adjacent. Each splits independently.
    assert_eq!(split_command_segments("a ;| b"), vec!["a", "b"]);
}

#[test]
fn script_execution_avoids_plain_argument_paths() {
    let dir = format!("{MEM_ROOT}/p1/fm_skill_security_abc");
    let guarded = vec![dir.clone()];
    assert!(script_execution_avoids_guarded_io(
        &format!("bash {dir}/scripts/build.sh"),
        &guarded,
    ));
    assert!(script_execution_avoids_guarded_io(
        &format!("bash {dir}/scripts/build.sh --input {dir}/resources/config.json"),
        &guarded,
    ));
    assert!(script_execution_avoids_guarded_io(
        &format!("bash {dir}/scripts/build.sh > /tmp/out.log 2>&1"),
        &guarded,
    ));
    assert!(script_execution_avoids_guarded_io(
        &format!("bash {dir}/scripts/build.sh '$(cat /tmp/not-guarded.txt)'"),
        &guarded,
    ));
}

#[test]
fn script_execution_blocks_guarded_io_channels() {
    let dir = format!("{MEM_ROOT}/p1/fm_skill_security_abc");
    let guarded = vec![dir.clone()];
    let cases = [
        format!("bash {dir}/scripts/build.sh < {dir}/SKILL.md"),
        format!("bash {dir}/scripts/build.sh <{dir}/SKILL.md"),
        format!("bash {dir}/scripts/build.sh \"$(cat {dir}/SKILL.md)\""),
        format!("bash {dir}/scripts/build.sh `cat {dir}/SKILL.md`"),
        format!("bash {dir}/scripts/build.sh <(cat {dir}/SKILL.md)"),
        format!("bash {dir}/scripts/build.sh <<< \"$(cat {dir}/SKILL.md)\""),
    ];
    for command in cases {
        assert!(
            !script_execution_avoids_guarded_io(&command, &guarded),
            "must block guarded read channel: {command}"
        );
    }
}

#[test]
fn script_execution_blocks_heredoc_expansion() {
    let dir = format!("{MEM_ROOT}/p1/fm_skill_security_abc");
    let guarded = vec![dir.clone()];
    let command = format!("bash {dir}/scripts/build.sh <<EOF\n$(cat {dir}/SKILL.md)\nEOF");
    assert!(!script_execution_avoids_guarded_io(&command, &guarded));
}
