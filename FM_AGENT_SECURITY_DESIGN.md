# FM Agent Security 设计文档（实施契约）

> 状态：分析/设计阶段，未实施。本文档是后续实施的唯一依据；实施过程中遇到歧义先回读本文档，不得自行改变语义、放宽边界或跳过清单项。所有“决策点”在实施前必须按第 13 节确认，未确认前不得实施对应部分。

## 1. 防护目标与不可违背的不变量

本文档描述“加密 Skill（encrypted skill）环境”下的 Agent Security 护栏。任何实现都必须同时满足以下不变量：

- I1：不得把 Skill 明文内容（SKILL.md、资源文件、脚本内容、解密后的任何字节）泄露给**模型上下文之外的任何面**（前端展示、工具输出、RPC 响应、持久化、日志/telemetry、hooks、外部进程）。模型上下文是唯一允许出现明文的面（这是 Skill 生效的前提），且必须与 engaged 判定一致：会话未 engaged 时连模型上下文也不得出现明文（例如 fork/resume 出的未 engaged 会话历史不得残留明文）。
- I2：不得把明文路径（`/dev/shm/...` 真实解密路径）泄露给任何模型/用户可见面；对外只允许出现逻辑技能路径或 `[REDACTED]`。
- I3：未 engaged（见第 3 节）时，Codex 行为与不存在加密技能时完全一致，零行为变化；不得拦截 `/dev/shm` 字符串，不得对输出做任何红act。
- I4：只要明文实际存在（或即将在本会话产生），护栏必须已经生效；判定基于 live 状态，不存在“明文在、护栏关”的空窗。
- I5：engaged 时，任何无沙箱执行面不得被用来读取明文：要么强制沙箱，要么拒绝该请求。
- I6：产品化构建**强制启用沙箱**，禁止用户关闭沙箱或把沙箱降级到无沙箱/full-access；engaged 时若有效沙箱策略不满足强制级别，拒绝解密与执行。
- I7：产品化构建**禁止用户安装/加载插件**（含 marketplace、插件提供的 hooks/tools/skills/apps），因为插件可注册各类 Hook、拿到工具输入输出，扩大泄露面；用户配置的 **MCP server 允许保留**（MCP 不能注册 Codex 的 Hook，暴露面限于工具参数/输出，已由 guard 与红act覆盖；MCP 进程与用户同 uid 属威胁模型外）。

“模型/用户可见面”的定义：模型上下文（history）、工具输出（stdout/stderr/错误）、reasoning、审批与评审 prompt、app-server RPC 响应与通知、client stream 事件、持久化（rollout、compaction trace）、hooks 附加上下文、telemetry 与日志（日志至少不得包含明文/路径原文，允许记录 redacted 摘要）。

## 2. 术语

- **AgentSecurityContext**：本设计引入的上下文类型，包含 `Arc<EncryptedSkillRuntime>` 与 `session_id`（thread id），以及按需派生的 `engaged` 判定；以 `Option<AgentSecurityContext>` 形式传递，`None` 表示该面不需要护栏。
- **engaged**：本会话当前处于“加密 Skill 环境、有泄露风险、需启用 Agent Security 路线”的状态。判定必须由 runtime 状态派生（见第 4 节），不维护独立标志位。
- **mem-root**：Linux 上为 `/dev/shm`；解密目录位于 `mem-root/fm_skill_security_<hex>/p<pid>/...`。
- **解密目录（decrypted dir）**：某个已加载 skill 的明文落盘目录，registry 中每个 `SkillRecord.dir`。
- **逻辑技能路径（original dir）**：加密包所在目录（`SkillRecord.original_dir`），模型在 engaged 环境下只能看到它；它是沙箱内 bind 的目标路径。
- **Agent Loop**：模型工具调用循环，即 turn 内 `tool call → before_tool → 执行 → 输出处理 → 历史/持久化` 的链路。
- **RPC 面**：app-server 的客户端直连 JSON-RPC（ACP/SDK/前端可调用），不经模型工具循环。

## 3. 状态模型：Session 级状态 + 每 Turn 决定翻转

状态（runtime、registry、明文目录）属于 Session/Thread 级，物理上建议把 `EncryptedSkillRuntime` 提升为**进程级共享服务**（registry 内部已按 `session_id` 隔离，mem-root 初始化本就进程级），这样 Agent Loop 与 RPC 面都能按 `thread_id` 解析到同一实例。`Session.services.encrypted_skills_runtime` 改为引用该共享实例；不允许出现同一 thread 多份 runtime。

每个 Turn 开始时，TurnContext 组装当次生效的 `Option<AgentSecurityContext>`，即“每 Turn 决定是否翻转”。翻转条件必须绑定“本会话当前是否实际存在明文（或本 turn 即将触发解密）”，而不是“本 turn 是否显式使用了 skill”。

