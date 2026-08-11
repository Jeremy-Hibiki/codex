# Agent Security 新增测试代码审查（对比 `rust-v0.146.0`）

> 口径：`git diff rust-v0.146.0..HEAD`，仅审查本次变更**新增的测试代码**（新增测试文件 + 既有测试文件的增量部分）。
> 审查目标：找出没必要、冗余、可合并、无实际意义的测试；同时评估缺失的测试面（判定表/边界）。
> 本文档为审查结论与优化清单；**问题统一记录后再统一优化**，优化落地情况在文末跟踪。

## 一、结论摘要

本次新增/修改的 Rust 测试涉及约 30 个文件、约 1.1 万行。总体质量中等偏上：

- **好的部分**：`fm-encrypted-skills` 的 runtime（TTL、并发、锁中毒、容量上限）、guard（路径/重写/红act 主分支）、bwrap readonly-binds、核心端到端 `encrypted_skills.rs` 覆盖扎实，边界意识明显（20 字符片段、TTL 60s、512 条目等）。
- **主要问题**：
  1. `core/src/agent/control/spawn_tests.rs` **没有覆盖新增逻辑**（`strip_encrypted_skill_tokens` 零测试），却放了 3 个断言同一旧行为的重复用例 —— 最典型的“为了测试而测试”。
  2. `guard_tests.rs` 75 个测试大量平铺单用例，约 40 个可合并为 ~10 组表格化测试；部分断言是恒真/空内容弱断言。
  3. 多个“允许/未 engaged”测试使用错误夹具或弱断言，几乎不可能失败（app-server `rpc_guard_allows_mem_root_paths_when_unengaged`、`read_only_plugin_listing_rpcs_are_not_blocked_by_product_policy`、`agent_security_tests::ensure_encrypted_skill_sandbox_gates_engaged_execution` 自我参照）。
  4. `ts_paths` 原本是 `#[cfg(test)]` 原型，306 行测试 + 228 行实现仅为“是否迁移”决策服务，且 `legacy_corpus_assertions_still_hold_for_ts` 与 differential 测试完全重复（**已按用户计划转正接入并合并进 `paths.rs`，见 T25**）。
  5. 判定表不完整：`binds_active=true` 的 view_image/export 分支、缺失/非字符串输入、未 engaged 的 redact 早退、fork 边界、stdin 守卫、RPC 全表（thread/goal/settings）等关键面没有测试。

> 更新（2026-08-08）：P0/P1 与部分 P3 已落地，见“四、优化批次建议”与“五、优化落地跟踪”；优化过程中额外发现并修复两个实现/测试环境问题，见“六、优化过程中新发现”。

## 二、逐文件问题清单

### 2.1 `codex-rs/fm/encrypted-skills/`

