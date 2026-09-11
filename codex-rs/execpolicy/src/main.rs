use anyhow::Result;
use clap::Parser;
use codex_execpolicy::ExecPolicyCheckCommand;

/// CLI for evaluating exec policies
#[derive(Parser)]
#[command(name = "codex-execpolicy")]
enum Cli {
    /// Evaluate a command against a policy.
    Check(ExecPolicyCheckCommand),
}

fn main() -> Result<()> {
    #[cfg(target_os = "linux")]
    #[cfg(not(debug_assertions))]
    codex_process_hardening::disable_process_dumping()
        .unwrap_or_else(|err| eprintln!("WARNING: failed to disable process dumping: {err}"));

    let cli = Cli::parse();
    match cli {
        Cli::Check(cmd) => cmd.run(),
    }
}
