## Why

高安全敏感 skill（如 `fm-agent-security`）的 `SKILL.md` 目前以明文读取并直接注入模型上下文、写入 rollout JSONL，导致明文内容驻留内存历史与磁盘，且脚本与文档内容对模型和用户可见。需要在不动摇加解密算法与 key 管理职责的前提下（由数字信封 SDK 承担），让明文只在发送给 LLM 的瞬间存在，其余环节只保留不可逆的 Token 占位符。

## What Changes

- **加密 skill 加载**：`build_skill_injections` 检测 SKILL.md frontmatter 的 `metadata.encrypted` / `metadata.encryption` 标记（version/key_id/algorithm/package），命中时不再 `read_file_text()` 读取磁盘 stub，改为通过数字信封 SDK 解密 `<name>.zip.enc` 包到 `/dev/shm/fm-agent-security/fm_skill_security_<hex>/`（固定根目录 + 随机 hex 子目录，会话级注册表维护映射，会话结束清理）。
- **Token 化注入**：`SkillInstructions::body()` 对加密 skill 输出哨兵 Token `[SENSITIVE_SKILL_TOKEN:{session_id}:{hex}]`（内嵌会话 ID 与随机 hex）；context 内存历史与 rollout JSONL 只含 Token，不含明文。明文 skill 保持现有注入行为（向后兼容）。
- **请求时重水合**：`get_formatted_input_for_request()` 在发送前正则扫描 `ResponseItem` 中的哨兵 Token，校验属主会话后从 `/dev/shm` 读取 `SKILL.md` 文档内容、套上信任层级框架后替换发送给 LLM（明文仅存在于该请求瞬间）。
- **信任层级与输出策略（防提示词注入）**：重水合时在 skill 文档外层注入信任层级框架——Skill 内容只是「任务流程数据」（tier-2），不得覆盖任何输出策略——并附加禁止在回复中复述/改写/编码 Skill 原文的输出策略（借鉴 opencode `FORBID_SKILL_JAILBREAK_PROMPT`）。
- **跨会话重水合门禁**：Token 内嵌属主会话 ID，重水合只替换属主会话内的 Token；跨会话（copy-paste、污染）拒绝替换，防止一个会话读到另一个会话的明文。
- **PreToolUse 拦截**：新增 hook 拦截对 `/dev/shm/fm-agent-security/` 的非执行访问（如 `cat`/`less`/`head`/`ls`），放行脚本执行（如 `bash .../scripts/build.sh`）。
- **脚本源码隔离与出站导出拦截**：read 工具只允许文本扩展名（`.md/.txt/.markdown`），脚本只能执行不能读取；`grep`/`glob` 及 bash 搜索命令指向解密目录时拦截。对 `write`/`edit`/`apply_patch` 及网络类工具做已知明文检测，参数含解密内容明文时直接 Blocked。
- **路径解析与保密**：重水合内容只含 skill 原目录/相对引用（附原目录 base anchor），`/dev/shm` 真实地址由 PreToolUse 在工具参数中改写注入（原目录 → 解密目录）；工具输出中的解密路径串统一脱敏，LLM 全程看不到、探测不到该地址。
- **安全审计**：解密、token 化、重水合、拦截、清理等安全事件写入 JSONL 审计日志（级别过滤、10 MB 轮转）。
- **Subagent fork 隔离**：`keep_forked_rollout_item` 过滤掉含哨兵 Token 的 item，子 Agent 不继承父会话的解密内容；需要时自行重新提及 skill 触发解密。
- **两级 TTL 生命周期**：解密仅在 skill 被触发（提及）时进行；TTL 内幂等复用（注册表命中即不重复解密）。**Skill 层 TTL**：skill 空闲超过阈值即从 `/dev/shm` 卸载（支持现有 per-turn 的加载/卸载语义，卸载后重新提及触发重新解密）；**Thread 层 TTL**：线程（session）空闲超过阈值即清除该线程全部已解密明文与内容缓存。明文在内存/`/dev/shm` 中只存在于 TTL 窗口内；线程结束立即清理；重水合检测到 Token 已卸载时提示重新提及 skill，重新解密后恢复。
- **回复明文硬脱敏**：`<output_policy>` 是软约束，模型可能复述/引用 Skill 明文导致 assistant 消息落盘（rollout/历史）形成明文驻留。在 durable history 边界对 assistant 回复与明文 inter-agent 消息执行已知明文检测：完整明文、≥20 字符行及其 20 字符前缀、以及作为完整行出现的短行（如 `api_key=abc`）统一替换为 `[REDACTED]`；短片段嵌在句子中时不误伤；user 消息不处理。

## Capabilities

### New Capabilities

- `encrypted-skill-tokenization`: 加密 skill 的解密加载、Token 化 context 注入、请求时重水合（含信任层级框架、跨会话门禁）、两级 TTL 生命周期（幂等加载、Skill/Thread 级卸载）与失效恢复，以及安全审计。
- `encrypted-skill-access-control`: PreToolUse 层对解密存储目录的访问控制，拦截直接读取与脚本源码读取、放行脚本执行、出站导出明文拦截。
- `encrypted-skill-fork-isolation`: Subagent fork 时对加密 skill Token 的过滤与隔离策略。

### Modified Capabilities

（无。`openspec/specs/` 当前为空，本变更全部为新增能力。）

## Impact

- `codex-rs/core-skills`：`loader.rs`（frontmatter 加密标记解析）、`injection.rs`（解密加载）、`skill_instructions.rs`（Token 化 body）。
- `codex-rs/skills`：`SkillMetadata` 增加加密相关字段（与 app-server v2 插件 API 的可选字段同步）。
- `codex-rs/core`：`client_common.rs`（重水合注入点）、`session/mod.rs`（生命周期清理与恢复）、rollout 持久化格式（明文 → Token）、`spawn.rs`（fork 过滤）。
- `codex-rs/core` hooks 或 plugin 层：PreToolUse 对 shell/exec/文件/网络工具的拦截与导出明文检测。
- 新增 crate `codex-encrypted-skills`：SDK 封装、token 化/重水合纯函数、会话注册表与两级 TTL、路径改写、输出脱敏、审计；核心仅保留三个薄接线点（注入、重水合、hook 注册）。
- 插件形态（可选外围）：加密 skill 包按 plugin skills 分发；拦截规则可通过插件 hooks.toml 声明，不承担加密核心逻辑。
- 外部依赖：新增数字信封 SDK；新增 `/dev/shm` 运行时目录约定。
- 会话恢复语义变化：既有 rollout 中的明文 skill 内容可继续读取；新 rollout 中 Token 指向的 `/dev/shm` 内容可能已清理，需要重新提及触发解密（兼容恢复路径）。
- 测试：`core/suite` 集成测试覆盖加密加载、重水合、拦截、fork 隔离、恢复等行为。