#### `src/guard_tests.rs`（75 个测试，问题最多）

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T01 | 可合并 | L272/284/299/393/410 | 5 个 `cat` 类“读取被拦”用例：mem-root 脚本、原始脚本、原始 SKILL.md、解密目录文本、解密目录脚本。同一 `non_execution_access` 分支，可合并为 1 个表格化测试（命令 × 路径）。 |
| T02 | 可合并 | L314/331/346 | 3 个 `grep` 用例（解密目录递归、原始目录递归、单文件）可合并。 |
| T03 | 可合并 | L360/376/478/1531 | 4 个“列举/查找”用例（ls、find、lsd/eza/tree/fd 已循环、ls mem-root）可合并（T03 中的 lsd 循环已是半表格化，其余并入）。 |
| T04 | 可合并 | L427/447/609/626/643 | 5 个“外带”用例（cp 脚本、tar 归档、cp 明文、重定向、base64）全部走同一 `non_execution_access` 判定，可表格化。 |
| T05 | 可合并 | L534/549/564 | 3 个链式走私用例（`;`、`&&`、`|`）可合并为循环。 |
| T06 | 可合并 | L723/736/1387/1399 | 4 个 `view_image` 用例（解密目录、未知 mem-root 子路径、默认 MEM_ROOT 前缀、外部路径允许）可表格化。 |
| T07 | 可合并 | L1423/1435/1450 | 3 个 MCP/extension 路径探测用例可合并。 |
| T08 | 可合并 | L1547/1559/1571/1588/1600 | 5 个“非 shell 工具参数含明文/不含明文”用例全部走 `guard_export` 同一代码路径，可合并为 `tool_name × payload` 表格。 |
| T09 | 可合并 | L1466/1493 | 两个 audit 记录用例（storage_probe reason / 具体 reason）setup 完全相同，可合并。 |
| T10 | 可合并 | L1108/1132 | `redacts_tool_output_text_plaintext_for_durable_surfaces` 与 `redacts_tool_output_content_items_for_durable_surfaces` 是同一函数两个 body 形态，可合并。 |
| T11 | 无意义/弱 | L1373 | `redact_turn_item_leaves_user_messages_untouched`：构造的 UserMessage `content: vec![]` 为空，即使实现错误地红act UserMessage 也会通过；断言只检查枚举变体未被替换。 |
| T12 | 命名误导 | L1217 | `redact_all_response_item_text_covers_every_role` 实际只覆盖 user Message + FunctionCallOutput，并未覆盖“every role”，名称与内容不符。 |
| T13 | 可合并 | L174-240 | stdin 守卫 6 个用例（原始路径、解密路径、无害命令、脚本执行、脚本后走私、未 engaged）中有 4 个可并入一个表格（原始/解密/无害/脚本执行），保留走私与未 engaged 两个独立用例。 |
| T14 | 可合并 | L242/262 | `is_skill_script_execution_detects_execute_only_commands` 与 `is_skill_script_execution_false_when_unengaged` 可合并为一张表格（命令 × engaged）。 |

#### `src/paths_tests.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T15 | 可合并 | L134-232 | `split_at_*` 共 13 个单用例（7 个分隔符 + 6 个非分隔符/空段/转义），可合并为 2 个表格化测试（分隔符表、非分隔符表），保留 1-2 个带注释的边界特例。 |
| T16 | 可合并 | L10-44 | 3 个 `rewrite_*` 用例可合并为 1 个表格（输入 × mappings × 期望输出）。 |

#### `src/export_guard_tests.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T17 | **重复** | L171 与 L193 | `redact_short_fragments_are_not_redacted` 与 `redact_short_line_embedded_in_a_sentence_is_kept` 完全同义：同样 plaintext、同样输入、同样断言（`prefix api_key=abc suffix` 原样保留）。应删除其一。 |
| T18 | 可合并 | L105/218/226/260 | 4 个幂等测试（middle fragment / 普通 / 短行 / 引号片段）可合并为 1 个表格化测试。 |
| T19 | 可合并 | L93-131 | 归一化相关（`normalized_variants_match`、`fullwidth_variants_match`、`redact_normalized_line_variant`、`normalized_short_line_redacts_as_complete_line_only`）可合并为 1-2 个表格化测试。 |
| T20 | 可合并 | L21-24 与 L162-168 | `exact_plaintext_matches` 与 `redact_exact_plaintext`、`no_known_plaintext_passes` 与 `redact_unknown_text_is_unchanged` 是 contains/redact 的同断言双份，可合并为“contains 与 redact 共用一张表”。 |

#### `src/audit_tests.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T21 | 无意义 | L47 | `event_type_and_session_id_helpers` 测试的是枚举静态 getter（`match` 常量），属于“为测试而测试”。 |

#### `src/token_tests.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T22 | 可合并 | L77/86/92 | `strip_tokens_replaces_all_sentinels` / `keeps_surrounding_text` / `without_sentinels_is_identity` 可合并为 1 个表格化测试。 |

#### `src/mem_root_tests.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T23 | 弱/静态 | L100 | `resolve_default_mem_root_returns_absolute_path` 在 Linux 上断言 `root == DEFAULT_MEM_ROOT`，即对静态常量断言（违反“不为静态定义值写测试”的仓库约定）。非 Linux 分支仅测绝对路径，可保留但应去掉 Linux 相等断言。 |

