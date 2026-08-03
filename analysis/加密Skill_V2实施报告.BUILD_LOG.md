# BUILD_LOG · 加密 Skill V2 实施报告（v16）

## 数据故事

- 事实源：`FM_AGENT_SECURITY_IMPLEMENTATION_REPORT_V2.md`（§1–§19）为主，OpenSpec change、测试结果与核心代码文件为辅；所有关键数字与报告一致（95/25/10/2146、600s/1800s、64 条/8MiB、4MiB、10MB）。
- 支撑轴：主 Agent 方案与 V1 子代理隔离的差异 → 回复明文硬脱敏补齐；两级 TTL 满足常驻部署；fail-closed SDK 保障部署期。
- 冲突口径：V2 设计设想的 SQLite 缓存/Ukey 与本项目取舍并列呈现（§1.4），未取单一叙事。
- 删除规则：未核实的性能数字与无日期宣称未入渲染层。

## 部署口径（v2 更新，2026-08-01）

- **单容器部署 CodeX**：每用户一容器、非 root、持久化常驻；对外仅暴露 HTTP / WebSocket（app-server 网关）给前端。
- **无 SSH**：用户没有 shell 远程连接机会；/dev/shm、skill 包、审计日志对用户不可见。
- 前端口径（v4）：用户唯一入口为 <b>AHP Client</b>，经 <b>AHP 协议（HTTP/WebSocket）</b>与 CodeX 交互；不再出现 OpenChamber/汇报 PPT 等表述。

## 内容扩充（v2）

- §1 方案设计：六项决策逐项展开（识别/注入/重水合/TTL/guard/脱敏），每项含实现要点与边界路径；新增「决策展开」小节与 guard 工具矩阵表。
- §2 总体架构：新增明文生命周期表、存储布局与安全基元（Zip-Slip、secure wipe、启动清理、容量）。
- §3 实现情况：模块表补职责列；测试矩阵扩展（config/app-server-protocol/Windows 交叉编译）；配置与 schema 小节。
- §4 关键流程：新增每步输入→输出→失败路径表、两级 TTL 状态机、特殊路径（compaction/resume/fork/进程重启）、审计事件表。
- §5 部署与安全：单容器拓扑、9 项部署清单、6 项残余风险。
- K 锚点扩展至 K1–K9，全部可回溯。

## v3 更新（2026-08-01）

- **字体**：正文切换为衬线体（Songti SC / Noto Serif CJK SC / Source Han Serif SC + Georgia），mono 代码保持等宽。
- **时序图**：全部改用 Mermaid sequenceDiagram，CDN 引入（`cdn.jsdelivr.net/npm/mermaid@11`，用户明确要求；离线时图表不渲染，其余内容不受影响）。
- **多轮会话场景分 Tab（§5，7 类）**：① 触发明文 Skill；② 触发加密 Skill（首次）；③ 反复触发同一加密 Skill（TTL 内幂等）；④ TTL 过期后重新触发；⑤ 触发其他加密 Skill（同线程并存）；⑥ 跨会话/多线程隔离；⑦ 回复引用明文的脱敏。每个 Tab = 执行要点 + Mermaid 时序图。
- Tab 懒渲染：首次激活才 `mermaid.run`（避免隐藏容器零宽渲染）；修复一处选择器 bug（`[data-rendered!='1']` 非法 → `:not([data-rendered='1'])`，QA 渲染测试发现）。
- 章节重排：多轮场景为 §5，部署与安全顺延为 §6；nav/chips/摘要同步。

## v4 更新（2026-08-01）

