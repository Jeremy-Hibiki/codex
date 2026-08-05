use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

async fn build_server() -> Result<(TestAppServer, TempDir)> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    Ok((mcp, codex_home))
}

async fn read_invalid_request_error(mcp: &mut TestAppServer, request_id: RequestId) -> Result<()> {
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(request_id),
    )
    .await??;
    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "danger-full-access is disabled by product policy"
    );
    Ok(())
}

#[tokio::test]
async fn thread_start_rejects_danger_full_access_sandbox() -> Result<()> {
    let (mut mcp, _codex_home) = build_server().await?;

    let request_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            sandbox: Some(SandboxMode::DangerFullAccess),
            ..Default::default()
        })
        .await?;
    read_invalid_request_error(&mut mcp, RequestId::Integer(request_id)).await?;

    Ok(())
}

#[tokio::test]
async fn turn_start_rejects_danger_full_access_sandbox() -> Result<()> {
    let (mut mcp, _codex_home) = build_server().await?;

    let thread = mcp
        .start_thread(ThreadStartParams {
            ..Default::default()
        })
        .await?
        .thread;
    let request_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![V2UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
            ..Default::default()
        })
        .await?;
    read_invalid_request_error(&mut mcp, RequestId::Integer(request_id)).await?;

    Ok(())
}
