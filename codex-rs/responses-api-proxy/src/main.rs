use clap::Parser;
use codex_responses_api_proxy::Args as ResponsesApiProxyArgs;

#[cfg(target_os = "linux")]
#[cfg(not(debug_assertions))]
use debugoff;

#[ctor::ctor]
fn pre_main() {
    codex_process_hardening::pre_main_hardening();
}

pub fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    #[cfg(not(debug_assertions))]
    debugoff::multi_ptraceme_or_die();

    let args = ResponsesApiProxyArgs::parse();
    codex_responses_api_proxy::run_main(args)
}
