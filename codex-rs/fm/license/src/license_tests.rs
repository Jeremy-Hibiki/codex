use pretty_assertions::assert_eq;

use super::LicenseConfig;
use super::ensure_active;
use super::is_active;
use super::mark_license_active;
use super::mark_license_lost;
use super::resolve_config;

#[test]
fn all_values_are_resolved_from_env() {
    let config = resolve_config(Some("PRO"), Some("2.0"), Some("MyApp")).unwrap();
    assert_eq!(
        config,
        LicenseConfig {
            feature: "PRO".to_owned(),
            version: "2.0".to_owned(),
            display_name: "MyApp".to_owned(),
        }
    );
}

#[test]
fn display_name_defaults_to_codex() {
    let config = resolve_config(Some("PRO"), Some("2.0"), None).unwrap();
    assert_eq!(config.display_name, "Codex");
}

#[test]
fn display_name_empty_string_falls_back_to_default() {
    let config = resolve_config(Some("PRO"), Some("2.0"), Some("")).unwrap();
    assert_eq!(config.display_name, "Codex");
}

#[test]
fn missing_values_are_errors() {
    assert!(resolve_config(/*feature*/ None, Some("2.0"), None).is_err());
    assert!(resolve_config(Some("PRO"), /*version*/ None, None).is_err());
    assert!(resolve_config(Some(""), Some("2.0"), None).is_err());
    assert!(resolve_config(Some("PRO"), Some(""), None).is_err());
}

#[test]
fn missing_or_empty_server_is_an_error() {
    // This tests the pattern: required fields produce errors, optional don't
    assert!(resolve_config(None, Some("2.0"), Some("MyApp")).is_err());
    assert!(resolve_config(Some(""), Some("2.0"), Some("MyApp")).is_err());
}

#[test]
fn license_state_is_active_by_default() {
    assert!(is_active());
    assert!(ensure_active().is_ok());
}

#[test]
fn lost_license_blocks_new_requests_until_recovery() {
    mark_license_lost();
    assert!(!is_active());
    assert!(ensure_active().is_err());

    mark_license_active();
    assert!(is_active());
    assert!(ensure_active().is_ok());
}
