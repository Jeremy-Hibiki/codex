## Context

当前 skill 注入链路（`build_skill_injections` → `SkillInstructions` → `record_conversation_items`）会以明文读取 `SKILL.md` 并把完整内容放入模型上下文、内存历史与 rollout JSONL。对 `fm-agent-security` 这类高安全敏感 skill，明文驻留意味着模型、用户、以及磁盘上的 rollout 都能看到完整内容。

分析文档（`analysis/FM_AGENT_SECURITY_SKILL_ENCRYPTION_IMPLEMENTATION.md`）验证了关键注入点：

- `build_skill_injections`（`core-skills/src/injection.rs:72`）通过 `fs.read_file_text()` 读明文；
- `SkillInstructions::body()`（`core-skills/src/skill_instructions.rs`）把 `name/path/contents` 原样拼进 context；
- `get_formatted_input_for_request`（`core/src/client_common.rs:52`）是发送给 LLM 前的最后一跳，`strip_image_details` 已证明在该点遍历改写 `ResponseItem::Message.content` 是既有模式；
- 常规 turn、compaction、恢复会话的 LLM 请求全部经过该重水合点；
- Guardian review 通过 `CONTEXTUAL_USER_FRAGMENTS` 过滤天然跳过 skill 内容，不需要改动；
- PreToolUse hook 能拦截 shell/exec 命令；`/dev/shm` 已被 bwrap/landlock 支持为沙箱内 writable root。

约束：不加解密算法与 key 管理代码（由数字信封 SDK 承担）；不引入 Subagent 作为执行模型；保持明文 skill 向后兼容。

**参考实现（subagent 版）**：opencode 仓库 `packages/opencode/src/plugin/fm-agent-security` 已落地一版以 `encrypted-skill-executor` 子代理隔离的完整实现，本方案按其关键机制重新落位到主 Agent 模型：

- 主 Agent 调用 `skill()` 被拦截，返回 `delegate_required` 委派指令；子代理独享解密，主 Agent 只见 Token 与脱敏摘要；
- Token 哨兵 `[SENSITIVE_SKILL_TOKEN:<sessionID>:<hex>]`，重水合按属主会话门禁（`rehydrateText` 校验 token 内嵌 sessionID）；
- 解密目录 `/dev/shm/fm-agent-security/fm_skill_security_<hex>/`（固定根 + 随机 hex 子目录），`sessionID → Map<skillName, dirPath>` 注册表维护会话映射；
- `FORBID_SKILL_JAILBREAK_PROMPT` 反越狱提示词（output_policy / injection_defense / on_injection_attempt）经 hook 硬注入，不依赖 LLM 原样转述；
- `[SKILL-EXEC:flowID]` 标记 + `session.created` 确定性标记 + `chat.message` 校验构成 executor 身份注入与防伪造；
- read/bash/grep 的脚本源码隔离、出站 `deterministicRedact` 与导出工具参数明文拦截、JSONL 审计日志、secure wipe 清理。

主 Agent 模型的本质差异：明文边界从「子代理隔离」变为「请求瞬间重水合」，因此**不需要** flowID 委派身份注入（无子代理边界），但**必须**用信任层级框架补齐反越狱提示词，并用出站导出拦截补上「模型见过明文后可能复述/外泄」的通道。

**加密包实际形态**（`opencode-plugins/new-plugins/.opencode/skills/*` 验证）：

- 磁盘上的 `SKILL.md` 是 **stub**：frontmatter 携带 `metadata.encrypted: true` 与 `metadata.encryption: {version, key_id, algorithm, package}`（如 `algorithm: ZIP-AES-256-CBC`、`package: <name>.zip.enc`），正文是 `<!-- ENCRYPTED:SKILL -->` 提示标记；
- 真实内容在 skill 目录下的 `<name>.zip.enc` 包内：完整 `SKILL.md` + `scripts/`、`agents/`、`references/`、`templates/` 等（当前实现为普通 ZIP 的模拟加密，生产目标是 AES-256 硬件 key 加密）；
- 磁盘上的其余文件（LICENSE.txt、templates/ 等）均为解密前的 stub，不作为注入来源；
- 加密检测 = frontmatter `metadata.encrypted` 标记（或 `<name>.zip.enc` 存在），不需要独立元数据库。

