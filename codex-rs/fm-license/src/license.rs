//! FMSH LicenseManager integration used to verify the Codex license at startup.

use anyhow::Context;
use anyhow::Result;
use lmclient_rust_sdk::InitConfig;
use lmclient_rust_sdk::LM_NOWAIT;
use lmclient_rust_sdk::LicenseClient;
use lmclient_rust_sdk::RetryCallback;
use std::ffi::c_int;

/// Environment variable pointing at the LicenseService (`<port>@<host>`).
pub const SERVER_ENV_VAR: &str = "FMSH_LIC_SERVER";
/// Environment variable overriding the licensed feature name.
pub const FEATURE_ENV_VAR: &str = "FMSH_LIC_FEATURE";
/// Environment variable overriding the licensed feature version.
pub const VERSION_ENV_VAR: &str = "FMSH_LIC_VERSION";
/// When set to `1` or `true`, startup verification is skipped even in release builds.
pub const DISABLE_ENV_VAR: &str = "FMSH_LIC_DISABLE";
/// When set to `1` or `true`, startup verification runs even in debug builds.
pub const FORCE_ENV_VAR: &str = "FMSH_LIC_FORCE";

const RECHECK_INTERVAL_SECONDS: c_int = 30;
const RETRY_COUNT: c_int = 10;
const SLEEP_TIME_SECONDS: c_int = 1;

/// License settings resolved from the environment.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LicenseConfig {
    pub server: String,
    pub feature: String,
    pub version: String,
}

/// Resolve license settings from the environment; every value is required.
pub(crate) fn resolve_config(
    server: Option<&str>,
    feature: Option<&str>,
    version: Option<&str>,
) -> Result<LicenseConfig> {
    let server = server.filter(|value| !value.is_empty()).context(format!(
        "{SERVER_ENV_VAR} is not set; point it at the FMSH LicenseService as `<port>@<host>`"
    ))?;
    let feature = feature.filter(|value| !value.is_empty()).context(format!(
        "{FEATURE_ENV_VAR} is not set; set it to the licensed feature name"
    ))?;
    let version = version.filter(|value| !value.is_empty()).context(format!(
        "{VERSION_ENV_VAR} is not set; set it to the licensed feature version"
    ))?;
    Ok(LicenseConfig {
        server: server.to_owned(),
        feature: feature.to_owned(),
        version: version.to_owned(),
    })
}

fn license_check_enabled(disable: Option<&str>, force: Option<&str>, debug_build: bool) -> bool {
    if disable.is_some_and(|value| matches!(value, "1" | "true")) {
        return false;
    }
    if force.is_some_and(|value| matches!(value, "1" | "true")) {
        return true;
    }
    !debug_build
}

unsafe extern "C" fn on_retry() -> c_int {
    eprintln!("license heartbeat retrying...");
    0
}

unsafe extern "C" fn on_retry_success() -> c_int {
    eprintln!("license heartbeat recovered.");
    0
}

unsafe extern "C" fn on_license_lost() -> c_int {
    eprintln!("license lost: heartbeat retries exhausted.");
    0
}

/// Holds a checked-out license for the lifetime of a Codex session.
///
/// The license is returned on drop; `lmExit` in the C library also releases
/// outstanding licenses as a fallback.
pub struct LicenseGuard {
    client: Option<LicenseClient>,
    feature: String,
}

impl Drop for LicenseGuard {
    fn drop(&mut self) {
        if let Some(client) = self.client.as_mut() {
            let _ = client.check_in(&self.feature);
            let _ = client.exit();
        }
    }
}

/// Check out the configured license feature at startup.
///
/// Returns `Ok(None)` when verification is disabled via [`DISABLE_ENV_VAR`],
/// or when running a debug build without [`FORCE_ENV_VAR`].
/// On failure the license error is returned with context so the caller can
/// refuse to start.
pub fn verify_at_startup() -> Result<Option<LicenseGuard>> {
    if !license_check_enabled(
        std::env::var(DISABLE_ENV_VAR).ok().as_deref(),
        std::env::var(FORCE_ENV_VAR).ok().as_deref(),
        cfg!(debug_assertions),
    ) {
        return Ok(None);
    }

    let config = resolve_config(
        std::env::var(SERVER_ENV_VAR).ok().as_deref(),
        std::env::var(FEATURE_ENV_VAR).ok().as_deref(),
        std::env::var(VERSION_ENV_VAR).ok().as_deref(),
    )?;

    let client = LicenseClient::init(InitConfig {
        auto_recheck: true,
        recheck_interval: RECHECK_INTERVAL_SECONDS,
        retry_count: RETRY_COUNT,
        sleep_time: SLEEP_TIME_SECONDS,
        display_name: Some("Codex".to_owned()),
        retry_routine: Some(on_retry as RetryCallback),
        retry_success: Some(on_retry_success as RetryCallback),
        exit_routine: Some(on_license_lost as RetryCallback),
        ..InitConfig::default()
    })
    .with_context(|| {
        format!(
            "failed to initialize the FMSH license client at {}",
            config.server
        )
    })?;

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

    Ok(Some(LicenseGuard {
        client: Some(client),
        feature: config.feature,
    }))
}

#[cfg(test)]
#[path = "license_tests.rs"]
mod tests;