- **前端口径**：删除 PPT/OpenChamber 相关表述；部署拓扑表改为「AHP Client（前端）经 AHP 协议（HTTP/WebSocket）访问 CodeX」；Sources K3 替换为 AHP 协议说明；正文部署形态句同步。
- **进程异常分析（§4.6）**：明确 TTL 是进程内活动驱动的惰性清理——进程挂起时 sweep 不触发、明文驻留超时；进程被杀时内存状态消失但 /dev/shm 残留，由下次启动的 pid 存活清理与容器销毁兜底；部署清单新增 healthcheck + 自动重启项，残余风险表新增对应行。
- **视觉优化**：hero 渐变 + 底部 accent 条、章节号徽章化、h2 装饰下划线、h3 左侧色条、表格圆角/表头着色/数字强调、卡片与 flow 步骤 hover 上浮、callout 图标前缀、tab 激活顶边高亮、回到顶部按钮、滚动条美化、打印样式（打印时展开全部 Tab）。

## v5 更新（2026-08-01）

- **强调体系升级**：关键信息不再只靠加粗——新增 `.hl`（accent 底色高亮）、`.hl.warn`（琥珀底）、`.hl.good`（olive 底）与 `.num`（大号等宽数字胶囊）：
  - 摘要 4 条核心结论 → 整句 accent 高亮；
  - 关键数字表 10 个数值 → 大号等宽数字胶囊（95/95、600s/1800s、64 条/8MiB 等）；
  - 12 张决策/修复卡标题与 8 个流程步骤 → accent 高亮；
  - 架构关键路径词（主 Agent 方案、单容器部署 CodeX、AHP Client、AHP 协议、无 SSH）与风险词（进程内活动驱动的惰性清理、进程挂起、进程被杀、超过 TTL 驻留、fail-closed）→ 语义色高亮；
  - 列表项标题统一 accent 着色，表格内加粗保留深色（`li b` / `td b` 规则）。
- 全部替换后 `<b>` 由 96 处降至 52 处（保留给次级强调）。

## v6 更新（2026-08-01）

- **风格换新（DeepSeek 蓝白系）**：主色由陶土色换为 DeepSeek 蓝（#4D6BFE）；背景 #F7F8FA、卡片白、表头浅蓝、成功/警示/危险三色语义；hero 浅蓝渐变 + 蓝色装饰条；Mermaid 主题同步蓝色；正文保留衬线体。
- **4.1 卡片布局重做**：原 flex 换行 + 绝对定位箭头（换行后箭头错位）改为 **grid 卡片**（自适应列宽、无箭头、hover 上浮、hot 步骤浅蓝渐变），修复样式崩坏。
- **章节结构重组（§1–§7）**：
  - §1 背景与部署前提（1.1 V1→主 Agent 演进与威胁模型；1.2 部署形态：单容器 + AHP；1.3 设计目标）
  - §2 方案设计（2.1 六项核心决策；2.2 决策展开；2.3 V2 差异；2.4 设计取舍）
  - §3 总体架构（3.1 架构图；3.2 组件职责；3.3 明文生命周期；3.4 存储布局）
  - §4 核心流程（4.1 主流程；4.2 步骤细节与失败路径；4.3 TTL 状态机；4.4 特殊路径；4.5 审计事件；4.6 进程异常）
  - §5 多轮会话场景（7 Tabs，不变）
  - §6 实现与验证（原 §3 顺延：模块/加固/配置/文档）
  - §7 部署与运维（原 §6 顺延：AHP 拓扑/配置/清单/残余风险）
  - nav、chips、h3 编号、正文章节引用全部同步重排。
- **footer 删除「生成方式」**：仅保留 CDN 渲染说明、部署口径与离线提示。

## v7 更新（2026-08-01）

- **多轮时序图改造（§5）**：所有「再次触发」场景从<b>第一轮触发</b>开始画——
  - ① 明文 Skill：第一轮注入原文 → 第二轮再次读取/注入原文（上下文保留至压缩）；
  - ② 加密首次：标注「此前未触发，注册表为空」；
  - ③ 反复触发：第一轮解密 → Token A；第二轮命中复用（SDK 零调用），上下文始终只有 Token A；
  - ④ TTL 过期：第一轮解密 → Token A → 空闲 600s wipe → 第二轮重新解密 → 新 Token B（旧 Token A stale 保留不替换）；
  - ⑤ 其他 Skill：第一轮 Skill A → Token A；第二轮 Skill B → 独立 Token B，上下文 A+B 并存。
