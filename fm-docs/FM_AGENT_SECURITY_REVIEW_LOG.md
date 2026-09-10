# FM Agent Security — Review/Fix 循环记录

分支:`fm-0.154.0-agent-security-review`(基于 `fm-0.154.0` = 11c75bd753,rust-v0.154.0 基线)。
每轮 Review→Fix 独立 commit。本文件是**唯一事实源**:修复前先核对本文件,避免把 By design 当问题、或重复修复同一问题。

## By design(设计决定,不是问题 — 修复前必读)

| # | 决定 | 原因 |
|---|------|------|
| D1 | 加密 skill 的 SKILL.md 在磁盘上是 stub;注入时正文替换为 sentinel token,请求构建时(`client_common::EncryptedSkillRehydrator`)才解密 rehydrate | 解密时机=请求构建,落盘只有 token;明文仅存在于内存目录(`resolve_default_mem_root()`,默认 `/dev/shm/fm-agent-security`) |
| D2 | `readonly_binds`/`ReadonlyBind` 旧机制已删除。0.154 无 source→target bind;改为 guard 恒定改写原路径→解密路径(`guard_shell` 只有一种模式),并向沙箱 profile 追加 `FileSystemSandboxEntry{access: Read}`(`orchestrator::grant_encrypted_skill_read_access`) | 上游沙箱策略模型不支持 bind;entry + bwrap 原生 Read 处理等效 |
| D3 | `guard_shell/guard_read/guard_export` 不再有 `binds_active` 参数 | 0.154 无 bind 概念,恒为"改写"模式 |
| D4 | 工具输出仅在 `engaged` 时包 `RedactingToolOutput`;非 engaged 会话保持原始输出类型 | 非 engaged 时无可泄露明文;且包装会破坏下游对具体类型的检查 |
| D5 | spawn_agent 不支持 model 派发:`SpawnAgentArgs` 无 `model` 字段、`requested_model=None`、item `model=None`、spec 门禁 `expose_spawn_agent_model_overrides` 强制 false(config 默认也 false) | 用户明确要求:不支持派不同模型去干活 |
| D6 | license check-in 信号线程收到 SIGINT/SIGTERM 只 `check_in_now()`,**不** `process::exit`;退出交由宿主自身的优雅排空(app-server 要 drain 在飞 turn),`LicenseGuard` drop 幂等 | exit 会以 143 抢杀 app-server 的 SIGTERM drain(ACP 客户端用 SIGTERM 关闭) |
| D7 | `codex mcp-server`、`codex-core-skills`、GREVO 品牌、spawn_agent model 覆盖均已随迁移删除,不保留 | 上游已删 / 用户明确不要 |
| D8 | `thread_id_child_asserts_skip` 测试会移除 bypass 做**真实 checkout**(消耗席位)。本地跑 fm-license 用 `FMSH_CODEX_LIC_TEST_BYPASS=1 --skip thread_id_child` | License 服务端席位数有限 |
| D9 | engaged 会话的 execute-only skill 脚本执行自动放行(`approvals.rs` 顶部,`ReviewDecision::ApprovedForSession`),不弹窗不进 guardian;FMSH FPGA 插件脚本额外经 `is_auto_approved_plugin` 全量预批 | fm 产品决定:turn 内部执行不对用户/审查者暴露 |
| D10 | 测试运行方式:`fm-license` 需 `FMSH_CODEX_LIC_TEST_BYPASS=1` 且 skip thread_id_child;codex-core/app-server 需 `LD_LIBRARY_PATH` 指向 UKey SDK(本机已装 `/usr/local/lib`),guardian/agent 深栈用例需 `RUST_MIN_STACK=8388608`(或直接用 `just test`) | 环境,非代码 |

## 已知上游问题(只记录,不修)

(暂无 — 发现 Codex 本身严重问题时记录在此)

## Round 1(评审完成 → 修复中)

评审:5 个并行 scout(时机 R1Timing / 沙箱 R1Sandbox / 脱敏 R1Redact / 拦截 R1Intercept / 升级缺漏 R1Migration),transcript 见 history://R1*。

### Round 1 修复项(F1-F5 分工)