#### `src/ts_paths_tests.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T24 | **重复** | L268 | `legacy_corpus_assertions_still_hold_for_ts` 的 8 个断言全部包含在 `differential_command_references_dir` / `differential_script_execution_avoids_guarded_io` 的语料内且无 expected_mismatch，属于完全重复，应删除。 |
| T25 | 已转正并合并 | 全文件 | `ts_paths` 原型曾因“无迁移计划文档”被删除，后确认用户计划用 tree-sitter 辅助 shell 解析与拦截，故恢复并转正：tree-sitter 实现并入 `paths.rs` 作为命令分类主实现（references/split/avoids），手写扫描器保留为 parse 失败时的 fallback（并修复 fallback 丢失 `MEM_ROOT_PARENT` 边界检查的问题）；`guard.rs`/`rpc.rs` 经 `paths::` 调用新实现；差分测试并入 `paths_tests.rs`（ts 主实现 vs legacy fallback）；`tree-sitter`/`codex-shell-command` 移至正式 dependencies。语义改进生效：注释内路径不再误拦、heredoc body 作为 IO 通道拦截、尾随注释不参与命令段。 |

#### `src/sdk.rs`（inline tests）

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T26 | **恒真断言** | L467 `sdk_for_selects_implementations` | `assert!(matches!(sdk_for(SdkKind::TestZip).as_ref(), _))` 与 `sdk_for(SdkKind::Noop).as_ref(), _` 是恒真断言（`_` 匹配一切），必须删除或改为具体行为断言。 |
| T27 | 可合并 | L502/544 | `software_sdk_decrypts_hpke_package` 与 `software_sdk_decrypts_sm2_cms_package` 除算法常量外结构完全相同（各 ~70 行重复 setup），可提取 `decrypt_with_algorithm(alg)` helper 后参数化。 |

#### `src/runtime_tests.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T28 | 可合并 | L203/217 | `clear_thread_wipes_dirs_and_cache` 与 `unload_turn_wipes_dirs_and_cache` 结构一致、断言几乎相同（后者多一条 `decrypted_dirs` 空断言），可合并为 `reason` 参数化的 1 个测试。 |

#### `src/rpc_tests.rs` / `src/cache_tests.rs` / `src/registry_tests.rs` / `src/rehydrate_tests.rs` / `src/memfd.rs`

未发现无意义或重复问题；各用例行为区分度良好。`registry_tests` 中 `register_then_loaded_within_ttl` 与 `touch_skill_refreshes_last_used_at` 部分重叠但各有独立语义，可保留。

### 2.2 `codex-rs/fm/license/src/license_tests.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T29 | 重复/可合并 | L36 与 L44 | `missing_values_are_errors` 与 `missing_or_empty_server_is_an_error` 都测“feature 缺失/空是错误”，第二个测试注释还声称测“可选字段不报错”，但实际没有该断言。应合并为一张缺失字段表格。 |
| T30 | 可合并 | L24/30 | `display_name_defaults_to_codex` 与 `display_name_empty_string_falls_back_to_default` 可合并。 |
| T31 | 弱 | L51 | `license_state_is_active_by_default` 测静态初始状态（`is_active()` 默认 true），价值低。 |

### 2.3 `codex-rs/core/`

#### `src/agent/control/spawn_tests.rs`（新增，问题最突出）

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T32 | **无意义/重复** | L20/28/36 | 3 个 `user_message_*` 测试断言同一件事：`keep_forked_rollout_item` 对 user 角色恒返回 true（该函数不看内容）。新加的 token 剥离逻辑与这 3 个测试无关，注释也承认“At the keep/drop filter level the message is always retained”。 |
| T33 | 无意义 | L43-63 | `assistant_final_answer_is_kept`、`function_call_items_are_dropped` 测试的是本次变更**之前**就存在的 keep/drop 行为，与新增功能无关（新增逻辑是 `strip_encrypted_skill_tokens` 和 legacy checkpoint 判定）。 |

