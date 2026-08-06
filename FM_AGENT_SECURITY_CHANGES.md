# Agent Security 变更明细（对比 `rust-v0.146.0`）

> 口径：`git diff rust-v0.146.0..HEAD`。本分支从 TAG 出发后的全部提交都属本特性，
> 分两个阶段：
>
> - **第一阶段**（`rust-v0.146.0` → `5f221ba59b`，43 个提交）：加密 Skill 基础实现
>   （crate、解密、token 化、execute-only 访问控制）、fm-license 启动校验、
>   fmsh-ukey/local 信封后端、I1~I23 修复。逐条记录见 `FM_AGENT_SECURITY_FIX_LOG.md`。
> - **第二阶段**（`5f221ba59b` → HEAD，本次会话）：engaged 门控、bwrap 只读 bind、
>   RPC guard、产品策略（强制沙箱/插件禁用）、TODO-6/7/8/9 收尾、swap 分析。
>
> 图例：**★**=第二阶段新增/修改；**◇**=两个阶段都改过；无标记=仅第一阶段。

## 汇总数字（排除 `*.md`、`openspec/**`）

| 范围 | 文件数 | 新增 | 删除 |
|---|---:|---:|---:|
| TAG..HEAD 全部 | 189 | +13,621 | -612 |
| 第一阶段（TAG..分支起点） | 134 | +10,783 | -594 |
| 第二阶段（分支起点..HEAD） | 73 | +2,893 | -73 |

---

# 第一阶段（TAG → 5f221ba59b）

## 构建与发布（根目录）

- `.bazelrc`、`BUILD.bazel`、`defs.bzl`、`MODULE.bazel`、`MODULE.bazel.lock`：
  fm-license 与 fmsh-ukey 的 Bazel 依赖、crate 注解（libssh2 等）。
- `patches/rules_rs_dedupe_build_script_deps.patch`、
  `patches/rules_rs_rust_archive_url.patch`、`patches/v8_module_deps.patch`：
  构建修复/版本锁定。
- `third_party/lmclient/BUILD.bazel`、`third_party/lmclient/additive.BUILD.bazel`：
  FMSH LMCLIENT SDK 的 Bazel 封装（CentOS 7 / x86_64 静态库）。
- `release/Dockerfile`、`release/.dockerignore`、`release/predownload-deps.sh`、
  `release/BUILD.md`：发布容器（license 依赖、SDK 预下载）。
- `scripts/format.py`、`.gitignore`、`.dockerignore`：工具/忽略项。
- `codex-rs/Cargo.toml`、`codex-rs/Cargo.lock` 及
  `cli`/`app-server`/`core`/`core-plugins`/`core-skills`/`exec`/`mcp-server`/
  `skills`/`thread-manager-sample` 的 `Cargo.toml`：新增 `fm-license`、
  `fm-encrypted-skills`、`fmsh-ukey-cipher`（git 依赖）、`zip` 等依赖。

## `codex-rs/fm/license`（新增 crate）

- `src/lib.rs`：LicenseManager 公共 API、`verify_at_startup`、`ensure_active`、
  测试 bypass 环境变量。
- `src/license.rs`：LMCLIENT 启动校验、LicenseGuard、信号处理器注册、心跳。
- `src/license_tests.rs`：测试。
- `Cargo.toml`、`BUILD.bazel`：依赖与 Bazel 目标。
- `README.md`：使用说明。

## `codex-rs/fm/encrypted-skills`

- `src/lib.rs`：crate 根，模块导出。
- `src/sdk.rs`：`EnvelopeSdk` trait、`TestZipSdk`、`UnavailableSdk`，容量上限
  （512 条目 / 16 MiB）；第二阶段仅 clippy 微调。
