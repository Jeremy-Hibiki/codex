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
fn display_name_defaults_to_codex_when_missing_or_empty() {
    for display_name in [None, Some("")] {
        let config = resolve_config(Some("PRO"), Some("2.0"), display_name).unwrap();
        assert_eq!(config.display_name, "Codex");
    }
}

#[test]
fn missing_or_empty_required_fields_are_errors() {
    for (feature, version, display_name) in [
        (None, Some("2.0"), None),
        (Some("PRO"), None, None),
        (Some(""), Some("2.0"), None),
        (Some("PRO"), Some(""), None),
        (None, Some("2.0"), Some("MyApp")),
        (Some(""), Some("2.0"), Some("MyApp")),
    ] {
        assert!(
            resolve_config(feature, version, display_name).is_err(),
            "required-field error expected for ({feature:?}, {version:?}, {display_name:?})"
        );
    }
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