| ID | 严重度 | 问题 | 修复归属 |
|----|--------|------|----------|
| F1-sec | HIGH | write_stdin 交互式 stdin 完全绕过 shell guard(guard_stdin_input 已有实现+测试但零调用点) | F2(core tools) |
| F2-sec | HIGH | 路径前缀文本匹配可被 `/dev/./shm`、`/run/shm` 符号链接、`$TMPDIR` 规避;guard_read(view_image)无 MEM_ROOT 兜底,图片明文直出 | F1(fm guard/paths) |
| F3-sec | HIGH | rollout-trace / inference trace / v1 compaction trace 会把 rehydrate 后的明文落盘(CODEX_ROLLOUT_TRACE_ROOT 开启时),违反 D1 | F3(core 请求/compaction) |
| F4-sec | HIGH | compaction `drain_to_completed` 将模型输出未经脱敏直接写 rollout | F3 |
| F5-sec | HIGH | thread/timeline(SQLite 分页路径)缺 hide_reasoning 过滤,仅 items/list 覆盖 | F4(app-server) |
| F6-sec | HIGH | TUI/exec 请求边界缺 `ensure_active()`(license 心跳丢失后 TUI 不阻断,违背 lib.rs 文档) | F3 |
| F7-sec | HIGH | 非 glibc stub `is_active()` 恒 true 且无告警(musl 产物 license 门禁静默消失) | F5(license crate) |
| M1 | MED | app-server license 门枚举缺 ThreadQueueStart/ThreadCompactStart/ThreadRealtimeAppend* | F4 |
| M2 | MED | engaged 期间配置可经 fs/writeFile、ExternalAgentConfigImport 等旁路改写(rpc 门是名单制) | F4(最小方案:名单扩充/engaged 时守护 codex_home 配置) |
| M3 | MED | `check_in_now()` 归还席位后 LICENSE_STATE 仍 ACTIVE,drain 期间门禁继续放行 | F5 |
| M4 | MED | guardrail 只查 UserInput::Text;inject_items/InterAgent 通道不经过 | F3 |
| M5 | MED | D9 自动放行用 `.any()`:混合命令(skill 脚本段 + 需审批段)整单免审 | F2 |
| M6 | MED | I6 用 `initial_sandbox != None` 判定,executor-managed 沙箱(ShellSnapshot/remote)下误判 → 合规部署 engaged 会话全拒 | F2(改用 sandbox_requested) |
| M7 | MED | RespondToModel 错误文本、Reasoning 三种 delta、非 AgentMessage OutputTextDelta 未脱敏 | F3 |
| M8 | MED | redact_response_item 跳过 McpToolCallOutput/ToolSearchOutput | F1 |
| M9 | MED | redact_guardian_request 不处理 justification/cwd/guardian_cwd | F4 |
| M10 | MED | registry 遥测 log_payload 含改写后 /dev/shm 路径;preview 仅路径级脱敏 | F2 |
| M11 | HIGH→MED | V1 spawn spec 仍广告 model(spec_plan V1 分支 expose=true);config 用户 true 仍被采纳(D5 只关了 V2) | F2(V1 分支)|
| L1 | LOW | mem_root 内文件 0644,应收紧 0600 | F1 |
| L2 | LOW | guard_stdin_input 注释残留 binds 措辞(随 F1-sec 清理) | F2 |
| L3 | LOW | guard_shell/guard_stdin_input/is_skill_script_execution 三处 guarded 集合不一致(抽 shared helper) | F1 |
| L4 | LOW | FMSH 预批未校验 plugin root 不在可写 root 下 | 记录,暂不修 |

### By design 确认(评审提出但维持原设计)
- `FMSH_CODEX_LIC_TEST_BYPASS` release 生效:部署文档明示的产品级开关(无 LicenseServer 环境用)。**已知接受**;`CODEX_THREAD_ID` 仅存在性检查属同一信任模型,记录为已知限制。
- guardrail fail-open(3s 超时放行):软缓解语义,合理;strict 模式列为后续可选项。
- CLI plugin 门对只读 list 也封锁:产品从宽拦截,维持。

### 上游观察(只记录,不修)
- write_stdin 不触发 PreToolUse hooks 是上游语义(fork 的 stdin 守卫需自行接线,即 F1-sec)。
- bwrap 全盘只读分支的 `--bind-try /dev/shm` 疑为上游所有;full-read 分支带 fm 注释的为 fork 增补。

