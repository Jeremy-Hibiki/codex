use anyhow::Result;
use predicates::str::contains;
use std::path::Path;
use tempfile::TempDir;

const SANDBOX_BYPASS_ERROR: &str =
    "sandbox bypass is disabled by product policy; use a sandboxed permission profile";

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    cmd.env("HOME", codex_home);
    std::fs::write(
        codex_home.join("config.toml"),
        "allow_sandbox_bypass = false\n",
    )?;
    Ok(cmd)
}

#[tokio::test]
async fn danger_full_access_sandbox_flag_is_rejected() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["--sandbox", "danger-full-access", "exec"])
        .assert()
        .failure()
        .stderr(contains(SANDBOX_BYPASS_ERROR));

    Ok(())
}

#[tokio::test]
async fn full_access_flags_are_rejected_across_subcommands() -> Result<()> {
    for args in [
        vec!["exec", "--sandbox", "danger-full-access"],
        vec!["--dangerously-bypass-approvals-and-sandbox", "exec"],
        vec!["exec", "--dangerously-bypass-approvals-and-sandbox"],
        vec!["resume", "--sandbox", "danger-full-access"],
    ] {
        let codex_home = TempDir::new()?;
        codex_command(codex_home.path())?
            .args(&args)
            .assert()
            .failure()
            .stderr(contains(SANDBOX_BYPASS_ERROR));
    }
    Ok(())
}
