# fm/agent-security review fix log

Each entry records one review finding and the commit that fixed it. Bazel/CI
items were excluded per request; the skill-level TTL stays single-tier by
design.

历史备注：实施前计划 `FM_AGENT_SECURITY_FIX_PLAN.md`（I1~I17 问题清单与方案）已全部完成，
逐条修复记录在本文件 I1~I31 中，原计划文档已删除。

| # | Commit | Scope | Fix |
|---|--------|-------|-----|
| 1 | `43d91df7ae` | `fm/encrypted-skills` token | Require 32-hex-char token keys, reject ambiguous session ids (`:`, `]`); replace per-byte `format!` with `write!` in `hex_encode` |
| 2 | `4abe4cfef4` | `fm/encrypted-skills` SDK | Cap package at 512 entries and 16 MiB decompressed bytes to close the zip-bomb/OOM window |
| 3 | `18f0ea9358` | `fm/encrypted-skills` audit | serde_json JSONL (escaping), `timestamp_ms` field, 0600 file mode, log rotation/write/flush failures |
| 4 | `a50294cb3c` | `fm/encrypted-skills` mem root | Mutex-serialized one-time init; chmod per-process namespace dir 0700 |
| 5 | `a98c559015` | paths + shell guard | Block script-execution segments that read guarded storage via `<`, `$(…)`, backticks, here-strings, process substitution; keep plain resource args allowed |
| 6 | `77394d1143` | cache/registry/runtime | Cache never evicts registry-referenced tokens; `register` returns the replaced record and `load_or_register` wipes replaced/partial dirs; remove dead public API; docs match single-tier TTL |
| 7 | `6594611221` | periodic sweep | Sweep task holds `Weak<EncryptedSkillRuntime>` so it exits when the session runtime drops instead of leaking it |
| 8 | `31cfaea079` | fork isolation | Strip sentinel tokens from all forked text surfaces (assistant/tool/reasoning, inter-agent messages, compacted history, function args) |
| 9 | `9d47154608` | skill injection | Honor `metadata.encryption.package`; fall back to `<name>.zip.enc` |
| 10 | `5864546904` | fm-license | `verify_at_startup` returns `Result<LicenseGuard>` instead of an always-`Some` `Option` |
| 11 | `e28a313c46` | audit sink | Log audit events dropped because the sink mutex was poisoned |
| 12 | `52a9289e22` | session wiring | `tracing::warn!` when the simulated `test_zip` envelope SDK is selected |
| 13 | `6c94e649e7` | rehydrate API | Remove unused `Runtime::rehydrate`; gate unframed helper to `cfg(test)` |
| 14 | `9119204ba7` | OpenSpec | Align access-control spec with shell-only decrypted-storage channel |
| 15 | `cde71bb57e` | hook/tool-output redaction | Redact skill plaintext + decrypted paths from hook additional contexts (PostToolUse etc.) at the source and extend persistence redaction to developer-role messages |
| 16 | `70d2ebabbd` | compaction trace | Redact known plaintext + paths on compaction trace input/replacement history before recording (remote-compaction only; local trace is disabled) |
| 17 | `160bdbecfb` | fragment matching | Match and redact any ≥20-char contiguous skill fragment (sed/head/cut/tail middle excerpts), not just full lines or 20-char prefixes |
| 18 | `3dca5c95e7` | tool-call handling | Block shell commands containing known skill plaintext (`shell_plaintext`) and redact FunctionCall/CustomToolCall arguments at durable surfaces |
| 19 | `749c38982a` | normalization | Normalize both sides before matching: markdown links/HTML tags stripped, Unicode NFKC + lowercase, symbols dropped as separators, whitespace collapsed; normalized matched lines redacted whole |
| 20 | `b92c426f51` | runtime races | F1: touch inside registry lock on cache hit (TOCTOU); F3: lifecycle state_lock serializes store+register vs clear_session; F5: per-(session,skill) in-flight gate so concurrent loads decrypt once |
| 21 | `2de0c3caa0` | audit sink | F4: process-wide per-(path, max_bytes) shared FileAuditSink (Weak registry) wired into Session::new; one writer per file, no concurrent rotation |
| 22 | `5d9fae664c` | fmsh-ukey groundwork | Reserve `SdkKind::UKey`/`Local` and `sdk = "ukey"/"local"` config variants (fail-closed until `fmsh-ukey` feature is wired); schema regenerated |
| 23 | `01d3a010c0` | guardian/review redaction | Reviewer models must not see the real decrypted `/dev/shm` location: `redact_guardian_request` rewrites registered decrypted dirs back to original skill paths and redacts remaining memory-root segments in every command-bearing `GuardianApprovalRequest` (Shell / ExecCommand / Execve / NetworkAccess trigger) before the review prompt is built; shared `redact_storage_paths` helper is now also used by `RedactingToolOutput` |
| 24 | openspec: encrypted-skill-engagement | 会话级 engaged 判定基础：`is_engaged`（registry 非空 || 解密 in-flight）、RAII `InFlightGuard` 覆盖“明文落盘→登记”窗口、失败路径保持 `secure_wipe`、`path_mappings` 导出 `(解密目录, 逻辑路径)`；`clear_thread` 同步清理 in-flight；未 engaged 不引入任何行为（门控由后续变更实现） |
| 25 | openspec: agent-security-context-gating | `AgentSecurityContext`（runtime + session_id + live `engaged()`）；`TurnContext.agent_security` 按 turn 组装（engaged 时 Some，否则 None）；`before_tool` 未 engaged 直接 `Allow`；`RedactingToolOutput` 未 engaged 原样透传；未 engaged 会话恢复零行为变化，engaged 行为与变更前一致 |
| 26 | openspec: encrypted-skill-sandbox-binds | P1 落地：protocol `ReadonlyBind` + `FileSystemSandboxPolicy.readonly_binds`；bwrap 在 writable roots/masks 之后应用 `--ro-bind`（缺失 target 用 `--dir`，缺失 source 跳过）；binds 经 `SandboxTransformRequest.readonly_binds` → helper `--ro-bind` CLI → 合并进 policy 传输（避免改 `PermissionProfile` 枚举，记录为决策）；`sandbox_applies_binds` 判定与 bwrap 跳过条件对齐（unreadable-glob 边角近似，沿用旧重写）；`SandboxAttempt.skill_binds` 由 orchestrator 注入；guard 在 binds 生效时保留逻辑路径并把逻辑路径纳入受保护集合，否则保持旧重写；D9：engaged execute-only 技能脚本在 shell/unified_exec 审批起点自动 Permit（不提示用户、不进 guardian）；远程 exec-server 经 `FileSystemSandboxContext.readonly_binds` 传递 |

