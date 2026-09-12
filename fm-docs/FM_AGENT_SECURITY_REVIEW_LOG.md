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
通过(R2Fixes 复核表实际 18 行,含 F4-sec compaction drain(compact.rs:787-795)与 M11 V1 spec 门控):F1-sec 接线、F2-sec paths 归一化(实现正确,语义盲区另立 G1)、M5 全分段、M6 首次尝试、F5-sec timeline(三路径全覆盖)、M1 全变体、M3 状态迁移+幂等、M9 覆盖面、F3-sec trace token 化(WS 增量退化为整体脱敏,方向安全)、M8 递归脱敏、M7 delta 全覆盖、M4 接线、M10 otel 路径、F6 边界、F7 stub 告警、M2 中央接线。「14/16」为 R2Fixes 自述口径,以其复核表为准。Round 1 的 17 项中未出现在复核表的 LOW 级项(如 L 级)无独立复核行。
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

---

## Process Hardening(debugoff → codex_process_hardening,用户指令)

按 grevo 分支做法,全仓移除 debugoff 注入,统一改用 `codex_process_hardening::disable_process_dumping()`。

### 变更(31 文件,+58/−163)
- **15 个二进制入口**:app-server-test-client、file-search、cli、windows-sandbox-rs(setup_main/command_runner)、thread-manager-sample、linux-sandbox、apply-patch、bwrap(×2 处)、exec、responses-api-proxy、stdio-to-uds、execpolicy、app-server — 删 `use debugoff;` 及 `debugoff::multi_ptraceme_or_die()`,原位替换为 grevo 同款:
  `codex_process_hardening::disable_process_dumping().unwrap_or_else(|err| eprintln!("WARNING: failed to disable process dumping: {err}"));`
  保留各处原有 `#[cfg(target_os = "linux")] #[cfg(not(debug_assertions))]` 门(debug 构建与旧行为一致为 no-op;失败 fail-open 仅告警,不会崩)。
- **tui**:fm 分支迁移时丢了注入点(仅剩死依赖),按 grevo tui/src/main.rs:53 同款补回 disable_process_dumping。
- **清单**:14 个 Cargo.toml 的 `debugoff` 依赖换/删;workspace Cargo.toml 删 `debugoff = "0.2"`;Cargo.lock 由 cargo 重新生成(−88 行,debugoff 及其依赖树消失)。process-hardening crate 本体与 grevo 完全一致,已在树中,零改动。
- **无 BUILD.bazel 引用**(grep 验证);Bazel 锁刷新仍按既有决策超范围。

### 验证
- 全仓 grep debugoff(codex-rs/ 下源码/Cargo.toml/Cargo.lock/BUILD.bazel)0 残留;**例外:仓库根 MODULE.bazel.lock 仍含 debugoff 元数据**(Bazel 锁刷新按既有决策超范围,功能无影响,Bazel 不用于本构建)
- 全部受影响 crate `cargo check` 0 错(**debug 构建**,证明 cfg 属性拓扑正确;曾修复两处机械替换事故:孤儿 cfg 属性对悬空到 `#[tokio::main]`/`use std::path::PathBuf` 导致 debug 构建丢 main — E0601,已清理)
- file-search debug 运行冒烟 OK;responses-api-proxy release 冒烟 OK(启动/参数/设计内报错路径正常)
- clippy(13 个受影响 crate --all-targets)0 错;cargo fmt 干净

### D14(by design):防调试/dump 机制选型
- 统一采用 `disable_process_dumping()`(prctl PR_SET_DUMPABLE=0 + RLIMIT_CORE=0,grevo 验证过的实现);弃用 debugoff crate 的 TRACEME 混淆方案(依赖第三方混淆分支,维护面大,且 `multi_ptraceme_or_die` 遇 ptrace 不可用环境可能 die)。二者互斥不叠加;保留各入口原有 cfg 门。
- 本环境无法直接观测 Dumpable/ptrace(容器屏蔽),以 debug/release 双构建 + 运行冒烟代替;机制行为由 process-hardening 上游语义保证。