判定时机规则：guard 的每个判定点必须读 **live session 状态**（现场调用 `runtime.is_engaged(thread_id)`），不得依赖 turn 开始时的快照；TurnContext 的快照仅用于决定哪些面需要初始化/传递上下文，不构成安全判定的唯一依据。原因：app-server 允许同一 session 并发多个 turn，turn A 触发解密后，若 turn B 拿着旧的 false 快照裸跑，会出现明文在、护栏关。

未 engaged 时 `Option<AgentSecurityContext>` 为 `None`，所有护栏路径直接透传（见第 11 节）。

## 4. is_engaged 派生与 in-flight 窗口

`is_engaged(session_id)` 定义为：`registry 非空（decrypted_dirs/known_plaintexts 非空）` 或 `该 session 存在解密 in-flight`。

in-flight 窗口定义：`load_or_register` 从 `decrypt_package()` 之后、`registry.register()` 完成之前的时段；其中 `write_package_entries()` 完成即明文已落盘，而 registry 尚未登记。实现必须在 `load_or_register` 开始时把 `session_id` 加入 runtime 的 per-session in-flight 集合，结束（成功或失败）时移除；失败路径必须 `secure_wipe` 并移除 in-flight。并发加载同一 (session, skill) 的已有 gate 保留。

生命周期映射：首个 skill 加载 → engaged；turn 结束销毁明文（若采用 turn 级，见第 13 节决策 D1）→ 下一 turn 从 false 开始；TTL 空闲清理 → 自动 false；`clear_thread`/会话结束 → false；进程退出后的 stale 目录由下次 mem-root init 清理。

## 5. 接入面清单与处置

### A. Agent Loop（必须全部接入）

- `before_tool` 的 shell guard（bash）：路径检查、明文检查、segment 切分、script-execution 判定、IO 通道检查；重写模式见第 8 节。
- 技能脚本执行（execute-only 放行）在 engaged 时**自动 Permit、不向用户征询**（决策 D9）：加载加密 Skill 后的 Turn 内执行过程不展示给用户，审批 UI 与评审面都不出现该命令。
- `before_tool` 的 read guard（view_image）：受保护路径拦截。
- `before_tool` 的 export guard（所有非 shell 工具）：明文片段拦截 + 受保护路径探测拦截。
- `RedactingToolOutput`：所有工具输出统一 unrewrite + 路径红act + 明文红act。
- assistant 消息与 reasoning 的红act：明文红act（现有）必须扩展为路径红act，且仅在 engaged 时启用。
- 持久化面（rollout、compaction trace、fork 文本）：明文 + 路径红act（现有保留，改为 engaged 门控）。
- guardian/approval 评审：`GuardianApprovalRequest` 与网络 trigger 的命令/程序参数必须红act（I23 已实现，保留）。

### B. RPC 面（必须新增接入）

以下方法在 engaged（live）时一律先过共享判定器（第 6 节），未 engaged 直接放行：

- 读/枚举/元数据：`fs/readFile`、`fs/readDirectory`、`fs/getMetadata`、`fs/watch`。路径位于 mem-root、解密目录、逻辑技能路径下 → Block；`fs/watch` 的通知内容同样走红act，engaged 时拒绝 watch 受保护路径。
- 写/改：`fs/writeFile`、`fs/createDirectory`、`fs/copy`、`fs/remove`。目标路径位于受保护路径下 → Block；参数内容含明文片段 → Block。
- 执行：`command/exec`（含 write/resize/terminate）、`thread/shellCommand`、`process/spawn`（含 write/resize/kill）。命令按 shell guard 规则检查；`thread/shellCommand` 与 `process/spawn` 明确无沙箱，engaged 时默认拒绝（决策 D2，见第 13 节），不得仅依赖字符串检查。
- 上下文注入：`thread/inject_items` 参数含明文片段或受保护路径 → Block。
- 历史/读取类：`thread/read`、`thread/turns/list`、`thread/items/list`、`thread/searchOccurrences`。读取持久化（已红act）数据时应放行，但必须验证持久化红act完整性；若任何读取面可能触达 in-memory 明文（加载中的 thread），engaged 时须红act或 Block。
- 展示/元数据类：`thread/name/set`、`thread/goal/set`、`thread/goal/*`、`thread/metadata/update`。engaged 时参数含明文片段或受保护路径 → Block（模型可能把 skill 摘要写进名字/目标）。
- 终端类：`thread/backgroundTerminals/*` 的输出流必须走与工具输出相同的红act链路；未实现前视为暴露点（TODO-8）。`thread/realtime/*` 明确排除（产品不提供该能力，见 D10 范围说明）。

### C. 其他外发面（评估后接入）

