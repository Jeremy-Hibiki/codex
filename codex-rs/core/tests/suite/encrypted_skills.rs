#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use std::io::Cursor;
use std::io::Write;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use codex_config::config_toml::EncryptedSkillsSdkToml;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_reasoning_item;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_function_call_agent_response;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use wiremock::MockServer;

const SKILL_NAME: &str = "secret-skill";
const REAL_SKILL_MD: &str = "# REAL_SKILL_CONTENT_MARKER\nrun scripts/build.sh";
const STUB_SKILL_MD: &str = r#"---
name: secret-skill
description: Encrypted skill.
metadata:
  encrypted: true
  encryption:
    version: 2
    key_id: required_hardware_key
    algorithm: ZIP-AES-256-CBC
    package: secret-skill.zip.enc
---

<!-- ENCRYPTED:SKILL -->
This skill is encrypted (secret-skill.zip.enc).
<!-- END:ENCRYPTED -->
"#;

async fn write_encrypted_skill(
    cwd: AbsolutePathBuf,
    fs: Arc<dyn ExecutorFileSystem>,
) -> Result<()> {
    let skill_dir = cwd.join(".agents").join("skills").join(SKILL_NAME);
    let skill_dir_uri = PathUri::from_host_native_path(&skill_dir)?;
    fs.create_directory(
        &skill_dir_uri,
        CreateDirectoryOptions { recursive: true },
        /*sandbox*/ None,
    )
    .await?;
    let skill_md_uri = PathUri::from_host_native_path(skill_dir.join("SKILL.md"))?;
    fs.write_file(
        &skill_md_uri,
        STUB_SKILL_MD.as_bytes().to_vec(),
        /*sandbox*/ None,
    )
    .await?;

    // Build a plain ZIP "encrypted" package (simulated encryption).
    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("SKILL.md", options)?;
    zip.write_all(REAL_SKILL_MD.as_bytes())?;
    zip.start_file("scripts/build.sh", options)?;
    zip.write_all(b"#!/bin/sh\necho guarded\necho '# REAL_SKILL_CONTENT_MARKER'\n")?;
    let bytes = zip.finish()?.into_inner();

    let package_uri =
        PathUri::from_host_native_path(skill_dir.join(format!("{SKILL_NAME}.zip.enc")))?;
    fs.write_file(&package_uri, bytes, /*sandbox*/ None).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn encrypted_skill_keeps_plaintext_out_of_context_and_rollout() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_config(|config| {
            config.encrypted_skills.sdk = EncryptedSkillsSdkToml::TestZip;
        })
        .with_workspace_setup(move |cwd, fs| async move { write_encrypted_skill(cwd, fs).await });
    let test = builder.build_with_auto_env(&server).await?;
    let skill_path = test
        .config
        .cwd
        .join(format!(".agents/skills/{SKILL_NAME}/SKILL.md"))
        .canonicalize()
        .unwrap_or_else(|_| {
            test.config
                .cwd
                .join(format!(".agents/skills/{SKILL_NAME}/SKILL.md"))
        })
        .to_path_buf();

    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "the skill says: # REAL_SKILL_CONTENT_MARKER"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![
                UserInput::Text {
                    text: format!("please use ${SKILL_NAME}"),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: SKILL_NAME.to_string(),
                    path: skill_path.clone(),
                },
            ],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    core_test_support::wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, codex_protocol::protocol::EventMsg::TurnComplete(_))
    })
    .await;

    // 9.1: rollout contains only the token, never the plaintext.
    let rollout_path = test
        .session_configured
        .rollout_path
        .as_ref()
        .expect("rollout path");
    let rollout = std::fs::read_to_string(rollout_path)?;
    assert!(
        rollout.contains("[SENSITIVE_SKILL_TOKEN:"),
        "rollout should contain the sentinel token"
    );
    assert!(
        !rollout.contains("REAL_SKILL_CONTENT_MARKER"),
        "rollout must not contain skill plaintext"
    );
    assert!(
        rollout.contains("[REDACTED]"),
        "assistant reply quoting skill plaintext must be redacted in rollout"
    );
    assert!(
        !rollout.contains("echo guarded"),
        "rollout must not contain script source"
    );
    assert!(
        !rollout.contains("/dev/shm/fm-agent-security"),
        "rollout must not contain the decrypted path"
    );
    let audit = std::fs::read_to_string(std::env::temp_dir().join("fm_skill_security_audit.log"))
        .unwrap_or_default();
    assert!(
        audit.contains("\"event\":\"decryption\""),
        "audit log should record the decryption, got: {audit}"
    );

    // 9.2: the request sent to the LLM carries framed plaintext and never the
    // decrypted path.
    let request = mock.single_request();
    let user_texts = request.message_input_texts("user");
    assert!(
        user_texts
            .iter()
            .any(|text| text.contains("REAL_SKILL_CONTENT_MARKER")),
        "request should contain rehydrated skill content, got {user_texts:?}"
    );
    assert!(
        user_texts
            .iter()
            .any(|text| text.contains("base_directory")),
        "request should frame the skill content, got {user_texts:?}"
    );
    assert!(
        user_texts
            .iter()
            .all(|text| !text.contains("/dev/shm/fm-agent-security")),
        "request must never contain the decrypted path, got {user_texts:?}"
    );

    Ok(())
}