---

## Round 3(定向复审,HEAD=5f8858ebad)

### 逐项结论
通过(7/8):G2(orchestrator engaged 布尔单源,重试必带沙箱;unix_escalation 同源)、G6(parallel.rs:81-89 全部 dispatch 错误单点 + registry redacted_log_payload)、G3(协议全量枚举无 FsMove/变体遗漏;lexical_normalize 在位)、G4(REMINDER 先于 flagged 原文;shared_file_sink 同源审计)、G5(输出侧 6 点全覆盖)、加固(16 入口 + 全树 debugoff 零残留)、G7(死 API 零残留)。
不通过:G1 — 复合语句盲区。

### 新发现(Round 4 修复项)
| ID | 严重度 | 问题 | 最小修复 |
|----|--------|------|----------|
| H1 | HIGH | split_command_segments 只收顶层 statement(is_top_level 排除祖先含 if/while/until/for/case/subshell/function_definition 的节点),复合语句整体不产 segment;guard 三入口无整命令兜底。PoC:`(cd /dev/shm/fm-agent-security/... && cat scripts/run.py)` 直接 Allow | 复合语句整体产出 segment(或递归子命令入段);被包裹的脚本执行因不可识别为 is_skill_script_execution 而 fail-closed 拒绝;补红→绿测试 |
| M1 | MED | cwd_shift 漏 popd → 虚拟 cwd 陈旧(`cd /dev && pushd /tmp && popd && cat shm/...` 漏判) | popd → CwdShift::Unknown(保守启发已兜底)+ 测试 |
| M2 | MED | globstar/extglob 未建模:`**` 被当单分量 zip 错位;`(` 不在 has_glob_meta(`shopt -s globstar; cat /dev/**/run.py` 漏判) | 锚定特判 `**` 吞任意多分量;`(` 入 has_glob_meta;补测试 |
| L1 | LOW | legacy 回退( tree-sitter 不可用时)分词未过 unquote_token,`/dev/s\hm` 绕过 | 分词后过 unquote_token |
| L2 | LOW | stream_events_utils.rs ~:313-322 tracing ToolCall preview 未脱敏(intake 早于 guard,被 block 命令明文片段落本地日志) | engaged 时过 redact_text_for |
| L3 | LOW(记录) | G3 词法不解析 symlink;FsRemove 整 codex_home 不拦(DoS 非绕过) | 记录,不修 |

已知限制确认:不可解析 cd 后无尾分量相对路径放行(Round 2 启发上限,钉死);D11/D12/D13 维持 by design。

**收敛判定:不收敛(H1 HIGH 在)。Round 4 修复 H1/M1/M2/L1/L2 后做定向复审。**

---

## Round 4(R3 新发现修复)— 全部完成 ✓

| ID | 修复 | 测试(先红后绿) |
|----|------|----------------|
| H1 ✓ | paths.rs split_command_segments 新增 7 种复合节点(if/while/until/for/case/subshell/function_definition)整体产 segment;is_top_level 同步放行 → 复合语句内子命令入段、cwd 链照常追踪;复合段首词为关键字/`(` 永不判 is_script_execution → 命中即 fail-closed 拒;guard 三入口零改动共享 segment 层;D9 顶层执行回归钉在位 | compound_subshell_guarded_read_is_flagged、compound_if_statement_guarded_read_is_flagged、top_level_script_execution_segments_unchanged_by_compound_support、blocks_guarded_read_inside_subshell_compound、blocks_guarded_read_inside_if_compound |
| M1 ✓ | cwd_shift 动词加 popd → Unknown → 保守尾启发兜底 | popd_resets_tracked_cwd_conservatively |
| M2 ✓ | glob_prefix_matches 递归锚定:`**` 吞任意多(含 0)目录分量;has_glob_meta 加 `(`(literal 先行,不放宽既有命中);extglob cd 目标保守回落本就由 cwd_shift `$`/反引号/`(` 覆盖(回归钉) | globstar_double_star_reaches_controlled_dirs、extglob_paren_cd_target_falls_back_to_tail_heuristic |
| L1 ✓ | legacy_command_references_dir 分词后逐词 unquote_token 再判 | legacy_scanner_unquotes_backslash_escapes |
| L2 ✓ | stream_events_utils 新增 redacted_tool_call_preview(engaged 时复用 registry redact_telemetry_text,提为 pub(crate) 一行可见性变更);非 engaged 严格透传 | tool_call_preview_redacts_for_engaged_session(core lib stream_events 13/13) |

