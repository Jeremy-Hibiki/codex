# fm-license 实现文档

本文档说明 `fm-license` 在产品化二次开发中的授权模型、全入口校验范围和心跳耗尽时的请求门禁行为。

## 授权粒度

`fm-license` 按**进程**授权，不按会话/线程授权：

- `verify_at_startup()` 在进程启动时向 FMSH LicenseService `check_out_incr` 一次，返回的
  `LicenseGuard` 持有到进程结束，`Drop` 或信号处理时 `check_in`。
- 一个 `codex app-server` 进程可以承载多个会话（thread），这些会话共享同一份 checkout；
  多开进程才会占用多份 license。
- 只有 Linux x86_64 glibc 启用真实校验，其它平台编译为 no-op stub（FMSH SDK 只提供
  CentOS 7 / x86_64 静态库）。

## 全入口校验

所有能启动 Codex agent 工作的入口都在启动时调用 `verify_at_startup()`，校验失败则进程直接退出：

| 入口 | 调用位置 |
|---|---|
| `codex`（交互 TUI） | `cli_main` 无子命令分支 |
| `codex exec` | `cli_main` `Exec` 分支 |
| `codex review` | `cli_main` `Review` 分支 |
| `codex resume` / `codex fork` | `cli_main` `Resume` / `Fork` 分支 |
| `codex app-server`（含 ACP 适配器、Python SDK、IDE 扩展） | `cli_main` `AppServer` 且无子命令 |
| `codex mcp-server`（Agents SDK 指南的接入方式） | `cli_main` `McpServer` 分支 |
| `codex remote-control`（前台运行 app-server） | `cli_main` `RemoteControl` 且无子命令 |
| `codex-exec`（独立二进制） | `codex-rs/exec/src/main.rs` |
| `codex-app-server`（独立二进制） | `codex-rs/app-server/src/main.rs` |
| `codex-mcp-server`（独立二进制） | `codex-rs/mcp-server/src/main.rs` |

管理类命令（`login`、`mcp`、`plugin`、`doctor`、`app-server daemon start/stop`、
`remote-control stop/pair`、schema 生成等）不启动 agent，不做校验。daemon 实际拉起
的 app-server 子进程仍会走 `codex app-server` 校验。

`codex cloud`、`exec-server`、`responses-api-proxy` 不启动本地 agent（cloud 任务在
OpenAI 云端执行），因此不占用 FMSH license，也不做校验。

## 心跳耗尽：拒绝新请求，不杀进程

旧实现中心跳重试耗尽会回调 `on_license_lost` 并 `process::exit(2)`，整个进程连同所有会话
一起退出。新实现改为进程级状态机：

```
Active ──心跳重试耗尽──> Lost ──心跳恢复(on_retry_success)──> Active
```

- `on_license_lost`：状态置为 `Lost`，记录 error，**不再退出进程**；已运行的会话继续运行。
- `on_retry_success`：状态恢复为 `Active`，新请求重新放行。
- `is_active()` / `ensure_active()`：请求门禁 API。

### 请求门禁点

license 为 `Lost` 时，以下“发起新工作”的请求会被拒绝，返回统一错误：

- app-server：`thread/start`、`thread/resume`、`thread/fork`、`turn/start`、
  `turn/steer`、`thread/inject_items`、`thread/realtime/start`、`review/start`
  （JSON-RPC 错误码 `-32002`）。
- MCP server：`codex` / `codex-reply` 工具调用返回 tool error。

错误消息统一为：

```
Codex license is unavailable; new requests are blocked until the license recovers
```

其它请求（读配置、列 thread、终止 turn、文件操作等）不受影响。

## 测试注入（仅 debug 构建）

仓库测试套件不依赖真实 FMSH 服务，`fm-license` 提供两个**仅在 `debug_assertions`
构建下生效**的环境变量；release 构建完全编译掉，没有逃生舱：

- `FMSH_CODEX_LIC_TEST_BYPASS=1`：跳过真实校验，进程正常启动。
- `FMSH_CODEX_LIC_TEST_FORCE_LOST=1`（需与 BYPASS 同时设置）：进程以
  `Lost` 状态启动，用于验证请求门禁。

拉起受控二进制的测试 harness（`app_test_support`、`mcp_test_support`、
`test_codex_exec`、`cli_stream` 等）默认注入 BYPASS。

## 主要改动文件

- `codex-rs/fm/license/src/license.rs`、`lib.rs`、`license_tests.rs`：状态机与门禁 API。
- `codex-rs/cli/src/main.rs`、`remote_control_cmd.rs`：全入口启动校验。
- `codex-rs/app-server/src/main.rs`、`message_processor.rs`、`error_code.rs`：
  独立二进制校验与 app-server 请求门禁。
- `codex-rs/mcp-server/src/main.rs`、`codex_tool_runner.rs`：独立二进制校验与工具门禁。
- `codex-rs/exec/src/main.rs`：独立 `codex-exec` 校验。
- 各测试 harness：注入 debug-only bypass。

## 验证

```bash
just test -p fm-license
just test -p codex-app-server license_lost_blocks_thread_start
just test -p codex-mcp-server test_license_lost_blocks_codex_tool_call
just test -p codex-exec test_apply_patch_tool
just test -p codex-cli app_server_emits_json_info_events
```
