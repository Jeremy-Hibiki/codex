## Why

`is_engaged` 判定已就绪（变更 `encrypted-skill-engagement`），但现有护栏仍然无条件启用：普通会话的命令只要包含 `/dev/shm` 就会被 shell guard 拦截、输出就会被红act，违反“未 engaged 零行为变化”的契约；同时缺少统一的 `AgentSecurityContext`，无法支撑后续的 per-turn 翻转与 RPC 面接入。

## What Changes

- core 新增 `agent_security.rs`：定义 `AgentSecurityContext { runtime: Arc<EncryptedSkillRuntime>, session_id: String }`，`engaged()` 实时调用 `runtime.is_engaged(session_id)`；`None` 表示该 turn 不需要护栏。
- `TurnContext` 增加 `agent_security: Option<AgentSecurityContext>`，在 turn 构造时从 session 组装；guard 判定仍以 live 状态为准（快照仅用于传递上下文）。
- `before_tool` 门控：未 engaged 时所有工具直接 `Allow`（不做明文/路径检查、不做重写）；engaged 时保持既有全部规则。
- `RedactingToolOutput` 门控：未 engaged 时输出原样透传（不 unrewrite、不红act）；engaged 时保持既有 unrewrite + 路径红act + 明文红act。
- 行为契约：未 engaged 零行为变化；engaged 行为与本变更前完全一致（本轮不改动规则本身）。

## Capabilities

### New Capabilities

- `agent-security-context-gating`: 会话/回合级 `AgentSecurityContext`、`before_tool` 与工具输出红act的 engaged 门控、未 engaged 零行为变化。

### Modified Capabilities

（无。）

## Impact

- `codex-rs/core`：新增 `agent_security.rs`；`session/turn_context.rs`（字段与构造）；`encrypted_skills_guard.rs`（门控）；`tools/registry.rs`（`RedactingToolOutput` 构造时计算 engaged）。
- 测试：`codex-core` 单测（`agent_security_tests.rs`、`encrypted_skills_guard_tests.rs`）。
- 无新依赖；`fm-encrypted-skills` 无改动。