### Round 4 验证
- fm-encrypted-skills 220/220(serial,较 Round 3 +9 测试);core check/clippy 0 错;fmt 干净

### 合并树 core lib 已知失败钉死清单(2026-09-11 最终运行,10 项)
7 基线:agent::control::tests::{send_input_submits_user_message, spawn_agent_can_fork_parent_thread_history_with_sanitized_items, spawn_agent_creates_thread_and_sends_prompt}, session::tests::{interrupting_compaction_fallback_retains_last_known_step_context, user_shell_commands_do_not_inherit_managed_network_proxy}, session::turn::tests::post_sampling_token_estimate_is_disabled_by_always_on_sinks, tools::handlers::multi_agents::tests::{send_input_accepts_structured_items, send_input_interrupts_before_prompt}(8 项);
3 负载 flake(隔离运行通过):environment_selection::tests::blocking_snapshot_waits_for_starting_environment、guardian::tests::guardian_ephemeral_retry_preserves_parallel_trunk_and_fork_history、thread_manager::tests::injected_models_manager_controls_refresh_policy。合并验证出现其他名字即回归。R2 期间多出的 unified_exec::stdin_approval_preserves_the_reviewed_terminal、config::multi_agent_v2_* 为负载 flake,隔离复跑通过。
注:测试计数无落盘工件,复现命令即上文各节所列命令。

---

## Round 5(收敛复审,HEAD=7160bb5aae)— ✅ 收敛

Round 4 五项修复(H1/M1/M2/L1/L2)逐点验证全部通过:
- H1:正常复合命令((ls && pwd)、for、if)无误拦;redirect/pipe 嵌套无行为漂移;D9 无冲突(复合整段首词关键字永不入 RUNNERS,per-segment guard 先行 Block;approvals D9 三断言在位)
- M2:`**` = globstar 递归语义,方向保守只增拦;`/dev/**` 真阳性;`(` 入 meta 无误拦
- M1/L1:无回归,纯加宽匹配
- L2:engaged 双源判定,非 engaged 零拷贝透传,复用 redact_telemetry_text 单一来源
- 组合 PoC(不可解析 cd × glob × 复合 × redirect)无新漏判

### 新发现(记录性质,不重开循环)
| ID | 级别 | 描述 | 处置 |
|----|------|------|------|
| N1 | MED | 裸 `**` 相对模式在 engaged 会话一律 fail-closed 拦截(无视已追踪安全 cwd 与 globstar 开关) | 记为 D15 已知限制 |
| N2 | LOW | 虚拟 cwd 不建模子壳作用域(cd /a && (cd /b && cat x) && cat y 以 /a/b 判定) | 不可达 miss,仅产生保守 FP;可选注释 |
| N3 | LOW | globstar 钉测注释与运行时锚定依据不同源(结论一致) | 可选改注释 |

**收敛判定:无新的放行侧缺陷,循环终止。**

### H1 fail-closed 已知未量化面
复合语句内触及受控路径且非 D9 顶层脚本形态的命令一律拒绝;R5 仅验证了良性复合命令无误拦与 D9 顶层回归,复合+受控+非脚本的误拦率未量化,依赖使用反馈再调。

### D15(by design):裸 `**` 相对模式保守拦截
engaged 会话中含 `**` 分量的相对 glob 模式一律视为可能命中受控目录而拦截(fail-closed);不区分 globstar 开关、不豁免已追踪的安全 cwd。代价是极小面误拦(模型改用单层 `*` 即可),收益是 globstar 语义不确定性下零漏判。