## Goals / Non-Goals

**Goals:**

- context 内存历史与 rollout JSONL 中只有 Token 占位符，不含明文；
- 仅在发送给 LLM 的请求瞬间重水合真实 `SKILL.md` 文档内容；
- 解密内容与会话绑定，会话结束清理，多会话互不干扰；
- 明文只在「触发 → TTL 窗口」内存在：Skill 级 TTL 卸载单个 skill，Thread 级 TTL 清空整个线程；
- 拦截对解密存储的直接读取，放行脚本执行；
- 明文 skill 保持现有行为，零破坏。

**Non-Goals:**

- 不实现加解密算法、key 生成与保管（SDK 的职责，仅定义接口约定）；
- 不实现"运行时安全"的输入端 safe guard 模型（未来单独交付）；
- 不加密非 skill 资源（如普通文档、配置）；
- 不在本变更中引入 Subagent 执行模型；
- 不做 Guardan review 相关改动（现有过滤已跳过 skill 内容）。

## Decisions

### 1. 解密落地到 `/dev/shm/fm-agent-security/fm_skill_security_<hex>/`

固定根目录 `/dev/shm/fm-agent-security`（启动时初始化、chmod 0700、清空残留），每次解密生成随机 `fm_skill_security_<hex>` 子目录。SDK 输入为 skill 目录下的 `<name>.zip.enc`，解出的包内结构原样落到子目录：`SKILL.md`（真实内容，非磁盘 stub）/ `scripts/` / `agents/` / `references/` / `templates/`。会话与目录的对应关系由进程内注册表维护：`sessionID → Map<skillName, dirPath>`，重水合与清理都通过注册表定位，目录名不携带 session 信息（不可预测、不暴露关联）。

内存文件系统保证读写延迟 <1ms，且不写磁盘；随机目录名避免跨会话路径可预测。

**替代方案**：会话 ID 命名空间目录（如 `/dev/shm/.../{session_id}/`）——目录名可预测且把会话关联写入文件系统；进程私有 tmpfs 挂载点——隔离更好但沙箱配置复杂。均否决，与 opencode 参考实现保持一致。

### 2. Token 哨兵 `[SENSITIVE_SKILL_TOKEN:{session_id}:{hex}]`

Token 携带属主会话 ID（门禁用）与内容缓存 key（重水合用），与解密目录名相互独立；明文 skill 沿用现有 `<skill>...</skill>` 格式，按 `is_encrypted` 分支。

**替代方案**：直接暴露绝对路径的注入格式——违背路径隐藏约束，已否决。

### 3. 重水合固定在 `get_formatted_input_for_request`

这是所有 LLM 请求（常规 turn、compaction、恢复）共用的最后跳转点，一次改动全覆盖。参照 `strip_image_details` 的遍历改写模式。

**替代方案**：在 `build_prompt` 或各调用点分别处理——点多易漏、与 compaction/恢复路径重复实现，已否决。

### 4. PreToolUse 拦截采用「语义化仅执行」(execute-only)模型

拦截不是命令名黑白名单，而是**语义判定**：对命令按未引用的链式操作符（`;` `|` `&&` `||` 背景 `&`）切段，先做原目录→解密目录路径改写，再逐段判断——任何段引用内存根或已解密目录、且不是脚本执行（`is_script_execution`）即 Blocked。这样 `cat SKILL.md`、`cp <dir>/SKILL.md /tmp`、`cat <dir>/SKILL.md > /tmp/leak`、`base64 <dir>/SKILL.md`、glob 探测（`cat /dev/shm/.../p*/f*/SKILL.md`）、跨会话目录、`grep -r` 全部被拦截，且 `bash run.sh; cat SKILL.md` 这类链式走私也被逐段判定命中。判定统一在改写后的命令上做，避免原路径/解密路径不一致漏判。`command_references_dir` 对 `MEM_ROOT` 常量做子串匹配，glob 与跨会话路径天然命中，无需专门处理。

