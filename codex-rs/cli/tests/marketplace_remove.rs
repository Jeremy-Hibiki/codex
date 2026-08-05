use anyhow::Result;
use predicates::str::contains;
use std::path::Path;
use tempfile::TempDir;

const POLICY_ERROR: &str = "plugin and marketplace management is disabled by product policy";

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[tokio::test]
async fn marketplace_remove_is_rejected_by_product_policy() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "remove", "debug"])
        .assert()
        .failure()
        .stderr(contains(POLICY_ERROR));

    Ok(())
}
