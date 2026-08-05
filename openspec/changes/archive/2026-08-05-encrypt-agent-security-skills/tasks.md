> 开发方法：TDD（红-绿-重构）。每个功能组先写失败的单测（红），再实现到通过（绿），最后重构。单元测试放 `*_tests.rs` 独立文件（`#[path]` 引入，遵循仓库规范），用可注入时钟/mock SDK 避免环境依赖；agent 行为层面按仓库要求补 `core/suite` 集成测试。

## 0. 工程形态：独立 crate 骨架

- [x] 0.1 在 Cargo workspace 新增 `codex-encrypted-skills` crate（依赖 codex-skills / codex-protocol / codex-utils-*），更新 MODULE.bazel.lock 与 BUILD.bazel
- [x] 0.2 定义 crate 公共 API：可 mock 的数字信封 SDK trait、token 类型、会话注册表、重水合/路径改写/脱敏纯函数（无 IO 依赖），建立 `*_tests.rs` 骨架
- [x] 0.3 确认三个接线点签名：`build_skill_injections`（注入）、`get_formatted_input_for_request`（重水合）、hook 注册（拦截/改写）

## 1. 加密元数据解析（loader，TDD）

- [x] 1.1 写失败单测（红）：`loader_tests.rs` 新增 frontmatter 解析用例——`metadata.encrypted: true` + `metadata.encryption`（version/key_id/algorithm/package）、缺省为明文、YAML 损坏报错
- [x] 1.2 实现（绿）：扩展 `SkillFrontmatterMetadata` / `ParsedSkillFrontmatter`，`SkillMetadata` 增加加密字段（含 app-server v2 可选字段）；未标记 skill 行为不变
- [x] 1.3 重构 + 全绿：运行 codex-core-skills 单测

## 2. 数字信封 SDK 与解密目录（TDD）

- [x] 2.1 写失败单测（红）：mock SDK——按 `<name>.zip.enc` 解密到 `/dev/shm/fm-agent-security/fm_skill_security_<hex>/` 并原样展开（SKILL.md/scripts/agents/references/templates）、Zip-Slip 拒绝、解密失败返回 warning、目录 0700、memRoot 启动初始化（清残留）
- [x] 2.2 实现（绿）：SDK 封装 + 目录布局 + memRoot 初始化
- [x] 2.3 单测：secure wipe（随机覆写后删除）与内容缓存去重/上限/FIFO 淘汰

## 3. Token 化注入（core-skills，TDD）

- [x] 3.1 写失败单测（红）：`skill_instructions_tests.rs`——加密分支 `body()` 输出 `[SENSITIVE_SKILL_TOKEN:{session_id}:{hex}]`、明文分支保持原格式；token 序列化/解析纯函数（sid/hex 提取、非法格式拒绝）
- [x] 3.2 实现（绿）：`build_skill_injections` 加密分支解密/幂等复用（注册表命中不重复解密）、`SkillInstructions::body()` 分支输出；解密失败产出 warning 与指标
- [x] 3.3 重构 + 全绿：运行 codex-core-skills 单测

## 4. 会话注册表与两级 TTL（TDD）

- [x] 4.1 写失败单测（红）：可注入时钟——幂等命中（TTL 内再次触发复用）、`last_used_at` 刷新（提及注入 + 重水合命中）、Skill TTL 过期卸载单个目录、Thread TTL 过期清空全部、线程结束立即清理
- [x] 4.2 实现（绿）：注册表（sessionID → skillName → {dir, loaded_at, last_used_at}）+ turn 边界/线程事件 TTL 扫描 + `skill_idle_ttl` / `thread_idle_ttl` 配置项
- [x] 4.3 重构 + 全绿

## 5. 请求时重水合（core，TDD）

- [x] 5.1 写失败单测（红）：`rehydrate` 纯函数——token → 内容替换、跨会话拒绝、失效 token 保留、信任层级框架包裹、base anchor 注入、内容中不含 `/dev/shm` 地址
- [x] 5.2 实现（绿）：`rehydrate_encrypted_skills` + 接入 `get_formatted_input_for_request`（常规 turn / compaction / 恢复路径）
- [x] 5.3 重构 + 全绿：运行 codex-core 相关单测

## 6. PreToolUse 路径改写与访问控制（TDD）

