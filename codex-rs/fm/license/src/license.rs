//! FMSH LicenseManager integration used to verify the Codex license at startup.

use anyhow::Context;
use anyhow::Result;
use codex_core::exec_env::CODEX_THREAD_ID_ENV_VAR;
use lmclient_rust_sdk::InitConfig;
use lmclient_rust_sdk::LM_NOWAIT;
use lmclient_rust_sdk::LicenseClient;
use lmclient_rust_sdk::RetryCallback;
use std::ffi::CString;
use std::ffi::c_int;
use std::sync::Mutex;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;

/// Environment variable overriding the licensed feature name.
pub const FEATURE_ENV_VAR: &str = "FMSH_CODEX_LIC_FEATURE";
/// Environment variable overriding the licensed feature version.
pub const VERSION_ENV_VAR: &str = "FMSH_CODEX_LIC_VERSION";
/// Environment variable for the display name shown to the license server.
pub const DISPLAY_NAME_ENV_VAR: &str = "FMSH_CODEX_LIC_DISPLAY_NAME";

const RECHECK_INTERVAL_SECONDS: c_int = 30;
const RETRY_COUNT: c_int = 10;
const SLEEP_TIME_SECONDS: c_int = 1;

/// License state while the process holds a checked-out license.
const LICENSE_STATE_ACTIVE: u8 = 0;
const LICENSE_STATE_LOST: u8 = 1;

/// Message returned by [`ensure_active`] when the license is unavailable.
pub const LICENSE_UNAVAILABLE_MESSAGE: &str =
    "Codex license is unavailable; new requests are blocked until the license recovers";

/// Environment variable that bypasses FMSH license verification.
///
/// This is a product-level skip switch honored in every build mode, including
/// release builds. It lets environments without access to the FMSH
/// LicenseService run Codex without a checkout. Integration tests that spawn
/// gated binaries also set it so the repo test suite does not require a live
/// FMSH license server.
pub const TEST_BYPASS_ENV_VAR: &str = "FMSH_CODEX_LIC_TEST_BYPASS";

/// Environment variable that starts the process with the license already lost
/// so request gates can be exercised without a real license server. Only
/// honored together with [`TEST_BYPASS_ENV_VAR`] in every build mode.
pub const TEST_FORCE_LOST_ENV_VAR: &str = "FMSH_CODEX_LIC_TEST_FORCE_LOST";

/// Process-wide license state, updated by the LMCLIENT heartbeat thread.
///
/// Starts active so library-only callers that never checked out a license are
/// unaffected; request gates only matter after a checkout was lost.
static LICENSE_STATE: AtomicU8 = AtomicU8::new(LICENSE_STATE_ACTIVE);

/// Feature currently checked out; `None` once the license has been returned.
///
/// The signal handler and the [`LicenseGuard`] share this so exactly one
/// `check_in` happens no matter which path shuts the license down first.
static ACTIVE_FEATURE: Mutex<Option<String>> = Mutex::new(None);

/// Returns true while the checked-out license is active.
///
/// On platforms where the FMSH SDK is not available this always returns true
/// because the crate compiles to a no-op stub.
pub fn is_active() -> bool {
    LICENSE_STATE.load(Ordering::Acquire) == LICENSE_STATE_ACTIVE
}

/// Returns an error when the license was lost at runtime.
///
/// Call this at request boundaries (new thread, new turn, steering, review,
/// realtime session) so users can keep the process open but cannot start new
/// work while the license is unavailable.
pub fn ensure_active() -> Result<(), anyhow::Error> {
    if is_active() {
        Ok(())
    } else {
        anyhow::bail!(LICENSE_UNAVAILABLE_MESSAGE);
    }
}

/// Marks the license lost from the LMCLIENT heartbeat thread.
fn mark_license_lost() {
    LICENSE_STATE.store(LICENSE_STATE_LOST, Ordering::Release);
}

/// Marks the license recovered after a successful heartbeat retry.
fn mark_license_active() {
    LICENSE_STATE.store(LICENSE_STATE_ACTIVE, Ordering::Release);
}

fn test_bypass_enabled() -> bool {
    std::env::var(TEST_BYPASS_ENV_VAR)
        .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

fn test_force_lost_enabled() -> bool {
    std::env::var(TEST_FORCE_LOST_ENV_VAR)
        .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

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
    mark_license_active();
    tracing::warn!("license heartbeat recovered.");
    0
}

unsafe extern "C" fn on_license_lost() -> c_int {
    mark_license_lost();
    tracing::error!(
        "license lost: heartbeat retries exhausted; new requests are blocked until the license recovers"
    );
    0
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

/// Install a handler that returns the checked-out license on SIGINT/SIGTERM.
///
/// Some shutdown paths bypass normal destructor ordering: a SIGINT during
/// startup, `std::process::exit` from deep call stacks, or an ACP adapter
/// terminating the app-server process. The license must therefore be returned
/// from the signal handler itself before the process terminates. The process
/// exits with `128 + signal` after checking in, matching shell conventions.
pub fn install_checkin_signal_handler() -> Result<()> {
    use signal_hook::consts::signal::SIGINT;
    use signal_hook::consts::signal::SIGTERM;
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    std::thread::spawn(move || {
        if let Some(signal) = signals.forever().next() {
            check_in_now();
            std::process::exit(128 + signal);
        }
    });
    Ok(())
}

/// One-call product entry point: verify the license and install the
/// SIGINT/SIGTERM check-in handler. Hold the returned guard for the process
/// lifetime; it returns the license on drop.
pub fn init_entry() -> Result<LicenseGuard> {
    let guard = verify_at_startup()?;
    install_checkin_signal_handler()?;
    Ok(guard)
}

/// Check out the configured license feature at startup.
///
/// All build modes enforce verification unless [`TEST_BYPASS_ENV_VAR`] is set.
/// The bypass is a product-level skip switch, not a debug-only test hook, so
/// release builds honor the same environment variable.
///
/// Processes spawned from inside a Codex session inherit `CODEX_THREAD_ID`
/// and skip the checkout, so a Codex session that starts another Codex does
/// not consume a second license seat.
///
/// The LMCLIENT SDK reads `FMSH_LIC_SERVER` internally (`<port>@<host>`).
/// This function reads `FMSH_CODEX_LIC_FEATURE` and `FMSH_CODEX_LIC_VERSION`
/// (both required) and optionally `FMSH_CODEX_LIC_DISPLAY_NAME` (defaults to `"Codex"`).
pub fn verify_at_startup() -> Result<LicenseGuard> {
    if std::env::var_os(CODEX_THREAD_ID_ENV_VAR).is_some() {
        tracing::info!("nested codex process detected; skipping FMSH license checkout");
        return Ok(LicenseGuard { client: None });
    }

    if test_bypass_enabled() {
        if test_force_lost_enabled() {
            mark_license_lost();
            tracing::warn!("{TEST_FORCE_LOST_ENV_VAR} is set; starting with the license lost");
        } else {
            tracing::warn!("{TEST_BYPASS_ENV_VAR} is set; skipping FMSH license verification");
        }
        return Ok(LicenseGuard { client: None });
    }

    // A fresh checkout always starts active, even if a previous guard in this
    // process observed a lost heartbeat.
    LICENSE_STATE.store(LICENSE_STATE_ACTIVE, Ordering::Release);

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
    Ok(LicenseGuard {
        client: Some(client),
    })
}

#[cfg(test)]
#[path = "license_tests.rs"]
mod tests;
