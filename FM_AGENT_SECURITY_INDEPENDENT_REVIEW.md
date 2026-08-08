# Agent Security 重构+测试系列 独立复审（2026-08-09）

> 复审者：独立 agent（rust-skills + code-review 双轴），不复信作者自评文档。
> 口径：`git diff 3e07a01ed8...HEAD`（10 个 commit，64 文件，+2719/-2036）。
> fixed point `3e07a01ed8` = 重构系列（`f09d996fa0` 测试整合）之前的提交。
> 验证：静态分析 + 实跑测试（fm-encrypted-skills 187/187、core spawn 105/105）。

## 结论

重构整体**忠实、行为保持**：R1/R3/R4/C1/R2/R5/R8 全部逐项核对为等价移植，F01 是真实有效的安全修复。
存在 **1 个需人工确认的产品策略翻转**（Spec 轴）、**1 个 AGENTS.md 硬违规**（模块体积）、若干标准/可维护性小项。

## Spec 轴（行为 / 意图）

| 项 | 结论 | 证据 |
|---|---|---|
| R4 token 遍历移植 | ✅ 等价 | `token.rs:90-174` 的 `for_each_response_item_text` / `strip_tokens_from_rollout_item` 覆盖的 ResponseItem/RolloutItem 变体与旧 `spawn.rs` 逐字一致（含相同 `_ => {}`）。fork 边界无 token 泄漏。spawn 测试 105/105 含 `spawn_agent_can_fork_parent_thread_history_with_sanitized_items`。 |
| C1 into_sdk | ✅ 等价 | `config/mod.rs:646-683` 的 match 覆盖 6 个 `EncryptedSkillsSdkToml` 变体，与旧 `session.rs:1082-1112` 完全一致（含 TestZip warn、Software algorithm 子 match、UKeyTwoPhase.key_envelope）。 |
| R1/R8 product-policy + sandbox bypass | ✅ 等价 | `fm/product-policy/src/lib.rs` 的 `full_access_requested`、`sandbox_policy_bypassed`（`#[cfg(debug_assertions)]`，release 恒 `false`）、`SANDBOX_BYPASS_ENV_VAR`；`agent_security.rs` 仅薄转发。deny 路径未被弱化。 |
| R2/R5 SessionGuard facade | ✅ 等价 | `session/session.rs:483-499` 两个访问器均用 `self.thread_id.to_string()` + `self.services.encrypted_skills_runtime`，与所有原调用点配对一致（抽查 `write_stdin.rs` 迁移：`session.thread_id.to_string()` → 访问器内部同值）。 |
| F01 `>(...)` 进程替换拦截 | ✅ 真实修复 | `paths.rs:216-220`（tree-sitter `process_substitution` 节点，双向）+ `paths.rs:285-291`（legacy fallback 显式处理 `>(`）。测试 `script_execution_blocks_guarded_io_channels` 通过。 |
| 测试 P0-P3 | ✅ 已落地 | 恒真断言（sdk `_` matches 等）已删；表格化合并生效（guard 75→46 等）；M08 spawn strip 覆盖已补。fm 187/187 通过。 |
| **feat(8d202400c9) 插件/市场默认翻转** | ⚠️ **行为变更，需确认** | 见下方 P0。 |

### P0 — 插件/市场管理从「默认禁用」翻转为「默认放开」（需人工确认）

- **变更前**（`8d202400c9~1`）：`app-server/rpc_guard.rs::ensure_plugin_management_allowed(request)` **无条件**拒绝所有 plugin/marketplace 管理 RPC；cli 对任何 `Plugin` 子命令无条件报错。
- **变更后**（HEAD）：两者均改为由 `[product_policy] plugin_management_disabled` / `marketplace_management_disabled` 门控，`config/mod.rs:4108/4112` 用 `unwrap_or(false)` → **默认不禁用 = 默认放开**。
- **意图**：doc 注释（"defaults are open, matching the upstream Codex behavior"）、commit message、测试重命名（`plugin_management_is_open_by_default`）一致，属**有意**翻转，非静默回归。
- **风险**：对一个 **agent-security fork**，这是安全态势的逆转——未显式设置 `[product_policy]` 的部署将重新获得插件/市场管理能力（插件可执行任意代码，扩大攻击面）。且与 `fm-product-policy` crate 的自述目的（"plugin/marketplace management are disabled product features"）相矛盾。
- **建议**：确认「open by default」是期望姿态；否则把默认翻回 `unwrap_or(true)`。
- **测试缺口**：`plugin_management_is_open_by_default` 只测 `plugin list`（读操作，旧代码也未拦），**没有测试证明 mutation（install/uninstall/marketplace add）默认放行**——「open」的核心行为未被锁定。

## Standards 轴（规范 / 可维护性）

### 硬违规