- [x] 6.1 写失败单测（红）：路径改写纯函数（原目录 → 解密目录，命令串与文件参数）、无关路径放行、读取命令拦截、扩展名白名单、输出路径脱敏（`/dev/shm/fm-agent-security` → `[REDACTED]`）
- [x] 6.2 实现（绿）：hook 接入——拦截直接读取、放行并改写执行型命令、脚本源码隔离、grep/glob 目录拦截、输出脱敏
- [x] 6.3 重构 + 全绿

## 7. 出站导出拦截（TDD）

- [x] 7.1 写失败单测（红）：参数明文检测——write/edit/apply_patch/网络工具含已知明文 Blocked、无明文放行、JSON 编码绕过不生效
- [x] 7.2 实现（绿）：复用内容缓存的明文集合做参数检测
- [x] 7.3 重构 + 全绿

## 8. Fork 隔离（TDD）

- [x] 8.1 写失败单测（红）：`keep_forked_rollout_item` 过滤含哨兵 token 的 item、普通 item 保留
- [x] 8.2 实现（绿）
- [x] 8.3 重构 + 全绿

## 9. 集成测试（core/suite，agent 行为兜底）

- [x] 9.1 加密 skill 提及 → context/rollout 只含 Token，不含明文；磁盘 stub 不被注入
- [x] 9.2 重水合请求 body 含真实 SKILL.md 内容 + 信任层级框架，且不含 `/dev/shm` 路径；明文仅存在于请求瞬间
- [x] 9.3 原地址 → 解密地址改写执行、工具输出路径脱敏
- [x] 9.4 PreToolUse 拦截直接读取、放行脚本执行
- [x] 9.5 fork 丢弃 Token、多会话隔离、会话恢复与失效重提
- [x] 9.6 两级 TTL（幂等复用、Skill 卸载、Thread 清空、卸载后重提重新解密）
- [x] 9.7 出站导出拦截、审计日志事件

## 10. 收尾

- [x] 10.1 运行 `just fmt` 与 `just fix -p codex-core-skills` / `just fix -p codex-core`，执行相关 `just test -p` 验证

## 11. 深度 Review 轮：回复明文脱敏（TDD）

- [x] 11.1 写失败单测（红）：`redact_known_plaintext`（完整明文、整行、20 字符前缀、短片段不误伤、多明文、幂等）+ runtime `redact_reply` 会话隔离 + core `redact_assistant_reply_items`（assistant 消息 InputText/OutputText、AgentMessage 明文内容、user 消息不动）
- [x] 11.2 实现（绿）：`export_guard::redact_known_plaintext`、`EncryptedSkillRuntime::redact_reply`、`encrypted_skills_guard::redact_assistant_reply_items`；`prepare_conversation_items_for_history`（durable history 边界）统一接线，覆盖 `record_conversation_items` 与 `record_inter_agent_communication`
- [x] 11.3 短行加固（红→绿）：SKILL 中 <20 字符的行以完整行形式出现在回复中时同样脱敏；嵌在句子中不脱敏；幂等
- [x] 11.4 原路径读/搜索绕过修复（红→绿）：`cat /skills/foo/SKILL.md`、`grep -r ... /skills/foo` 改写前检查改写结果，读/搜索命令命中解密目录即 Blocked；脚本执行仍正常改写放行
- [x] 11.5 回归：encrypted-skills 95/95、guard 18/18、集成 10/10（`RUST_MIN_STACK=16MiB`，测试基建栈溢出与本次改动无关，stash 复现确认）、`just fmt` + clippy 全绿
- [x] 11.6 流入口脱敏（红→绿）：`handle_output_item_done` 入口对模型输出项先脱敏——发现 `record_conversation_items` 之外还存在 `TurnItem`（ItemStarted/ItemCompleted）、`TaskCompleteEvent.last_agent_message`、`ResponseItem::Reasoning` 三条明文落盘副本；统一在流入口处理后全部干净，`emit_turn_item_*` 保留二次兜底
- [x] 11.7 E2E 加固：`encrypted_skill_keeps_plaintext_out_of_context_and_rollout` 的 mock 回复改为引用 `REAL_SKILL_CONTENT_MARKER`，断言 rollout 只含 `[REDACTED]`；guard 22/22、集成 10/10
- [x] 11.8 网络/扩展工具明文导出拦截（红→绿）：与部署汇报 V1 矩阵核对——`webfetch`/`web_search` 参数含明文必须拦截；guard_export 扩展为所有非 shell/view_image 工具默认分支（含 `webrun`/MCP/扩展工具），shell 保留为唯一运行时明文通道；guard 25/25、集成 10/10