#### `src/agent_security_tests.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T34 | **自我参照/弱** | L82 | `ensure_encrypted_skill_sandbox_gates_engaged_execution` 用同模块的 `!sandbox_policy_bypassed()` 作为期望，等于用实现推导期望；即使 `ensure_encrypted_skill_sandbox` 实现错误，只要错误方向与 `sandbox_policy_bypassed` 一致测试仍通过。应改为固定输入断言（并显式设置/模拟 bypass 状态）。 |
| T35 | 可合并 | L47/64 | `sandbox_applies_binds_under_bwrap_profiles` 与 `sandbox_applies_binds_false_without_bwrap` 可合并为一张表格。 |

#### `src/encrypted_skills_guard.rs`（inline tests）

4 个用例本身质量尚可；缺失见 M13/M14。

#### `src/encrypted_skills_periodic_tests.rs`、`src/client_common_tests.rs`（新增部分）、`src/session/tests.rs`（适配改动）

质量好，无冗余；适配性字段填充属于必要改动。

#### `tests/suite/encrypted_skills.rs`（端到端）

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T36 | **冗余** | L460 与 L730 | `skill_ttl_expiry_forces_redecryption_on_reminder` 与 `re_mention_in_next_turn_redecrypts` 断言完全一致（每 turn 一个 token、两次不同），且 turn-end unload 已保证“每次提及都重新解密”，TTL 测试多等的 1.5s 不产生可观测差异 —— 两个测试不能互相区分，TTL 场景无法由该断言证明。建议删除其一，或在 TTL 测试中增加真正只依赖 TTL 的可观测断言。 |
| T37 | 可合并 | L286/332 | `direct_read_of_decrypted_script_is_blocked`（shell_command）与 `exec_command_direct_script_read_is_blocked_by_the_same_guard`（exec_command）结构一致，可参数化工具名；两者分别验证不同接线面，合并时应保留两个工具名。 |

### 2.4 `codex-rs/app-server/tests/suite/v2/`

#### `rpc_guard.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T38 | **弱/夹具错误** | L347 | `rpc_guard_allows_mem_root_paths_when_unengaged` 使用的路径 `/dev/shm/fm_skill_security_nonexistent/secret` 缺少 `/fm-agent-security/` 段，**根本不匹配守卫路径**；即使 engaged 也会放行，测试无法区分状态。应改用真实 MEM_ROOT 形态路径。 |
| T39 | 弱 | L316 | `rpc_guard_allows_config_mutation_when_unengaged` 只断言“错误消息不等于策略消息”，未断言请求真实通过（未 mock 配置写入结果）。可接受但价值有限。 |

#### `product_policy.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T40 | 可合并 | L45/60/86 | 3 个测试（thread_start / turn_start / thread_settings_update）setup 与断言完全同构，可表格化（方法 × 参数），减少 ~70 行样板。 |

#### `plugin_policy.rs`

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T41 | **弱/几乎恒真** | L86 | `read_only_plugin_listing_rpcs_are_not_blocked_by_product_policy`：只读 plugin/list 在 3 秒内成功时不会出现 error 消息 → 超时 → 测试通过；只有立即返回 POLICY_ERROR 才会失败。测试没有成功断言，本质上是“只要不报策略错就过”的冒烟，几乎不可能失败。 |

#### `license_gate.rs`

单用例质量尚可；缺失见 M20。

### 2.5 `codex-rs/cli/tests/`

#### `plugin_policy.rs`（8 个用例）、`product_policy.rs`（4 个用例）

| 编号 | 类型 | 位置 | 说明 |
|---|---|---|---|
| T42 | 可合并 | plugin_policy.rs L30-126 | 8 个用例全部同构（同一 `codex_command` + args + `stderr(contains(POLICY_ERROR))`），可合并为 `args` 表格驱动 1 个测试，保留 `plugin_management_is_rejected_even_with_plugins_enabled`（带前置 config）为独立用例。 |
| T43 | 可合并 | product_policy.rs L32-76 | 4 个 full-access 拒绝用例完全同构，可合并为 args 表格驱动 1 个测试。 |