**演进**：初版用「命令名黑名单/脚本扩展名分类」(`is_read_command`/`command_targets_script`)，但该方案无法覆盖 `cp`/`mv`/重定向等逃逸路径，且 `cat` `.md` 等文本读取会放行——使 token 化与信任层级框架两条防线失效（明文经工具输出常驻内存历史且无框架包裹）。命令名分类函数已删除，改为纯语义段判定。

**替代方案**：命令名黑名单/白名单——`cp`/`mv`/重定向不在列表内即逃逸；脚本扩展名分类——`.md`/`.txt` 明文仍可读。均否决。

### 5. Subagent fork 剥离哨兵 Token（不丢弃整条消息）

`keep_forked_rollout_item`（`spawn.rs`）对 user 消息不再整体丢弃，而是在 fork 管线（`retain_forked_item`）中对 user 消息逐 content item 调用 `token::strip_tokens` 把哨兵替换为占位文本 `[encrypted-skill unavailable in this context]`，保留周围的用户指令。token 可能与用户正文同处一条消息，整体丢弃会丢失子 Agent 需要的用户指令；剥离只去掉解密句柄。

**演进**：初版用「整体丢弃含 token 的 user item」，但会连带丢弃同消息的用户指令。改为 token 剥离 + 保留正文。`contains_encrypted_skill_token` 辅助函数已随该改动删除。

### 6. 生命周期：触发解密 + 两级 TTL + 幂等复用

**解密时机**：仅在 skill 被提及/触发时解密（`build_skill_injections` 命中加密标记后），不预加载。

**幂等加载**：注册表条目为 `sessionID → Map<skillName, { dir, loaded_at, last_used_at }>`。再次触发时先查注册表：条目存在且目录仍有效（未超过 Skill TTL）即直接复用，不重复解密（对应 opencode `decryptSkillPackage` 的 cache-hit 语义）；`last_used_at` 随每次提及/重水合刷新。

**Skill 层 TTL（per-turn 语义）**：turn 不再提及该 skill 时，context 层即不再注入（现有行为）；`/dev/shm` 中的解密目录与内容缓存条目在空闲超过 `skill_idle_ttl` 后由扫描逻辑卸载（secure wipe）。下次提及重新解密。

**Thread 层 TTL**：线程（对应 opencode 的 session）空闲超过 `thread_idle_ttl` 后，清除该线程注册表中的**全部**解密目录与内容缓存（secure wipe），线程恢复活动时按需重新解密。线程结束 = Thread TTL 立即触发。

**扫描时机**：在 turn 边界与线程事件（结束/空闲检测）上执行 TTL 扫描，避免后台常驻定时器；扫描只遍历活跃线程的注册表，开销为 O(活跃解密目录数)。

**失效重提**：重水合发现 Token 指向已卸载内容时保留占位符并提示重新提及；恢复会话读取旧 rollout 时，明文项照常读取，Token 项按失效路径处理。

### 7. Token 哨兵与跨会话门禁（借鉴 opencode）

采用正则可扫描的哨兵格式 `[SENSITIVE_SKILL_TOKEN:{session_id}:{hex}]`，token 内嵌属主会话 ID（`hex` 为内容缓存 key，非目录名）。重水合先校验 token 属主与当前会话一致，跨会话（copy-paste、fork 残留、compaction 串扰）一律拒绝替换。

**替代方案**：无会话字段的裸 token——无法区分属主，跨会话泄漏风险高，已否决。

### 8. 重水合注入信任层级与输出策略

重水合时在 `SKILL.md` 文档外层注入信任层级框架：Skill 内容是 tier-2「任务流程数据」，只决定做什么、怎么做，**不能**覆盖任何输出策略；用户请求与外部文件是 tier-3 不可信数据。同时注入输出策略：禁止在回复中复述/释义/编码 Skill 原文与内部文件元数据。

对应 opencode 的 `FORBID_SKILL_JAILBREAK_PROMPT`。主 Agent 模型没有独立的 executor 提示词可注入，所以框架随重水合内容一起进入请求，是防 SKILL.md 自身夹带注入指令或用户诱导复述的主要防线。

