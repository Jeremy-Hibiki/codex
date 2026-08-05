use anyhow::Result;
use codex_config::CONFIG_TOML_FILE;
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
async fn plugin_management_is_rejected_even_with_plugins_enabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    )?;

    codex_command(codex_home.path())?
        .args(["plugin", "list"])
        .assert()
        .failure()
        .stderr(contains(POLICY_ERROR));

    Ok(())
}

#[tokio::test]
async fn plugin_add_is_rejected_by_product_policy() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .failure()
        .stderr(contains(POLICY_ERROR));

    Ok(())
}

#[tokio::test]
async fn plugin_remove_is_rejected_by_product_policy() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["plugin", "remove", "sample@debug"])
        .assert()
        .failure()
        .stderr(contains(POLICY_ERROR));

    Ok(())
}

#[tokio::test]
async fn marketplace_list_is_rejected_by_product_policy() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "list"])
        .assert()
        .failure()
        .stderr(contains(POLICY_ERROR));

    Ok(())
}

#[tokio::test]
async fn marketplace_add_is_rejected_by_product_policy() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    let source_parent = source.path().parent().unwrap();
    let source_arg = format!("./{}", source.path().file_name().unwrap().to_string_lossy());

    codex_command(codex_home.path())?
        .current_dir(source_parent)
        .args(["plugin", "marketplace", "add", source_arg.as_str()])
        .assert()
        .failure()
        .stderr(contains(POLICY_ERROR));

    Ok(())
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

#[tokio::test]
async fn marketplace_upgrade_is_rejected_by_product_policy() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "upgrade"])
        .assert()
        .failure()
        .stderr(contains(POLICY_ERROR));

    Ok(())
}

#[tokio::test]
async fn marketplace_commands_do_not_run_at_top_level() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["marketplace", "upgrade"])
        .assert()
        .failure()
        .stderr(contains("unrecognized subcommand 'upgrade'"));

    Ok(())
}