- **新增 §4.7 Skill 从上下文卸载的策略**（原版 / 明文 / 加密三行对比表）：
  - 原版：原文注入长期驻留上下文至压缩，无 TTL/Token；
  - 明文：同原版，零行为影响；
  - 加密：上下文只留 Token（不因 TTL 移除），明文按两级 TTL 从存储层卸载；过期 Token 变 stale，重新提及恢复。
- 关键区分 callout：卸载发生在存储层，Token 在上下文长期保留。

## v8 更新（2026-08-01 · 残存问题修复轮）

### 实现修复（全部 TDD，分笔提交）

- **P0 周期 TTL sweep**（`6f27cee9fe`）：新增 `encrypted_skills_periodic`——常驻进程后台每 30s 执行 sweep，覆盖「进程活着但无请求」空闲窗口；2 个 paused-time 单测（空闲卸载 / 新鲜保留）；进程挂起仍由部署侧 healthcheck/restart 兜底（§4.6 同步更新）。
- **P1 MCP/扩展路径探测拦截**（`b3689de01f`）：guard_export 默认分支增加路径引用检查——非 shell 工具参数引用 mem root/解密目录即 Blocked（audit reason `storage_probe`）；guard 单测 25 → 28（原 `mcp_tools_pass_through` 改为无存储引用的放行用例）。
- **P2a 引号片段脱敏**（`50d0cecdb1`）：`redact_quoted_fragments` 覆盖「long sensitive line」这类中段引用（引号包裹、≥12 字符、保留引号）；crate 单测 95 → 99。
- **P2b 离线版报告**（`d25890a5a0` + 本轮再生成）：`加密Skill_V2实施报告.offline.html` 内嵌 Mermaid（base64 + eval，剥离 `"use strict";` 规避间接 eval 全局 var 不挂载问题；`type="text/plain"` 防误执行），无需联网 7/7 渲染。
- **测试补强**：进程被杀残留清理已有等价单测（`init_mem_root_removes_dead_process_namespaces_and_keeps_live` / `removes_stale_decrypted_dirs_only`），在 §6.2 引用。

### 报告更新

- **恢复 §0 摘要与关键数字**（v6 重构时误删，本轮从历史版本恢复并更新数字：crate 99/99、guard 28/28、周期 sweep 30s、core lib 2152 过）；
- §2 决策展开 ④ 增加周期兜底条目；⑤ guard 矩阵更新 storage_probe；
- §2.4 设计取舍 callout 更新（路径探测已拦截、短片段部分缓解）；
- §4.6 进程异常更新为「三处触发 + 部署侧兜底」；
- §6.2 加固清单 +3 卡（引号片段、路径探测、周期 sweep）；
- §7.4 残余风险表状态更新（MCP 路径探测=已修复；短片段=部分缓解；进程挂起=已实现+部署兜底）；
- footer 增加离线版指引。

### 验证

- check_report（在线 --allow-external / 离线全项）全过；
- 在线与离线双版本 Mermaid 渲染 7/7、0 console error；
- core lib 全量 2152 过（2 个环境项：沙箱代理变量、token 估算，单独重跑全绿；/tmp/.git 杂散目录已移开归因）；
- encrypted-skills 99/99、guard 28/28、core-skills 131/131、集成 10/10（上一轮基线）。

## v9 更新（2026-08-02 · 文本/脚本分级访问策略）

### 需求（用户确认）

- 允许 `ls`/`find` 列目录与文件名；`grep` 允许；所有解密后的文本内容（md/txt 等）可临时给 agent 看到；
- 只有**脚本内容**禁止看到（参考 OpenCode 插件从结果摘除脚本类结果）；
- 之前 glob/grep 是独立工具、无需从 bash 解析限制——Codex 中统一在 guard 做命令级分级。

