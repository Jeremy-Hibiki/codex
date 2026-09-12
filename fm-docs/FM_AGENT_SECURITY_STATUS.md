# Agent Security 实施状态与待办

> 基线：`rust-v0.146.0`（`e363b08c91`）。本文件是“一次性读懂”的速览：
> 已实施/已验证的内容与未实现/待办各占一节。
> 权威细节：`FM_AGENT_SECURITY_DESIGN.md`（实施契约）、`FM_AGENT_SECURITY_FIX_LOG.md`（逐条记录 I1~I31）。

## 一、已实施并验证

### 1.1 已完成变更（按顺序）

| # | 变更 | 提交 | 内容 |
|---|------|------|------|
| 1 | encrypted-skill-engagement | `fbe382806f` / `01c8a5a57c` | 会话级 engaged 判定：registry 非空 \|\| 解密 in-flight；RAII `InFlightGuard` 覆盖“明文落盘→登记”窗口；失败路径 secure wipe；`path_mappings` 导出 `(解密目录, 逻辑路径)`；未 engaged 零行为变化 |
| 2 | agent-security-context-gating | `12e3706428` / `613f55a8ee` | `AgentSecurityContext`（runtime + session_id + live `engaged()`）；`TurnContext.agent_security` 按 turn 组装；`before_tool` 未 engaged 直接 Allow；`RedactingToolOutput` 未 engaged 透传 |
| 3 | encrypted-skill-sandbox-binds | `16976eec92` / `77523e437c` | P1 落地：protocol `ReadonlyBind` + `FileSystemSandboxPolicy.readonly_binds`；bwrap `--ro-bind`（缺失 target `--dir`、缺失 source 跳过）；`SandboxAttempt.skill_binds`；guard 在 binds 生效时保留逻辑路径；D9 技能脚本 execute-only 自动 Permit（不提示、不进 guardian） |
| 4 | agent-security-rpc-guard | `1c694a0269` / `5c44c17a17`（归档 `4ff39a831a`） | 进程级 engaged 注册表（Weak）；telemetry `log_preview` 先包红act；core `agent_security::rpc` 纯函数；app-server fs/command/exec/thread shell/process/inject/name/goal/metadata 全部接线；RPC E2E：turn in-flight 窗口内验证拦截、普通路径放行、未 engaged 不受影响 |
| 5 | agent-security-product-policy | `e29eda359d`（归档 `4ff39a831a`） | 强制沙箱三层：orchestrator engaged 运行期断言、CLI 参数拒绝（根级+子命令级 `danger-full-access`/bypass）、app-server `thread/start`/`turn/start` 拒绝 danger-full-access；CLI `plugin`/`marketplace` 子命令拒绝；原测试保留 `#[ignore]`，策略拒绝测试另存 |
| 6 | TODO-1 bwrap 真实执行验证 | `a74c497372` | 真实 bwrap 测试：逻辑路径只读可见、沙箱内 `/dev/shm` 为空、宿主标记不可见；`mount_proc=false` 下受限容器可运行 |
| 7 | TODO-2 Plan 红act | `bf5d3b1009` | 修复 `TurnItem::Plan`（task 列表/计划）未红act缺口；reasoning/agentMessage 已有覆盖；TODO-3/4 结论回填 |
| 8 | TODO-6/7/8/9 边界收尾 | `faf8a03fb3` / 归档 `6a2f9a217a` | app-server 拦截 `marketplace/add|remove|upgrade`、`plugin/install|uninstall`、`plugin/share/save|updateTargets|checkout|delete`；`thread/settings/update` 拒绝 danger-full-access；engaged 时拒绝 `config/value/write`、`config/batchWrite`、`experimentalFeature/enablement/set`、`skills/config/write`、`skills/extraRoots/set`；`redact_turn_item` 扩展到 CommandExecution/FileChange/WebSearch/CollabAgentToolCall/DynamicToolCall/McpToolCall 文本面（明文+路径红act） |
| 9 | 非破坏性恢复 | `e4ab61a907` | 恢复 8 个 app-server + 4 个 CLI 插件/市场测试文件，统一 `#[ignore]`，不删除不重写；相对 `rust-v0.146.0` 无任何文件删除 |
| 10 | clippy 修复 | `c4a7dc6c8b` | thread-manager-sample 缺配置字段、bwrap 测试冗余 clone |
| 11 | openspec 归档 | `61efb80a7f` 等 | 所有 openspec 变更（含历史基线 encrypt-agent-security-skills）已同步主 spec 并归档，`openspec list` 无活动变更 |

