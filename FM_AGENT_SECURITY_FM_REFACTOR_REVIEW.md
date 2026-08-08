# Agent Security fm/ 收拢 Review（对比 `rust-v0.146.0`）

> 口径：当前 HEAD（`9a12515769`）与 `rust-v0.146.0` 的全部 Rust 文件改动（159 个文件，其中 fm/ 29 个、非 fm/ 130 个）。
> 目标：判断非 fm/ 改动能否收拢到 fm/；识别跨文件重复代码；尽量少改原项目文件，以增量方式集成。
> 约定：fm/ = `codex-rs/fm/{encrypted-skills,license,product-policy}`。

## 一、结论摘要

1. 非 fm/ 改动绝大多数是必要适配（协议字段、模型字段、入口接线、sandbox 平台层），分层基本正确，不应强行收拢到 fm/。
2. 存在 6 类可收拢/可消除的重复（R1-R6），其中 3 类收益最高：
   - R1：full-access 产品策略判定在 app-server 重复 4 次、cli 1 次，收拢到 fm-product-policy；
   - R4：ResponseItem/RolloutItem 文本字段遍历在 core spawn.rs 与 fm guard.rs 结构重复，收拢到 fm 通用文本变换；
   - C1：加密技能配置类型与两段转换（core session.rs 60 行 match、config/mod.rs 30 行映射）整体收拢到 fm-encrypted-skills，原项目净减约 170 行新增。
3. 明确不应收拢：ReadonlyBind/FileSystemSandboxPolicy（protocol，跨 8+ crate 共享）、bwrap/landlock/exec-server 的 readonly_binds 执行、SkillMetadata 模型扩展、license 入口一行接线。

## 二、逐文件判断（非 fm/ 源码）

### 2.1 必须留在原项目（合理，不动）

