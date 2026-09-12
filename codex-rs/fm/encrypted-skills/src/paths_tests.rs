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
fn command_references_lexical_path_aliases() {
    let dirs = vec!["/dev/shm/fm-agent-security/fm_skill_security_abc".to_string()];
    // 点段与符号链接别名不含字面 `/dev/shm`，但词法归一后必须命中。
    for command in [
        "cat /dev/shm/./fm-agent-security/p1/fm_skill_security_abc/SKILL.md",
        "cat /dev//shm/fm-agent-security/p1/fm_skill_security_abc/SKILL.md",
        "cat /run/shm/fm-agent-security/p1/fm_skill_security_abc/SKILL.md",
        "cat /dev/shm/fm-agent-security/./p1/fm_skill_security_abc/SKILL.md",
        // `$VAR` 开头的路径无法词法解析，保守视为可能命中。
        "cat $ROOT/fm_skill_security_abc/SKILL.md",
    ] {
        assert!(
            command_references_dir(command, &dirs),
            "must flag lexical alias: {command}"
        );
    }
    // 归一不得扩大边界匹配。
    assert!(!command_references_dir("cat /dev/shmx/foo", &dirs));
    assert!(!command_references_dir("cat /run/shmx/foo", &dirs));
    assert!(!command_references_dir("bash /tmp/run.sh", &dirs));
    // 非路径形状的变量引用（不含 `/`）不视为路径引用。
    assert!(!command_references_dir("echo $HOME", &dirs));
}

// ---- HIGH-1 ① glob 元字符顶替 ----

#[test]
fn glob_tokens_anchored_on_guarded_dirs_are_flagged() {
    let dirs = vec!["/dev/shm/fm-agent-security/p1/x".to_string()];
    for command in [
        "cat /dev/s*/fm-agent-security/p1/x/SKILL.md",
        "cat /dev/*/fm-agent-security/p1/x/SKILL.md",
        "cat /dev/sh*/fm-agent-security/p1/x/SKILL.md",
        "cat /dev/sh?/fm-agent-security/p1/x/SKILL.md",
        "cat /dev/sh[m]/fm-agent-security/p1/x/SKILL.md",
        "cat /dev/s*/fm-agent-security/p1/x/*.md",
        // `/run/shm` 别名叠加 glob。
        "cat /run/sh*/fm-agent-security/p1/x/SKILL.md",
        // brace 展开。
        "cat /dev/sh{m,M}/fm-agent-security/p1/x/SKILL.md",
        "cat /dev/{shm,tmp}/fm-agent-security/p1/x/SKILL.md",
    ] {
        assert!(
            command_references_dir(command, &dirs),
            "glob token anchored on a guarded dir must be flagged: {command}"
        );
    }
}

#[test]
fn unanchored_glob_tokens_are_not_flagged() {
    let dirs = vec!["/dev/shm/fm-agent-security/p1/x".to_string()];
    for command in [
        // 纯通配、无受控目录锚点：不能全盘误拦。
        "cat /dev/*",
        "echo *",
        "cat /usr/local/*/bin",
        // 固定前缀够不到受控目录的其余分量。
        "cat /dev/s*/mouse",
        "cat /opt/*/notes.txt",
    ] {
        assert!(
            !command_references_dir(command, &dirs),
            "unanchored glob must stay allowed: {command}"
        );
    }
}

// ---- HIGH-1 ② 词中引号切分 ----

#[test]
fn quote_split_guarded_paths_are_flagged() {
    let dirs = vec!["/dev/shm/fm-agent-security/p1/x".to_string()];
    for command in [
        "cat /dev/sh\"m/fm\"-agent-security/p1/x/SKILL.md",
        "cat /dev/sh'm/fm'-agent-security/p1/x/SKILL.md",
        "cat /dev/\"shm\"/fm-agent-security/p1/x/SKILL.md",
    ] {
        assert!(
            command_references_dir(command, &dirs),
            "quote-split guarded path must be flagged: {command}"
        );
    }
    // 去引号后不指向受控目录的词保持放行。
    assert!(!command_references_dir("cat /da\"t\"a/x/SKILL.md", &dirs));
}

// ---- HIGH-1 ③ cd 链 cwd 状态 ----

