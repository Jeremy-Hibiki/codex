//! Startup license verification for Grevo.
//!
//! On Linux x86_64 with glibc this crate checks out a license from the FMSH
//! LicenseService before Grevo starts and holds it for the lifetime of the
//! process. Everywhere else the crate compiles to an empty stub because the
//! underlying LMCLIENT SDK only ships a CentOS 7 / x86_64 static library.
//!
//! If the license heartbeat exhausts its retries at runtime, Grevo tries one
//! fresh checkout on the existing license client. This handles a license
//! server whose in-memory authorization state was reset. If that checkout
//! also fails, the process stays alive in a "license lost" state: surfaces
//! must call [`ensure_active`] at request boundaries so users cannot start new
//! work until the license recovers. The process is never killed by the
//! heartbeat callback.
//!
//! Verification is enforced in every build mode unless the product-level skip
//! switch `FMSH_CODEX_LIC_TEST_BYPASS` is set (release builds honor it too).
//! A Grevo process spawned from inside another Grevo session (detected on
//! Linux via the inherited `CODEX_THREAD_ID` shell-tool marker) skips the
//! checkout so nested sessions do not consume extra license seats.
//! The LMCLIENT SDK reads `FMSH_LIC_SERVER` internally (`<port>@<host>`).
//! `FMSH_CODEX_LIC_FEATURE` and `FMSH_CODEX_LIC_VERSION` must be set
//! explicitly. `FMSH_CODEX_LIC_DISPLAY_NAME` is optional and defaults to
//! `"Grevo"`; `FMSH_CODEX_LIC_HOSTNAME` is optional and defaults to the
//! LMCLIENT SDK default client host name.

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
mod license;

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::LICENSE_UNAVAILABLE_MESSAGE;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::LicenseGuard;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::TEST_BYPASS_ENV_VAR;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::TEST_FORCE_LOST_ENV_VAR;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::check_in_now;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::ensure_active;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::init_entry;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::install_checkin_signal_handler;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::is_active;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::verify_at_startup;

// Stubs for platforms where the FMSH SDK is unavailable: the license gate is a
// no-op so request boundaries compile everywhere and never block on platforms
// that cannot check out licenses.
#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
pub const LICENSE_UNAVAILABLE_MESSAGE: &str =
    "Grevo license is unavailable; new requests are blocked until the license recovers";
#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
pub const TEST_BYPASS_ENV_VAR: &str = "FMSH_CODEX_LIC_TEST_BYPASS";
#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
pub const TEST_FORCE_LOST_ENV_VAR: &str = "FMSH_CODEX_LIC_TEST_FORCE_LOST";

#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
pub fn is_active() -> bool {
    warn_stub_once();
    true
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
pub fn ensure_active() -> Result<(), anyhow::Error> {
    warn_stub_once();
    Ok(())
}

/// Set after the no-op-stub warning was emitted once. Declared outside the
/// stub cfg so unit tests on glibc hosts can exercise the latch logic too.
#[cfg(any(
    test,
    not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))
))]
static STUB_WARNING_EMITTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Warns once per process that the license gate is a no-op on this platform.
///
/// Non-glibc builds cannot link the FMSH SDK, so [`is_active`] always returns
/// true and [`ensure_active`] always succeeds; the missing gate must be loud
/// instead of silent. Both `tracing::warn` (for log collectors) and stderr
/// (guaranteed visible even without a tracing subscriber installed) fire.
#[cfg(any(
    test,
    not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))
))]
fn warn_stub_once() {
    use std::sync::atomic::Ordering;

    if STUB_WARNING_EMITTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let message = "FMSH license gate disabled: this build targets a platform without the FMSH LicenseService SDK; is_active() always returns true";
    tracing::warn!("{message}");
    eprintln!("warning: {message}");
}

#[cfg(test)]
mod stub_tests {
    use std::sync::atomic::Ordering;

    use super::STUB_WARNING_EMITTED;
    use super::warn_stub_once;

    #[test]
    fn stub_warning_latches_after_first_call() {
        warn_stub_once();
        warn_stub_once();
        assert!(STUB_WARNING_EMITTED.load(Ordering::Relaxed));
    }
}