- permission request hooks、PostToolUse 附加上下文：明文 + 路径红act（现有 source 红act保留，改为 engaged 门控）。
- `ExecApprovalRequestEvent.command` 等发给客户端的审批事件：engaged 时发送 unrewrite + 红act后的命令。
- telemetry/analytics：**已确认存在原始预览外发点**——工具结果在包装 `RedactingToolOutput` 之前先取 `log_preview()` 并交给 `otel.log_tool_result_with_tags`，原始输出预览可能含明文/路径；engaged 时必须先红act再记录（TODO-8）。
- telemetry/log：engaged 时命令与输出先红act再记录。
- 模型 provider 请求体：模型上下文允许含明文，但用户可配置自定义 `base_url` 使明文经过用户控制的服务器——属同 uid 威胁模型外，产品文档需注明，不在代码强制范围。

## 6. 共享判定器（checker）契约

所有护栏共用同一套判定逻辑，禁止 Agent Loop 与 RPC 面各写一套。建议形态：

`check(runtime, session_id, input, kind) -> Allow | Block{reason, message} | Rewrite{value}`，其中 `kind` 区分：`ShellCommand`、`FilePath`、`ToolArguments`、`OutputText`、`ClientCommand`。

规则（与现有实现一致并收敛）：

- 明文片段：≥20 字符、归一化（NFKC + 小写 + 去符号 + 去空白）后匹配；命中 → Block（reason 含 `plaintext`）。
- 受保护路径集合（仅 engaged 时非空）：mem-root、所有解密目录、所有逻辑技能路径。命中规则：
  - 输出文本：unrewrite 解密目录 → 逻辑路径，再对残余 mem-root 前缀红act；不得原样透传。
  - 文件路径类（读/写/元数据/watch）：位于集合内 → Block（`storage_probe`/`file_view`）。
  - shell/客户端命令：按 segment 切分；引用集合内路径时，仅“script-execution 且避免 guarded IO”放行（Agent Loop 内）；RPC 面是否放行 execute-only 由决策 D3 决定。
  - 工具参数（非 shell）：含明文或引用集合内路径 → Block。
- Block 的返回消息必须是通用文案，不得回显被拦的路径或内容。

## 7. RPC 面强制点与上下文可达性

强制点在 app-server 处理器层：`fs_processor`、`command_exec_processor`、`process_exec_processor`、`thread/shellCommand` 处理路径、`thread/inject_items` 处理路径。处理器按请求中的 `thread_id` 解析共享 runtime，再调用 checker；解析不到 runtime 时按“该 thread 未 engaged”放行（因为无明文）。

`fs/*` 的读路径当前直接 `read_file(path, None)`，接入后必须先过 checker 再落盘；写路径同理。`process/spawn` 与 `thread/shellCommand` 在 engaged 时默认拒绝，响应使用通用错误消息。

## 8. 沙箱视图隔离（P1：readonly bind 方案）

目标：engaged 环境下执行的命令**永不包含 `/dev/shm`**；解密目录以只读 bind 挂到沙箱内的逻辑技能路径；bwrap 的 `/dev/shm` 保持私有空 tmpfs。

- protocol：`FileSystemSandboxPolicy` 新增 `readonly_binds: Vec<ReadonlyBind>`，`ReadonlyBind { source, target }`（source 为解密目录，target 为逻辑技能路径），serde 默认空、JsonSchema/TS 同步；远程 exec-server 往返兼容。
- bwrap：在基础挂载、writable roots、unreadable masks 之后应用 binds；target 在沙箱视图缺失时用 `--dir` 创建（复用 `append_mount_target_parent_dir_args` 语义）；source 不存在则跳过（该 skill 已被清理）。
- guard 的重写模式由 `sandbox_applies_binds` 决定：Linux 且 bwrap 实际生效（非 legacy landlock、非 full-disk-write 跳过条件、网络模式需要隔离）→ 保留逻辑路径（不重写），命令引用逻辑技能路径，走 execute-only 规则；否则（full-access/无 bwrap/非 Linux）→ 保留现有重写到真实路径的行为。
- orchestrator/SandboxAttempt：按同一 `sandbox_applies_binds` 判定注入 binds 到 permission profile；`env_for` 与 `env_for_exec_server` 都生效。
- 未 engaged 时 readonly_binds 为空，bwrap 行为与现状完全一致。

## 9. 上下文传递方式

主链路（Agent Loop、RPC 处理器）以显式参数传递 `&Option<AgentSecurityContext>`（或 `None` 时提前返回），等价 Go ctx。可选桥接：在 turn dispatch 入口用 `tokio::task_local!` scope 一次，供不便改签名的深层调用只读；禁止跨 `tokio::spawn`/`spawn_blocking` 隐式依赖，禁止在深层代码写入。

## 10. 生命周期与销毁

当前明文跨 turn 存活（`clear_encrypted_skills` 无调用者，TTL 空闲 600s）。若决策 D1 采用 turn 级翻转，必须先实现 turn 结束销毁（`clear_thread`），否则禁止 turn 级语义。销毁路径必须 `secure_wipe` 解密目录、清 cache 与 registry、清 in-flight，且幂等。

## 11. 未 engaged 回归基线

未 engaged 时以下行为必须与现状（无加密技能）完全一致：

