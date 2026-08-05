## Context

变更 1/2 已提供 `is_engaged`、`path_mappings` 与 `AgentSecurityContext` 门控。当前技能脚本执行仍走“原路径→解密路径”的字符串重写，导致真实 `/dev/shm` 路径进入执行副本、在 bwrap 下不可见、且无法对“间接读取”做视图隔离。本变更改为：解密目录以只读 bind 挂进沙箱的逻辑技能路径，命令保留逻辑路径；无 bwrap 时保留旧重写。

## Goals / Non-Goals

**Goals:**

- 命令中永不出现 `/dev/shm`（binds 生效时）。
- 沙箱内技能脚本可执行（逻辑路径真实可见），且沙箱内 `/dev/shm` 保持私有空 tmpfs。
- engaged 技能脚本执行自动 Permit，用户不见 Turn 内执行过程。

**Non-Goals:**

- 不改动 engaged 时的明文/路径红act规则。
- 不做 RPC 面（后续变更）。
- 不处理 legacy landlock 模式（该模式继续走旧重写）。

## Decisions

### 1. `ReadonlyBind` 放 protocol，`readonly_binds` serde 默认空

`FileSystemSandboxPolicy` 通过 JSON 在 core → codex-linux-sandbox → bwrap 间传递；加字段并 `#[serde(default)]` 保证旧请求/远程 exec-server 兼容。

### 2. bwrap 挂载顺序与缺失 target

binds 在基础挂载、writable roots、unreadable masks、metadata masks 之后应用，保证覆盖；target 在沙箱视图缺失时用 `--dir` 创建（复用 `append_mount_target_parent_dir_args` 语义）；source 不存在说明 skill 已被清理，跳过避免启动失败。

### 3. `sandbox_applies_binds` 与 bwrap 跳过条件一致

判定：Linux && !use_legacy_landlock && !(full-disk-write && 无 unreadable globs && network Enabled && 无 managed network)。与 `create_bwrap_command_args` 的跳过条件保持一致；guard 与 orchestrator 共用同一 helper，避免两套判定漂移。

### 4. D9 自动 Permit 放在 shell/unified exec 审批起点

`start_approval_async` 中，在进入用户审批/guardian 之前判断：engaged 且命令为 execute-only 技能脚本执行 → 直接返回 `ApprovedForSession`。判定复用 guard 的 `is_skill_script_execution`（引用受保护路径 + `is_script_execution` + `script_execution_avoids_guarded_io`）。

## Risks / Trade-offs

- [full-disk-write + unreadable globs 的边角组合] → 该组合 bwrap 生效但 guard 走旧重写会失败；与现状一致（现状同样失败），不构成回归。
- [skill 目录与用户 writable root 重叠] → ro-bind 后于 writable root 应用，覆盖生效；重叠配置属用户自担。

## Migration Plan

协议字段向后兼容；guard 重写模式按运行环境自动选择；可随时回滚（binds 为空即旧行为）。

## Open Questions

无阻塞项。
