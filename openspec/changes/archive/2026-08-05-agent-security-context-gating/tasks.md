## 1. 测试先行（RED）

- [x] 1.1 写失败单测：`AgentSecurityContext::engaged()` 实时反映 runtime 状态（空会话 false、加载后 true）
- [x] 1.2 写失败单测：未 engaged 时 `before_tool` 对含 mem-root 路径的 bash 命令返回 `Allow`
- [x] 1.3 写失败单测：未 engaged 时非 shell 工具参数含 mem-root 路径返回 `Allow`
- [x] 1.4 写失败单测：未 engaged 时 `RedactingToolOutput` 原样透传（不 unrewrite/不红act）
- [x] 1.5 写失败单测：engaged 时 `before_tool` 与 `RedactingToolOutput` 保持既有行为（回归）

## 2. 实现（GREEN）

- [x] 2.1 新增 `agent_security.rs`：`AgentSecurityContext`（runtime + session_id + `engaged()`）
- [x] 2.2 `TurnContext` 增加 `agent_security: Option<AgentSecurityContext>` 并在构造时组装
- [x] 2.3 `before_tool` 入口增加未 engaged 直接 `Allow` 门控
- [x] 2.4 `RedactingToolOutput` 增加 engaged 字段，未 engaged 透传

## 3. 验证与收尾

- [x] 3.1 `just test -p codex-core`（`agent_security` + `encrypted_skills_guard` 过滤）全绿
- [x] 3.2 `just fmt` 与 `just fix -p codex-core` 通过
- [x] 3.3 提交变更并在 `FM_AGENT_SECURITY_FIX_LOG.md` 记录决策