- 不拦截任何命令（包括含 `/dev/shm` 的字符串）。
- 不对任何输出做 unrewrite/红act（`RedactingToolOutput` 透传，`redact_storage_paths` 不生效）。
- RPC fs/exec/inject 全部放行，readonly_binds 为空，bwrap 参数不变。
- 不进入任何 Agent Security 分支（guard 提前返回 Allow）。

回归测试必须覆盖这些点，防止“为加密技能加的规则污染普通会话”。

## 12. 测试契约（实施时必须完整执行）

- fm-encrypted-skills：`is_engaged`（registry 空/非空、in-flight 加入与移除、失败清理）、`path_mappings`。
- protocol：`ReadonlyBind` 与 `readonly_binds` serde round-trip（含缺省反序列化）。
- linux-sandbox：bwrap 参数单测（binds 顺序、缺失 target 的 `--dir`、source 缺失跳过、未 engaged 空 binds）；真实 bwrap 集成测试验证“解密目录在逻辑路径可见、/dev/shm 内为空”。
- core guard：engaged/未 engaged 两态矩阵（未 engaged 时 `/dev/shm` 命令与输出透传）；逻辑路径 execute-only 放行与 IO 通道拦截；legacy rewrite 分支（无 bwrap 时）不变。
- orchestrator/SandboxAttempt：binds 注入正确，`env_for`/`env_for_exec_server` 均生效。
- app-server RPC 集成测试：`fs/readFile` 等读面拦截受保护路径；`fs/writeFile` 目标与内容检查；`command/exec` 按 shell 规则；`thread/shellCommand` 与 `process/spawn` engaged 时拒绝；未 engaged 全部放行；错误响应不含被拦路径。
- 远程 exec-server 往返测试：readonly_binds 序列化/反序列化一致。
- 回归：普通会话（无加密技能）全量关键路径行为不变。

## 13. 实施前决策点（默认值）

| 编号 | 决策点 | 建议默认值 | 说明 |
|---|---|---|---|
| D1 | 翻转粒度 | Session 级状态 + 每 Turn 重算，guard 读 live 状态；turn 级销毁暂不引入 | 当前明文跨 turn 存活，先不做 turn 结束销毁；若产品要求严格 turn 级，必须先加销毁再翻转 |
| D2 | `thread/shellCommand`、`process/spawn` engaged 处置 | 默认拒绝 | 无沙箱面，字符串检查不够；后续可改为“强制沙箱”作为增强 |
| D3 | RPC 面 execute-only 放行 | 默认不放行（RPC 面引用受保护路径一律 Block） | Agent Loop 内保留 execute-only；RPC 面收紧 |
| D4 | `fs/watch`、`thread/inject_items` 纳入 | 纳入 | 通知流与上下文注入同样可携带明文/路径 |
| D5 | runtime 形态 | 进程级共享服务 + 按 thread_id 隔离 | 保证 Agent Loop 与 RPC 面解析同一实例 |
| D6 | 未 engaged 时 `/dev/shm` 输出红act | 关闭（透传） | 满足 I3 |
| D7 | 强制沙箱的最低级别 | workspace-write（bwrap 生效） | 低于该级别时拒绝解密/执行；read-only 是否允许取决于脚本执行需求 |
| D8 | 插件与 MCP 的禁用范围 | 插件/marketplace 全禁；MCP server 允许 | MCP 不能注册 Codex Hook，暴露面限于工具参数/输出（guard 已覆盖）与同 uid 进程（威胁模型外） |
| D9 | 技能脚本执行的审批 | engaged 时自动 Permit，不向用户征询、不进 guardian 评审 | 用户不应看到加载加密 Skill 后的 Turn 内执行过程；命令本身也走逻辑路径/红act |
| D10 | 配置与模型配置的修改途径 | 客户端不提供任何改配置/改模型配置的入口；Skill 仅可配置启用/禁用；配置由未来受信管理工具负责 | `thread/start` config 覆盖、`experimentalFeature/enablement/set`、`skills/config/write` 等客户端入口不暴露；`thread/realtime/*` 产品不提供，排除在范围外 |

## 14. 相关记录

- 风险与问题记录：`FM_AGENT_SECURITY_FIX_LOG.md` 的 P1/P2 与 I23。
- 本契约与 FIX_LOG 冲突时，以本契约为实施依据，并把冲突记录回 FIX_LOG。

## 15. 待深入探索（TODO，分析模式，未实施）

以下条目是后续必须逐项深入探索、摸清机制、发现问题即修复的代办；每项完成后要回填结论（发现、修复、或确认无需处理）到本文档。

### TODO-1 bwrap 的作用范围与防护边界

已确认的事实：默认 workspace-write/read-only 在 Linux 走 bwrap，`--ro-bind / /`（或 `--tmpfs /`）加 `--dev /dev` 会把沙箱内 `/dev/shm` 挂成私有空 tmpfs；full-disk-write 且 full network 且无 unreadable globs 时跳过 bwrap；legacy landlock 模式不使用 bwrap；系统无 bwrap 时裸跑；WSL1 有专门处理。

