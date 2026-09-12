# FM Agent Security — V2 实现成果与安全报告

> 日期：2026-07-31（v2）· 对应 OpenSpec change：`encrypt-agent-security-skills`（35/35 任务完成，strict 校验通过）
> 前序设计：`FM_AGENT_SECURITY_SKILL_ENCRYPTION_IMPLEMENTATION.md`（v2.0 主 Agent + Token 重水合）

## 0. 当前解密 / 存活 / 卸载速览（2026-08-06）

> 本节为当前分支实现快照，面向同事说明；历史轮次细节见下文各节。

### 0.1 解密

| 项 | 现状 |
|---|---|
| 包布局 | 每个加密 skill 目录：`SKILL.md` stub（frontmatter 声明 `metadata.encryption`：version/key_id/algorithm/mode/package）+ `<name>.zip.enc`；`ukey-two-phase` 额外带 `key.enc`（每包一份，整批共用同一把 AES 密钥） |
| 四种模式 | `software`：HPKE `hpke-x25519-aes256-gcm`（默认）或标准 CMS `sm2-sm4-cbc`（可与 `openssl cms` 互操作）；`ukey`：UKey 硬件解 CMS SM2/SM4 信封；`ukey-two-phase`：UKey 只 unwrap 一次 `key.enc`，之后全部软件 AES-256-GCM；`test_zip` / `noop` 仅供测试 |
| 密钥处理 | 软件私钥 PEM 经 memfd 载入，不落盘；two-phase 的 AES 密钥只驻内存，按 `key.enc` 哈希寻址缓存（同一把 key 只调一次 UKey） |
| 模式选择 | 部署配置 `[encrypted_skills]`：`sdk` / `software_algorithm` / `software_privkey` / `key_envelope`（默认 `"key.enc"`）；模式为部署级固定，不逐包自动探测 |
| 上游依赖 | `fmsh-ukey-cipher` 同步至 `e7986f1b`（2026-08-06，库 API 未变）；已用真实样例（software-hpke / software-sm2sm4 / ukey-two-phase）验证，crate 148/148 单测通过 |

### 0.2 存活（明文驻留窗口）

| 项 | 现状 |
|---|---|
| 落盘位置 | `/dev/shm/fm-agent-security/p<pid>/fm_skill_security_<hex>/`（tmpfs，内存承载、不进 swap；与设计文档早期的“`/dev/shm/fm_skill_security_<hex>/p<pid>`”表述不同，以本节为准） |
| 权限 | mem root、每进程命名空间、每个叶子目录均 0700；叶子名 16 位随机 hex |
| 进程隔离 | 每进程一个 `p<pid>` 命名空间；新进程启动 `init_mem_root` 按 `/proc/<pid>` 存活清理残留（含 legacy 平铺目录） |
| 容量门控 | 解密前检查 mem root 余量 ≥ 4MiB；包上限 512 条目 / 解压后 16MiB |
| 缓存/注册表 | 每会话内容缓存（64 条 / 8MiB、FIFO、去重）；session 级 registry + skill/thread 两级 TTL |
| 审计 | 解密（含 cache hit）、token 化、重水合、清理、拦截均记 JSONL（10MB 轮转） |

### 0.3 卸载

| 触发 | 行为 |
|---|---|
| turn 结束 | `unload_turn`：wipe 解密目录、清 cache/registry、audit `cleanup` |
| TTL 过期 | skill 空闲默认 600s 卸载单个目录；thread 空闲默认 1800s 清空该线程全部目录与缓存 |
| 请求活动 | `rehydrate_framed` 入口先 `sweep()`：任何模型请求都会触发两级 TTL 清理；活跃 token 由命中续费（touch）保护 |
| thread/delete | `clear_encrypted_skills` 立即清理 |
| 进程异常退出 | 残留明文目录由下一个进程启动的 pid 存活清理兜底；容器销毁重建后 `/dev/shm` 整体清空 |
| secure wipe | tmpfs 根直接删除（覆写无意义）；非 tmpfs 根先随机覆写再删除 |

## 1. 实现成果

### 1.1 独立 crate：`codex-rs/encrypted-skills`（`codex-encrypted-skills`）

| 模块 | 职责 | 单测 |
|---|---|---|
| `token.rs` | 哨兵 `[SENSITIVE_SKILL_TOKEN:{session_id}:{hex}]` 序列化/解析 | 7 |
| `sdk.rs` | `EnvelopeSdk` trait、`UnavailableSdk`（fail-closed）、`TestZipSdk`（模拟加密）、`SdkKind` 选择 | 3 |
| `mem_root.rs` | `/dev/shm/fm-agent-security/p<pid>/fm_skill_security_<hex>` 布局、`resolve_default_mem_root`（非 Linux 回退 temp dir）、Zip-Slip 拒绝、0700、`init_mem_root_once`（按 pid 存活清理跨进程残留）、secure wipe | 10 |
| `cache.rs` | 每会话明文内容缓存（去重、条目上限 64、总大小上限 8MiB、FIFO） | 7 |
| `registry.rs` | 两级 TTL 注册表（可注入时钟）、幂等命中、Skill/Thread 卸载 | 9 |
| `rehydrate.rs` | 请求时重水合（跨会话门禁、失效保留、信任层级框架 + base anchor） | 9 |
| `paths.rs` | 原目录→解密目录改写、`/dev/shm` 输出脱敏、读取/搜索/执行命令分类、扩展名白名单 | 10 |
| `export_guard.rs` | 出站明文检测（整段/整行/20 字符前缀） | 6 |
| `runtime.rs` | 宿主 runtime：解密编排、幂等复用、TTL sweep、`rehydrate_framed`、路径改写、脱敏 | 7 |
| `audit.rs` | 审计事件类型与 JSONL 序列化 | 4 |

合计 70 个单测全绿，clippy（`--tests`）干净。

### 1.2 核心接线（薄接线点）

- **元数据**：core-skills loader 解析 frontmatter `metadata.encrypted` / `metadata.encryption`；`SkillMetadata` / `EnvironmentSkillMetadata` 增加 `encrypted` + `encryption`（`is_encrypted()`）；app-server v2 `SkillMetadata` 增加可选字段并重生成 schema fixtures。
- **注入**：`build_skill_injections(..., encrypted_skills, session_id, ...)` 加密分支解密 → 只注入 Token；失败出 warning + error 指标；`SkillInstructions::body()` 加密分支返回哨兵。
- **重水合**：`Prompt.encrypted_skills` → `get_formatted_input_for_request` 对 `InputText` 做 `rehydrate_framed`（信任层级 + skill 名 + 原目录 base anchor，绝不出现 `/dev/shm`），覆盖常规 turn + compaction 三条路径。
- **拦截**：`encrypted_skills_guard` 在工具分发前置执行，工具匹配基于 **codex 真实工具体系**（`HookToolName` 类型化匹配，非 opencode 风格字符串）——`Bash`（`shell_command`/`unified_exec`）读取/搜索命令 Blocked（含 `ls` 探测与自定义 mem root）、执行命令原目录→解密目录改写；`view_image` 读取解密目录 Blocked（handler 暴露 pre payload）；`apply_patch` 参数含已知明文 Blocked；其余工具（`mcp__*`、扩展工具等）放行。`RedactingToolOutput` 对所有工具输出按**运行时实际 mem root** 脱敏。
- **Fork**：`keep_forked_rollout_item` 丢弃含哨兵 token 的用户消息。
- **TTL**：turn 边界 `runtime.sweep()`（Skill 10min / Thread 30min 默认值）；`thread/delete` 时 `CodexThread::clear_encrypted_skills()` 立即清理。
- **审计**：`AuditSink` / `FileAuditSink`（JSONL + 10MB 轮转）；runtime 在解密（含 cache_hit）、token 化、重水合、清理时发射事件；guard 拦截时 `record_blocked`；host 在 Session 构建时注入 sink（`$TMPDIR/fm_skill_security_audit.log`）。
- **配置**：`config.toml` 新增 `[encrypted_skills]` 表——`sdk`（`unavailable` 默认 / `test_zip` 测试）、`skill_idle_ttl_secs`（默认 600）、`thread_idle_ttl_secs`（默认 1800），config schema 已重生成。

### 1.3 验证数据

