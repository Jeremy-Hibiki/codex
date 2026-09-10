#[cfg(target_os = "linux")]
#[cfg(not(debug_assertions))]
use debugoff;

pub fn main() -> ! {
    #[cfg(target_os = "linux")]
    #[cfg(not(debug_assertions))]
    debugoff::multi_ptraceme_or_die();

    codex_apply_patch::main()
}