### 2.6 `codex-rs/mcp-server/`、`codex-rs/sandboxing/`、`codex-rs/linux-sandbox/`、`codex-rs/core-skills/`

- `mcp-server/tests/suite/codex_tool.rs` 新增的 license-lost 用例质量好。
- `linux-sandbox/src/bwrap.rs` 新增 4 个 readonly-binds 用例 + 真实 bwrap 自跳过集成用例，覆盖到位。
- `core-skills` 新增的 injection/loader/skill_instructions 用例质量好，无冗余。
- `sandboxing`/`tui`/`skills`/`ext` 的改动均为结构字段适配，无新增冗余测试。

## 三、缺失测试面（M 清单）

### 3.1 判定表分析

guard 的核心决策面可以抽象为：`工具名 × 输入形态 × engaged × binds_active × 路径类型 × plaintext`。现有测试对“engaged=true、binds_active=false、字符串输入、路径类命令”这一主格覆盖密集；以下格几乎为空：

| 编号 | 缺失面 | 位置 | 说明 |
|---|---|---|---|
| M01 | `binds_active=true` 的 read/export 分支 | guard_tests | 仅 shell 有两例 binds 用例；`guard_read`/`guard_export` 的 `path_mappings` 扩展分支（binds_active 时原始逻辑路径应被拦/放行）无任何测试。 |
| M02 | 缺失/非字符串输入 | guard_tests | `tool_input` 无 `command`/`path` 键或值为非字符串时应 `Allow`，该分支无测试。 |
| M03 | 未 engaged 的 redact 早退 | guard_tests | `redact_assistant_reply_items`、`redact_tool_output_plaintext_for_persistence`、`redact_turn_item` 在 `known` 为空时的原样返回分支无测试。 |
| M04 | `redact_turn_item` 的 `CollabAgentToolCall` 分支 | guard_tests | match 中存在该 arm 但无用例；`McpToolCall.result` 等字段也未覆盖。 |
| M05 | `redact_json`/`redact_response_item`/`redact_payload`/`redact_storage_paths` | guard_tests / encrypted_skills_guard | 这些公开 guard 函数在 guard_tests 无直接用例（`redact_storage_paths` 仅经 adapter 间接覆盖）。 |
| M06 | `RedactingToolOutput::to_response_item` | encrypted_skills_guard | 最重要的消费面（工具输出 → ResponseInputItem 红act）无单测；`redact_json`/`redact_response_item` 方法也无测试。 |
| M07 | `before_tool_with_runtime_and_binds` 的 HookToolName 映射 | encrypted_skills_guard | hook 名到 guard 名的适配无测试。 |
| M08 | **fork 边界 token 剥离** | spawn_tests | 新增 `strip_encrypted_skill_tokens` 及 legacy checkpoint 判定（`preserve_reference_context_item`）**完全无测试**；同时集成套件也没有 fork 隔离用例。 |
| M09 | stdin 守卫端到端 | core/tests/suite/encrypted_skills.rs | `guard_stdin_input` 只有单测，无“运行中 shell 被注入明文/路径读取被拦”的集成用例。 |
| M10 | app-server RPC 全表 | rpc_guard.rs | engaged 时仅覆盖 fs/read、thread/shell、command/exec、process/spawn + 3 个配置方法；thread 目标/goal、metadata、settings update、turn 相关方法无覆盖。 |

### 3.2 边界与组合缺失

