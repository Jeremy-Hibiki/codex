# Codex 插件系统分析报告

> 生成日期：2026-07-30
> 项目：openai/codex
> 范围：插件系统、Hook 引擎、上下文注入、工具调用事件

---

## 一、架构分层

Codex 的插件/Hook 系统分为 **三层**：

| 层              | crate                | 职责                                                                                                     |
| --------------- | -------------------- | -------------------------------------------------------------------------------------------------------- |
| **类型层**      | `codex-plugin`       | 共享结构体：`PluginId`、`PluginManifest<R>`、`PluginProvider` trait、`LoadedPlugin`、`PluginLoadOutcome` |
| **管理器层**    | `codex-core-plugins` | `PluginsManager` 编排加载，处理 marketplace 解析 (local/git/npm/remote)、bundle 解包、store 持久化       |
| **Hook 引擎层** | `codex-hooks`        | `ClaudeHooksEngine` 持有 `Vec<ConfiguredHandler>`，通过 **preview → run** 模式分发到 11 个事件模块       |

### 关键源码路径

```
codex-rs/
├── plugin/src/                          # Plugin 类型定义
│   ├── lib.rs                           # 导出: PluginId, PluginManifest, PluginProvider
│   ├── manifest.rs                      # PluginManifest<R> 泛型结构
│   ├── provider.rs                      # PluginProvider trait, ResolvedPlugin
│   ├── plugin_id.rs                     # PluginId (name@marketplace)
│   └── load_outcome.rs                  # PluginLoadOutcome, LoadedPlugin
├── core-plugins/src/                    # Plugin 管理器
│   ├── lib.rs                           # 插件入口
│   ├── manager.rs                       # PluginsManager (加载/卸载)
│   ├── marketplace.rs                   # Marketplace 解析
│   ├── discoverable.rs                  # 可发现插件
│   ├── loader.rs                        # Plugin 加载器
│   └── provider.rs                      # 各来源 provider
├── hooks/src/                           # Hook 引擎 ★核心
│   ├── lib.rs                           # 11 个事件常量
│   ├── types.rs                         # Hook, HookResult, HookEvent
│   ├── registry.rs                      # Hooks 注册与分发
│   ├── engine/
│   │   ├── discovery.rs                 # 发现 + 信任验证 ★核心
│   │   ├── mod.rs                       # ConfiguredHandler, ClaudeHooksEngine
│   │   ├── dispatcher.rs                # 并发执行调度
│   │   ├── command_runner.rs            # Shell 进程管理
│   │   ├── output_parser.rs             # JSON 输出解析
│   │   └── events/                      # 11 个事件处理
│   ├── config_rules.rs                  # 配置层合并
│   └── declarations.rs                  # PluginHookDeclaration
├── context-fragments/src/               # Context 注入
│   ├── lib.rs                           # 导出
│   ├── fragment.rs                      # ContextualUserFragment trait ★核心
│   └── additional_context.rs            # 附加上下文
├── analytics/src/                       # 事件追踪
│   ├── events.rs                        # TrackEventRequest 枚举
│   └── reducer.rs                       # 事件生成逻辑
├── protocol/src/                        # 协议层
│   ├── protocol.rs                      # EventMsg, RawResponseItem
│   ├── mcp.rs                           # McpInvocation
│   ├── dynamic_tools.rs                 # DynamicToolCallRequest/Response
│   ├── items.rs                         # ThreadItem 枚举
│   └── legacy_events.rs                 # 兼容层
└── app-server-protocol/src/protocol/v2/ # v2 API 定义
    ├── item.rs                          # ThreadItem types
    ├── mcp.rs                           # McpServerToolCallParams
    └── shared.rs                        # DynamicToolCall, CollabAgentToolCall
```

---

## 二、11 种 Hook 事件类型

| #   | 事件名                   | 触发时机             | 关键能力                                                       |
| --- | ------------------------ | -------------------- | -------------------------------------------------------------- |
| 1   | **`PreToolUse`**         | 每个工具调用之前     | **最核心**：读取工具名/参数，决定是否允许，可 `should_block`   |
| 2   | **`PermissionRequest`**  | 请求权限时           | 拦截权限审批流程，附加 `additional_context`                    |
| 3   | **`PostToolUse`**        | 工具执行完成后       | 读取工具输出，可追加 `additional_context`                      |
| 4   | **`PreCompact`**         | 上下文压缩前         | 注入压缩前的上下文信息                                         |
| 5   | **`PostCompact`**        | 上下文压缩后         | 处理压缩后的上下文                                             |
| 6   | **`SessionStart`**       | 会话开始时           | 注入 `system_message`、`additional_context`，可 `should_block` |
| 7   | **`SessionEnd`**         | 会话结束时           | 清理操作                                                       |
| 8   | **`UserPromptSubmit`**   | 用户提交 prompt 后   | 拦截用户输入，修改 `additional_context`                        |
| 9   | **`SubagentStart`**      | 子 Agent 启动时      | 拦截子 Agent 创建                                              |
| 10  | **`SubagentStop`**       | 子 Agent 停止时      | 处理子 Agent 结束                                              |
| 11  | **`Stop`**               | Agent 决定停止时     | 读取 stop 原因，追加上下文                                     |

