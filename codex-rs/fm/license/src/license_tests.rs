use pretty_assertions::assert_eq;
use std::process::Command;
use std::sync::Mutex;

use super::LicenseConfig;
use super::LicenseEnv;
use super::ensure_active;
use super::is_active;
use super::mark_license_active;
use super::mark_license_lost;
use super::resolve_config;
use super::verify_at_startup;

const THREAD_ID_ENV_VAR: &str = "CODEX_THREAD_ID";
const NESTED_CHILD_TEST_ENV_VAR: &str = "FM_LICENSE_NESTED_CHILD_TEST";
const THREAD_ID_CHILD_OK: &str = "THREAD_ID_CHILD_OK";

/// Serializes tests that mutate the process-global license state; Bazel runs
/// all unit tests in one process with parallel threads.
static LICENSE_STATE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn all_values_are_resolved_from_env() {
    let config = resolve_config(LicenseEnv {
        feature: Some("PRO"),
        version: Some("2.0"),
        display_name: Some("MyApp"),
        host_name: Some("build-agent-7"),
    })
    .unwrap();
    assert_eq!(
        config,
        LicenseConfig {
            feature: "PRO".to_owned(),
            version: "2.0".to_owned(),
            display_name: "MyApp".to_owned(),
            host_name: Some("build-agent-7".to_owned()),
        }
    );
}

#[test]
fn display_name_defaults_to_codex_when_missing_or_empty() {
    for display_name in [None, Some("")] {
        let config = resolve_config(LicenseEnv {
            feature: Some("PRO"),
            version: Some("2.0"),
            display_name,
            ..LicenseEnv::default()
        })
        .unwrap();
        assert_eq!(config.display_name, "Codex");
    }
}

#[test]
fn host_name_is_optional_and_empty_falls_back_to_sdk_default() {
    for host_name in [None, Some("")] {
        let config = resolve_config(LicenseEnv {
            feature: Some("PRO"),
            version: Some("2.0"),
            host_name,
            ..LicenseEnv::default()
        })
        .unwrap();
        assert_eq!(config.host_name, None);
    }
}

#[test]
fn missing_or_empty_required_fields_are_errors() {
    for (feature, version) in [
        (None, Some("2.0")),
        (Some("PRO"), None),
        (Some(""), Some("2.0")),
        (Some("PRO"), Some("")),
    ] {
        assert!(
            resolve_config(LicenseEnv {
                feature,
                version,
                ..LicenseEnv::default()
            })
            .is_err(),
            "required-field error expected for ({feature:?}, {version:?})"
        );
    }
}

#[test]
fn license_state_is_active_by_default() {
    let _guard = LICENSE_STATE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(is_active());
    assert!(ensure_active().is_ok());
}

#[test]
fn lost_license_blocks_new_requests_until_recovery() {
    let _guard = LICENSE_STATE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    mark_license_lost();
    assert!(!is_active());
    assert!(ensure_active().is_err());

    mark_license_active();
    assert!(is_active());
    assert!(ensure_active().is_ok());
}

#[test]
fn child_with_thread_id_skips_startup_checkout() {
    let exe = std::env::current_exe().expect("current test executable");
    let output = Command::new(exe)
        .arg("--exact")
        .arg("license::tests::thread_id_child_asserts_skip")
        .arg("--nocapture")
        .env(THREAD_ID_ENV_VAR, "thread-1")
        .env(NESTED_CHILD_TEST_ENV_VAR, "1")
        .env_remove("FMSH_CODEX_LIC_TEST_BYPASS")
        .env_remove("FMSH_CODEX_LIC_TEST_FORCE_LOST")
        .output()
        .expect("spawn test binary as child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "thread-id child failed: {stdout}\n{stderr}"
    );
    assert!(
        stdout.contains(THREAD_ID_CHILD_OK),
        "thread-id child did not run its assertions: {stdout}"
    );
}

#[test]
fn thread_id_child_asserts_skip() {
    if std::env::var_os(NESTED_CHILD_TEST_ENV_VAR).is_none() {
        return;
    }
    let guard = verify_at_startup().expect("thread-id child must skip checkout");
    assert!(
        guard.client.is_none(),
        "thread-id child must not hold a license checkout"
    );
    println!("{THREAD_ID_CHILD_OK}");
}
