use super::*;
use codex_analytics::build_track_events_context;
use codex_skills::SkillEncryption;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use fm_encrypted_skills::registry::TtlConfig;
use fm_encrypted_skills::runtime::EncryptedSkillRuntime;
use fm_encrypted_skills::sdk::EnvelopeError;
use fm_encrypted_skills::sdk::EnvelopeSdk;
use fm_encrypted_skills::sdk::PackageEntry;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

fn make_skill(name: &str, path: &str) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: format!("{name} skill"),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: test_path_buf(path).abs(),
        scope: codex_protocol::protocol::SkillScope::User,
        plugin_id: None,
        remote_plugin_id: None,
        encrypted: false,
        encryption: None,
    }
}

fn set<'a>(items: &'a [&'a str]) -> HashSet<&'a str> {
    items.iter().copied().collect()
}

fn assert_mentions(text: &str, expected_names: &[&str], expected_paths: &[&str]) {
    let mentions = extract_tool_mentions(text);
    assert_eq!(mentions.names, set(expected_names));
    assert_eq!(mentions.paths, set(expected_paths));
}

fn linked_skill_mention(name: &str, unix_path: &str) -> String {
    format!("[${name}]({})", test_path_buf(unix_path).display())
}

fn collect_mentions(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<AbsolutePathBuf>,
    connector_slug_counts: &HashMap<String, usize>,
) -> Vec<SkillMetadata> {
    collect_explicit_skill_mentions(inputs, skills, disabled_paths, connector_slug_counts)
}

struct MockEncryptedSdk {
    calls: Arc<AtomicUsize>,
    fail: bool,
}

impl EnvelopeSdk for MockEncryptedSdk {
    fn decrypt_package(&self, _package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(EnvelopeError::Decrypt("mock failure".into()))
        } else {
            Ok(vec![PackageEntry {
                rel_path: PathBuf::from("SKILL.md"),
                contents: b"# Real encrypted content".to_vec(),
            }])
        }
    }
}

fn encrypted_skill(name: &str, path: &str) -> SkillMetadata {
    SkillMetadata {
        encrypted: true,
        encryption: Some(SkillEncryption {
            version: Some(2),
            key_id: Some("required_hardware_key".into()),
            algorithm: Some("ZIP-AES-256-CBC".into()),
            mode: Some("mock".into()),
            package: Some(format!("{name}.zip.enc")),
        }),
        ..make_skill(name, path)
    }
}

fn track_events() -> codex_analytics::TrackEventsContext {
    build_track_events_context(
        "test".into(),
        "thread-1".into(),
        "turn-1".into(),
        "p".into(),
    )
}

