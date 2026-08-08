use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::CommandExecParams;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::ProcessSpawnParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadShellCommandParams;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);
const SKILL_NAME: &str = "rpc-guard-secret";
const MARKER: &str = "RPC_GUARD_PLAINTEXT_MARKER_7f3c9a";
const BLOCK_MESSAGE: &str =
    "Blocked by encrypted skill policy: operation is not allowed while encrypted skills are in use";
const CONFIG_MUTATION_POLICY_ERROR: &str =
    "configuration changes are disabled while encrypted skills are in use";

fn stub_skill_md() -> String {
    format!(
        r#"---
name: {SKILL_NAME}
description: Encrypted skill for RPC guard tests.
metadata:
  encrypted: true
  encryption:
    version: 2
    key_id: required_hardware_key
    algorithm: ZIP-AES-256-CBC
    package: {SKILL_NAME}.zip.enc
---

<!-- ENCRYPTED:SKILL -->
Encrypted stub.
<!-- END:ENCRYPTED -->
"#
    )
}

fn write_encrypted_skill(codex_home: &Path) -> Result<()> {
    let skill_dir = codex_home.join("skills").join(SKILL_NAME);
    std::fs::create_dir_all(skill_dir.join("scripts"))?;
    std::fs::write(skill_dir.join("SKILL.md"), stub_skill_md())?;

    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("SKILL.md", options)?;
    zip.write_all(format!("# {MARKER}\nrun scripts/build.sh").as_bytes())?;
    zip.start_file("scripts/build.sh", options)?;
    zip.write_all(b"#!/bin/sh\necho guarded\n")?;
    let bytes = zip.finish()?.into_inner();
    std::fs::write(skill_dir.join(format!("{SKILL_NAME}.zip.enc")), bytes)?;
    Ok(())
}

fn find_file_with_marker(root: &Path, marker: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_with_marker(&path, marker) {
                return Some(found);
            }
        } else if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md")
            && std::fs::read_to_string(&path).is_ok_and(|content| content.contains(marker))
        {
            return Some(path);
        }
    }
    None
}

async fn find_decrypted_skill_dir() -> Result<PathBuf> {
    let mut roots = vec![PathBuf::from("/dev/shm")];
    roots.push(std::env::temp_dir().join("fm-agent-security"));
    for root in &roots {
        if root.exists()
            && let Some(skill_md) = find_file_with_marker(root, MARKER)
        {
            return skill_md
                .parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| anyhow::anyhow!("decrypted skill dir has no parent"));
        }
    }
    Err(anyhow::anyhow!(
        "decrypted skill dir not found under {roots:?}"
    ))
}

async fn read_error_message(mcp: &mut TestAppServer, request_id: i64) -> Result<String> {
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    Ok(error.error.message)
}