| 套件 | 结果 |
|---|---|
| codex-encrypted-skills | 75/75 |
| codex-core-skills | 131/131（含 3 个 frontmatter 解析 + 4 个注入分支测试） |
| codex-app-server-protocol | 277/277（含 schema 一致性） |
| codex-config | 227/227 |
| codex-core（client_common / guard / spawn） | 8 / 8 / 4 |
| core/suite `encrypted_skills` 集成 | 5/5：rollout 只含 Token；请求带 framed 内容且无 `/dev/shm`；原地址改写执行 + 输出脱敏；直接读取被拦；TTL 内重提复用同一 token；导出明文拦截 |
| codex-core 全量单元测试 | 2132/2134：仅 2 个环境相关失败——① `user_shell_commands_do_not_inherit_managed_network_proxy`（本沙箱设置了环境级 `HTTPS_PROXY`，隔离复跑同样失败）；② `post_sampling_token_estimate_is_disabled_by_always_on_sinks`（并行下 tracing 状态偶发，隔离运行通过）。均与本次改动无关，无回归 |
| codex-core 全量测试（lib + 集成） | 3195 项：**测试基建修复**——`rmcp-client` 的 5 个 stdio 测试服务二进制只有 Bazel `extra_binaries`、无 Cargo `[[bin]]`，导致 `just test` 下 rmcp/search/truncation/sqlite/token_budget 等约 40 项套件测试找不到二进制而失败；已补 `[[bin]]` 声明，这些测试全部转绿。剩余失败均为环境项：zsh fork approval 超时（需交互式 zsh）、网络 denial/approval 测试（沙箱代理+网络策略）、compact_remote token 估算（预算/环境敏感）、tracing 并行偶发；均不触及加密技能代码路径（guard 对 `mcp__*` 与非匹配工具恒放行） |

`just fmt`、`just fix -p codex-core-skills` / `just fix -p codex-core` 均干净。

## 2. 沙箱配置建议

威胁模型：模型通过 shell 执行解密后的脚本；`/dev/shm/fm-agent-security/` 是机器级共享内存目录。进程内 guard（已接线）是第一道控制，沙箱是**纵深防御**——不能只依赖 guard 的字符串匹配。

### 2.1 现状

- codex 沙箱配置（`config.toml`）只有：`sandbox_workspace_write.writable_roots`、`network_access`、`exclude_tmpdir_env_var`、`exclude_slash_tmp`。
- exec crate 的 bwrap/landlock 挂载由 workspace 根 + writable roots 推导；`/dev/shm` 可作为 writable root（exec/tests/suite/sandbox.rs:171 已验证）。
- **没有** per-tool 的只读 bind 粒度配置。

### 2.2 推荐配置（workspace-write 沙箱）

```toml
[sandbox_workspace_write]
writable_roots = ["./", "/dev/shm/fm-agent-security"]   # 允许执行解密脚本
network_access = false                                    # 阻断脚本外泄明文（curl/wget 通道）
exclude_slash_tmp = false
```

要点：

1. **网络隔离优先**：`network_access = false` 是投入产出比最高的项——脚本即使拿到明文也无法外传。若业务需要网络，改用出口白名单代理。
2. **解密目录的可见性**：`/dev/shm/fm-agent-security` 作为 writable root 只服务于 shell 执行；文件读取类工具由 guard 拦截，理想情况下还应由权限层（PermissionProfile）拒绝 `read` 工具对该前缀的访问。
3. **不要用 ro-bind**：bwrap 的 `--ro-bind` 是把目录**只读暴露**进沙箱，会让 `cat` 等读取成功——它不能表达「可执行但不可读」。解密目录必须以 writable root（`--bind`）挂载供解释器读取脚本；读拦截只能留在进程内 guard + 权限层。
4. **会话级隔离（后续）**：多会话并发时，bwrap 只挂当前 session 的解密子目录（`/dev/shm/fm-agent-security/fm_skill_security_*`），而不是整个根目录，避免跨会话可读。
5. **macOS / Windows**：seatbelt 与 Windows 沙箱需要等价规则（允许 exec 该前缀、拒绝 file-read）；当前集成测试 `cfg(not(target_os = "windows"))`，Windows 尚未覆盖。

## 3. 未解决的安全隐患

| # | 隐患 | 现状 | 风险 | 缓解 / 后续 |
|---|---|---|---|---|
| 1 | **沙箱纵深缺失** | 只靠 guard 字符串匹配，沙箱未显式限制 `/dev/shm/fm-agent-security` | 命令混淆可绕过字符串检查 | 按 2.2 配置网络隔离 + writable roots；exec crate 增加只读 bind（后续） |
| 2 | ~~审计日志未接线~~ | **已解决（2026-07-31 v2/v3）**：`AuditSink`/`FileAuditSink` + runtime 事件发射 + guard `record_blocked` + host 注入（见 1.2）；集成测试断言审计文件含 `decryption`/`blocked` 事件 | — | — |
| 3 | **命令混淆绕过 guard** | `command_references_dir` 是子串匹配 | `bash -c 'cat /dev/shm/fm-agent$(printf security)...'`、环境变量拼路径可绕过 | 沙箱只读 bind + 网络隔离兜底；后续可加命令归一化解析 |
| 4 | **输出脱敏是尽力而为** | `redact_decrypted_paths` 只匹配字面路径 | 脚本打印 base64/改写形式的路径可泄露地址 | 信任层级框架引导 + 未来输入端 safe guard 模型（设计已声明） |
| 5 | **进程内明文缓存** | `ContentCache` 持有明文（TTL 有界、每会话上限 64） | 「明文不长期驻留内存」依赖 TTL 扫描正确触发 | 保持 TTL 扫描；必要时对缓存条目做透明加密（后续） |
| 6 | ~~多进程竞态~~ | **已解决（2026-07-31 v4）**：解密目录位于 `/dev/shm/fm-agent-security/p<pid>/` 进程命名空间；`init_mem_root_once` 按 pid 存活（Linux `/proc`）清理其他进程残留，本进程命名空间与存活进程不受影响；非 Linux 平台保守不清除 | — | — |
| 7 | ~~线程结束未即时清理~~ | **已解决（2026-07-31 v2）**：`thread/delete` 处理器对每个待删线程调用 `CodexThread::clear_encrypted_skills()` | — | — |
| 8 | **测试 SDK 配置暴露** | `[encrypted_skills] sdk = "test_zip"` 出现在生产 config schema | 误配置把 `.zip.enc` 当普通 zip 解（真加密包会解压失败，fail 保守） | 真实 SDK 接入后收敛该枚举；或移到 `#[cfg(test)]` 注入 |
| 9 | **外部 hook 信任边界** | guard 在外部 PreToolUse hook 之前执行，hook 可继续改写工具输入 | 被攻陷/恶意 hook 可注入 `/dev/shm` 路径 | hook 属于宿主信任配置；文档声明边界 |
| 10 | ~~工具覆盖不完整~~ | **已解决（2026-07-31 v5）**：`view_image` 新增 pre payload 并被 guard 拦截（handler 统一输出 `path` 键，guard 以 `path` 匹配）；`mcp_resource` 为服务端资源 URI 不涉及宿主路径 | — | — |

> 注（v6 复核）：扩展工具（`web_search`/`webfetch`）实现 `ToolExecutor<ToolCall>`，不暴露 `pre_tool_use_payload`，guard 的 web 导出拦截分支**在现网实际不触发**；弥补手段是沙箱 `network_access = false`（模型拿不到路径也传不出去）。分支保留以兼容未来暴露 pre payload 的 handler。

## 5. 工具匹配全量核对（2026-07-31 v8）

codex 全部 handler 的 hook 名与 guard 归属：

| handler（模型工具名） | hook 名 | guard 分支 | 验证 |
|---|---|---|---|
| `shell_command` | `Bash` | guard_shell（读取/搜索 Blocked、执行改写） | 集成 ✓ |
| `exec_command`（unified_exec，参数键 `cmd` 归一化为 `command`） | `Bash` | guard_shell（同上） | 集成 ✓（`exec_command_direct_read_is_blocked_by_the_same_guard`） |
| `write_stdin` | 无（有意不重复触发） | 不参与（原 exec 已过 guard） | 代码注释确认 |
| `apply_patch`（自定义工具） | `apply_patch` | guard_export（参数含已知明文 Blocked） | 集成 ✓ |
| `view_image` | `view_image` | guard_read（`path` 指向解密目录 Blocked） | 单测 ✓ |
| `mcp__<server>__<tool>`（MCP 工具） | `mcp__*` | guard_export（参数含已知明文 Blocked；路径探测仍放行） | 单测 ✓（`mcp_tools_pass_through` + `blocks_extension_tool_args_containing_known_plaintext`） |
| 其余 Function 工具（current_time/plan/sleep/request_*/mcp_resource/tool_search/get_context_remaining/multi_agents 等） | 默认 `function_hook_tool_name`（= 工具名） | guard_export（参数含已知明文 Blocked） | 单测 ✓ |
| 扩展工具（extension_tools / `web/run` → hook 名 `webrun`） | 默认 `function_hook_tool_name` | guard_export（搜索/请求参数含已知明文 Blocked） | 单测 ✓（`blocks_web_search_tool_containing_known_plaintext`） |