待探索：P1 readonly_binds 方案落地后对正常 bash/脚本调用有无行为影响（挂载顺序、缺失 target 的 `--dir` 创建、与 writable roots/unreadable masks 的交互、bwrap 用户命名空间内的能力）；是否存在绕过路径让沙箱内进程访问宿主 `/dev/shm`（未挂 `--proc` 时的宿主 proc、symlink 组件、`/dev/fd`、继承的已打开 fd、bind 目标父目录、硬链接）；结论必须明确“能否被绕过、是否产生泄露”，有问题就修复并补回归测试。

结论（已验证）：`--ro-bind` 在 base mounts 之后应用，缺失 source 跳过、缺失 target 以 `--dir` 创建（argv 单元测试）；新增真实 bwrap 执行测试确认逻辑路径可读、沙箱内 `/dev/shm` 为空、宿主 `/dev/shm` 标记不可见（`a74c497372`）。`--proc` 挂载在受限容器内可能被拒，但 `mount_proc=false` 下其余命名空间可正常创建；产品路径仍默认 `mount_proc=true`。

### TODO-2 Reasoning 与中间过程

现状：assistant 消息与 reasoning 的红act只处理明文片段，不处理路径，且未做 engaged 门控；turn item/持久化红act覆盖明文，部分面覆盖路径。

待探索：偏 Workflow 的 Skill 会让模型产出 task 列表、计划、步骤摘要、子代理消息等中间产物，这些可能复述 skill 内容或路径；逐一梳理 engaged 环境下所有模型可见中间产物（reasoning summary/raw、plan items、task list、agentMessage、commentary、subagent 消息、steer 注入、realtime 文本）是否都被明文+路径红act覆盖；前端展示策略需产品决策：防护上下文内这些产物是否允许展示给用户，默认按 I1/I2 不展示明文/路径，允许的只能是脱敏摘要。

结论（已验证并修复）：模型流入口 `redact_assistant_reply_item` 覆盖 assistant message 与 reasoning summary/raw；持久化/事件面 `redact_turn_item` 覆盖 AgentMessage、Reasoning，本次补上 `PlanItem.text`（task 列表/计划即 Plan 面，含明文时整体红act）；subagent 消息走 agentMessage 面；commentary/steer 非模型产出，不适用；`thread/realtime/*` 排除（D10）。路径面统一由 `RedactingToolOutput`/`redact_storage_paths` 处理（模型上下文只见逻辑路径）。

### TODO-3 Rollout 与 State DB 的可读性

现状：持久化内容已做明文+路径红act（rollout、compaction trace、fork），但 rollout 文件与 sqlite（state db）位于用户可访问目录，同 uid 的脚本/命令可以直接读取或搜索。

待探索：是否需要把 rollout 路径与 state db 路径纳入受保护路径集合（engaged 时禁止脚本/命令/搜索工具读取这些路径）；app-server 是否有对应读接口需要拦截；与 resume/fork/compact/rollback 等正常读取路径的兼容性；是否需要在文件系统层收紧（0600、移出可枚举目录、或对含明文的状态做加密）——注意 I1/I2 对“用户可见面”的约束同样适用于这些文件内容。

结论（已验证）：持久化内容（rollout、compaction trace、fork 历史、state db 输入面）在源头上完成明文+路径红act（`RedactingToolOutput`、`redact_assistant_reply_items`、`redact_turn_item`、fork 隔离），测试断言 rollout 不含明文/解密路径；因此磁盘上不存在明文，脚本/命令读取 rollout/state db 不会触达明文，无需纳入受保护路径集合，也不影响 resume/fork/compact 正常读取。RPC 面历史读取（`thread/read` 等）的 engaged 判定属于 TODO-8 范围。

### TODO-4 TUI `!` 用户直执行令

结论（已验证）：TUI `!` 走 app-server `thread/shellCommand`（README 明确 unsandboxed full access）；变更 4 在 `thread/shellCommand` 处理器入口用 `ensure_not_engaged_unsandboxed` 拦截，进程内任一会话 engaged 即拒绝；app-server E2E（`rpc_guard`）已覆盖该入口。`process/spawn` 同样拦截；`command/exec` 按命令是否引用受保护路径拦截。

已核实：TUI 聊天输入以 `!` 开头会走 app-server `thread/shellCommand` → `Op::RunUserShell`，README 明确该命令 unsandboxed full access，且不经 `before_tool`。

待探索与处置方向：engaged 时默认拒绝该入口（与决策 D2 一致），不能只依赖字符串检查；盘点所有等价入口（TUI `!`、`thread/shellCommand`、`command/exec`、`process/spawn`、用户直接跑 `codex exec`/终端命令等），逐一确认关闭/拒绝的判定与提示文案；威胁模型注明：虽然用户同 uid 本可自行读取，但产品面原则是不向用户展示明文，因此用户直执行令同样受控。

