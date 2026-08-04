//! Startup license verification for Codex.
//!
//! On Linux x86_64 with glibc this crate checks out a license from the FMSH
//! LicenseService before Codex starts and holds it for the lifetime of the
//! process. Everywhere else the crate compiles to an empty stub because the
//! underlying LMCLIENT SDK only ships a CentOS 7 / x86_64 static library.
//!
//! Verification is always enforced. The LMCLIENT SDK reads
//! `FMSH_LIC_SERVER` internally (`<port>@<host>`). `FMSH_CODEX_LIC_FEATURE`
//! and `FMSH_CODEX_LIC_VERSION` must be set explicitly. `FMSH_CODEX_LIC_DISPLAY_NAME`
//! is optional and defaults to `"Codex"`.

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
mod license;

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::LicenseGuard;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::check_in_now;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::verify_at_startup;