### 实现（`34ec4b0d0b` + `1426ea67cc`）

- `paths.rs`：新增 `file_targets`（跳过 flag/重定向）、`is_script_file`、`command_targets_script`、`is_recursive_search`、`inject_script_exclusions`（grep/rg 追加 `--exclude='*.sh'` 等）；paths 单测 15 → 17。
- `guard_shell` 重写：脚本执行仍放行（改写）；读取/搜索命令按文件类型分级——脚本目标 Blocked（reason `script_source`）、文本放行；递归 grep/rg 命中解密目录注入脚本排除；ls/find 放行；原路径绕过修复保留（脚本场景）。
- `runtime.unrewrite_paths`：工具输出中解密目录路径改写回原目录路径（agent 可复用原路径执行，/dev/shm 不暴露）；未知 mem root 子路径仍 [REDACTED] 兜底。
- **持久化层脱敏**（`1426ea67cc`）：`redact_tool_output_plaintext_for_persistence` 对 FunctionCallOutput/CustomToolCallOutput 文本在 rollout 与 API/WS 流落盘前替换已知明文为 [REDACTED]；**内存历史保留原文**（模型多轮可见，符合“临时给 agent 看到”）。新 E2E `text_read_of_skill_md_is_allowed_but_rollout_stays_clean`：cat SKILL.md 放行执行，rollout 无明文。
- guard 单测 28 → 36；集成 10 → 11；core lib 全量 2160 过（2 个环境项）。

### 报告更新

- §0 数字（guard 36/36、集成 11/11、core lib 2160）；
- §2.2 ⑤ guard 矩阵（Bash 分级策略、输出反改写 + 落盘脱敏）；
- §2.4 取舍 callout（文本临时可见、rollout/API 流零明文、脚本源码不可见）；
- §4.2 步骤表 ⑤、§6.2 加固卡（文本/脚本分级访问）、§7.4 残余风险（工具输出明文=已收口）；
- 在线/离线双版本同步，check_report 全过。

## v10 更新（2026-08-02 · 目标类型模型，不再依赖命令名）

### 需求

- 用户指出 fd/rg/tree/lsd/eza 等现代命令不在命令名名单里——命令名黑名单模式无法覆盖新命令。

### 实现（`89334af690`）

- **guard 模型升级**：从「命令名黑名单（READ/SEARCH_COMMANDS）」改为「**脚本文件目标 + 执行白名单**」：
  - 只有解释器执行语义（runner 后的第一个非 flag 参数为脚本文件，如 `bash run.sh`）放行；
  - 任何非执行语义的命令，只要参数引用解密目录内的脚本文件（cat/bat/cp/tar/归档/编码等，无论命令名）→ Blocked（script_source）；
  - 列目录（ls/lsd/eza/tree/fd）与文本读取（cat/bat/head）自动放行，无需逐个登记；
  - `bash -c 'cat run.sh'` 等包装不再被当作“执行”放行（`is_script_execution` 只认 runner 后第一个非 flag 参数），且文件目标解析支持去引号。
- paths 单测 17（含引号执行、bash -c 判定）；guard 单测 36 → **41**（cp/tar 复制、bash -c 包装、lsd/eza/tree/fd 列名、bat 文本/脚本）；crate 103/103；集成 11/11；core lib 全量 **2164 过**（3 个并行偶发/环境项单独重跑全绿）。

### 剩余边界（已记录）

- `fd -x cat`、`xargs cat`、变量拼接、命令替换等运行时生成的路径仍在字符串层之外——治本靠 bwrap/私有挂载或“脚本 stdin 执行”架构（见报告 §7.4 与部署清单）。

### 报告更新

- §0 数字（guard 41/41、crate 103/103、core lib 2164）；
- §2.2 ⑤ guard 矩阵改“目标类型 + 执行白名单”描述（含 lsd/eza/tree/fd）；
- §6.2 加固卡同步；在线/离线双版本 check_report 全过。