#[test]
fn cd_chain_resolves_relative_guarded_reads() {
    let dirs = vec!["/dev/shm/fm-agent-security/p1/x".to_string()];
    for command in [
        "cd /dev && cd shm && cat fm-agent-security/p1/x/SKILL.md",
        "cd /dev && pushd shm && cat fm-agent-security/p1/x/SKILL.md",
        "cd /dev/shm/fm-agent-security/p1 && cd x && cat SKILL.md",
        "cd /dev/shm/fm-agent-security && cat p1/x/SKILL.md",
        "cd /dev && cd shm && cat ./fm-agent-security/p1/x/SKILL.md",
        // 相对词 + glob，经虚拟 cwd 解析后命中。
        "cd /dev/shm && cat fm*/SKILL.md",
    ] {
        let segments = split_command_segments(command);
        let flags = segment_guarded_flags(&segments, &dirs);
        assert!(
            flags.iter().any(|flag| *flag),
            "cd-chain relative guarded read must be flagged: {command} → {flags:?}"
        );
    }
}

#[test]
fn cd_chain_dotdot_moves_virtual_cwd() {
    let dirs = vec!["/dev/shm/fm-agent-security/p1/x".to_string()];
    // 进入 `/dev/shm` 本身就是受控引用（既有策略）；`..` 弹栈后必须离开
    // 受控树：`cd ..` 段与其后的相对读取都不得命中。
    let segments = split_command_segments("cd /dev/shm && cd .. && cat fm-agent-security/x/f");
    let flags = segment_guarded_flags(&segments, &dirs);
    assert!(flags[0], "entering /dev/shm is itself a guarded reference");
    // 段 2 的动词词 `cd` 经虚拟 cwd 连接成 `/dev/shm/cd`（保守产物，整体
    // 判定不变）；`..` 弹栈正确性由段 3 证明：离开受控树后的相对读取
    // `/dev/fm-agent-security/...` 不得命中。
    assert!(
        !flags[2],
        "cd .. out of the guarded tree must stay allowed: {flags:?}"
    );
    // 再进受控目录则必须命中（命中的是回到树内的相对读取段）。
    let segments = split_command_segments(
        "cd /dev/shm && cd .. && cd shm && cat fm-agent-security/p1/x/SKILL.md",
    );
    let flags = segment_guarded_flags(&segments, &dirs);
    assert!(
        flags.last().is_some_and(|flag| *flag),
        "re-entering the guarded tree must be flagged: {flags:?}"
    );
}

#[test]
fn unresolvable_cd_target_falls_back_to_tail_heuristic() {
    let dirs = vec!["/dev/shm/fm-agent-security/p1/fm_skill_security_abc".to_string()];
    for command in [
        "cd $DIR && cat fm-agent-security/p1/x/SKILL.md",
        "cd $(pwd) && cat fm_skill_security_abc/SKILL.md",
        "cd /tmp && cd $D && cat fm_skill_security_abc/scripts/run.sh",
    ] {
        let segments = split_command_segments(command);
        let flags = segment_guarded_flags(&segments, &dirs);
        assert!(
            flags.iter().any(|flag| *flag),
            "relative tail of a guarded dir after an unresolvable cd must be flagged: {command} → {flags:?}"
        );
    }
}

#[test]
fn unresolvable_cd_target_ignores_unrelated_relative_paths() {
    let dirs = vec!["/dev/shm/fm-agent-security/p1/fm_skill_security_abc".to_string()];
    let segments = split_command_segments("cd $D && cat notes.txt && ls src");
    let flags = segment_guarded_flags(&segments, &dirs);
    assert!(
        flags.iter().all(|flag| !flag),
        "unrelated relative paths after an unresolvable cd must stay allowed: {flags:?}"
    );
}

#[test]
fn cd_chain_relative_skill_script_stays_executable_for_d9() {
    let dirs = vec!["/dev/shm/fm-agent-security/p1/fm_skill_security_abc".to_string()];
    // D9 语义由上层判定；paths 层只需保证：相对 skill 脚本在保守模式下算
    // 受控引用，同时仍被识别为无 IO 通道的脚本执行。
    let segments = split_command_segments("cd $D && python fm_skill_security_abc/scripts/run.py");
    let flags = segment_guarded_flags(&segments, &dirs);
    assert!(
        flags[1],
        "relative skill script must count as guarded reference"
    );
    assert!(is_script_execution(&segments[1]));
    assert!(
        script_execution_avoids_guarded_io(&segments[1], &dirs),
        "plain script argument must avoid guarded io"
    );
}

