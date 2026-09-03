#[cfg(target_os = "linux")]
#[cfg(not(debug_assertions))]
use debugoff;

/// Note that the cwd, env, and command args are preserved in the ultimate call
/// to `execv`, so the caller is responsible for ensuring those values are
/// correct.
fn main() -> ! {
    #[cfg(target_os = "linux")]
    #[cfg(not(debug_assertions))]
    debugoff::multi_ptraceme_or_die();

    codex_linux_sandbox::run_main()
}
