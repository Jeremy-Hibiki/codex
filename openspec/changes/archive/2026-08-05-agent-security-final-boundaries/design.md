## Context

变更 5 已封住 CLI 与 thread/turn start 边界；剩余 app-server 变更入口（settings update、配置写、feature enablement、插件安装/分享 RPC）与 turn item 输出面红act缺口在本变更补齐。

## Goals / Non-Goals

**Goals:**

- 插件安装/卸载/分享/marketplace RPC 全禁；只读 list/read 保留（产品前端不暴露该入口，且内部验证依赖）。
- settings update 拒绝 danger-full-access。
- engaged 时客户端配置变更全禁；未 engaged 零行为变化。
- 技能脚本/工具输出中的明文与解密路径在事件与持久化面被红act。

**Non-Goals:**

- 不强制关闭 `features.plugins=true` 的启动加载（I30，由产品默认配置/受信管理工具承接）。
- 不实现受信管理工具（D10）。

## Decisions

### 1. 策略拦截收敛在消息分发边界

`handle_client_request` 入口统一执行 `ensure_plugin_management_allowed` 与 `ensure_config_mutation_allowed`，避免在各处理器重复；错误文案与既有产品策略一致。

### 2. 配置变更仅在 engaged 时拒绝

未 engaged 保持原有行为，避免破坏内部工具与既有测试；engaged 时客户端不能降级沙箱/关安全特性/改技能配置。

### 3. turn item 输出面统一红act

`redact_turn_item` 从“仅明文”扩展为“unrewrite 解密路径 + 明文红act + mem-root 兜底”，覆盖脚本输出、文件变更、搜索查询、子代理提示、动态工具与 MCP 错误文本。

## Risks / Trade-offs

- [插件测试面收敛] → 8 个 app-server 测试文件合并为 `plugin_policy` 策略拒绝断言；core-plugins 单元测试保留底层覆盖。
- [只读插件 RPC 保留] → 不构成安装/加载风险，产品前端不暴露；I30 记录启动加载残留。

## Migration Plan

可回退：移除分发边界检查与 turn item 红act分支即恢复。

## Open Questions

无阻塞项。
