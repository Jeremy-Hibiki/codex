# Agent Security 待办与残留问题

> 按“已记录、未实现/未闭环”口径汇总；每条都有出处与处置方向。
> 权威细节：`FM_AGENT_SECURITY_DESIGN.md`（TODO 节）、`FM_AGENT_SECURITY_FIX_LOG.md`（I30/I31 等）。

## 1. 待产品决策后实施

### I31 / TODO-10：`/dev/shm` Swap 落盘与 Pin

- 现状：Linux `/dev/shm` 为 tmpfs，内存压力下页可被换出到 swap；当前无 `mlock`/pin。
- 方案：解密目录内文件 mmap+mlock；memfd 不适用（Skill 是 zip 展开的目录树，脚本/工具依赖真实路径与相对引用）。
- 两种部署场景：
  - 有 root（特权容器/systemd）：`LimitMEMLOCK=infinity` 或 `setrlimit` 提升上限，mmap+mlock，失败 fail-closed，可配合无 swap 设备。
  - 无 root（普通容器）：`RLIMIT_MEMLOCK` 通常 8 MiB < 单包 16 MiB，无法保证全部 pin；默认接受 swap 属 root/取证威胁模型外，或对无法 pin 的 Skill fail-closed，或用 `/proc/self/status` `VmSwap` 监控告警，部署侧提 `LimitMEMLOCK` 后再启用。
- 状态：分析模式，未实现；等待产品对“明文不落盘（含 swap）”承诺的决策。

## 2. 明确保留为未来/产品侧承接

### I30 / TODO-7/9 残留：插件启动加载

- `features.plugins=true` 配置下插件启动加载/同步未在产品层强制关闭；TUI 插件管理入口未单独收敛。
- 处置方向：产品默认配置禁用 plugins feature，或由受信管理工具下发配置；安全路线不得挂在用户可关闭的 flag 下。

### 受信管理工具（D10）

- 配置/模型配置由未来受信管理工具负责；客户端不暴露入口；工具本身待单独设计（高权限面）。

### MCP 范围（TODO-7 范围说明）

- 当前产品 MCP 由官方提供、用户不能自装；MCP 输入输出与执行暂不纳入实施范围。若未来放开用户自装 MCP，需重新评估 guard/红act 覆盖。

### `thread/realtime/*`（D10）

- 产品不提供该能力，明确排除在范围外。

## 3. 已接受/威胁模型外的项

| 项 | 结论 |
|---|---|
| Rollout / State DB 被同 uid 脚本读取 | 持久化内容源头红act，磁盘无明文，不需要纳入受保护路径（TODO-3 已验证） |
| `config.toml` 直改、环境变量、同 uid 直接读 `/dev/shm` | 同 uid 威胁模型外，产品官方入口已全部封住 |
| Swap 内容 | 只有 root/取证可读；是否纳入威胁模型待产品决策（I31） |

## 4. 环境相关未闭环项（非本分支代码问题）

- codex-core 全量测试中约 21 个失败 + 1 个超时：真实 `~/.agents/skills` 污染 skills 目录测试、项目信任状态、代理网络下的 approvals/network/unified_exec、MCP 超时。
- codex-linux-sandbox 2 个网络用例（wget/socketpair）在代理环境超时。
- app-server 4 个 zsh-fork 用例在全量负载下偶发超时，单独重跑全绿。
- 这些用例在干净 CI/网络环境下应可复现为绿色；与本分支改动文件无关。

## 5. 后续建议顺序

1. 产品决策 I31（swap 是否纳入威胁模型）→ 实施 mlock 或修正文档表述。
2. 产品默认配置/受信管理工具承接 I30（插件启动加载关闭）。
3. 若放开用户自装 MCP，按 TODO-7 重新评估。
4. 每项完成后回填本文件与 FIX_LOG。
