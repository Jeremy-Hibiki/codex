## Context

`EncryptedSkillRuntime` 是 per-session 状态载体：registry 以 `session_id` 为 key 记录已加载 skill 的 `(dir, original_dir, token, 时间戳)`，mem-root 是进程级目录。当前 `load_or_register` 的解密流程存在“明文已落盘但 registry 尚未登记”的窗口；现有 guard 与红act是无条件启用的，后续需要按“会话是否 engaged”门控，但当前没有任何统一的判定入口。

本变更只交付判定与映射基础：`is_engaged`、in-flight 生命周期、`path_mappings`。它是 `FM_AGENT_SECURITY_DESIGN.md` 中 P2（AgentSecurityContext、engaged 门控）与 P1（沙箱 readonly bind）、RPC 面接入的前置依赖。

## Goals / Non-Goals

**Goals:**

- 提供 `is_engaged(session_id)`，语义为“该会话当前存在明文或将产生明文”，且只由 runtime 状态派生。
- 覆盖解密 in-flight 窗口：明文落盘到 registry 登记之间，`is_engaged` 也必须为 true。
- 提供 `path_mappings(session_id)`，供后续沙箱 bind 与 RPC 面使用。
- 保持现有 engaged 会话的 guard/红act行为完全不变；未 engaged 会话本变更不引入任何行为。

**Non-Goals:**

- 不做 guard/红act 的 engaged 门控（后续变更）。
- 不把 runtime 提升为进程级共享服务（后续变更，RPC 面需要）。
- 不实现沙箱 readonly bind（后续变更）。
- 不改变 `load_or_register` 的公开签名与既有返回语义。

## Decisions

### 1. engaged 由“registry 非空 + in-flight 集合”派生，不引入独立标志位

`is_engaged = registry 中存在该 session 的记录 || in_flight 集合包含该 session`。独立布尔标志容易与真实状态失同步（开了但没解密、解密了但没开），派生判定天然一致。

替代方案：维护 `engaged: HashMap<SessionId, bool>`——需要在解密、清理、TTL、clear_thread 多处手动同步，遗漏即漏洞；否决。

### 2. in-flight 集合放在 runtime 上，用 RAII guard 保证移除

`EncryptedSkillRuntime` 增加 `in_flight: Mutex<HashSet<String>>`。`load_or_register` 进入真正解密路径前插入 `session_id`，并返回一个 `InFlightGuard`（Drop 时移除），成功、失败、panic unwind 都会清理；失败路径仍执行现有 `secure_wipe`。

替代方案：在每个 return 分支手动移除——容易漏分支，且 panic 不覆盖；否决。

### 3. `clear_thread` 同时清理 in-flight

`clear_thread(session_id)` 在清 registry/cache/目录时，也必须从 in_flight 移除该 session，保证清理后 `is_engaged` 立即为 false。

### 4. `path_mappings` 只导出非空 original_dir 的映射

`original_dir` 为空表示无逻辑路径可 bind，导出无意义；返回 `(dir, original_dir)` 列表，顺序无关（调用方按需排序）。

## Risks / Trade-offs

- [in-flight 标记在进程 abort 时无法 Drop] → abort 属进程终止，无后续请求，可接受；正常错误路径由 RAII 覆盖。
- [TTL sweep 与 in-flight 并发] → sweep 只遍历 registry，不遍历 in_flight；未登记目录不会被 sweep 误删；登记后由 registry 保护。
- [is_engaged 读锁开销] → 每次判定一次 Mutex 读，频率低，可忽略。

## Migration Plan

纯新增 API，无迁移成本；后续变更逐个接入 `is_engaged`/`path_mappings` 即可。

## Open Questions

无阻塞项。