- **S1 [AGENTS.md 模块体积] `guard.rs` 898 行，超 800 硬限且本次使其增长。**
  `guard.rs` 原已 846 行；R8 把 `sandbox_applies_binds` / `ensure_encrypted_skill_sandbox` 从 core 搬**进** `guard.rs`（而非新模块），推至 898。
  AGENTS.md："If a file exceeds roughly 800 LoC, add new functionality in a new module."
  建议：这两谓词落新模块 `sandbox_policy.rs`，不要追加到已超限的 `guard.rs`。

### 判断项（judgement calls）

- **S2 [Middle Man + Divergent Change] `agent_security.rs` 退化为半使用的前转发层。**
  5 个 pub 项全是到 fm crate 的一行转发 + 一个 const 别名。网关不一致：app-server/cli 直接调 `fm_product_policy::*`，core 调用方却经 `codex_core::agent_security`。建议二选一（要么全走 fm，要么保留 core 适配层并让 app-server/cli 也走它）。
- **S3 [缺陷] `agent_security.rs:49 & 55` doc 注释首行重复。** 合并残留，删其一。
- **S4 [fmt] `config/mod.rs:695-700`** 两个 `#[derive]`（应合一）+ 701-702 双空行；`just fmt` 会修。
- **S5 [Primitive Obsession] `full_access_requested(sandbox_danger, permissions_danger)`** 函数体即 `||`，4 处调用点传两个位置 bool。可接受（集中了文案），但属弱抽象。
- **S6 [软目标] `paths.rs` 588 行 > 500。** tree-sitter 分类块（`parse_shell`/`collect_literal_tokens`/`is_top_level` + 三个主实现）是内聚的 AST 单元，可拆 `shell_classify.rs`。未触 800 硬限，优先级低。

### 已核 Clean

- 无 `collapsible_if` / `uninlined_format_args` / `redundant_closure` 违规（`format!`/`warn!` 参数均已内联）。
- 无新增 `#[async_trait]` / `async_fn_in_trait`。
- `argument_comment_lint` 在关键处遵守（如 `/*binds_active*/ true`）。
- `_ => {}` 通配是 spawn.rs 的**忠实搬运**（非新引入），且 ResponseItem 多为非文本变体，合理。

### RESIST-ADDING-TO-CORE 量化

core/src 净 **+506/-351 = +155 行**（23 文件），core 净增长。但增长合理：`config/mod.rs` +72（`into_sdk`/`audit_path`/`ProductPolicyRuntimeConfig`）替换了 session.rs 更冗长的内联映射；3 个薄 Session 访问器消除了 ~15 调用点的重复。搬出属实（spawn.rs 删 ~80 行遍历、ts_paths.rs/tests 共 -534 折入 paths.rs）。**非违规**，但 core 仍净增——后续批次应继续把核心逻辑外迁。

## 验证证据

- `just test -p fm-encrypted-skills` → **187/187 通过**（含 F01、R4 token、tree-sitter 差分、guard 表格化）。
  - 注：cargo 测试二进制 `NEEDED libfmsh_ukey_sdk.so.0` 但无 RPATH（rustc-link-arg 不传播，已知问题）；需 `LD_LIBRARY_PATH=<fmsh-ukey checkout>/9c47981/build/lib`。
- `just test -p codex-core spawn` → **105/105 通过**（含 R4 fork sanitization）。

## 待办（按优先级）

1. **[决策] P0** 确认插件/市场「默认放开」是否预期；补 mutation 默认放行的测试。
2. **[硬违规] S1** `guard.rs` 拆分（sandbox 谓词移至新模块）。
3. **[小修] S3/S4** 删重复 doc 行、跑 `just fmt`。
4. **[可选] S2** 统一 agent_security 网关路径。


---

## 第二轮：修复落地 + rust-skills 深度审计（2026-08-09）

用户确认 P0（插件/市场默认放开）符合预期。S1/S2/S3/S4 全部修复，并完成 rust-skills 逐条深度审计（锁/生命周期/unwrap/所有权）。

### 已落地修复

