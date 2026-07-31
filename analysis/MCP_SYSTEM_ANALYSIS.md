# Codex MCP 系统分析

## 概述

Codex 的 Model Context Protocol (MCP) 系统支持两种传输方式，具有三层架构：注册发现、认证鉴权、执行调用。本文档基于源码详细分析各层实现机制。

## 1. 传输模型

Codex MCP 支持两种传输方式，认证鉴权完全由传输类型决定：

### 1.1 Stdio 传输（本地子进程）

```rust
// codex-rs/codex-mcp/src/rmcp_client.rs:1050-1080
McpServerTransportConfig::Stdio {
    command,
    args,
    env,
    env_vars,
    cwd,
}
```

**特点**：
- 运行本地子进程，通过标准输入/输出通信
- 不支持 OAuth 认证
- 认证状态：`McpAuthState::Unsupported` (mcp/auth.rs:224)
- 适用于本地工具和插件提供的 MCP 服务器

### 1.2 StreamableHttp 传输（远程服务器）

```rust
// codex-rs/codex-mcp/src/rmcp_client.rs:1081-1110
McpServerTransportConfig::StreamableHttp {
    url,
    http_headers,
    env_http_headers,
    bearer_token_env_var,
}
```

**特点**：
- 通过 HTTP/HTTPS 通信，支持 SSE 流式传输
- 支持 OAuth 2.0 认证
- 支持会话管理和 token 刷新
- 适用于远程 MCP 服务器和 Codex Apps

---

## 2. 注册与发现

MCP 服务器的注册来自三个来源，通过 `McpCatalogBuilder` 解决冲突：

### 2.1 注册来源

#### A. 配置文件注册（用户配置）

```rust
// codex-rs/core/src/config/mod.rs:1701-1705
for (name, server) in self.mcp_servers.get() {
    catalog.register(McpServerRegistration::from_config(
        name.clone(),
        server.clone(),
    ));
}
```

- 配置文件：`config.toml` 中的 `[mcp_servers]` 块
- 最低优先级，可被插件和扩展覆盖

#### B. 插件注册（自动发现）

```rust
// codex-rs/core/src/config/mod.rs:1690-1693
catalog.register(McpServerRegistration::from_plugin(
    name,
    attribution.clone(),
    plugin_order,
    plugin_server,
));
```

- 插件通过 `McpPluginAttribution` 提供身份信息
- 中等优先级，基于 `plugin_order` 排序

#### C. 扩展注册（内置服务）

```rust
// codex-rs/core/src/mcp.rs:206-209
catalog.register(McpServerRegistration::from_compatibility(
    CODEX_APPS_MCP_SERVER_NAME.to_string(),
    LEGACY_CODEX_APPS_REGISTRATION_ID,
    codex_apps_mcp_server_config(
```

- Codex Apps 内置服务
- 兼容层注册，最高优先级

### 2.2 冲突解决

```rust
// codex-rs/codex-mcp/src/catalog.rs
pub enum McpServerSource {
    Config,       // 配置文件注册
    Plugin,       // 插件注册
    Extension,    // 扩展注册
}
```

**优先级规则**（catalog.rs:40-80）：
1. `Extension` > `Plugin` > `Config`
2. 同级中，后注册的覆盖先注册的
3. 冲突操作支持注册和删除

---

## 3. 认证与鉴权

**重要**：认证鉴权仅适用于 `StreamableHttp` 传输。`Stdio` 传输返回 `McpAuthState::Unsupported` (mcp/auth.rs:224)。

### 3.1 OAuth 认证流程

#### 3.1.1 认证状态检测

```rust
// codex-rs/rmcp-client/src/auth_status.rs:75-110
pub async fn determine_streamable_http_auth_status(
    server_name: &str,
    url: &str,
    bearer_token_env_var: Option<&str>,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    http_client: Arc<dyn HttpClient>,
    discovery_timeout: OAuthDiscoveryTimeout,
) -> Result<McpAuthState>
```