---

## 审查循环总结(5 轮)
| 轮次 | 内容 | 结果 |
|------|------|------|
| R1 | 并行 5 方向初审 | 83 冲突分类 + 17 项修复(F1-F5) |
| R2 | 修复复核 + 3 残留面 | 14/16 通过;新发现 HIGH×2/MED×2/LOW×2 → G1-G7 修复 |
| R3 | G1-G7 + 加固定向复审 | 7/8 通过;复合语句盲区 HIGH + 2 MED |
| R4 | 盲区修复(H1/M1/M2/L1/L2) | 全部落地,fm-encrypted-skills 220/220 |
| R5 | 收敛复审 | 全部通过;收敛 ✅ |
| 加固 | debugoff → disable_process_dumping | 16 入口,全树零残留 |

---

## 分支差异三方审计(fm vs fm-0.154.0,回答「是否涵盖 0.146→0.154」)

文件级集合分解(diff 内容级):
- A = diff(0.146, fm)=338 文件;B = diff(0.146, 0.154)=4020;X = diff(fm, fm-0.154.0)=4077
- **B ⊆ X(0 例外)**:上游 0.146→0.154 改动的每个文件,两分支间都有差异 → X 在文件级完全涵盖上游演进
- 冲突文件 A∩B=166:逐字保留 fork 旧版而丢上游改动 = **0**;完全取上游(丢 fork 内容)= 49(全部对上 by-design:GREVO 品牌/spawn model 覆盖/mcp-server/core-skills 相关);真合并适配 = 117
- fork 独改 172:逐字移植 122 + 适配移植 50(fm/encrypted-skills、fm/license 主体原样)
- 两边都没动、本分支新改 = 7(orchestrator_tests、exec/main_tests、encrypted-skills/build.rs、审查日志 + 3 个测试残留已清理)

### 上游观察(新增,不修)
- rust-v0.154.0 `core/tests/suite/unified_exec_zsh_fork_approvals.rs:61/:126` 用 `tempfile::tempdir_in(std::env::current_dir())` 把临时目录建进 crate 目录;测试进程被杀时 TempDir 析构不跑 → `.tmp*/secret.env` 残留仓库并被 git add 扫入。已从本分支删除 3 个残留并加 `codex-rs/core/.gitignore`(`.tmp*/`)防复发;上游代码不动。

---

## Round 6(升级后新消息通路专项,双路 security-reviewer 独立复审)

背景:用户要求专项复审 0.146→0.154 升级后 App Server 新消息传递途径。R6Protocol(协议面盘点)+ R6Transport(传输/会话路径)两路独立审查,交叉确认 2 项。

### 结论总览
干净面:thread/start/resume 初始输入(guardrail ✓)、v1 legacy 面(已消亡,handle_client_request 单漏斗全过门)、出站通知全景(约 80 变体,含模型文本家族全部继承发射点脱敏)、exec-server 传输(Noise+强制 wss,无明文流)、timeline 搜索面(⊆ 可见面)。

