# `config.toml` 字段说明

本文基于当前 `ConfigToml` 生成的 schema。配置使用严格字段校验，未知顶层字段会报 `unknown configuration field`。字段名使用 snake_case。

## 最小示例

```toml
model = "deepseek-v4-flash-0731"
model_provider = "my-provider"
model_catalog_json = "my-models.json"
approval_policy = "never"
default_permissions = ":workspace"
web_search = "disabled"

[model_providers.my-provider]
name = "My Provider"
base_url = "https://example.com/v1"
wire_api = "responses"
```

## `web_search`

不能用布尔值：

```toml
# 错误
web_search = false
```

正确写法是枚举字符串：

```toml
web_search = "disabled"
```

取值：

- `disabled`：不提供 web search 工具。
- `cached`：默认值，cached search。
- `indexed`：indexed search。
- `live`：live search。

另有一个嵌套工具配置：

```toml
[tools.web_search]
# 见 WebSearchToolConfig；用于 filters、location、context size 等细化设置
```

注意 `[tools.web_search]` 不是简单开关；布尔写法会被反序列化成 `None`，不应依赖。

## 顶层字段总表

“默认”列为 schema/类型层面可确认的值；未标注的嵌套结构以对应配置类型为准。

| 字段 | 类型/取值 | 默认 | 说明 |
|---|---|---:|---|
| `agents` | table | 无 | 子代理并发、深度、默认模型、角色等。 |
| `allow_login_shell` | bool | `true` | 是否允许模型请求 login shell。 |
| `allow_managed_marketplaces_only` | bool | `true` | 是否允许 marketplace 管理。 |
| `allow_managed_plugins_only` | bool | `true` | 是否允许插件管理。 |
| `allow_sandbox_bypass` | bool | `true` | full access 是否可绕过强制沙箱要求。 |
| `analytics` | bool/table | `true` | analytics 总开关。 |
| `approval_policy` | string/table | 无 | 审批策略。 |
| `approvals_reviewer` | `user` / `auto_review` / `guardian_subagent` | 无 | 审批请求由谁审查。 |
| `apps` | table | 无 | App 控制配置。 |
| `apps_mcp_product_sku` | string | 无 | Apps MCP 产品 SKU。 |
| `audio` | table | 无 | 实时语音设备配置。 |
| `auto_review` | table | 无 | Guardian reviewer 策略文案。 |
| `background_terminal_max_timeout` | integer | `300000` | `write_stdin` 最大轮询窗口，毫秒。 |
| `chatgpt_base_url` | string | 无 | ChatGPT backend base URL。 |
| `check_for_update_on_startup` | bool | `true` | 启动检查更新。 |
| `cli_auth_credentials_store` | `file` / `keyring` / `auto` / `ephemeral` | `file` | CLI auth 存储后端。 |
| `compact_prompt` | string | 无 | 历史压缩 prompt 覆盖。 |
| `debug` | table | 无 | 调试与复现配置。 |
| `default_permissions` | string | 无 | 默认权限 profile；`:workspace` 为内置。 |
| `desktop` | table | 无 | 桌面端 opaque 配置。 |
| `developer_instructions` | string | 无 | 注入 developer message。 |
| `disable_paste_burst` | bool | `false` | 禁用粘贴爆发检测。 |
| `encrypted_skills` | table | 无 | 加密技能 SDK/TTL 配置。 |
| `experimental_compact_prompt_file` | path | 无 | 压缩 prompt 文件。 |
| `experimental_realtime_start_instructions` | string | 无 | 实时会话启动指令覆盖。 |
| `experimental_realtime_webrtc_call_base_url` | string | 无 | WebRTC call endpoint。 |
| `experimental_realtime_ws_backend_prompt` | string | 无 | realtime WS 指令覆盖。 |
| `experimental_realtime_ws_base_url` | string | 无 | realtime WS base URL。 |
| `experimental_realtime_ws_model` | string | 无 | realtime WS 模型/快照。 |
| `experimental_realtime_ws_startup_context` | string | 无 | realtime 启动上下文覆盖；空字符串禁用注入。 |
| `experimental_thread_config_endpoint` | string | 无 | 远端 thread config endpoint。 |
| `experimental_thread_store` | table | 无 | thread store 选择。 |
| `experimental_use_unified_exec_tool` | bool | 无 | 旧 Unified Exec 开关。 |
| `features` | table | 无 | 集中 feature flags。 |
| `feedback` | bool/table | `true` | 反馈收集开关。 |
| `file_opener` | table | 无 | URI 文件打开器。 |
| `forced_chatgpt_workspace_id` | string/array | 无 | 限制 ChatGPT workspace。 |
| `forced_login_method` | enum | 无 | 限制登录方式。 |
| `ghost_snapshot` | table | 无 | legacy 兼容。 |
| `hide_agent_reasoning` | bool | `false` | 隐藏 reasoning 事件。 |
| `history` | table | persistence 开 | `history.jsonl` 配置。 |
| `hooks` | table | 无 | lifecycle hooks。 |
| `include_apps_instructions` | bool | 无 | 注入 apps instructions。 |
| `include_collaboration_mode_instructions` | bool | 无 | 注入 collaboration mode instructions。 |
| `include_environment_context` | bool | 无 | 注入 environment context。 |
| `include_permissions_instructions` | bool | 无 | 注入 permissions instructions。 |
| `instructions` | string | 无 | 系统指令覆盖。 |
| `log_dir` | path | `$GREVO_HOME/log` | 日志目录。 |
| `marketplaces` | table map | 无 | marketplace 定义。 |
| `mcp_oauth_callback_port` | integer | ephemeral | OAuth callback 固定端口。 |
| `mcp_oauth_callback_url` | string | 无 | OAuth redirect URL。 |
| `mcp_oauth_credentials_store` | `auto` / `file` / `keyring` | `auto` | MCP OAuth 凭据存储。 |
| `mcp_servers` | table map | 无 | MCP server 定义。 |
| `memories` | table | 无 | memories 子系统配置。 |
| `model` | string | 无 | 模型名。 |
| `model_auto_compact_token_limit` | integer | 无 | 自动压缩阈值。 |
| `model_auto_compact_token_limit_scope` | `total` / `body_after_prefix` | 无 | 阈值计数范围。 |
| `model_catalog_json` | path | 无 | 本地模型 catalog。 |
| `model_context_window` | integer | 无 | 模型上下文窗口覆盖。 |
| `model_instructions_file` | path | 无 | 指令文件覆盖。 |
| `model_provider` | string | 无 | 选中的 provider ID。 |
| `model_providers` | table map | 无 | provider 定义。 |
| `model_reasoning_effort` | string | 无 | 默认 reasoning effort；可为自定义非空字符串。 |
| `model_reasoning_summary` | `auto` / `concise` / `detailed` / `none` | 无 | reasoning summary。 |
| `model_verbosity` | `low` / `medium` / `high` | 无 | Responses verbosity。 |
| `notice` | table | 无 | 产品内 notice。 |
| `notify` | string array | 无 | 通知外部命令。 |
| `openai_base_url` | string | 无 | 内置 OpenAI provider base URL。 |
| `orchestrator` | table | 无 | orchestrator 配置。 |
| `oss_provider` | string | 无 | 本地 OSS provider，如 `ollama`。 |
| `otel` | table | 无 | OTEL 配置。 |
| `permissions` | table | 无 | 权限 profile 定义。 |
| `personality` | `none` / `friendly` / `pragmatic` | 无 | 模型 personality。 |
| `plan_mode_reasoning_effort` | string | 无 | Plan mode effort。 |
| `plugins` | table map | 无 | 插件配置。 |
| `profile` | string | 无 | 当前 named profile。 |
| `profiles` | table map | 无 | named profiles。 |
| `project_doc_fallback_filenames` | string array | 内置顺序 | GREVO.md 缺失时的 fallback 文件名。 |
| `project_doc_max_bytes` | integer | 内置默认 | 项目文档最大字节数。 |
| `project_root_markers` | string array | `[".git"]` | 项目根识别 marker。 |
| `projects` | table map | 无 | 项目配置。 |
| `realtime` | table | 无 | realtime WS 版本/类型。 |
| `review_model` | string | 无 | `/review` 模型。 |
| `sandbox_mode` | `read-only` / `workspace-write` / `danger-full-access` | 无 | legacy sandbox mode。 |
| `sandbox_workspace_write` | table | 无 | legacy workspace-write 配置。 |
| `service_tier` | string | 无 | service tier 请求 ID。 |
| `shell_environment_policy` | table | 无 | shell 环境变量策略。 |
| `show_raw_agent_reasoning` | bool | `false` | 显示 raw reasoning。 |
| `skills` | table | 无 | 用户级 skill 配置。 |
| `sqlite_home` | path | `$CODEX_SQLITE_HOME` 或 `$GREVO_HOME` | SQLite 状态目录。 |
| `suppress_unstable_features_warning` | bool | `false` | 隐藏 unstable feature 警告。 |
| `tool_output_token_limit` | integer | 无 | 工具输出上下文预算。 |
| `tool_suggest` | table | 无 | 可建议安装工具配置。 |
| `tools` | table | 无 | 工具细配置。 |
| `tui` | table | 无 | TUI 配置。 |
| `web_search` | `disabled` / `cached` / `indexed` / `live` | `cached` | web search 模式。 |
| `windows` | table | 无 | Windows 特有配置。 |