### TODO-5 实施原则：高内聚、低耦合

约束：AgentSecurityContext、共享 checker、`is_engaged`、readonly_binds 相关逻辑尽量收拢在 `fm-encrypted-skills`（或新增独立 crate），核心改动点集中在 runtime/checker 所在 crate、guard 入口、sandbox orchestrator、bwrap、app-server 处理器；避免把逻辑散落到 core 各模块和大文件（遵守 500/800 行模块约束，优先新增模块而非扩展现有文件）。

待探索：protocol 新增 `readonly_binds` 是否可用独立扩展类型减小对公共 schema 的影响（serde default 保持兼容）；与上游 main 合并时的冲突面清单（记录所有被改动的公共文件）；测试尽量放在对应 crate 内，减少跨 crate 测试耦合。

结论（已验证）：加密技能逻辑收拢在 `fm-encrypted-skills`（runtime/guard 纯函数）、core
`encrypted_skills_guard`/`agent_security`（护栏与判定）、`linux-sandbox` bwrap、app-server
`rpc_guard`/处理器边界；`readonly_binds` 走 serde default 兼容；测试放在对应 crate；
被改动的公共文件已由 FIX_LOG 逐提交记录。

### TODO-6 强制启用沙箱的实施面盘点（I6）

现状：Codex 存在多条可关闭/降级沙箱的入口：`config.toml` 的 `sandbox_mode`、permission profile 选择（含 `danger-full-access`）、`dangerously_bypass_approvals_and_sandbox`、CLI `--sandbox`/`--full-auto` 类参数、TUI `/permissions` 切换、app-server `thread/start`/`turn/start` 的 sandbox/permissions 覆盖参数、以及 requirements.toml/managed config 的约束（已有 `permission_profile_constraint` 机制可复用）。

待探索与处置：逐一盘点上述入口，产品化构建统一拒绝“低于 workspace-write”的配置与参数；managed/requirements 层强制最低策略；engaged 时在 runtime 判定处做最终校验（有效策略不达标 → 拒绝解密/执行，返回通用错误）；确认 CLI 直跑、环境变量、直接改配置等用户侧绕过不在产品保证范围内（同 uid 威胁模型），但官方入口必须全部封住；补回归测试覆盖每个入口。

结论（已验证并补全）：CLI（根级与子命令级 `--sandbox danger-full-access`/bypass）、
`thread/start`、`turn/start`、`thread/settings/update`（本次补上）均在边界拒绝 danger-full-access；
engaged 时 orchestrator 以 `SandboxType::None` 判定兜底；managed/requirements 层已有
`permission_profile_constraint`（测试 `turn_start_rejects_invalid_permission_selection...` 保留）；
config.toml 直改与环境变量属于同 uid 威胁模型外，不在产品保证范围（I29/TODO-9 记录）。

### TODO-7 禁止安装/加载插件的实施面盘点（I7）

现状：Codex 支持 `codex plugin`/`marketplace` 子命令、config 中的 plugin/marketplace 配置、启动时从 plugins 目录加载插件、插件注册 PreToolUse/PostToolUse 等 hooks、插件提供 MCP 工具/skills/apps；另有用户配置的独立 MCP server（stdio/http），同样能接收工具参数与输出。

待探索与处置：产品化构建禁用 plugin/marketplace 命令与配置，启动时跳过插件加载（必要时编译期 feature 关闭）；确认 hooks 注册面、插件提供的 skills/apps/tools 面全部不可达；MCP server 不禁用，但需确认 MCP 工具参数与输出、MCP resources/prompts 内容是否全部经过 guard 与 `RedactingToolOutput`（MCP 不能注册 Codex Hook，但可通过工具/资源把内容带回模型上下文，必须走同一红act链路）；补测试验证：插件无法安装/加载/注册 hook，MCP 正常可用且其参数/输出被护栏覆盖。

范围说明：当前产品的 MCP 由官方提供，用户不能自行安装；因此 **MCP 的输入输出与执行暂不纳入本次实施范围**，记录为“当前上下文不构成问题”。若未来放开用户自装 MCP，再按 TODO-7 的确认项重新评估。

结论（已验证并补全）：CLI `plugin`/`marketplace` 子命令全禁；app-server 在消息分发边界新增
产品策略拦截：`marketplace/add|remove|upgrade`、`plugin/install|uninstall`、
`plugin/share/save|updateTargets|checkout|delete` 一律拒绝；只读 `plugin/list|installed|read|
skill/read|share/list` 保留（产品前端不暴露该入口，且内部验证与既有测试依赖只读列表）。
插件“启动时加载”仍未在产品层强制关闭（I29，产品默认配置/受信管理工具负责）。

### TODO-8 展示/中间产物/流式面的深挖