## I23 补充说明：`/dev/shm` 路径的模型可见性

审计确认：常规模型上下文不会暴露解密路径——工具调用历史保留模型原始参数，工具输出统一经过
`RedactingToolOutput`（解密目录改写回原目录 + mem-root 前缀兜底红act）。但发现一条真实泄露通道：
guardian / auto-review 的评审请求（`GuardianApprovalRequest::Shell` / `ExecCommand` / `Execve` /
`NetworkAccess.trigger`）携带的是 guard 重写后的**真实 `/dev/shm` 命令**，会被序列化进评审模型的
prompt。I23 在 `run_guardian_review` 入口统一红act，评审模型只看到逻辑技能路径。

仍待处理（根因修复，非本条目范围）：
- 用户审批 UI 与 permission hooks 仍会看到真实命令（不面向大模型）。
- 默认 workspace-write 的 bwrap 沙箱把 `/dev/shm` 挂成私有空 tmpfs，重写后的真实路径在沙箱内
  不可见，导致技能脚本执行与沙箱隔离存在张力；根治方案是把解密目录以只读 bind 挂到沙箱内的
  逻辑技能路径（`--ro-bind <decrypted> <logical>`），使命令永不包含 `/dev/shm`。
| 23 | `41c2632016` + `33f94b4371` | fmsh-ukey integration | Add `fmsh-ukey-cipher` as optional git dependency pinned to upstream `f09dc46` (openssl >= 0.10.76), add optional `fmsh-ukey` feature, implement `UKeySdk`/`LocalSdk`, wire `local_privkey` config; openssl lock bumped 0.10.75 -> 0.10.81; feature build verified with a stub SDK (`136 passed`) |