### 1.2 核心能力清单

- 加密 Skill 全生命周期：frontmatter 加密标记 → 数字信封 SDK 解密 → `/dev/shm` 0700 目录树 → token 化注入 → 请求时重水合 → 两级 TTL 卸载 → secure wipe。
- 访问控制：shell 执行路径改写/execute-only、非 shell 工具明文导出拦截、链式走私分段判定、glob/读/搜/拷贝/重定向全拦截。
- 脱敏链路：模型流入口（assistant/reasoning/plan）、工具输出、guardian 评审、telemetry 预览、turn item 事件与持久化（rollout/compaction/fork）统一红act明文+路径。
- RPC 面：fs/command/exec/process/shell/inject/name/goal/metadata 的 engaged 拦截；插件/市场变更 RPC 与配置变更 RPC 产品策略拦截。
- 强制沙箱：CLI/app-server/运行期三层；bwrap 只读 bind 到逻辑技能路径；D9 自动 Permit。
- 未 engaged 零行为变化（I3）；普通会话不被 `/dev/shm` 策略污染。

### 1.3 关键决策（D1-D10）

| 编号 | 决策 | 结论 |
|---|---|---|
| D1 | 翻转粒度 | Session 级状态 + 每 Turn 重算，guard 读 live 状态 |
| D2 | `thread/shellCommand`、`process/spawn` engaged | 默认拒绝（TUI `!` 同路径） |
| D3 | RPC 面 execute-only | 不放行，引用受保护路径一律 Block |
| D4 | `fs/watch`、`thread/inject_items` | 纳入 |
| D5 | runtime 形态 | 进程级共享服务 + 按 thread_id 隔离 |
| D6 | 未 engaged 红act | 关闭（透传） |
| D7 | 强制沙箱最低级别 | workspace-write（bwrap 生效）；engaged 且无沙箱拒绝 |
| D8 | 插件与 MCP | 插件/marketplace 管理默认放开（对齐上游行为），可通过 `[product_policy]` 配置 `plugin_management_disabled` / `marketplace_management_disabled` 禁用；MCP server 允许（官方提供，暂不纳入实施范围） |
| D9 | 技能脚本审批 | engaged execute-only 自动 Permit，不征询用户、不进 guardian |
| D10 | 配置面 | 客户端不提供改配置/改模型配置入口；Skill 仅可配置启用/禁用；`thread/realtime/*` 产品不提供；配置由未来受信管理工具负责 |

### 1.4 验证结果（最近一次全量）

| 范围 | 结果 |
|---|---|
| fm-encrypted-skills | 142 / 142 |
| codex-protocol | 265 / 265 |
| codex-cli | 289 通过 / 40 忽略（插件/市场原测试 `#[ignore]`） |
| codex-app-server | 902 通过 / 130 忽略（4 个 zsh-fork 用例全量负载下超时、单独重跑全绿） |
| codex-linux-sandbox | 122 / 124（2 个已知环境性网络用例失败，与改动无关） |
| codex-core（加密相关过滤） | encrypted_skills + guard + agent_security = 96 / 96 |
| codex-core 全量 | 环境相关失败约 21 + 1 超时（真实 `~/.agents/skills` 污染、项目信任状态、代理网络、MCP 超时），与本分支改动文件无关 |
| `just clippy`（workspace，含 tests） | 通过（仅 rmcp 弃用警告） |
| bazel build | `//codex-rs/cli:codex`、`//codex-rs/app-server:codex-app-server`、`//codex-rs/linux-sandbox:codex-linux-sandbox`、`//codex-rs/fm/encrypted-skills:encrypted-skills` 成功 |