探索已确认或高度疑似未覆盖的面：telemetry 原始 `log_preview()`（红act包装前记录）；`thread/read`、`thread/turns|items/list`、`thread/searchOccurrences`（可能触达 in-memory 明文）；`thread/backgroundTerminals/*` 输出流；`thread/name/set`、`thread/goal/*`、`thread/metadata/update`；detached `review/start` 评审线程的内容来源。逐项确认 engaged 时是否全部走统一红act链路，未覆盖的补上，并决定前端展示策略（默认不展示明文/路径）。`thread/realtime/*` 不在范围（D10）。

结论（已验证并修复）：
- telemetry `log_preview`：已先包 `RedactingToolOutput` 再取预览（变更 4）。
- `thread/read`、turns/items/list、searchOccurrences：读取持久化/事件面内容，来源全部经过
  `redact_turn_item`/`RedactingToolOutput`，无明文 at rest。
- `thread/name/set`、`thread/goal/*`、`thread/metadata/update`：变更 4 已按受保护路径拦截。
- `thread/backgroundTerminals/*`：当前只暴露命令/cwd 元数据，不暴露输出流；命令含明文会被
  shell guard 拦截。
- 本次补上真实缺口：`redact_turn_item` 原先只覆盖 AgentMessage/Reasoning/Plan，现在扩展到
  CommandExecution（stdout/stderr/aggregated/formatted/interaction_input）、FileChange
  （stdout/stderr）、WebSearch query、CollabAgentToolCall prompt、DynamicToolCall
  content/error、McpToolCall error，统一做“解密路径 unrewrite + 明文红act + mem-root 兜底”；
  集成测试让技能脚本回显明文标记，断言 rollout 中明文被红act。

### TODO-9 产品强制面的完整性

探索已确认的潜在绕过：CLI 侧 `--sandbox`/`--full-auto` 类参数与 `dangerously_bypass_approvals_and_sandbox`；代码层仍存在 `thread/start`/`turn/start` config 覆盖、`experimentalFeature/enablement/set`、`skills/config/write` 等入口，但产品不向客户端暴露（D10），配置由未来受信管理工具负责，Skill 仅可配置启用/禁用。实施时：客户端入口按 D10 不暴露；代码层保留最终校验兜底（降级沙箱、关闭安全特性、修改加密配置一律拒绝/忽略），并补回归测试；受信管理工具视为高权限面，后续单独设计。

结论（已验证并补全）：engaged 时（进程内任一会话）`config/value/write`、`config/batchWrite`、
`experimentalFeature/enablement/set`、`skills/config/write`、`skills/extraRoots/set` 一律拒绝，
未 engaged 时行为不变（内部工具与测试不受影响）；`thread/start`/`turn/start` 的 config 覆盖
参数由产品前端不暴露（D10）+ danger-full-access 边界拒绝承接；受信管理工具仍待单独设计。

### TODO-10 `/dev/shm` 与 Swap 落盘（I31，分析模式，未实施）

现状：Linux 下 `/dev/shm` 为 tmpfs，解密明文以文件形式驻留其中；tmpfs 页在内存压力下可被
内核换出到 swap，属于“落盘”。当前实现没有任何 `mlock`/pin；`secure_wipe` 仅删除文件，
防不了自然换出。默认 `RLIMIT_MEMLOCK`（8 MiB 量级）低于单包明文上限（16 MiB），未提权时
mlock 可能失败。

待决策与处置方向：
- 若产品承诺“明文永不落盘（含 swap）”：解密后对 `/dev/shm` 目录内的文件 mmap+mlock（保持
  映射存活），失败时 fail-closed 或显式降级并记录；需处理多技能并发峰值与 RLIMIT_MEMLOCK。
  memfd 方案不适用：Skill 解密后是 zip 展开出的目录树（SKILL.md/scripts/agents/references/
  templates），脚本执行与工具读取依赖真实路径和相对引用，memfd 只有单个 blob、没有目录层级，
  无法承载该结构。
- 若接受 root/取证威胁模型外：不 pin，但需把“明文仅存在于请求瞬间/内存”改为“明文仅存在于
  tmpfs 页与模型请求内存，内存压力下可能进入 swap”。
- 无论哪种方案，补回归测试与文档结论回填（本条目）。

## 16. 暴露风险矩阵（场景 → 行为 → 潜在暴露 → 处置）