opencode → codex 工具映射（guard 迁移依据）：

| opencode 工具 | codex 对应 | guard 落点 |
|---|---|---|
| `read` | 无独立工具（读取走 shell/apply_patch 上下文/view_image） | Bash + view_image 分支 |
| `bash` | `shell_command` / `exec_command` | Bash 分支 |
| `grep` / `glob` | 无独立工具（搜索走 shell 命令） | Bash 搜索命令分支 |
| `write` / `edit` / `apply_patch` | `apply_patch` | apply_patch 分支 |
| `webfetch` / `web_search` | 扩展 `web/run`（无 pre payload） | 沙箱网络隔离 |

权限/审批（PermissionRequest）与沙箱工具分类与 guard 无冲突：guard 在工具分发前置先执行，审批不改变工具名匹配。

## 6. 深度 Review 记录（2026-07-31 v9）

对三个 spec 逐条核对、安全边界与代码标准复查，发现并修复：

1. **P1 重水合命中不刷新 TTL（已修复）**：`rehydrate_framed` 此前只发射审计事件，未调用 `touch`，违反设计「提及注入 + 重水合命中均刷新 `last_used_at`」——活跃会话中持续被重水合的 skill 可能在 TTL 后被误卸载。修复：重水合替换成功后对命中的 skill 逐个 `touch`（更新 `last_used_at` 与会话活动）；新增对照测试（重水合刷新 vs 未触碰卸载）。
2. **P2 `decrypt_to_dir` 死代码（已删除）**：仅被测试使用，生产路径未引用；连同其专属测试桩一并移除。
3. **P2 spec 与实现偏差（已更新）**：access-control spec 原按 opencode 语义描述 `read` 工具（.md 允许/脚本拦截），codex 无独立 `read` 工具，实现为「文件查看工具（`view_image`）与 shell 读取/搜索命令对解密目录全拦截」。已把 spec 场景更新为 codex 实际工具语义。
4. **P3 记录在案（未改）**：`RedactingToolOutput` 不脱敏 MCP/ToolSearch 输出（MCP server 属外部信任边界）；shell 命令字符串匹配可被混淆绕过（缓解：沙箱网络隔离 + 权限层）。

## 7. 续深 Review 记录（2026-07-31 v10）

1. **P1 `guard_read` 与 `guard_shell` 语义不一致（已修复）**：`view_image` 此前只检查会话已注册的解密目录，指向 mem root 下未知/其他会话子路径时不拦截；`guard_shell` 同时检查运行时 root 与默认常量前缀。修复：`guard_read` 合并检查（注册目录 + 运行时 root + `MEM_ROOT` 常量前缀）；新增 3 个单测（未知子路径 Blocked、默认常量前缀 Blocked、外部路径 Allow），guard 12/12。
2. **集成覆盖补全**：新增 `view_image_on_mem_root_is_blocked`（模型调 view_image 指向 mem root → 拦截）与 `skill_ttl_expiry_forces_redecryption_on_reminder`（1s 短 TTL 配置，TTL 过期后重提产生新 token）——集成 8/8。

## 8. 续深 Review 记录（2026-07-31 v11）

1. **P1 共享 token 的缓存误删（已修复）**：内容去重使两个 skill 共享同一 token，skill 级 TTL 卸载其中一个时会无条件 `cache.remove`，导致另一个仍在 TTL 内的 skill token 失效。修复：`Registry::token_ref_count` 统计 token 存活引用，sweep 仅在无剩余引用时移除缓存项；新增对照测试（共享内容双 skill，evict 一个后另一个仍可重水合），crate 81/81。
2. **集成覆盖补全（compaction）**：新增 `compaction_request_rehydrates_encrypted_skill_content`——`Op::Compact` 触发的本地 compaction 请求体断言包含 framed 技能内容（`REAL_SKILL_CONTENT_MARKER` + `base_directory`）且不含 `/dev/shm` 路径，验证 spec「Rehydration covers all request paths」的 compaction 分支——集成 9/9。

## 9. 续深 Review 记录（2026-07-31 v12）

**集成覆盖补全（resumed-session）**：新增 `resumed_session_keeps_stale_skill_tokens_unreplaced`——turn 1 提及加密 skill 后，用同一 home + rollout 在新会话 `resume`（新 runtime 缓存为空），turn 2 断言请求体保留 stale token 占位符、不重水合为明文、无 `/dev/shm` 路径。至此 spec「Rehydration covers all request paths」的常规 turn、compaction、resumed-session 三个分支均有集成验证——集成 10/10。

## 10. 续深 Review 记录（2026-07-31 v13）

**审计 reason 细化（已修复）**：guard 的 `Blocked` 从单一字符串改为 `{ message, reason }`，审计事件区分拦截类型——`direct_read`（shell 读取命令）/ `search_probe`（搜索/探测命令）/ `file_view`（view_image）/ `export_plaintext`（apply_patch 导出）；新增 reason 断言单测，guard 13/13。全量 codex-core lib 回归 2135/2137（2 个已知环境/偶发失败，无新增回归）。

## 11. 续深 Review 记录（2026-07-31 v14）

**fork 集成测试调查结论（测试限制，已记录）**：尝试为 v1 spawn 增加 fork 隔离集成测试，经诊断确认：

- 父 rollout 的 token 消息格式正确（`<skill>[SENSITIVE_SKILL_TOKEN:...]</skill>`），`keep_forked_rollout_item` 过滤逻辑有 4 个直接单测覆盖（token 消息丢弃/普通保留/assistant 保留/工具项丢弃）；
- v1 spawn 的子会话请求在父 turn 完成后的测试窗口内不可靠到达；且父 followup 请求的历史包含 `spawn_agent` 的 FunctionCall（参数含子 prompt 文本），会干扰 `mount_sse_once_match` 的 matcher（捕获到父 followup 而非子请求）；
- 结论：fork 隔离的端到端集成测试在现有 suite 基建下不可靠，**移除该测试**，安全不变式继续由 spawn 单测保证；记录为已知测试限制，待 v1 spawn 时序稳定或专用 harness 后再补。

## 12. Spec 覆盖矩阵（2026-07-31 v15）

三个 spec 的全部 Requirement/Scenario 与测试锚点对照（U=单元测试，I=集成测试）：

### encrypted-skill-tokenization

| Requirement / Scenario | 锚点 |
|---|---|
| 解密 on mention / 磁盘 stub 不注入 | I `encrypted_skill_keeps_plaintext_out_of_context_and_rollout` |
| 重提及 TTL 内复用 | I `re_mention_within_ttl_reuses_the_same_token` |
| 解密失败 warning | U `encrypted_skill_decrypt_failure_produces_warning` |
| Token-only 注入 / 明文兼容 | I 上述 + U `plaintext_body_keeps_existing_format` |
| 重水合 chokepoint | I 上述 + U `request_input_rehydrates_encrypted_skill_tokens` |
| 重水合覆盖三路径（常规/compaction/resume） | I `compaction_request_rehydrates...` + `resumed_session_keeps_stale...` |
| Stale token 保留 | I resume + U `keeps_stale_token_when_content_missing` |
| 跨会话门禁 | U `refuses_cross_session_token` + runtime gate |
| 路径改写 / base anchor / 输出脱敏 | I `script_execution_rewrites...` + guard U |
| 信任层级框架 | I 请求 framing 断言 + U `framing_includes_skill_name_and_original_base_dir` |
| 内容双上限 | U cache（条目 64 + 8MiB） |
| 审计日志 | U audit/runtime events + I 审计文件断言 |
| Skill TTL 卸载 / Thread TTL 清空 / 线程结束 | U registry + I `skill_ttl_expiry_forces_redecryption...` + `clear_thread` 接线 |
| 多会话隔离 | U registry/cache 隔离 + rehydrate 门禁 |
| 路径隐藏（无 /dev/shm） | I 全部请求/rollout 断言 |

### encrypted-skill-access-control

