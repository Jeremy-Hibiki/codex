# BUILD_LOG · Agent 输入侧注入/越狱检测拦截 · 交互式研究站

## 数据故事
- 事实源：input_guard_report.agent.final.md（11 章），全部文案忠实转写；关键数字与原文一致。
- 支撑轴：分层管线（规则 <1ms → 小分类器 20–100ms → judge 秒级，95%+ 流量前两层终结）× 检测天花板（自适应攻击 >90% 绕过）× 中文空白（Qwen3Guard 唯一 SOTA）。
- 冲突口径（Lakera 92.55/95.22、CaMeL 67/77%、NeMo 成熟度、PG2 延迟与拦截率）全部并列呈现，未取单一叙事。
- 删掉的数据：无日期的次要宣称未进渲染层；Knostic/RAPIDS 微调数字保留并标注论文口径。

## 主题原子
「安检闸门 / 分层过滤管线」。封面四态同一物理对象：A 闸门阵列无限递归（3×3 自相似 + 整数倍变焦无缝循环）；B 单闸门四层爆炸图（L2 judge 舱 / L1 转闸 / L0 归一化格栅 / 架构底座，真实材质 + 层间软影）；C 同几何蓝图线框；D B 引擎 + 开箱时间线（crate 震裂 → 层回弹弹出 → 归位爆炸态）。

## 图表选型（CHARTS.md）
- §0 证据对象（P17）：闸门侧视图，6 个硬数字挂语义部位（读数屏=PINT、闸翼=AgentDojo、传送带=延迟分层、队列=95%、闸下漏网卡=emoji 100%、裂缝=>90%）。
- 威胁映射 / 开源模型 / 非模型技术 / 推荐管线 / 冲突口径：P5 矩阵热力表（DOM + 逐行 drill）。
- PINT 榜：横向条形（排名读取任务）+ 利益冲突语义标注 + 92.55 旧口径虚线。
- 低 FPR：配对条 + 实用阈值叙事（0–12% vs 71.45%）。
- AgentDojo：条形 + 自适应口径并列脚注。
- 工程落地：签名图「级联闸道」——对数延迟轴 + 流带宽度=流量份额 + 闸机立于实测延迟带。
- 前沿趋势：P7 赔率板（四档主观概率 + 双点区间 + UNDER WAY）。
- 路线图：横向时间带（4 周）。
- 右栏：P14 情境面板（canvas，随 10 个 data-win 章节切换，相位条 + 4 读数 + 迷你闸机 + 对数延迟尺，全部可钻取）。

## 降级与取舍
- 未用 d3/topojson（无地图需求），全部手写 SVG/Canvas；bundle.py 相应无 vendor 内联。
- Cover D 为简化开箱（无木屑粒子），保留震屏/抛盖/回弹/归位核心节拍。
- K 表 32 条（报告引用中取被实际引用者，无日期来源按分级规则处理）。

## QA 门禁（全过）
- node --check：js/ 9 文件全过。
- Playwright 双宽度 1680/1280 全页慢滚：0 pageerror、0 console error、横向溢出 0、document.fonts.check('16px et-book')=true（多文件 http:// 与单文件 file:// 均通过）。
- reduced-motion：0 pageerror；封面 A 静态帧已绘制（像素抽查非空白）；所有 IO 入场在 reduced 下直出。
- 标签零重叠：逐模块截图目检 3 轮，修复 9 处碰撞（pipeline 顶注/副标、evidence 红字、pint 括注、lowfpr 左标截断与右值溢出、roadmap ASCII 断词等）。
- 钻取抽查 5/5：PINT 榜首、AgentDojo、低 FPR、管线 L1 闸、路线快路径 → drill card 正常，回溯来源齐全。
- 单文件版：/mnt/agents/output/input_guard_site_single.html（315 KB），file:// 双宽度 0 错误 0 溢出。

## 已知残留风险
- Cover A 递归深度受 LOD 阈值限制（2 层可见），视觉上是"阵列+递归中心"而非无限纵深。
- 右栏 dashboard 在 <1100px 视口由 CSS 隐藏（theme 约定）。
- 图表内中文换行为定长切片（已避免 ASCII 词中断）。

