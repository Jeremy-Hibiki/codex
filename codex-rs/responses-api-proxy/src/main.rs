use clap::Parser;
use codex_responses_api_proxy::Args as ResponsesApiProxyArgs;

#[ctor::ctor]
fn pre_main() {
    codex_process_hardening::pre_main_hardening();
}

pub fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    #[cfg(not(debug_assertions))]
    codex_process_hardening::disable_process_dumping()
        .unwrap_or_else(|err| eprintln!("WARNING: failed to disable process dumping: {err}"));

    let args = ResponsesApiProxyArgs::parse();
    codex_responses_api_proxy::run_main(args)
}
