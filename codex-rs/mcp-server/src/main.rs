#![recursion_limit = "256"]

use codex_arg0::Arg0DispatchPaths;
use codex_arg0::arg0_dispatch_or_else;
use codex_mcp_server::run_main;
use codex_utils_cli::CliConfigOverrides;

#[cfg(target_os = "linux")]
#[cfg(not(debug_assertions))]
use debugoff;

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    #[cfg(not(debug_assertions))]
    debugoff::multi_ptraceme_or_die();

    arg0_dispatch_or_else(|arg0_paths: Arg0DispatchPaths| async move {
        // The standalone MCP server is a product entry point that can start
        // Codex work; it must hold a license for the process lifetime.
        let _license_guard = fm_license::init_entry()?;
        run_main(
            arg0_paths,
            CliConfigOverrides::default(),
            /*strict_config*/ false,
        )
        .await?;
        Ok(())
    })
}