**返回状态**（auth_status.rs:22-31）：
- `McpAuthState::Authenticated` - 已认证
- `McpAuthState::NotAuthenticated` - 未认证
- `McpAuthState::Unsupported` - 不支持 OAuth（Stdio 传输）
- `McpAuthState::LoginRequired` - 需要登录
- `McpAuthState::AuthenticationRequired` - 认证失败需要重新授权

#### 3.1.2 Token 存储策略

```rust
// codex-rs/rmcp-client/src/oauth.rs:269-284
pub fn save_oauth_tokens(
    server_name: &str,
    tokens: &StoredOAuthTokens,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<()> {
    match store_mode {
        OAuthCredentialsStoreMode::Auto => save_oauth_tokens_with_keyring_with_fallback_to_file(...),
        OAuthCredentialsStoreMode::File => save_oauth_tokens_to_file(tokens),
        OAuthCredentialsStoreMode::Keyring => save_oauth_tokens_with_keyring_and_cleanup_file(...),
    }
}
```

**存储模式**：
- `Auto`：优先 keyring，失败回退到文件
- `File`：仅使用文件存储（`CODEX_HOME/.credentials.json`）
- `Keyring`：仅使用 keyring 存储

#### 3.1.3 Token 刷新机制

```rust
// codex-rs/rmcp-client/src/oauth/refresh_transaction.rs:33-83
pub(crate) async fn refresh_if_needed(&self) -> Result<()> {
    self.refresh_if_needed_in(&DefaultKeyringStore, REFRESH_REQUEST_TIMEOUT).await
}
```

**刷新特性**（refresh_transaction.rs:43-153）：
1. **序列化**：通过 `RefreshCredentialLock` 跨进程序列化刷新事务
2. **重新读取**：获取锁后重新读取权威凭证，避免并发问题
3. **超时控制**：刷新请求有 45 秒超时（REFRESH_REQUEST_TIMEOUT）
4. **失败策略**：
   - 网络错误：返回错误，允许后续重试
   - Token 拒绝（`TokenRefreshRejected`）：返回 `AuthorizationRequired`，需要重新授权
   - 超时：记录警告，允许后续重试
   - 持久化失败：恢复旧凭证，返回错误

**锁机制**（oauth/refresh_lock.rs:45-99）：
```rust
pub(super) struct RefreshCredentialLock {
    _file: File,  // 文件锁在 Drop 时自动释放
}
```

- 获取超时：60 秒（REFRESH_LOCK_ACQUIRE_TIMEOUT）
- 重试间隔：50 毫秒（REFRESH_LOCK_RETRY_SLEEP）
- 锁文件位置：`CODEX_HOME/mcp-oauth-locks/{hash}.lock`

#### 3.1.4 HTTP 401 错误处理

```rust
// codex-rs/rmcp-client/src/http_client_adapter.rs:185-206
if response.status == StatusCode::UNAUTHORIZED.as_u16()
    && let Some(header) = response_header(&response.headers, WWW_AUTHENTICATE)
{
    return Err(StreamableHttpError::AuthRequired(AuthRequiredError::new(header)));
}
if response.status == StatusCode::FORBIDDEN.as_u16()
    && let Some(challenge) = insufficient_scope_challenge(&response.headers)
{
    return Err(StreamableHttpError::InsufficientScope(
        InsufficientScopeError::new(
            challenge.www_authenticate_header,
            challenge.required_scope,
        ),
    ));
}
```

**401 错误解析**（http_client_adapter/www_authenticate.rs）：
- 提取 `WWW-Authenticate` 头部
- 解析 `Bearer insufficient_scope` 挑战
- 提取 `required_scope` 参数

---

### 3.2 认证失败处理（两层模型）

#### Layer A：启动/初始化 401 错误

**触发位置**：MCP 客户端初始化时的 HTTP 请求（http_client_adapter.rs:185-197）

