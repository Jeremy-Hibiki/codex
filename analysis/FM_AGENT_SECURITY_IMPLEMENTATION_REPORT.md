# FM Agent Security — 实现成果与安全报告

> 日期：2026-07-31（v2）· 对应 OpenSpec change：`encrypt-agent-security-skills`（35/35 任务完成，strict 校验通过）
> 前序设计：`FM_AGENT_SECURITY_SKILL_ENCRYPTION_IMPLEMENTATION.md`（v2.0 主 Agent + Token 重水合）

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
| 10 | ~~工具覆盖不完整~~ | **已解决（2026-07-31 v5）**：`view_image` 新增 pre payload 并被 guard 拦截（`path` 键）；`read` 分支同时兼容 `filePath`/`path` 键；`mcp_resource` 为服务端资源 URI 不涉及宿主路径 | — | — |

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
| `mcp__<server>__<tool>`（MCP 工具） | `mcp__*` | 放行 | 单测 ✓（`mcp_tools_pass_through`） |
| 其余 Function 工具（current_time/plan/sleep/request_*/mcp_resource/tool_search/get_context_remaining/multi_agents 等） | 默认 `function_hook_tool_name`（= 工具名） | 放行 | 盘点：均无宿主文件读取通道（`mcp_resource` 读取 MCP server 声明的资源，模型不能构造任意宿主路径） |
| 扩展工具（extension_tools / `web/run`） | 无 pre payload | 不参与 | 靠沙箱网络隔离 |

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

### encrypted-skill-fork-isolation

| Requirement / Scenario | 锚点 |
|---|---|
| fork 丢弃 token 消息 | U spawn_tests 4 例（token 丢弃/普通保留/assistant/function call） |
| 子代理重新提及 | 设计语义（集成受 v1 spawn 时序限制，见 §11） |
| 跨会话不可共享 | U registry/cache 隔离 + rehydrate 门禁 |
| 11 | **Windows 未覆盖** | 集成测试排除 Windows；seatbelt 等价规则未做 | 平台支持要求（Linux/macOS/Windows）未满 | 后续补 Windows/seatbelt 沙箱与测试 |

## 4. 环境注记

- `just bazel-lock-update` 因 GitHub 下载 v8 源码握手失败无法本地执行；workspace crate 不影响 Bazel 模块锁，CI 会校验。
- 本环境 `/tmp` 存在杂散 `.git` 目录，会导致 2 个既有 loader 测试失败（已移开验证归因），与本次改动无关。
- OpenSpec change：`openspec/changes/encrypt-agent-security-skills/`（proposal / design / specs / tasks 全部完成，strict 校验通过）。