## Verification

- `just test -p fm-encrypted-skills`: 124 passed.
- `just test -p codex-core` (`encrypted_skills` + `spawn` filters): 66 + 103 passed.
- `just test -p codex-core-skills` (excluding two pre-existing environment-only
  failures in this workspace): 135 passed.
- `just test -p fm-license`: passed.
- `cargo check -p codex-cli`: passed.

After the I11–I14 round: `fm-encrypted-skills` 124 passed and the
`codex-core` `spawn` filter 102 passed (one approvals test is flaky in this
workspace and passed on the identical code in the previous run).

After I15–I16: `codex-core` `encrypted_skills` filter 69 passed (3 new
redaction tests), `spawn` filter 102 passed.

After I17–I18: `fm-encrypted-skills` 127 passed (3 new fragment tests),
`codex-core` `encrypted_skills` filter 73 passed (4 new tool-call tests),
`spawn` filter 102 passed.

After I19: `fm-encrypted-skills` 131 passed (4 new normalization tests),
`codex-core` `encrypted_skills` filter 74 passed (1 new normalized-variant
guard test), `spawn` filter 102 passed.

### 最终验证（agent-security 全部变更后）

- `just test -p fm-encrypted-skills`: 142 passed。
- `just test -p codex-protocol`: 265 passed。
- `just test -p codex-cli`: 290 passed。
- `just test -p codex-app-server`: 1028 passed（3 个 zsh-fork 用例 flaky 重试通过）。
- `just test -p codex-linux-sandbox`: 122/124 passed；2 个失败为已知环境性网络用例
  （wget 超时、socketpair），与本次改动无关。
- `just test -p codex-core`（`encrypted_skills` 过滤）: 87 passed；guard 73 passed；
  agent_security 6 passed。全量 3208 个用例中 21 个失败 + 1 个超时，均为环境相关
  （真实 `~/.agents/skills` 污染 skills 目录测试、项目信任状态、代理网络下的
  approvals/network/unified_exec、MCP 超时），与本变更涉及文件无关；其中
  `script_execution_rewrites_original_path_and_redacts_output` 因产品策略（engaged 必须
  沙箱）改为 workspace-write 配置后通过。
- `bazel build //codex-rs/cli:codex //codex-rs/app-server:codex-app-server
  //codex-rs/linux-sandbox:codex-linux-sandbox //codex-rs/fm/encrypted-skills:encrypted-skills`:
  成功；`just bazel-lock-update` 无 lockfile 变化（zip 已在依赖图中）。

TODO-6/7/8/9 收尾后复跑：`codex-app-server` 全量 906/906；`codex-core`
`encrypted_skills`+`encrypted_skills_guard`+`agent_security` 96/96；`just fix` 干净。

四种加密模式（I33）后复跑：`codex-config` 224/224；`fm-encrypted-skills` 143/143；
feature 构建（`--features fmsh-ukey` + stub SDK）下 `sdk::tests` 8/8，包含
`software_sdk_decrypts_hpke_package`（软件私钥经 memfd 载入、HPKE 解密）与
`two_phase_sdk_decrypts_package_with_in_memory_key`（key.enc 解开后内存密钥解密）；
真实 UKey 硬件路径仍需带 `FMSH_UKEY_SDK_DIR` 与设备的环境验证。

