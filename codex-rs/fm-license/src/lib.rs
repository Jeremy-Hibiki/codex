//! Startup license verification for Codex.
//!
//! On Linux x86_64 with glibc this crate checks out a license from the FMSH
//! LicenseService before Codex starts and holds it for the lifetime of the
//! process. Everywhere else the crate compiles to an empty stub because the
//! underlying LMCLIENT SDK only ships a CentOS 7 / x86_64 static library.
//!
//! Verification is always enforced in release builds; `FMSH_LIC_DISABLE` and
//! `FMSH_LIC_FORCE` only affect debug builds so the development loop and test
//! suite do not require a license server. `FMSH_LIC_SERVER`,
//! `FMSH_LIC_FEATURE`, and `FMSH_LIC_VERSION` must all be set explicitly;
//! there are no defaults.

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
mod license;

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::LicenseGuard;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::check_in_now;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
pub use license::verify_at_startup;