| 场景 | 行为（当前/设计） | 潜在暴露风险 | 处置/状态 |
|---|---|---|---|
| 模型执行技能脚本（`bash <logical>/script.sh`） | 当前重写为真实解密路径后执行，可能触发审批 | 明文路径暴露给用户审批 UI、评审、日志；bwrap 下路径不可见导致执行失败 | 设计：engaged 自动 Permit 不征询用户（D9）；P1 后命令保留逻辑路径；评审红act（I23）保留 |
| 模型读取技能文件（`cat <logical>/SKILL.md`） | guard 拦截（non_execution_access） | 明文内容/路径进入模型或输出 | Block，保持 |
| 模型写脚本后间接读取（python 拼接/glob 访问 `/dev/shm`） | 字符串检测可被拼接绕过；bwrap 视图隔离可挡；full-access 挡不住 | 明文内容泄露给模型/输出 | P1 后逻辑路径 + bwrap 私有 `/dev/shm`；full-access 由 I6 强制沙箱封堵 |
| 工具输出/错误回显含明文或真实路径 | `RedactingToolOutput` 红act + unrewrite | 未覆盖字段（部分错误消息、事件）泄露 | 统一红act链路，engaged 门控 |
| reasoning / task 列表 / 计划等中间产物 | 模型流入口与持久化面统一红act明文；路径面由 `RedactingToolOutput`/`redact_storage_paths` 处理；`PlanItem.text` 已补 | Workflow Skill 内容/路径出现在 reasoning 或前端 | engaged 时全部红act；前端不展示明文/路径（产品决策，TODO-2 已验证） |
| guardian/auto-review 评审 | I23 已红act评审请求 | 审批事件、hooks、telemetry 仍见真实命令 | 扩展统一红act（TODO-2/5 覆盖） |
| RPC fs 读（ACP 前端 `fs/readFile` 等） | 当前无 guard | 明文/路径直接返回前端 | engaged 时 Block（第 7 节） |
| RPC/用户直执行令（`thread/shellCommand`、`command/exec`、`process/spawn`、TUI `!`） | engaged 时处理器入口拦截（变更 4） | 读明文、看路径 | engaged 时拒绝（D2）；技能脚本例外走 D9 自动 Permit（TODO-4 已验证） |
| 未 engaged 普通会话 | 无明文 | 无 | 零行为变化（I3） |
| 插件 hooks | 可注册 hook 拿工具输入输出原文 | 明文/路径外发给插件 | 禁用（I7）：CLI 全禁；app-server 安装/卸载/分享/marketplace RPC 全禁（TODO-7 已验证） |
| MCP（官方提供） | 用户不能自装；工具参数/输出走 guard/红act | 当前上下文不构成问题 | 暂不纳入实施范围（记录，见 TODO-7） |
| Rollout / State DB 被脚本读取 | 文件同 uid 可读，但持久化内容在源头已红act | 持久化明文/路径被外部读取 | 无明文 at rest，无需纳入受保护路径（TODO-3 已验证）；继续保留 resume/fork 正常读取 |
| 关闭/降级沙箱 | 用户可配置 | 视图隔离失效，间接读取可达成 | I6 强制启用 + engaged 时拒绝解密/执行 |
| telemetry/analytics 原始预览 | `log_preview()` 已先包 `RedactingToolOutput` 再取预览 | 工具输出预览含明文/路径进入遥测日志 | engaged 时先红act再记录（TODO-8 已验证） |
| `thread/read`、`thread/turns\|items/list`、`thread/searchOccurrences` | 读取持久化/事件面内容，来源统一红act | 若触达 in-memory 明文或持久化红act不完整，历史/搜索结果泄露给前端 | 持久化红act完整性已验证；明文 at rest 不存在（TODO-3/8 已验证） |
| `thread/backgroundTerminals/*` 输出流 | 只暴露命令/cwd 元数据，不暴露输出流 | 技能脚本输出直接流到终端 UI | 输出经 CommandExecutionItem 红act（TODO-8 已验证）；后续若加输出流需走统一红act |
| `thread/name/set`、`thread/goal/*`、`thread/metadata/update` | engaged 时参数含受保护路径 → Block | 模型把 skill 摘要/路径写进名称、目标、元数据并展示/持久化 | 已接入 RPC guard（TODO-8 已验证） |
| `thread/realtime/*` | 产品不提供该能力 | — | 排除（D10 范围说明） |
| `thread/start`/`turn/start`/`thread/settings/update` 的 sandbox/permissions 覆盖参数 | 客户端可传覆盖 | 可覆盖 `sandbox_mode`，绕过 I6 | danger-full-access 一律拒绝；产品不暴露该入口（D10/TODO-6/9 已验证） |
| `experimentalFeature/enablement/set` | 客户端可切换 feature | 若安全路线挂在 feature flag 下可被关闭 | engaged 时拒绝；客户端不暴露（D10/TODO-9 已验证） |
| `skills/config/write`、`skills/extraRoots/set`、`config/value/write`、`config/batchWrite` | 客户端可写配置 | 配置完整性（根目录、加密开关）可能被篡改 | engaged 时拒绝；Skill 仅可配置启用/禁用，加密/根目录由受信管理工具负责（D10/TODO-9 已验证） |
| fork/resume 出的新会话 | 从持久化（已红act）重建历史 | 若持久化红act有遗漏，新会话未 engaged 却携带明文历史 | 扩展 TODO-3 验证持久化红act完整性；fork 后新会话 `is_engaged` 必须为 false 且历史无明文 |
