## Why

当前加密 Skill 护栏是“无条件启用”的：即使会话没有加载任何加密技能，`/dev/shm` 字符串也会被 shell guard 拦截、输出也会被红act，污染普通会话；同时缺少“本会话是否处于加密 Skill 环境”的统一判定，明文落盘到 registry 登记之间存在 in-flight 空窗，后续 Session/Turn 级翻转与 RPC 面接入也没有可依赖的基础。

## What Changes

- `fm-encrypted-skills` 新增 per-session 判定 `is_engaged(session_id)`：registry 非空或该 session 存在解密 in-flight 时为 true，否则 false。
- `load_or_register` 维护 per-session in-flight 集合：解密开始插入，成功或失败移除；失败路径继续 `secure_wipe`，保证“明文已落盘但未登记”的窗口内 engaged 也为 true。
- 新增 `path_mappings(session_id)`，返回当前会话已注册的 `(解密目录, 逻辑技能路径)` 映射，供沙箱 readonly bind 与 RPC 面使用；未注册时为 `[]`。
- 行为契约：未 engaged 时所有护栏透传（零行为变化）；engaged 时保持既有拦截/红act规则。本次变更只交付“判定与门控基础”，不改变 engaged 时的规则本身。

## Capabilities

### New Capabilities

- `encrypted-skill-engagement`: 会话级 engaged 判定（registry + in-flight）、解密 in-flight 生命周期、`(解密目录, 逻辑路径)` 映射导出。

### Modified Capabilities

（无，`openspec/specs/` 当前为空，全部为新增能力。）

## Impact

- `codex-rs/fm/encrypted-skills`：`runtime.rs`（`is_engaged`、in-flight 集合、`path_mappings`）、`registry.rs`（只读访问已存在）、`mem_root.rs`（失败清理语义不变）。
- 后续接线点（本变更不实现）：core guard 的 engaged 门控、SandboxAttempt 的 bind 注入、app-server RPC 处理器按 thread_id 解析 runtime。
- 测试：`fm-encrypted-skills` 单测（`runtime_tests.rs`），不引入新依赖。
