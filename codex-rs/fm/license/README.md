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

## 信号退出：立即归还 License

信号默认动作会直接终止进程，`LicenseGuard::drop` 不会执行，license 只能等服务器侧
租约超时才能回收。`fm-license` 提供 `install_checkin_signal_handler()`：注册
SIGINT/SIGTERM 后，收到信号先 `check_in_now()`（`lmCheckIn` + `lmExit`），再以
`128 + signal` 退出。

- TUI Ctrl-C（SIGINT）走同一个处理器，行为与之前一致。
- `codex app-server`（ACP 适配器、Python SDK、IDE 扩展实际拉起的进程）之前没有安装
  处理器，ACP 断开时 `codex-acp` 对进程发 SIGTERM，license 不会立即归还；现在
  `cli_main` 对**所有**持有 license 的子命令统一安装处理器，断开即归还。
- 独立 `codex-app-server` 二进制同样安装处理器。

## License Client 连接复用（研究结论）

LMClient Rust SDK 只是 C 库的封装，C 库保持进程级全局状态：`lmInit`/`lmExit` 管理
全局唯一连接并启动后台 heartbeat 线程，SDK 明确“同一时间只能初始化一个 client”。
因此：

- “一个 ACP 会话一个 License Client 连接”并不是 Codex 为每个会话建连接，而是
  `codex-acp` 为每个 ACP 连接 spawn 一个 `codex app-server` 进程，每个进程各自初始化
  一个 license client。
- 单个进程内无法创建多个 license client 实例（C 库全局唯一），所以没有“连接池”式的
  复用。
- 可复用方向是让一个 app-server 进程长期驻留、同时服务多个 ACP 连接/会话，进程内只
  初始化一次 license client；app-server 本身是单进程多连接模型，技术上可行。但当前
  `codex-acp` 在 stdin close 后约 2 秒 kill 进程（SIGTERM），要复用连接需要改适配器
  的生命周期（进程不随单个连接退出），而不是改 Codex 侧。
- 没有 LM Client/Server 源码；以上结论来自本地 Rust SDK 源码
  （`~/.cargo/git/checkouts/lmclient-rust-sdk-*`）中 `LicenseClient` 的注释与
  `lmInit`/`lmExit` 的全局状态说明。

## 后续方向（记录，暂不实施）：后台 lm client daemon

可选架构：独立 daemon 进程持有唯一 `LicenseClient`（一条到 FMSH Server 的连接 + 一个
heartbeat），所有 Codex 进程 / ACP 会话通过 Unix socket JSON-RPC 与 daemon 通信，
调用 `acquire` / `release` / `keepalive`；acquire 映射一次 `check_out_incr`，
release 映射一次 `check_in`，服务器侧计数不变；租约带 TTL，消费进程被
SIGKILL/断开时由 TTL 兜底回收，从结构上解决“进程被杀不归还 license”的问题。

可行性依据（SDK `client.rs`）：同一 client 可反复调用 `check_out_incr` / `check_in`
（按 feature 增量/减量，调用不返回会话 id），因此单进程多 checkout 理论上可行，但
必须用真实 FMSH_LIC_SERVER 实测确认：

- 同一 feature 多次 checkout 后，C 库 heartbeat 是否覆盖全部 checkout；
  `check_in(feature)` 是否能正确逐次减量。
- 服务器端是否限制单连接的并发 checkout 数量；daemon 崩溃后服务器侧的
  释放/回收语义（决定 TTL 与故障恢复设计）。

当前决定：仅记录，不实施；保留现有“每进程直连 + SIGINT/SIGTERM 归还”方案。

## 环境变量开关（所有构建模式）

`fm-license` 提供两个**所有构建模式（debug 与 release）都生效**的环境变量：

- `FMSH_CODEX_LIC_TEST_BYPASS=1`：产品级跳过开关，设置后跳过真实校验、进程正常启动。
- `FMSH_CODEX_LIC_TEST_FORCE_LOST=1`（需与 BYPASS 同时设置）：进程以 `Lost` 状态
  启动，用于验证请求门禁。

仓库测试 harness（`app_test_support`、`mcp_test_support`、`test_codex_exec`、
`cli_stream` 等）默认注入 BYPASS；release 构建也保留同一开关。

## 主要改动文件

- `codex-rs/fm/license/src/license.rs`、`lib.rs`、`license_tests.rs`：状态机、门禁 API、
  全构建模式 bypass、SIGINT/SIGTERM 归还。
- `codex-rs/cli/src/main.rs`、`remote_control_cmd.rs`：全入口启动校验与统一信号处理器。
- `codex-rs/app-server/src/main.rs`、`message_processor.rs`、`error_code.rs`：
  独立二进制校验、信号处理器与 app-server 请求门禁。
- `codex-rs/mcp-server/src/main.rs`、`codex_tool_runner.rs`：独立二进制校验与工具门禁。
- `codex-rs/exec/src/main.rs`：独立 `codex-exec` 校验。
- 各测试 harness：注入全构建模式生效的 bypass。

## 验证

```bash
just test -p fm-license
just test -p codex-app-server license_lost_blocks_thread_start
just test -p codex-mcp-server test_license_lost_blocks_codex_tool_call
just test -p codex-exec test_apply_patch_tool
just test -p codex-cli app_server_emits_json_info_events
```