### 修复状态(Round 1 完成)
- F1(fm guard/paths):F2-sec ✓(paths 归一化:点段/`/run/shm`/`$VAR`;guard_read 加 MEM_ROOT_PARENT)、M8 ✓(McpToolCallOutput/ToolSearchOutput 脱敏)、L3 ✓(guarded_paths_for_session 统一)、L1 ✓(0600)。测试 6 新增全过;套件 211/211(serial)。
- F2(core tools):F1-sec ✓(write_stdin 接 guard_stdin_input,Blocked 拒绝)、M5 ✓(D9 改为全分段均为 skill-script 才放行,混合命令不再免审)、M6 ✓(I6 判据改 sandbox_requested)、M10 ✓(遥测 log_payload+preview 走 unrewrite→redact)、M11 ✓(V1 spec expose=false 并真正门控 model 属性;config 默认 false)。新增 6 测试全过;core tools 442 过。
- F3(core 请求/compaction):F3-sec ✓(inference/v1-compaction trace 记录 rehydrate 前 token 版;v2 路径本就安全)、F4-sec ✓(compaction drain 入 rollout 前脱敏)、F6-sec ✓(tui/exec 提交边界 ensure_active,FORCE_LOST 拒绝启动)、M4 ✓(InterAgentCommunication 文本过 guardrail;ResponseItem 注入记审计)、M7 ✓(非 AgentMessage delta + Reasoning 三种 delta 过 redact_text_streaming)。新增 6 测试全过。
- F4(app-server):F5-sec ✓(timeline 路径挂 hide_reasoning)、M1 ✓(补 Queue/Compact/RealtimeAppend* 变体)、M2 ✓(engaged 时 ExternalAgentConfigImport 拒、fs/writeFile 写 codex_home 配置拒;unengaged 不变)、M9 ✓(guardian request 的 cwd/guardian_cwd/justification 脱敏)。新增 4 测试全过。
- F5(license):M3 ✓(check_in_now 后 mark_license_lost,门禁即时关闭;幂等)、F7-sec ✓(非 glibc stub 首次调用告警一次)、M11 ✓(config 默认 false 断言钉死;spec_plan/multi_agents_spec V1 部分由 F2 完成)。

### Round 1 附带修复
- fm-encrypted-skills rpc_tests 两个用例断言进程级 any_engaged,并行必炸(基线即有,40 次中 26 次):加 RPC_TESTS_LOCK 串行化。
- config_tests `multi_agent_v2_exposes_model_overrides_by_default`:上游旧默认(true)的测试 → 按 D5 改为断言默认 false + 显式开启时门禁机制仍工作。
- M7③ 残留:`tools/events.rs` dispatch 路径的 "unsupported call" 回显仍记录未脱敏错误文本(F2 所有权外) → **Round 2 处理**。

### Round 1 验证
- workspace check 0 error;clippy(全部涉及 crate)0 error;fmt 干净
- fm-encrypted-skills 211/211(serial);fm-license 12/12(bypass,skip child);codex-core lib 2467 过/10 败(7 基线 + 3 负载 flake,隔离全过);codex-app-server 失败集 ⊆ 基线
- tui composer 333 过;codex-exec 78 过


---

## Round 2(复审完成 → 修复中)

复审:R2Fixes(Round 1 修复正确性逐条复核)+ R2Residual(三个遗留面)。结论:**16 项修复中 14 项干净通过**,M2 部分通过;paths 归一化实现正确但有 shell 语义层盲区。

### Round 1 修复复核结论(全部有据)
通过:F1-sec 接线、M5 全分段、M6 首次尝试、F5-sec timeline(三路径全覆盖)、M1 全变体、M3 状态迁移+幂等、M9 覆盖面、F3-sec trace token 化(WS 增量退化为整体脱敏,方向安全)、M8 递归脱敏、M7 delta 全覆盖、M4 接线、M10 otel 路径、F6 边界、F7 stub 告警、M2 中央接线。
部分通过:M2(FsCopy/FsRemove 漏 + `..` 词法穿透 → Round 2 MED-1)。

### Round 2 新发现与修复项(G1-G3 分工)

| ID | 严重度 | 问题 | 修复归属 |
|----|--------|------|----------|
| G1-sec | HIGH | paths 词法守卫的 shell 语义绕过:①glob 元字符顶替受控组件(`/dev/s*/...`);②词中引号切分(`/dev/sh"m/fm"-agent-security/...`);③cd 链 cwd 状态(`cd /dev && cd shm && cat ...`,分段独立判定全放行)。后果:execute-only 被破(脚本源码不在 known plaintexts 内) | G1(fm guard/paths) |
| G2-sec | HIGH | I6 只盖首次沙箱尝试:escalation 重试以 sandbox_requested=false 无沙箱执行明文脚本(engaged+WorkspaceWrite+D9 免审组合);unix_escalation 的 Unsandboxed 分支同样无 engaged 检查 | G2(core tools) |
| G3-med | MED | M2 残口:rpc 门漏 FsCopy/FsRemove;is_codex_home_config_file 的 `..` 词法穿透 | G3(app-server) |
| G4-med | MED | realtime 文本输入(Op::RealtimeConversationText)绕过 guardrail,无 REMINDER 也无审计 | G3(app-server) |
| G5-lm | LOW-MED | inference trace **输出侧**(client.rs:2267 items_added → record_completed)原样落盘模型输出(engaged 时可能含复述明文),无脱敏无审计 —— F3-sec 只修了请求侧 | G3(core client.rs) |
| G6-low | LOW | ①events.rs/parallel.rs 错误回显家族绕过 RedactingToolOutput(片段级明文+解密路径片段落 rollout/trace);②stream_events_utils.rs:318 tracing 用未脱敏 payload | G2(core tools) |
| G7-low | LOW | session_guard 的 redact_turn_item / redact_tool_output_plaintext_for_persistence 零调用点(死防御层,易误认为有保护) | G1(删除并记录) |

