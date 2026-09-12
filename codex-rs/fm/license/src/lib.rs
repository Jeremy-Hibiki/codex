//! Startup license verification for Codex.
//!
//! On Linux x86_64 with glibc — and only when the `lmclient` cargo feature is
//! enabled (`codex-cli`'s `ukey` feature turns it on for full-fat builds) —
//! this crate checks out a license from the FMSH LicenseService before Codex
//! starts and holds it for the lifetime of the process. Everywhere else (and
//! in default builds without the feature) the crate compiles to an empty stub
//! because the underlying LMCLIENT SDK only ships a CentOS 7 / x86_64 static
//! library. The stub passes every request through (baseline Codex behavior)
//! and no internal GitLab dependency is needed to build.
//!
//! If the license heartbeat exhausts its retries at runtime, Codex tries one
//! fresh checkout on the existing license client. This handles a license
//! server whose in-memory authorization state was reset. If that checkout
//! also fails, the process stays alive in a "license lost" state: surfaces
//! must call [`ensure_active`] at request boundaries so users cannot start new
//! work until the license recovers. The process is never killed by the
//! heartbeat callback.
//!
//! Verification is enforced in every build mode unless the product-level skip
//! switch `FMSH_CODEX_LIC_TEST_BYPASS` is set (release builds honor it too).
//! A Codex process spawned from inside another Codex session (detected on
//! Linux via the inherited `CODEX_THREAD_ID` shell-tool marker) skips the
//! checkout so nested sessions do not consume extra license seats.
//! The LMCLIENT SDK reads `FMSH_LIC_SERVER` internally (`<port>@<host>`).
//! `FMSH_CODEX_LIC_FEATURE` and `FMSH_CODEX_LIC_VERSION` must be set
//! explicitly. `FMSH_CODEX_LIC_DISPLAY_NAME` is optional and defaults to
//! `"Codex"`; `FMSH_CODEX_LIC_HOSTNAME` is optional and defaults to the
//! LMCLIENT SDK default client host name.

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
))]
mod license;

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
))]
pub use license::LICENSE_UNAVAILABLE_MESSAGE;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
))]
pub use license::LicenseGuard;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
))]
pub use license::TEST_BYPASS_ENV_VAR;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
))]
pub use license::TEST_FORCE_LOST_ENV_VAR;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
))]
pub use license::check_in_now;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
))]
pub use license::ensure_active;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
))]
pub use license::init_entry;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
))]
pub use license::install_checkin_signal_handler;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
))]
pub use license::is_active;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
))]
pub use license::verify_at_startup;

// Stubs for builds without the real gate: non-glibc platforms and glibc
// builds with the `lmclient` feature off (the default). The license gate is
// a no-op so request boundaries compile everywhere and never block: this is
// baseline Codex behavior, loud about being disabled.
#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
)))]
pub const LICENSE_UNAVAILABLE_MESSAGE: &str =
    "Codex license is unavailable; new requests are blocked until the license recovers";
#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
)))]
pub const TEST_BYPASS_ENV_VAR: &str = "FMSH_CODEX_LIC_TEST_BYPASS";
#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
)))]
pub const TEST_FORCE_LOST_ENV_VAR: &str = "FMSH_CODEX_LIC_TEST_FORCE_LOST";

/// Whether the real LMCLIENT-backed license gate is compiled in.
///
/// Compile-time constant: false on non-glibc platforms and whenever the
/// `lmclient` feature is off. The stub gate then passes every request
/// through and ignores the test env vars, so tests that exercise the
/// lost-license request gates should skip themselves when this returns
/// false.
pub fn lmclient_available() -> bool {
    cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "gnu",
        feature = "lmclient"
    ))
}

#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
)))]
pub fn is_active() -> bool {
    true
}

#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
)))]
pub fn ensure_active() -> Result<(), anyhow::Error> {
    Ok(())
}

/// Stub guard: there is no checkout to hold, so it is empty.
#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
)))]
#[derive(Debug, Default)]
pub struct LicenseGuard;

#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
)))]
pub fn check_in_now() {}

#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
)))]
pub fn install_checkin_signal_handler() -> Result<(), anyhow::Error> {
    Ok(())
}

#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
)))]
pub fn verify_at_startup() -> anyhow::Result<LicenseGuard> {
    warn_stub_once();
    Ok(LicenseGuard)
}

#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu",
    feature = "lmclient"
)))]
pub fn init_entry() -> anyhow::Result<LicenseGuard> {
    verify_at_startup()
}

/// Set after the no-op-stub warning was emitted once. Declared outside the
/// stub cfg so unit tests on glibc hosts can exercise the latch logic too.
#[cfg(any(
    test,
    not(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "gnu",
        feature = "lmclient"
    ))
))]
static STUB_WARNING_EMITTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Warns once per process that the license gate is a no-op in this build.
///
/// Non-glibc builds cannot link the FMSH SDK and glibc builds without the
/// `lmclient` feature do not compile it in, so [`is_active`] always returns
/// true and [`ensure_active`] always succeeds; the missing gate must be loud
/// instead of silent. Both `tracing::warn` (for log collectors) and stderr
/// (guaranteed visible even without a tracing subscriber installed) fire.
#[cfg(any(
    test,
    not(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "gnu",
        feature = "lmclient"
    ))
))]
fn warn_stub_once() {
    use std::sync::atomic::Ordering;

    if STUB_WARNING_EMITTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let message = "FMSH license gate disabled: this build was compiled without the FMSH LicenseService SDK (lmclient feature off or unsupported platform); is_active() always returns true";
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
