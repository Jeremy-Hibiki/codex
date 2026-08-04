use pretty_assertions::assert_eq;

use super::LicenseConfig;
use super::license_check_enabled;
use super::resolve_config;

#[test]
fn all_values_are_resolved_from_env() {
    let config = resolve_config(Some("5010@127.0.0.1"), Some("PRO"), Some("2.0")).unwrap();
    assert_eq!(
        config,
        LicenseConfig {
            server: "5010@127.0.0.1".to_owned(),
            feature: "PRO".to_owned(),
            version: "2.0".to_owned(),
        }
    );
}

#[test]
fn missing_values_are_errors() {
    assert!(resolve_config(Some("5010@127.0.0.1"), /*feature*/ None, Some("2.0")).is_err());
    assert!(resolve_config(Some("5010@127.0.0.1"), Some("PRO"), /*version*/ None).is_err());
    assert!(resolve_config(Some("5010@127.0.0.1"), Some(""), Some("2.0")).is_err());
    assert!(resolve_config(Some("5010@127.0.0.1"), Some("PRO"), Some("")).is_err());
}

#[test]
fn missing_or_empty_server_is_an_error() {
    assert!(resolve_config(/*server*/ None, Some("PRO"), Some("2.0")).is_err());
    assert!(resolve_config(Some(""), Some("PRO"), Some("2.0")).is_err());
}

#[test]
fn debug_builds_skip_verification_by_default() {
    assert!(!license_check_enabled(
        /*disable*/ None, /*force*/ None, /*debug_build*/ true
    ));
    assert!(license_check_enabled(
        /*disable*/ None, /*force*/ None, /*debug_build*/ false
    ));
}

#[test]
fn force_enables_verification_in_debug_builds() {
    assert!(license_check_enabled(
        /*disable*/ None,
        Some("1"),
        /*debug_build*/ true
    ));
    assert!(license_check_enabled(
        /*disable*/ None,
        Some("true"),
        /*debug_build*/ true
    ));
}

#[test]
fn empty_values_do_not_toggle_verification() {
    assert!(license_check_enabled(
        Some(""),
        Some(""),
        /*debug_build*/ false
    ));
    assert!(!license_check_enabled(
        Some(""),
        Some(""),
        /*debug_build*/ true
    ));
}

#[test]
fn disable_only_applies_in_debug_builds() {
    assert!(!license_check_enabled(
        Some("1"),
        Some("1"),
        /*debug_build*/ true
    ));
    assert!(!license_check_enabled(
        Some("true"),
        /*force*/ None,
        /*debug_build*/ true
    ));
}

#[test]
fn release_builds_always_verify() {
    assert!(license_check_enabled(
        Some("1"),
        /*force*/ None,
        /*debug_build*/ false
    ));
    assert!(license_check_enabled(
        Some("true"),
        Some("1"),
        /*debug_build*/ false
    ));
}