### 新发现与修复项(R7)
| ID | 级别 | 问题 | 修复方向 |
|----|------|------|----------|
| R6-1 | **HIGH**(双路确认) | 0.154 新增 ext/queue 持久化队列绕过 license 门:①ThreadQueueAdd/Update/Reorder 不在 message_processor.rs:995-1016 门名单(失活仍可入队);②idle 自动派发链 on_thread_idle→dispatch_if_idle→start_turn_if_idle(ext/queue/src/service.rs:405-467,549-565)零 license 检查(全仓 is_active 仅 RPC 门/exec/tui 三处)——失活后队列被逐条自动启动为新 turn,架空 M1 | dispatch_if_idle 与 service::start 加 fm_license::is_active() 检查(失活保留队列项);ThreadQueueAdd/Update 补入门名单 |
| R6-2 | **HIGH** | command/exec RPC(0.154 与 process/spawn 同代新面)对 fork 执行防护完全缺席:engaged 无整体门(对照 process/spawn 的 ensure_not_engaged_unsandboxed);start 仅 legacy 文本扫描(command_references_dir,非 segment 级);write 无 stdin 守卫(process/writeStdin 有);Unix 路径解构丢弃 sandbox 裸 pty spawn(command_exec.rs:148-157);输出 base64 不脱敏;不在 license 门名单。engaged 进程内任一 v2 客户端可经 start+stdin 交互读取解密目录明文(F1-sec 场景在新面重现) | engaged 时 command/exec 整体拒绝(镜像 process/spawn 同款门,最小且封 stdin/输出/沙箱全部子面);补 license 门名单;Unix 丢 sandbox 为上游 bug 记录不修 |
| R6-3 | MED(双路确认) | realtime initialItems(thread_realtime_start_inner :1244-1252)与 appendSpeech(:1383-1400)绕过 G4 的 guardrail 扫描与审计,仅 appendText 被覆盖——同一面三类入口审计可视性不一致 | G4 助手抽公共函数,initial_items 逐条与 speech 文本复用(soft 缓解语义) |
| R6-4 | LOW | bedrock setup 疑似旁路 engaged 配置变异门(R6P-3,细节截断) | 修复代理核实后按实修/记录 |
| R6-5 | LOW | tracing 卫生:realtime startup context 整包 info!(realtime_context.rs:126)、客户端文本 debug!(realtime_conversation.rs:1769/1787);内容脱敏继承、本地信任域,同 L2/M10 先例 | engaged 时过 redact 或降 trace!(顺手) |
| R6-6 | 记录 | timeline 的 realtime TranscriptSegment 不受 token-hide 约束(与 AgentMessage 同级可见,策略一致);上游 command/exec Unix 丢 sandbox | 观察项:realtime 上游若引入未脱敏内容源,session/mod.rs:2412-2420 持久化点即成泄露汇 |

### Round 7 修复状态 — 全部完成 ✓
| ID | 修复 | 测试(先红后绿) |
|----|------|----------------|
| R6-1 ✓ | ext/queue dispatch_if_idle + service::start 加 license 探针(QueuedItemService 持 license_active fn-pointer,默认 fm_license::is_active,测试可注入;失活保留队列项,恢复自然续派);license 门名单补 ThreadQueueAdd/Update | ext-queue: license_inactive_dispatch_retains_queue_until_recovered、license_inactive_start_retains_queued_submission;app-server: license_lost_blocks_queue_add_and_update |
| R6-2 ✓ | command_exec 入口加 ensure_not_engaged_unsandboxed(engaged 整体拒,同 process/spawn,封 stdin/输出/sandbox 全部子面);license 门补 command/exec 与 write | command_exec_blocked_while_engaged_even_for_benign_commands、license_lost_blocks_command_exec_and_write |
| R6-3 ✓ | G4 guardrail+审计+REMINDER 逻辑抽公共助手,initial_items(v3)与 appendSpeech(v2)复用 | realtime_guardrail_flags_initial_items_and_injects_reminder_when_engaged、realtime_guardrail_flags_speech_and_injects_reminder_when_engaged |
| R6-4 ✓ | 确认属实:BedrockSetup 与 account/login 的 AmazonBedrock{,AccessKeys} 变体经 configure_bedrock_provider 直写 config.toml,绕过 engaged 配置变异门 → 全部补进 ensure_config_mutation_allowed | rpc_guard_blocks_bedrock_setup_when_engaged |
| R6-5 ✓ | realtime startup context/客户端文本日志:engaged 时过脱敏 | 随 R6-3 套件覆盖 |