### 9. 出站导出拦截（内容缓存 + 参数明文检测）

为每个会话维护明文内容缓存（去重、硬上限、FIFO 淘汰），编译为待匹配明文集合。PreToolUse 对 `write`/`edit`/`apply_patch` 及网络类工具的参数逐字段做明文检测，命中即 Blocked（对应 opencode `EXPORT_TOOLS` + `argsContainPlaintext`）。

### 10. 生命周期加固与审计

- 启动时初始化 memRoot（0700、清空残留目录）；
- TTL 扫描在 turn 边界与线程事件上执行：Skill 级空闲超限卸载单个目录，Thread 级空闲超限清空整个线程；
- 线程结束按注册表对每个 `fm_skill_security_<hex>/` 目录做 secure wipe（随机覆写后删除）；
- 每会话内容硬上限（条目数 + 总大小），超限拒绝/淘汰并告警；
- 安全事件（解密、token 化、重水合、拦截、清理）写入 JSONL 审计日志，级别过滤、10 MB 轮转，日志不含明文。

### 11. 触发检测：无 skill 工具时的「触发」定义

Codex 没有 skill 工具，触发由**宿主侧在 turn 开始时对当前用户输入做提及扫描**确定，不依赖 LLM 行为：

- `collect_explicit_skill_mentions(user_input, skills, ...)`（`core-skills/src/injection.rs:160`）在 `build_skills_and_plugins`（`turn.rs:720`）中执行，产出 `mentioned_skills` 集合；
- 三种触发形态：① 文本 sigil `$skill-name`（`$` 为 `TOOL_MENTION_SIGIL`）；② 链接/路径 `[name](skill://path)` 或指向 `SKILL.md` 的路径；③ app-server v2 的结构化输入 `UserInput::Skill { name, path }`（`protocol/user_input.rs`）；
- 触发集非空即进入 `build_skill_injections`（解密/复用点）；扫描**只针对当前 turn 的用户输入**，模型回复中的 `$skill` 不会触发——这就是 per-turn 加载/卸载语义的来源；
- `last_used_at` 刷新点：提及注入成功 + 重水合命中；两者都未发生时 skill 进入空闲倒计时；
- 隐式触发扩展点：`detect_implicit_skill_invocation_for_command`（`invocation_utils.rs:31`）已能识别「运行 `<skill>/scripts/` 下脚本 / 读取 SKILL.md」的 shell 命令，当前仅用于 analytics，可扩展为隐式触发与 TTL 刷新信号（设计决策，不在本变更强制范围内）。

### 12. 路径解析：执行用真实 `/dev/shm` 地址，LLM 只见原地址

模型必须能执行 skill 脚本、读取附属资源，但 `/dev/shm` 真实地址不得进入 LLM 可见内容。三层处理：

- **内容层（重水合）**：重水合内容保持 skill 的相对引用与原目录地址（必要时注入「Base directory: <原目录>」锚点）；`/dev/shm` 地址**绝不写入 LLM 请求内容**；
- **工具层（PreToolUse）**：执行前把命令/文件参数中指向已知 skill 原目录（或包内相对路径）的引用改写为对应 `fm_skill_security_<hex>/` 真实地址（映射表 `原目录 → 解密目录`，对应 opencode `rewriteSkillPaths` 的方向，但落在工具边界而非内容边界）；
- **输出层（after-hook）**：工具输出中的 `/dev/shm/fm-agent-security` 路径串替换为 `[REDACTED]`（对应 opencode `redaction.ts` 的 `SHM_RE`），防止脚本自身打印路径导致模型得知。

探测防护（ls/cat/grep 拦截）与持久化保密（rollout/context/审计不含路径）由访问控制与路径隐藏需求覆盖。

**权衡**：模型不知道真实地址 → 无法构造/越权访问解密路径；代价是每次执行依赖 PreToolUse 改写，且映射表必须随注册表维护（TTL 卸载时同步删除）。

### 13. 实施形态：独立 crate + 薄接线，插件仅承担外围

评估了三种实施形态，结论：