### By design 补充(Round 2 确认)
- D11:WS 增量 delta 的 guard-redacted 视图不含哨兵 token,trace 回放/重水合映射不可重建 —— 保真度取舍,机密性方向安全,接受。
- D12:guardrail 为软缓解(命中仍放行 + 注入 REMINDER + 审计);strict 模式列为后续可选,当前不做。
- D13:failure_response 错误回显的泄露下限=片段级明文+解密路径片段(before_tool 在 dispatch 前已 Block 含完整明文的参数;exec 工具输出仍经 RedactingToolOutput)。

### 修复状态(Round 2)— 全部完成 ✓
| ID | 修复 | 测试(先红后绿) |
|----|------|----------------|
| G1 ✓ | paths.rs:glob_component_matches(分量级 fnmatch,前缀锚定受控目录才命中;/dev/* 不误拦)+ strip_shell_quotes(去引号归一后重跑全部检查)+ segment_guarded_flags(cd/pushd 虚拟 cwd,含 .. 弹栈;目标不可解析时退化为受控尾分量启发) | glob_tokens_anchored_on_guarded_dirs_are_flagged、unanchored_glob_tokens_are_not_flagged、quote_split_guarded_paths_are_flagged、cd_chain_resolves_relative_guarded_reads、cd_chain_dotdot_moves_virtual_cwd、unresolvable_cd_target_falls_back_to_tail_heuristic、unresolvable_cd_target_ignores_unrelated_relative_paths、cd_chain_relative_skill_script_stays_executable_for_d9 + guard 层 4 个 |
| G2 ✓ | I6 选(b):unsandboxed_allowed 追加 !session_is_engaged(orchestrator.rs,与 :314 同源;同时盖住 retry_sandbox_requested 与 codex_linux_sandbox_exe);unix_escalation determine_action 同门,shell_request_escalation_execution 增加 engaged 参数 → RequireEscalated 降 TurnDefault 而非 Unsandboxed | engaged_session_sandbox_denial_does_not_retry_unsandboxed、engaged_sessions_never_escalate_to_unsandboxed |
| G6 ✓ | ①failure_response 持 session,engaged 时经 redact_text_for 单点覆盖全部 RespondToModel 回显;②registry 新增 redacted_log_payload(未知工具/不兼容 payload 两处 dispatch 错误 otel 路径) | failure_response_redacts_decrypted_paths_when_engaged、dispatch_error_telemetry_redacts_decrypted_paths_and_plaintext |
| G3 ✓ | ensure_config_mutation_allowed 补 FsCopy(destination)/FsRemove/FsCreateDirectory;is_codex_home_config_file 双侧词法归一(CurDir 剔除/ParentDir 弹栈)后再 strip_prefix | is_codex_home_config_file_resolves_lexical_traversal、rpc_guard_blocks_config_file_fs_mutations_when_engaged |
| G4 ✓ | turn_processor realtime 文本:guardrail enabled 时 is_attack,命中记审计 + 在 flagged 文本前注入 developer-role REMINDER(软缓解,同 M4 语义;审计经 shared_file_sink 同进程共享 sink) | realtime_guardrail_flags_text_and_injects_reminder_when_engaged |
| G5 ✓ | client.rs map_response_events/stream 增 encrypted_skills 参数;**全部 6 个**输出侧 trace record 点(cancelled×3/completed/failed×2)过 redact_all_response_item_text;会话侧 intake 不变(同 D1 请求侧模式) | trace 测试(encrypted_skills suite,红→绿) |
| G7 ✓ | 删除 session_guard.redact_turn_item / redact_tool_output_plaintext_for_persistence 死防御链(全仓零调用;guard.rs 自由函数+独占辅助+12 个死测试+21 个失效 import) | 全仓 grep 零残留;crate 211 测试全过 |

**G3 勘误**:工单示例 `$CODEX_HOME/../x/.codex/config.toml` 归一后在 home 外,本就允许(合法);真正被堵的是 `$CODEX_HOME/sub/../config.toml` 回拼写法。AbsolutePathBuf 反序列化即解析 `..`,RPC 路径 `..` 到不了守卫。

### Round 2 验证
- workspace check 0 错;clippy(fm-encrypted-skills/fm-license/codex-core/codex-app-server/codex-exec --all-targets)0 错(修掉 G1 引入的 2 处 needless_borrow + 1 处 useless_format)
- fm-encrypted-skills 211/211(serial);fm-license 12/12(bypass,skip child);encrypted_skills 集成 20/20;core lib 2469 通过/12 失败(10 个既有基线+2 个负载 flake:stdin_approval_preserves_the_reviewed_terminal、multi_agent_v2_does_not_expose_model_overrides_by_default 隔离运行均过;基线 7 + guardian_ephemeral_retry/isolated 过往 flake)