#![recursion_limit = "256"]

use codex_arg0::Arg0DispatchPaths;
use codex_arg0::arg0_dispatch_or_else;
use codex_mcp_server::run_main;
use codex_utils_cli::CliConfigOverrides;

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|arg0_paths: Arg0DispatchPaths| async move {
        // The standalone MCP server is a product entry point that can start
        // Codex work; it must hold a license for the process lifetime.
        let _license_guard = fm_license::verify_at_startup()?;
        fm_license::install_checkin_signal_handler()?;
        run_main(
            arg0_paths,
            CliConfigOverrides::default(),
            /*strict_config*/ false,
        )
        .await?;
        Ok(())
    })
}