| Requirement / Scenario | 锚点 |
|---|---|
| 直接读取拦截（shell + exec） | I `direct_read_of_decrypted_storage_is_blocked` + `exec_command_direct_read...` |
| 脚本执行放行 | I `script_execution_rewrites_original_path_and_redacts_output` |
| 工具参数路径改写 | U `rewrites_original_skill_path_in_execution_commands` + I 执行验证 |
| 输出路径脱敏 | U `redact_uses_the_runtime_mem_root` + I 无路径断言 |
| 文件查看工具拦截（view_image） | U `blocks_view_image_on_decrypted_directory` + I `view_image_on_mem_root_is_blocked` |
| 搜索/探测命令拦截（含 ls） | U `blocks_search_commands...` + `blocks_listing_the_runtime_mem_root` |
| 导出明文拦截（apply_patch） | I `export_tool_with_skill_plaintext_is_blocked` + U export guard |
| 回复明文脱敏（长行/前缀） | U `redact_exact_plaintext`/`redact_full_line_and_prefix` + `redacts_assistant_reply_plaintext_before_persistence` |
| 回复明文脱敏（短行完整行） | U `redact_short_line_quoted_as_a_complete_line` + `redact_short_line_is_idempotent` |
| 回复明文脱敏（不误伤 user/句子内片段） | U `redacts_only_assistant_reply_items` + `redact_short_line_embedded_in_a_sentence_is_kept` |
| inter-agent 明文脱敏 | U `redacts_plaintext_in_agent_messages` |

### encrypted-skill-fork-isolation

| Requirement / Scenario | 锚点 |
|---|---|
| fork 丢弃 token 消息 | U spawn_tests 4 例（token 丢弃/普通保留/assistant/function call） |
| 子代理重新提及 | 设计语义（集成受 v1 spawn 时序限制，见 §11） |
| 跨会话不可共享 | U registry/cache 隔离 + rehydrate 门禁 |

## 13. 最终回归与边界评估（2026-07-31 v16）

- **全量 codex-core lib 回归**：2135/2137——与上次完全一致（2 个已知环境失败：沙箱代理环境变量、tracing 并行偶发），guard reason 重构与 view_image 语义调整后**无新增回归**；
- **线程删除清理集成测试评估**：`thread/delete` 的 `clear_encrypted_skills` 接线（3 行）已编译验证，`clear_thread` 行为有单测；集成级验证需 app-server 子进程基建（TestAppServer + config.toml + skill 包 + 线程生命周期），成本/收益比不佳，记录为待扩展项；
- 最终验证矩阵（v15 覆盖矩阵 + 全量回归）确认三个 spec 全部场景有测试锚点，本地可执行验证全绿。

## 14. Windows 交叉编译验证与依赖瘦身（2026-07-31 v17）

- **Windows 目标验证成功**：安装 `x86_64-pc-windows-gnu` target 后，`codex-encrypted-skills` 交叉编译通过（此前因缺 Windows target 无法本地验证）；
- **依赖瘦身（必要前置）**：zip crate 的默认 features 启用了 zstd/bzip2/lzma（C 依赖，触发 cc-rs 交叉编译失败）；改为 workspace 统一 `default-features = false` + 各使用方（encrypted-skills/core/core-skills/core-plugins）显式 `features = ["deflate"]`；Cargo.lock 移除 bzip2-sys/lzma-sys 等（-68 行）；
- **本地回归**：encrypted-skills 81/81、core-skills 131/131、集成 10/10 全绿；core-plugins 359/360 的 1 个失败为**环境预存项**（测试断言技能列表，本机预装了 opencode-plugins 外部技能目录），与 zip 改动无关（startup_sync 全部 zip 测试通过）；
- 剩余记录项收窄：真实 SDK 收敛 `test_zip`、两个时序敏感集成测试待基建扩展。

## 15. 部署前提深度 Review 与修正（2026-07-31 v18）

依据部署前提（Docker、每用户一容器、非 root 无 sudo、无 SSH 仅 HTTP API、断外网/局域网推理、持久化常驻）与项目汇报（V1 方案与 V2 设计思路）逐项审查：

### 部署适配结论

| 部署前提 | 审查结论 |
|---|---|
| 每用户一容器 | mem root 的 per-process 命名空间（`p<pid>/`）已隔离容器内多进程；每容器独立 `/dev/shm`，无跨容器共享 |
| 非 root + 无 sudo | `/dev/shm` 容器默认 0777 可写 ✓；**bwrap user namespace 在容器内可能受限**——部署需验证；若沙箱不可用，guard 仍是主防线（命令混淆缓解依赖网络隔离，需容器级 `--network none` 或出口白名单） |
| 无 SSH、仅 HTTP API | 明文只在 `/dev/shm` + 请求瞬间 + rollout token；API 可见面（rollout/workspace）无明文 ✓；唯一执行面是模型工具调用（guard 拦截）✓ |
| 断外网 + 局域网推理 | 模型调用由 host 进程发起（不在 bwrap 内），沙箱 `network_access=false` 不影响推理服务 ✓；shell 外泄由 `--unshare-net`/容器网络策略阻断 |
| 持久化常驻 | V1 的「缓存长期驻留」已被两级 TTL + turn 边界 sweep + thread/delete 清理解决 ✓；本轮补充两个持久化适配点（见下） |

### 本轮修正（TDD）

1. **P1 `/dev/shm` 容量门控（已实现）**：Docker 默认 `/dev/shm` 仅 64MB，多 skill 并发可能占满。新增 `available_bytes`（Linux statvfs）与 `EncryptedSkillRuntime::set_min_free_bytes`（默认 4MiB，可注入）：解密前检查剩余空间，不足则失败（`Internal`）。新增 2 个测试（可用空间报告、空间不足拒绝）。
2. **P1 审计日志路径可配置（已实现）**：常驻容器重启后 `$TMPDIR` 审计丢失。`[encrypted_skills]` 新增 `audit_path`，Session 使用配置路径（默认仍为 temp dir）；config schema 已重生成。新增解析测试。
3. **P1 部署要求（文档）**：**加密 skill 包（`.zip.enc`）所在目录不得挂载/暴露给 OpenChamber 容器**（用户有 SSH 且 workspace 双挂载）——当前为模拟加密（deflate zip），用户拿到包即可解压出明文；真实 AES 加密接入后此风险消除。skill 包应只位于执行容器私有路径（如 codex_home/skills）。
4. **P2 记录在案**：bwrap 在容器内的可用性需部署实测；不可用时告警并依赖 guard + 容器网络策略。

### 验证

encrypted-skills 83/83（含容量门控测试）、config 229/229（含 audit_path 解析）、集成 10/10、Windows 交叉编译通过。

## 16. 部署配置示例与验证清单（2026-07-31 v19）

### 推荐容器 config.toml 片段

```toml
[encrypted_skills]
sdk = "unavailable"              # 真实 SDK 接入前保持 fail-closed
skill_idle_ttl_secs = 600        # Skill 层空闲 TTL
thread_idle_ttl_secs = 1800      # Thread 层空闲 TTL
audit_path = "/data/codex/audit/encrypted-skills-audit.log"  # 持久卷且非 root 可写，防容器重启丢失

[sandbox_workspace_write]
writable_roots = ["./", "/dev/shm/fm-agent-security"]      # shell 可执行解密脚本
network_access = false                                      # shell 断网；推理服务由 host 进程调用，不受影响
```

### 部署验证清单

1. **bwrap 可用性**：容器内执行 `bwrap --ro-bind / / /bin/true`；失败则需容器授权 user namespace，或接受 guard-only 并收紧容器网络（命令混淆绕过的缓解依赖网络隔离）；
2. **/dev/shm 容量**：`df -h /dev/shm`，建议 ≥ 256MB；容量门控默认 4MiB 余量（`set_min_free_bytes` 可调），多 skill 并发按需评估；
3. **/dev/shm 可写性**：非 root 用户可写入；初始化失败现在会输出 `failed to initialize encrypted-skill memory root` 告警（2026-07-31 修正静默失败）；
4. **skill 包位置**：`<name>.zip.enc` 只放执行容器私有路径（如 codex_home/skills），**不得**挂载/暴露给 OpenChamber 容器（用户有 SSH 且 workspace 双挂载）；
5. **审计持久化**：`audit_path` 指向**非 root 可写**的持久卷（如 `/data/codex/audit/`，避免 `/var/log` 等 root 目录），验证 10MB 轮转（`.1` 文件）与容器重启后审计保留；打开失败会输出 `failed to open encrypted-skill audit log` 告警（2026-07-31 修正静默降级）；
6. **网络策略**：容器 `--network none` 或出口白名单；局域网推理服务经白名单可达（模型 API 由 host 进程发起，不经 bwrap）。
7. **TTL 保洁语义**：两级 TTL 在 turn 边界与**每次模型请求**（重水合入口）双触发——常驻服务器上任何活动都会清理过期解密内容；完全无请求时内容驻留到容器重启（/dev/shm 自动清空），无残留跨容器；
8. **审计保留**：`audit_path` 指向持久卷，容器重启后审计保留；`/dev/shm` 内明文随容器销毁自动消失。
9. **双容器拓扑收口（汇报第三章）**：执行容器**不开 SSH**，`/dev/shm` 与 skill 包仅存在于执行容器且对使用者不可见；OpenChamber 容器（用户 SSH + VSCode）只经 HTTP(S) 与执行容器通信，**不得**挂载 skill 目录或 `/dev/shm`；Workspace/EDA 工具双挂载时确认不含加密 skill 包。汇报原文「/dev/shm 不是隔离边界：同容器同 uid 进程可直接读取」→ 必须以**容器文件系统 ACL + 网络策略**收口（skill 包目录权限 0700/非共享，`--network none` 或出口白名单）。

