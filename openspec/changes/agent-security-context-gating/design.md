## Context

变更 `encrypted-skill-engagement` 已提供 `is_engaged`/in-flight/`path_mappings`。当前 `before_tool` 与 `RedactingToolOutput` 无条件执行明文/路径检查与红act，普通会话被 `/dev/shm` 字符串策略影响；后续 RPC 面需要按 thread_id 拿到会话上下文。本变更引入统一的 `AgentSecurityContext` 并按 engaged 门控现有两条主要护栏。

## Goals / Non-Goals

**Goals:**

- 提供 `AgentSecurityContext`（runtime + session_id + live `engaged()`）。
- `TurnContext` 携带每 turn 的 `Option<AgentSecurityContext>`。
- `before_tool` 与 `RedactingToolOutput` 未 engaged 时完全透传；engaged 时行为不变。

**Non-Goals:**

- 不改动 engaged 时的既有规则（路径集合、明文阈值、重写行为等）。
- 不引入进程级 runtime 共享（RPC 面变更再做）。
- 不做 reasoning/中间产物路径红act扩展（后续变更）。

## Decisions

### 1. `AgentSecurityContext` 放 core 新模块，`engaged()` 实时派生

`agent_security.rs` 定义 `AgentSecurityContext`，内部持有 `Arc<EncryptedSkillRuntime>` 与 `session_id`。`engaged()` 直接调 `runtime.is_engaged()`，避免快照失同步。`None` 表示本 turn 无护栏需求。

替代方案：把上下文放 fm crate——fm crate 不应依赖 session 概念；放 core 最贴近使用方。否决前者。

### 2. `before_tool` 在入口统一门控

`before_tool` 开头：`if !runtime.is_engaged(session_id) { return Allow; }`。这同时覆盖 shell/read/export 三个分支，避免逐个分支加条件漏改。

### 3. `RedactingToolOutput` 在构造时记录 `engaged`，未 engaged 直接透传

`redact_text` 开头 `if !self.engaged { return text.to_string(); }`。构造时用 `runtime.is_engaged(session_id)` 计算；即便 turn 中途加载 skill，live 判定由 `before_tool` 承担，输出层只服务“构造时已 engaged”的输出（工具执行期间 skill 状态不会在单次调用内改变）。

### 4. `TurnContext.agent_security` 由构造器组装

在 `TurnContext::new` 里从传入的 session services 组装 `Option<AgentSecurityContext>`；guard 判定仍以 live 状态为准，快照只用于传递与展示。

## Risks / Trade-offs

- [输出层快照与 live 状态不一致] → 单次工具调用内 skill 加载/卸载不会发生；跨调用由 `before_tool` live 判定兜底。
- [TurnContext 构造点增加参数] → 构造点有限（主 turn、review、测试），一次性改动。

## Migration Plan

纯门控：未 engaged 会话恢复“无加密技能”原行为；engaged 会话行为不变。可随时回滚（移除门控）。

## Open Questions

无阻塞项。