## 常用枚举展开

### `approval_policy`

- `"untrusted"`：只自动放行已知安全的只读命令，其余询问。
- `"on-request"`：模型按需请求审批。
- `"never"`：不询问；失败直接返回模型。
- `{ granular = { ... } }`：细粒度控制哪些审批允许展示，哪些直接拒绝。

### `sandbox_mode`

- `"read-only"`
- `"workspace-write"`
- `"danger-full-access"`

新配置优先使用 `default_permissions` / `[permissions]`，`sandbox_mode` 是 legacy 兼容面。

### reasoning

`model_reasoning_effort` 常见值：

- `none`
- `minimal`
- `low`
- `medium`
- `high`
- `xhigh`
- `max`
- `ultra`

也允许其他非空字符串，由模型/服务端解释。

`model_reasoning_summary`：

- `auto`
- `concise`
- `detailed`
- `none`

### 凭据存储

CLI auth：

- `file`
- `keyring`
- `auto`
- `ephemeral`

MCP OAuth：

- `auto`
- `file`
- `keyring`

## 常用嵌套配置

### provider

```toml
[model_providers.my-provider]
name = "My Provider"
base_url = "https://example.com/v1"
wire_api = "responses"
```

常用字段包括 `env_key`、`http_headers`、`query_params`、`request_max_retries`、`stream_max_retries` 等。内置 provider ID 不能被覆盖。

### permissions profile

```toml
default_permissions = "my-workspace"

[permissions.my-workspace]
extends = ":workspace"

[permissions.my-workspace.filesystem.":workspace_roots"]
".git" = "write"
```

`:workspace_roots` 下的 key 是相对每个 workspace root 的 subpath。

### `[features]`

集中 feature flags，例如：

```toml
[features]
web_search = false
standalone_web_search = false
```

这里才是布尔开关。`features.web_search=false` 控制功能开关；顶层 `web_search="disabled"` 控制 web search 模式。两者同时存在时，feature gate 可能阻止工具出现。

### `[tools]`

当前 schema 下：

- `tools.experimental_request_user_input.enabled`
- `tools.update_plan.enabled`
- `tools.web_search` 为细化配置 table

示例：

```toml
[tools.update_plan]
enabled = false
```

## 配置层级

配置按层级合并，managed/requirements 层可约束或覆盖用户值。`profiles` 是一组覆盖；`profile` 选择启用哪个 profile。启动后读取的 catalog 类字段（如 `model_catalog_json`）不会因 per-thread config 覆盖而重新应用。
