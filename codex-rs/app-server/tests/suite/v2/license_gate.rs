//! License request-gate tests for app-server.
//!
//! These tests spawn the real `codex-app-server` binary with the debug-only
//! test env vars that force the license into the "lost" state, then verify
//! that requests which start new Codex work are rejected while other requests
//! still work.

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const LICENSE_UNAVAILABLE_ERROR_CODE: i64 = -32002;

#[tokio::test]
async fn license_lost_blocks_thread_start() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[
            (fm_license::TEST_BYPASS_ENV_VAR, Some("1")),
            (fm_license::TEST_FORCE_LOST_ENV_VAR, Some("1")),
        ])
        .build_initialized_with_timeout(Duration::from_secs(30))
        .await?;

    let request_id = server
        .send_thread_start_request(ThreadStartParams::default())
        .await?;

    let error: JSONRPCError = timeout(
        Duration::from_secs(30),
        server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, LICENSE_UNAVAILABLE_ERROR_CODE);
    assert_eq!(error.error.message, fm_license::LICENSE_UNAVAILABLE_MESSAGE);
    Ok(())
}

#[tokio::test]
async fn license_lost_allows_non_work_requests() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[
            (fm_license::TEST_BYPASS_ENV_VAR, Some("1")),
            (fm_license::TEST_FORCE_LOST_ENV_VAR, Some("1")),
        ])
        .build_initialized_with_timeout(Duration::from_secs(30))
        .await?;

    let request_id = server
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;
    // A successful `ConfigReadResponse` (rather than the license error) proves
    // non-work requests keep working while the license is lost.
    let _response: codex_app_server_protocol::ConfigReadResponse =
        timeout(Duration::from_secs(30), server.read_response(request_id)).await??;
    Ok(())
}