- `src/cache.rs`：内容缓存（上限/FIFO/引用回调，I6）。
- `src/registry.rs`：会话注册表、替换记录返回与 wipe（I2/F1/F3/F5）。
- `src/rehydrate.rs`：token → 明文重水合（framing 版本，I12）。
- `src/token.rs`：哨兵 token 序列化/解析、格式校验（I10）。
- `src/mem_root.rs`：`/dev/shm` 初始化、0700 权限、容量门控、`secure_wipe`。
- `src/paths.rs`：路径改写/命令分段/受保护目录判定（I1/I14）。
- `src/audit.rs`：JSONL 审计（timestamp/转义/0600/轮转）、锁中毒记录、进程级
  共享 sink（F4）。
- `src/export_guard.rs`：明文片段匹配/规范化红act（I17~I19）；第二阶段 clippy。
- `src/runtime.rs`：解密、目录落盘、注册、TTL、sweep、`unload_turn`、
  `is_engaged`/in-flight、`path_mappings`、unrewrite/redact；第二阶段加入进程级
  `new_shared_with_audit`（Weak 注册表）。
- `*_tests.rs`：各模块单测（含并发/共享 sink/生命周期用例）。
- `Cargo.toml`、`BUILD.bazel`：依赖（含 fmsh-ukey 可选 feature）。

## `codex-rs/fm/ukey`（新增，vendored）

- `fmsh-ukey-wrapper/`：FFI 封装（error/ffi/lib、build.rs、sm2 互操作测试）。
- `fmsh-ukey-cipher/`：本地 X25519/AES-GCM 与 UKey 两阶段密文实现。

## `codex-rs/config`

- `src/config_toml.rs`：`EncryptedSkillsToml`（`sdk`/`skill_idle_ttl_secs`/
  `audit_path`/`local_privkey`）与解析测试。

## `codex-rs/core-skills`

- `src/model.rs`、`src/loader.rs`、`src/loader/environment.rs`：frontmatter 加密标记
  （`metadata.encrypted` / `encryption`）、加密字段解析。
- `src/injection.rs`：`build_skill_injections` 加密分支（解密/token/幂等）、
  `encryption.package` 优先（I8）。
- `src/skill_instructions.rs`：加密 Skill body 输出哨兵 token。
- `src/render.rs`、`src/service_tests.rs`、`src/invocation_utils_tests.rs`、
  `tests/environment_loader.rs` 及各 `*_tests.rs`：配套测试。
- `Cargo.toml`：新增 `fm-encrypted-skills` 依赖。

## `codex-rs/core`

- `src/encrypted_skills_guard.rs`：第一版工具拦截（shell 路径改写/execute-only、
  导出拦截、回复红act、guardian 红act）；第二阶段大幅扩展（见下）。
- `src/client_common.rs`：`Prompt` 重水合器接线、`EncryptedSkillRehydrator`。
- `src/session/mod.rs`、`src/session/session.rs`、`src/session/turn.rs`：
  runtime 创建、turn 注入、重水合、TTL sweep、事件红act。
- `src/encrypted_skills_periodic.rs`：周期 sweep（Weak 防泄漏，I3）。
- `src/agent/control/spawn.rs`、`src/agent/control.rs`：fork 时剥离 token（I7）。
- `src/compact.rs`、`src/compact_remote.rs`、`src/compact_remote_v2_attempt.rs`、
  `src/compact_remote_request.rs`：压缩历史红act（I16）。
- `src/hook_runtime.rs`：hook 附加上下文红act（I15）。
- `src/guardian/review.rs`：评审请求红act（I23）。
- `src/stream_events_utils.rs`：回复项红act。
- `src/codex_thread.rs`：`clear_encrypted_skills`。
- `src/tools/handlers/view_image.rs`、`src/tools/hook_names.rs`：工具拦截接线。
- `src/config/mod.rs`：`Config` 加密技能字段与加载。
- `src/session_startup_prewarm.rs`、`src/prompt_debug.rs`、`src/state/service.rs`：
  启动预热/调试/状态面适配。
- `src/lib.rs`：模块导出；第二阶段加 `agent_security`。
- 各 `*_tests.rs` 与 `tests/suite/encrypted_skills.rs`、`tests/suite/mod.rs`：
  单测与集成测试（第一版）。