## 17. 与汇报 V2 设计差异核对 + 容量预检查（2026-07-31 v20）

### 与项目汇报 V2 思路的差异核对

| 汇报 V2 设想 | 本项目实现 | 结论 |
|---|---|---|
| 会话复用、一次解密多次复用 | TTL 内幂等复用（`load_or_register` cache-hit） | ✓ 等价 |
| TTL 卸载、每次委托续费 | 两级 TTL + 重水合命中续费（`touch`） | ✓ 等价且更细（Skill/Thread 双 TTL） |
| 内容缓存 SQLite 持久化（突破 64 条） | **有意保持内存缓存 + 硬上限（64 条/8MiB）** | 安全取舍：明文不落盘是核心要求，SQLite 持久化与其冲突；常驻容器不重启，内存缓存足够；容器重启后 token stale → 重新提及（fail-safe） |
| 硬件 Key 真实集成（PKCS#11/Ukey） | `EnvelopeSdk` trait 预留（`UnavailableSdk`/`TestZipSdk`） | 待 Ukey 接入；部署期保持 `sdk = "unavailable"` fail-closed |

### 本轮修正：解密前容量预检查

原容量检查在解密之后（entries 已进内存）——超大加密包会把 64MB `/dev/shm` 与内存先打满。新增 `check_capacity_for_package`：解密前按包文件大小 + 余量预检查，不足则拒绝且 **SDK 解密不会被调用**（新增测试断言 calls==0）。crate 84/84、Windows 交叉编译通过。

## 18. 常驻清理增强与容器生命周期（2026-07-31 v21）

### 请求级 TTL 清理（已修正）

TTL sweep 原只在 **turn 边界**（`build_skills_and_plugins`）执行——常驻服务器上用户长时间无新 turn 时，空闲线程的解密目录会驻留到容器重启。修正：`rehydrate_framed`（每次模型请求的重水合入口）先执行 `sweep()`，使**任何请求活动都会触发两级 TTL 清理**；活跃 token 由重水合续费（`touch`）保护，不会被误删。时序正确：sweep 先于重水合，过期内容被卸载后 token 自然 stale（重新提及恢复）。新增测试 `request_rehydration_enforces_ttl_for_idle_skills`（61s 空闲后重水合返回 stale token），crate 85/85。

### 容器生命周期说明

- **容器重启即 /dev/shm 清空**：Docker 销毁重建后 `/dev/shm` 是新挂载，解密明文自动消失（部署优势）；audit 日志依赖 `audit_path` 持久卷保留；
- 同容器内 codex 进程重启（app-server 崩溃拉起）：per-process 命名空间 + pid 存活清理处理残留（pid 复用极端场景下残留由容量门控与 audit 轮转兜底，风险已记录）。

## 19. 深度 Review 轮二：模型回复明文硬脱敏（2026-07-31 v22）

## 19.1 元数据必填字段的编译期修正（2026-08-01 v23）

`SkillMetadata`/`EnvironmentSkillMetadata`（core 模型）与 v2 协议 `SkillMetadata` 新增 `encrypted: bool` + `encryption: Option<SkillEncryption>` 后，Rust 结构体字面量必须显式给出新字段，全 workspace 共 13 处旧构造点报 E0063（ext/skills 5、tui 7、thread-manager-sample Config 1）。已全部修复：

- **明文默认语义**：`encrypted: false, encryption: None` 即「明文常规 Skill」，加载/执行走原有 `load_plaintext_skill` 路径（直接读 SKILL.md），`is_encrypted()` 为 false 时不进入加解密分支——明文 Skill 行为零变化；
- **wire 层缺省**：v2 协议字段已带 `#[serde(default)]`，JSON 缺省即明文；只有 Rust 字面量需要显式；
- **为何不用 `..Default::default()`**：`SkillMetadata` 含 `AbsolutePathBuf`、`EnvironmentSkillMetadata` 含 `PathUri`，均无 `Default`（需手写占位路径，语义差）；且展开语法会静默重置漏写的旧字段（隐患）。显式两字段是当前最小且清晰的修复；
- 验证：`cargo check --workspace --all-targets` 全绿；codex-skills-extension 15/15、codex-tui bottom_pane 758/758、composer_submission 40/40。

### 发现的问题（P1）

主 Agent 方案与 V1 executor 隔离的关键差异：`SKILL.md` 明文（带信任框架）直接注入 LLM 请求，`<output_policy>` 只是软约束。模型回复可能复述/引用明文，导致 **assistant 消息落盘到 rollout/内存历史，形成明文驻留**——违反「明文不允许长时间驻留」要求。V1 子代理边界天然避免，主 Agent 方案缺硬拦截。

### 修正（TDD，两笔提交）

1. **`82ffdcb649` Redact model-reply plaintext at the durable history boundary**：
   - `export_guard::redact_known_plaintext(text, known)`：完整明文、≥20 字符整行、20 字符前缀统一替换为 `[REDACTED]`；幂等（二次替换 no-op）。
   - `EncryptedSkillRuntime::redact_reply(session_id, text)`：按会话 `known_plaintexts` 做会话隔离脱敏。
   - `encrypted_skills_guard::redact_assistant_reply_items`：只处理 `ResponseItem::Message{role:"assistant"}` 的 InputText/OutputText 与 `ResponseItem::AgentMessage` 的明文 InputText（EncryptedContent 不动）；user 消息不处理。
   - 接线点在 `Session::prepare_conversation_items_for_history`（durable history 边界）：`record_conversation_items`（主回复/reasoning）与 `record_inter_agent_communication`（子代理通信）两条持久化路径统一覆盖；脱敏后的 items 同时写入 state、rollout 与客户端流。
2. **`9bacbf7fcb` Redact quoted short skill lines that appear as complete reply lines**：
   - 补盲：<20 字符的短行（如 `api_key=abc`）单独被引用时不命中长行规则；现按「完整行」语义脱敏（行首空白保留），嵌在句子中不误伤；幂等。

### 设计取舍（已记录）

- **客户端流同步脱敏**：`send_raw_response_items` 使用同一批 items，API 端用户看到的是脱敏后文本；理由：API 原文可能被前端二次落盘/透出，与「AI 侧零明文驻留」目标一致；代价是模型合法引用 Skill 内容的业务输出会被 `[REDACTED]`（Skill 输出策略本就禁止复述原文，属预期行为）。
- **工具输出/函数调用参数中的明文不脱敏**：skill 脚本可能合法输出密钥/令牌供模型下一步使用（如 `scripts/get_token.sh`），脱敏会破坏多步技能流程；`apply_patch` 等文件写出口已 Blocked，shell 命令中的明文属合法运行时使用。该残余路径（FunctionCallOutput 参数/输出含明文）与 V1 无差异（工具输出同样回流主 agent），已记录。
- **短行语义**：`contains_known_plaintext`（工具参数拦截）保持保守（<20 字符不匹配）；只有持久化脱敏使用「完整短行」激进语义。

### 回归验证

- encrypted-skills **95/95**、guard **16/16**、集成 **10/10**、`just fmt` + `just fix -p codex-encrypted-skills`/`-p codex-core` 全绿。
- 集成测试二进制出现 `tokio-rt-worker stack overflow`：经 `git stash` 复现，**与本次改动无关**（测试基建/环境栈大小问题）；`RUST_MIN_STACK=16MiB`（`just test` 默认 8MiB 不够）可稳定通过，已记录。

### 本轮复核的其他项（结论不变）

- **工具拦截已迁移到 Codex 工具体系**：`hook_names.rs` 的 `HookToolName` 是 Codex 自身 hook 契约——`shell_command`/`unified_exec::exec_command` 两个 handler 的 `pre_tool_use_payload` 都发出 `HookToolName::bash()`（序列化名 `Bash`），`apply_patch`/`view_image` 同理；guard 在 `tools/registry.rs` 工具分发前置按该类型化名称匹配，非 opencode 字符串匹配。exec_command 的 `cmd` 参数改写经 `with_updated_hook_input` 正确回写。
- **MCP/扩展工具放行**为文档化设计决策（外部信任边界）；部署清单补充建议：受限部署不启用带宿主文件访问能力的 MCP server，或容器层隔离 MCP 进程。
- **部署清单配置键**已对照 `config/permissions.rs`/`config/mod.rs` 复核：`[sandbox_workspace_write] writable_roots` 与 `network_access` 均为真实键名，示例 config 可直接使用。