**处理流程**（streamable_http_retry.rs:25-178）：
```rust
StreamableHttpError::AuthRequired(_)  // 不重试
| StreamableHttpError::InsufficientScope(_)  // 不重试
| StreamableHttpError::SessionExpired  // 不重试
```

**错误传播**（connection_manager.rs:413-453）：
```rust
Err(error) if error.is_authentication_required() && !has_runtime_auth => {
    // 检测认证状态并显示错误信息
    let auth_state = determine_streamable_http_auth_status_from_credentials(...)?;
    // 返回 McpStartupFailureReason::ReauthenticationRequired
}
```

**认证失败原因检测**（connection_manager/startup.rs:55-66）：
```rust
pub(super) fn mcp_startup_failure_reason(
    auth_state: Option<McpAuthState>,
    error: &StartupOutcomeError,
) -> Option<McpStartupFailureReason> {
    if !error.is_authentication_required() {
        return None;
    }
    match auth_state {
        Some(McpAuthState::LoggedOut(McpLoginRequirement::Reauthentication)) => {
            Some(McpStartupFailureReason::ReauthenticationRequired)
        }
        // 其他情况不返回认证失败原因
        _ => None,
    }
}
```

**用户可见的错误消息**（connection_manager/startup.rs:74-116）：
```rust
pub(super) fn mcp_init_error_display(
    server_name: &str,
    config: Option<&McpServerConfig>,
    error: &StartupOutcomeError,
) -> String {
    if matches!(error, StartupOutcomeError::Failed { error, .. } if error.contains("Auth required")) {
        format!(
            "The {server_name} MCP server is not logged in. Run `codex mcp login {server_name}`."
        )
    }
    // ... 其他错误处理
}
```

**结论**：Layer A 的 401 错误**不会自动触发重新认证**，而是：
1. 失败连接，显示错误信息
2. 用户必须手动运行 `codex mcp login {server_name}` 重新授权
3. 适用于所有 StreamableHttp MCP 服务器

#### Layer B：工具结果认证失败

**触发位置**：MCP 工具执行后的结果元数据（mcp_tool_call.rs:709-711）

**检测逻辑**（auth_elicitation.rs:80-108）：
```rust
pub fn connector_auth_failure_from_tool_result(
    result: &CallToolResult,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
    install_url: Option<String>,
) -> Option<CodexAppsConnectorAuthFailure> {
    let auth_failure = result.meta
        .and_then(|meta| meta.get(CONNECTOR_AUTH_FAILURE_META_KEY))?
        .as_object()?;
    
    if auth_failure.get(CONNECTOR_AUTH_FAILURE_IS_AUTH_FAILURE_KEY)?.as_bool() != Some(true) {
        return None;
    }
    // ... 提取认证失败详细信息
}
```

**关键元数据**（auth_elicitation.rs:20-29）：
```rust
pub const CONNECTOR_AUTH_FAILURE_META_KEY: &str = "connector_auth_failure";
pub const CONNECTOR_AUTH_FAILURE_IS_AUTH_FAILURE_KEY: &str = "is_auth_failure";
pub const CONNECTOR_AUTH_FAILURE_AUTH_REASON_KEY: &str = "auth_reason";
pub const CONNECTOR_AUTH_FAILURE_CONNECTOR_ID_KEY: &str = "connector_id";
pub const CONNECTOR_AUTH_FAILURE_ERROR_CODE_KEY: &str = "error_code";
```

**自动征询流程**（mcp_tool_call.rs:670-710）：
```rust
async fn maybe_request_codex_apps_auth_elicitation(
    sess: &Arc<Session>,
    turn_context: &TurnContext,
    call_id: &str,
    server: &str,
    metadata: Option<&McpToolApprovalMetadata>,
    result: CallToolResult,
) -> CallToolResult {
    // 1. 构建认证征询计划
    let plan = build_auth_elicitation_plan(call_id, &result, connector_id, connector_name, install_url)?;
    
    // 2. 发起 URL 征询请求
    let response = sess.request_mcp_server_elicitation(
        turn_context,
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        request_id,
        ElicitationRequest::Url { ... },
    ).await.response;
    
    // 3. 用户接受后刷新工具目录
    if response.action == ElicitationAction::Accept {
        refresh_codex_apps_after_connector_auth(sess, turn_context).await;
        auth_elicitation_completed_result(&plan.auth_failure, result.meta)
    }
}
```

