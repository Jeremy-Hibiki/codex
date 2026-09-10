use super::*;
use pretty_assertions::assert_eq;

#[test]
fn top_cli_parses_resume_prompt_after_config_flag() {
    const PROMPT: &str = "echo resume-with-global-flags-after-subcommand";
    let cli = TopCli::parse_from([
        "codex-exec",
        "resume",
        "--strict-config",
        "--last",
        "--json",
        "--model",
        "gpt-5.2-codex",
        "--config",
        "reasoning_level=xhigh",
        "--dangerously-bypass-approvals-and-sandbox",
        "--skip-git-repo-check",
        PROMPT,
    ]);
    let mut inner = cli.inner;
    inner
        .config_overrides
        .prepend_root_overrides(cli.config_overrides);

    let Some(codex_exec::Command::Resume(args)) = inner.command.as_ref() else {
        panic!("expected resume command");
    };
    let effective_prompt = args.prompt.clone().or_else(|| {
        if args.last {
            args.session_id.clone()
        } else {
            None
        }
    });
    assert_eq!(effective_prompt.as_deref(), Some(PROMPT));
    assert_eq!(inner.config_overrides.raw_overrides.len(), 1);
    assert_eq!(
        inner.config_overrides.raw_overrides[0],
        "reasoning_level=xhigh"
    );
    assert!(inner.strict_config);
}

/// F6: the exec binary is a request boundary — when the license heartbeat
/// reports the license lost (simulated via the product-level test switch),
/// the process must refuse to start the run with a visible error.
#[test]
fn exec_refuses_to_start_when_license_is_force_lost() {
    let bin = codex_utils_cargo_bin::cargo_bin("codex-exec")
        .expect("codex-exec binary should be built for tests");
    let output = std::process::Command::new(bin)
        .arg("validate-license-gate")
        .env("FMSH_CODEX_LIC_TEST_BYPASS", "1")
        .env("FMSH_CODEX_LIC_TEST_FORCE_LOST", "1")
        .output()
        .expect("spawn codex-exec");

    assert!(
        !output.status.success(),
        "a force-lost license must block exec startup"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stderr.contains("license is unavailable") || stdout.contains("license is unavailable"),
        "expected the license-unavailable error, got stderr: {stderr}, stdout: {stdout}"
    );
}