## `codex-rs/app-server-protocol`

- `src/protocol/v2/plugin.rs`：`SkillMetadata` 加密字段（`encrypted`/`encryption`）。
- `schema/json/*`、`schema/typescript/v2/*`：schema 与 TS 定义重新生成。

## `codex-rs/app-server`

- `src/request_processors/catalog_processor.rs`：skills 列表携带加密元数据。
- `src/request_processors/thread_delete.rs`：清理关联加密状态。
- `src/error_code.rs`、`src/main.rs`、`src/message_processor.rs`：fm-license
  启动校验与请求边界 gate（第一阶段）。
- 测试：`tests/suite/strict_config.rs`、`tests/suite/v2/license_gate.rs`、
  `tests/suite/v2/connection_handling_websocket.rs`、`tests/common/test_app_server.rs`。

## `codex-rs/cli`

- `src/main.rs`：fm-license 启动校验（第一阶段）；第二阶段叠加产品策略（见下）。
- `src/remote_control_cmd.rs`：license gate。
- `tests/app_server.rs`：license bypass 测试环境变量。

## 其他 crate（license 接线）

- `codex-rs/exec/src/main.rs`、`codex-rs/mcp-server/src/*`、
  `codex-rs/mcp-server/tests/*`、`codex-rs/skills/src/*`、
  `codex-rs/tui/src/bottom_pane/*`、`codex-rs/tui/src/chatwidget/tests/*`：
  Codex 各入口 license 校验与提示。
- `codex-rs/thread-manager-sample/src/main.rs`：示例配置初始化；第二阶段补
  `encrypted_skills_local_privkey`（clippy）。

## 文档与 OpenSpec

- 根文档 `FM_AGENT_SECURITY_DESIGN.md`、`FM_AGENT_SECURITY_FIX_LOG.md`、
  `FM_AGENT_SECURITY_FIX_PLAN.md`（第一阶段创建；第二阶段删除计划文件）。
- `openspec/changes/encrypt-agent-security-skills/`：早期 OpenSpec 变更。
- `openspec/config.yaml`：OpenSpec 配置。

---

# 第二阶段（5f221ba59b → HEAD）

## `codex-rs/fm/encrypted-skills`

- `src/audit.rs` ★：`FileAuditSink` 锁中毒时 `tracing::error!`；`shared_file_sink`
  进程级 `Weak` 注册表，同一文件只保留一个 writer（F4 收敛）。
- `src/registry.rs` ★：生命周期锁与注册表一致性（F1/F3/F5），替换记录返回并
  wipe 旧目录。
- `src/runtime.rs` ★：进程级注册（`new_shared_with_audit`，Weak）、`is_engaged`
  （registry 非空 \|\| in-flight）、RAII `InFlightGuard`、`unload_turn`（turn 结束
  清明文）、`path_mappings`、`unrewrite_paths`/`redact`、sweep 持 Weak。
- `src/export_guard.rs` ★：clippy 方法引用清理。
- `src/sdk.rs` ★：clippy 清理。
- `src/runtime_tests.rs` ★：新生命周期/共享 sink/并发测试。

## `codex-rs/core`

- `src/agent_security.rs` ★（新增）：`AgentSecurityContext`（runtime+session_id+
  live `engaged()`）、`sandbox_applies_binds`、`ensure_encrypted_skill_sandbox`
  （engaged 且无沙箱拒绝）、`rpc` 纯函数（`any_engaged`/`is_guarded_path`/
  `command_references_guarded_path`/`args_reference_guarded_path`）。
- `src/agent_security_tests.rs` ★（新增）：engaged/门控/路径判定单测。
- `src/encrypted_skills_guard.rs` ◇：`before_tool` 未 engaged 直接 Allow；
  binds 生效时保留逻辑路径并把逻辑路径纳入受保护集合；`RedactingToolOutput`
  engaged 门控与透传；`redact_turn_item` 扩展覆盖 Plan/CommandExecution/
  FileChange/WebSearch/CollabAgentToolCall/DynamicToolCall/McpToolCall 文本面
  （明文+路径红act）；`before_tool_with_runtime` 标记 `#[cfg(test)]`。