**热重载工具目录**（mcp_tool_call.rs:713-729）：
```rust
async fn refresh_codex_apps_after_connector_auth(sess: &Arc<Session>, turn_context: &TurnContext) {
    // 1. 硬刷新 Codex Apps 工具
    let mcp_tools_result = sess.hard_refresh_latest_codex_apps_tools().await;
    
    // 2. 更新可访问的连接器缓存
    match mcp_tools_result {
        Ok(mcp_tools) => {
            let auth = sess.services.auth_manager.auth().await;
            connectors::refresh_accessible_connectors_cache_from_mcp_tools(
                &turn_context.config,
                auth.as_ref(),
                &mcp_tools,
            );
        }
        Err(err) => {
            tracing::warn!("failed to refresh Codex Apps tools after connector auth: {err:#}");
        }
    }
}
```

**结论**：Layer B 的认证失败**自动触发征询 + 热重载**，但：
1. **仅适用于 Codex Apps MCP 服务器**（CODEX_APPS_MCP_SERVER_NAME）
2. 依赖 MCP 服务器在工具结果中返回 `connector_auth_failure` 元数据
3. 自动征询用户，用户接受后立即刷新工具目录
4. **不是通用 MCP 服务器的认证失败处理机制**

---

### 3.3 重试机制

**适用场景**：仅适用于 MCP 初始化/握手阶段的可重试错误

**重试次数**（streamable_http_retry.rs:17）：
```rust
const STREAMABLE_HTTP_RETRY_DELAYS_MS: [u64; 2] = [250, 1_000];
```

**重试条件**（streamable_http_retry.rs:119-166）：
```rust
fn is_retryable_streamable_http_error(error: &StreamableHttpError<...>) -> bool {
    match error {
        StreamableHttpError::Client(ExecServerError::HttpRequest(_)) => true,  // HTTP 请求错误
        StreamableHttpError::Client(ExecServerError::Server { code, message }) => {
            *code == JSON_RPC_INTERNAL_ERROR_CODE && message.starts_with("http/request failed:")
        }
        StreamableHttpError::UnexpectedServerResponse(message) => {
            is_retryable_unexpected_server_response(message.as_ref())
        }
        // 以下错误不重试：
        StreamableHttpError::AuthRequired(_)          // 401 认证失败
        | StreamableHttpError::InsufficientScope(_)   // 403 权限不足
        | StreamableHttpError::SessionExpired         // 会话过期
        | StreamableHttpError::UnexpectedContentType   // 错误的内容类型
        | StreamableHttpError::ServerDoesNotSupportSse // 不支持 SSE
        | StreamableHttpError::Deserialize(_)         // 反序列化失败
        | StreamableHttpError::Header(_)              // 头部错误
        => false,
    }
}
```

**重试逻辑**（streamable_http_retry.rs:25-92）：
```rust
for (attempt, retry_delay_ms) in STREAMABLE_HTTP_RETRY_DELAYS_MS
    .iter()
    .copied()
    .map(Some)
    .chain(std::iter::once(None))  // 最后一次尝试不延迟
    .enumerate()
{
    // ... 初始化逻辑
    
    match self.connect_pending_transport(...).await {
        Ok(result) => return Ok(result),
        Err(error) if should_retry && Self::is_retryable_initialize_error(&error) => {
            let Some(retry_delay_ms) = retry_delay_ms else {
                return Err(error);  // 最后一次尝试失败，返回错误
            };
            // ... 等待重试
        }
        Err(error) => return Err(error),  // 不可重试错误，立即返回
    }
}
```