After I20–I21: `fm-encrypted-skills` 135 passed (4 new shared-sink tests +
concurrent-load single-decryption assertion), `codex-core` `encrypted_skills`
filter 74 passed, `spawn` filter 102 passed. F2 kept as-is (stale-token
eviction at request boundary is the intended semantics, covered by
`request_rehydration_enforces_ttl_for_idle_skills`); F6 (per-session periodic
task count) left as documented overhead.

Two pre-existing loader tests fail identically before and after these changes
when run inside this git worktree (`non_git_repo_skills_search_does_not_walk_parents`,
`skill_roots_include_admin_with_lowest_priority`); they are environment-dependent
and unrelated to this fix set.

## Deliberately not changed

- Bazel/CI items (MODULE.bazel.lock, workspace `zip` default-features): excluded
  per request.
- Thread-level TTL: single skill-level TTL is by design.
- `TestZipSdk` remains selectable as the documented simulated envelope, but
  selecting it now emits a loud startup warning.
- Non-shell tool relative-path rewriting: the access-control spec was updated
  to match the implemented shell-only channel decision.

## 待处理问题（已记录，未修复）

- **P1 `/dev/shm` 路径可见性与技能脚本执行**：guard 目前把命令中的技能路径重写为真实解密路径
  （`/dev/shm/fm_skill_security_*/p<pid>/...`）。该真实路径会进入执行副本，并可能随审批/评审
  负载外发（in-core guardian 评审已红act，见 I23；用户审批 UI、permission hooks、telemetry
  仍可见）。同时默认 workspace-write 的 bwrap 沙箱把 `/dev/shm` 挂为私有空 tmpfs，重写后的
  路径在沙箱内不可见，技能脚本在 bwrap 下实际不可执行；且“写脚本 → 解释器间接读 `/dev/shm`”
  的攻击在无沙箱/full-access 模式下无法靠字符串检测拦截。
  已定方案（未实现）：把解密目录以只读 bind 挂到沙箱内的逻辑技能路径
  （`--ro-bind <decrypted> <logical>`），命令不再包含 `/dev/shm`；guard 按“是否启用 bwrap bind”
  决定是否保留旧的重写行为；涉及 protocol（`FileSystemSandboxPolicy.readonly_binds`）、
  linux-sandbox bwrap、core sandbox orchestrator、guard 与测试。

- **P2 缺少统一的“加密技能环境”上下文**：当前没有全局/上下文标志标识“本会话处于加密 Skill
  环境、需启用 Agent Security 路线”。`EncryptedSkillRuntime` 是 per-session 的
  （`Session.services.encrypted_skills_runtime`），但 guard/红act 调用点靠零散传参
  （`&Session`、`runtime + session_id`），且 `guard_shell` 无条件把 mem-root（`/dev/shm`）纳入
  保护——即使会话没有加载任何加密技能，普通会话也会被“命令提及 `/dev/shm` 即拦截”影响。
  已定方向（未实现）：做 **Session/Thread 级环境**（`Option<AgentSecurityContext>`，内含
  `Arc<EncryptedSkillRuntime>` + `thread_id`），**懒加载启用**——只有会话加载/触发了加密
  Skill、开始解密、或存在泄露风险时才 `engaged`，此时才开启严格护栏（shell 路径 guard、
  export guard、输出/评审红act、审批红act等）；未触发时 `is_engaged() == false`，解密目录
  不存在、明文不可读，完全走 Codex 正常路径（连 `/dev/shm` 字符串拦截与输出红act都不启用）。
  上下文在 Session/TurnContext 创建时组装；`engaged` 直接由 runtime 状态派生
  （registry 非空或解密 in-flight），不维护容易失同步的独立标志位。细化：
  **Session 级状态 + 每 Turn 决定翻转**——状态（runtime/registry/明文目录）在 Session 级，
  TurnContext 每 turn 决定当前生效的 `Option<AgentSecurityContext>`；翻转条件必须绑定“明文
  当前是否实际存在”，而不是“本 turn 是否使用 skill”。注意：当前明文不会在 turn 结束时自动
  销毁（`clear_encrypted_skills` 无调用者，TTL 空闲 600s 才清理），所以现状下严格的 turn 级
  翻转不安全；若要做 turn 级，需先加 turn 结束清理。并发场景（app-server 同 session 多 turn）
  下 guard 判定应读 live session 状态，而非 turn 开始时的快照。
  **范围补充：RPC 面纳入护栏**——Codex 可能通过 ACP 协议连接并提供前端，app-server 的客户端
  直连 RPC 也必须受保护：`fs/readFile`、`fs/readDirectory`、`fs/getMetadata`、`fs/watch`、
  `fs/writeFile`、`fs/copy`、`fs/remove` 等 fs 方法，以及 `command/exec`、`process/spawn`、
  `thread/shellCommand` 等执行方法。这些处理器目前没有 encrypted runtime 访问路径，需要让
  app-server 能按 thread_id 解析到该 session 的上下文（例如把 runtime 提升为进程级共享服务，
  registry 内部仍按 thread_id 隔离），并复用与 Agent Loop 相同的路径/命令检查逻辑。
  完整实施契约见 `FM_AGENT_SECURITY_DESIGN.md`。
