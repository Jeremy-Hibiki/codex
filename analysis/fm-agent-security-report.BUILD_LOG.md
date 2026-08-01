# BUILD_LOG · fm-agent-security 加密 Skill 单页汇报（v2）

## 数据故事
- 事实源：`FM_AGENT_SECURITY_IMPLEMENTATION_REPORT.md`（§1–§19）为主，OpenSpec change、测试结果与核心代码文件为辅；所有关键数字与报告一致（95/25/10/2146、600s/1800s、64 条/8MiB、4MiB、10MB）。
- 支撑轴：主 Agent 方案与 V1 子代理隔离的差异 → 回复明文硬脱敏补齐；两级 TTL 满足常驻部署；fail-closed SDK 保障部署期。
- 冲突口径：汇报 V2 的 SQLite 缓存/Ukey 设想与本项目取舍并列呈现（§1.4），未取单一叙事。
- 删除规则：未核实的性能数字与无日期宣称未入渲染层。

## 部署口径（v2 更新，2026-08-01）
- **单容器部署 CodeX**：每用户一容器、非 root、持久化常驻；对外仅暴露 HTTP / WebSocket（app-server 网关）给前端。
- **无 SSH**：用户没有 shell 远程连接机会；/dev/shm、skill 包、审计日志对用户不可见。
- 原「OpenCode + OpenChamber 双容器」为陈旧方案，本报告 §5 已按新口径重写（拓扑表、配置、验证清单同步）。

## 内容扩充（v2）
- §1 方案设计：六项决策逐项展开（识别/注入/重水合/TTL/guard/脱敏），每项含实现要点与边界路径；新增「决策展开」小节与 guard 工具矩阵表。
- §2 总体架构：新增明文生命周期表、存储布局与安全基元（Zip-Slip、secure wipe、启动清理、容量）。
- §3 实现情况：模块表补职责列；测试矩阵扩展（config/app-server-protocol/Windows 交叉编译）；配置与 schema 小节。
- §4 关键流程：新增每步输入→输出→失败路径表、两级 TTL 状态机、特殊路径（compaction/resume/fork/进程重启）、审计事件表。
- §5 部署与安全：单容器拓扑、9 项部署清单、6 项残余风险。
- K 锚点扩展至 K1–K9，全部可回溯。

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
- `check_report.py`：外部引用 0 / 内联 JS 语法过（node --check）/ K 锚点 9 条无断链 / HTML 标签平衡。
- chromium headless 双宽度（1680/1280）：页面渲染正常，stderr 仅环境 dbus 噪音，无页面 console/pageerror。
- 横向溢出：宽表与 pre 均有 `overflow-x:auto`，nav 可横向滚动；SVG 等比缩放。

## 残留风险
- 未做 Playwright 全页慢滚与标签零重叠逐模块截图目检（环境无 playwright 模块）；后续轮次可补。
- 架构图文字密度较高，窄视口（&lt;1100px）下 SVG 文字按比例缩小，可读性需真机确认。
- 本 Skill 属于个人技能目录（~/.codex/skills/html-report），不是 feat/agent-security 分支的实现产物，分支内不跟踪。
