use anyhow::Result;
use tokio::runtime::Builder;

#[cfg(target_os = "linux")]
#[cfg(not(debug_assertions))]
use debugoff;

fn main() -> Result<()> {
    #[cfg(target_os = "linux")]
    #[cfg(not(debug_assertions))]
    debugoff::multi_ptraceme_or_die();

    let runtime = Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(codex_app_server_test_client::run())
}
