## Context

变更 1-3 提供了 engaged 判定、上下文门控与沙箱 bind。app-server 的 RPC 面（ACP/SDK 前端直连）不经 `before_tool`，需要独立的检查通道；`fs/*`、`command/exec` 等请求不携带 thread_id，无法按 thread 判定，因此采用进程级“任一 engaged”的保守策略。

## Goals / Non-Goals

**Goals:**

- 进程级 engaged 注册表与受保护路径导出。
- telemetry 工具预览先红act再记录。
- app-server RPC 面按 spec 规则拦截。

**Non-Goals:**

- 不改 `thread/read`/`items/list`/`searchOccurrences` 的读取行为（持久化已红act，in-memory 风险留 TODO-8）。
- 不做按连接/线程绑定的精确判定（请求协议无 thread_id，保守策略已记录）。
- 不重写既有 fs 实现，只加检查层。

## Decisions

### 1. 进程级注册表用 Weak 引用，避免生命周期钩子

`EncryptedSkillRuntime::new` 把自身 `Weak` 注册进 `OnceLock<Mutex<Vec<Weak<...>>>>`；`any_engaged()`/`engaged_guarded_paths()` 遍历时清理失效 Weak。不需要在线程结束回调，Drop 自然失效。

### 2. RPC 检查做成 core 纯函数，app-server 只接线

`agent_security::rpc` 提供 `check_fs_path(path) -> bool`（engaged 进程 + 路径命中）、`check_command(command) -> bool`、`check_args(value) -> bool` 等纯函数（内部用 `any_engaged` + `engaged_guarded_paths` + 现有 `paths` 匹配），app-server 处理器只调用并返回通用 Block 错误。便于单测与复用。

### 3. telemetry 预览修复：先包装后取预览

工具结果在 `handle_any_tool` 返回后立即包 `RedactingToolOutput`，`log_preview()` 从包装对象取；避免双包装，改为只包装一次并让后续流程复用同一包装。

## Risks / Trade-offs

- [进程级 engaged 使无关线程的 RPC 也受限] → 保守但安全；未 engaged 进程零影响；后续协议若带 thread_id 可细化。
- [fs 写内容含明文未检测] → 仅做路径检查，内容检测依赖每会话明文集合，进程级通道不适用；记录为已知限制。

## Migration Plan

检查层默认放行（未 engaged）→ 渐进接线到各处理器；可随时回退。

## Open Questions

无阻塞项。