async fn submit_single_turn(
    test: &core_test_support::test_codex::TestCodex,
    text: &str,
) -> Result<()> {
    submit_single_turn_with_permission_profile(test, text, PermissionProfile::Disabled).await
}

async fn submit_single_turn_with_permission_profile(
    test: &core_test_support::test_codex::TestCodex,
    text: &str,
    permission_profile: PermissionProfile,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;
    core_test_support::wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, codex_protocol::protocol::EventMsg::TurnComplete(_))
    })
    .await;
    Ok(())
}

async fn build_test_with_encrypted_skill(
    server: &MockServer,
) -> Result<core_test_support::test_codex::TestCodex> {
    let mut builder = test_codex()
        .with_config(|config| {
            config.encrypted_skills.sdk = EncryptedSkillsSdkToml::TestZip;
        })
        .with_workspace_setup(move |cwd, fs| async move { write_encrypted_skill(cwd, fs).await });
    builder.build_with_auto_env(server).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_reads_of_decrypted_script_are_blocked_across_tools() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    for (tool, key) in [("shell_command", "command"), ("exec_command", "cmd")] {
        let server = start_mock_server().await;
        let test = build_test_with_encrypted_skill(&server).await?;
        let mock = mount_sse_once(
            &server,
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    "call-1",
                    tool,
                    &serde_json::to_string(&serde_json::json!({
                        key: "cat /dev/shm/fm-agent-security/whatever/scripts/build.sh"
                    }))?,
                ),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-1"),
            ]),
        )
        .await;

        submit_single_turn(&test, "please use $secret-skill").await?;

        let rollout_path = test
            .session_configured
            .rollout_path
            .as_ref()
            .expect("rollout path");
        let rollout = std::fs::read_to_string(rollout_path)?;
        assert!(
            rollout.contains("Direct access to encrypted skill storage is not allowed"),
            "{tool} reads should be blocked by the guard, got: {rollout}"
        );
        let audit =
            std::fs::read_to_string(std::env::temp_dir().join("fm_skill_security_audit.log"))
                .unwrap_or_default();
        assert!(
            audit.contains("\"event\":\"blocked\""),
            "audit log should record the blocked access, got: {audit}"
        );
        let _ = mock;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn text_read_of_skill_md_is_blocked() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_with_encrypted_skill(&server).await?;
    let skill_md = format!(
        "{}/.agents/skills/{SKILL_NAME}/SKILL.md",
        test.config.cwd.as_path().display()
    );
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                "call-1",
                "shell_command",
                &serde_json::to_string(&serde_json::json!({
                    "command": format!("cat {skill_md}")
                }))?,
            ),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    submit_single_turn(&test, "please use $secret-skill").await?;

    let rollout_path = test
        .session_configured
        .rollout_path
        .as_ref()
        .expect("rollout path");
    let rollout = std::fs::read_to_string(rollout_path)?;
    assert!(
        rollout.contains("Direct access to encrypted skill storage is not allowed"),
        "reading SKILL.md from decrypted storage must be blocked, got: {rollout}"
    );
    assert!(
        !rollout.contains("REAL_SKILL_CONTENT_MARKER"),
        "rollout must not contain skill plaintext from a text read, got: {rollout}"
    );
    let _ = mock;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn implicit_read_of_skill_md_injects_encrypted_content() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_with_encrypted_skill(&server).await?;
    let skill_md = format!(
        "{}/.agents/skills/{SKILL_NAME}/SKILL.md",
        test.config.cwd.as_path().display()
    );
    let mocks = mount_function_call_agent_response(
        &server,
        "call-1",
        &serde_json::to_string(&serde_json::json!({
            "command": format!("cat {skill_md}")
        }))?,
        "shell_command",
    )
    .await;

    submit_single_turn(&test, "review the current state of the repo").await?;

    let request = mocks.completion.last_request().expect("second request");
    let injected = request
        .inputs_of_type("message")
        .into_iter()
        .find_map(|item| {
            serde_json::to_string(&item)
                .ok()
                .filter(|text| text.contains("REAL_SKILL_CONTENT_MARKER"))
        })
        .expect("implicitly invoked skill must be injected into the request");
    assert!(
        injected.contains("<skill_name>secret-skill</skill_name>"),
        "decrypted content must be framed, got: {injected}"
    );
    assert!(
        request
            .function_call_output_text("call-1")
            .is_some_and(|output| !output.contains("REAL_SKILL_CONTENT_MARKER")),
        "tool output must keep the stub; plaintext only enters via injection"
    );
    let rollout = std::fs::read_to_string(
        test.session_configured
            .rollout_path
            .as_ref()
            .expect("rollout path"),
    )?;
    assert!(
        !rollout.contains("REAL_SKILL_CONTENT_MARKER"),
        "rollout must never contain skill plaintext, got: {rollout}"
    );
    let _ = mocks;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unrelated_turn_has_no_skill_token_or_framing() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_with_encrypted_skill(&server).await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    submit_single_turn(&test, "hello, no skills here").await?;

    let request = mock.single_request();
    let user_texts = request.message_input_texts("user");
    assert!(
        user_texts
            .iter()
            .all(|text| !text.contains("[SENSITIVE_SKILL_TOKEN:")),
        "unrelated turn must not carry skill tokens, got {user_texts:?}"
    );
    assert!(
        user_texts
            .iter()
            .all(|text| !text.contains("REAL_SKILL_CONTENT_MARKER")),
        "unrelated turn must not carry skill plaintext, got {user_texts:?}"
    );

    let rollout_path = test
        .session_configured
        .rollout_path
        .as_ref()
        .expect("rollout path");
    let rollout = std::fs::read_to_string(rollout_path)?;
    assert!(
        !rollout.contains("[SENSITIVE_SKILL_TOKEN:"),
        "unrelated turn must not persist skill tokens, got: {rollout}"
    );
    assert!(
        !rollout.contains("REAL_SKILL_CONTENT_MARKER"),
        "unrelated turn must not persist skill plaintext, got: {rollout}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engaged_session_keeps_reasoning_in_context_and_rollout() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_with_encrypted_skill(&server).await?;
    let _mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_reasoning_item(
                "rsn-1",
                &["REASONING_SUMMARY_MARKER"],
                &["REASONING_RAW_MARKER"],
            ),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    submit_single_turn(&test, "please use $secret-skill").await?;

    let rollout_path = test
        .session_configured
        .rollout_path
        .as_ref()
        .expect("rollout path");
    let rollout = std::fs::read_to_string(rollout_path)?;
    assert!(
        rollout.contains("REASONING_SUMMARY_MARKER"),
        "reasoning must stay in rollout for context/follow-up requests, got: {rollout}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unrelated_turn_keeps_reasoning_output() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_with_encrypted_skill(&server).await?;
    let _mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_reasoning_item(
                "rsn-1",
                &["REASONING_SUMMARY_MARKER"],
                &["REASONING_RAW_MARKER"],
            ),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    submit_single_turn(&test, "hello, no skills here").await?;

    let rollout_path = test
        .session_configured
        .rollout_path
        .as_ref()
        .expect("rollout path");
    let rollout = std::fs::read_to_string(rollout_path)?;
    assert!(
        rollout.contains("REASONING_SUMMARY_MARKER"),
        "unengaged sessions must keep assistant reasoning, got: {rollout}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_on_mem_root_is_blocked() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_with_encrypted_skill(&server).await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                "call-1",
                "view_image",
                &serde_json::to_string(&serde_json::json!({
                    "path": "/dev/shm/fm-agent-security/whatever/image.png"
                }))?,
            ),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    submit_single_turn(&test, "please use $secret-skill").await?;

    let rollout_path = test
        .session_configured
        .rollout_path
        .as_ref()
        .expect("rollout path");
    let rollout = std::fs::read_to_string(rollout_path)?;
    assert!(
        rollout.contains("Direct access to encrypted skill storage is not allowed"),
        "view_image reads under the memory root should be blocked, got: {rollout}"
    );
    let _ = mock;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compaction_request_keeps_stale_skill_token_unreplaced() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_with_encrypted_skill(&server).await?;
    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("r1"),
                ev_assistant_message("m1", "first"),
                ev_completed("r1"),
            ]),
            sse(vec![
                ev_response_created("r2"),
                ev_assistant_message("m2", "summary"),
                ev_completed("r2"),
            ]),
            sse(vec![
                ev_response_created("r3"),
                ev_assistant_message("m3", "done"),
                ev_completed("r3"),
            ]),
        ],
    )
    .await;

    submit_single_turn(&test, "please use $secret-skill").await?;
    test.codex.submit(Op::Compact).await?;
    core_test_support::wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, codex_protocol::protocol::EventMsg::TurnComplete(_))
    })
    .await;
    submit_single_turn(&test, "continue").await?;

    let requests = request_log.requests();
    assert_eq!(requests.len(), 3, "expected three requests");
    let compaction_request = &requests[1];
    let user_texts = compaction_request.message_input_texts("user");
    assert!(
        user_texts
            .iter()
            .any(|text| text.contains("[SENSITIVE_SKILL_TOKEN:")),
        "compaction request should carry the stale token placeholder, got {user_texts:?}"
    );
    assert!(
        user_texts
            .iter()
            .all(|text| !text.contains("REAL_SKILL_CONTENT_MARKER")),
        "compaction request must not rehydrate turn-unloaded plaintext, got {user_texts:?}"
    );
    assert!(
        user_texts
            .iter()
            .all(|text| !text.contains("/dev/shm/fm-agent-security")),
        "compaction request must never contain the decrypted path"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resumed_session_keeps_stale_skill_tokens_unreplaced() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("r1"),
                ev_assistant_message("m1", "first"),
                ev_completed("r1"),
            ]),
            sse(vec![
                ev_response_created("r2"),
                ev_assistant_message("m2", "after resume"),
                ev_completed("r2"),
            ]),
        ],
    )
    .await;

    let builder = test_codex().with_config(|config| {
        config.encrypted_skills.sdk = EncryptedSkillsSdkToml::TestZip;
    });
    let initial = builder
        .with_workspace_setup(move |cwd, fs| async move { write_encrypted_skill(cwd, fs).await })
        .build_with_auto_env(&server)
        .await?;
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .context("rollout path")?;

    submit_single_turn(&initial, "please use $secret-skill").await?;

    // Resume into a fresh session whose runtime cache is empty: the token in
    // the resumed history must stay as a stale placeholder, never plaintext.
    let mut resume_builder = test_codex().with_config(|config| {
        config.encrypted_skills.sdk = EncryptedSkillsSdkToml::TestZip;
    });
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;
    submit_single_turn(&resumed, "continue without mentioning skills").await?;

    let requests = responses_mock.requests();
    assert_eq!(requests.len(), 2, "expected one request per session");
    let resumed_request = &requests[1];
    let user_texts = resumed_request.message_input_texts("user");
    assert!(
        user_texts
            .iter()
            .any(|text| text.contains("[SENSITIVE_SKILL_TOKEN:")),
        "resumed history should carry the stale token placeholder, got {user_texts:?}"
    );
    assert!(
        user_texts
            .iter()
            .all(|text| !text.contains("REAL_SKILL_CONTENT_MARKER")),
        "resumed request must not rehydrate stale tokens into plaintext"
    );
    assert!(
        user_texts
            .iter()
            .all(|text| !text.contains("/dev/shm/fm-agent-security")),
        "resumed request must never contain the decrypted path"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn script_execution_rewrites_original_path_and_redacts_output() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_with_encrypted_skill(&server).await?;
    let original_script = format!(
        "{}/.agents/skills/{SKILL_NAME}/scripts/build.sh",
        test.config.cwd.as_path().display()
    );
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    "call-1",
                    "shell_command",
                    &serde_json::to_string(&serde_json::json!({
                        "command": format!("bash {original_script}")
                    }))?,
                ),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let cwd = test.config.cwd.clone();
    submit_single_turn_with_permission_profile(
        &test,
        "please use $secret-skill",
        PermissionProfile::workspace_write_with(
            &[cwd],
            NetworkSandboxPolicy::Restricted,
            /*exclude_tmpdir_env_var*/ false,
            /*exclude_slash_tmp*/ false,
        ),
    )
    .await?;

    let rollout_path = test
        .session_configured
        .rollout_path
        .as_ref()
        .expect("rollout path");
    let rollout = std::fs::read_to_string(rollout_path)?;
    assert!(
        rollout.contains("guarded"),
        "rewritten script should execute, got: {rollout}"
    );
    assert!(
        !rollout.contains("REAL_SKILL_CONTENT_MARKER"),
        "script output quoting skill plaintext must be redacted, got: {rollout}"
    );
    assert!(
        rollout.contains("[REDACTED]"),
        "script output quoting skill plaintext must be redacted, got: {rollout}"
    );
    assert!(
        !rollout.contains("/dev/shm/fm-agent-security"),
        "decrypted path must be redacted from tool output, got: {rollout}"
    );
    let _ = mock;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn re_mention_in_next_turn_redecrypts() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_with_encrypted_skill(&server).await?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    submit_single_turn(&test, "please use $secret-skill").await?;
    submit_single_turn(&test, "please use $secret-skill again").await?;

    let rollout_path = test
        .session_configured
        .rollout_path
        .as_ref()
        .expect("rollout path");
    let rollout = std::fs::read_to_string(rollout_path)?;
    let mut tokens = Vec::new();
    for line in rollout.lines() {
        if let Some(start) = line.find("[SENSITIVE_SKILL_TOKEN:") {
            let token = &line[start
                ..line[start..]
                    .find(']')
                    .map(|end| start + end + 1)
                    .unwrap_or(line.len())];
            tokens.push(token.to_string());
        }
    }
    assert_eq!(
        tokens.len(),
        2,
        "expected one token per turn, got {tokens:?}"
    );
    assert_ne!(
        tokens[0], tokens[1],
        "decrypted state is unloaded at turn end, so a re-mention must decrypt a fresh token"
    );
    let _ = mock;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_tool_with_skill_plaintext_is_blocked() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_with_encrypted_skill(&server).await?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_apply_patch_custom_tool_call(
                    "call-1",
                    "*** Begin Patch\n*** Update File: /tmp/leak.md\n+run scripts/build.sh\n*** End Patch",
                ),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    submit_single_turn(&test, "please use $secret-skill").await?;

    let rollout_path = test
        .session_configured
        .rollout_path
        .as_ref()
        .expect("rollout path");
    let rollout = std::fs::read_to_string(rollout_path)?;
    assert!(
        rollout.contains("Blocked by encrypted skill policy"),
        "export attempt should be blocked, got: {rollout}"
    );
    let _ = mock;
    Ok(())
}