- **I30（未修复，TODO-7/9 残留）**：`features.plugins=true` 配置下插件启动加载/同步仍未在产品层
  强制关闭；TUI 插件管理入口（依赖只读 plugin/list 与变更 RPC）未单独收敛；产品构建需默认禁用
  plugins feature 或由受信管理工具下发配置，安全路线不得挂在用户可关闭的 flag 下。
- **I31（未修复，TODO-10，分析模式）**：`/dev/shm`（tmpfs）页在内存压力下可被内核换出到
  swap，解密明文存在落盘可能；当前实现无 `mlock`/pin 逻辑。默认 `RLIMIT_MEMLOCK`（本环境
  8 MiB）小于单包明文上限 16 MiB，未提权时无法保证 pin 成功。待产品决策：若承诺“明文不落盘
  （含 swap）”，需对解密目录内的文件 mmap+mlock（失败 fail-closed 或显式降级并记录）；memfd
  方案不适用：Skill 解密后是 zip 展开出的目录树，脚本/工具都依赖真实路径与相对引用，memfd
  只有单个 blob 无目录层级。否则需修正文档中“明文仅在内存”的表述，并明确 swap 内容属
  root/取证威胁模型外。
  两种部署场景记录：有 root（特权部署，可 `LimitMEMLOCK=infinity`/`setrlimit` 提升上限，
  推荐 mmap+mlock、失败 fail-closed，可配合无 swap 设备）；无 root（普通容器，8 MiB 限制，
  默认接受 swap 属威胁模型外，或对无法 pin 的 Skill fail-closed，或用 `VmSwap` 监控告警，
  部署侧提 `LimitMEMLOCK` 后再启用 pin）。