### 复核中发现并修复的绕过（P1，`5ff15ef99e`）

`guard_shell` 原实现只对「命令串直接包含 mem root 或已知解密目录」的读/搜索命令拦截，路径改写（原目录 → 解密目录）发生在拦截判断**之后**。因此 `cat /skills/secret/SKILL.md`、`grep -r secret /skills/secret`、`ls /skills/secret` 等引用**原目录**的读/搜索命令会被改写为指向 `/dev/shm/.../fm_skill_security_<hex>/...` 后照常执行，直接绕过脚本源码隔离。

修复（TDD，先写 2 个失败单测复现 `Updated` 行为）：`guard_shell` 在改写后对读/搜索命令**二次检查改写结果**，命中 mem root/解密目录即 `Blocked`（reason `direct_read`/`search_probe`）；执行型命令（`bash .../build.sh`）不受影响，继续改写放行。guard 单测 18/18、集成 10/10。

### 残余风险记录（本轮复核新增）

- **`write_stdin` 无 pre-tool payload**：guard 无法拦截写入既有 shell 会话的键盘输入；缓解依赖路径随机化 + 输出脱敏（模型无法获知 `fm_skill_security_<hex>` 真实地址，`echo /dev/shm/*` 类探测输出被脱敏）。如后续要求严格拦截，需为 `WriteStdinHandler` 增加 pre payload（会改变全量 hook 可见性，属其他模块行为变更，当前不做）。
- **`mcp__*`/扩展工具路径通道**：参数含已知明文现已被 guard_export 拦截；但路径探测（如 `mcp__filesystem__list_directory("/dev/shm")`）仍放行，属外部信任边界；受限部署建议不启用带宿主文件访问能力的 MCP server，或容器层隔离 MCP 进程。
- **工具输出/函数调用参数中的明文**：skill 脚本可能合法输出密钥/令牌（多步流程需要），脱敏会破坏功能；`apply_patch` 文件写出口已 Blocked。

### 复核中发现并修复的落盘副本（P1，`64ebd25942`）

首轮回复脱敏只接在 `record_conversation_items`/`prepare_conversation_items_for_history`，E2E 增强（mock 回复引用 `REAL_SKILL_CONTENT_MARKER`）暴露同一句回复在 rollout 出现**两份明文**：

1. **`TurnItem`（`ItemStarted`/`ItemCompleted` 事件）**：`handle_non_tool_response_item` 把 `ResponseItem` 转成 `TurnItem::AgentMessage`，经 `send_event_raw_with_persistence` 落盘；
2. **`TaskCompleteEvent.last_agent_message`**：由 finalize 时的原文派生；
3. **`ResponseItem::Reasoning`**：reasoning summary/content 也经 `record_conversation_items` 落盘，原脱敏只覆盖 assistant Message/AgentMessage。

修复（TDD）：在**模型流入口** `handle_output_item_done` 对输出项先做 `redact_assistant_reply_item`（Message/AgentMessage/Reasoning 全覆盖），使 ResponseItem、派生 TurnItem、`last_agent_message` 全部在源头即干净；`emit_turn_item_started/completed` 保留 TurnItem 级二次兜底（覆盖其他 TurnItem 发射路径）。E2E 断言 rollout 只含 `[REDACTED]`；guard 22/22、集成 10/10、`just fmt` + clippy 全绿。

### 与部署汇报逐项核对：网络/扩展工具明文导出拦截（P1，`f7fa0b941a`）

汇报 V1 安全矩阵明确要求「`write` / `edit` / `apply_patch` / `webfetch` / `web_search` 参数含明文直接拦截」。核对发现 guard_export 此前只覆盖 `apply_patch`：独立 web_search（扩展工具 `web/run`，hook 平面名 `webrun`）与 MCP/扩展工具的参数明文均放行，模型可把 Skill 明文直接放入搜索/请求参数外泄。

修复（TDD，3 个新单测红→绿）：`before_tool_with_runtime` 改为——`Bash` → guard_shell、`view_image` → guard_read、**其余所有工具默认 guard_export**（参数含已知明文 Blocked，reason `export_plaintext`）；shell 保持唯一允许运行时使用明文的通道（脚本经路径改写执行）。Codex 无独立 `webfetch` 工具（盘点确认），未来新增网络工具自动落入默认拦截。guard 25/25、集成 10/10。

### V1 辅助层未移植项（记录，不阻塞）

- **入站意图检测（keyword/model 检测器，fail-open 辅助）**：V1 中为辅助层，主防线是 token 化 + 白名单 + 网络隔离；主 Agent 方案已用确定性工具拦截（guard）+ 明文脱敏替代，未移植模型检测器。如需接入可在 `agent/control` 消息入口加 fail-open 检测，不属加解密核心。
- **压缩链路约束**：V1 用 `session.compacting` prompt 约束「摘要不得写 Skill 明文」（软约束）；本项目压缩摘要取自**已脱敏的历史记录**（`record_conversation_items` 边界），是结构保证而非 prompt 约束，且压缩请求的重水合与回复脱敏走同一机制（`compact.rs` 摘要经 redacted history 提取，`RolloutItem::Compacted` 无明文）。
| 11 | **Windows 未覆盖** | 集成测试排除 Windows；seatbelt 等价规则未做 | 平台支持要求（Linux/macOS/Windows）未满 | 后续补 Windows/seatbelt 沙箱与测试 |

## 4. 环境注记

- `just bazel-lock-update` 因 GitHub 下载 v8 源码握手失败无法本地执行；workspace crate 不影响 Bazel 模块锁，CI 会校验。
- 本环境 `/tmp` 存在杂散 `.git` 目录，会导致 2 个既有 loader 测试失败（已移开验证归因），与本次改动无关。
- OpenSpec change：`openspec/changes/encrypt-agent-security-skills/`（proposal / design / specs / tasks 全部完成，strict 校验通过）。

## 20. npm 安装包（2026-08-02）

- **构建**：`cargo build --release -p codex-cli --bin codex`（约 10.5 分钟）→ `codex-rs/target/release/codex`（strip 后约 397MB，`codex --version` = `codex-cli 0.0.0`）。
- **打包**：按官方 `codex-cli/scripts/build_npm_package.py` 布局，将 `bin/codex.js` 与 `vendor/x86_64-unknown-linux-musl/bin/codex` 组织为本地 `@openai/codex` 包并 `npm pack` → `dist/npm/openai-codex-0.0.0-dev.tgz`（约 130MB；`dist/` 已 gitignore，不入库）。
- **验证**：`npm install --prefix <tmp> <tgz>` 后 `.bin/codex --version` = `codex-cli 0.0.0`。
- **说明**：本地二进制为 glibc 动态链接，置于官方 musl 名义目录（`bin/codex.js` 的 vendor 回退路径），适用于本容器/glibc 环境；正式发布需按官方 release 流程构建 musl 与多平台产物。

## 21. Codex 插件体系与源码修改说明（2026-08-02）

### Codex 插件体系

| 层 | 机制 | 能力 | 边界 |
|---|---|---|---|
| Extensions | `codex-extension-api`：ExtensionRegistry + contributors（config/turn_input/context/turn_item/thread_lifecycle） | web-search、skills 展示、image-generation、connectors 等外围能力；注册工具、上下文片段、生命周期事件 | 不能改写核心请求形成与历史持久化 |
| Hooks | `hooks.toml`：pre_tool_use / post_tool_use / session 事件 | 按工具名/字符串匹配拦截与改写工具入参 | 拿不到解密运行时状态（注册表、路径映射、明文缓存、TTL） |
| core-plugins | skills/plugins 分发与列举 | 安装/列举 | 不参与单次请求解密/注入 |

### 为什么插件无法实现需求

1. **注入与重水合在核心路径**：`build_skill_injections`（core-skills 内部）与 `Prompt::get_formatted_input_for_request`（client_common 内部），扩展 contributor 无法在“模型请求形成”边界做 Token→明文替换；
2. **工具拦截依赖会话注册表**：按“脚本文件目标 + 执行白名单”分级、路径改写、已知明文判断都需要 `EncryptedSkillRuntime` 状态；hooks 无此状态且只能字符串匹配；
3. **明文驻留面在 core 内部**：ResponseItem / TurnItem / task_complete / compaction / rollout 多处持久化副本分布在 session 与流处理内部，插件无法在每个落盘点做结构保证；
4. **需要新协议/配置类型**：`SkillMetadata.encrypted/encryption`、app-server v2 字段、`[encrypted_skills]` 配置，扩展 API 没有这些类型。

### 为什么修改源码

