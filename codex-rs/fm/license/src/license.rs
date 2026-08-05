//! FMSH LicenseManager integration used to verify the Codex license at startup.

use anyhow::Context;
use anyhow::Result;
use lmclient_rust_sdk::InitConfig;
use lmclient_rust_sdk::LM_NOWAIT;
use lmclient_rust_sdk::LicenseClient;
use lmclient_rust_sdk::RetryCallback;
use std::ffi::CString;
use std::ffi::c_int;
use std::sync::Mutex;

/// Environment variable overriding the licensed feature name.
pub const FEATURE_ENV_VAR: &str = "FMSH_CODEX_LIC_FEATURE";
/// Environment variable overriding the licensed feature version.
pub const VERSION_ENV_VAR: &str = "FMSH_CODEX_LIC_VERSION";
/// Environment variable for the display name shown to the license server.
pub const DISPLAY_NAME_ENV_VAR: &str = "FMSH_CODEX_LIC_DISPLAY_NAME";

const RECHECK_INTERVAL_SECONDS: c_int = 30;
const RETRY_COUNT: c_int = 10;
const SLEEP_TIME_SECONDS: c_int = 1;

/// Exit code when the license is lost at runtime (heartbeat retries exhausted).
const EXIT_LICENSE_LOST: i32 = 2;

/// Feature currently checked out; `None` once the license has been returned.
///
/// The signal handler and the [`LicenseGuard`] share this so exactly one
/// `check_in` happens no matter which path shuts the license down first.
static ACTIVE_FEATURE: Mutex<Option<String>> = Mutex::new(None);

/// License settings resolved from the environment.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LicenseConfig {
    pub feature: String,
    pub version: String,
    pub display_name: String,
}

/// Resolve license settings from the environment; every value is required
/// except `display_name` which defaults to `"Codex"`.
pub(crate) fn resolve_config(
    feature: Option<&str>,
    version: Option<&str>,
    display_name: Option<&str>,
) -> Result<LicenseConfig> {
    let feature = feature.filter(|value| !value.is_empty()).context(format!(
        "{FEATURE_ENV_VAR} is not set; set it to the licensed feature name"
    ))?;
    let version = version.filter(|value| !value.is_empty()).context(format!(
        "{VERSION_ENV_VAR} is not set; set it to the licensed feature version"
    ))?;
    let display_name = display_name
        .filter(|value| !value.is_empty())
        .unwrap_or("Codex");
    Ok(LicenseConfig {
        feature: feature.to_owned(),
        version: version.to_owned(),
        display_name: display_name.to_owned(),
    })
}

unsafe extern "C" fn on_retry() -> c_int {
    tracing::warn!("license heartbeat retrying...");
    0
}

unsafe extern "C" fn on_retry_success() -> c_int {
    tracing::warn!("license heartbeat recovered.");
    0
}

unsafe extern "C" fn on_license_lost() -> c_int {
    tracing::error!("license lost: heartbeat retries exhausted; exiting");
    std::process::exit(EXIT_LICENSE_LOST)
}

/// Holds a checked-out license for the lifetime of a Codex session.
///
/// The license is returned on drop; `lmExit` in the C library also releases
/// outstanding licenses as a fallback.
pub struct LicenseGuard {
    client: Option<LicenseClient>,
}

impl Drop for LicenseGuard {
    fn drop(&mut self) {
        let feature = ACTIVE_FEATURE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(feature) = feature
            && let Some(client) = self.client.as_mut()
        {
            let _ = client.check_in(&feature);
        }
        if let Some(client) = self.client.as_mut() {
            let _ = client.exit();
        }
    }
}

/// Return the checked-out license immediately, if one is still active.
///
/// This is the entry point used by the Ctrl-C/SIGINT/SIGTERM handler so the
/// license is returned and the client shut down before the process terminates,
/// without waiting for `Drop`.
pub fn check_in_now() {
    let feature = ACTIVE_FEATURE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    let Some(feature) = feature else {
        return;
    };
    let Ok(feature) = CString::new(feature) else {
        return;
    };
    // SAFETY: `feature` is a valid NUL-terminated C string for this call, and
    // the C library keeps global license state so no client handle is needed.
    // `lmExit` is called after `lmCheckIn` to stop the background heartbeat
    // thread, matching the library's recommended shutdown sequence.
    unsafe {
        lmclient_rust_sdk::ffi::lmCheckIn(feature.as_ptr());
        lmclient_rust_sdk::ffi::lmExit();
    }
}

/// Check out the configured license feature at startup.
///
/// All build modes enforce verification unconditionally. There is no escape
/// hatch — tests must use a mock license server or `cfg`-gate the call site.
///
/// The LMCLIENT SDK reads `FMSH_LIC_SERVER` internally (`<port>@<host>`).
/// This function reads `FMSH_CODEX_LIC_FEATURE` and `FMSH_CODEX_LIC_VERSION`
/// (both required) and optionally `FMSH_CODEX_LIC_DISPLAY_NAME` (defaults to `"Codex"`).
pub fn verify_at_startup() -> Result<Option<LicenseGuard>> {
    let config = resolve_config(
        std::env::var(FEATURE_ENV_VAR).ok().as_deref(),
        std::env::var(VERSION_ENV_VAR).ok().as_deref(),
        std::env::var(DISPLAY_NAME_ENV_VAR).ok().as_deref(),
    )?;

    let client = LicenseClient::init(InitConfig {
        auto_recheck: true,
        recheck_interval: RECHECK_INTERVAL_SECONDS,
        retry_count: RETRY_COUNT,
        sleep_time: SLEEP_TIME_SECONDS,
        display_name: Some(config.display_name.clone()),
        retry_routine: Some(on_retry as RetryCallback),
        retry_success: Some(on_retry_success as RetryCallback),
        exit_routine: Some(on_license_lost as RetryCallback),
        ..InitConfig::default()
    })
    .with_context(|| "failed to initialize the FMSH license client".to_string())?;

    client
        .check_out_incr(
            &config.feature,
            &config.version,
            /*num_lic*/ 0,
            LM_NOWAIT,
            /*lic_type*/ 0,
            /*on_fail*/ None,
        )
        .with_context(|| {
            format!(
                "license checkout failed for feature {} version {}",
                config.feature, config.version
            )
        })?;

    *ACTIVE_FEATURE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(config.feature);
    Ok(Some(LicenseGuard {
        client: Some(client),
    }))
}

#[cfg(test)]
#[path = "license_tests.rs"]
mod tests;
