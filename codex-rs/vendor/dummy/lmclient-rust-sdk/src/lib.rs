//! Placeholder for the internal `lmclient-rust-sdk` crate (internal GitLab
//! 192.168.131.126:8089). See `fmsh-ukey-*` placeholders for the general
//! rationale.
//!
//! This stub mirrors the real crate's public API surface so that
//! `fm-license`'s `lmclient` feature compiles and links offline. It is
//! deliberately **non-enforcing**: `LicenseClient::init` and
//! `check_out_incr` succeed without contacting any license server, so a
//! A build against this stub starts with the gate active as long as
//! `FMSH_CODEX_LIC_FEATURE` / `FMSH_CODEX_LIC_VERSION` are configured.
//! Without them, `fm-license`'s own `resolve_config` still fails at
//! startup, and `FMSH_CODEX_LIC_TEST_BYPASS=1` remains the explicit escape
//! hatch. Internal full-fat builds: run `release/use-real-fm-deps.sh`
//! first, then build with `--features lmclient`.

use std::ffi::c_char;
use std::ffi::c_int;

#[allow(non_snake_case)]
pub mod ffi {
    //! Raw FFI entry points of the LMCLIENT C library. The stub versions are
    //! inert: the heartbeat machinery that would call them never runs without
    //! a real client.

    use super::c_char;
    use super::c_int;
    use super::RetryCallback;

    /// Successful LMCLIENT call result.
    pub const LM_SUCCESS: c_int = 0;
    /// Non-blocking checkout flag (`lm_check_out` `LM_NOWAIT` semantics).
    pub const LM_NOWAIT: c_int = 1;

    /// Stub checkout: reports success without holding any license seat.
    ///
    /// # Safety
    /// Arguments are accepted to match the real signature; the stub ignores
    /// them and touches no C state.
    pub unsafe fn lmCheckOutIncr(
        _feature: *const c_char,
        _version: *const c_char,
        _num_lic: c_int,
        _nowait: c_int,
        _lic_type: c_int,
        _on_fail: Option<RetryCallback>,
    ) -> c_int {
        LM_SUCCESS
    }

    /// Stub check-in; no-op.
    ///
    /// # Safety
    /// Matches the real signature; the stub ignores the argument.
    pub unsafe fn lmCheckIn(_feature: *const c_char) -> c_int {
        LM_SUCCESS
    }

    /// Stub client teardown; no-op.
    pub unsafe fn lmExit() {}
}

pub use ffi::LM_NOWAIT;
pub use ffi::LM_SUCCESS;

/// Heartbeat callback signature (`extern "C"` so the C library can call it).
pub type RetryCallback = unsafe extern "C" fn() -> c_int;

/// Errors reported by the stub license client.
#[derive(Debug, thiserror::Error)]
pub enum LmClientError {
    /// The offline stub never talks to a license server; this variant exists
    /// so the error type mirrors the real crate's shape.
    #[error("lmclient stub operation failed: {0}")]
    Stub(String),
}

/// Client initialization settings; field-for-field compatible with the real
/// `InitConfig`.
#[derive(Debug, Default)]
pub struct InitConfig {
    pub auto_recheck: bool,
    pub recheck_interval: c_int,
    pub retry_count: c_int,
    pub sleep_time: c_int,
    pub display_name: Option<String>,
    pub host_name: Option<String>,
    pub retry_routine: Option<RetryCallback>,
    pub retry_success: Option<RetryCallback>,
    pub exit_routine: Option<RetryCallback>,
}

/// Handle to the (stub) LMCLIENT license client.
#[derive(Debug)]
pub struct LicenseClient {
    config: InitConfig,
}

impl LicenseClient {
    /// Initialize the stub client. Always succeeds; logs a loud warning so
    /// the non-enforcing build is visible in logs.
    pub fn init(config: InitConfig) -> anyhow::Result<Self, LmClientError> {
        tracing::warn!(
            auto_recheck = config.auto_recheck,
            "lmclient-rust-sdk placeholder is in use: license checkout is NOT enforced; \
             run release/use-real-fm-deps.sh for the real FMSH license gate"
        );
        Ok(Self { config })
    }

    /// Stub checkout: succeeds without consuming a license seat.
    pub fn check_out_incr(
        &self,
        feature: &str,
        version: &str,
        num_lic: c_int,
        nowait: c_int,
        lic_type: c_int,
        on_fail: Option<RetryCallback>,
    ) -> anyhow::Result<(), LmClientError> {
        let _ = (num_lic, nowait, lic_type, on_fail);
        tracing::warn!(
            feature,
            version,
            "lmclient stub accepted a license checkout without a license server"
        );
        Ok(())
    }

    /// Stub check-in; no-op.
    pub fn check_in(&self, feature: &str) -> anyhow::Result<(), LmClientError> {
        tracing::debug!(feature, "lmclient stub checked in a license (no-op)");
        Ok(())
    }

    /// Stub teardown; no-op.
    pub fn exit(&self) -> anyhow::Result<(), LmClientError> {
        tracing::debug!(
            auto_recheck = self.config.auto_recheck,
            "lmclient stub client exited (no-op)"
        );
        Ok(())
    }
}