| 编号 | 缺失面 | 位置 | 说明 |
|---|---|---|---|
| M11 | 片段长度边界 | export_guard_tests | `MIN_FRAGMENT_LEN=20`：19 字符片段（应不匹配）与 20 字符片段（应匹配）的精确边界无测试；`MIN_QUOTED_FRAGMENT_LEN=12`：11/12 字符边界无测试。 |
| M12 | CRLF 与前导空白 | export_guard_tests | `redact_normalized_lines`/`redact_complete_short_lines` 显式处理 `\r\n` 与前导空白保留，但无对应用例。 |
| M13 | 空 known / 空 text / 空行 plaintext | export_guard_tests | `contains_known_plaintext(text, &[])`、空字符串、纯空行 plaintext 无测试。 |
| M14 | `redact_path_prefix` 多出现/开头/结尾 | paths_tests | 目前只有“中间出现一次”的用例；多次出现、串首、串尾、无后续路径段无测试。 |
| M15 | `contains_path_prefix` 非路径边界 | paths_tests | `/dev/shm-backup`、`/dev/shm.foo` 等“前缀后跟非路径字符”应不匹配，无测试（`/dev/shmx` 已覆盖）。 |
| M16 | IO 通道组合 | paths_tests | `script_execution_avoids_guarded_io` 的 `>(...)` 进程替换、允许型 heredoc（body 不含守卫路径）、`$(...)` 未加引号形式无测试。 |
| M17 | 脚本执行判定组合 | paths_tests | `sudo bash /x/run.sh`、`bash -e /x/run.sh`、无扩展名脚本等 runner/token 组合无测试。 |
| M18 | token 边界 | token_tests | `strip_tokens` 对未闭合 token、token 在串尾、相邻多 token 无测试。 |
| M19 | RPC 守卫 JSON 形态 | rpc_tests | `args_reference_guarded_path` 的嵌套数组/对象、非字符串值无测试；`is_guarded_path` 的组件边界（mem-root 兄弟目录）无测试。 |
| M20 | mem_root 初始化/遍历 | mem_root_tests | `init_mem_root_once` 的 once 语义、root 为普通文件、`a/../../x` 嵌套穿越、`secure_wipe` 对不存在目录无测试。 |
| M21 | 配置解析边界 | config_toml.rs inline tests | 未知 sdk 值、显式 `"unavailable"`、software 未指定算法时的默认值无测试。 |
| M22 | license 门“其他请求仍可用” | license_gate.rs | 文件头注释声称验证“其他请求仍可用”，实际只测 thread_start 被拒。 |
| M23 | TTL 集成可观测性 | core/tests/suite/encrypted_skills.rs | 现有 TTL 测试与 turn-end unload 不可区分（见 T36）；需要一个只在 TTL 内复用、TTL 外重解密的真正可观测断言（例如同一 turn 内重复提及不重解密？当前 turn-end unload 语义下需先明确设计）。 |
| M24 | 集成层“允许”面 | core/tests/suite/encrypted_skills.rs | 端到端只测了拒绝/红act/重解密，缺少“未提及加密 skill 时完全无感（rollout 无 token、请求无 framing）”的对照用例。 |

## 四、优化批次建议

按“先删无意义/恒真，再合并同构，最后补缺失面”的顺序，分 4 批统一落地：

1. **P0 删除/修正无意义与恒真断言**
   - T26（sdk 恒真断言）、T11（空内容 UserMessage 用例）、T21（静态 getter）、T17（export_guard 重复用例）、T24（ts_paths 重复用例）。
   - 修正 T38（rpc_guard 夹具路径）、T34（自我参照期望）、T41（plugin_policy 弱测试改为“读到非策略结果/成功响应”）。
   - 重写 spawn_tests：删 3 个重复 user_message 用例，改为覆盖 `strip_encrypted_skill_tokens`（各 ResponseItem 变体、Compacted/InterAgentCommunication、无 token 原样）。
2. **P1 表格化合并同构用例**（净减约 600-800 行）
   - T01-T08、T13-T16、T18-T20、T22、T27、T28、T35、T40、T42、T43。
   - 注意合并时保留每个工具名/分支的行为断言（T37 合并但保留两工具名）。
3. **P2 决策删除/收敛**
   - T25：与产品负责人确认 ts_paths 原型去留；保留则把 expected_mismatch 改为显式断言列表。
   - T36：删除 TTL 冗余用例或补真实 TTL 断言（先与实现语义对齐：turn-end unload 与 skill TTL 的关系）。
   - T23：去掉 Linux 静态断言；T31/T33 视维护价值删除。
4. **P3 补缺失面**（按安全优先级）
   - M01/M02/M03/M08/M10 优先（安全判定分支）；
   - M11-M17 边界补齐（导出守卫、路径解析）；
   - M20-M24 基础设施面。

