use anyhow::Result;
use predicates::str::contains;
use std::path::Path;
use tempfile::TempDir;

const FULL_ACCESS_ERROR: &str =
    "full-access execution is disabled by product policy; use a sandboxed permission profile";

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    cmd.env("HOME", codex_home);
    Ok(cmd)
}

#[tokio::test]
async fn danger_full_access_sandbox_flag_is_rejected() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["--sandbox", "danger-full-access", "exec"])
        .assert()
        .failure()
        .stderr(contains(FULL_ACCESS_ERROR));

    Ok(())
}

#[tokio::test]
async fn exec_subcommand_danger_full_access_flag_is_rejected() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["exec", "--sandbox", "danger-full-access"])
        .assert()
        .failure()
        .stderr(contains(FULL_ACCESS_ERROR));

    Ok(())
}

#[tokio::test]
async fn dangerously_bypass_approvals_and_sandbox_flag_is_rejected() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["--dangerously-bypass-approvals-and-sandbox", "exec"])
        .assert()
        .failure()
        .stderr(contains(FULL_ACCESS_ERROR));

    Ok(())
}

#[tokio::test]
async fn exec_subcommand_bypass_flag_is_rejected() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["exec", "--dangerously-bypass-approvals-and-sandbox"])
        .assert()
        .failure()
        .stderr(contains(FULL_ACCESS_ERROR));

    Ok(())
}

#[tokio::test]
async fn resume_subcommand_danger_full_access_flag_is_rejected() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["resume", "--sandbox", "danger-full-access"])
        .assert()
        .failure()
        .stderr(contains(FULL_ACCESS_ERROR));

    Ok(())
}
