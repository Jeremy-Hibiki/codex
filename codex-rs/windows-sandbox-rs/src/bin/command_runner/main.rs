#![cfg_attr(all(target_os = "windows", not(test)), windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
mod win;

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    win::main()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    #[cfg(target_os = "linux")]
    #[cfg(not(debug_assertions))]
    debugoff::multi_ptraceme_or_die();

    panic!("codex-command-runner is Windows-only");
}
