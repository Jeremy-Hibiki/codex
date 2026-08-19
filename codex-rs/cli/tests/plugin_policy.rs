use anyhow::Result;
use codex_config::CONFIG_TOML_FILE;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::path::Path;
use tempfile::TempDir;

const PLUGIN_POLICY_ERROR: &str = "only managed plugins are allowed by product policy";
const MARKETPLACE_POLICY_ERROR: &str = "only managed marketplaces are allowed by product policy";

fn write_policy_config(
    codex_home: &Path,
    allow_managed_plugins_only: bool,
    allow_managed_marketplaces_only: bool,
) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        format!(
            "allow_managed_plugins_only = {allow_managed_plugins_only}\nallow_managed_marketplaces_only = {allow_managed_marketplaces_only}\n"
        ),
    )
}

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[tokio::test]
async fn plugin_management_is_open_by_default() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    )?;

    // With no policy fields set, neither read nor mutation surfaces are blocked
    // by product policy (they may fail for other reasons, e.g. missing
    // marketplace, but never with the policy error).
    for args in [
        vec!["plugin", "list"],
        vec!["plugin", "add", "sample@debug"],
        vec!["plugin", "remove", "sample@debug"],
        vec!["plugin", "marketplace", "list"],
    ] {
        codex_command(codex_home.path())?
            .args(&args)
            .assert()
            .stderr(
                contains(PLUGIN_POLICY_ERROR)
                    .not()
                    .and(contains(MARKETPLACE_POLICY_ERROR).not()),
            );
    }

    Ok(())
}

#[tokio::test]
async fn plugin_management_is_restricted_to_managed_plugins() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_policy_config(codex_home.path(), true, false)?;
    for args in [
        vec!["plugin", "list"],
        vec!["plugin", "add", "sample@debug"],
        vec!["plugin", "remove", "sample@debug"],
    ] {
        codex_command(codex_home.path())?
            .args(&args)
            .assert()
            .failure()
            .stderr(contains(PLUGIN_POLICY_ERROR));
    }
    Ok(())
}

#[tokio::test]
async fn marketplace_management_is_restricted_to_managed_marketplaces() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_policy_config(codex_home.path(), false, true)?;
    for args in [
        vec!["plugin", "marketplace", "list"],
        vec!["plugin", "marketplace", "remove", "debug"],
        vec!["plugin", "marketplace", "upgrade"],
    ] {
        codex_command(codex_home.path())?
            .args(&args)
            .assert()
            .failure()
            .stderr(contains(MARKETPLACE_POLICY_ERROR));
    }
    Ok(())
}

#[tokio::test]
async fn marketplace_add_is_restricted_to_managed_marketplaces() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_policy_config(codex_home.path(), false, true)?;
    let source = TempDir::new()?;
    let source_parent = source.path().parent().unwrap();
    let source_arg = format!("./{}", source.path().file_name().unwrap().to_string_lossy());

    codex_command(codex_home.path())?
        .current_dir(source_parent)
        .args(["plugin", "marketplace", "add", source_arg.as_str()])
        .assert()
        .failure()
        .stderr(contains(MARKETPLACE_POLICY_ERROR));

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