## v11 更新（2026-08-02 · npm 安装包 + 插件体系/源码修改说明）

- **npm 安装包**：`cargo build --release -p codex-cli --bin codex`（10m34s）→ `codex-rs/target/release/codex`（strip 后 397MB）；按官方布局打包 → `dist/npm/openai-codex-0.0.0-dev.tgz`（130MB，dist/ gitignore 不入库）；`npm install` 后 `.bin/codex --version` = `codex-cli 0.0.0` 已验证。说明：glibc 二进制置于 musl 名义目录，正式发布需官方多平台流程。
- **§6 重构（详略得当、突出重点）**：新增 6.1 实现状态一览（能力/质量/部署/交付四行重点表）；模块单测数字按实际更新（token 7 / sdk 3 / mem_root 10 / cache 7 / registry 8 / rehydrate 9 / paths 16 / export_guard 19 / audit 6 / runtime 18 = 103）；6.6 npm 安装包；6.7 Codex 插件体系与源码修改说明（Extensions/Hooks/core-plugins 三层、四点插件无法实现的原因、独立 crate+薄接线策略、按 crate 的修改面清单）。
- **Markdown 实现报告**追加 §20（npm 安装包）与 §21（插件体系与源码修改说明）。
- 在线/离线双版本 check_report 全过。

## v12 更新（2026-08-03 · 章节结构调整）

- **修复顶栏重复**：nav 中 §0 摘要链接出现两次（一个带 class="on"、一个普通），已删除重复项。
- **插件体系与源码修改独立成章（§3）**：从原 §6.7 提取，插入方案设计（§2）与总体架构（原 §3）之间，单独成章节：
  - 3.1 Codex 插件体系（Extensions / Hooks / core-plugins 三层与边界）；
  - 3.2 为什么插件无法实现需求（四个原因）；
  - 3.3 为什么修改源码（核心边界非扩展点、独立 crate + 薄接线）；
  - 3.4 修改了哪些源码（按 crate 修改面清单表）。
- **章节顺延**：总体架构 §3→§4、核心流程 §4→§5、多轮场景 §5→§6、实现与验证 §6→§7、部署与运维 §7→§8；h3 编号、nav、chips、正文引用（多轮场景 §5→§6）全部同步。
- 在线/离线双版本 check_report 全过。

## v13 更新（2026-08-03 · 存储文件与格式一览）

- **方案设计新增 2.5 存储文件与格式一览**（按实际代码核实）：
  - /dev/shm：`p<pid>/fm_skill_security_<hex>/` 目录树（0700），解密包全部条目，TTL 窗口内明文；
  - rollout：`<codex_home>/sessions/<yyyy/mm/dd>/<thread_id>.jsonl`（JSONL），只存 Token 与 [REDACTED]；
  - SQLite（state_db）：sqlite_home（`CODEX_SQLITE_HOME` 可覆盖），记忆/token 用量/会话索引等，无 Skill 明文；
  - 审计日志：默认 `<temp>/fm_skill_security_audit.log`（可配置 audit_path），JSONL 10MB 轮转；
  - 配置、日志/遥测（**注意**：改写后工具命令可能含解密路径，敏感字段脱敏未实现）、skill 包原目录（密文）、内存 cache/registry。
- Markdown 实现报告同步追加 §22 存储一览表。
- 在线/离线双版本 check_report 全过。

## v14 更新（2026-08-03 · 实施报告重命名 V2）

- Markdown 实施报告重命名为 `analysis/FM_AGENT_SECURITY_IMPLEMENTATION_REPORT_V2.md`（git mv），标题同步为「V2 实现成果与安全报告」；
- HTML 在线/离线报告与 BUILD_LOG 中的引用路径全部同步更新；
- 在线/离线双版本 check_report 全过。

## 主题原子

