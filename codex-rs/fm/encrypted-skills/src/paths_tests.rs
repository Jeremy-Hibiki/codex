use std::path::PathBuf;

use super::*;

fn dir(path: &str) -> PathBuf {
    PathBuf::from(path)
}

#[test]
fn rewrite_skill_paths_replaces_mappings() {
    let cases = vec![
        (
            "Run: bash ~/.codex/skills/foo/scripts/build.sh and read ~/.codex/skills/foo/resources/config.json",
            vec![(
                dir("~/.codex/skills/foo"),
                dir("/dev/shm/fm-agent-security/fm_skill_security_abc"),
            )],
            "Run: bash /dev/shm/fm-agent-security/fm_skill_security_abc/scripts/build.sh and read /dev/shm/fm-agent-security/fm_skill_security_abc/resources/config.json",
        ),
        (
            "run foo/scripts/run.py and foo/scripts/run.py.bak",
            vec![
                (dir("foo"), dir("/mnt/dec")),
                (dir("foo/scripts/run.py"), dir("/mnt/dec/real.py")),
            ],
            "run /mnt/dec/real.py and /mnt/dec/real.py.bak",
        ),
        (
            "echo hello && pwd",
            vec![(dir("/a"), dir("/b"))],
            "echo hello && pwd",
        ),
    ];
    for (text, mappings, expected) in cases {
        assert_eq!(rewrite_skill_paths(text, &mappings), expected);
    }
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
fn redact_path_prefix_hides_multiple_occurrences_and_edges() {
    assert_eq!(
        redact_path_prefix("/tmp/mem-root/a /tmp/mem-root/b", "/tmp/mem-root"),
        "[REDACTED] [REDACTED]"
    );
    assert_eq!(
        redact_path_prefix("/tmp/mem-root", "/tmp/mem-root"),
        "[REDACTED]"
    );
    assert_eq!(
        redact_path_prefix("x/tmp/mem-root", "/tmp/mem-root"),
        "x[REDACTED]"
    );
}

#[test]
fn script_execution_detected() {
    assert!(is_script_execution("python /x/scripts/run.py"));
    assert!(is_script_execution("bash /x/scripts/build.sh"));
    assert!(is_script_execution("bash -e /x/scripts/build.sh"));
    assert!(!is_script_execution("cat /x/scripts/run.py"));
    assert!(!is_script_execution("python -m pytest"));
    assert!(!is_script_execution("bash -c 'cat /x/scripts/build.sh'"));
    assert!(!is_script_execution("sudo bash /x/scripts/build.sh"));
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
    assert!(!command_references_dir("cat /dev/shm-backup/foo", &dirs));
    assert!(!command_references_dir("bash /tmp/run.sh", &dirs));
}

#[test]
fn split_at_chain_operators() {
    for (command, expected) in [
        ("cat /a; cat /b", vec!["cat /a", "cat /b"]),
        ("cat /a | grep x", vec!["cat /a", "grep x"]),
        ("bash run.sh && cat /b", vec!["bash run.sh", "cat /b"]),
        ("bash run.sh || cat /b", vec!["bash run.sh", "cat /b"]),
        ("sleep 1 & cat /b", vec!["sleep 1", "cat /b"]),
        ("cat /a\ncat /b", vec!["cat /a", "cat /b"]),
        ("bash run.sh |& cat /b", vec!["bash run.sh", "cat /b"]),
        ("cat /a;& cat /b", vec!["cat /a", "cat /b"]),
        ("a ;| b", vec!["a", "b"]),
        ("a ;; b", vec!["a", "b"]),
        ("echo \"a; b\" && cat /c", vec!["echo \"a; b\"", "cat /c"]),
    ] {
        assert_eq!(
            split_command_segments(command),
            expected,
            "command: {command}"
        );
    }
}

#[test]
fn split_ignores_quoted_escaped_and_redirection_operators() {
    for command in [
        "bash -c 'cat /a; cat /b'",
        "cat /a 2>&1",
        "cat a\\|b",
        "echo a\\;b",
    ] {
        assert_eq!(
            split_command_segments(command),
            vec![command],
            "command: {command}"
        );
    }
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
        format!("bash {dir}/scripts/build.sh >(cat {dir}/SKILL.md)"),
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
