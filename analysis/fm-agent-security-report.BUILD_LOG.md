# BUILD_LOG · fm-agent-security 加密 Skill 单页汇报（v6）

## 数据故事
- 事实源：`FM_AGENT_SECURITY_IMPLEMENTATION_REPORT.md`（§1–§19）为主，OpenSpec change、测试结果与核心代码文件为辅；所有关键数字与报告一致（95/25/10/2146、600s/1800s、64 条/8MiB、4MiB、10MB）。
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
- chromium headless 双宽度（1680/1280）：页面渲染正常，stderr 仅环境 dbus 噪音，无页面 console/pageerror。
- 横向溢出：宽表与 pre 均有 `overflow-x:auto`，nav 可横向滚动；SVG 等比缩放。
- 残留表述检查：grep PPT/OpenChamber/NB2602/Hermes = 0（报告中已无这些来源名）。

## 残留风险
- 未做 Playwright 全页慢滚与标签零重叠逐模块截图目检（环境无 playwright 模块）；后续轮次可补。
- 架构图文字密度较高，窄视口（&lt;1100px）下 SVG 文字按比例缩小，可读性需真机确认。
- Mermaid CDN 依赖外网：受限网络（--network none）环境下打开页面图表不渲染；如需离线可用需内嵌 mermaid.min.js（与用户当前「CDN 引入」要求相反，按需求保留 CDN）。
- 本 Skill 属于个人技能目录（~/.codex/skills/html-report），不是 feat/agent-security 分支的实现产物，分支内不跟踪。