- **纯运行时插件（不改源码）不可行**：插件能提供 skill 包、MCP server 与 hooks（`PluginManifest.paths.hooks`，TOML 声明 PreToolUse/PostToolUse/PermissionRequest），但存在硬缺口——① 请求构建点**没有扩展钩子**（无 `messages.transform` 等价物），重水合必须改 `get_formatted_input_for_request`；② PostToolUse 只能 `should_block`/`additional_contexts`，**不能改写工具输出**，路径脱敏无法做成 hook；③ 插件 hook 是外部命令（jq/python 脚本），注册表/TTL/幂等只能落文件，宿主内存态与保洁无法保证；④ token 化注入点在 `build_skill_injections`，核心内部。
- **独立 crate（推荐）**：新增 workspace crate（如 `codex-encrypted-skills`）承载全部逻辑——SDK 封装、token 序列化、会话注册表与两级 TTL、重水合纯函数、路径改写、输出脱敏、审计——全部可单测；核心只留三个薄接线点：`build_skill_injections`（解密/注入）、`get_formatted_input_for_request`（重水合）、hook 注册（拦截/改写）。符合仓库「resist adding to codex-core、新功能优先新 crate」的指引。
- **混合形态**：拦截规则可通过配置/插件 hooks.toml 声明（PreToolUse 由 crate 提供的 helper 命令或脚本执行），加密 skill 包按 plugin skills 分发；核心改动仍限于上述三个接线点。

结论：本变更按「独立 crate + 三个薄接线点」实施；插件形态覆盖 skill 分发与 hook 声明，不承担加密核心逻辑。

### 14. 3.2 接线决策（实现期确认）

- **runtime 归属**：`EncryptedSkillRuntime`（注册表 + 内容缓存 + SDK + memRoot）挂在 `SessionServices.encrypted_skills_runtime`，随 Session 生命周期；
- **SDK 默认值**：`UnavailableSdk` fail-closed——未配置数字信封 SDK 时加密 skill 加载产出 warning，绝不回退读 stub；
- **memRoot 初始化**：`init_mem_root_once` 进程级 OnceLock 守卫（清残留 + 0700），避免多会话互相清理；
- **接线签名**：`build_skill_injections(mentioned_skills, loaded_skills, encrypted_skills: Option<&EncryptedSkillRuntime>, session_id, otel, analytics_client, tracking)`；session_id 取自 `sess.thread_id`；
- **幂等**：`load_or_register` 在 Skill TTL 内命中注册表直接复用 token（不重复解密），解密失败 → warning + 指标 error。

### 15. 5.2/6.x/7.x/8.x 接线状态（实现期确认）

- **重水合**：`Prompt.encrypted_skills: Option<EncryptedSkillRehydrator>` 覆盖常规 turn 与 compaction 三条路径；
- **访问控制**：`encrypted_skills_guard` 在工具分发前置（registry.rs PreToolUse 前）执行——shell/exec 读取与搜索命令指向解密存储时 Blocked、执行型命令改写原目录→解密目录、read/grep/glob 拦截、write/edit/apply_patch/网络工具参数含已知明文时 Blocked；`RedactingToolOutput` 对工具输出做 `/dev/shm` 路径脱敏；
- **fork 隔离**：`keep_forked_rollout_item` 丢弃含哨兵 token 的用户消息；
- **TTL 扫描**：turn 边界调用 `runtime.sweep()`（Skill/Thread 两级）；`clear_thread` 已实现并单测，线程删除路径接线待 app-server 线程生命周期接入；
- **TTL 配置**：当前使用 `TtlConfig::default()`（skill 10min / thread 30min），config.toml 暴露待后续接入。

### 16. 审计与线程清理接线（2026-07-31 修复轮）

- **审计落地**：`AuditSink` trait + `FileAuditSink`（JSONL、10MB 轮转）；`EncryptedSkillRuntime` 在解密（含 `cache_hit`）、token 化、重水合（token 计数）、清理（sweep/thread_end）、`record_blocked` 时发射事件；Session 构建时注入 `$TMPDIR/fm_skill_security_audit.log` sink；guard 拦截统一调用 `record_blocked`；
- **线程结束清理**：`CodexThread::clear_encrypted_skills()` 公开方法，`thread/delete` 处理器对子树每个线程调用；
- **内容上限补全**：`ContentCache` 增加每会话总大小上限（8MiB，FIFO 淘汰），满足 spec「entry count + total size」双上限。

