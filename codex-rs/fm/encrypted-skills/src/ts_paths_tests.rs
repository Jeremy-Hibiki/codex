//! Differential comparison: tree-sitter-bash prototype vs the hand-written
//! scanner in `paths.rs`, over the existing guard corpus plus bypass-shaped
//! commands.

use super::paths;
use super::ts_paths;

fn guarded_dir() -> String {
    format!("{}/p1/fm_skill_security_abc", paths::MEM_ROOT)
}

fn corpus() -> Vec<(String, Vec<String>)> {
    let d = guarded_dir();
    vec![
        // ---- 现有 paths_tests 语料 ----
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
        .filter(|(cmd, _)| ts_paths::parse(cmd).is_none_or(|tree| tree.root_node().has_error()))
        .map(|(cmd, _)| cmd)
        .collect()
}

/// Documented, intentional divergences between the legacy scanner and the
/// tree-sitter prototype (each is a semantic improvement in the prototype).
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
        let existing = paths::command_references_dir(&cmd, &guarded);
        let ts = ts_paths::command_references_dir(&cmd, &guarded);
        if existing != ts {
            match expected_mismatch("references", &cmd) {
                Some(reason) => {
                    eprintln!("[expected divergence] {cmd:?}: {reason}");
                }
                None => mismatches.push(format!("{cmd:?}: existing={existing}, ts={ts}")),
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
        let existing = paths::split_command_segments(&cmd);
        let ts = ts_paths::split_command_segments(&cmd);
        if existing != ts {
            match expected_mismatch("split", &cmd) {
                Some(reason) => {
                    eprintln!("[expected divergence] {cmd:?}: {reason}");
                }
                None => mismatches.push(format!("{cmd:?}: existing={existing:?}, ts={ts:?}")),
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
        let existing = paths::script_execution_avoids_guarded_io(&cmd, &guarded);
        let ts = ts_paths::script_execution_avoids_guarded_io(&cmd, &guarded);
        if existing != ts {
            match expected_mismatch("avoids", &cmd) {
                Some(reason) => {
                    eprintln!("[expected divergence] {cmd:?}: {reason}");
                }
                None => mismatches.push(format!("{cmd:?}: existing={existing}, ts={ts}")),
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
        for segment in paths::split_command_segments(&cmd) {
            let existing = paths::command_references_dir(&segment, &guarded)
                && (!paths::is_script_execution(&segment)
                    || !paths::script_execution_avoids_guarded_io(&segment, &guarded));
            let ts = ts_paths::command_references_dir(&segment, &guarded)
                && (!paths::is_script_execution(&segment)
                    || !ts_paths::script_execution_avoids_guarded_io(&segment, &guarded));
            if existing != ts {
                match expected_mismatch("decision", &segment) {
                    Some(reason) => {
                        eprintln!("[expected divergence] {segment:?}: {reason}");
                    }
                    None => mismatches.push(format!(
                        "segment={segment:?} (from {cmd:?}): existing_blocked={existing}, ts_blocked={ts}"
                    )),
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "guard block decision mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn legacy_corpus_assertions_still_hold_for_ts() {
    let d = guarded_dir();
    let guarded = vec![d.clone()];
    // 与 paths_tests::command_references_decrypted_dir 对齐
    assert!(ts_paths::command_references_dir(
        "ls /dev/shm/fm-agent-security",
        &guarded
    ));
    assert!(ts_paths::command_references_dir(
        &format!("bash {d}/scripts/run.sh"),
        &guarded,
    ));
    assert!(ts_paths::command_references_dir(
        "cat /dev/shm/fm-agent-security/p*/f*/SKILL.md",
        &guarded,
    ));
    assert!(!ts_paths::command_references_dir(
        "bash /tmp/run.sh",
        &guarded
    ));

    // 与 paths_tests::script_execution_avoids_guarded_io 对齐
    assert!(ts_paths::script_execution_avoids_guarded_io(
        &format!("bash {d}/scripts/build.sh"),
        &guarded,
    ));
    assert!(!ts_paths::script_execution_avoids_guarded_io(
        &format!("bash {d}/scripts/build.sh < {d}/SKILL.md"),
        &guarded,
    ));
    assert!(!ts_paths::script_execution_avoids_guarded_io(
        &format!("bash {d}/scripts/build.sh \"$(cat {d}/SKILL.md)\""),
        &guarded,
    ));
    assert!(!ts_paths::script_execution_avoids_guarded_io(
        &format!("bash {d}/scripts/build.sh `cat {d}/SKILL.md`"),
        &guarded,
    ));
}
