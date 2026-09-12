## Why

产品化构建需要两条强制策略：强制启用沙箱（禁止关闭/降级到 full-access，否则视图隔离失效、间接读取可达成），以及禁止安装/加载插件（插件可注册 Hook、拿工具输入输出，扩大泄露面）。客户端不应有改配置/改模型配置的入口（D10），Skill 仅可配置启用/禁用。

## What Changes

- core：新增 `ensure_encrypted_skill_sandbox(engaged, sandbox_requested)` 判定，orchestrator 在 engaged 且无沙箱时拒绝工具执行。
- CLI：拒绝 `--sandbox danger-full-access` 与 `dangerously_bypass_approvals_and_sandbox`；拒绝 `plugin`/`marketplace` 子命令（I7）。
- app-server：`thread/start`、`turn/start` 请求携带 danger-full-access 沙箱策略时拒绝；插件安装/卸载与 marketplace RPC 拒绝（标记为最终阶段接线）。
- 决策：产品级强制点在外部边界（CLI/RPC）与 engaged 运行期断言，不在核心配置解析层全局禁止，避免破坏内部合法测试；D10 的“客户端不暴露配置入口”由产品前端约束 + 受信管理工具承接。

## Capabilities

### New Capabilities

- `agent-security-product-policy`: 强制沙箱（I6）、禁用插件（I7）、客户端配置面约束（D10）。

### Modified Capabilities

（无。）

## Impact

- `codex-rs/core`：`agent_security.rs`（helper）、`tools/orchestrator.rs`（engaged 无沙箱拒绝）。
- `codex-rs/cli`：`main.rs` 沙箱/插件禁令；`cli/tests` 相应用例更新。
- `codex-rs/app-server`：`thread_processor.rs`/`turn_processor.rs` 危险沙箱拒绝；插件/marketplace 处理器（最终阶段）。
- 测试：core helper、cli 用例、app-server 编译与单元检查。
