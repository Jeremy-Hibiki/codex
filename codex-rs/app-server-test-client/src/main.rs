use anyhow::Result;
use tokio::runtime::Builder;

fn main() -> Result<()> {
    #[cfg(target_os = "linux")]
    #[cfg(not(debug_assertions))]
    codex_process_hardening::disable_process_dumping()
        .unwrap_or_else(|err| eprintln!("WARNING: failed to disable process dumping: {err}"));

    let runtime = Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(codex_app_server_test_client::run())
}