#[test]
fn script_execution_blocks_alias_io_channels() {
    let dir = format!("{MEM_ROOT}/p1/fm_skill_security_abc");
    let guarded = vec![dir.clone()];
    // 点段别名使重定向目标不含字面 guarded 目录。
    let dot_alias = "/dev/shm/./fm-agent-security/p1/fm_skill_security_abc".to_string();
    let cases = [
        format!("bash {dir}/scripts/build.sh < {dot_alias}/SKILL.md"),
        format!("bash {dir}/scripts/build.sh \"$(cat {dot_alias}/SKILL.md)\""),
    ];
    for command in cases {
        assert!(
            !script_execution_avoids_guarded_io(&command, &guarded),
            "alias-shaped guarded io channel must be blocked: {command}"
        );
    }
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

// ---- differential: tree-sitter implementation vs legacy fallback ----

fn guarded_dir() -> String {
    format!("{MEM_ROOT}/p1/fm_skill_security_abc")
}

fn corpus() -> Vec<(String, Vec<String>)> {
    let d = guarded_dir();
    vec![
        ("ls /dev/shm/fm-agent-security".to_string(), vec![d.clone()]),
        (format!("bash {d}/scripts/run.sh"), vec![d.clone()]),
        (
            "cat /dev/shm/fm-agent-security/p*/f*/SKILL.md".to_string(),
            vec![d.clone()],
        ),
        ("bash /tmp/run.sh".to_string(), vec![d.clone()]),
        ("cat /a; cat /b".to_string(), vec![d.clone()]),
        ("cat /a | grep x".to_string(), vec![d.clone()]),
        ("bash run.sh && cat /b".to_string(), vec![d.clone()]),
        ("bash run.sh || cat /b".to_string(), vec![d.clone()]),
        ("sleep 1 & cat /b".to_string(), vec![d.clone()]),
        ("bash -c 'cat /a; cat /b'".to_string(), vec![d.clone()]),
        ("echo \"a; b\" && cat /c".to_string(), vec![d.clone()]),
        ("cat /a 2>&1".to_string(), vec![d.clone()]),
        ("a ;; b".to_string(), vec![d.clone()]),
        ("cat a\\|b".to_string(), vec![d.clone()]),
        ("echo a\\;b".to_string(), vec![d.clone()]),
        ("cat /a\ncat /b".to_string(), vec![d.clone()]),
        ("bash run.sh |& cat /b".to_string(), vec![d.clone()]),
        ("cat /a;& cat /b".to_string(), vec![d.clone()]),
        ("a ;| b".to_string(), vec![d.clone()]),
        (format!("bash {d}/scripts/build.sh"), vec![d.clone()]),
        (
            format!("bash {d}/scripts/build.sh --input {d}/resources/config.json"),
            vec![d.clone()],
        ),
        (
            format!("bash {d}/scripts/build.sh > /tmp/out.log 2>&1"),
            vec![d.clone()],
        ),
        (
            format!("bash {d}/scripts/build.sh '$(cat /tmp/not-guarded.txt)'"),
            vec![d.clone()],
        ),
        (
            format!("bash {d}/scripts/build.sh < {d}/SKILL.md"),
            vec![d.clone()],
        ),
        (
            format!("bash {d}/scripts/build.sh <{d}/SKILL.md"),
            vec![d.clone()],
        ),
        (
            format!("bash {d}/scripts/build.sh \"$(cat {d}/SKILL.md)\""),
            vec![d.clone()],
        ),
        (
            format!("bash {d}/scripts/build.sh `cat {d}/SKILL.md`"),
            vec![d.clone()],
        ),
        (
            format!("bash {d}/scripts/build.sh <(cat {d}/SKILL.md)"),
            vec![d.clone()],
        ),
        (
            format!("bash {d}/scripts/build.sh <<< \"$(cat {d}/SKILL.md)\""),
            vec![d.clone()],
        ),
        (
            format!("bash {d}/scripts/build.sh <<EOF\n$(cat {d}/SKILL.md)\nEOF"),
            vec![d.clone()],
        ),
        // ---- 绕过形状样例 ----
        (format!("bash -lc \"cat {d}/SKILL.md\""), vec![d.clone()]),
        (format!("bash -lc 'cat {d}/SKILL.md'"), vec![d.clone()]),
        (format!("cat '{d}/SKILL.md'"), vec![d.clone()]),
        (format!("cat \"{d}/SKILL.md\""), vec![d.clone()]),
        (format!("cat {d}/SKILL.md 2>&1"), vec![d.clone()]),
        (format!("echo x > {d}/out"), vec![d.clone()]),
        (format!("cat $(echo {d}/SKILL.md)"), vec![d.clone()]),
        (format!("cat `echo {d}/SKILL.md`"), vec![d.clone()]),
        (format!("eval cat {d}/SKILL.md"), vec![d.clone()]),
        (format!("v={d}; cat \"$v/SKILL.md\""), vec![d.clone()]),
        (format!("printf '%s\\n' {d}/SKILL.md"), vec![d.clone()]),
        (format!("cat {d}/SKI\"LL.md\""), vec![d.clone()]),
        (format!("# {d}/SKILL.md"), vec![d.clone()]),
        (
            format!("cat {d}/SKILL.md # trailing comment"),
            vec![d.clone()],
        ),
        (
            "cat /dev/shm/fm-agent-securit*/p*/fm_skill_security_abc/SKILL.md".to_string(),
            vec![d.clone()],
        ),
        // HIGH-1 ①: glob 元字符顶替受控目录分量。
        (
            "cat /dev/s*/fm-agent-security/p1/fm_skill_security_abc/SKILL.md".to_string(),
            vec![d.clone()],
        ),
        (
            "cat /dev/*/fm-agent-security/p1/fm_skill_security_abc/SKILL.md".to_string(),
            vec![d.clone()],
        ),
        (
            "cat /dev/{shm,tmp}/fm-agent-security/p1/fm_skill_security_abc/SKILL.md".to_string(),
            vec![d.clone()],
        ),
        // HIGH-1 ②: 词中引号切分。
        (
            "cat /dev/sh\"m/fm\"-agent-security/p1/fm_skill_security_abc/SKILL.md".to_string(),
            vec![d.clone()],
        ),
        // HIGH-1 ③: cd 链（分段判定层不感知 cwd，两种切分实现须一致）。
        (
            "cd /dev && cd shm && cat fm-agent-security/p1/fm_skill_security_abc/SKILL.md"
                .to_string(),
            vec![d.clone()],
        ),
        (
            "cd $D && cat fm_skill_security_abc/SKILL.md".to_string(),
            vec![d.clone()],
        ),
        (format!("cd {d} && cat SKILL.md"), vec![d.clone()]),
        (format!("sh -c 'cat {d}/SKILL.md'"), vec![d.clone()]),
        (
            format!("python3 -c 'open(\"{d}/SKILL.md\")'"),
            vec![d.clone()],
        ),
        (format!("cat <<'EOF'\n{d}/SKILL.md\nEOF"), vec![d.clone()]),
        ("find /dev/shm".to_string(), vec![d.clone()]),
        ("ls /dev/shm".to_string(), vec![d.clone()]),
        ("cat /dev/shmx/foo".to_string(), vec![d.clone()]),
        ("echo hi # ; cat /b".to_string(), vec![d]),
    ]
}

fn parse_failures() -> Vec<String> {
    corpus()
        .into_iter()
        .filter(|(cmd, _)| parse_shell(cmd).is_none_or(|tree| tree.root_node().has_error()))
        .map(|(cmd, _)| cmd)
        .collect()
}

/// Documented, intentional divergences between the legacy fallback scanner
/// and the tree-sitter implementation (each is a semantic improvement in the
/// tree-sitter implementation).
fn expected_mismatch(kind: &str, cmd: &str) -> Option<&'static str> {
    let d = guarded_dir();
    match (kind, cmd) {
        ("references", cmd) if cmd == format!("# {d}/SKILL.md") => {
            Some("comment-only mention is not an executing read; legacy flags it")
        }
        ("split", "echo hi # ; cat /b") => {
            Some("`;` inside a comment is not a separator; legacy mis-splits")
        }
        ("split", cmd)
            if cmd.starts_with(
                "bash /dev/shm/fm-agent-security/p1/fm_skill_security_abc/scripts/build.sh <<EOF",
            ) || cmd.starts_with("cat <<'EOF'") =>
        {
            Some("heredoc body is not a separate command; ts keeps one segment")
        }
        ("split", cmd) if cmd == format!("# {d}/SKILL.md") => {
            Some("comment-only input has no statements; legacy returns the comment as a segment")
        }
        ("split", cmd) if cmd == format!("cat {d}/SKILL.md # trailing comment") => {
            Some("trailing comment is not part of the command; ts drops it")
        }
        ("avoids", cmd) if cmd == format!("cat <<'EOF'\n{d}/SKILL.md\nEOF") => {
            Some("ts treats a guarded heredoc body as an IO channel directly")
        }
        ("decision", cmd) if cmd == format!("# {d}/SKILL.md") => {
            Some("comment-only command blocked by legacy, allowed by ts")
        }
        _ => None,
    }
}

