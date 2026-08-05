## Why

ACP/SDK 前端可直连 app-server 的 JSON-RPC，绕过 Agent Loop 的 `before_tool` 与 `RedactingToolOutput`：`fs/*`、`command/exec`、`thread/shellCommand`、`process/spawn`、`thread/inject_items`、`thread/name|goal|metadata` 都可能让前端读到解密明文/路径或执行命令；同时 telemetry 在红act包装前记录工具输出原始预览，存在日志泄露。

## What Changes

- `fm-encrypted-skills` 新增进程级 runtime 注册表（Weak 引用）：`any_engaged()` 返回是否有任一注册 runtime engaged；`engaged_guarded_paths()` 返回 mem-root 与所有 engaged 会话解密目录，供无 thread 上下文的 RPC 面做路径检查。
- core 工具分发修复 telemetry 预览：工具结果在计算 `log_preview()` 之前先经 engaged 红act，日志不再出现明文/路径。
- app-server RPC 面接入检查：fs 读/枚举/watch、fs 写/复制/删除、`command/exec`、`thread/shellCommand`、`process/spawn`、`thread/inject_items`、`thread/name|goal|metadata` 在进程级 engaged 时按规则 Block（具体规则见 spec）。
- 决策：`fs/*`、`command/exec` 等请求不携带 thread_id，采用“进程内任一会话 engaged 即启用 RPC 检查”的保守策略；未 engaged 进程零行为变化（满足 I3）。

## Capabilities

### New Capabilities

- `agent-security-rpc-guard`: 进程级 engaged 注册表、RPC 面路径/执行/注入/元数据检查、telemetry 预览红act。

### Modified Capabilities

（无。）

## Impact

- `codex-rs/fm/encrypted-skills`：runtime 注册表（`runtime.rs`/`lib.rs`）。
- `codex-rs/core`：`tools/registry.rs`（telemetry 预览红act）、新增 RPC 检查纯函数（可单测）。
- `codex-rs/app-server`：message_processor 与各 request_processor 接入检查。
- 测试：fm 单测、core 单测、app-server 集成测试。