async fn build_engaged_server() -> Result<(
    TestAppServer,
    wiremock::MockServer,
    TempDir,
    TempDir,
    String,
)> {
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "done"),
        responses::ev_completed("resp-1"),
    ]);
    let server = responses::start_mock_server().await;
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responses::sse_response(body).set_delay(Duration::from_millis(4000)))
        .expect(1)
        .mount(&server)
        .await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_extra_config("[encrypted_skills]\nsdk = \"test_zip\"")
        .write(codex_home.path())?;
    let workspace = TempDir::new()?;
    write_encrypted_skill(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    let thread = mcp
        .start_thread(ThreadStartParams {
            cwd: Some(workspace.path().display().to_string()),
            ..Default::default()
        })
        .await?
        .thread;
    let skill_path = codex_home
        .path()
        .join("skills")
        .join(SKILL_NAME)
        .join("SKILL.md");
    let _turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![
                V2UserInput::Text {
                    text: format!("please use ${SKILL_NAME}"),
                    text_elements: Vec::new(),
                },
                V2UserInput::Skill {
                    name: SKILL_NAME.to_string(),
                    path: skill_path,
                },
            ],
            approval_policy: Some(AskForApproval::Never),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("item/started"),
    )
    .await??;

    let mut decrypted_dir = None;
    for _ in 0..50 {
        if let Ok(dir) = find_decrypted_skill_dir().await {
            decrypted_dir = Some(dir);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if decrypted_dir.is_none() {
        anyhow::bail!("decrypted skill dir not found while turn in flight");
    }

    Ok((mcp, server, codex_home, workspace, thread.id))
}

async fn wait_for_turn_completed(mcp: &mut TestAppServer) -> Result<()> {
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rpc_guard_blocks_guarded_paths_when_engaged() -> Result<()> {
    let (mut mcp, _server, _codex_home, workspace, thread_id) = build_engaged_server().await?;
    let decrypted_dir = find_decrypted_skill_dir().await?;
    let guarded_file = decrypted_dir.join("SKILL.md");

    let request_id = mcp
        .send_fs_read_file_request(FsReadFileParams {
            path: AbsolutePathBuf::try_from(guarded_file.clone())?,
        })
        .await?;
    assert_eq!(
        read_error_message(&mut mcp, request_id).await?,
        BLOCK_MESSAGE
    );

    let request_id = mcp
        .send_thread_shell_command_request(ThreadShellCommandParams {
            thread_id: thread_id.clone(),
            command: format!("cat {}", guarded_file.display()),
        })
        .await?;
    assert_eq!(
        read_error_message(&mut mcp, request_id).await?,
        BLOCK_MESSAGE
    );

    let request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["cat".to_string(), guarded_file.display().to_string()],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;
    assert_eq!(
        read_error_message(&mut mcp, request_id).await?,
        BLOCK_MESSAGE
    );

    let request_id = mcp
        .send_process_spawn_request(ProcessSpawnParams {
            command: vec!["cat".to_string(), guarded_file.display().to_string()],
            process_handle: "rpc-guard-spawn".to_string(),
            cwd: AbsolutePathBuf::try_from(workspace.path().to_path_buf())?,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            timeout_ms: None,
            env: None,
            size: None,
        })
        .await?;
    assert_eq!(
        read_error_message(&mut mcp, request_id).await?,
        BLOCK_MESSAGE
    );

    let request_id = mcp
        .send_thread_set_name_request(ThreadSetNameParams {
            thread_id: thread_id.clone(),
            name: guarded_file.display().to_string(),
        })
        .await?;
    assert_eq!(
        read_error_message(&mut mcp, request_id).await?,
        BLOCK_MESSAGE
    );

    let request_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(serde_json::json!({
                "threadId": thread_id,
                "objective": guarded_file.display().to_string(),
            })),
        )
        .await?;
    assert_eq!(
        read_error_message(&mut mcp, request_id).await?,
        BLOCK_MESSAGE
    );

    wait_for_turn_completed(&mut mcp).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rpc_guard_allows_normal_fs_read_when_engaged() -> Result<()> {
    let (mut mcp, _server, _codex_home, workspace, _thread_id) = build_engaged_server().await?;
    let normal_file = workspace.path().join("note.txt");
    std::fs::write(&normal_file, "hello")?;

    let request_id = mcp
        .send_fs_read_file_request(FsReadFileParams {
            path: AbsolutePathBuf::try_from(normal_file)?,
        })
        .await?;
    let response: codex_app_server_protocol::FsReadFileResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(response.data_base64, "aGVsbG8=");

    wait_for_turn_completed(&mut mcp).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rpc_guard_blocks_config_mutation_when_engaged() -> Result<()> {
    let (mut mcp, _server, _codex_home, _workspace, _thread_id) = build_engaged_server().await?;

    for (method, params) in [
        (
            "skills/config/write",
            serde_json::json!({ "enabled": true }),
        ),
        (
            "experimentalFeature/enablement/set",
            serde_json::json!({ "enablement": {} }),
        ),
        ("config/batchWrite", serde_json::json!({ "edits": [] })),
    ] {
        let request_id = mcp.send_raw_request(method, Some(params)).await?;
        assert_eq!(
            read_error_message(&mut mcp, request_id).await?,
            CONFIG_MUTATION_POLICY_ERROR
        );
    }

    wait_for_turn_completed(&mut mcp).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rpc_guard_allows_config_mutation_when_unengaged() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "skills/config/write",
            Some(serde_json::json!({ "enabled": true })),
        )
        .await?;
    let message = read_error_message(&mut mcp, request_id).await?;
    assert_ne!(message, CONFIG_MUTATION_POLICY_ERROR);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rpc_guard_allows_mem_root_paths_when_unengaged() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_fs_read_file_request(FsReadFileParams {
            path: AbsolutePathBuf::try_from(PathBuf::from(
                "/dev/shm/fm-agent-security/fm_skill_security_nonexistent/secret",
            ))?,
        })
        .await?;
    let message = read_error_message(&mut mcp, request_id).await?;
    assert_ne!(message, BLOCK_MESSAGE);

    Ok(())
}
