/// Note that the cwd, env, and command args are preserved in the ultimate call
/// to `execv`, so the caller is responsible for ensuring those values are
/// correct.
fn main() -> ! {
    #[cfg(target_os = "linux")]
    #[cfg(not(debug_assertions))]
    codex_process_hardening::disable_process_dumping()
        .unwrap_or_else(|err| eprintln!("WARNING: failed to disable process dumping: {err}"));

    codex_linux_sandbox::run_main()
}