需求本质是在“模型请求形成”与“历史持久化”两个核心边界做**结构保证**（而非提示词/字符串级软约束），这两个边界不是 Codex 的扩展点。因此采用**独立 crate + 有限薄接线**：加解密逻辑全部收敛在 `codex-encrypted-skills`（不依赖 core），core 只保留注入、重水合、拦截、清理等少量接线点。

### 修改了哪些源码

| Crate / 目录 | 文件 | 改动 |
|---|---|---|
| `codex-rs/encrypted-skills`（新增） | token / sdk / mem_root / cache / registry / rehydrate / paths / export_guard / audit / runtime | 加解密核心：SDK、Token、两级 TTL、路径改写/反改写、脱敏、审计、周期 sweep |
| `codex-rs/core-skills` | loader.rs · injection.rs · skill_instructions.rs · environment.rs | frontmatter 加密标记、加密分支解密、Token 化 body |
| `codex-rs/skills` | model.rs | SkillMetadata / EnvironmentSkillMetadata 加密字段 |
| `codex-rs/core` | client_common.rs · session/mod.rs · session/session.rs · session/turn.rs · stream_events_utils.rs · encrypted_skills_guard.rs · encrypted_skills_periodic.rs · tools/registry.rs · codex_thread.rs · agent/control/spawn.rs · config/mod.rs · lib.rs | 请求重水合、持久化脱敏、runtime 构建与周期 sweep、工具拦截/输出脱敏、fork 过滤、配置接线 |
| `codex-rs/config` | config_toml.rs · core/config.schema.json | [encrypted_skills] 配置与 schema |
| `codex-rs/app-server-protocol` | v2/plugin.rs | SkillMetadata 可选加密字段 |
| 示例/测试/工具 | thread-manager-sample · ext/skills · tui · core/tests/suite/encrypted_skills.rs · Cargo.toml/Cargo.lock | 构造点补齐、集成测试、zip deflate-only |

## 22. 存储文件与格式一览（2026-08-03）

| 存储面 | 位置 | 格式 | 内容 | 明文策略 |
|---|---|---|---|---|
| /dev/shm（解密存储） | `/dev/shm/fm-agent-security/p<pid>/fm_skill_security_<hex>/` | 目录树（0700） | 解密包全部条目（SKILL.md、scripts、resources） | **明文（TTL 窗口内）**：600s/1800s 卸载 secure wipe、进程命名空间隔离、容量门控 |
| rollout | `<codex_home>/sessions/<yyyy/mm/dd>/<thread_id>.jsonl`（归档 `archived_sessions/`） | JSONL（RolloutItem） | ResponseItem、TurnContext、WorldState、Compacted、EventMsg | **零明文**：只存哨兵 Token 与 [REDACTED]（E2E 断言） |
| SQLite（state_db） | sqlite_home（默认 codex_home 下，可 `CODEX_SQLITE_HOME` 覆盖） | SQLite | 记忆、token 用量/预算、会话索引、rollout 引用索引、线程状态 | **无 Skill 明文**：明文不落盘是设计约束，缓存 SQLite 有意不用 |
| 审计日志 | 默认 `<temp>/fm_skill_security_audit.log`，可配置 `audit_path` | JSONL（10MB 轮转 .1） | decryption/tokenization/rehydration/blocked/cleanup | **无明文**：仅元数据 |
| 配置 | `<codex_home>/config.toml` | TOML | [encrypted_skills]、[sandbox_workspace_write] | 无明文 |
| 日志/遥测 | `<codex_home>/log/` | 文本 / OTLP | 工具调用、turn 元数据 | **注意**：改写后工具命令可能含解密路径（不含明文正文）；敏感字段脱敏未实现，属潜在写入面 |
| skill 包（原目录） | workspace `.agents/skills/<name>/` 或 `codex_home/skills/` | SKILL.md（stub）+ `<name>.zip.enc` | 加密包只读来源 | 密文：部署要求只放执行容器私有路径 |
| 内存（非磁盘） | EncryptedSkillRuntime cache / registry | 内存 | 明文缓存（64 条/8MiB）、TTL 注册表 | 明文（进程内）：TTL 卸载 + 周期 sweep |

## 23. 第三方安全复核与语义化拦截重写（2026-08-04 v23）

本节记录对前述实现的独立安全复核（P0–P2）及代码修复。复核发现 **访问控制层与 token 化层的设计前提自相矛盾**——guard 允许 shell 读取被 token 化保护的同一份 SKILL.md 明文，使整套 token 化 + 信任层级框架可被三条 shell 命令（`ls`→`ls`→`cat`）绕过。

### 23.1 P0：guard 放行 shell 读取 token 化保护的明文（已修复）

**问题**：`guard_shell` 初版（commit `491a567c62`）只拦截脚本扩展名，放行文本扩展名读取；`is_read_command`/`READ_COMMANDS` 已定义但**从未接入 guard**（死代码）。SKILL.md 是 `.md`，因此 `cat <decrypted>/SKILL.md` 放行 → 拿到**无框架明文** → 经工具输出进入内存历史（持久化脱敏明确保留内存原文 `redact_tool_output_plaintext_for_persistence`）→ 跨 turn 常驻 → token 化「明文只在请求瞬间存在」前提作废，`FRAMING_HEAD` 反注入防线被整段跳过。

**为何不能靠命令名修复**：`cp`/`mv`/重定向/glob 都不在读取命令列表内，命令名黑名单无法覆盖；`bash -c 'cat ...'` 可绕过执行白名单判定。因此改为**语义判定**。

**修复**：`guard_shell` 重写为**语义化仅执行（execute-only）模型**——
- 对命令按未引用链式操作符（`;` `|` `&&` `||` 背景 `&`）切段（`paths::split_command_segments`：引号感知 + 反斜杠转义感知 + `\n` 换行分隔 + `|&` stderr 管道），逐段判定；
- 先做原目录→解密目录路径改写，再逐段判断「引用解密存储且非脚本执行」即 Blocked（reason `non_execution_access`）；
- 删除命令名分类函数（`is_read_command`/`is_search_command`/`is_allowed_text_file`/`command_targets_script`/`file_targets`/`is_recursive_search`/`inject_script_exclusions` 及 `READ_COMMANDS`/`SEARCH_COMMANDS`/`TEXT_EXTENSIONS`/死代码 `command_basenames`）；
- `command_references_dir` 对 `MEM_ROOT` 常量做子串匹配，glob（`cat /dev/shm/.../p*/f*/SKILL.md`）与跨会话路径天然命中。

**覆盖逃逸向量**（均有单测）：`cp <dir>/SKILL.md /tmp`、`cat <dir>/SKILL.md > /tmp/leak`、`base64 <dir>/SKILL.md`、glob 探测、跨会话目录、`grep -r`、链式走私（`bash run.sh; cat SKILL.md`）逐段拦截。tirith (`sheeki03/tirith`) 的 `tokenize_posix` 提供了反斜杠转义/换行/`|&` 三处的参考边界，但其为 **AGPL-3.0** 且未发布 crates.io，无法链式依赖；仅借鉴算法思路按 codex 风格独立重写。

### 23.2 P1：`rg --exclude` 非法 + grep 泄 .md（随 P0 解决）

`inject_script_exclusions` 对 `rg` 统一追加 `--exclude`（`rg` 无此 flag，用 `-g/--glob`），且排除只覆盖脚本扩展名，`.md` 明文仍经 grep 进输出。改为 execute-only 后递归搜索对解密目录直接 Blocked，问题随 P0 消失。

### 23.3 P1：进程内跨会话明文可读（随 P0 解决）

guard_shell 的 block 判断从不引用跨会话目录成员。改为 execute-only 后，任何引用 `MEM_ROOT` 常量的段都被拦截，跨会话 `cat` 自然命中。

### 23.4 P2：fork 隔离丢弃整条 user 消息（已修复）

**问题**：`keep_forked_rollout_item` 对 `user` 消息用 `!contains_encrypted_skill_token(item)`，若 token 与用户正文同处一条消息则**整条被丢**，子 Agent 丢失用户指令。
**修复**：`user` 消息始终保留；fork 时对 user 消息逐 content item 调用新增 `token::strip_tokens`（哨兵→占位文本），保留正文。`contains_encrypted_skill_token` 辅助删除。

### 23.5 P2：tmpfs 上 secure_wipe 空操作（已修复）

`/dev/shm` 是 tmpfs（RAM），`overwrite_with_random` 随机覆写再删对内存文件系统无取证意义，反而可能把明文页推入 swap。`secure_wipe` 检测 `dir` 是否在 `DEFAULT_MEM_ROOT` 下，是则跳过覆写直接 `remove_dir_all`；非 tmpfs 回退路径（非 Linux `temp_dir()` 或自定义磁盘 root）仍覆写。

