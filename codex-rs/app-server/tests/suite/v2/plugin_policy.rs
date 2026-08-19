use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::RequestId;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const PLUGIN_POLICY_ERROR: &str = "only managed plugins are allowed by product policy";
const MARKETPLACE_POLICY_ERROR: &str = "only managed marketplaces are allowed by product policy";

async fn build_server_with_policy(
    allow_managed_plugins_only: bool,
    allow_managed_marketplaces_only: bool,
) -> Result<(TestAppServer, TempDir)> {
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new("http://localhost/unused").write(codex_home.path())?;
    std::fs::write(
        codex_home.path().join("requirements.toml"),
        format!(
            "allow_managed_plugins_only = {allow_managed_plugins_only}\nallow_managed_marketplaces_only = {allow_managed_marketplaces_only}\n"
        ),
    )?;
    let mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    Ok((mcp, codex_home))
}

async fn build_server() -> Result<(TestAppServer, TempDir)> {
    build_server_with_policy(false, false).await
}

async fn assert_policy_rejection(
    mcp: &mut TestAppServer,
    method: &str,
    params: Value,
    expected_message: &str,
) -> Result<()> {
    let request_id = mcp.send_raw_request(method, Some(params)).await?;
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.message, expected_message);
    Ok(())
}

#[tokio::test]
async fn marketplace_rpcs_are_rejected_by_product_policy() -> Result<()> {
    let (mut mcp, _codex_home) = build_server_with_policy(false, true).await?;
    for (method, params) in [
        (
            "marketplace/add",
            json!({ "source": "file:///tmp/example" }),
        ),
        (
            "marketplace/remove",
            json!({ "marketplaceName": "example" }),
        ),
        ("marketplace/upgrade", json!({})),
    ] {
        assert_policy_rejection(&mut mcp, method, params, MARKETPLACE_POLICY_ERROR).await?;
    }
    Ok(())
}

#[tokio::test]
async fn plugin_share_rpcs_are_rejected_by_product_policy() -> Result<()> {
    let (mut mcp, _codex_home) = build_server_with_policy(true, false).await?;
    for (method, params) in [
        ("plugin/share/save", json!({ "pluginPath": "/tmp/example" })),
        (
            "plugin/share/updateTargets",
            json!({
                "remotePluginId": "example",
                "discoverability": "PRIVATE",
                "shareTargets": [],
            }),
        ),
        (
            "plugin/share/checkout",
            json!({ "remotePluginId": "example" }),
        ),
        (
            "plugin/share/delete",
            json!({ "remotePluginId": "example" }),
        ),
    ] {
        assert_policy_rejection(&mut mcp, method, params, PLUGIN_POLICY_ERROR).await?;
    }
    Ok(())
}

#[tokio::test]
async fn read_only_plugin_listing_rpcs_are_not_blocked_by_product_policy() -> Result<()> {
    let (mut mcp, _codex_home) = build_server().await?;
    let request_id = mcp.send_raw_request("plugin/list", Some(json!({}))).await?;
    let response: serde_json::Value =
        timeout(Duration::from_secs(3), mcp.read_response(request_id)).await??;
    assert_eq!(
        response,
        serde_json::json!({
            "featuredPluginIds": [],
            "marketplaceLoadErrors": [],
            "marketplaces": []
        })
    );
    Ok(())
}

#[tokio::test]
async fn plugin_mutation_rpcs_are_not_blocked_by_product_policy_by_default() -> Result<()> {
    let (mut mcp, _codex_home) = build_server().await?;
    // Mutation requests that fail for non-policy reasons must never report
    // the product-policy error.
    for (method, params) in [
        (
            "marketplace/add",
            json!({ "source": "file:///tmp/example" }),
        ),
        ("plugin/install", json!({ "pluginName": "example" })),
    ] {
        let request_id = mcp.send_raw_request(method, Some(params)).await?;
        let error: JSONRPCError = timeout(
            Duration::from_secs(3),
            mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
        )
        .await??;
        assert_ne!(
            error.error.message, PLUGIN_POLICY_ERROR,
            "{method} must not be blocked by product policy by default"
        );
        assert_ne!(
            error.error.message, MARKETPLACE_POLICY_ERROR,
            "{method} must not be blocked by product policy by default"
        );
    }
    // And a mutation that succeeds must not be blocked either.
    let request_id = mcp
        .send_raw_request(
            "plugin/uninstall",
            Some(json!({ "pluginId": "example@market" })),
        )
        .await?;
    let response: serde_json::Value =
        timeout(Duration::from_secs(3), mcp.read_response(request_id)).await??;
    assert_eq!(response, serde_json::json!({}));
    Ok(())
}

#[tokio::test]
async fn plugin_install_rpcs_are_rejected_by_product_policy() -> Result<()> {
    let (mut mcp, _codex_home) = build_server_with_policy(true, false).await?;
    for (method, params) in [
        ("plugin/install", json!({ "pluginName": "example" })),
        ("plugin/uninstall", json!({ "pluginId": "example@market" })),
    ] {
        assert_policy_rejection(&mut mcp, method, params, PLUGIN_POLICY_ERROR).await?;
    }
    Ok(())
}