#[test]
fn tree_sitter_parses_corpus_without_errors() {
    let expected = ["cat /a;& cat /b", "a ;| b"];
    let failures: Vec<String> = parse_failures()
        .into_iter()
        .filter(|cmd| !expected.contains(&cmd.as_str()))
        .collect();
    assert!(
        failures.is_empty(),
        "tree-sitter failed to parse:\n{}",
        failures.join("\n")
    );
}

#[test]
fn differential_command_references_dir() {
    let mut mismatches = Vec::new();
    for (cmd, guarded) in corpus() {
        let legacy = legacy_command_references_dir(&cmd, &guarded);
        let ts = command_references_dir(&cmd, &guarded);
        if legacy != ts {
            match expected_mismatch("references", &cmd) {
                Some(reason) => {
                    eprintln!("[expected divergence] {cmd:?}: {reason}");
                }
                None => mismatches.push(format!("{cmd:?}: legacy={legacy}, ts={ts}")),
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "command_references_dir mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn differential_split_command_segments() {
    let mut mismatches = Vec::new();
    for (cmd, _) in corpus() {
        let legacy = legacy_split_command_segments(&cmd);
        let ts = split_command_segments(&cmd);
        if legacy != ts {
            match expected_mismatch("split", &cmd) {
                Some(reason) => {
                    eprintln!("[expected divergence] {cmd:?}: {reason}");
                }
                None => mismatches.push(format!("{cmd:?}: legacy={legacy:?}, ts={ts:?}")),
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "split_command_segments mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn differential_script_execution_avoids_guarded_io() {
    let mut mismatches = Vec::new();
    for (cmd, guarded) in corpus() {
        let legacy = legacy_script_execution_avoids_guarded_io(&cmd, &guarded);
        let ts = script_execution_avoids_guarded_io(&cmd, &guarded);
        if legacy != ts {
            match expected_mismatch("avoids", &cmd) {
                Some(reason) => {
                    eprintln!("[expected divergence] {cmd:?}: {reason}");
                }
                None => mismatches.push(format!("{cmd:?}: legacy={legacy}, ts={ts}")),
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "script_execution_avoids_guarded_io mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn differential_guard_block_decision() {
    let mut mismatches = Vec::new();
    for (cmd, guarded) in corpus() {
        let legacy_blocked = legacy_split_command_segments(&cmd).iter().any(|segment| {
            legacy_command_references_dir(segment, &guarded)
                && (!is_script_execution(segment)
                    || !legacy_script_execution_avoids_guarded_io(segment, &guarded))
        });
        let ts_blocked = split_command_segments(&cmd).iter().any(|segment| {
            command_references_dir(segment, &guarded)
                && (!is_script_execution(segment)
                    || !script_execution_avoids_guarded_io(segment, &guarded))
        });
        if legacy_blocked != ts_blocked {
            match expected_mismatch("decision", &cmd) {
                Some(reason) => {
                    eprintln!("[expected divergence] {cmd:?}: {reason}");
                }
                None => mismatches.push(format!(
                    "{cmd:?}: legacy_blocked={legacy_blocked}, ts_blocked={ts_blocked}"
                )),
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "guard block decision mismatches:\n{}",
        mismatches.join("\n")
    );
}

// ---- Round 4 fixes ----

#[test]
fn compound_subshell_guarded_read_is_flagged() {
    // H1 PoC 1: a guarded `cd` plus relative read inside a subshell produced
    // no segment at all, leaving the read invisible to the token layer.
    let dirs = vec![guarded_dir()];
    let command = "(cd /dev/shm/fm-agent-security/p1/fm_skill_security_abc && cat scripts/run.py)";
    let segments = split_command_segments(command);
    assert!(
        !segments.is_empty(),
        "subshell body must produce segments: {segments:?}"
    );
    let flags = segment_guarded_flags(&segments, &dirs);
    assert!(
        flags.iter().any(|flag| *flag),
        "guarded cd+read inside a subshell must be flagged: {segments:?} → {flags:?}"
    );
}

#[test]
fn compound_if_statement_guarded_read_is_flagged() {
    // H1 PoC 2: an `if` compound hiding `cd /dev/shm`, chained into a
    // relative guarded read.
    let dirs = vec![guarded_dir()];
    let command = "if true; then cd /dev/shm; fi && cat fm_skill_security_abc/scripts/run.py";
    let segments = split_command_segments(command);
    assert!(
        !segments.is_empty(),
        "if compound must produce segments: {segments:?}"
    );
    let flags = segment_guarded_flags(&segments, &dirs);
    assert!(
        flags.iter().any(|flag| *flag),
        "guarded read hidden in an if compound must be flagged: {segments:?} → {flags:?}"
    );
}

#[test]
fn top_level_script_execution_segments_unchanged_by_compound_support() {
    // D9 regression guard: a non-compound top-level script execution keeps
    // its single-segment shape.
    let dirs = vec![guarded_dir()];
    let segments = split_command_segments("bash /skills/run.sh");
    assert_eq!(segments, vec!["bash /skills/run.sh".to_string()]);
    let flags = segment_guarded_flags(&segments, &dirs);
    assert!(
        flags.iter().all(|flag| !flag),
        "unrelated script execution must stay allowed: {flags:?}"
    );
}

#[test]
fn popd_resets_tracked_cwd_conservatively() {
    // M1: `popd` shifts the dirstack but left the tracked cwd stale, so a
    // later relative guarded read resolved against the wrong directory.
    let dirs = vec![guarded_dir()];
    let segments = split_command_segments(
        "cd /dev && pushd /tmp && popd && cat shm/fm-agent-security/x/SKILL.md",
    );
    let flags = segment_guarded_flags(&segments, &dirs);
    assert!(
        flags.last().is_some_and(|flag| *flag),
        "relative guarded read after popd must be flagged: {segments:?} → {flags:?}"
    );
}

#[test]
fn globstar_double_star_reaches_controlled_dirs() {
    // M2: `**` was matched as one component, so a globstar pattern anchored
    // at `/dev` could never cover a controlled dir's depth.
    let dirs = vec![guarded_dir()];
    for token in [
        "/dev/**/run.py",
        "/dev/shm/**/SKILL.md",
        "/dev/shm/fm-agent-security/**/scripts/run.sh",
    ] {
        assert!(
            path_references_guarded(token, None, &dirs),
            "globstar pattern must be flagged: {token}"
        );
    }
    // HIGH-1 ① keeps working: a too-shallow breadth glob stays unflagged.
    assert!(
        !path_references_guarded("/dev/*", None, &dirs),
        "breadth glob without enough depth must stay allowed"
    );
}

#[test]
fn extglob_paren_cd_target_falls_back_to_tail_heuristic() {
    // M2: extglob `(...)` forms are runtime patterns; a cd target holding
    // one must fall back to the conservative tail heuristic instead of
    // tracking an unresolvable lexical cwd.
    let dirs = vec![guarded_dir()];
    let segments = split_command_segments("cd /tmp/@(a|b) && cat fm_skill_security_abc/SKILL.md");
    let flags = segment_guarded_flags(&segments, &dirs);
    assert!(
        flags.iter().any(|flag| *flag),
        "relative guarded read after an extglob cd target must be flagged: {segments:?} → {flags:?}"
    );
}

#[test]
fn legacy_scanner_unquotes_backslash_escapes() {
    // L1: the legacy fallback judged raw words, so `/dev/s\hm` bypassed it.
    let dirs = vec![guarded_dir()];
    assert!(
        legacy_command_references_dir("/dev/s\\hm", &dirs),
        "backslash-escaped guarded path must be caught by the legacy scanner"
    );
    assert!(
        legacy_command_references_dir(
            "cat /dev/s\\hm/fm-agent-security/p1/fm_skill_security_abc/SKILL.md",
            &dirs,
        ),
        "backslash-escaped guarded read must be caught by the legacy scanner"
    );
    // Unrelated escaped names must stay allowed.
    assert!(
        !legacy_command_references_dir("cat /tm\\p/notes.txt", &dirs),
        "escaped unrelated path must stay allowed"
    );
}