| 27 | `1c694a0269` + `5c44c17a17` | openspec: agent-security-rpc-guard | 进程级 engaged 注册表（Weak，`any_engaged`/`engaged_guarded_paths`）；telemetry 工具预览先包 `RedactingToolOutput` 再取 `log_preview`（日志不再含明文/路径）；core `agent_security::rpc` 纯函数（fs path/command/args）；app-server RPC 面接线：fs 读/枚举/watch/写/复制/删除、`command/exec`、`thread/shellCommand`、`process/spawn`、`thread/inject_items`、`thread/name|goal|metadata`；无 thread_id 的 RPC 采用进程级“任一 engaged”保守判定（决策记录）；E2E 集成测试：真实加载加密 Skill，在 turn in-flight 窗口内验证 fs/readFile、command/exec、thread/shellCommand、process/spawn 被拦截，普通路径放行，未 engaged 时不受影响 |
| 28 | `e29eda359d` | openspec: agent-security-product-policy | I6/I7/D10 落地：core `ensure_encrypted_skill_sandbox`（engaged 且无有效沙箱 → 拒绝执行）；CLI 拒绝 `--sandbox danger-full-access` 与 `dangerously-bypass-approvals-and-sandbox`（根级 `interactive.shared` 之外，覆盖 exec/resume/fork/archive/delete/unarchive 子命令自身 shared 参数），拒绝 `plugin`/`marketplace` 子命令；app-server `thread/start`/`turn/start` 请求参数携带 danger-full-access 时 `invalid_request` 拒绝；插件/marketplace CLI 原集成测试保留并标记 `#[ignore]`（策略拒绝断言另存 `plugin_policy.rs`/`product_policy.rs`）；app-server 既有 full-access 用例改为配置级 full-access 或 WorkspaceWrite 保持原意图 |
| 29 | `faf8a03fb3` + `6a2f9a217a` | TODO-6/7/8/9 收尾 | app-server 消息边界统一拦截 `marketplace/add|remove|upgrade`、`plugin/install|uninstall`、`plugin/share/save|updateTargets|checkout|delete`（只读 list/read 保留）；`thread/settings/update` 拒绝 danger-full-access；engaged 时拒绝 `config/value/write`、`config/batchWrite`、`experimentalFeature/enablement/set`、`skills/config/write`、`skills/extraRoots/set`（未 engaged 行为不变）；`redact_turn_item` 扩展覆盖 CommandExecution/FileChange/WebSearch/CollabAgentToolCall/DynamicToolCall/McpToolCall 文本面（明文+路径红act），集成测试验证技能脚本回显明文在 rollout 中被红act；原 8 个 app-server 插件/市场测试文件保留并标记 `#[ignore]`（非删除），策略拒绝测试另存 `plugin_policy.rs` |
| 32 | `a1e101d764` | debug 沙箱 bypass | 新增 `FMSH_CODEX_AGENT_SECURITY_SANDBOX_BYPASS=1`：仅 `debug_assertions` 构建生效（release 恒 false），CLI/app-server 的 danger-full-access 拒绝与 engaged 运行期沙箱校验全部放行；插件禁令与 engaged 配置写拒绝不受影响；启用时打一次性 `tracing::warn!` |
| 33 | `a8ee85aa0e` | 四种加密模式与两阶段解密 | 同步 `fmsh-ukey-lib` 最新（`3516cd5`，`local` 更名 `software`，四种方式：`noop`/`software`(hpke 默认或 sm2-sm4-cbc)/`ukey`/`ukey-two-phase`）；`EncryptedSkillsSdkToml` 增加 `Noop`/`Software`/`UKeyTwoPhase` 与 `software_algorithm`/`software_privkey`/`key_envelope` 配置；`SdkKind` 同步扩展；新增 `memfd.rs`（软件私钥经 memfd 载入、瞬态密钥不落盘）；`UkeyTwoPhaseSdk` 每 Skill 一个 `key.enc`、一次 UKey 解开后在内存缓存 AES key，后续包软件解密；新增 noop/memfd 单测与 feature-gated software/two-phase 单测 |
| 34 | `8102050b6d` | write_stdin / app-server stdin / lock poisoning | 二次安全复核三轮修复：① `guard_stdin_input` 守卫注入交互式 shell 的 stdin（write_stdin 的 pre_tool_use_payload 返回 None，spawn 时 guard 只检查启动命令，后续 `cat <skill路径>` 绕过信任层级）；② app-server `process/writeStdin` 解码 base64 delta 后过 `ensure_command_not_guarded`，关闭 spawn-time/unengaged→engaged 的 TOCTOU；③ `runtime.rs` 9 个只读安全查询方法（is_engaged/known_plaintexts/decrypted_dirs/path_mappings/rewrite_paths/unrewrite_paths/touch/has_engaged_state/engaged_session_paths）从 `unwrap_or(false)`/`unwrap_or_default()` 改为 `recover_lock`（`PoisonError::into_inner`），锁中毒后继续服务内存状态而非静默 fail-open；变更路径（load_or_register_inner/clear_session/sweep/rehydrate_framed）保持 `map_err(lock_error)` fail-safe；故意用 std Mutex 而非 parking_lot（poisoning 是 panic 检测锚点） |
| 35 | `8b81ccc792` | 上游 fmsh-ukey-lib vendor SDK + thread-manager-sample 修复 | ① 上游 `8626472` vendor FMSH UKey SDK linux 库进仓库（`vendor/fmsh-ukey-sdk/linux/lib`），wrapper build.rs fallback 到 workspace vendor 目录，feature 构建不再需要 `FMSH_UKEY_SDK_DIR`；同步 rev `a37065c`→`8626472` 并验证：`cargo check/test -p fm-encrypted-skills --features fmsh-ukey` 无 SDK 环境变量全绿（**146/146**，含 `software_sdk_decrypts_hpke_package` 与 `two_phase_sdk_decrypts_package_with_in_memory_key` 真实软件解密；UKey 硬件路径返回 `HardwareKeyRequired`，测试经注入 `with_key_wrap` 不碰硬件）；② 修 thread-manager-sample 残留 API 漂移：`encrypted_skills_local_privkey` → `encrypted_skills_software_privkey`（I33 四模式重构改名），补 `software_algorithm`/`key_envelope` 字段——这是全量 clippy 唯一真实 error（其余为 pre-existing bwrap C 警告与 rmcp-client 弃用）。注意：vendored `.so` 为 Linux x86_64，macOS/Windows 仍需厂商 SDK 或对应平台变体；测试运行时需 `LD_LIBRARY_PATH` 指向 vendored lib（build.rs 的 rpath 只传播给 wrapper 自身产物） |
| 36 | `待提交` | 两阶段密钥缓存内容寻址 + fmsh-ukey 硬绑定 | ① `UkeyTwoPhaseSdk.ciphers` 缓存键从 `PathBuf`（key_path）改为 `Vec<u8>`（key.enc 文件内容本身）——N 个 Skill 共享同一份 key.enc 时，unwrap 从 N 次收敛为 1 次，后续全部命中同一内存 cipher；独立密钥 Skill 不受影响；零额外依赖（文件内容即键）。新增 `shared_key_envelope_is_unwrapped_exactly_once_across_skills`（counting KeyWrap 断言两个目录同一 key.enc 只 unwrap 一次）；② 按部署要求将 fmsh-ukey-cipher 从 `optional` feature 改为**硬绑定**（`[target.'cfg(linux x86_64 gnu)'.dependencies]` 非 optional），删除 `fmsh-ukey` feature 定义与 sdk.rs/config_toml.rs 全部 `feature = "fmsh-ukey"` cfg（改为纯 target 门控）；默认构建即含 software/ukey/ukey-two-phase 三个真实后端（147/147 默认跑通，含 HPKE/两阶段/共享密钥真实解密）；非 Linux 平台仍 fail-closed（vendored `.so` 仅 Linux x86_64） |