#[tokio::test]
async fn encrypted_skill_injects_token_instead_of_plaintext() {
    let calls = Arc::new(AtomicUsize::new(0));
    let sdk = Arc::new(MockEncryptedSdk {
        calls: calls.clone(),
        fail: false,
    });
    let tmp = tempfile::tempdir().unwrap();
    let runtime =
        EncryptedSkillRuntime::new(sdk, TtlConfig::default(), tmp.path().join("mem-root"));
    let skill = encrypted_skill("secret", "/skills/secret/SKILL.md");

    let outcome = build_skill_injections(
        &[skill],
        Some(&SkillLoadOutcome::default()),
        Some(&runtime),
        "thread-1",
        None,
        &codex_analytics::AnalyticsEventsClient::disabled(),
        track_events(),
    )
    .await;

    assert!(
        outcome.warnings.is_empty(),
        "unexpected warnings: {:?}",
        outcome.warnings
    );
    assert_eq!(outcome.items.len(), 1);
    let item = &outcome.items[0];
    assert!(item.encrypted);
    let token = item.token.as_deref().expect("encrypted token");
    assert!(token.starts_with("[SENSITIVE_SKILL_TOKEN:thread-1:"));
    assert!(item.contents.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn encrypted_skill_is_idempotent_within_ttl() {
    let calls = Arc::new(AtomicUsize::new(0));
    let sdk = Arc::new(MockEncryptedSdk {
        calls: calls.clone(),
        fail: false,
    });
    let tmp = tempfile::tempdir().unwrap();
    let runtime =
        EncryptedSkillRuntime::new(sdk, TtlConfig::default(), tmp.path().join("mem-root"));
    let skill = encrypted_skill("secret", "/skills/secret/SKILL.md");

    let first = build_skill_injections(
        std::slice::from_ref(&skill),
        Some(&SkillLoadOutcome::default()),
        Some(&runtime),
        "thread-1",
        None,
        &codex_analytics::AnalyticsEventsClient::disabled(),
        track_events(),
    )
    .await;
    let second = build_skill_injections(
        &[skill],
        Some(&SkillLoadOutcome::default()),
        Some(&runtime),
        "thread-1",
        None,
        &codex_analytics::AnalyticsEventsClient::disabled(),
        track_events(),
    )
    .await;

    assert_eq!(first.items[0].token, second.items[0].token);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn encrypted_skill_without_runtime_produces_warning() {
    let skill = encrypted_skill("secret", "/skills/secret/SKILL.md");

    let outcome = build_skill_injections(
        &[skill],
        Some(&SkillLoadOutcome::default()),
        None,
        "thread-1",
        None,
        &codex_analytics::AnalyticsEventsClient::disabled(),
        track_events(),
    )
    .await;

    assert!(outcome.items.is_empty());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("encrypted"));
}

#[tokio::test]
async fn encrypted_skill_decrypt_failure_produces_warning() {
    let sdk = Arc::new(MockEncryptedSdk {
        calls: Arc::new(AtomicUsize::new(0)),
        fail: true,
    });
    let tmp = tempfile::tempdir().unwrap();
    let runtime =
        EncryptedSkillRuntime::new(sdk, TtlConfig::default(), tmp.path().join("mem-root"));
    let skill = encrypted_skill("secret", "/skills/secret/SKILL.md");

    let outcome = build_skill_injections(
        &[skill],
        Some(&SkillLoadOutcome::default()),
        Some(&runtime),
        "thread-1",
        None,
        &codex_analytics::AnalyticsEventsClient::disabled(),
        track_events(),
    )
    .await;

    assert!(outcome.items.is_empty());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("mock failure"));
}

#[test]
fn text_mentions_skill_requires_exact_boundary() {
    assert_eq!(
        true,
        text_mentions_skill("use $notion-research-doc please", "notion-research-doc")
    );
    assert_eq!(
        true,
        text_mentions_skill("($notion-research-doc)", "notion-research-doc")
    );
    assert_eq!(
        true,
        text_mentions_skill("$notion-research-doc.", "notion-research-doc")
    );
    assert_eq!(
        false,
        text_mentions_skill("$notion-research-docs", "notion-research-doc")
    );
    assert_eq!(
        false,
        text_mentions_skill("$notion-research-doc_extra", "notion-research-doc")
    );
}

#[test]
fn text_mentions_skill_handles_end_boundary_and_near_misses() {
    assert_eq!(true, text_mentions_skill("$alpha-skill", "alpha-skill"));
    assert_eq!(false, text_mentions_skill("$alpha-skillx", "alpha-skill"));
    assert_eq!(
        true,
        text_mentions_skill("$alpha-skillx and later $alpha-skill ", "alpha-skill")
    );
}

#[test]
fn text_mentions_skill_handles_many_dollars_without_looping() {
    let prefix = "$".repeat(256);
    let text = format!("{prefix} not-a-mention");
    assert_eq!(false, text_mentions_skill(&text, "alpha-skill"));
}

#[test]
fn extract_tool_mentions_handles_plain_and_linked_mentions() {
    assert_mentions(
        "use $alpha and [$beta](/tmp/beta)",
        &["alpha", "beta"],
        &["/tmp/beta"],
    );
}

#[test]
fn extract_tool_mentions_skips_common_env_vars() {
    assert_mentions("use $PATH and $alpha", &["alpha"], &[]);
    assert_mentions("use [$HOME](/tmp/skill)", &[], &[]);
    assert_mentions("use $XDG_CONFIG_HOME and $beta", &["beta"], &[]);
}

#[test]
fn extract_tool_mentions_requires_link_syntax() {
    assert_mentions("[beta](/tmp/beta)", &[], &[]);
    assert_mentions("[$beta] /tmp/beta", &["beta"], &[]);
    assert_mentions("[$beta]()", &["beta"], &[]);
}

#[test]
fn extract_tool_mentions_trims_linked_paths_and_allows_spacing() {
    assert_mentions("use [$beta]   ( /tmp/beta )", &["beta"], &["/tmp/beta"]);
}

#[test]
fn extract_tool_mentions_stops_at_non_name_chars() {
    assert_mentions(
        "use $alpha.skill and $beta_extra",
        &["alpha", "beta_extra"],
        &[],
    );
}

#[test]
fn extract_tool_mentions_keeps_plugin_skill_namespaces() {
    assert_mentions(
        "use $slack:search and $alpha",
        &["alpha", "slack:search"],
        &[],
    );
}

#[test]
fn collect_explicit_skill_mentions_text_respects_skill_order() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let beta = make_skill("beta-skill", "/tmp/beta");
    let skills = vec![beta.clone(), alpha.clone()];
    let inputs = vec![UserInput::Text {
        text: "first $alpha-skill then $beta-skill".to_string(),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    // Text scanning should not change the previous selection ordering semantics.
    assert_eq!(selected, vec![beta, alpha]);
}

#[test]
fn collect_explicit_skill_mentions_prioritizes_structured_inputs() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let beta = make_skill("beta-skill", "/tmp/beta");
    let skills = vec![alpha.clone(), beta.clone()];
    let inputs = vec![
        UserInput::Text {
            text: "please run $alpha-skill".to_string(),
            text_elements: Vec::new(),
        },
        UserInput::Skill {
            name: "beta-skill".to_string(),
            path: test_path_buf("/tmp/beta"),
        },
    ];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![beta, alpha]);
}