---

# 第二轮 · 补充篇增量整合（2026-07）

## 目标与事实源
- 事实源：input_guard_supplement.agent.final.md（gpt-oss-safeguard/ShieldGemma、AgentDoG 勘误、I/O 六形态、多轮防护、512 上下文墙），全部文案忠实转写。

## 关键决策
- 新增 §8–§12 五章，插入 §7 前沿之后；选型路线图改 §13、附录改 §14；main.js 导航标签、dashboard WINS（新增 gptoss/agentdog/iospec/multiturn/ctxwall 五面板）、封面 chips 同步。
- 新签名图：「512 上下文墙阶梯图」（CHARTS P13 gate ladder 变体）——对数 token 轴上每个模型的"闸门视野条"，红色带=Agent 典型输入带（4K–200K），红色斜纹=各模型盲区；编码上限（对数位置）×形态（颜色）×盲区（斜纹段）三变量，全条可钻取。
- 数据层新增 14 组数据集（gptoss/shieldgemma/policy_compare/agentdog/new_benches/industry_moves/io_forms/io_specs×18/multiturn_methods/multiturn_matrix/ctx_limits/ctx_solutions/ctx_routing/five_layers）；K 表扩至 K47（K33–K47 取自补充篇引用清单）。
- §0 摘要与关键数字表、§3 oss-matrix（+gpt-oss/AgentDoG 两行）、§7 前沿、§13 场景表（+高权限审计行）、冲突口径表（+gpt-oss 口径）同步更新。
- 勘误按补充篇口径执行：主报告"AgentDoG 1.5 查无实据"作废，图表与正文均按"真实存在、轨迹级标杆"呈现并标注勘误。

## 文件改动
- index.html（+5 章节、编号、§0/§3/§7/§13 更新、chips）、js/data.js、js/sources.js、js/charts-supplement.js（新增）、js/main.js、js/dashboard.js、css/style.css（#io-table fixed 布局防溢出）。

## QA 门禁（全过）
- node --check：js/ 12 文件全过。
- Playwright 1680/1280 慢滚（http:// 多文件 + file:// 单文件）：0 pageerror、0 console error、横向溢出 0、fonts.check('16px et-book')=true；reduced-motion 0 错误。
- 标签零重叠：新章节截图目检，修复 2 处截断（ctx 图 qualifire 左标、底部注记右溢出）与 1 处 io-table 1280 溢出 3px（table-layout fixed + 允许换行）。
- 钻取抽查 4/4（新）：ctx 阶梯行（512 token→[K43]）、AgentDoG ClawSafety 56.25%→18.75%（[K36]）、io-table Lakera 行、policy-table 检测目标行 → drill card 正常、来源可回溯。
- 单文件版：/mnt/agents/output/input_guard_site_single.html（363.6 KB），file:// 双宽度 0 错误 0 溢出。

## 残留风险
- io-table 为 fixed 布局，窄视口下长英文 token 以 word-break 换行，观感可接受。
- AgentDoG/gpt-oss 成绩均为论文/官方口径 + 单第三方榜，置信度中，已在文案标注。

---

# 第三轮 · 封面收敛为 B 爆炸图 + Sources 分组重构（2026-07）

## 修改项 1 · 首页只保留 Cover B（爆炸图）
- 移除 A（闸门阵列递归）/ C（蓝图线框）/ D（开箱）三态：删除 js/cover.js、js/cover-wire.js（含四态切换器、localStorage/URL 参数记忆、COVER_MODE 全局）；index.html 删除 #cover-canvas / #cover-canvas-w 与 #cover-mode 切换按钮组；css 同步收敛 canvas 选择器并删除 .cover-mode 样式。
- cover-exploded.js 清理 D 态死代码（crate 开箱状态机、INTRO、shakeAmp、uK/yawExtra、backOut、dblclick 重播、playIntro）；封面加载即激活唯一形态：k 0→1 组装→爆炸入场动画（reduced-motion 直出完成态 k=1 静态帧）。
- 保留 B 态全部交互：右侧四层论点标注（L2 judge 舱 / L1 转闸 / L0 格栅 / 底座 HITL）、点击空白处组装/拆解切换、caption 状态行。