| 项 | 动作 | 验证 |
|---|---|---|
| S1 硬违规 | 新建 `fm/encrypted-skills/src/sandbox_policy.rs`，把 `sandbox_applies_binds` + `ensure_encrypted_skill_sandbox` 从 `guard.rs` 移出；`guard.rs` 898→833 行（脱离 800 硬限），清掉随之失效的 `FileSystemSandboxPolicy`/`NetworkSandboxPolicy` import。 | fm 190/190 |
| S2 网关统一 | 删除 `agent_security.rs` 的 4 个纯转发函数 + const 别名（`sandbox_applies_binds`/`sandbox_policy_bypassed(_for)`/`ensure_encrypted_skill_sandbox`/`SANDBOX_BYPASS_ENV_VAR`）；调用方直调源头——core 经 `fm_encrypted_skills::sandbox_policy::*`、app-server/cli 经 `fm_product_policy::*`；`agent_security.rs` 仅保留有真实状态的 `AgentSecurityContext`。 | core 编译通过 |
| S3 doc 重复 | 随 S2 删除一并消除。 | — |
| S4 fmt | `config/mod.rs` 双 `#[derive]` 合一、去双空行。 | `just fmt` 通过 |
| 测试迁移 | 把测 fm 逻辑的测试移到 fm crate：`sandbox_policy_tests.rs`（2 个）+ `product-policy/src/lib.rs` inline `bypass_env_only_accepts_exact_one`；core `agent_security_tests.rs` 精简为仅 `AgentSecurityContext` 测试。 | fm 190/190（+3 新）、core 1/1 |
| FFI 并发 | `sdk.rs` `cipher_for` 加 double-checked locking：write lock 下重新检查，消除并发下同 key 重复 `unwrap_key`（UKey 硬件调用）的 TOCTOU；快路径仍走 read lock。 | sdk 10/10（含 `shared_key_envelope_is_unwrapped_exactly_once_across_skills`） |

### rust-skills 深度审计结论

审计范围：runtime.rs（6×Mutex）、session_guard.rs（生命周期）、token.rs、guard.rs、paths.rs、sdk.rs、config/mod.rs。派发 scout 广扫 + 独立精读交叉验证。

**修复的真问题：**
- **err-01 [安全] `GuardDecision` 缺 `#[must_use]`**：调用方丢弃 `Blocked` 会让拦截静默失效。已加 `#[must_use]` 到 enum（`guard.rs:50`）。所有现有调用方均 `match`/`let` 处理返回值，无 warning。
- **conc-[并发] `cipher_for` 缺 double-checked locking**：两线程并发首次 miss 同一 `key.enc` 都会 `unwrap_key`（UKey 硬件调用），注释声称的 "exactly once" 不成立。已加 write-lock 下重新检查。非安全缺陷（解密结果相同、Arc 回收），是性能 + 注释准确性问题。
- **own-[死代码] `SessionGuard` 无用的 `#[derive(Clone)]`**：所有调用方都是临时链式调用，从不存储或 clone。已删 derive。

**确认为误判（scout 提出，我驳回）：**
- `unwrap_or_else(PoisonError::into_inner)`（runtime.rs:91/565 等）：正是 rust-skills 推荐的 fail-safe 锁中毒恢复，全代码库一致使用，**正确**。
- `ReasoningItemReasoningSummary` 的 `let ... = entry`（token.rs:124）：单变体枚举，irrefutable pattern，**不会 panic**。
- `original_dir.unwrap_or_default()`：path_mappings 已 filter 空值，package_path 非敌手直接控制 root。
- `tool_input.clone()`（guard.rs:172）：redact 场景必要（需修改副本），非热路径。

**判断项（记录，不改——避免扩大到 protocol 层）：**
- `_ => {}` 通配（token.rs ResponseItem/RolloutItem、guard.rs TurnItem）：是 spawn.rs 的忠实搬运，对应**非文本变体**（无需 redact），非缺陷。`ResponseItem`/`RolloutItem`/`TurnItem` 均无 `#[non_exhaustive]`（同 crate 可见），新增文本变体时 redaction 测试会捕获。加 `#[non_exhaustive]` 属 protocol 层大改，超本次范围。

**确认为 Clean：**
- **锁/await**：runtime 全同步方法，无 Mutex guard 跨 `.await`；锁序一致（唯一嵌套是 state→cache→registry @ load_or_register_inner 234-239，无反向路径，无死锁）；锁中毒恢复 fail-safe 一致。
- **生命周期**：`SessionGuard<'a>` 借用 `&'a runtime`，所有调用方临时使用即弃，无跨 await 存储，无悬垂引用。
- **FFI/并发**：`EnvelopeSdk: Send + Sync`；UKey `RwLock<HashMap>` 缓存。**已修复** `cipher_for` 的 TOCTOU（原 read-check→释放→unwrap→write-insert 缺 double-check，并发下同 key 重复 unwrap_key；现 write lock 下重新检查，让注释声称的 "exactly once" 成立；快路径仍 lock-free，吞吐影响小）。
- **错误处理**：`EnvelopeError` 用 thiserror + `#[from]`，消息 lowercase 无尾标点，fail-closed 默认。

### 最终验证

- `just fmt` 通过。
- `cargo check -p fm-encrypted-skills -p fm-product-policy -p codex-core -p codex-app-server -p codex-cli` 全通过（唯一 warning 是既有的 `before_tool_with_runtime_and_binds` dead code，非本次引入）。
- `just test -p fm-encrypted-skills -p fm-product-policy` → **190/190 通过**。
- `just test -p codex-core agent_security spawn` → **106/106 通过**。