「加密信封 → 请求瞬间可见 → TTL 即消失」：封面用 SKILL 巨型背景字，架构图以明文路径（loader→runtime→/dev/shm→rehydrate→LLM）为唯一高亮链，其余链路全部 Token/脱敏灰化。

## 图表选型

- 总体架构：手写 SVG 组件图（9 组件 + 10 条流向，明文链 hot 高亮）。
- 关键数字/TTL/配置/审计：表格 + 语义 tag。
- 流程：§4 八步 flow 条 + 步骤细节表 + TTL 状态机。
- 钻取：幂等去重、跨会话门禁两处 `<details>` drill。

## 降级与取舍

- 未引 d3/CDN/外部字体，单文件自包含，file:// 可开。
- 图表为静态 SVG（无 dashboard 右栏、无逐行 drill 动效），符合「快速汇报」档位。
- reduced-motion：CSS 全局禁用平滑/过渡；chips 用 `behavior:"auto"`。

## QA 门禁（全过）

- `check_report.py --allow-external`（CDN 为本次明确要求）：内联 JS 语法过（node --check）/ K 锚点 9 条无断链 / HTML 标签平衡。
- Mermaid 渲染测试：临时副本把 CDN 换成本地 mermaid.min.js，逐个激活全部 7 个 Tab——7/7 时序图渲染成功（`aria-roledescription="sequence"` ×7，全部 `data-rendered`），0 console error。
- v5 强调体系回归：7/7 时序图仍正常；check_report 三项全过（JS/K 锚点/HTML 平衡）。
- v6 回归：结构重排后 check_report 三项全过（修复 §1 组装时一处重复 `</div>`）；7/7 时序图渲染正常；1680/1280 双宽度截图正常。
- v7 回归：5 个多轮时序图重写后 7/7 全部渲染成功（含更长 diagram），0 console error；check_report 三项全过。
- chromium headless 双宽度（1680/1280）：页面渲染正常，stderr 仅环境 dbus 噪音，无页面 console/pageerror。
- 横向溢出：宽表与 pre 均有 `overflow-x:auto`，nav 可横向滚动；SVG 等比缩放。
- 残留表述检查：grep PPT/OpenChamber/NB2602/Hermes = 0（报告中已无这些来源名）。

## 残留风险

- 未做 Playwright 全页慢滚与标签零重叠逐模块截图目检（环境无 playwright 模块）；后续轮次可补。
- 架构图文字密度较高，窄视口（&lt;1100px）下 SVG 文字按比例缩小，可读性需真机确认。
- Mermaid CDN 依赖外网：受限网络（--network none）环境下打开页面图表不渲染；如需离线可用需内嵌 mermaid.min.js（与用户当前「CDN 引入」要求相反，按需求保留 CDN）。
- 本 Skill 属于个人技能目录（~/.codex/skills/html-report），不是 feat/agent-security 分支的实现产物，分支内不跟踪。

## v15 更新（2026-08-03 · HTML 报告中文重命名）

- HTML 报告重命名为中文并明确 V2：`analysis/加密Skill_V2实施报告.html`（在线）、`加密Skill_V2实施报告.offline.html`（离线）、`加密Skill_V2实施报告.BUILD_LOG.md`；
- `<title>`、hero kicker、导航 brand 同步为「V2 实施报告」；在线版 footer 的离线版文件名引用同步；
- 在线/离线双版本 check_report 全过。

## v16 更新（2026-08-03 · CDN 换 npmmirror）

- 在线版 Mermaid CDN 由 jsdelivr 换为淘宝镜像：`https://registry.npmmirror.com/mermaid/11.16.0/files/dist/mermaid.min.js`（npmmirror 文件路径格式为 `<pkg>/<version>/files/<path>`；已实测 200 且与 jsdelivr 内容逐字节一致）；
- footer 文案同步标注「npmmirror CDN」；
- 离线版用 npmmirror 源重新生成（内嵌 base64，内容一致）；
- 在线/离线双版本 check_report 全过。