- `src/encrypted_skills_guard_tests.rs` ◇：76 个单测（engaged 门控、binds、
  turn item 红act、路径拦截等）。
- `src/lib.rs` ◇：导出 `agent_security` 模块。
- `src/exec.rs` ★：`build_exec_request` 透传 readonly_binds/上下文。
- `src/sandbox_tags_tests.rs` ★：`readonly_binds` 默认值断言更新。
- `src/session/review.rs` ★：review 线程继承 `agent_security` 上下文。
- `src/session/turn_context.rs` ★：`TurnContext.agent_security` 字段与按 turn 组装。
- `src/session/session.rs` ◇：runtime 构造改为 `new_shared_with_audit`。
- `src/tools/orchestrator.rs` ★：首次 attempt 前 engaged 沙箱校验；
  `SandboxAttempt.skill_binds` 注入；D9 技能脚本 execute-only 自动 Permit。
- `src/tools/registry.rs` ◇：工具结果包 `RedactingToolOutput`；telemetry
  `log_preview` 先红act再记录；`before_tool` engaged 门控。
- `src/tools/runtimes/shell.rs` ★、`src/tools/runtimes/shell/unix_escalation.rs` ★、
  `src/tools/runtimes/unified_exec.rs` ★：D9 auto-permit 审批起点。
- `src/tools/runtimes/apply_patch.rs` ★、`src/tools/runtimes/mod_tests.rs` ★、
  `src/tools/runtimes/apply_patch_tests.rs` ★、`src/tools/sandboxing.rs` ★、
  `src/tools/sandboxing_tests.rs` ★：`readonly_binds`/`skill_binds` 透传与测试。
- `tests/suite/encrypted_skills.rs` ◇：集成测试改为 workspace-write 沙箱执行，
  新增“脚本回显明文在 rollout 中被红act”断言。

## `codex-rs/protocol`

- `src/permissions.rs` ★：新增 `ReadonlyBind`（source/target）；`FileSystemSandboxPolicy`
  增加 `readonly_binds`（serde default 空列表）；序列化与单测。
- `src/models.rs` ★：built-in 权限档案相关常量/辅助（danger-full-access 判定用）。

## `codex-rs/sandboxing`

- `src/landlock.rs` ★：`create_linux_sandbox_command_args...` 把 `readonly_binds`
  转成 `--ro-bind` CLI 参数。
- `src/manager.rs` ★：`SandboxTransformRequest.readonly_binds` 通道。
- `src/landlock_tests.rs` ★、`src/manager_tests.rs` ★：透传/默认值测试。

## `codex-rs/linux-sandbox`

- `src/bwrap.rs` ★：`readonly_binds` 在 base mounts 之后应用 `--ro-bind`
  （缺失 target `--dir`、缺失 source 跳过）；真实 bwrap 执行测试（逻辑路径可见、
  `/dev/shm` 私密）。
- `src/linux_run_main.rs` ★：`--ro-bind` CLI 参数解析与合并进 policy。

## `codex-rs/exec-server` / `codex-rs/file-system`

- `src/fs_sandbox.rs` ★、`src/process_sandbox.rs` ★：把
  `FileSystemSandboxContext.readonly_binds` 传递到 sandboxing 请求。
- `codex-rs/file-system/src/lib.rs` ★：`FileSystemSandboxContext` 增加
  `readonly_binds` 字段与序列化。

## `codex-rs/cli`

- `src/main.rs` ◇：产品策略——拒绝 `--sandbox danger-full-access` 与
  `dangerously-bypass-approvals-and-sandbox`（根级+exec/resume/fork/archive/
  delete/unarchive 子命令级）；拒绝 `plugin`/`marketplace` 子命令。