## 五、优化落地跟踪

| 批次 | 状态 | 备注 |
|---|---|---|
| P0 删除/修正无意义断言 | **已完成** | T11 改为有内容断言；T12 重命名；T17/T21/T24/T26 删除；T38 夹具路径修正；T41 保留为弱冒烟（见下）；spawn_tests 重写（删 3 个重复用例，补 M08 全表面覆盖）。T34 保留（CI 下 `bypassed()` 恒 false，断言等价于固定期望）；T31 保留（`is_active()` 初始状态的唯一覆盖）。 |
| P1 表格化合并 | **已完成** | guard_tests 75 → 46 个用例；paths split 13 → 2；export_guard 幂等 4 → 1；token strip 3 → 1；runtime clear/unload 2 → 1；license 7 → 5；app-server product_policy 3 → 1；cli plugin_policy 8 → 4、product_policy 5 → 2；sdk software 2 → 1；core 集成 direct-read 2 → 1、TTL 冗余用例删除（T36）。 |
| P2 决策删除/收敛 | **已完成（含转正）** | T23 Linux 静态断言已删；T24 已删；T25 按用户计划将 ts_paths 转正并合并进 `paths.rs`（tree-sitter 主实现 + legacy fallback）。 |
| P3 补缺失面 | **已完成** | 已补：M01、M02、M03、M04（CollabAgentToolCall prompt）、M05、M06/M07、M08、M10（engaged 时 thread/name/set、thread/goal/set）、M11、M12、M13、M14、M15、M16、M17、M18、M19、M20（init_mem_root_once once 语义）、M21（未知 sdk 拒绝）、M22（license lost 时 config/read 仍可用）、M24（无关 turn 无 token/framing 集成对照）。**决定不补**：M09（曾尝试在原有测试文件 `unified_exec_tests.rs` 补 write_stdin 接线用例，为避免把审计驱动用例注入项目原有代码已撤销；`guard_stdin_input` 已有完整单测，集成层覆盖留待独立需求）；M23（TTL 语义已由 runtime 单测 `request_rehydration_enforces_ttl_for_idle_skills`/`periodic_sweep_*` 覆盖，集成层因 turn-end unload 无可区分断言，决定不补；若未来调整 turn-end unload 语义再评估）。 |

> 边界约定：本次审计的优化改动只落在**本次变更新增的文件**（新 crate 测试、新增测试文件、新增实现文件）与新增功能自身的配套测试（如 `config_toml.rs` 的 `encrypted_skills_toml_*`）；**不向项目原有文件注入审计驱动用例**。`paths.rs` 的 `>( ... )` 修复是测试暴露的真实安全缺口修复，属于实现修复而非审计痕迹。

## 六、优化过程中新发现

| 编号 | 类型 | 说明 | 处理 |
|---|---|---|---|
| F01 | **真实安全缺口** | 新增 `>(...)` 输出进程替换用例失败，暴露 `paths::script_execution_avoids_guarded_io` 只识别 `<( ... )`/`<<`，漏掉 `>( ... )`：`bash run.sh >(cat SKILL.md)` 可绕过守卫读取明文。 | 已修复 `paths.rs`：`>` 后跟 `(` 进入 substitution 深度检查；`script_execution_blocks_guarded_io_channels` 增加该用例；`fm-encrypted-skills` 181/181 通过。 |
| F02 | 测试环境非封闭 | app-server/cli 的 product-policy 测试在继承 `FMSH_CODEX_AGENT_SECURITY_SANDBOX_BYPASS=1` 的开发环境中全部失败（策略被 debug-only 旁路）。 | `product_policy` 测试显式 `env_remove` 该变量（app-server 用 `with_env_overrides(..., None)`；cli 用 `cmd.env_remove`），测试恢复封闭。 |
| F03 | 弱测试修复 | `rpc_guard_allows_mem_root_paths_when_unengaged` 原夹具路径缺少 `/fm-agent-security/` 段，形同虚设。 | 改为真实 MEM_ROOT 形态路径；仍断言“未被策略错误拦截”。 |