## 修改项 2 · Sources 章节按来源类型分组
- js/sources.js 重构：47 条 K 锚点新增 grp 字段，按固定五组渲染（Ⅰ 学术论文 29 条置顶 · Ⅱ 厂商官方文档与博客 4 · Ⅲ 模型卡与代码仓库 7 · Ⅳ 第三方评测与独立分析 5 · Ⅴ 其他 2）；分组小标题 + K 编号连续不变（正文钻取卡 [K#] 引用零破坏）。
- 学术论文条目全部改 IEEE 著录（cite 字段）：作者, "标题," arXiv preprint arXiv:编号, 年份（会议论文写 in Proc. NeurIPS/ACL）。作者可核实者写全（Hackett/Jacob/Palit&Woods/Hines/Russinovich/Wei 等，经论文引用页核实）；调研文件仅给出姓氏者写「姓 et al.」；完全无法核实者（K8/K21/K23/K36/K42/K43/K44）以标题起首著录。
- 非论文条目统一「来源名. 标题. URL, 日期.」格式；新增「全部 K1–K47 / 仅学术论文（29 条）」筛选。
- arXiv 编号核对：27/27 条与 /mnt/agents/output/research/ 调研文件引用清单一致（脚本自动比对，含 K2 2504.11168、K3 2510.09023、K13 2404.01833、K36 2605.29801、K43 2605.23196 等）。

## QA 门禁（全过）
- node --check：js/ 10 文件全过（cover.js、cover-wire.js 已删除）。
- Playwright 1680/1280 慢滚（http:// 多文件 + file:// 单文件各一轮）：0 pageerror、0 console error、横向溢出 0、fonts.check('16px et-book')=true；reduced-motion 0 错误、封面 B 静态帧已绘制（像素抽查 10,409 非空白采样点）。
- 封面残留检查：整站 grep cover-mode / cover-canvas-w / COVER_A / COVER_W = 0。
- 钻取抽查 6 处：PINT 榜首 [K1]、AgentDojo PG2 [K4][K30]、低 FPR 8B [K5]、管线 L1 闸 [K4][K20]、路线快路径、512 墙阶梯行 [K43][K44][K45] → drill card 正常、K 编号回溯无断链。
- 「仅论文」筛选：29 行可见、仅 Ⅰ 组标题保留。
- 单文件版：/mnt/agents/output/input_guard_site_single.html（353.8 KB），file:// 双宽度 0 错误 0 溢出。

## 残留风险
- 管线图 L2 judge 舱 g 元素 bbox 几何中心点击不落 drill（与第二轮行为一致，charts-pipeline.js 未改，非本轮回归；L0/L1/流带/SaaS 点击正常）。
- 6 条未来日期论文作者无法从公开渠道核实，按规则以标题起首著录，置信度标注见各条目。

---

# 第四轮 · 最终版报告对齐（§0 重写 + 学术脉络/训练数据两章 + Sources K48–K67）（2026-07）

## 目标与事实源
- 事实源切换为 input_guard_final.agent.final.md（18 章唯一最终版）：开头自含概括（问题定义→六类 I/O 形态方案地图→三条核心结论），新增第 15 章（学术脉络·引用网络）与第 16 章（训练数据与微调）。

## 关键决策
- §0/首页去框架化：封面 lede 与 §0 摘要按最终版第 1 章重写——问题定义（直接/间接注入）→ 六类 I/O 形态地图 → 三条核心结论；删除"同事推荐三工具"叙事（§2 章号改"三大候选方案详解"，正文保留 Lakera/NeMo/Rebuff 对比内容，与报告 17.1 一致）。关键数字表 +3 行：Qwen3Guard 119 万条/中文 26.6%、AgentDoG 1.5 ~1k 净化样本、Crescendo 98–100%。
- 新增 §13 学术脉络（谱系树 10 组核心论文表 + 13 篇净增量论文表，含两篇"打脸作"高亮）与 §14 训练数据与微调（九模型明细表 + 自建指南/LoRA 成本表）；插入 §12 512 墙之后、§15 选型路线图之前。附录章号由 §9 修正为 §16（前三轮遗留错位）。
- 新签名图「训练数据规模 × 中文占比」（CHARTS 对数规模条形变体）：横条=训练条数（对数轴，1K→1M 三个数量级），颜色=中文占比（红=26.6% 中文 / 铜=中英混合 / 蓝灰=仅英语），未披露数据量的 PG1/2 与 LG3 不入图并在副标声明；全条可钻取。
- 数据层新增 4 组数据集（lineage×10、new_papers×13、train_data×9、finetune×8）；K 表 K48–K67 新增 20 条（论文 17 条 IEEE 著录 + Aegis 2.0 数据集 / ProtectAI v2 模型卡 / Tomoro 分析 3 条非论文），arXiv 编号与 sup6/sup7 调研文件一致；五分组与筛选器沿用，"全部 K1–K67"。
- main.js 导航标签、dashboard WINS（+lineage/traindata 两面板，roadmap→§15、appendix→§16、登记来源 K1–K67）、封面 chips（+学术脉络/训练数据）、footer 事实源同步。

## 文件改动
- index.html（封面 lede/chips、§0 重写、关键数字表 +3 行、+§13/§14 两章、§15/§16 重编号、footer）、js/data.js、js/charts-supplement.js（+train-chart 签名图与 4 张表）、js/sources.js（+K48–K67）、js/main.js、js/dashboard.js。

## QA 门禁（全过）
- node --check：js/ 10 文件全过；K 引用完整性脚本核验：67 条无重复、正文/图表 [K#] 引用 0 断链。
- Playwright 1680/1280 慢滚（http:// 多文件 + file:// 单文件各一轮）：0 pageerror、0 console error、横向溢出 0（docOverflow=0、无溢出元素）、fonts.check('16px et-book')=true；reduced-motion 双协议 0 错误。
- 钻取抽查 8/8（新章 6 处）：train-chart Qwen3Guard 119 万条 [K6]、AgentDoG ~1k [K36]、InjecGuard MOF [K63]、谱系行 Qwen3Guard←GuardReasoner、DataSentinel 行 [K48]、LoRA $1–2 行 → drill card 正常；旧图回归 PINT 榜首 [K1] 正常；Sources「仅论文」筛选 46 行可见。
- 新章节截图目检：train-chart 标签/图例无重叠，dashboard §13/§14 面板渲染正常。
- 单文件版：/mnt/agents/output/input_guard_site_single.html（380.1 KB），file:// 双宽度 0 错误 0 溢出。

## 残留风险
- 多数 2026 年新论文作者无法从公开渠道核实，按既有规则以标题起首 IEEE 著录（K48–K60、K63–K64 等）。
- 顶部 era-rail 17 段标签在 1280 宽度下较拥挤（沿用既有压缩行为，未重叠遮挡内容）。

---

# 第五轮 · 19 章最终版对齐（论文精读章 + 措辞清理 + Sources K68–K82）（2026-07）

## 目标与事实源
- 事实源：input_guard_final.agent.final.md（19 章最终版）——新增第 16 章"论文精读选编"（35 篇，攻击 11 / 检测 12 / 防御架构与评测 12）。

## 关键决策
- **措辞清理（修订叙事 → 中性表述）**：§8 章名"新增方案：gpt-oss-safeguard 与 ShieldGemma"→"gpt-oss-safeguard 与 ShieldGemma"；§0 关键数字表"（补充篇）/§新增方案"→"§政策推理"；§9 h2"勘误与补档"→"轨迹级诊断护栏：从概念走向 SOTA"（dek 同步去"表述作废"叙事）；"2025Q4–2026.7 新增量精选（已剔除主报告覆盖项）"→"最新进展精选"；charts-supplement/data/dashboard/sources 中全部"补充篇 §x.y"src 注记改写为 19 章版章节号（报告 §10–§14），训练数据章 src 由"§16.x"更正为"§17.x"；dashboard"勘误"读数→"三维细粒度诊断 55.2%"；sources K35 fact 尾部勘误注记删除。grep「补充篇/新增方案/勘误/修订」渲染层=0。学术脉络章"新增论文 13 篇"为报告 §15.3 原文措辞，属内容本身，保留。
- **新增 §14 论文精读章**（插在 §13 学术脉络之后）：报告第 16 章 35 篇逐篇精读，三组 tab（Ⅰ 攻击 11 / Ⅱ 检测 12 / Ⅲ 防御架构与评测 12）切换；每篇条目=mono kicker + serif 标题 + 信息行（作者/机构 · 会议 · 日期 · 可点击 arXiv 链接 · [K#]）+ 150–220 字摘要（脚本校验 35/35 全部在区间内）+ 蓝色 mono"关键数字"钻取钮（ASR、F1、233→0 等，钻取卡含数值含义与 K 锚点回溯）。编辑级排版：细线分隔、无卡片背景。数据进 js/data.js（RPT.papers，35 条，K 引用 35 个去重后全部命中 SRC）。
- **Sources 增补 K68–K82（15 条，全部 grp=paper、IEEE 著录、arXiv 编号与报告一致）**：GCG 2307.15043 / PAIR 2310.08419 / TAP 2312.02119 / AutoDAN-Turbo 2410.05295 / PLeak（CCS 2024，无 arXiv，按报告给 chatpaper 链接）/ GuardReasoner 2501.18492 / PromptArmor 2507.15219 / GenTel-Safe 2409.19521 / StruQ 2402.06363 / SecAlign 2410.05451 / Instruction Hierarchy 2404.13208 / FIDES 2505.23643 / MELON 2502.05174 / Token Highlighter 2412.18171 / RTC-Bench 2505.21936。其余 20 篇复用既有 K（K3/K5/K6/K11–K16/K22/K26/K35/K36/K48–K51/K53/K56/K57/K61/K63），钻取直接指向原条目，未重复建条。筛选标签"全部 K1–K82"，仅论文 61 条。
- **19 章结构同步**：训练数据 §14→§15、选型 §15→§16、附录 §16→§17（index.html sec-no/注释、main.js era-rail 标签、dashboard WINS no 字段与新 papers 面板）；封面 chips +"论文精读 35 篇"；§0 关键数字表 +35 篇行；footer 事实源"（19 章最终版）"、Sources 标题 K1–K82。

## 文件改动
- index.html（+§14 章、三章重编号、措辞清理、chips/§0 表/footer）、js/data.js（+papers 35 条、措辞清理）、js/charts-supplement.js（+papers 渲染器、src 注记改 19 章章节号）、js/sources.js（+K68–K82、筛选标签、K35 清理）、js/main.js（rail 标签）、js/dashboard.js（+papers 面板、重编号、K1–K82）、css/style.css（+.papers-tabs/.paper-item 编辑级样式）。

## QA 门禁（全过）
- node --check：js/ 10 文件全过；K 引用完整性：SRC 82 条无重复、papers 35 条 K 引用 0 断链、内联 [K#] 0 断链。
- Playwright 1680/1280 慢滚（http:// 多文件 + file:// 单文件各一轮）：0 pageerror、0 console error、横向溢出 0（docOverflow=0、无溢出元素）、fonts.check('16px et-book')=true；reduced-motion 0 错误，论文精读条目在 reduced 下直出。
- 钻取抽查 8/8：论文精读 4 处（攻击组 Jailbroken [K12] / AutoDAN-Turbo 6.72 次 [K71] / 检测组 PromptShield 71.45% [K5] / 防御组 CaMeL 233→0 [K16]）+ 防御组条数=12 + 回归 train-chart Qwen3Guard 119 万 [K6]、PINT 榜首 [K1] → drill card 正常、来源可回溯。
- 新章节截图目检：tab 切换正常、条目排版无重叠无溢出、dashboard §14 面板渲染正常。
- 单文件版：/mnt/agents/output/input_guard_site_single.html（408.8 KB），file:// 双宽度 0 错误 0 溢出。

## 残留风险
- era-rail 现有 19 段标签在 1280 宽度下更拥挤（沿用既有压缩行为，未遮挡内容）。
- 论文精读摘要为忠实转写的浓缩版（150–220 字/篇），个别条目为保证区间砍去了次级数字，完整数字见钻取卡与 Sources 条目。

---

# 第六轮 · SingGuard 双子收录（报告 §11.6 / sup8）（2026-07-24）

## 目标与事实源
- 事实源：research/input_guard_sup8_singuard.md（完整调研）+ input_guard_final.agent.final.md §11.6（措辞口径以此为准），保留"厂商自建基准、暂无第三方复测"口径提醒于正文、图表副标与钻取卡三处。

## 关键决策
- **§9 新增 SingGuard 小节**：正文两段（SingGuard 政策自适应多模态护栏 Qwen3-VL 2B/4B/8B · 35 数据集平均 F1 第一 / SingGuard-NSFA Qwen3.5-Base 0.8–9B · 7 域 28 风险 185 变体 · 133 语言 · CoT 审计 + 判别头 45–57ms · 官方口径 F1>94% · 插件增强 Llama Guard 3 +17.6 F1 · 2026-07-13 开源 Apache-2.0）；新签名图 singuard-chart（Llama Guard 3 加装 NSFA 分类头三基准 before→after 配对条形，编码 基准×前后 F1 双变量，可钻取）；训练数据未公开披露 → 不入 §15 规模图并在正文说明。
- **对比表补位**：oss_models +2 行（SingGuard / NSFA，注明自建基准口径）；io_specs +2 行（SingGuard=形态 D 政策推理；NSFA=形态 A/C 之间，无状态单轮进多标签出）；io_forms D 卡代表 +SingGuard；io-table 标题 18→20 方案。
- **§0 关键数字表 +1 行**（45–57ms / F1>94%，标注自建基准口径）；dashboard §9 面板换两条 SingGuard 读数（45–57ms、35 数据集 F1 第一），iospec 方案数 18→20，附录登记来源 K1–K86。
- **Sources +K83–K86**：K83 SingGuard arXiv:2606.22873、K84 NSFA arXiv:2607.13081（均 IEEE 著录、grp=paper）；K85 inclusionAI 开源仓库（grp=model，Apache-2.0/双平台/GGUF/基准集公开）；K86 Help Net Security 第三方报道（grp=third，注明仍系转述官方数据非独立复测）。筛选标签改动态计算（K1–K86）。

## 文件改动
- index.html（§0 表 +1 行、§9 dek + SingGuard 小节两段 + #singuard-chart 容器、Sources 标题 K1–K86）、js/data.js（+RPT.singuard、oss_models +2、io_specs +2、io_forms D 卡）、js/charts-supplement.js（+singuard-chart 渲染器、io-table 计数与高亮）、js/sources.js（+K83–K86、筛选标签动态化）、js/dashboard.js（§9 面板、计数）、BUILD_LOG.md。

## QA 门禁（全过）
- node --check：js/ 10 文件全过。
- Playwright 1680/1280 慢滚（http:// 多文件 + file:// 单文件各一轮）：0 pageerror、0 console error、横向溢出 0（docOverflow=0、无溢出元素）、fonts.check('16px et-book')=true；io-table 20 行、singuard-chart 渲染、Sources 86 条。
- 钻取抽查 5/5：singuard-chart Query 67.66→85.23（+17.6）[K84]、Response 83.61→92.44 [K84]、io-table SingGuard 形态 D 行、回归 agentdog-chart ClawSafety [K36]、train-chart Qwen3Guard 119 万 [K6] → drill card 全部正常、来源可回溯。
- 截图目检：singuard-chart 标签/轴/口径提醒无重叠无溢出；dashboard §9 新读数渲染正常。
- 单文件版：/mnt/agents/output/input_guard_site_single.html（396.6 KB），file:// 双宽度 0 错误 0 溢出；rsync 同步 input_guard_site/。

## 残留风险
- SingGuard/NSFA 全部成绩为厂商自建基准官方口径（K86 亦系转述），第三方复测出现后需更新数字与口径注记。
- era-rail 标签在 1280 宽度下沿用既有压缩行为（未遮挡内容）。
