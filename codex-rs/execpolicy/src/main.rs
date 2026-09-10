use anyhow::Result;
use clap::Parser;
use codex_execpolicy::ExecPolicyCheckCommand;

#[cfg(target_os = "linux")]
#[cfg(not(debug_assertions))]
use debugoff;

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
    debugoff::multi_ptraceme_or_die();

    let cli = Cli::parse();
    match cli {
        Cli::Check(cmd) => cmd.run(),
    }
}