### 17. TTL 配置暴露与审计断言（2026-07-31 第三轮）

- **配置**：`ConfigToml` 新增 `[encrypted_skills]`（`sdk` / `skill_idle_ttl_secs` / `thread_idle_ttl_secs`），`Config` 解析为 `EncryptedSkillsSdkToml` + `TtlConfig`，Session 构建 runtime 时使用；config schema 已重生成；
- **沙箱澄清**：bwrap 的 `--ro-bind` 会让解密目录**可读**（有害），解密目录必须以 writable root（`--bind`）挂载供脚本执行；读拦截依赖 guard + 权限层（已更新分析报告）；
- **审计集成断言**：`core/suite` 测试断言审计文件包含 `decryption`/`blocked` 事件。

### 18. 多进程 mem root 隔离（2026-07-31 第四轮）

- 解密目录改为 `/dev/shm/fm-agent-security/p<pid>/fm_skill_security_<hex>`（进程命名空间）；
- `init_mem_root_once` 清理策略：`p<pid>` 命名空间按 pid 存活（Linux `/proc/<pid>`）删除死亡进程残留；旧式扁平 `fm_skill_security_*` 目录按废弃清理；本进程命名空间始终保留；
- 非 Linux 平台保守不清除跨进程目录（残留由 TTL 与人工清理兜底）；
- 多进程并发不再互相清理活动解密目录。

### 19. view_image 覆盖与 Skill 级清理审计（2026-07-31 第五轮）

- `ViewImageHandler` 暴露 `pre_tool_use_payload`（`{ "path" }`，hook 名 `view_image`），guard 对 `view_image` 读取解密目录 Blocked；`guard_read` 兼容 `filePath`/`path` 两种参数键；
- `sweep` 对 skill 级 TTL 卸载也发射 `Cleanup` 审计事件（`reason: "skill_ttl_sweep"`），补全审计完整性；
- Windows 目标本地不可用（仅 Linux toolchain），平台编译检查留待 CI/后续。

### 20. 平台可移植与自定义 mem root（2026-07-31 第六轮）

- `resolve_default_mem_root()`：Linux 用 `/dev/shm/fm-agent-security`，非 Linux 回退 `temp_dir()/fm-agent-security`；host 构建与初始化统一使用该解析结果；
- 脱敏与拦截改为按运行时实际 mem root：`redact_path_prefix` 参数化（`runtime.redact` 覆盖自定义 root + 默认 root 双重）；guard 将运行时 root 纳入引用检测（`ls <root>` 探测拦截）；
- 搜索命令分类补充 `ls`（目录探测拦截）；
- 新增测试：自定义 root 脱敏、`ls <root>` 拦截、`resolve_default_mem_root` 解析。

### 21. guard 迁移到 codex 工具体系（2026-07-31 第七轮）

- `before_tool_with_runtime` 的工具匹配从 opencode 风格字符串（`read`/`grep`/`glob`/`write`/`edit`/`webfetch`）迁移为 **codex 真实工具**的 `HookToolName` 类型化匹配：`HookToolName::bash()`（`shell_command` + `unified_exec`）、`HookToolName::view_image()`（新增构造器）、`HookToolName::apply_patch()`；
- 删除 codex 中不存在工具的匹配分支（`read`/`grep`/`glob`/`write`/`edit`/`webfetch`/`web_search`）与对应单测；`guard_read` 只服务 `view_image`（`path` 键）；
- 新增 `mcp__*` 工具放行测试；web 导出面（扩展 `web/run`，无 pre payload）依赖沙箱网络隔离；
- 集成测试（apply_patch 导出拦截、shell 读取拦截）验证迁移后行为不变。
- **全量核对**：codex 所有 handler 的 hook 名与 guard 归属已盘点（见分析报告 §5）——`shell_command`/`exec_command` 共用 `Bash`（exec 参数键 `cmd` 由 pre payload 归一化为 `command`，新增 exec 集成用例验证）、`apply_patch`、`view_image`、`mcp__*` 放行、其余 Function 工具走默认 hook 名放行（无宿主文件读取通道）、扩展工具无 pre payload 靠网络隔离。