### Round 7 验证
- app-server license_/rpc_guard/command_exec/realtime_ 91/91;clippy(app-server/queue-extension/core --all-targets)0 错;fmt 干净
- **测试环境注记(2026-09-11 勘误,实验定论)**:codex-queue-extension 的 queue_service 测试在默认 2MiB 测试线程栈下栈溢出 —— **纯 upstream rust-v0.154.0 干净检出同样溢出,加 8MiB 通过**(临时 worktree 实验证实);上游三个 CI workflow 均全局设 RUST_MIN_STACK=8388608(.github/workflows/rust-ci*.yml)掩盖了它。**非 fm 造成,无 fm 侧可优化项**;此前「fm 使 future 增大」的归因有误,予以更正。动作(已落地):codex-rs/.cargo/config.toml 增 [env] RUST_MIN_STACK=8388608(与上游 CI 对齐;该文件本为上游跟踪文件,hunk 极小),本地 cargo test/just test 不再需要手工 export;guardian 深栈用例同此约定。已验证:默认 shell 环境下 queue_service 溢出用例转绿。对照实验另证:短路 apply_external_guardrail 不改变溢出,guardrail HTTP 链不在该测试的 poll 路径。

---

## Round 8:skill 加解密与 license 的 feature 开关(离线构建)

需求:另一办公地无法访问内部 GitLab(192.168.131.126:8089,lmclient/fmsh-ukey),默认构建不得拉取。

### 实现
- **feature 设计**:`fm-encrypted-skills` 增 `ukey` feature(转发 3 个 ukey 依赖)、`fm-license` 增 `lmclient` feature(转发 lmclient),**default = []**;GitLab 依赖全部 optional = true——未激活即不 fetch。
- **降级语义(用户确认)**:无 feature = 「当作这两个功能不存在」,恢复基础 Codex 行为——license `is_active()=true`/`ensure_active()=Ok`(门全放行,非无证拒绝);skills 走既有非 engaged 直通路径(等同上游),加密技能不可解密。公共 API 签名不变,**core/app-server 等下游零改动**。
- **SDK 使用面**(已收敛):encrypted-skills 仅 `sdk.rs`(963 行,已有 EnvelopeSdk trait + UnavailableSdk/Noop stub 接缝);license 仅 `license.rs`。cfg 边界 + 新增 `ukey_available()`(供跨 crate 测试运行时跳过)。
- **链接面**:cli/Cargo.toml 增 `ukey` feature(转发 wrapper 依赖 + 两个 fm crate feature),build.rs 按 CARGO_FEATURE_UKEY 跳过 SDK 链接;全仓仅 cli/build.rs 与 fm/encrypted-skills/build.rs 引用 DEP_FMSH(已核实)。
- **发布管线**:release/build-fm-cargo.sh 与 release/Dockerfile.cargo 的构建命令补 `--features ukey`(否则产物静默降级);bazel BUILD(cli/license/encrypted-skills)按 v8-poc 先例补 crate_features/deps_extra。
- **便捷入口**:.cargo/config.toml [alias] `fm-check`/`fm-test`(带双 feature)。

### 双模式验证矩阵
| 项 | OFF(默认) | ON(--features fm-license/lmclient,fm-encrypted-skills/ukey) |
|----|------------|-----------|
| cargo tree 依赖图 | lmclient/fmsh 均 0 引用 | 完整 |
| workspace check(**--offline**) | 0 错(离线可建 = 无 GitLab 等价证明) | 0 错 |
| fm-encrypted-skills 测试 | 214(门控子集) | 220/220 |
| fm-license 测试 | 1(门控子集) | 12/12(bypass) |
| core encrypted_skills 集成 | 20/20(运行时跳过) | 20/20 |
| app-server license_/rpc_guard/realtime_ | 64/64(运行时跳过) | (ON 由既有轮次覆盖) |
| clippy(两 crate + codex-cli) | 0 错 | 0 错 |

### 用法
- 另一办公地(离线):`cargo build/test --workspace` 原样即可
- 本地全功能:`cargo fm-check` / `cargo fm-test`,或显式 `--features fm-license/lmclient,fm-encrypted-skills/ukey`;`-p codex-cli --features ukey` 一站式转发

---

## Round 8 勘误(用户在另一办公地实测反馈)

**「OFF 模式 --offline 0 错 = 离线可建」的证明不成立**:该验证在 warm git 缓存上运行,掩盖了 resolve 行为。用户在干净机器上 `cargo fetch`/`cargo check` 实测仍拉取 GitLab。