> **源码验证**：HOOK_EVENT_NAMES 常量定义于 `codex-rs/hooks/src/lib.rs:20-31`

---

## 三、Hook 处理流程

每个 Hook handler 是一个 **Shell 命令**：

```json
// stdin: HookPayload
{
  "event_name": "pre_tool_use",
  "tool_name": "execute_bash",
  "tool_input": {"command": "ls -la"},
  "tool_use_id": "...",
  "session_id": "...",
  "cwd": "/workspace"
}

// stdout: HookResult
{
  "should_block": false,
  "additional_context": [...],
  "updated_input": {...},    // pre_tool_use: 重写工具参数
  "system_message": "...",   // UI 警告信息
  "feedback_message": "..."  // post_tool_use: 反馈消息
}
```

**执行模型**：

- 同一事件的所有 handlers 通过 `FuturesUnordered` **并发执行**
- `command_runner.rs` 派生 shell 进程，stdin 管道传入 JSON，捕获 stdout/stderr
- 强制 timeout 保护
- 退出码 2 表示拒绝（fallback）

---

## 四、Plugin 信任机制

`HookTrustStatus` 四种状态：Managed / Trusted / Modified / Untrusted

信任验证流程：

1. `discover_handlers()` 扫描所有 plugin source 的 `hooks/hooks.json`
2. `hook_trust_status()` 读取 `hooks.state.<key>.trusted_hash` 并与文件 SHA256 对比
3. 信任后写入 `hooks.state` 配置层持久化
4. `hook_enabled()` 最终决定是否启用

---

## 五、Marketplace 系统

Plugin 来源 (通过 `PluginProvider` trait 抽象)：

| 来源                | 描述                                     |
| ------------------- | ---------------------------------------- |
| **Environment**     | 环境变量指向的本地文件系统路径           |
| **Local**           | 工作区内的本地目录 (`~/.codex/plugins/`) |
| **Git**             | Git 仓库中的插件                         |
| **Npm**             | npm 包中的插件                           |
| **Remote**          | 远程目录 (ChatGPT 插件后端)              |
| **WorkspaceListed** | 工作区目录下的 `hooks/` 目录，自动发现   |

---

## 六、ContextualUserFragment — 模型上下文注入系统

Trait 定义 (`codex-rs/context-fragments/src/fragment.rs:46-93`)：

```rust
pub trait ContextualUserFragment {
    fn role(&self) -> &'static str;

    fn requires_separate_message(&self) -> bool {
        false
    }

    fn markers(&self) -> (&'static str, &'static str);

    fn body(&self) -> String;

    fn type_markers() -> (&'static str, &'static str)
    where
        Self: Sized;

    fn matches_text(text: &str) -> bool
    where
        Self: Sized;

    fn render(&self) -> String {
        let (start_marker, end_marker) = self.markers();
        let body = self.body();
        if start_marker.is_empty() && end_marker.is_empty() {
            return body;
        }
        format!("{start_marker}{body}{end_marker}")
    }

    fn into(self) -> ResponseItem
    where
        Self: Sized;
}
```

**关键实现**：UserInstructions、SkillInstructions、EnvironmentsState、HookAdditionalContext、TurnAborted、SubAgentNotification、WorldState 等，受 token 预算约束。

---

## 七、工具调用事件系统

三种 `TrackEventRequest` 变体：

| 事件类型                  | ThreadItem 类型                   | 触发位置              |
| ------------------------- | --------------------------------- | --------------------- |
| **`McpToolCall`**         | `ThreadItem::McpToolCall`         | `core/src/reducer.rs` |
| **`DynamicToolCall`**     | `ThreadItem::DynamicToolCall`     | `core/src/reducer.rs` |
| **`CollabAgentToolCall`** | `ThreadItem::CollabAgentToolCall` | `core/src/reducer.rs` |

共享基础字段 `CodexToolItemEventBase`：`thread_id`, `session_id`, `turn_id`, `item_id`, `product_client_id`。

---

## 八、Plugin Manifest 格式

```json
// .codex-plugin/plugin.json
{
  "name": "demo",
  "version": "1.0.0",
  "marketplace": "test-marketplace",
  "hooks": ["hooks/hooks.json"],
  "skills": ["skills/*.md"],
  "mcp_servers": ["mcp_servers/"],
  "apps": ["apps/"]
}
```

---

## 九、Hook Payload 与 Result 结构

### PreToolUse Payload

```rust
PreToolUseCommandInput {
    session_id: String,
    turn_id: String,
    agent_id: Option<String>,
    tool_name: String,
    tool_input: serde_json::Value,
    tool_use_id: String,
    cwd: String,
    model: String,
    permission_mode: String,
}
```

### PreToolUse Result

```rust
PreToolUseOutcome {
    should_block: bool,
    block_reason: Option<String>,
    additional_contexts: Vec<String>,
    updated_input: Option<serde_json::Value>,
}
```

PostToolUse、SessionStart 结构类似，各有特定字段。