## Risks / Trade-offs

- `/dev/shm` 空间不足 → 解密失败 → 限制 skill 总大小并实现清理策略，失败时给出可读 warning。
- 重水合增加每请求开销 → `/dev/shm` 为内存文件系统，读取 <1ms，可忽略。
- Token 指向已清理的 `/dev/shm` 内容 → 重水合检测失效并提示重新提及，重新解密后恢复。
- 脚本执行暴露路径线索 → 路径不出现在 context/rollout；PreToolUse 拦截直接读取。
- 既有 rollout 含明文 → 保持明文项可读，仅在新增内容采用 Token；恢复路径兼容两种格式。
- Compaction 重水合后 input 变大 → 符合预期（compaction 本就处理大输入）；摘要后历史只含自然语言摘要。
- 明文 skill 兼容性 → `is_encrypted: false` 完全走原路径，无行为变化。
- 重水合后的明文可能被模型复述或写入文件 → 信任层级框架 + 出站导出拦截（write/edit/apply_patch/网络工具参数检测）+ 未来输入端 safe guard 模型。
- SKILL.md 自身夹带提示词注入 → tier-2 数据处理（视为流程数据而非指令）+ 输出策略兜底。
- 审计日志误记敏感信息 → 只记录事件类型、session、token、时间戳，明文内容一律不落日志。
- Skill TTL 过短 → 高频提及的 skill 反复解密 → TTL 可配置，幂等命中不产生重复解密；默认值以实际 turn 节奏校准。
- Thread 长时间空闲但无事件触发 TTL 扫描 → 明文滞留 → 在 turn 边界、线程事件与惰性重水合检查三处都做 TTL 判定，任一路径命中即卸载。
- Token 指向已卸载的明文 → 重水合失效检测，提示重新提及后重新解密，不阻塞会话。

## 测试策略（TDD）

本变更采用 TDD（红-绿-重构）作为默认开发流程，tasks.md 按「先单测、后实现」组织：

- **单元测试优先**：每个功能（frontmatter 解析、SDK 解密、token 序列化、重水合纯函数、路径改写、TTL 注册表、导出拦截、fork 过滤）都有独立的 `*_tests.rs`（`#[path]` 引入，遵循仓库规范），用 mock SDK 与可注入时钟隔离环境依赖，避免操作真实 `/dev/shm`；
- **集成测试兜底**：agent 行为变更按仓库规范（AGENTS.md）必须补 `core/suite` 集成测试（`test_codex`），覆盖端到端行为（提及→token→重水合→执行→清理）；集成测试验证的是行为契约，单测验证的是逻辑正确性，二者不互相替代；
- **断言风格**：优先整对象 `assert_eq!`（pretty_assertions），避免逐字段断言；不为静态值写测试。

## Migration Plan

1. 元数据层：skill 元数据增加加密标记与 token 字段；未标记 skill 走原路径。
2. 按能力分阶段落地：先 Token 化注入 + 重水合（主链路），再 PreToolUse 拦截，最后 fork 隔离与生命周期清理。
3. rollout 读写兼容：新格式只含 Token；旧格式明文照常解析。
4. 回滚：关闭加密标记即回到明文路径，无数据迁移成本。

## Open Questions

- 数字信封 SDK 的具体选型与接口约定（元数据查询、解密回调、token 生成）。
- 拦截范围是否覆盖 shell/exec 之外的读取工具（如文件浏览/编辑类 tool），以及是否需要对 `apply_patch` 类工具做同源保护。
- Token 格式是否需要版本号前缀，以便未来演进重水合协议。
- `skill_idle_ttl` 与 `thread_idle_ttl` 的默认值与可配置入口（配置项还是环境变量），以及 Skill TTL 是否应绑定 turn 数而非墙钟时间。