### 根因(cargo 机制,非配置问题)
- Cargo.lock 是**全量声明图**(feature 无关),optional git 依赖始终在列;
- 任何 cargo resolve(fetch/check/build)都要读 lockfile 内 git 包的 manifest → 缺 checkout 就 fetch;
- 实验实证(grevo worktree,insteadOf 屏蔽 GitLab + 空 git 缓存):①裸 `cargo fetch` 拉全量 lockfile;②`cargo check -p codex-exec`(feature 关)照样 "Updating git repository" ×2;③**手工剪掉 lockfile 条目后 cargo 回填并重新 fetch**——声明了 optional git 依赖就必然参与 resolve,无 flag 可绕。

### 结论:feature 开关解决的是「编译与链接」解耦,不解决「拉取」
真正离线可建的两条路(需用户决策):
1. **镜像(推荐,零代码)**:把 `jiangzhengqi/fmsh-ukey-lib` 与 `jiangzhengqi/lmclient-rust-sdk` 镜像到双方办公地都可达的 git 服务(如托管本仓库的服务),改 workspace Cargo.toml 4 个 URL;或另一办公地一行 git 配置指向镜像:`git config --global url."<镜像URL>".insteadOf "http://192.168.131.126:8089/"`。
2. **vendor 进仓库**:4 个 crate(+ linux SDK 库,checkout 实测 199MB/7.7MB,可裁剪平台)转 path 依赖入库,彻底离线,代价是仓库体积与 SDK 库入库的合规确认。

feature 开关仍有价值:无 feature 构建不**编译/链接** SDK(另一办公地能出全量功能的非 SDK 产物),但「不拉取」必须靠镜像或 vendor。

---

## Round 9:placeholder path crate 方案(采纳 Codex 建议,替代 strip 脚本)

strip 脚本方案有硬伤:仓库清单里仍写着内网 URL,fresh clone 不跑脚本、第一次 `cargo fetch` 照样失败。采纳 dummy path crate 方案:**公共清单里根本不存在内网 URL,resolver 解析到的 source 就是本地路径**。

### 实现
- `codex-rs/vendor/dummy/{fmsh-ukey-core,fmsh-ukey-sdk-wrapper,fmsh-ukey-skill,lmclient-rust-sdk}`:同名同版本 placeholder crate,`compile_error!` —— 默认构建(feature 关)根本不编译它们;误开 ukey/lmclient 时编译失败并明确提示跑 `release/use-real-fm-deps.sh`。
- 根 Cargo.toml 的 4 个 workspace 依赖从 `git = "http://192.168.131.126:8089/..."` 改为 `path = "vendor/dummy/..."` —— **内网地址从公共清单中消失**。
- `release/use-real-fm-deps.sh`(内部切真依赖)+ `release/use-dummy-fm-deps.sh`(切回 placeholder);切换会改写 Cargo.lock,切换态的 lock 不入库。
- strip-fm-deps.sh 方案废弃移除(824cc1ea2e 已 reset 丢弃)。
- **入库的 Cargo.lock 为 placeholder 形态**(零内网 URL;git 源条目消失)。内部切真依赖会本地改写 lock,勿提交切换态 lock;提交回 placeholder 用 release/use-dummy-fm-deps.sh。

### 双模式验证(最终版,干净 git 缓存 + insteadOf 屏蔽 GitLab)
| 命令 | 结果 |
|------|------|
| `cargo fetch`(R8 之前失败的命令) | **exit 0**,仅拉 github 上游依赖 |
| `cargo check -p codex-exec` | Finished |
| `cargo test -p fm-encrypted-skills`(OFF) | 基线全绿 |
| use-real 后 `cargo test -p fm-encrypted-skills --features ukey` | 220/220 |
| use-real 后 `cargo test -p fm-license --features lmclient`(bypass) | 12/12 |
| use-dummy-fm-deps.sh 恢复 | 工作树回到已提交状态 |
