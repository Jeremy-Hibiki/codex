# fm/agent-security 修复计划

范围：`feat/agent-security` 分支上 `fm/encrypted-skills`、`fm/license` 以及
`core`/`core-skills`/app-server 主机侧接线的 Rust 实现。Bazel/CI 相关项
（`MODULE.bazel.lock` 漂移、workspace `zip` default-features 收窄）按要求排除；
Skill 层 TTL 维持单层（by design），不实现 thread 级 TTL。

## 问题清单

| ID | 严重度 | 问题 | 位置 |
|----|--------|------|------|
| I1 | P1 | execute-only 可被脚本段内重定向/命令替换绕过 | `paths.rs::is_script_execution`、`encrypted_skills_guard.rs::guard_shell` |
| I2 | P1 | TTL 重载/并发加载/错误路径产生孤儿明文目录 | `runtime.rs::load_or_register`、`registry.rs::register` |
| I3 | P1 | 每个 Session 泄漏一个永不终止的周期 sweep 任务 | `encrypted_skills_periodic.rs`、`session.rs` |
| I4 | P1 | 解压无上限（ZIP 炸弹/OOM、inode 耗尽） | `sdk.rs::TestZipSdk` |
| I5 | P2 | 审计日志无时间戳、手拼 JSON 无转义、错误静默 | `audit.rs` |
| I6 | P2 | cache 淘汰与 registry 引用不一致，token 变 stale 且目录滞留 | `cache.rs::store`、`runtime.rs` |
| I7 | P2 | fork 隔离只剥离 user InputText，其他文本面可携带 token | `agent/control/spawn.rs` |
| I8 | P2 | `load_encrypted_skill` 忽略 frontmatter `encryption.package` | `core-skills/src/injection.rs` |
| I9 | P2 | 内存根命名空间目录权限默认 0755；初始化存在竞态 | `mem_root.rs` |
| I10 | P2 | `Token::parse` 不校验 hex 长度/session 字符集；`hex_encode` 每字节 format! | `token.rs` |
| I11 | P2 | `TestZipSdk` 可在生产配置中启用（模拟加密）而无提示 | `session.rs` SDK 选择处 |
| I12 | P2 | 无 framing 的 `Runtime::rehydrate` 等 public API 未使用 | `runtime.rs`、`rehydrate.rs` |
| I13 | P3 | audit sink 锁中毒时事件静默丢弃 | `audit.rs::FileAuditSink::emit` |
| I14 | P3 | 文件工具 relative-path 重写场景与“shell 为唯一通道”规格冲突 | `openspec/.../encrypted-skill-access-control/spec.md` |
| I15 | P3 | app-server `From<CoreSkillMetadata>` 硬编码 `encrypted:false` | `app-server-protocol/v2/plugin.rs` |
| I16 | P3 | 单变体枚举不可反驳 let（加变体即 panic 风险） | `encrypted_skills_guard.rs`、`spawn.rs` |
| I17 | P3 | 死代码 public API（`clear_all`、`redact_reply`） | `registry.rs`、`runtime.rs` |

## 修复方案

| ID | 方案 |
|----|------|
| I1 | 新增 `paths::script_execution_avoids_guarded_io`：脚本段引用受保护目录时，扫描重定向目标、`$(…)`、反引号、here-string、`<(`/`>(` 进程替换；命中即拦截。普通资源参数（`--input <dir>/config.json`）仍放行 |
| I2 | `Registry::register` 返回被替换记录；`load_or_register` 对替换/部分写入/锁中毒路径执行 `secure_wipe` |
| I3 | 周期任务改持 `Weak<EncryptedSkillRuntime>`，强引用消失后下一 tick 退出 |
| I4 | `TestZipSdk` 限制 512 条目、16 MiB 解压总量，读取用 `Read::take` 防元数据谎报 |
| I5 | `serde_json` 序列化 + `timestamp_ms` + Unix 0600 + 写入/轮转/刷新失败 `tracing::error!` |
| I6 | `ContentCache::store` 增加 `is_referenced` 回调，被 registry 引用的条目不淘汰 |
| I7 | fork 前对所有文本承载面（Message/AgentMessage/FunctionCall/Reasoning/InterAgentCommunication/Compacted）统一 `strip_tokens` |
| I8 | 优先使用 `encryption.package`，缺省回退 `<name>.zip.enc` |
| I9 | 初始化用 Mutex 串行化；命名空间父目录显式 `chmod 0700` |
| I10 | 校验 `hex.len()==32` 且 session 不含 `:`/`]`；`hex_encode` 用 `write!` |
| I11 | 配置选中 `TestZip` 时输出显式 `tracing::warn!`（模拟加密、非生产加密） |
| I12 | 删除 `Runtime::rehydrate`，`rehydrate_text` 收窄为 crate 内可见，测试改用 framing 版本 |
| I13 | 锁中毒时 `tracing::error!` 后返回 |
| I14 | 规格改为明确“非 shell 工具不得触达解密存储”，消除与 shell-only 实现的对立 |
| I15 | 复核：`codex_protocol::protocol::SkillMetadata` 本身无加密字段，该 `From` 无法透传；真实 catalog 路径已正确携带。无需代码改动，文档说明 |
| I16 | 保留 `let` 模式：单变体枚举加变体时编译器强制更新，`match + _` 会产生 unreachable pattern 警告 |
| I17 | 删除 `Registry::clear_all`、`Runtime::redact_reply` |

## 测试方案

| ID | 测试 |
|----|------|
| I1 | `paths_tests`：`<`、`$(…)`、反引号、`<(...)`、here-string 全部拦截；普通资源参数、`> /tmp/log`、`2>&1`、单引号字面量放行。`encrypted_skills_guard_tests`：整段命令级回归 |
| I2 | `runtime_tests`：TTL 过期重载后旧目录被 wipe；两个线程并发 `load_or_register` 后命名空间下仅剩一个目录 |
| I3 | `encrypted_skills_periodic_tests`：runtime 强引用释放后任务 `is_finished()` |
| I4 | `sdk_tests`：超限条目数、超限解压大小均返回对应错误 |
| I5 | `audit_tests`：JSON 可解析、引号转义、`timestamp_ms` 存在 |
| I6 | `cache_tests`：被引用条目在 cap 触发时不淘汰 |
| I7 | `spawn_tests`：`user_message_with_encrypted_skill_token_is_kept` 等 fork 用例保持通过 |
| I8 | `codex-core-skills` 既有注入测试保持通过 |
| I9 | `mem_root_tests` 既有权限/清理用例保持通过 |
| I10 | `token_tests`：短 hex、含 `:`/`]` session 被拒绝 |
| I11 | 编译级验证 + 日志可见性检查（`tracing::warn!`） |
| I12 | `runtime_tests`/`periodic_tests` 改用 `rehydrate_framed` 断言 |
| I13 | 代码审查 + 编译验证 |
| I14 | 规格文档更新（无代码测试） |
| I15 | 无 |
| I16 | 无（编译器保证） |
| I17 | `registry_tests` 移除 `clear_all` 用例；`runtime_tests` 移除 `redact_reply` 用例 |

## 状态

全部完成。I1–I10、I17 在计划文档提交前完成（见
`FM_AGENT_SECURITY_FIX_LOG.md`）；I11–I13 为代码修复、I14 为规格对齐，
在本轮按计划完成并单独提交。I15 复核为“无需代码改动”（源类型无加密字段，
真实 catalog 路径已携带）；I16 保留 `let` 模式（加变体时由编译器强制更新，
`match + _` 会产生 unreachable pattern 警告）。