**重要**：
- 重试次数：最多 3 次（2 次延迟 + 1 次最终尝试）
- 延迟时间：250ms → 1000ms → 无延迟
- **不重试认证错误**（401/403），这些错误需要用户干预
- 重试不影响 OAuth 刷新的 45 秒超时（refresh_transaction.rs:34）

---

## 4. 执行与审批

### 4.1 执行入口点

```rust
// codex-rs/core/src/codex_thread.rs:669-683
pub async fn call_mcp_tool(
    &self,
    server: &str,
    tool: &str,
    arguments: Option<serde_json::Value>,
    meta: Option<serde_json::Value>,
) -> anyhow::Result<CallToolResult> {
    self.session.refresh_mcp_if_dirty().await;
    self.session
        .services
        .mcp_runtime
        .latest_call_tool(server, tool, arguments, meta)
        .await
}
```

**流程**：
1. 刷新 MCP 运行时（如果需要）
2. 调用最新的 MCP 运行时执行工具调用
3. 返回工具执行结果

### 4.2 工具调用流程

```rust
// codex-rs/codex-mcp/src/connection_manager.rs:624-645
pub async fn call_tool(
    &self,
    server: &str,
    tool: &str,
    arguments: Option<serde_json::Value>,
    meta: Option<serde_json::Value>,
) -> Result<CallToolResult> {
    // 1. 检查服务器是否存在
    let view = self.servers.get(server)
        .ok_or_else(|| anyhow!("unknown MCP server '{server}'"))?;
    
    // 2. 检查工具是否被允许
    if !view.tool_filter.allows(tool) {
        return Err(anyhow!("tool '{tool}' is disabled for MCP server '{server}'"));
    }
    
    // 3. 获取客户端
    let client = view.connection.client().await?;
    
    // 4. 调用工具
    let result: rmcp::model::CallToolResult = client
        .client
        .call_tool(tool.to_string(), arguments, meta, view.tool_timeout)
        .await?;
    
    // 5. 转换结果
    Ok(CallToolResult {
        content: result.content.into_iter().map(...).collect(),
        structured_content: result.structured_content,
        is_error: result.is_error,
        meta: result.meta.and_then(|meta| serde_json::to_value(meta).ok()),
    })
}
```

### 4.3 审批流程

```rust
// codex-rs/core/src/mcp_tool_call.rs:68-150
pub(crate) async fn handle_mcp_tool_call(
    sess: Arc<Session>,
    step_context: &Arc<StepContext>,
    call_id: String,
    server: String,
    tool_name: String,
    hook_tool_name: HookToolName,
    arguments: String,
) -> HandledMcpToolCall {
    // 1. 准备调用
    let invocation = McpInvocation { ... };
    let prepared_call = sess.services.mcp_runtime.latest_binding().prepare_call(...)?;
    let metadata = mcp_tool_metadata(&prepared_call);
    
    // 2. 请求审批
    let decision = maybe_request_mcp_tool_approval(
        sess,
        step_context,
        &call_id,
        &invocation,
        &hook_tool_name,
        Some(&metadata),
        &config,
        policy,
    ).await;
    
    // 3. 处理审批决策
    match decision {
        Some(McpToolApprovalDecision::Accept) |
        Some(McpToolApprovalDecision::AcceptForSession) |
        Some(McpToolApprovalDecision::AcceptAndRemember) => {
            handle_approved_mcp_tool_call(...).await
        }
        Some(McpToolApprovalDecision::Decline { message }) => {
            notify_mcp_tool_call_skip(..., message.unwrap_or("user declined")).await
        }
        Some(McpToolApprovalDecision::Cancel) => {
            notify_mcp_tool_call_skip(..., "user cancelled").await
        }
        None => {
            // 无需审批，直接执行
            handle_approved_mcp_tool_call(...).await
        }
    }
}
```

#### 4.3.1 审批策略

