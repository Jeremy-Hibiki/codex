//! Startup license verification for Codex.
//!
//! On Linux x86_64 with glibc this crate checks out a license from the FMSH
//! LicenseService before Codex starts and holds it for the lifetime of the
//! process. Everywhere else the crate compiles to an empty stub because the
//! underlying LMCLIENT SDK only ships a CentOS 7 / x86_64 static library.
//!
//! If the license heartbeat exhausts its retries at runtime, the process stays
//! alive but switches to a "license lost" state: surfaces must call
//! [`ensure_active`] at request boundaries so users cannot start new work
//! until the license recovers. The process is never killed by the heartbeat
//! callback.
//!
//! Verification is enforced in every build mode unless the product-level skip
//! switch `FMSH_CODEX_LIC_TEST_BYPASS` is set (release builds honor it too).
//! The LMCLIENT SDK reads `FMSH_LIC_SERVER` internally (`<port>@<host>`).
//! `FMSH_CODEX_LIC_FEATURE` and `FMSH_CODEX_LIC_VERSION` must be set
//! explicitly. `FMSH_CODEX_LIC_DISPLAY_NAME` is optional and defaults to
//! `"Codex"`.

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
    "Codex license is unavailable; new requests are blocked until the license recovers";
#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
pub const TEST_BYPASS_ENV_VAR: &str = "FMSH_CODEX_LIC_TEST_BYPASS";
#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
pub const TEST_FORCE_LOST_ENV_VAR: &str = "FMSH_CODEX_LIC_TEST_FORCE_LOST";

#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
pub fn is_active() -> bool {
    true
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
pub fn ensure_active() -> Result<(), anyhow::Error> {
    Ok(())
}
