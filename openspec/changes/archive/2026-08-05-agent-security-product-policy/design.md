## Context

强制策略是产品化边界（CLI/RPC/运行期断言），不在核心配置解析层全局禁止 danger-full-access，否则破坏仓库内部大量合法测试（审批、沙箱测试等）。I6 的“engaged 时最终校验”由 orchestrator 承担；I7 的插件禁令先在 CLI 入口落实，app-server 插件 RPC 与 TUI 入口在最终阶段补全。

## Goals / Non-Goals

**Goals:**

- engaged + 无沙箱 → 拒绝执行。
- CLI 拒绝 full-access 与插件管理。
- app-server 拒绝 danger-full-access 的 thread/turn 启动。

**Non-Goals:**

- 不全局禁止核心库使用 danger-full-access（内部测试与合法 API 保留）。
- 不实现受信管理工具（D10 后续）。

## Decisions

### 1. engaged 运行期断言放在 orchestrator

`ensure_encrypted_skill_sandbox(engaged, sandbox_requested)` 为纯函数；orchestrator 在首次 attempt 前调用，未达标返回 `ToolError::Rejected`。

### 2. 强制点收敛在外部边界

CLI（`--sandbox`/bypass/plugin）与 app-server 请求参数校验，避免改动核心解析逻辑；产品前端不暴露配置入口（D10）。

测试侧记录：app-server 既有依赖请求级 danger-full-access 的用例（zsh fork、plugin attribution、
process-id、elevated override）改为配置级 full-access 或 WorkspaceWrite，保持原测试意图；
新增 `product_policy` 集成测试覆盖 thread/turn start 拒绝。

## Risks / Trade-offs

- [CLI 插件用例需更新] → 仓库内 cli 插件/marketplace 测试改为断言禁令。
- [app-server 插件 RPC 未在本变更落地] → 标记为最终阶段，产品前端不暴露该入口。

## Migration Plan

可回退：移除边界检查即恢复。

## Open Questions

无阻塞项。