| 文件 | 改动 | 判断 |
|---|---|---|
| protocol/src/permissions.rs | ReadonlyBind + FileSystemSandboxPolicy.readonly_binds | 跨 core/sandboxing/linux-sandbox/exec-server/file-system/cli 共享的协议类型；收拢到 fm 会破坏分层并形成依赖环。维持。 |
| protocol/src/models.rs | 构造器补 readonly_binds: Vec::new() | 必要适配。 |
| skills/src/model.rs、core-skills/src/model.rs、app-server-protocol/src/protocol/v2/plugin.rs | SkillMetadata.encrypted/encryption 模型扩展 | 各层领域模型，fm 不能定义；必须改原文件。已是最小字段扩展。 |
| linux-sandbox/src/bwrap.rs、linux_run_main.rs、sandboxing/src/landlock.rs、manager.rs、exec-server/src/fs_sandbox.rs、process_sandbox.rs、file-system/src/lib.rs | readonly_binds 透传与 --ro-bind 执行 | sandbox 平台执行层，属通用能力；收拢到 fm 会引入反向依赖。维持。 |
| cli/build.rs | libcrypto NEEDED + rpath | rustc-link-arg-bins 不能从依赖 crate 传播，必须留在最终二进制 crate。 |
| cli/src/remote_control_cmd.rs、tui/*、thread-manager-sample/src/main.rs | 薄适配 | 维持。 |
| app-server/src/error_code.rs | LICENSE_UNAVAILABLE_ERROR_CODE | app-server 错误码，维持。 |
| 各二进制入口（cli/app-server/exec/mcp-server main.rs） | fm_license::init_entry()? 一行 | 入口接线，一行调用不算重复。 |

### 2.2 已合理收拢到 fm（核心逻辑在 fm，宿主只剩薄适配）

| 文件 | 说明 |
|---|---|
| app-server/src/request_processors/rpc_guard.rs | engaged/path/command/args 判定全部委托 fm_encrypted_skills::rpc；本文件只剩 JSONRPC 包装与 ClientRequest 枚举匹配（协议知识，必须留 app-server）。 |
| core/src/encrypted_skills_guard.rs | 411 行适配层，全部委托 fm guard；本文件只做 core 类型（ToolOutput/GuardianApprovalRequest）包装。分层正确。 |
| core/src/encrypted_skills_periodic.rs | 只负责 tokio task 生命周期；sweep 语义在 fm runtime。可进一步收拢（见 R2 可选）。 |
| core-skills/src/injection.rs、loader.rs | 包名解析/前端解析属于技能模型层；解密/注册在 fm runtime。分层正确。 |
| core/src/agent/control/spawn.rs 的 token 剥离 | 当前在 core 实现（见 R4，建议收拢）。 |

### 2.3 建议收拢/消除的重复代码

| 编号 | 优先级 | 位置 | 重复内容 | 收拢方案 |
|---|---|---|---|---|
| R1 | 高 | app-server thread/turn_processor.rs 4 处 + cli main.rs 1 处 | 同样的 danger-full-access 布尔判定 | fm-product-policy 新增 full_access_requested(sandbox_danger, permissions_danger) -> bool；宿主只保留错误组装。 |
| R2 | 高 | core 约 25 处 | 反复传 (runtime, thread_id) 参数对 | fm 提供 EncryptedSkillGuard（runtime.guard(session_id) 句柄）；core 调用点改单句柄。 |
| R3 | 中 | app-server 3 个 processor | serde_json::to_value + ensure_args_not_guarded 3 处 | rpc_guard 提供泛型 ensure_serializable_args_not_guarded<T: Serialize>。 |
| R4 | 高 | core spawn.rs（约 90 行）vs fm guard.rs | ResponseItem/RolloutItem 文本字段遍历结构重复 | fm 提供 map_response_item_text / map_rollout_item_text / token::strip_tokens_from_rollout_items；spawn.rs 收拢为一行。 |
| R5 | 中 | core compact*.rs、prompt_debug.rs | EncryptedSkillRehydrator 构造重复 4 次 | 与 R2 合并：fm 提供 EncryptedSkillRehydrator::new，core 提供访问器。 |
| R6 | 低 | cli/build.rs（其他二进制如需要） | libcrypto/rpath 链接参数 | 共享脚本或在 fm 文档集中说明；Bazel 已用 linkopts 收拢。 |
| R7 | 中 | 7+ 个测试基建文件 | FMSH_CODEX_LIC_TEST_BYPASS 字面量重复 | 测试基建 helper 统一设置。 |
| R8 | 中 | core/src/agent_security.rs | 产品策略/环境判定放在 core，cli/app-server 都经 core 调用 | 收拢到 fm-product-policy（bypass 判定）与 fm-encrypted-skills（sandbox_applies_binds）。 |

### 2.4 配置类型收拢评估（C1）

codex-config 的 EncryptedSkillsToml/EncryptedSkillsSdkToml/SoftwareAlgorithmToml（约 80 行）与 core 的两段转换（session/session.rs SDK match 60 行、config/mod.rs 字段映射 30 行）属于同一领域，可整体收拢：

- fm-encrypted-skills 新增 config 模块：类型定义（serde/JsonSchema）+ Into<SdkKind> + into_runtime_config()（含 TtlConfig、默认 audit path、默认 key.enc）。
- codex-config 改为 pub use fm_encrypted_skills::config（原文件净减约 80 行）。
- core session/session.rs 的 60 行 match 删掉，改一行 sdk_for(config.encrypted_skills.sdk.into())；config/mod.rs 映射改一行。
- 依赖验证：codex-shell-command 不依赖 codex-config，fm 不依赖 config，codex-config 指向 fm 无环。
- 附带工作：fm 增加 serde/schemars 依赖；just write-config-schema 刷新 schema；bazel-lock-update。
- 净效果：原项目新增量减少约 170 行，fm 增加约 150 行；重复转换单一化。

**评估结论（更新）**：依赖无环，但让通用配置层 `codex-config` 依赖 fm-encrypted-skills 会把 tree-sitter/shell-command 拉进 config 依赖树，分层收益为负；且需改原项目 Cargo.toml。**建议不实施**，保持类型定义在 codex-config；仅把 core 内部转换收拢为 `EncryptedSkillsRuntimeConfig::into_sdk()` 方法（消除 session.rs 的长 match，不改跨 crate 依赖）。

## 三、实施批次建议

| 批次 | 内容 | 影响面 |
|---|---|---|
| P1 | R1（fm-product-policy full_access_requested）+ R3（rpc_guard 泛型 helper） | app-server/cli 小改，独立可验证 |
| P2 | R4（fm 通用文本变换 + token 剥离，spawn.rs 收拢） | fm + core spawn，需跑 spawn/encrypted_skills 测试 |
| P3 | C1（配置类型与转换收拢到 fm） | config/core/fm + schema，需全量 config/core 验证 |
| P4 | R2/R5（EncryptedSkillGuard/Rehydrator 句柄） | core 约 25 处调用点改写，最大收益也最大改动 |
| P5 | R7/R8（测试基建 helper、产品策略收拢） | 多 crate 引用改动 |

> 每批独立提交、独立验证；P2-P4 均不改变行为，只做结构收拢。

## 四、实施跟踪

| 批次 | 状态 | 验证 |
|---|---|---|
| P1（R1 + R3） | **已完成** | fm-product-policy 新增 `full_access_requested`；app-server 4 处 danger-full-access 判定改调用，3 处 to_value+guard 收拢为 `ensure_serializable_args_not_guarded`；cli `requests_full_access` 复用 fm 判定。app-server product_policy/rpc_guard 10/10、cli product_policy 2/2 通过；fmt/clippy 干净。 |
| P2（R4） | **已完成** | fm token.rs 新增 `for_each_response_item_text` / `strip_tokens_from_response_item` / `strip_tokens_from_rollout_item`；spawn.rs 的 ~90 行本地遍历删除，收拢为逐项调用。fm 187/187、core spawn 7/7 通过；fmt/clippy 干净。 |
| P3（C1） | **部分完成** | 类型收拢到 fm 会让 codex-config 依赖 tree-sitter，评估为不建议；已按修订方案落地：`EncryptedSkillsRuntimeConfig::into_sdk()` / `audit_path()` 收拢转换逻辑，session.rs 的 60 行 match 删除。core encrypted_skills 集成 19/19 通过。 |
| P4（R2/R5） | **已完成** | R2：fm 新增 `session_guard::SessionGuard`（`EncryptedSkillRuntime::guard(session_id)`），core `Session::encrypted_skills_guard()` 提供句柄；`(runtime, thread_id)` 参数对调用点从 25+ 处收拢为句柄方法（stream_events_utils、compact_remote×2、compact_remote_v2、codex_thread、hook_runtime、session/mod×5、turn×2、guardian/review、shell/unified_exec×2、orchestrator×2、registry、write_stdin），仅保留需要 owned Arc/String 的场景（RedactingToolOutput、AgentSecurityContext、build_skill_injections 跨 crate 参数）。R5：`Session::encrypted_skill_rehydrator()` 消除 5 处重复构造。fm 187/187、core 相关 30/30 通过；fmt/clippy 干净。 |
| P5（R8） | **已完成** | sandbox bypass 判定与常量收拢到 fm-product-policy（sandbox_policy_bypassed/for/SANDBOX_BYPASS_ENV_VAR）；sandbox_applies_binds/ensure_encrypted_skill_sandbox 收拢到 fm-encrypted-skills::guard；core agent_security.rs 只剩薄转发。fm 187/187、core agent_security 4/4、encrypted_skills 19/19、app-server/cli product_policy 通过；bazel-lock 无漂移。R7（测试基建 bypass helper）维持“收益低，暂不实施”。 |