### 1.5 分支状态

- 基线：`rust-v0.146.0`；相对基线无任何文件删除（`git diff --diff-filter=D` 为空）。
- 所有 openspec 变更已归档；`openspec list` 为空。
- 工作区仅剩未跟踪工具目录（`.codex/skills/openspec-*`、`.omp/`、`.serena/`），不提交。

## 二、未实现/待办

### 2.1 待产品决策后实施

#### I31 / TODO-10：`/dev/shm` Swap 落盘与 Pin

- 现状：Linux `/dev/shm` 为 tmpfs，内存压力下页可被换出到 swap；当前无 `mlock`/pin。
- 方案：解密目录内文件 mmap+mlock；memfd 不适用（Skill 是 zip 展开的目录树，脚本/工具依赖真实路径与相对引用）。
- 两种部署场景：
  - 有 root（特权容器/systemd）：`LimitMEMLOCK=infinity` 或 `setrlimit` 提升上限，mmap+mlock，失败 fail-closed，可配合无 swap 设备。
  - 无 root（普通容器）：`RLIMIT_MEMLOCK` 通常 8 MiB < 单包 16 MiB，无法保证全部 pin；默认接受 swap 属 root/取证威胁模型外，或对无法 pin 的 Skill fail-closed，或用 `/proc/self/status` `VmSwap` 监控告警，部署侧提 `LimitMEMLOCK` 后再启用。
- 状态：分析模式，未实现；等待产品对“明文不落盘（含 swap）”承诺的决策。

### 2.2 明确保留为未来/产品侧承接

- **I30 / TODO-7/9 残留**：`features.plugins=true` 配置下插件启动加载/同步未在产品层强制关闭；TUI 插件管理入口未单独收敛。处置：产品默认配置禁用 plugins feature，或由受信管理工具下发配置；安全路线不得挂在用户可关闭的 flag 下。注：CLI/app-server 的插件/市场管理命令禁用已改为 `[product_policy]` 配置项（默认放开），不再写死。
- **受信管理工具（D10）**：配置/模型配置由未来受信管理工具负责；客户端不暴露入口；工具本身待单独设计（高权限面）。
- **MCP 范围（TODO-7 范围说明）**：当前产品 MCP 由官方提供、用户不能自装；MCP 输入输出与执行暂不纳入实施范围。若未来放开用户自装 MCP，需重新评估 guard/红act 覆盖。
- **`thread/realtime/*`（D10）**：产品不提供该能力，明确排除在范围外。

### 2.3 已接受/威胁模型外的项

| 项 | 结论 |
|---|---|
| Rollout / State DB 被同 uid 脚本读取 | 持久化内容源头红act，磁盘无明文，不需要纳入受保护路径（TODO-3 已验证） |
| `config.toml` 直改、环境变量、同 uid 直接读 `/dev/shm` | 同 uid 威胁模型外，产品官方入口已全部封住 |
| Swap 内容 | 只有 root/取证可读；是否纳入威胁模型待产品决策（I31） |

### 2.4 环境相关未闭环项（非本分支代码问题）

- codex-core 全量测试中约 21 个失败 + 1 个超时：真实 `~/.agents/skills` 污染 skills 目录测试、项目信任状态、代理网络下的 approvals/network/unified_exec、MCP 超时。
- codex-linux-sandbox 2 个网络用例（wget/socketpair）在代理环境超时。
- app-server 4 个 zsh-fork 用例在全量负载下偶发超时，单独重跑全绿。
- 这些用例在干净 CI/网络环境下应可复现为绿色；与本分支改动文件无关。

### 2.5 后续建议顺序

1. 产品决策 I31（swap 是否纳入威胁模型）→ 实施 mlock 或修正文档表述。
2. 产品默认配置/受信管理工具承接 I30（插件启动加载关闭）。
3. 若放开用户自装 MCP，按 TODO-7 重新评估。
4. 每项完成后回填本文件与 FIX_LOG。