## I28 补充说明：产品策略边界与决策记录

- 强制沙箱（I6）落在三层：engaged 运行期断言（orchestrator 兜底，`initial_sandbox == None`
  即拒绝）、CLI 参数拒绝、app-server 请求参数拒绝。`config.toml` 直改与同 uid 直接读写不在
  产品保证范围内（同 uid 威胁模型，官方入口必须全部封住）。
- `dangerously_bypass_approvals_and_sandbox` 与 `--sandbox danger-full-access` 等价，一并拒绝；
  exec/resume/fork/archive/delete/unarchive 子命令自身携带的 shared 参数也要检查（根级
  `interactive.shared` 覆盖不到子命令参数）。
- 插件禁令（I7）当前落在 CLI 入口；app-server 插件/marketplace RPC 与 TUI 入口标记为最终阶段
  （TODO-9）。核心配置 `features.plugins=true` 的启动加载暂未在产品层强制关闭，产品构建需由
  默认配置/受信管理工具保证，已列入“待处理问题”。
- D10 确认（用户决策回填）：`thread/realtime/*` 产品不提供，排除在范围外；客户端不提供任何
  改配置/改模型配置入口；Skill 仅可配置启用/禁用；配置由未来受信管理工具负责。
- 测试策略（非破坏性）：插件/marketplace 原有 CLI 与 app-server 集成测试全部保留，统一加
  `#[ignore = "plugin and marketplace management is disabled by product policy"]` 跳过；
  产品策略拒绝行为由新增的 `plugin_policy.rs`/`product_policy.rs` 测试覆盖；core-plugins
  单元测试继续覆盖底层插件逻辑。

