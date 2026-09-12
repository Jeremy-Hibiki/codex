## Why

设计文档 TODO-6/7/8/9 的收尾：官方入口全部封住（强制沙箱、禁止插件安装/分享、engaged 时禁止客户端配置变更），并补齐 turn item 输出面的明文+路径红act缺口。

## What Changes

- app-server 消息边界：拦截 `marketplace/add|remove|upgrade`、`plugin/install|uninstall`、`plugin/share/save|updateTargets|checkout|delete`。
- `thread/settings/update` 拒绝 danger-full-access（与 `thread/start`/`turn/start` 一致）。
- engaged 时拒绝 `config/value/write`、`config/batchWrite`、`experimentalFeature/enablement/set`、`skills/config/write`、`skills/extraRoots/set`；未 engaged 行为不变。
- `redact_turn_item` 扩展覆盖 CommandExecution、FileChange、WebSearch、CollabAgentToolCall、DynamicToolCall、McpToolCall 的文本字段（解密路径 unrewrite + 明文红act + mem-root 兜底）。

## Capabilities

### New Capabilities

- `agent-security-final-boundaries`: TODO-6/7/8/9 边界收尾。

### Modified Capabilities

（无。）

## Impact

- `codex-rs/app-server`：消息分发边界、turn_processor（settings update）、rpc_guard 新策略助手；插件/市场测试收敛为 `plugin_policy`。
- `codex-rs/core`：`encrypted_skills_guard.rs` turn item 红act扩展与测试。
- 文档：设计文档 TODO-6/7/8/9 结论、FIX_LOG 记录。