```rust
// codex-rs/core/src/mcp_tool_call.rs:420-440
pub(crate) const MCP_TOOL_APPROVAL_ACCEPT: &str = "Allow";
pub(crate) const MCP_TOOL_APPROVAL_ACCEPT_FOR_SESSION: &str = "Allow for this session";
pub(crate) const MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER: &str = "Allow and don't ask me again";
pub(crate) const MCP_TOOL_APPROVAL_CANCEL: &str = "Cancel";
```

**审批策略**（mcp_tool_call.rs:1195-1220）
- `OnRequest`：每次都请求审批
- `UnlessTrusted`：信任的工具不请求审批
- `Granular`：细粒度控制，支持 `AcceptForSession`（会话级）和 `AcceptAndRemember`（持久化）

#### 4.3.2 记住审批决策

**审批决策枚举**（mcp_tool_call.rs:1018-1024）
```rust
enum McpToolApprovalDecision {
    Accept,
    AcceptForSession,
    AcceptAndRemember,
    Decline { message: Option<String> },
    Cancel,
}
```

**会话级记忆**（mcp_tool_call.rs:1951-1954）
```rust
async fn remember_mcp_tool_approval(sess: &Session, key: McpToolApprovalKey) {
    let mut store = sess.services.tool_approvals.lock().await;
    store.put(key, ReviewDecision::ApprovedForSession);
}
```

**记忆存储类型**：
- **会话级记忆**（`AcceptForSession`）：写入内存存储 `sess.services.tool_approvals`，**无时间限制**，仅在会话期间有效
- **持久化记忆**（`AcceptAndRemember`）：写入用户配置文件，跨会话持久化
```

---

## 5. 常量汇总

| 常量 | 值 | 源码位置 |
|------|-----|---------|
| `STREAMABLE_HTTP_RETRY_DELAYS_MS` | `[250, 1_000]` | rmcp-client/src/streamable_http_retry.rs:17 |
| `REFRESH_REQUEST_TIMEOUT` | `45s` | rmcp-client/src/oauth/refresh_transaction.rs:20 |
| `MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES` | `1 MiB` | rmcp-client/src/oauth_http_client.rs:31 |
| `REFRESH_LOCK_ACQUIRE_TIMEOUT` | `60s` | rmcp-client/src/oauth/refresh_lock.rs:13 |
| `REFRESH_LOCK_RETRY_SLEEP` | `50ms` | rmcp-client/src/oauth/refresh_lock.rs:14 |
| `DEFAULT_STARTUP_TIMEOUT` | `30s` | codex-mcp/src/rmcp_client.rs:85 |
| `DEFAULT_TOOL_TIMEOUT` | `300s` | codex-mcp/src/rmcp_client.rs:86 |

---

## 6. 总结

Codex MCP 系统采用分层架构：

### 传输层
- **Stdio**：本地子进程，不支持 OAuth
- **StreamableHttp**：远程服务器，支持 OAuth 2.0

### 注册层
- **配置文件**：`config.toml` [mcp_servers]，最低优先级
- **插件**：自动发现，中等优先级
- **扩展**：内置服务（Codex Apps），最高优先级

### 认证层（仅 StreamableHttp）
- **Layer A（启动 401）**：连接失败，显示错误，手动登录
- **Layer B（工具结果认证失败）**：自动征询 + 热重载，仅 Codex Apps
- **Token 存储**：Auto（优先 keyring） / File / Keyring
- **Token 刷新**：跨进程序列化，45 秒超时，失败回滚

### 执行层
- **入口点**：`codex_thread.rs:669` 的 `call_mcp_tool`
- **审批流程**：支持会话级和持久化记忆
- **错误处理**：401/403 不重试，网络错误最多重试 3 次

### 关键设计原则
- **传输门控认证**：Stdio 传输不支持 OAuth，避免不必要的认证逻辑
- **双层认证失败处理**：启动失败手动处理，工具结果失败自动征询（仅 Codex Apps）
- **跨进程序列化**：Token 刷新使用文件锁，避免并发问题
- **渐进式审批**：支持记住决策，减少重复询问
- **失败安全策略**：Token 刷新失败时回滚旧凭证，不服务未持久化的令牌