## TODO-1 验证记录：bwrap 只读 bind 真实执行

新增 `codex-linux-sandbox` 真实执行测试 `readonly_binds_are_visible_in_real_bwrap_and_dev_shm_stays_private`：
在宿主 `/dev/shm` 放置标记文件、把临时解密目录 `--ro-bind` 到逻辑技能路径后启动真实 bwrap，
断言（1）逻辑路径下能读到绑定内容；（2）沙箱内 `/dev/shm` 为空；（3）宿主 `/dev/shm` 标记
在沙箱内不可见。该测试在 bwrap 不可用或无法创建用户命名空间时自动跳过。

## TODO-2/3/4 验证记录

- TODO-2：发现并修复 `TurnItem::Plan`（task 列表/计划）未红act的缺口——`redact_turn_item` 新增
  `PlanItem.text` 红act并补测试；reasoning summary/raw、agentMessage 已有覆盖；路径面由
  `RedactingToolOutput`/`redact_storage_paths` 统一处理；subagent 消息走 agentMessage 面。
- TODO-3：确认 rollout/compaction/fork/state 持久化在源头红act（有测试断言），磁盘无明文，
  不需要把 rollout/state db 路径纳入受保护路径集合；resume/fork 正常读取不受影响。
- TODO-4：确认 TUI `!` 走 `thread/shellCommand`，变更 4 的 `ensure_not_engaged_unsandboxed`
  在处理器入口拦截，`rpc_guard` E2E 已覆盖；`process/spawn` 同样拦截。

## I34 验证记录（2026-08-06）

- `fm-encrypted-skills`: **144/144**（新增 `read_only_queries_survive_registry_poisoning`：故意毒化 registry mutex，断言 is_engaged/decrypted_dirs/known_plaintexts 返回正确数据而非空）
- `codex-core` encrypted_skills_guard/periodic: **85/85**（新增 6 个 stdin guard 测试：原始路径 Blocked、解密路径 Blocked、无害命令 Allow、脚本执行 Allow、链式走私 Blocked、unengaged 放行）
- `cargo clippy -p fm-encrypted-skills -p codex-core --lib --tests`: 零 error 零 warning（1 个 pre-existing rmcp-client 弃用警告无关）
- `cargo fmt`: 干净

## I35 验证记录（2026-08-06）

- 上游 `fmsh-ukey-lib` `8626472`（vendor SDK）：`cargo check -p fm-encrypted-skills --features fmsh-ukey` 无 `FMSH_UKEY_SDK_DIR` 编译通过（build.rs fallback `vendor/fmsh-ukey-sdk`）
- `cargo test -p fm-encrypted-skills --features fmsh-ukey`（`LD_LIBRARY_PATH` 指向 vendored lib）：**146/146**，含 `software_sdk_decrypts_hpke_package`、`two_phase_sdk_decrypts_package_with_in_memory_key` 真实软件解密
- 非 feature：**144/144** 无回归
- `cargo clippy -p fm-encrypted-skills --features fmsh-ukey --all-targets`: 干净
- 全量 `cargo clippy --workspace --all-targets`: 唯一真实 error（thread-manager-sample 字段漂移）已修；余下为 pre-existing bwrap C 警告（`nl_pid` 初始化）与 rmcp-client 弃用
