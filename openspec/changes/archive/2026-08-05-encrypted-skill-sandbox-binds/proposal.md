## Why

技能脚本执行当前把命令中的技能路径重写为真实的 `/dev/shm` 解密路径：真实路径会进入执行副本并可能外发（审批、日志、评审），而且在默认 bwrap 沙箱里 `/dev/shm` 是私有空 tmpfs，重写后的路径不可见导致脚本实际无法执行；同时“写脚本→解释器间接读 `/dev/shm`”的攻击只能靠字符串检测拦截。

## What Changes

- protocol 新增 `ReadonlyBind { source, target }` 与 `FileSystemSandboxPolicy.readonly_binds`（serde 默认空、JsonSchema/TS 同步、远程 exec-server 往返兼容）。
- linux-sandbox bwrap 应用 readonly binds：解密目录以只读 bind 挂到沙箱内的逻辑技能路径；缺失 target 用 `--dir` 创建；source 缺失（已清理）则跳过；挂载顺序在基础挂载、writable roots、unreadable masks 之后。
- core 新增 `sandbox_applies_binds` 判定（Linux、非 legacy landlock、bwrap 实际生效时）；`SandboxAttempt` 携带 `skill_binds`，`env_for`/`env_for_exec_server` 把 binds 注入 permission profile。
- guard 重写模式二选一：binds 生效时命令保留逻辑技能路径（不重写），逻辑路径加入受保护集合（execute-only 规则不变）；binds 不生效（full-access/无 bwrap/非 Linux）时保留现有重写为真实路径的行为。
- D9：engaged 环境下的技能脚本执行（execute-only 放行）自动 Permit，不向用户征询、不进 guardian 评审；用户看不到加载加密 Skill 后的 Turn 内执行过程。

## Capabilities

### New Capabilities

- `encrypted-skill-sandbox-binds`: 沙箱 readonly bind、guard 重写模式决策、engaged 技能脚本执行自动 Permit。

### Modified Capabilities

（无。）

## Impact

- `codex-rs/protocol`：`permissions.rs`（`ReadonlyBind`、`FileSystemSandboxPolicy.readonly_binds`）。
- `codex-rs/linux-sandbox`：`bwrap.rs`（应用 binds）。
- `codex-rs/core`：`tools/sandboxing.rs`（`SandboxAttempt.skill_binds`）、`tools/orchestrator.rs`（binds 计算与注入）、`tools/runtimes/shell.rs` 与 `unified_exec.rs`（D9 自动 Permit）、`encrypted_skills_guard.rs`（重写模式与逻辑路径受保护）、`exec.rs`（binds 判定 helper 使用）。
- 测试：protocol serde、linux-sandbox bwrap 参数、core guard/approval 单测；真实 bwrap 集成测试（验证逻辑路径可见、`/dev/shm` 内为空）。