#[test]
fn collect_explicit_skill_mentions_skips_invalid_structured_and_blocks_plain_fallback() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![
        UserInput::Text {
            text: "please run $alpha-skill".to_string(),
            text_elements: Vec::new(),
        },
        UserInput::Skill {
            name: "alpha-skill".to_string(),
            path: test_path_buf("/tmp/missing"),
        },
    ];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_skips_disabled_structured_and_blocks_plain_fallback() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![
        UserInput::Text {
            text: "please run $alpha-skill".to_string(),
            text_elements: Vec::new(),
        },
        UserInput::Skill {
            name: "alpha-skill".to_string(),
            path: test_path_buf("/tmp/alpha"),
        },
    ];
    let disabled = HashSet::from([test_path_buf("/tmp/alpha").abs()]);
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &disabled, &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_dedupes_by_path() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha.clone()];
    let mention = linked_skill_mention("alpha-skill", "/tmp/alpha");
    let inputs = vec![UserInput::Text {
        text: format!("use {mention} and {mention}"),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![alpha]);
}

#[test]
fn collect_explicit_skill_mentions_skips_ambiguous_name() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta];
    let inputs = vec![UserInput::Text {
        text: "use $demo-skill and again $demo-skill".to_string(),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_prefers_linked_path_over_name() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta.clone()];
    let inputs = vec![UserInput::Text {
        text: format!(
            "use $demo-skill and {}",
            linked_skill_mention("demo-skill", "/tmp/beta")
        ),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![beta]);
}

#[test]
fn collect_explicit_skill_mentions_skips_plain_name_when_connector_matches() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![UserInput::Text {
        text: "use $alpha-skill".to_string(),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::from([("alpha-skill".to_string(), 1)]);

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_allows_explicit_path_with_connector_conflict() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha.clone()];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("alpha-skill", "/tmp/alpha")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::from([("alpha-skill".to_string(), 1)]);

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![alpha]);
}

#[test]
fn collect_explicit_skill_mentions_skips_when_linked_path_disabled() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/alpha")),
        text_elements: Vec::new(),
    }];
    let disabled = HashSet::from([test_path_buf("/tmp/alpha").abs()]);
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &disabled, &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_prefers_resource_path() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta.clone()];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/beta")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![beta]);
}

#[test]
fn collect_explicit_skill_mentions_skips_missing_path_with_no_fallback() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/missing")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_skips_missing_path_without_fallback() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/missing")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}