- `src/debug_sandbox.rs` ★：`readonly_binds` 透传。
- `tests/plugin_policy.rs` ★（新增）：插件/市场策略拒绝断言。
- `tests/product_policy.rs` ★（新增）：full-access/bypass 拒绝断言。
- `tests/plugin_cli.rs` ◇、`tests/marketplace_add.rs` ◇、
  `tests/marketplace_remove.rs` ◇、`tests/marketplace_upgrade.rs` ◇：
  恢复为原测试并统一加 `#[ignore = "...disabled by product policy"]`（非删除）。

## `codex-rs/app-server`

- `src/request_processors/rpc_guard.rs` ★（新增）：engaged 判定与统一 Block 文案；
  `ensure_path_not_guarded`/`ensure_command_not_guarded`/`ensure_args_not_guarded`/
  `ensure_not_engaged_unsandboxed`/`ensure_plugin_management_allowed`/
  `ensure_config_mutation_allowed`。
- `src/request_processors.rs` ★：`rpc_guard` 改为 `pub(crate) mod`。
- `src/request_processors/fs_processor.rs` ★、`command_exec_processor.rs` ★、
  `process_exec_processor.rs` ★、`thread_goal_processor.rs` ★、
  `thread_processor.rs` ★、`turn_processor.rs` ★：RPC guard 接线（fs 读写/枚举/
  watch/copy/remove、command/exec、process/spawn、thread shell/name/goal/metadata、
  inject_items、thread/start、turn/start、thread/settings/update danger 拒绝）。
- `src/message_processor.rs` ◇：分发入口统一执行插件/市场与配置变更策略检查。
- `tests/suite/v2/rpc_guard.rs` ★（新增）：E2E——真实加载加密 Skill，在 turn
  in-flight 窗口验证 fs/command/shell/process 拦截、普通路径放行、配置变更 engaged
  拒绝、未 engaged 放行。
- `tests/suite/v2/product_policy.rs` ★（新增）：thread/turn start 与 settings
  update 的 danger-full-access 拒绝。
- `tests/suite/v2/plugin_policy.rs` ★（新增）：marketplace/plugin 变更 RPC 拒绝、
  只读 list 放行。
- `tests/suite/v2/marketplace_add|remove|upgrade.rs` ★、
  `tests/suite/v2/plugin_install|list|read|share|uninstall.rs` ★：恢复原测试并
  `#[ignore]`（非删除）。
- `tests/suite/v2/mod.rs` ◇：注册新增模块与恢复模块。
- `tests/suite/v2/thread_settings_update.rs` ★、
  `tests/suite/v2/turn_start.rs` ★、`tests/suite/v2/turn_start_zsh_fork.rs` ★：
  用例改为 WorkspaceWrite/配置级 full-access，保持原测试意图。
- `Cargo.toml` ◇：dev-dependencies 增加 `zip`（测试构造加密包）。

## 根目录文档

- `FM_AGENT_SECURITY_DESIGN.md`：实施契约（需求、D1-D10、风险矩阵、TODO-1~10
  结论、推荐默认配置模板）。
- `FM_AGENT_SECURITY_FIX_LOG.md`：I1~I31 逐条记录与最终验证结果。
- `FM_AGENT_SECURITY_STATUS.md` ★：实施状态与待办速览（由两份 SUMMARY 合并）。
- `FM_AGENT_SECURITY_FIX_PLAN.md`：删除（内容已全部完成并被 FIX_LOG 覆盖，
  删除前在 FIX_LOG 头部留历史备注）。

## OpenSpec（第二阶段归档）

- `openspec/changes/archive/2026-08-05-*`：encrypted-skill-engagement、
  agent-security-context-gating、encrypted-skill-sandbox-binds、
  agent-security-product-policy、agent-security-rpc-guard、
  agent-security-final-boundaries、encrypt-agent-security-skills。
- `openspec/specs/*`：对应主 spec（含新增 capability 汇总）。