### 23.6 P2：无条件构造 EncryptedSkillRuntime（已评估，保留原状）

约 20 个调用点（guard、redaction、compaction、registry、turn、stream）依赖非可选 `&EncryptedSkillRuntime`/`Arc::clone` 契约；更重要是 **fail-closed 语义要求 runtime 即使在 SDK 不可用时也在场**（`load_or_register` 对 `UnavailableSdk` 返 warning）。故保留无条件构造，改 `Option` 属大面积表面改动换微弱内存收益。

### 23.7 验证与 Rebase 记录（2026-08-05）

- `codex-encrypted-skills`：**114/114** 通过；
- `codex-core` guard/spawn/fork 单测 + `core/suite` encrypted_skills 集成：**63/63** 通过（execute-only 拦截、token 化注入、重水合、TTL、fork 隔离、出口明文拦截全覆盖）；
- 完整 `just clippy --all-targets` 全 workspace 通过（零 error 零 warning）；
- **Rebase 到 `rust-v0.146.0`** 发行 tag（squash 为单 commit）：冲突聚焦在 `session/mod.rs`（`prepare_conversation_items_for_history` 返回类型从 tuple 改为 `Cow`）、`spawn.rs`（0.146.0 重构 fork 过滤，删除引用不存在的 developer-instruction 变量的 `retain_forked_item`，保留 token-strip 独立 for-loop）、`turn.rs`（补 `turn_extension_data` 参数）、过时 metrics 测试（依赖已删的 `extension_metrics`/`RecordingMetrics` API）删除。rebase 后**此前已知失败的 `resumed_session_keeps_stale_skill_tokens_unreplaced` 随上游恢复路径修复转绿**；
- `codex-core-skills` 135/137：2 个失败（`non_git_repo_skills_search_does_not_walk_parents`/`skill_roots_include_admin_with_lowest_priority`）为 git-worktree 环境性失败（测试假定非 git 目录，当前 `.worktrees/` 有父级 `.git`），与 encrypted-skill 无关。


## 24. 二次安全复核：交互式 shell 注入、app-server TOCTOU、锁中毒恢复（2026-08-06 v24）

在强制 bwrap 沙箱（`network_access=false`）+ 开源框架部署前提下的独立复核，发现并修复三个残留漏洞。

### 24.1 P1：write_stdin 绕过 guard 读取解密明文（已修复）

**问题**：`write_stdin` 工具（向已运行的交互式 shell 进程注入 stdin 字符）的 `pre_tool_use_payload` 有意返回 `None`（`write_stdin.rs`），理由是"启动 shell 的那次 `exec_command` 已过 guard"。但 spawn 时 guard 只检查了 `command="bash"` 这条启动命令，后续 `write_stdin` 注入的 `cat <skill路径>/SKILL.md` **完全不检查**——guard 只把住了进程的出生，没把住进程的一生。

**攻击链**（binds 激活时，即推荐配置 `network_access=false` 下成立）：
1. 模型调 `exec_command("bash")` → 无路径引用 → guard 放行
2. bwrap 内 `/skills/secret/` = 解密明文（`ReadonlyBind`，只读不影响 `cat` 读）
3. 模型调 `write_stdin("cat /skills/secret/SKILL.md")` → 不经 guard → bash cat 出明文 → stdout 回流 → 工具输出不脱敏（设计取舍）→ 明文进入内存历史

**危害边界**：网络外传被沙箱挡住（`--unshare-net`）；持久化（rollout/流）有回复层脱敏兜底；但**信任层级注入防线被击穿**——模型拿到无 `FRAMING_HEAD` 的裸明文，恶意 skill 的 prompt injection 可影响模型决策。

**修复**（`encrypted_skills_guard.rs` + `write_stdin.rs`，+85 行）：
- 新增 `guard_stdin_input(runtime, session_id, chars)`——复用 guard_shell 的分段+路径引用+明文导出检查，两个差异：①不改写路径（shell 文件系统视图在 spawn 时已固定）；②始终检查 original + decrypted 两套路径
- `write_stdin.rs::handle_call` 解析 `chars` 后、注入前调用 guard，Blocked 时返回错误并记审计
- 6 个新单测：原始路径 Blocked、解密路径 Blocked、无害命令 Allow、脚本执行 Allow、链式走私 Blocked、unengaged 放行

### 24.2 P1：app-server process/writeStdin TOCTOU（已修复）

**问题**：app-server 的 `process/spawn` 在 spawn 时检查 `ensure_not_engaged_unsandboxed`，但 spawn 时 unengaged 放行的进程会存活到会话 engaged 之后，此时 `process/writeStdin` **完全无 guard**——客户端可注入 `cat <skill路径>`。这是 spawn-time check 与 write-time execution 的 TOCTOU 窗口。

**修复**（`process_exec_processor.rs`，+13 行）：
- `process_write_stdin` 解码 base64 delta 后调用 `rpc_guard::ensure_command_not_guarded`，关闭 spawn/write TOCTOU

### 24.3 P1：只读查询互斥锁中毒后 fail-open（已修复）

**问题**：`runtime.rs` 的 9 个只读安全查询方法在 mutex 中毒时 `unwrap_or(false)` / `unwrap_or_default()` → guard 静默降级为完全无防护，且**永久不可恢复**（需进程重启）。`is_engaged`=false → guard 第一道门全开；`known_plaintexts`=空 → 回复脱敏也失效 → 明文落盘 rollout。

**设计决策**：采用 `PoisonError::into_inner()` 恢复而非 fail-closed。原因：
- 中毒后 `into_inner` 取回锁守卫继续用，不丢弃数据——Rust 内存安全保证 Mutex 保护的内存本身完整
- 安全关键路径（`is_engaged`/`known_plaintexts`/`decrypted_dirs` 等）全是只读 HashMap 查询，自身不可能 panic，中毒只能由外部并发操作触发
- fail-closed 会让常驻服务在中毒后**永久拒绝所有操作**直到重启——对"持久化常驻、无 SSH 仅 HTTP API"部署是不可接受的运维负担
- 代码已有先例：`runtime_registry()`（`runtime.rs:90`）就用 `unwrap_or_else(PoisonError::into_inner)`

**修复**（`runtime.rs`，1 文件）：
- 引入 `recover_lock(result)` helper（`unwrap_or_else(PoisonError::into_inner)`）
- 9 个只读查询方法改为 recover_lock：`is_engaged`、`known_plaintexts`、`decrypted_dirs`、`path_mappings`、`rewrite_paths`、`unrewrite_paths`、`touch`、`has_engaged_state`、`engaged_session_paths`
- 变更路径（`load_or_register_inner`、`clear_session`、`sweep`、`rehydrate_framed`）保持 `map_err(lock_error)` / `Err(_) => return`——对半更新结构应用变更是危险的
- `lock_error` 函数继续服务于变更路径（fail-safe on write）

**为何不用 parking_lot**：安全代码**故意用 `std::sync::Mutex`**——poisoning 语义本身是特性。`parking_lot` 不中毒，`recover_lock` 依赖的 `PoisonError::into_inner` 在那里无意义；安全代码需要"panic 检测锚点"。

### 24.4 已评估但不修的项（部署前提确认）

| 项 | 结论 | 原因 |
|---|---|---|
| test_zip/noop SDK 留在生产 config 枚举 | 不修（暂） | 待 Ukey 接入；当前 fail-closed 默认（Unavailable）是主路径 |
| SM4-CBC 无认证 | 不修 | 优先 HPKE(AES-256-GCM)；国密场景后续单独处理 |
| Remote Compact API 明文 | 不修 | 开源框架部署不支持 Remote Compact |
| 回复脱敏仅到子串匹配 | 取舍 | 当前实现到此；编码化外泄靠沙箱断网兜底 |
| /proc 私钥泄露 | 沙箱已关闭 | bwrap `--unshare-pid` + procfs PID-namespace-aware，host codex 的 memfd 对沙箱进程不可见 |

### 24.5 验证（2026-08-06）

| 套件 | 结果 |
|---|---|
| `fm-encrypted-skills` | **144/144**（含 `read_only_queries_survive_registry_poisoning`：故意毒化 registry mutex 后断言 `is_engaged`/`decrypted_dirs`/`known_plaintexts` 返回正确数据而非空） |
| `codex-core` encrypted_skills_guard/periodic | **85/85**（含 6 个新 stdin guard 测试） |
| `cargo clippy -p fm-encrypted-skills -p codex-core --lib --tests` | 零 error 零 warning（1 个 pre-existing rmcp-client 弃用警告无关） |
| `cargo fmt` | 干净 |

变更总量：4 文件 +166 行（write_stdin + app-server）+ 1 文件锁恢复（runtime.rs 重构，净增约 +30 行）。
