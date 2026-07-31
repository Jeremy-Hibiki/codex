// Data layer — compiled from input_guard_report.agent.final.md (唯一事实源)
// window.RPT: snake_case keys; 每个数字均可回溯报告章节
window.RPT = {

  // §1.4 / §7.1 PINT 官方榜（Lakera 自建基准, balanced accuracy, 测试日期）
  pint_board: [
    { name: "Lakera Guard",              score: 95.22, date: "2025-05-02", kind: "商业 API", note: "自家基准榜首，利益冲突需打折", self: true },
    { name: "AWS Bedrock Guardrails",    score: 89.24, date: "2025-05-02", kind: "云服务",   note: "prompt attack 过滤器三档强度" },
    { name: "Azure AI Prompt Shield",    score: 89.12, date: "2025-05-02", kind: "云服务",   note: "官方训练含中文，8 语言" },
    { name: "ProtectAI DeBERTa-v3 v2",   score: 79.14, date: "2025-05-02", kind: "开源 184M", note: "TPR@0.1%FPR≈0，低误报死区" },
    { name: "Llama Prompt Guard 2 (86M)",score: 78.76, date: "2025-05-05", kind: "开源 86M",  note: "AgentDojo 拦截率最强小模型" },
    { name: "Google Model Armor",        score: 70.07, date: "2025-08-27", kind: "云服务",   note: "独立评测 acc 0.779 / FPR 0.225" },
    { name: "Aporia Guardrails",         score: 66.44, date: "2025-09-19", kind: "商业 API", note: "Multi-SLM 引擎，平均延迟 0.34s" },
  ],
  pint_conflict: { old: 92.55, now: 95.22, note: "CourtGuard 论文引旧版 92.55%（7.54% 误分类），现版 95.22%；基准/模型迭代的时间性差异" },

  // §8.2 延迟分层（毫秒；三个数量级）
  latency_layers: [
    { layer: "L0", name: "规则/归一化（正则·NFKC·去零宽·Base64 解码）", lo: 0.1, hi: 1,     label: "<1ms",      share: 95, note: "Aho-Corasick <0.1–0.2ms；纯字符串处理" },
    { layer: "L1", name: "小分类器（PromptGuard2 22M/86M · Qwen3Guard-0.6B）", lo: 19, hi: 100, label: "20–100ms", share: 4,  note: "22M 19ms / 86M 20–92ms（A100/H100 FP8）；95%+ 流量终结于前两层" },
    { layer: "L2", name: "LLM judge（NeMo self-check · 8B 复核）",  lo: 1200, hi: 1500, label: "1.2–1.5s",  share: 1,  note: "每方向一次额外 LLM 调用；仅 1–5% 灰区流量升级" },
    { layer: "SaaS", name: "云托管（Azure Prompt Shields）",        lo: 200, hi: 500,  label: "200–500ms", share: null, note: "同步调用；Lakera 宣称 P95 ~50ms" },
  ],

  // §5.1 开源模型对比
  oss_models: [
    { name: "ProtectAI DeBERTa-v3 v2", params: "184M",       license: "Apache-2.0", zh: "✗ 仅英文",     pint: 79.14, agentdojo: 22.2, tpr_low: "≈0",    lat: "CPU ~50ms",      note: "HF 下载量最大；自报 99.99% 为分布内虚高" },
    { name: "Llama Prompt Guard 2 86M", params: "86M",       license: "Llama 社区许可", zh: "△ 多语言可",  pint: 78.76, agentdojo: 81.2, tpr_low: "0–12%", lat: "A100 92ms / H100 FP8 p50 20ms", note: "英文 AUC 0.998；静态基准最强小模型" },
    { name: "Llama Prompt Guard 2 22M", params: "22M",       license: "Llama 社区许可", zh: "△ 掉档明显",  pint: null,  agentdojo: 78.4, tpr_low: "0–12%", lat: "A100 19.3ms",    note: "多语言 AUC 0.942，轻量首选" },
    { name: "deepset deberta-v3-base-injection", params: "184M", license: "MIT",   zh: "✗",              pint: null,  agentdojo: 13.5, tpr_low: "—",     lat: "CPU 级",          note: "546 条样本微调；模型卡官方背书'收自家数据重训'" },
    { name: "Qwen3Guard Gen/Stream", params: "0.6B/4B/8B",   license: "Apache-2.0", zh: "✓ 最强档",     pint: null,  agentdojo: null, tpr_low: "—",     lat: "89–253ms (A10G)", note: "119 语言，中英文 SOTA；0.6B/4B/8B 准确率 82%/91%/96%" },
    { name: "gpt-oss-safeguard 20b", params: "21B MoE", license: "Apache-2.0", zh: "△ 第三方 F1≈80", pint: null, agentdojo: null, tpr_low: "—", lat: "秒级（CoT）", note: "政策推理护栏：慢路径深检定位；ToxicChat F1 79.9% 开源榜首；官方自认注入基准弱于基座 [K33][K37]" },
    { name: "AgentDoG 1.5", params: "0.8B–8B", license: "开源（Qwen3.5 系）", zh: "△ 未专项评", pint: null, agentdojo: null, tpr_low: "—", lat: "TTFT 亚秒", note: "轨迹审计形态：Pre-Reply 护栏 ClawSafety ASR 56.25%→18.75%；R-Judge 92.2% [K36]" },
    { name: "SingGuard", params: "2B/4B/8B", license: "Apache-2.0", zh: "✓ 多模态多语言", pint: null, agentdojo: null, tpr_low: "—", lat: "快/慢双模式", note: "政策自适应多模态护栏：35 数据集平均 F1 第一（厂商自建基准口径，暂无第三方复测）[K83]" },
    { name: "SingGuard-NSFA", params: "0.8B/2B/4B/9B", license: "Apache-2.0", zh: "✓ 133 语言", pint: null, agentdojo: null, tpr_low: "—", lat: "判别头 45–57ms", note: "Agent 行为护栏：7 域 185 变体含注入/越狱域；官方口径 F1>94%；分类头可插拔增强 Llama Guard 3 +17.6 F1（自建基准口径）[K84]" },
  ],

  // §5.1 PromptShield 低 FPR 数据 (arXiv:2501.15145)
  promptshield_lowfpr: {
    small: { lo: 0, hi: 12, label: "DeBERTa 级小模型 TPR@0.1%FPR 0–12%" },
    large: { v: 71.45, label: "8B 级检测器 TPR@0.1%FPR 71.45%（AUC 0.997）" },
    extra: "ProtectAI v2 TPR@1%FPR 仅 1.7%、@0.1%FPR 为 0 —— 实用的低误报阈值恰是 encoder 小模型死区",
  },

  // §5.1/§8.3 AgentDojo 真实攻击拦截率（%）
  agentdojo: [
    { name: "Prompt Guard 2 86M", v: 81.2, note: "Meta 口径；3% 效用损失" },
    { name: "Prompt Guard 2 22M", v: 78.4, note: "轻量版掉档有限" },
    { name: "ProtectAI DeBERTa-v3 v2", v: 22.2, note: "真实攻击下大幅失分" },
    { name: "deepset deberta-v3", v: 13.5, note: "训练样本仅 546 条" },
  ],
  agentdojo_adaptive: "独立 SoK 口径：自适应多轮攻击下各 guardrail ASR 仍 >90% —— 静态基准与自适应攻击两个口径必须同时呈现",

  // §2.5 攻击手法 → 检测手段映射
  attack_map: [
    { atk: "DAN / 角色扮演 / 虚构包装", det: "注入/越狱专用分类器（Prompt Guard 2 等）+ 语义 LLM 判定", basis: "角色扮演是攻击数据集最高频成分（331 次）", cls: "model" },
    { atk: "深度嵌套（DeepInception 类）", det: "语义级 LLM 检测 + 输出侧有害内容分类，双层缺一不可", basis: "诱导内容可绕过 LlamaGuard / OpenAI API", cls: "model" },
    { atk: "Prefix injection / 拒绝抑制", det: "输出侧监控（首 token 前缀模式、拒绝率漂移）+ 输入句式规则", basis: "利用浅层对齐；输出侧比输入侧可靠；单项成功率 31.1%", cls: "output" },
    { atk: "Crescendo 多轮渐进", det: "会话级检测（话题升级/漂移监控）；单轮检测基本无效", basis: "单轮'完全人类可读且良性'；GPT-4 二元成功率 98%", cls: "session" },
    { atk: "Many-shot 长上下文", det: "结构异常检测（大量伪造 Q-A 对）、限制上下文长度", basis: "结构特征明确，适合规则+统计", cls: "rule" },
    { atk: "编码混淆（Base64/ROT13/hex）", det: "检测前解码归一化管道（识别→解码→再分类）", basis: "编码原文过分类器必漏（Wei et al.）", cls: "norm" },
    { atk: "低资源语言 / 中文拼音谐音", det: "翻译归一后检测 + 多语言分类器 + 拼音/谐音规范化", basis: "中英文安全边界不对称已被实证", cls: "norm" },
    { atk: "Emoji / Unicode / 零宽字符", det: "Unicode 规范化（NFKC、去零宽/tag 字符、emoji 剥离）前置", basis: "6 个商用护栏 100% 被绕过", cls: "norm" },
    { atk: "Token smuggling / payload splitting", det: "拼接/赋值句式识别 + '执行拼接结果'指令的语义检测", basis: "单片段无害，需整体语义判定", cls: "model" },
    { atk: "ASCII art（ArtPrompt）", det: "ASCII art 检测 + 视觉/空间解码预处理", basis: "绕过困惑度过滤、改写防御与重分词", cls: "norm" },
    { atk: "系统提示提取", det: "输入侧句型规则+分类器；输出侧相似度比对（canary）；零机密原则", basis: "输入句型长尾；泄漏点在输出侧", cls: "output" },
    { atk: "间接注入（RAG/工具/网页）", det: "检索/工具内容过注入检测 + Spotlighting 标记 + 最小权限", basis: "文本检测只降风险，结构隔离为必需", cls: "arch" },
  ],

  // §6.9 非模型技术对比
  nonmodel_tech: [
    { tech: "关键词/正则/YARA", when: "推理时", eff: "recall 0.25–0.33", cost: "极低", bypass: "同义词、leet、编码、多语言" },
    { tech: "Perplexity 过滤", when: "推理时", eff: "仅对 GCG 乱码有效；流畅攻击仅降 4–12pp", cost: "低", bypass: "AutoDAN、Base64、低资源语言、ASCII art" },
    { tech: "Paraphrase 复述", when: "推理时", eff: "ASR 降 22–28pp（净化类最强）", cost: "高（一次 LLM 调用）", bypass: "语义保留型注入、ArtPrompt" },
    { tech: "Retokenization", when: "推理时", eff: "ASR 降 7–15pp", cost: "极低", bypass: "流畅攻击" },
    { tech: "预处理归一化", when: "推理时", eff: "不单独拦截；缺它下游全失效（emoji 混淆 100% 绕过）", cost: "极低（<1ms）", bypass: "—（是前提而非防线）" },
    { tech: "Spotlighting（datamarking/encoding）", when: "推理时", eff: "间接注入 ASR >50%→<2%（GPT 系列）", cost: "低", bypass: "自适应攻击（LLMail-Inject 37 万提交中大量成功）" },
    { tech: "StruQ（训练时）", when: "训练时", eff: "无优化攻击 ~0% ASR；GCG 56%", cost: "微调+前端", bypass: "优化攻击" },
    { tech: "SecAlign（训练时）", when: "训练时", eff: "无优化 0% ASR；优化攻击主要 <10%", cost: "偏好微调", bypass: "多轮/外泄类攻击未覆盖" },
    { tech: "Instruction Hierarchy", when: "训练时（厂商）", eff: "直接注入降 ~30%", cost: "由厂商承担", bypass: "角色伪造、RAG 提权" },
    { tech: "CaMeL（推理时系统层）", when: "推理时系统层", eff: "AgentDojo 攻击成功 233→0；utility −7pp", cost: "高", bypass: "侧信道" },
    { tech: "金丝雀 token", when: "推理时", eff: "默认检测率 0%；加显式处理指令后部分有效", cost: "极低", bypass: "清洗 canary 后再泄露" },
    { tech: "零机密 / Rule of Two / 出口白名单", when: "设计时", eff: "非概率性硬约束", cost: "架构成本", bypass: "—（削减影响面而非注入本身）" },
  ],

  // §10.2 推荐分层管线
  pipeline: [
    { layer: "L0", comp: "网关快筛：正则 · Unicode NFKC 归一化 · 去零宽/Bidi · Base64/编码识别解码 · 长度结构异常", lat: "<1ms", duty: "先归一化，否则下游全失效", share: "大部分流量" },
    { layer: "L1", comp: "预筛分类器：英文/多语 Prompt Guard 2 86M（<1GB 显存，vLLM 共池）；中文必选 Qwen3Guard-Stream-0.6B", lat: "20–100ms", duty: "95%+ 流量在此终结", share: "95%+" },
    { layer: "L2", comp: "升级复核：Qwen3Guard-Gen-8B / LlamaGuard 8B 级 judge（INT4 ~4GB），仅灰区或高危会话", lat: "秒级", duty: "低 FPR 下的 TPR 保障；1–5% 流量", share: "1–5%" },
    { layer: "L3", comp: "输出侧：输出与系统提示相似度比对 + 改进版 canary（显式处理指令）", lat: "<10ms", duty: "防 Skill/系统提示泄露兜底", share: "全量输出" },
  ],

  // §9.1 三条技术路线（odds board）
  routes: [
    { name: "小型专用检测器（快路径）", grade: 4, range: [3.4, 4], evidence: "PromptGuard 2 / Qwen3Guard / YuFeng-XGuard：毫秒级、可共池、覆盖多语；工程现实最强，但只解决已知模式", underway: true, boundary: "已知模式之外必然漏检" },
    { name: "推理型护栏（语义/政策层）", grade: 3, range: [2.4, 3.2], evidence: "GuardAgent LPA >98% / ShieldAgent 召回 90.1% / AlignmentCheck 召回 >80%·FPR <4%；代价是延迟与成本，本质仍是概率防御", underway: true, boundary: "概率防御，无法对抗自适应攻击者" },
    { name: "架构级免疫（设计层）", grade: 4, range: [3.0, 3.8], evidence: "CaMeL 是 *The Attacker Moves Second* 中唯一部分幸存者；ProGent 把 AgentDojo 间接注入 ASR 39.9%→1.0%；共识度上升最快", underway: false, boundary: "需编写安全策略，utility 损失真实存在" },
  ],

  // §10.4 四周落地路线图
  roadmap: [
    { week: "第 1 周", title: "log-only 影子部署", body: "网关 L0 归一化 + L1（Qwen3Guard-0.6B）上线只记录不拦截；建中文攻击/良性 held-out 集（含 Skill 套取句式变体、拼音/谐音样本）" },
    { week: "第 2 周", title: "阈值校准", body: "在 held-out + 影子流量上扫 ROC，按 FPR 业务成本换算选阈值，而非默认 0.5" },
    { week: "第 3 周", title: "开阻断 + 升级链路", body: "L1 高置信直接阻断；L2 8B 复核灰区（1–5% 流量）；garak / promptfoo / PyRIT 红队回归进 CI" },
    { week: "第 4 周", title: "输出侧 + 闭环", body: "输出相似度比对 + canary 上线；打通'拦截样本→标注→LoRA 微调→回归'回流闭环；补齐最小权限与 HITL 清单" },
  ],

  // §0 关键数字（证据对象）
  key_numbers: [
    { v: "95.22%", k: "PINT 官方榜首（Lakera Guard，自家基准）", site: "scanner" },
    { v: "81.2%", k: "Prompt Guard 2 86M · AgentDojo 真实攻击拦截率", site: "gate1" },
    { v: "<1ms → 1.5s", k: "规则→小分类器→judge：三个数量级的延迟分层", site: "belt" },
    { v: "95%+", k: "流量终结于 L0/L1 前两层", site: "queue" },
    { v: "100%", k: "emoji/Unicode 混淆对 6 个商用护栏的绕过率", site: "card" },
    { v: ">90%", k: "12 种检测防御被自适应攻击绕过（The Attacker Moves Second）", site: "breach" },
  ],

  // §11.2 冲突口径
  conflicts: [
    { item: "Lakera PINT 分数", a: "现版官方 95.22%", b: "CourtGuard 论文引旧版 92.55%", resolution: "基准/模型迭代的时间性差异；以 95.22% 为准并注明旧值" },
    { item: "CaMeL 任务完成率", a: "原文 67%（AgentDojo 可证安全）", b: "第三方复现 77%（无防御基线 84%）", resolution: "并列呈现，标注复现差异；收益和任务损失都真实存在" },
    { item: "NeMo 生产成熟度", a: "早期来源：'NVIDIA 明示不建议生产'", b: "一手仓库核实：免责声明仅存于 ≤0.15.0，0.23.0 已移除", resolution: "以一手核实（口径 B）为准；内置护栏仍需自行加固" },
    { item: "Prompt Guard 2 86M 延迟", a: "Meta 官方 92.4ms（A100）", b: "H100+FP8 实测 p50 20ms；CPU 50–200ms", resolution: "硬件/量化差异，按硬件分档给出" },
    { item: "Prompt Guard 2 拦截率", a: "Meta 口径 AgentDojo 81.2%（3% 效用损失）", b: "独立 SoK：自适应多轮攻击下各 guardrail ASR 仍 >90%", resolution: "静态基准 vs 自适应攻击两个口径，必须同时呈现" },
    { item: "gpt-oss-safeguard 注入检测力", a: "官方多政策准确率 46.3%/43.6%，ToxicChat 独立榜开源第一", b: "官方自认注入/越狱基准弱于 gpt-oss 基座（20b 低 1–5 分）", resolution: "定位慢路径深检/审计，不做唯一在线闸；两个口径并列" },
  ],

  // ═══ 报告第 10–14 章：政策推理 · AgentDoG · I/O 形态 · 多轮 · 512 墙 ═══

  // §10 gpt-oss-safeguard 规格与成绩（政策推理式护栏，慢路径）
  gptoss: {
    what: "OpenAI 2025-10-29 发布的开放权重安全推理模型（research preview）：120b（117B 总参/5.1B 激活）与 20b（21B/3.6B）两档，Apache 2.0；推理时读取开发者自写政策做 CoT 判定（bring your own policy），源自内部 Safety Reasoner（Sora 2/GPT-5/ChatGPT Agent 生产栈）",
    io: "harmony 格式：developer/system 放政策（最优 400–600 token），user 放待判定内容；输出=判定（0/1 或 JSON）+ 完整推理链（analysis 通道）；reasoning effort low/medium/high 可调",
    ctx: "131,072 token（128K 级），可装完整会话",
    hw: "120b 需单张 80GB GPU；20b MXFP4 后 16GB 消费级可跑，vLLM/Transformers/Ollama",
    scores: [
      { k: "官方多政策准确率（全对才对）", v: "46.3% / 43.6%", who: "120b / 20b，超 gpt-5-thinking 43.2%", src: "K33" },
      { k: "ToxicChat 独立榜 F1", v: "79.9% / 79.3%", who: "20b / 120b，开源护栏榜首（Qwen3Guard-8B 73%、LlamaGuard3-8B 51%）", src: "K37" },
      { k: "ML-Bench 多语言二元 F1", v: "0.85", who: "20b，远超 Qwen3Guard-Gen-8B 0.61 / LG4-12B 0.45", src: "K33" },
      { k: "第三方中文 F1", v: "≈80", who: "YuFeng-XGuard 实测（response 侧 80.4），官方未评多语言", src: "K31" },
      { k: "官方注入/越狱基准", v: "弱于基座", who: "20b 越狱基准比 gpt-oss 基座低 1–5 分——注入检测的警惕信号", src: "K33" },
    ],
    role: "慢路径深检/异步审计：自定义注入政策 + 判定理由可审计；秒级延迟，不做唯一在线闸（官方推荐'小分类器预筛 + Safety Reasoner 深检'双层）",
  },

  // 补 §1.2 ShieldGemma 排除结论
  shieldgemma: {
    what: "Google 基于 Gemma 2 IT 的 SFT 内容安全分类器（2024-07-31，2B/9B/27B）；固定 4 类内容危害（性/危险/仇恨/骚扰），输出 Yes/No 单 token softmax 概率；8K 上下文、仅英语训练",
    exclude: [
      "词表无注入/越狱类：越狱提示往往不含仇恨色情语义，会被判 No",
      "仅英语：RabakBench 明示 supports only English，多语言实测垫底，中文流量不可用",
      "ToxicChat 独立榜大档反而更差（27B 仅 48%）",
    ],
    note: "ShieldGemma 2（2025-03）是 4B 纯图像分类器，不处理文本；文本线实质停在 Gemma 2 代。Gemma ToU 非 OSI 开源：禁用用途政策+传递义务，企业用需法务审查",
  },

  // 补 §1.3 政策推理/定位对比表
  policy_compare: [
    { dim: "检测目标", gptoss: "任意自定义政策（可写注入政策）", sg: "4 类内容危害，不含注入", qwen: "9 类危害含 Jailbreak 专项，三级严重度", pg: "专精注入+越狱二分类" },
    { dim: "许可", gptoss: "Apache 2.0", sg: "Gemma ToU（禁用政策+传递义务）", qwen: "Apache 2.0", pg: "Llama 许可" },
    { dim: "上下文", gptoss: "131K", sg: "8K", qwen: "32K 级", pg: "512 token（切块）" },
    { dim: "中文/多语言", gptoss: "官方未评；第三方中文 F1≈80", sg: "仅英语", qwen: "119 语言", pg: "8 语无中文披露" },
    { dim: "延迟", gptoss: "秒级（CoT 推理）", sg: "毫秒~百毫秒（1 token）", qwen: "亚秒级", pg: "86M ~92ms / 22M 19ms" },
    { dim: "适用层", gptoss: "慢路径深检/异步审计", sg: "本场景排除", qwen: "中文快/中路径强候选", pg: "快路径前置闸（间接注入盲区）" },
  ],

  // §11 AgentDoG 核实与成绩
  agentdog: {
    v1: "AgentDoG 1.0（arXiv:2601.18491，2026-01-26，SJTU/复旦/PKU/UIUC/Shanghai AI Lab）：诊断式轨迹级护栏；输入=完整执行轨迹（action–observation 序列）+工具描述，输出=轨迹级 safe/unsafe + 三维诊断（风险来源×失败模式×现实后果）；4B/7B/8B 三档，配套 ATBench",
    v15: "AgentDoG 1.5（arXiv:2605.29801，2026-05-28）：0.8B/2B/4B/8B 四档（Qwen3.5 系），约 1k 条影响函数净化样本 SFT+RL；轻量化+可在线 Pre-Reply 部署",
    scores: [
      { k: "R-Judge（1.5-4B）", v: "92.2% acc / 92.7 F1" },
      { k: "ATBench（1.5-4B）", v: "72.4% acc / 74.3 F1", note: "较 1.0-4B +8.4" },
      { k: "三维细粒度诊断均值", v: "55.2%", note: "1.0-4B 34.6%；GPT-5.4 25.8%" },
    ],
    prerply: [
      { bench: "ClawSafety", before: 56.25, after: 18.75 },
      { bench: "AgentHazard", before: 41.92, after: 26.92 },
      { bench: "CIK-Bench", before: 94.29, after: 42.86 },
    ],
    limit: "仅文本轨迹（GUI 多模态是未来方向）；只覆盖交付前检查点，无法阻止已发生的外部副作用",
  },

  // §11.6 SingGuard 与 SingGuard-NSFA（蚂蚁 inclusionAI，2026-07-13 开源，Apache-2.0）
  // 注意口径：所有基准均为蚂蚁自建，横向对比仅官方口径，尚无第三方独立复测
  singuard: {
    sg: "SingGuard（arXiv:2606.22873，v1 2026-06-22）：政策自适应多模态护栏，把安全政策作为运行时自然语言输入而非训练期固化分类法，规则热更新无需重训；基于 Qwen3-VL 发布 2B/4B/8B 三档，快-慢解耦 RL 支持快速直判↔政策落地慢推理双模式；默认政策 8 大风险类别（E 类=智能体安全：系统提示/内部政策泄露）",
    nsfa: "SingGuard-NSFA（arXiv:2607.13081）：智能体行为护栏，CIA 三元组×3 份 OWASP 指南交叉构建 7 一级域 / 28 二级风险 / 185 三级变体分类法（查询侧含 Prompt Injection & Jailbreak、敏感信息窃取、危险工具滥用、资源耗尽）；骨干 Qwen3.5-Base 0.8B/2B/4B/9B，四阶段合成数据管线扩展至 133 种语言；双模式=生成式 CoT 审计报告 + 判别式可插拔 MLP 分类头（冻结骨干，单次前向）",
    keynums: [
      { v: "45–57ms", k: "NSFA 判别式分类头单样本延迟；分类头 5→50,000 个端到端仅 +9ms", src: "K84" },
      { v: "F1 >94%", k: "NSFA 四档在自建多语言基准均 >94%，比最强竞品高 6–12 个绝对点；9B 泛化 F1 91.29%", src: "K84" },
      { v: "+17.6 F1", k: "NSFA 分类头作插件增强 Llama Guard 3：多语言 Query F1 67.66→85.23", src: "K84" },
      { v: "35 个数据集", k: "SingGuard 在 6 大评测家族 35 个数据集平均 F1 全部第一（对标 Llama Guard 3 / ShieldGemma / GPT-5.1 / Gemini3-Pro）", src: "K83" },
      { v: "133 种语言", k: "NSFA 四阶段合成管线经 TranslateGemma-27B 扩展；公开基准 NSFA-Query-Multilingual 63,431 条", src: "K84" },
      { v: "185 风险变体", k: "NSFA 分类法：7 一级域（查询侧 5+响应侧 2）× 28 二级风险 × 185 三级变体", src: "K84" },
    ],
    plugin: [
      { bench: "Query（多语言）", before: 67.66, after: 85.23 },
      { bench: "Response", before: 83.61, after: 92.44 },
      { bench: "CrossSource-Query", before: 82.28, after: 85.26 },
    ],
    caveat: "口径提醒：SingGuard 与 NSFA 的全部基准均为蚂蚁自建，与 Qwen3Guard、Llama Guard 3 等的横向对比目前仅有官方口径，尚无第三方独立复测——引用其 F1 数字按'厂商宣称'口径处理",
    policy_acc: "动态规则评测：运行时政策切换下政策遵循准确率 0.6465→0.7415；配套 SingGuard-Bench 56,340 条 / 80+ 细粒度风险类型（待发布）",
  },

  // 补 §2.3 2025Q4 后新基准精选
  new_benches: [
    { name: "WAInjectBench", date: "2025-10", obj: "Web Agent 注入检测器，6 类攻击×双模态，评 12 个检测器", find: "隐式/无显式指令攻击下几乎所有检测器 TPR≈0", src: "K41" },
    { name: "ClawSafety", date: "2026-04", obj: "高权限个人 Agent，120 场景×3 通道，2,520 次沙箱", find: "ASR 40–75%；skill 文件通道最危险（平均 69.4%）", src: "K42" },
    { name: "AgentHazard", date: "2026-04", obj: "Computer-Use Agent，2,653 可执行实例", find: "'局部合理、全局不安全'多步轨迹（平均 11.55 步）；Claude Code ASR 73.63%", src: "K42" },
    { name: "OS-BLIND", date: "2026-04", obj: "CUA 环境嵌入攻击，300 手工样本", find: "良性指令即可中招；单 Agent ASR 73.0%、多 Agent 92.7%；对齐几乎只在第 1 步激活", src: "K42" },
    { name: "LITMUS", date: "2026-05", obj: "真实 Ubuntu OS 行为越狱，819 高危用例", find: "'执行幻觉'：嘴上拒绝、手已执行；Claude Sonnet 4.6 执行 40.6% 高危操作", src: "K42" },
  ],

  // 补 §2.4 工业界动态精选
  industry_moves: [
    { who: "Palo Alto Prisma AIRS 3.0", date: "2026-03", what: "整合 Protect AI；AI Agent Gateway + Agent Identity（RBAC+审计）；2026-04 再收购 AI 网关 Portkey" },
    { who: "Cisco AI Defense", date: "2026-02", what: "AI BOM（覆盖 MCP 依赖）+ 多语言多轮自适应红队（200+ 子类）+ 实时 agentic 护栏；与 NeMo Guardrails 开发者级集成" },
    { who: "SentinelOne × Prompt Security", date: "2025-08 收购", what: "浏览器扩展/桌面/API 三入口；注入/越狱/系统提示抽取检测 <200ms；MCP 网关覆盖 13,000+ 服务器" },
    { who: "Cloudflare AI Security for Apps", date: "2026 更名并入 WAF", what: "模型无关 LLM 专项检测（注入评分/PII/主题），写入 WAF 字段供规则拦截" },
    { who: "网关标配化 + LiteLLM 投毒", date: "2026-03", what: "Kong/Portkey 网关护栏成熟；LiteLLM PyPI 1.82.7/1.82.8 遭供应链投毒——检测组件自身供应链可信度纳入选型" },
  ],

  // 补 §3.1 I/O 六形态
  io_forms: [
    { id: "A", name: "encoder 分类器", io: "单段纯文本进 / 标签+softmax 概率出", reps: "ProtectAI · deepset · Prompt Guard 2", lat: "20–100ms（可 CPU）", note: "512 token 硬截断，无 role 概念；长输入必须自行切片；阈值自由可调" },
    { id: "B", name: "托管 verdict API", io: "messages[] 或 userPrompt+documents[] 进 / 布尔 verdict 出", reps: "Lakera · Azure · AWS · Model Armor", lat: "50–500ms + RTT", note: "普遍无连续分数、无请求级阈值；策略在厂商控制台调；灰度只能先 log 不拦" },
    { id: "C", name: "生成式判定", io: "chat template 对话进 / 需正则解析的生成文本出", reps: "Llama Guard 3/4 · Qwen3Guard-Gen · WildGuard", lat: "百 ms–亚秒", note: "能吃完整历史（32K–164K）；输出解析与失败兜底是必写胶水代码" },
    { id: "D", name: "政策推理", io: "policy + content 进 / 判定+理由出", reps: "gpt-oss-safeguard · ShieldGemma · SingGuard", lat: "百 ms–秒级", note: "政策即代码（需版本管理与回归测试）；理由可审计，适合高风险流量第二级；SingGuard 把政策做成运行时热加载输入，免重训" },
    { id: "E", name: "token 级流式", io: "token ids + stream_state 进 / 逐 token 标签出", reps: "Qwen3Guard-Stream", lat: "随生成零额外首包", note: "唯一能生成中途止损的形态；但需侵入推理服务层，HTTP 网关接不了" },
    { id: "F", name: "轨迹审计", io: "完整 trace 进 / 轨迹级判定+诊断出", reps: "LlamaFirewall AlignmentCheck · AgentDoG", lat: "百 ms–秒级", note: "唯一能检测间接注入/目标劫持的形态——单消息分类器看不到跨步意图偏离" },
  ],

  // 补 §3.2 I/O 主对照表（16 方案 + 2 新框架补充）
  io_specs: [
    { name: "Lakera Guard", form: "B", input: "OpenAI 风格 messages[]", role: "✅", ctx: "单 prompt ≤8,000 token", output: "flagged 布尔 + breakdown（序数档 l1–l5）", thr: "❌ 控制台 L1–L4", ship: "托管 REST", lat: "官方 <50ms / 实测 51–66ms" },
    { name: "Azure Prompt Shields", form: "B", input: "{userPrompt, documents[]}", role: "❌（用户提示 vs 文档）", ctx: "约 10K 字符/次", output: "attackDetected 纯布尔，无分数", thr: "❌", ship: "Azure REST", lat: "第三方 200–500ms" },
    { name: "AWS Bedrock Guardrails", form: "B", input: "source=INPUT/OUTPUT + content[] + input-tagging", role: "⚠️ 靠标签", ctx: "视配置", output: "action + assessments[]（LOW/MED/HIGH）", thr: "⚠️ 三档离散强度", ship: "AWS SDK", lat: "+50–200ms" },
    { name: "Google Model Armor", form: "B", input: "文本/消息", role: "⚠️", ctx: "视配置", output: "verdict + 过滤器明细", thr: "❌", ship: "GCP API", lat: "百 ms 级" },
    { name: "ProtectAI DeBERTa-v3-v2", form: "A", input: "单段英文文本", role: "❌", ctx: "512 token", output: "INJECTION/SAFE + softmax", thr: "✅ 任意阈值", ship: "HF 模型 Apache-2.0 · 184M", lat: "CPU ~50ms/512tok" },
    { name: "Meta Prompt Guard 2（86M/22M）", form: "A", input: "单段任意文本", role: "❌", ctx: "512 token", output: "二分类 LABEL_0/1 + 概率", thr: "✅", ship: "HF 模型 Llama 许可（gated）", lat: "86M 92.4ms / 22M 19.3ms（A100）" },
    { name: "deepset deberta-v3-injection", form: "A", input: "单段英文文本", role: "❌", ctx: "512 token", output: "LEGIT/INJECTION + 概率", thr: "✅", ship: "HF 模型 MIT · 184M", lat: "同 ProtectAI 量级" },
    { name: "Qwen3Guard-Gen（0.6/4/8B）", form: "C", input: "chat template（user=prompt 审；user+assistant=response 审）", role: "✅", ctx: "32K（YaRN 131K）", output: "生成文本 Safety/Categories/Refusal，需正则解析", thr: "❌ 三级标签即阈值", ship: "HF/vLLM Apache-2.0", lat: "首 token 89–253ms" },
    { name: "Qwen3Guard-Stream", form: "E", input: "token ids + role= + stream_state 增量", role: "✅", ctx: "32K 级", output: "逐 token risk_level/category，可中途掐断", thr: "❌", ship: "HF 模型（trust_remote_code）", lat: "随生成零额外首包" },
    { name: "Llama Guard 3（1/8B）", form: "C", input: "chat template 对话，S1–S14 可裁剪", role: "✅ User/Agent", ctx: "128K", output: "safe/unsafe + 类别码（可取 logit 概率）", thr: "⚠️ logit 自建阈值", ship: "HF 模型 Llama 许可", lat: "8B 级百 ms" },
    { name: "Llama Guard 4（12B 多模态）", form: "C", input: "chat template，content 可混 text+image", role: "✅", ctx: "163,840 token", output: "safe/unsafe + S1–S14 码", thr: "⚠️ 同上", ship: "HF 模型 + Moderations API", lat: "12B 级百 ms" },
    { name: "WildGuard（7B）", form: "C", input: "固定模板填空 {prompt}/{response}", role: "⚠️ 模板内角色", ctx: "~32K（Mistral-7B）", output: "三行键值：Harmful request / refusal / response", thr: "❌", ship: "HF 模型 Apache-2.0 / pip", lat: "7B 级百 ms" },
    { name: "ShieldGemma（2/9/27B）", form: "D", input: "chat + 单条 guideline（政策文本）", role: "✅", ctx: "8K", output: "Yes/No 两 token softmax 概率（不生成文本）", thr: "✅ 概率阈值自定", ship: "HF 模型 Gemma 许可（gated）", lat: "2B 近实时" },
    { name: "gpt-oss-safeguard（20b/120b）", form: "D", input: "system=自写政策全文；user=待审内容（harmony）", role: "✅ system/user", ctx: "131K", output: "reasoning 链 + 政策自定义输出（0/1 或 JSON）", thr: "✅ 政策自定；effort 控延迟", ship: "HF 开放权重 Apache-2.0 · vLLM", lat: "随 effort 百 ms–秒级" },
    { name: "LLM Guard（scanner 套件）", form: "框架（挂 A）", input: "scan_prompt(scanners, prompt) 单段文本", role: "❌", ctx: "内置 scanner 512", output: "(sanitized, valid{bool}, score{float}) 三元组", thr: "✅ 每 scanner 独立", ship: "Python 库 MIT", lat: "DeBERTa 级 ~50ms" },
    { name: "NeMo Guardrails", form: "框架/编排", input: "rails.generate(messages) 或 OpenAI 兼容端点", role: "✅", ctx: "视所挂 rail", output: "通过的 messages / 拒绝消息 + output_data", thr: "✅ config.yml", ship: "库 / server / K8s Apache-2.0", lat: "分类器 rail 20–50ms；self-check ≈+1 次 LLM 调用" },
    { name: "LlamaFirewall", form: "框架（A+F）", input: "scan(UserMessage) 按 Role 路由；AlignmentCheck 吃完整 Trace", role: "✅ 五类 Role", ctx: "PromptGuard 512 / 审计窗", output: "ScanResult(ALLOW/BLOCK/HITL, reason, score)", thr: "⚠️ score 连续、decision 内置", ship: "Python 库 / sidecar BSD", lat: "PromptGuard <20ms；审计百 ms–秒级" },
    { name: "AgentDoG 1.5（0.8–8B）", form: "F", input: "完整执行轨迹（action–observation）+ 工具描述", role: "—", ctx: "轨迹级", output: "轨迹级 safe/unsafe + 三维诊断标签", thr: "—", ship: "开源（Qwen3.5 系）", lat: "Pre-Reply TTFT 亚秒级" },
    { name: "SingGuard（2B/4B/8B）", form: "D", input: "chat 消息（query / query+response / 图文多模态）+ 可选 policy 自然语言规则", role: "✅", ctx: "Qwen3-VL 级", output: "首行 safe/unsafe + 政策落地推理链 + <answer>风险类别</answer>", thr: "⚠️ 政策即阈值（运行时热加载）", ship: "HF/ModelScope Apache-2.0 · vLLM", lat: "快模式直判 / 慢模式推理链" },
    { name: "SingGuard-NSFA（0.8–9B）", form: "A/C", input: "无状态单轮文本（query 侧或 response 侧，<untrusted_*> 边界标签）", role: "⚠️ query/response 两侧", ctx: "单轮", output: "生成式 CoT 审计报告 / 判别式多标签风险判定（末 token 并行分类头）", thr: "✅ 分类头分数可设阈", ship: "HF/ModelScope Apache-2.0（含 GGUF）", lat: "判别模式 45–57ms/样本" },
  ],

  // 补 §4 多轮对话防护：四类方法
  multiturn_methods: [
    { name: "① 压缩检测", rep: "Defensive M2S（KAIST）", io: "多轮对话 → 结构化压缩成伪单轮 → 常规护栏", perf: "Qwen3Guard+hyphenize 93.8% 召回（+38.9pp）；token 3,231→173（−94.6%）", role: "网关低成本首道筛" },
    { name: "② 会话级风险累积", rep: "vLLM Semantic Router 等", io: "逐轮分数流 → 状态机累积/衰减/取 max → 会话级风险分", perf: "include_history 取历史最大风险分；per-category 阈值 0.5–0.9", role: "网关侧 EMA/max；必须有衰减，否则良性长对话被误拦" },
    { name: "③ 多轮感知护栏", rep: "Llama Guard 3/4 · Qwen3Guard", io: "messages 数组进 → 末次交互判定+类别", perf: "全链路可见的末条判定器；跨轮累积逻辑仍需调用方补齐", role: "长历史配合压缩（混合窗口）" },
    { name: "④ 行为审计（轨迹级）", rep: "LlamaFirewall AlignmentCheck · AgentDoG 1.5", io: "推理轨迹+用户目标 → 逐步一致性判定", perf: "AgentDojo ASR：PromptGuard2 单用 7.5% → 组合 1.75%（相对降 >90%）", role: "高权限场景；成本最高，异步化" },
  ],

  // 补 §4.3 多轮支持矩阵
  multiturn_matrix: [
    { name: "Lakera Guard", hist: "✅ messages 数组", scope: "仅末次交互；历史仅作上下文不重筛", sem: "上下文感知末条判定，逐轮调用" },
    { name: "Azure Prompt Shields", hist: "❌ 单 userPrompt", scope: "userPrompt + documents 逐份判定", sem: "会话无感" },
    { name: "AWS Bedrock Guardrails", hist: "⚠️ 文本单元数组无角色", scope: "source: INPUT|OUTPUT 按单元", sem: "会话无感；可拼历史评估" },
    { name: "Prompt Guard 2", hist: "❌ 单字符串 512 窗口", scope: "二分类", sem: "纯单轮，长输入分段并行扫" },
    { name: "Qwen3Guard", hist: "✅ chat template", scope: "末对 user(+assistant)，三级+Jailbreak 类", sem: "上下文感知；长历史配压缩" },
    { name: "Llama Guard 3/4", hist: "✅ conversation 原生多轮", scope: "末条 + 全前文", sem: "全链路可见；长会话效果递减" },
    { name: "NeMo Guardrails", hist: "✅ generate(messages)", scope: "input/dialog/output rails", sem: "dialog rails 走 Colang 流；内置启发式为单轮" },
    { name: "京东 JoySafety v2", hist: "✅ 内置多轮上下文注入检测", scope: "DAG 策略引擎多模型融合", sem: "国产生产级框架；v2 重点加强多轮" },
    { name: "AgentDoG 1.5 / AlignmentCheck", hist: "✅ 轨迹级", scope: "跨步意图一致性", sem: "唯一覆盖间接注入/目标劫持的形态" },
  ],

  // 补 §5.1 各模型上下文上限核实
  ctx_limits: [
    { name: "ProtectAI DeBERTa-v3 v2", base: "DeBERTa-v3-base", ctx: 512, label: "512", note: "官方示例写死 truncation max_length=512" },
    { name: "deepset deberta-v3-injection", base: "DeBERTa-v3-base", ctx: 512, label: "512", note: "AgentDojo APR 仅 13.5%" },
    { name: "Meta Prompt Guard 2（86M/22M）", base: "mDeBERTa-v3 / deberta-xsmall", ctx: 512, label: "512", note: "官方提供长输入并行扫描 utilities" },
    { name: "qualifire Sentinel", base: "ModernBERT-large 395M", ctx: 8192, label: "8K", note: "原生 8K；但 >512 注入指标未公开，第三方仍按 512 调用——利用率存疑" },
    { name: "ShieldGemma", base: "Gemma 2", ctx: 8192, label: "8K", note: "仅英语、无注入类，本场景排除" },
    { name: "Qwen3Guard-Gen", base: "Qwen3", ctx: 32768, label: "32K", note: "YaRN 可扩 131K；官方 --max-model-len 32768" },
    { name: "Llama Guard 3 8B", base: "Llama-3.1-8B", ctx: 131072, label: "128K", note: "可一次吃下整段对话历史" },
    { name: "gpt-oss-safeguard 20b/120b", base: "gpt-oss", ctx: 131072, label: "131K", note: "政策本身建议 400–600 token" },
    { name: "Llama Guard 4 12B", base: "Llama 4 Scout 剪枝", ctx: 163840, label: "164K", note: "多模态；单卡 24GB" },
  ],
  ctx_wall: {
    zh: "中文换算：1 汉字≈1–2 token，512 token 约容 250–500 汉字；按英文字符数估算窗口会严重误判，留 10–15% 余量",
    traps: [
      "窗口缝隙已武器化：'What the Guardrail Inspects' 专门攻击 512 护栏 vs 400K 下游模型的盲区",
      "Lost in the middle：注入藏中段，'保首尾丢中段'的截断方向恰好反了",
      "Many-shot 随上下文幂律上升（MSJ-128 ASR 31%），逐块打分被稀释",
      "分块切断攻击句：两半各自无害→漏检；需 10–20% 重叠+按句边界切",
    ],
  },

  // 补 §5.2 512 墙六种工程解法
  ctx_solutions: [
    { name: "① 首尾截断", mech: "保前 512 或前 128+后 382（LLM Guard HEAD_TAIL）", pro: "零成本、单次推理、延迟最低", con: "注入藏中段即完全绕过——致命" },
    { name: "② 滑窗分块 + any-hit", mech: "≤512 重叠窗（stride 50，重叠 10–20%）逐块打分，任一命中即报", pro: "注入是局部信号，any-hit 是标准聚合", con: "FPR 随块数膨胀；需调高阈值或要求连续两块命中" },
    { name: "③ ModernBERT 8K 底座", mech: "原生 8K encoder 微调（Sentinel：F1 0.980 vs ProtectAI 0.728）", pro: "速度 2×、显存 1/5，单块覆盖多数 RAG chunk", con: "8K 对数十万 token 仍杯水车薪；>512 指标未公开" },
    { name: "④ 生成式护栏整段审", mech: "Qwen3Guard 32K / LG3 128K / gpt-oss-safeguard 131K 换架构绕墙", pro: "整篇一次判完，理解'意图+外部内容'组合攻击", con: "延迟/成本高 1–2 个数量级，放第二级" },
    { name: "⑤ Embedding 粗筛", mech: "已知注入向量库相似度检索，或'意图+内容'拼接过 embedding", pro: "无需大模型、攻击库可增量更新", con: "全局语义压缩稀释注入句；只配做第一道粗筛" },
    { name: "⑥ 网关两段式：粗筛→精审", mech: "正则/embedding 定位可疑片段 → 只送命中片段精判", pro: "成本精度兼得，天然适配 RAG；gpt-oss 官方推荐 hybrid", con: "第一道漏掉的段落永远进不了第二道（级联漏检）" },
  ],

  // 补 §5.4 按输入类型分流建议
  ctx_routing: [
    { type: "用户消息（<2K token）", plan: "直接过 Prompt Guard 2；超长首尾截断或 2–4 块滑窗+max；中文超 2K 字必须分块；这一层 <100ms，不上生成式" },
    { type: "RAG chunk", plan: "chunk 本身是 256–512 天然单位：逐 chunk any-hit，命中只丢毒 chunk；chunk 更大换 Sentinel 8K" },
    { type: "工具返回长文档", plan: "两段式：embedding/关键词粗筛 → Qwen3Guard-Gen 32K 精审；高危操作前的返回直接生成式整段审" },
    { type: "完整对话历史", plan: "512 encoder 不可用（FPR 膨胀）：生成式整段审（128K/131K）+ 增量扫描（每轮只扫新增）+ 限制历史长度" },
  ],

  // ══ 第四轮 · 最终版第 15 章：学术脉络谱系树（10 组核心论文）══
  lineage: [
    { core: "LlamaFirewall + Prompt Guard 2", base: "Instruction Hierarchy、Spotlighting、CaMeL", succ: "MAC 框架（特权升级形式化）；AttriGuard 直接对比；Unsafer in Many Turns 打脸单轮护栏", pos: "开源'分层护栏系统'参考实现，2026 年 agent 安全论文标准对比基线", src: "K22][K51" },
    { core: "Qwen3Guard", base: "GuardReasoner（100 引直系前驱）、WildGuard、Llama Guard 系", succ: "NExT-Guard 免训练流式护栏攻其标注成本；MultiBreak/One Turn Too Late 作多轮评测对象；TWGuard 走本地化", pos: "119 语言开源护栏当前 SOTA，被各类流式/本地化护栏对标", src: "K6][K61" },
    { core: "AgentDoG 1.5", base: "AGrail、SafeArena 类轨迹评测 + 护栏训练系", succ: "被用于 Clawdbot/OpenClaw 轨迹级安全审计；ATBench 同时引它与 Qwen3Guard", pos: "'输入级分类→轨迹级诊断'范式代表，正被真实 agent 审计采用", src: "K36" },
    { core: "WildGuard / Aegis 2.0 / GuardReasoner", base: "Llama Guard + WildJailbreak 数据；R1 式推理蒸馏", succ: "GuardReasoner-VL 扩到多模态；HaloGuard/kNNGuard 走免训练；Aegis 2.0 以数据集形态持续被用作训练集", pos: "奠定'开放式护栏数据'底座并开启'护栏也要推理'路线", src: "K61][K65][K52" },
    { core: "SecAlign ← StruQ ← Spotlighting 一脉", base: "Spotlighting 数据标记思想 → StruQ 结构化指令微调 → SecAlign 偏好优化", succ: "DataSentinel（137 引）是检测侧最强回应；DefensiveTokens 轻量替代；ProGent 走特权控制", pos: "'让模型自己分清指令与数据'训练侧主线，2025 后分支为权限收缩与 token 化", src: "K14][K48][K50][K49" },
    { core: "Spotlighting / CaMeL / FIDES", base: "数据标记 → 能力+策略架构隔离 → 信息流控制形式化", succ: "CaMeLs Can Use Computers Too 移植到 CUA；Skill-Inject 以其为防御对照；ASIDE 架构内分离指令/数据", pos: "唯一在 Attacker Moves Second 自适应评测中幸存的路线", src: "K16][K54][K53][K59" },
    { core: "The Attacker Moves Second", base: "Jailbroken 失败分析方法论 + 12 种被测防御", succ: "BrowseSafe、ceLLMate 按其标准重测浏览器场景；CaMeLs CUA 版以它为评测协议；PISmith 把自适应攻击自动化", pos: "防御评测的'终审法庭'：之后的新防御论文被迫补做自适应攻击实验", src: "K3][K55][K58" },
    { core: "Crescendo / Jailbroken", base: "能力/安全训练竞争的失败分类学", succ: "Foot-In-The-Door 登门槛多轮越狱；SafeArena 把 Crescendo 式攻击搬进 Web agent；IH-Challenge 回应指令混淆", pos: "攻击侧'祖父论文'，演化为多轮与 agent 化两条线", src: "K13][K12" },
    { core: "PromptShield / InjecGuard / GenTel", base: "encoder 分类器系 + 基准建设", succ: "'Learn Surface Heuristics'打脸整条检测路线；'Firewalls All You Need'证明基准太弱；Rennervate 走机理化", pos: "检测器系三篇已成'评测对象'：新工作主要论证它们为何不靠谱", src: "K5][K63][K56][K57" },
    { core: "gpt-oss-safeguard 技术报告", base: "gpt-oss 模型卡 + Instruction Hierarchy + deliberative alignment", succ: "被 monitor 研究方向当标准基线；OpenGuardrails 与之并列", pos: "'自带政策推理的开放权重安全模型'代表", src: "K33" },
  ],

  // 最终版第 15.3 章：新增论文 13 篇（净增量，已去重）
  new_papers: [
    { name: "DataSentinel", id: "arXiv:2504.11358", date: "2025-04 · 137 引", grp: "检测与防御", gist: "博弈论 min-max 训练注入检测器：与自适应攻击者在训练循环里互相博弈，对'针对检测器定制'的注入保持鲁棒", why: "SecAlign/StruQ/IH 三篇共同的最高引检测侧被引，检测侧必读", src: "K48" },
    { name: "ProGent", id: "arXiv:2504.11703", date: "2025-04 · 79 引", grp: "检测与防御", gist: "最小权限策略控制 agent 工具调用：每个动作只拿完成任务所需最小特权，注入成功也无法越权", why: "与 CaMeL 并列被 StruQ/SecAlign/IH 三篇共同引用；比架构隔离更易渐进落地", src: "K49" },
    { name: "DefensiveTokens", id: "arXiv:2507.07974", date: "2025-07 · 42 引", grp: "检测与防御", gist: "输入前插入少量训练好的'防御 token'即达接近 SecAlign 的防注入效果，效用损失更小", why: "不想为防御重训整个模型时的轻量替代，部署成本几乎为零", src: "K50" },
    { name: "AttriGuard", id: "arXiv:2603.10749", date: "2026-03", grp: "检测与防御", gist: "不做输入级语义判别，改对'工具调用'做因果归因——判断动作由用户指令因果导致还是被不可信数据驱动", why: "'轨迹/因果级检测'对'输入级分类'的替代尝试，直接以 LlamaFirewall 为基线", src: "K51" },
    { name: "HaloGuard", id: "arXiv:2607.02079", date: "2026-07", grp: "检测与防御", gist: "开源复现 Anthropic Constitutional Classifier 路线，约 1/10 参数量达多语言安全分类 SOTA", why: "可自托管、可审计的多语言护栏新选项，引 WildGuard 作多语言基线", src: "K52" },
    { name: "Skill-Inject", id: "arXiv:2602.20156", date: "2026-02 · 55 引", grp: "场景与基准", gist: "首个系统测量'Agent Skill 文件注入'的基准，以 CaMeL/FIDES/Spotlighting 为防御对照", why: "直击 Skill 文件泄露/注入场景，评估自家 Skill 加载链路的首选基准", src: "K53" },
    { name: "CaMeLs Can Use Computers Too", id: "arXiv:2601.09923", date: "2026-01", grp: "场景与基准", gist: "把 CaMeL 能力+策略架构隔离移植到 Computer-Use Agent，并按自适应攻击协议验证仍成立", why: "证明架构隔离不是论文玩具，可以跟着 agent 形态走", src: "K54" },
    { name: "BrowseSafe", id: "arXiv:2511.20597", date: "2025-11 · Perplexity", grp: "场景与基准", gist: "浏览器 agent 注入的威胁建模与防御基准，评测含 FIDES 式防御", why: "浏览器是间接注入最高发场景，目前最系统的场景化研究", src: "K55" },
    { name: "Defenses Learn Surface Heuristics（打脸作）", id: "arXiv:2601.07185", date: "2026-01", grp: "打脸作", gist: "实证现有注入防御学到的是表面启发式（关键词、格式特征）而非语义理解，分布稍偏即失效", why: "对'训练一个分类器防注入'路线的根本质疑，选型必读", src: "K56" },
    { name: "Firewalls All You Need?（打脸作）", id: "arXiv:2510.05244", date: "2025-10", grp: "打脸作", gist: "简单的 agent-tool 接口防火墙在现有基准上能做到'完美'分数——不是防御够好，是基准太弱", why: "任何基准数字引用前都该先看这篇", src: "K57" },
    { name: "PISmith", id: "arXiv:2603.13026", date: "2026-03", grp: "其余增量", gist: "用 RL 红队自动生成绕过注入防御的自适应攻击", why: "把 Attacker Moves Second 的手工精神自动化", src: "K58" },
    { name: "ASIDE", id: "arXiv:2503.10566", date: "2025-03", grp: "其余增量", gist: "模型架构层面用独立嵌入分离指令与数据表示", why: "Spotlighting'输入侧标记'的架构化平行路线", src: "K59" },
    { name: "Can IPI Be Detected and Removed?", id: "arXiv:2502.16580", date: "2025-02 · 65 引", grp: "其余增量", gist: "系统研究间接注入'检测+移除'双任务的可行性边界", why: "StruQ/Spotlighting 的直接被引", src: "K60" },
  ],

  // ══ 最终版第 16 章：九模型训练数据明细 ══
  train_data: [
    { name: "Qwen3Guard", size: 1190000, slabel: "119 万条", zh: 26.6, comp: "prompt 41.2% + response 58.8%，人工+合成混合", lang: "中文 26.6%、英文 21.9%、尾部 119 语", pub: "不公开", label: "三级标签（safe/controversial/unsafe），严格与宽松阈值模型交叉标定", src: "K6", hl: true },
    { name: "ProtectAI v2", size: 290000, slabel: "~29 万条（推算）", zh: 0, comp: "22 个开源集 + 手工构造注入", lang: "仅英语", pub: "配方公开、数据不公开", label: "数据集自带标签 + 人工", src: "K66" },
    { name: "WildGuard", size: 92000, slabel: "92K", zh: 0, comp: "四象限（vanilla/adversarial × harmful/benign）+ refusal/compliance", lang: "英语", pub: "CC-BY-4.0 全公开", label: "87% 合成 + 11% 野外真实交互；GPT-4 标注 + 人工抽检（>90% 一致率）", src: "K61" },
    { name: "ShieldGemma", size: 100000, slabel: "各 50K（输入/响应）", zh: 0, comp: "AART 管线全合成 + 混入 HH-RLHF", lang: "英语", pub: "配方公开", label: "LLM 生成 + 自我批判扩增，3 名标注员多数投票", src: "K34" },
    { name: "Aegis 2.0", size: 34248, slabel: "34,248 条", zh: 0, comp: "12 核心危害类 + 9 细粒度风险，prompt 来自 HH-RLHF/DAN/AART 等", lang: "英语", pub: "公开，明确可商用", label: "12 名受训标注员 + 3 个 LLM 陪审团多数投票，5,000 条合成 refusal", src: "K65" },
    { name: "Prompt Guard 1/2", size: null, slabel: "未披露", zh: null, comp: "开源良性/恶意集 + 合成攻击 + 红队回流", lang: "英语为主 + 8 语机翻", pub: "配方公开、数据不公开", label: "未披露", src: "K4" },
    { name: "Llama Guard 3", size: null, slabel: "未披露", zh: null, comp: "HH-RLHF 衍生 + 13 类危害人工/合成 + 8 语对话", lang: "8 语", pub: "不公开", label: "人工+合成；safe 样本刻意选贴近 unsafe 边界的难例", src: "K4" },
    { name: "AgentDoG 1.5", size: 1000, slabel: "~1k 精选", zh: -1, comp: "分类法引导数据引擎 → 偏好感知影响函数筛出信息量最高样本", lang: "中英", pub: "开源", label: "目标正/负响应对构造，长度归一化似然梯度筛样", src: "K36", hl: true },
    { name: "InjecGuard/NotInject", size: 1000, slabel: "MOF 仅 1,000 条纠偏", zh: 0, comp: "训练集 14 良性+12 恶意开源集；MOF 加 1,000 条含触发词良性句", lang: "英语", pub: "全公开", label: "词频差找触发词 → 人审 → 合成 → 复核", src: "K63" },
  ],

  // 报告 17.3/17.4：自建数据集指南 + 训练成本
  finetune: [
    { dim: "训练主粮（可商用）", v: "WildGuardMix 92K · Aegis 2.0 34K · TensorTrust 126K", note: "WildGuardMix 87% 合成含 GPT-4 产出，有衍生限制争议；TensorTrust 为真人玩家注入 [K61][K65][K62]" },
    { dim: "避坑", v: "WildJailbreak 门控 · qualifire CC-BY-NC · jackhhao 标注争议", note: "jackhhao 把角色扮演标 benign 被社区批评，直接用会让 FPR 显著失真" },
    { dim: "中文构造", v: "MBT-DA 翻译回译扩增", note: "MLJailDe：11 语 2,232 良性 + 1,239 越狱即把低资源语言 F1 从 22–54% 提到 94–98%；功能有效性校验不可省 [K64]" },
    { dim: "标注规范", v: "三级标签 + 3 人多数投票", note: "标签相对'你的应用政策'而非绝对善恶；每条记录违反的政策条目，便于审计与批量重标" },
    { dim: "防污染切分", v: "来源隔离 + 攻击族分层 + 标签统一", note: "同源自评虚高见 ProtectAI 99.99%→95.25%；YuFeng-XGuard 重标注过滤约 10% 极性翻转 [K66]" },
    { dim: "数据量门槛", v: "1k 纠偏 → 10k 生产 → 30k 打平一线", note: "MOF 1,000 条纠偏；Tomoro 10k 垂直域 57% vs safeguard-120B 15%；Aegis 2.0 34K 持平 3 倍数据的 WildGuard [K63][K67]" },
    { dim: "LoRA 成本", v: "$1–2/run · 单卡 L40S · 1 小时", note: "Sentinel-v2 598M 用 LoRA r=8 只训 2.3M 参数（−99.6%），垂直域 F1 0.97；仅微调分类头显著差于 LoRA" },
    { dim: "推荐架构", v: "小分类器预筛 + 生成式护栏复核", note: "分类头低延迟但 512 窗口+触发词捷径；生成式可输出类别+理由、支持动态政策；Recall@1%FPR 为北极星指标" },
  ],

  // 报告结语：五层检测体系
  five_layers: [
    { layer: "快路径", io: "吃单条消息 → 吐标签+分数", what: "encoder（512 切块 any-hit）或 Qwen3Guard-0.6B，<100ms 终结绝大部分流量" },
    { layer: "深检", io: "吃'政策+可疑内容' → 吐判定+理由", what: "gpt-oss-safeguard-20b 或 8B 生成式护栏，只处理快路径筛出的可疑流量" },
    { layer: "会话风险分", io: "吃逐轮分数流 → 吐会话级风险分", what: "网关侧 EMA/max 状态机，解决多轮攻击的单轮盲区" },
    { layer: "轨迹审计", io: "吃完整执行轨迹 → 吐轨迹级判定+诊断", what: "AgentDoG 1.5 / AlignmentCheck；高权限场景唯一有效形态" },
    { layer: "架构兜底", io: "不赌检测得准", what: "最小权限、HITL、信息流控制（CaMeL）；承认 >90% 绕过天花板" },
  ],

  // 报告第 16 章：论文精读 35 篇（攻击 11 / 检测 12 / 防御架构与评测 12）
  papers: [
    // ── 16.1 攻击方法与威胁实证（11 篇）──
    { grp: "attack", title: "Jailbroken: How Does LLM Safety Training Fail?", info: "A. Wei, N. Haghtalab, J. Steinhardt（UC Berkeley）· NeurIPS 2023 · 2023-07", arxiv: "2307.02483", k: "K12", metric: "2 条失效机制", msub: "目标竞争 + 泛化错配，统一解释后续几乎所有输入侧攻击",
      sum: "对齐后的 LLM 为何仍系统性失守？两条根本失效机制：目标竞争——预训练/指令遵循目标与安全目标冲突时安全目标常落败（角色扮演、前缀注入、拒绝抑制均属此类）；泛化错配——能力泛化远广于安全训练覆盖的分布，Base64、低资源语言、密码对话皆利用此点。实证中 Base64 混淆提示可让 GPT-4 输出受管制物合成说明。意义：这是后续几乎所有输入侧攻击的统一解释框架，也直接说明'检测前先做解码/规范化预处理'为什么是第一优先级。" },
    { grp: "attack", title: "Universal and Transferable Adversarial Attacks on Aligned Language Models（GCG）", info: "A. Zou, Z. Wang, N. Carlini 等（CMU 等）· 2023-07", arxiv: "2307.15043", k: "K68", metric: "迁移版 ASR 4.3–11.2%", msub: "GCG-T 在低查询预算下的实测（AutoDAN-Turbo 对比口径）",
      sum: "能否自动构造对开源模型有效、且可迁移到黑盒商用模型的对抗提示？方法是用贪心坐标梯度（GCG）优化一段乱码对抗后缀，最大化模型以'Sure, here is...'开头作答的概率，可附加到任意有害请求后；对 Vicuna、LLaMA-2 高度有效，但低查询预算下迁移版 ASR 仅 4.3–11.2%。意义：乱码后缀有显著困惑度尖峰，统计级困惑度过滤即可高效拦截——最容易被输入侧预处理防住的自动化攻击。" },
    { grp: "attack", title: "Jailbreaking Black Box Large Language Models in Twenty Queries（PAIR）", info: "P. Chao, A. Robey, E. Dobriban 等（宾夕法尼亚大学）· 2023-10", arxiv: "2310.08419", k: "K69", metric: "≤20 次查询", msub: "语义可读越狱提示，查询效率较此前黑盒方法高一个数量级以上",
      sum: "研究问题：没有梯度、只有 API 访问时能否高效自动化越狱？PAIR 用一个攻击者 LLM 生成候选提示、评估 LLM 对目标回复打分、再迭代精炼语义，通常在二十次查询内产出语义可读的越狱提示，对 GPT-3.5/4、Vicuna、Gemini 等有竞争力 ASR，查询效率比此前黑盒方法高出一个数量级以上。意义：黑盒语义越狱没有乱码等统计特征，困惑度过滤无效，防御必须靠语义分类器/LLM 判定加查询行为侧检测。" },
    { grp: "attack", title: "Tree of Attacks: Jailbreaking Black-Box LLMs Automatically（TAP）", info: "A. Mehrotra, M. Zampetakis, P. Kassianik 等（CMU、Bosch Center for AI 等）· NeurIPS 2024 · 2023-12", arxiv: "2312.02119", k: "K70", metric: "ASR >80%", msub: "对 GPT-4 Turbo / GPT-4o，且可绕过 LlamaGuard 等外围检测器",
      sum: "PAIR 的单链迭代浪费了大量中间候选，能否更系统地探索提示空间？TAP 把 PAIR 扩展为树搜索：攻击者 LLM 每轮分支生成多个变体，用评估器剪枝（剪除离题分支），沿最有希望的路径深入。结果对 GPT-4 Turbo、GPT-4o 的 ASR 超过 80%，且生成的越狱可绕过 LlamaGuard 等外围检测器，查询预算与 PAIR 相当。意义：即便叠加了开源护栏，黑盒迭代攻击仍能以少量查询打穿，单一前置检测器不构成防线。" },
    { grp: "attack", title: "AutoDAN-Turbo: A Lifelong Agent for Strategy Self-Exploration to Jailbreak LLMs", info: "X. Liu, P. Li, E. Suh 等（UIUC 等）· 2024-10", arxiv: "2410.05295", k: "K71", metric: "平均 6.72 次查询/案", msub: "低查询预算下 ASR 全面领先 GCG-T、PAIR、TAP",
      sum: "研究问题：现有自动化攻击每打一个目标都从零开始，能否把'越狱策略'积累成可复用知识？AutoDAN-Turbo 是终身学习 agent：测试期自主探索、把成功策略结构化存入策略库，之后检索复用，无需人工设计模板。在低查询预算下其 ASR 全面领先 GCG-T、PAIR、TAP，平均每案仅约 6.72 次查询。意义：攻击成本已进入'个位数查询、无人值守'区间，防御方的红队回归也必须自动化、常态化，人工红队节奏跟不上。" },
    { grp: "attack", title: "Great, Now Write an Article About That: The Crescendo Multi-Turn LLM Jailbreak Attack", info: "M. Russinovich, A. Salem, R. Eldan（Microsoft）· USENIX Security 2025 · 2024-04", arxiv: "2404.01833", k: "K13", metric: "GPT-4 98% · Gemini Pro 100%", msub: "二元成功率；平均 ASR 比其他多轮方法高 29–71 个百分点",
      sum: "如果每一轮输入都完全无害，还能越狱吗？Crescendo 从无害问题开始，逐轮引用模型自己的回复逐步升级话题，大多数任务 5 轮内完成；对 GPT-4 二元成功率 98%，对 Gemini Pro 100%。关键特性是输入'不含任何对抗性文本、完全人类可读且表面良性'，配套工具 Crescendomation 支持被拒后回溯重试。意义：对单轮输入检测器威胁最大的一类攻击——单条消息无任何特征，必须上会话级话题漂移/升级监控。" },
    { grp: "attack", title: "DeepInception: Hypnotize Large Language Model to Be Jailbreaker", info: "X. Li, Z. Lin 等（MBZUAI、上海交通大学等）· 2023-11", arxiv: "2311.03191", k: "K26", metric: "绕过 LlamaGuard", msub: "诱导内容可绕过 LlamaGuard 与 OpenAI 审核 API",
      sum: "能否让模型'自己催眠自己'产出有害内容？DeepInception 构造多层嵌套虚拟场景（每层人物承接上一层指令），让有害内容由虚构角色说出，深度嵌套稀释对齐信号；方法轻量、无需优化，对 Llama-2/3、GPT-3.5/4/4o 均有效，诱导内容可绕过 LlamaGuard 与 OpenAI 审核 API。意义：token 级与浅层语义检测都会漏'语义相关深嵌套'，需要语义级 LLM 判定加输出侧分类的双层结构。" },
    { grp: "attack", title: "PLeak: Prompt Leaking Attacks against Large Language Model Applications", info: "B. Hui, H. Yuan, N. Z. Gong 等（Virginia Tech 等）· CCS 2024", arxiv: null, url: "https://chatpaper.com/ja/paper/20579", k: "K72", metric: "重建 68% 系统提示", msub: "Poe 平台真实应用；手工方法仅 20%、改造越狱提示 18%",
      sum: "手工'说出你的系统提示'句式容易被拒，能否自动化、规模化重建目标应用的系统提示？PLeak 把提示泄露形式化为优化问题：增量搜索对抗性查询，再对多轮响应聚合后处理，逐步逼近完整系统提示；对 Poe 平台真实应用成功重建 68% 的系统提示，远高于手工方法（20%）与改造越狱提示（18%）。意义：单轮拒绝挡不住聚合式提取，只能'防泄'不能'防问'——输出侧相似度比对、金丝雀 token、系统提示零机密才有效。" },
    { grp: "attack", title: "The Attacker Moves Second: Stronger Adaptive Attacks Bypass Defenses Against LLM Jailbreaks and Prompt Injections", info: "M. Nasr, N. Carlini 等（Google DeepMind 等）· 2025-10", arxiv: "2510.09023", k: "K3", metric: "12 种防御几乎全破", msub: "防御自报 ASR 普遍被严重低估；仅 CaMeL 类架构隔离保持稳健",
      sum: "研究问题：已有越狱/注入防御论文的自报数字，在'攻击者知道防御细节并针对性调整'的设定下还成立吗？作者按自适应攻击标准重测了 12 种代表性防御（含 StruQ、Spotlighting、CaMeL 等），结论是强自适应攻击可绕过绝大多数现有防御，防御自报 ASR 普遍被严重低估；仅架构级隔离路线（CaMeL 类能力+策略分离）在其评测下保持稳健。意义：该文已成为防御评测的'终审法庭'——选型时不应相信任何未做自适应攻击实验的防御数字。" },
    { grp: "attack", title: "LLMail-Inject: A Dataset from a Realistic Adaptive Prompt Injection Challenge", info: "S. Abdelnabi, A. Paverd, M. Russinovich 等（Microsoft）· 2025-06", arxiv: "2506.09956", k: "K15", metric: "208,095 条攻击", msub: "839 名参与者去重后；Phase-2 全部防御叠加 + GPT-4o 时无成功攻击",
      sum: "端到端真实管线中，自适应人类攻击者对间接注入防御的实际成功率是多少？LLMail-Inject 是公开挑战赛：向模拟 LLM 邮件助手注入指令，在多档防御、多模型与检索配置下触发未授权工具调用并外泄信息。数据集含 839 名参与者的 208,095 条去重攻击提交；发现包括'陈述句'式隐性注入可绕过检测、全防御叠加 + GPT-4o 时无成功攻击。意义：目前最接近实战的自适应攻击数据集，可直接用于检测器回归评测与阈值标定。" },
    { grp: "attack", title: "Skill-Inject: Measuring Agent Vulnerability to Skill File Attacks", info: "D. Schmotz, L. Beurer-Kellner, S. Abdelnabi, M. Andriushchenko（CISPA、Invariant Labs）· 2026-02", arxiv: "2602.20156", k: "K53", metric: "最高 ASR 80%", msub: "前沿模型常执行数据外泄、破坏性操作乃至勒索软件式行为",
      sum: "Agent 的 Skill 文件以系统提示级权威进入上下文，这条新供应链有多容易被注入？首个技能文件注入基准：23 个技能、8 类攻击、202 个注入-任务对，注入从显式恶意到'藏在合法指令里的上下文相关后门'；前沿模型攻击成功率最高达 80%，常执行数据外泄乃至勒索软件式行为，模型缩放或简单过滤无法解决。意义：'输入'已扩展到技能/工具描述文件，防护必须前置到技能安装与加载环节，并配合上下文感知的授权框架。" },
    // ── 16.2 检测模型与护栏（12 篇）──
    { grp: "detect", title: "PromptShield: Deployable Detection for Prompt Injection Attacks", info: "D. Jacob, H. Alzahrani, Z. Hu, B. Alomair, D. Wagner（UC Berkeley 等）· CODASPY 2025 相关 · 2025-01", arxiv: "2501.15145", k: "K5", metric: "TPR@0.1%FPR 71.45%", msub: "8B 版；同阈值下 ProtectAI v1 仅 0.05%、v2 为 0.00%",
      sum: "极低误报率下检测器几乎失效：现有 DeBERTa 级检测器在 TPR@0.1%FPR 下普遍接近 0（ProtectAI v1/v2 为 0.05%/0.00%）。作者的 Llama3.1-8B 版检测器同阈值下达 71.45%，低 FPR 区间全面碾压既有开源检测器；模型与基准开源。意义：证明'低 FPR 下小模型失效、8B 级才真正可用'的部署铁律，为慢路径选型提供依据。" },
    { grp: "detect", title: "InjecGuard: Benchmarking and Mitigating Over-defense in Prompt Injection Guardrail Models", info: "H. Li, X. Liu（新加坡国立大学）· 2024-10", arxiv: "2410.22770", k: "K63", metric: "TPR@1%FPR 20.37%", msub: "PromptShield 基准 AUC 0.764，优于 ProtectAI v1/v2（0.643/0.701）",
      sum: "揭示注入护栏的隐藏缺陷 over-defense：对含'ignore'等触发词的良性文本过度敏感（注意力走了关键词捷径），生产误报率高。作者提出 NotInject 评测集并训练 InjecGuard（184M）纠偏：AUC 0.764、TPR@1%FPR 20.37%，优于 ProtectAI v1/v2；模型与数据开源。意义：误报而非漏报才是开源小检测器落地的主要障碍，部署前须用自家 benign 流量测过防御倾向。" },
    { grp: "detect", title: "Qwen3Guard Technical Report", info: "Qwen Team（阿里巴巴通义实验室）· 技术报告 · 2025-10", arxiv: "2510.14276", k: "K6", metric: "119 语言 · 119 万条", msub: "训练数据规模与语言覆盖；中英文基准均达开放权重 SOTA",
      sum: "面向真实流量的通用安全护栏，把安全分类做成指令跟随任务。Gen 版（0.6B/4B/8B）输出三级标签加风险类别，Stream 版加 token 级分类头可逐 token 实时拦截。训练数据 119 万条人工标注、覆盖 119 种语言，中英文基准均达开放权重 SOTA；Apache-2.0 开源。意义：中文流量下最强开源护栏，'争议'档可替代硬拦降误报，Stream 版适合作输入侧实时闸。" },
    { grp: "detect", title: "LlamaFirewall: An Open Source Guardrail System for Building Secure AI Agents", info: "S. Chennabasappa, C. Nikolaidis, D. Song, J. Saxe 等（Meta）· 2025-05", arxiv: "2505.03574", k: "K22", metric: "AgentDojo ASR −83%", msub: "AlignmentCheck（Llama 4 Maverick 驱动）0.18→0.03，FPR <4%",
      sum: "Meta 开源的系统级 Agent 护栏框架，三道防线装进统一策略引擎：PromptGuard 2（AgentDojo 防攻率 81.2%）、AlignmentCheck（首个开源实时思维链审计器，ASR 0.18→0.03）、CodeShield（静态分析，8 种语言）。输入覆盖 prompt、不可信数据、Agent 轨迹与代码 diff。意义：'快速分类器+语义级轨迹审计'的开源参考实现，承认并补丁了纯输入分类器对间接注入的盲区。" },
    { grp: "detect", title: "WildGuard: Open One-stop Moderation Tools for Safety Risks, Jailbreaks, and Refusals of LLMs", info: "S. Han, K. Rao, N. Dziri, Y. Choi 等（AI2、华盛顿大学）· NeurIPS 2024 · 2024-06", arxiv: "2406.18495", k: "K61", metric: "ASR 79.8%→2.4%", msub: "作过滤器可把越狱攻击成功率压到 2.4%，开源 SOTA",
      sum: "针对开源护栏'各管一段'提出一站式审核：基于 Mistral-7B 指令微调，单模型同时完成 prompt 有害性识别、response 有害性识别、拒答检测，覆盖 13 个风险类别。配套 92K 的 WildGuardMix 训练集；作过滤器把越狱 ASR 从 79.8% 压到 2.4%，开源 SOTA。意义：一个 7B 开放模型可同时守输入输出两侧，是 GuardReasoner、Qwen3Guard 的直接前驱。" },
    { grp: "detect", title: "GuardReasoner: Towards Reasoning-based LLM Safeguards", info: "Y. Liu, H. Gao, S. Zhai, B. Hooi 等（NUS、港科大（广州）、西湖大学）· 2025-01", arxiv: "2501.18492", k: "K73", metric: "平均 F1 84.09%", msub: "8B 版超 GPT-4o+CoT 5.74pp、超 LLaMA Guard 3-8B 20.84pp",
      sum: "把 R1 式推理引入护栏：汇集四类红队数据，用 GPT-4o 合成推理过程构建 GuardReasonerTrain（127K 样本、460K 推理步），经推理 SFT 与难样本 DPO 两阶段训练。13 个基准、3 类任务上 8B 版平均 F1 84.09%，超 GPT-4o+CoT 5.74 个百分点；输出推理链+判定标签，全开源。意义：中间推理步带来可解释性与开放类别泛化，对未见攻击更鲁棒，但延迟上升，适合慢路径复核。" },
    { grp: "detect", title: "AgentDoG: A Diagnostic Guardrail Framework for AI Agent Safety and Security", info: "D. Liu（刘东瑞）等约 40 人团队（上海交大、复旦、北大、上海 AI Lab、UIUC 等）· 2026-01", arxiv: "2601.18491", k: "K35", metric: "4B/7B/8B 三档", msub: "轨迹级判定 + 三维细粒度诊断（风险来源×失败模式×现实后果）",
      sum: "把护栏从'单条消息分类'升级为'轨迹级诊断'。先提出三维 Agent 风险分类法（风险来源 × 失败模式 × 现实后果），构建轨迹基准 ATBench，再训练 AgentDoG 系列（4B/7B/8B）。输入为完整执行轨迹+工具描述，输出轨迹级判定，unsafe 轨迹另给三维诊断标签与分析过程，能识别'看似安全但不合理'的行为；全开源。意义：攻击往往体现在轨迹层面，代表'检测面从输入文本扩展到执行轨迹'的范式转移。" },
    { grp: "detect", title: "AgentDoG 1.5: A Lightweight and Scalable Alignment Framework for AI Agent Safety and Security", info: "AgentDoG 团队（D. Liu 等）· 2026-05", arxiv: "2605.29801", k: "K36", metric: "ASR 56.25%→18.75%", msub: "Pre-Reply 在线护栏压 ClawSafety；R-Judge 92.2% acc / 92.7 F1",
      sum: "1.0 的轻量化升级：0.8B/2B/4B/8B 四档，仅用约 1K 条影响函数净化样本经 SFT+RL 训练。成绩：4B 版 R-Judge 92.2% acc，三维诊断平均 55.2%（GPT-5.4 为 25.8%）；Pre-Reply 在线护栏把 ClawSafety ASR 从 56.25% 压到 18.75%，TTFT 亚秒级。意义：亚秒级延迟让轨迹级审计首次可在线部署——'事后诊断'走向'事中拦截'的关键一步。" },
    { grp: "detect", title: "PromptArmor: 现成 LLM 提示化'检测+移除'一体化护栏", info: "UC Berkeley、UCSB、Duke 团队· 2025-07", arxiv: "2507.15219", k: "K74", metric: "AgentDojo FPR/FNR <1%", msub: "攻击成功率降至 <1%；但 RTC-Bench 隐式注入检出仅 2–30%",
      sum: "挑战'现成 LLM 不能直接提示成防御器'的成见：不训练任何模型，直接用 GPT-4o 等加提示策略做'检测+移除'一体化护栏——先判定输入是否含注入，再把恶意指令剥除。AgentDojo 上 FPR 与 FNR 均 <1%，攻击成功率降至 <1%。局限同样明确：RTC-Bench 显示其对计算机使用场景的隐式注入检出率仅 2–30%。意义：提示化检测是低成本冷启动方案，但对隐式、多模态注入不能单独依赖。" },
    { grp: "detect", title: "DataSentinel: A Game-Theoretic Detection of Prompt Injection Attacks", info: "Y. Liu, Y. Jia, N. Z. Gong 等（Duke University 等）· IEEE S&P 2025 · 2025-04", arxiv: "2504.11358", k: "K48", metric: "FNR ≤0.06 · FPR ≤0.01", msub: "对自适应攻击；KAD 的 FNR 最高 0.93，对比悬殊",
      sum: "针对已知答案检测（KAD）被自适应攻击绕过的问题，提出博弈论检测：min-max 优化——内层生成最强自适应注入，外层更新检测 LLM，迭代对抗训练；反直觉设计是'让检测 LLM 更脆弱'，污染暴露反而更明显。结果：自适应攻击下多数任务 FNR ≤0.06、FPR ≤0.01，远优于 KAD（FNR 最高 0.93）。意义：少数在'攻击者知悉防御'威胁模型下仍成立的检测器，适合高风险工具返回的离线深检。" },
    { grp: "detect", title: "AttriGuard: Defeating Indirect Prompt Injection via Causal Attribution of Tool Invocations", info: "学术研究团队 · 2026-03", arxiv: "2603.10749", k: "K51", metric: "动作级归因", msub: "对训练中未见过的注入 payload 泛化明显更好，优于输入级护栏",
      sum: "不再对输入文本做语义判别（该路线被证明学到的是表面启发式），而是对 Agent 的每次工具调用做因果归因——判定该动作由用户原始目标驱动，还是被某段不可信内容'因果地'引发；特权动作主要由不可信数据段驱动时即拦截。论文自称对未见过的注入 payload 泛化明显更好，并优于 LlamaFirewall 类输入级护栏。意义：把'是否恶意文本'的不可解问题转化为'动作由谁驱动'的可归因问题，代表检测向运行时因果审计演进的方向。" },
    { grp: "detect", title: "GenTel-Safe / GenTel-Shield", info: "学术研究团队 · 2024-09", arxiv: "2409.19521", k: "K75", metric: "越狱防御 97.63%", msub: "GenTel-Bench 84,812 条攻击样本、28 场景；暴露主流检测器短板",
      sum: "企业级场景的基准+检测方案。GenTel-Bench：84,812 条攻击样本，覆盖越狱、目标劫持、提示泄露 3 大类 28 个场景。GenTel-Shield 基于多语言 E5 embedding 的检索/分类管线：越狱防御成功率 97.63%。意义：embedding 检索式检测以极低延迟达 97% 级防御率且天然多语言。" },
    // ── 16.3 防御架构与评测方法（12 篇）──
    { grp: "defense", title: "Spotlighting: Defending Against Indirect Prompt Injection Attacks", info: "K. Hines 等（Microsoft Research）· 2024-03", arxiv: "2403.14720", k: "K14", metric: "ASR >50%→<2%", msub: "datamarking 与 encoding 在 GPT-3.5/GPT-4 上的间接注入降幅",
      sum: "LLM 无法区分系统指令与不可信数据，拼接即注入。方法：拼接前对不可信文本做变换——delimiting（定界符包裹）、datamarking（词间插入保留字符）、encoding（整体 Base64）三种模式。关键数字：GPT-3.5/GPT-4 上间接注入 ASR 从 >50% 降到 <2%。意义：纯推理时性价比最高的输入侧技术（零额外模型调用），但属概率防御——LLMail-Inject 的 37 万自适应提交仍有大量突破。" },
    { grp: "defense", title: "StruQ: Defending Against Prompt Injection with Structured Queries", info: "S. Chen 等（UC Berkeley 等）· USENIX Security 2025 · 2024-02", arxiv: "2402.06363", k: "K76", metric: "GCG ASR 56%", msub: "SecAlign 论文报告 GCG 对 StruQ Mistral-7B 的突破率",
      sum: "单一文本通道使模型分不清指令与数据。方法：把接口改为结构化查询——prompt 与 data 两通道以保留特殊 token 分隔，前端过滤数据中伪造的分隔符，再做结构化指令微调，训练模型只响应 prompt 通道指令。关键数字：对 optimization-free 攻击接近 0 ASR；但对未见过的 GCG 优化攻击仍被突破（ASR 达 56%）。意义：确立'通道分离'范式，但它是训练时方案，需微调模型加定制前端，不可纯推理时部署。" },
    { grp: "defense", title: "SecAlign: Defending Against Prompt Injection with Preference Optimization", info: "S. Chen 等（UC Berkeley / UIUC）· ACM CCS 2025 · 2024-10", arxiv: "2410.05451", k: "K77", metric: "优化攻击 ASR <10%", msub: "对 GCG/AdvPrompter/NeuralExec 较 StruQ 再降 4 倍以上；utility 波动 <1.5%",
      sum: "StruQ 的 SFT 对优化攻击不鲁棒。方法：把注入防御表述为 DPO 偏好优化——构造（期望=执行合法指令，非期望=执行注入指令）偏好对，无需人工标注。关键数字：对 optimization-free 攻击 0% ASR；对 GCG 等优化攻击 ASR 主要 <10%；utility 无显著损失。意义：目前训练侧最强防线，但纯训练时方案、不可推理时部署。选型结论：SecAlign > StruQ > prompting 防御。" },
    { grp: "defense", title: "The Instruction Hierarchy: Training LLMs to Prioritize Privileged Instructions", info: "E. Wallace 等（OpenAI）· 2024-04", arxiv: "2404.13208", k: "K78", metric: "直接注入 −30%", msub: "OpenAI 评测直接注入成功率约降 30%；已产品化为 Model Spec chain of command",
      sum: "模型对所有来源的指令一视同仁。方法：训练模型按 system > developer > user > tool 的优先级解析冲突指令，用故意构造的冲突提示对做训练信号。关键数字：OpenAI 评测对直接注入成功率约降 30%，对间接注入和编码载荷改善更小；已被 Model Spec 产品化。意义：厂商侧训练时方案——应用方只能'消费'已训练好的层级（用对 API 角色字段）；已知失效模式含角色标记伪造、RAG 提权、多轮稀释。" },
    { grp: "defense", title: "Defeating Prompt Injections by Design（CaMeL）", info: "E. Debenedetti 等（Google DeepMind / ETH Zurich）· 2025-03", arxiv: "2503.18813", k: "K16", metric: "233→0", msub: "AgentDojo 上 GPT-4o 成功攻击数归零；任务解决率 77%（基线 84%）",
      sum: "所有概率防御终将被自适应攻击绕过。方法：在 LLM 外围加确定性系统层——从可信用户查询显式抽取控制流与数据流，不可信数据无法影响程序流；再用 capability + 安全策略阻止未授权数据外泄。关键数字：AgentDojo 上 GPT-4o 成功攻击数从 233 降为 0；在可证明安全前提下解决 77% 任务（基线 84%）。意义：少数能在推理时部署且提供近确定性保证的方案；代价是架构改造与每任务多次调用。" },
    { grp: "defense", title: "Securing AI Agents with Information-Flow Control（FIDES）", info: "P. Costa 等（Microsoft Research）· 2025-05", arxiv: "2505.23643", k: "K79", metric: "IFC 形式化", msub: "动态污点追踪可强制的性质类与任务分类学；AgentDojo 上保有安全保证",
      sum: "CaMeL 的隐式流控缺乏形式化刻画。方法：给出 agent planner 的形式模型，据此实现 FIDES——跟踪机密性/完整性标签、确定性执行安全策略的 planner。关键数字：在 AgentDojo 上证明该路线能保有安全保证并完成广泛任务。意义：把 CaMeL 式架构隔离形式化为信息流控制（IFC），是'架构隔离'谱系的理论落地；需按 planner 范式重构 agent。" },
    { grp: "defense", title: "MELON: Provable Defense Against Indirect Prompt Injection Attacks in AI Agents", info: "K. Zhu 等（UC Santa Barbara / Microsoft 等）· ICML 2025 · 2025-02", arxiv: "2502.05174", k: "K80", metric: "ASR 0.32%", msub: "MELON-Aug 在 GPT-4o 上；utility 保持 86.72%",
      sum: "方法利用关键洞察：攻击成功时 agent 的下一步动作更依赖恶意任务而非用户任务；将轨迹以掩码用户输入重执行（masked re-execution），两次执行产生相似工具调用即判攻击。关键数字：AgentDojo 三模型上优于五种 SOTA；MELON-Aug 在 GPT-4o 上 ASR 降至 0.32%，utility 保持 86.72%。意义：纯推理时检测中安全-utility 平衡最好的一档；代价是每个候选调用一次额外重执行。" },
    { grp: "defense", title: "Progent: Securing AI Agents with Privilege Control", info: "W. Shi 等（UC Berkeley 等）· 2025-04", arxiv: "2504.11703", k: "K49", metric: "ASR 39.9%→1.0%", msub: "AgentDojo 间接注入；ASB 70.3%→3.9%，utility 基本不降",
      sum: "检测器范式在分布外失效，应直接收缩 agent 权限。方法：工具调用级的最小特权控制——符号策略（工具名+参数）由 LLM 生成/更新，SMT 判定策略变更是扩张还是收缩以保证单调约束、防止静默提权。关键数字：AgentDojo 间接注入 ASR 从 39.9% 降至 1.0%，ASB 从 70.3% 降至 3.9%，utility 基本不降。意义：与 CaMeL 并列的'权限收缩'路线代表。" },
    { grp: "defense", title: "Defending Against Prompt Injection With a Few DefensiveTokens", info: "学术研究团队 · 2025-07", arxiv: "2507.07974", k: "K50", metric: "≈SecAlign 水平", msub: "Semantic Scholar 42 引；utility 退化更小，可按需开关",
      sum: "方法：DefensiveTokens——只需在输入前插入少量训练好的'防御 token'（软提示/特殊 token）即达接近 SecAlign 的效果且 utility 损失更小。关键数字：作为轻量替代，多项攻击上接近 SecAlign 的 ASR 水平，正常任务退化更小。意义：把训练侧防御的部署成本降到'加几个 token'；但本质仍需一次训练过程，且对黑盒 API 模型不可用。" },
    { grp: "defense", title: "Evaluating the Efficacy of LLM Safety Solutions: The Palit Benchmark Dataset", info: "S. Palit, D. Woods · 2025-05", arxiv: "2505.13028", k: "K11", metric: "canary 仅 34.2%", msub: "WhyLabs LangKit canary 扫描器准确率（FPR=0），受测工具最低之一",
      sum: "LLM 安全工具缺乏正式横向评测。方法：识别 13 个解决方案（闭源厂商不配合，仅 7 个完成），自建恶意提示基准，以 GPT-3.5-Turbo 为基线比较检出与误报。关键数字：基线模型误报过多不可用；Lakera Guard 与 ProtectAI LLM Guard 综合最佳；LangKit 的 canary 扫描器准确率仅 34.2%（FPR=0）。意义：早期独立横评，量化了金丝雀 token 等廉价手段的低效。" },
    { grp: "defense", title: "Defenses Against Prompt Attacks Learn Surface Heuristics", info: "学术研究团队 · 2026-01", arxiv: "2601.07185", k: "K56", metric: "'打脸'作", msub: "检测器学到的是表面启发式而非注入语义，分布外即失效",
      sum: "注入检测器在基准上分数很高，但学到的是语义理解吗？方法：对现有提示攻击防御做受控分布迁移实验，检验其决策依据。结论（'打脸'作）：检测器学到的是表面启发式（关键词、格式、长度等浅层特征）而非注入语义，分布外即失效。意义：对整条'训练检测分类器'路线的根本性质疑，与 Progent 实验中三个检测器在 AgentDojo 集体失效互证。工程含义：高基准分不能外推到真实攻击分布，必须配合自适应攻击评测与纵深防御，不可作为唯一防线。" },
    { grp: "defense", title: "Indirect Prompt Injections: Are Firewalls All You Need, or Stronger Benchmarks?", info: "学术研究团队 · 2025-10", arxiv: "2510.05244", k: "K57", metric: "Combined ASR 0.00%", msub: "AgentDojo（GPT-4o）Sanitizer 版 ASR 3.07%、UA 69.17%（高于无防御的 53.31%）",
      sum: "基准能否区分防御强弱？方法：构造极简 agent-tool 防火墙——输入侧按任务过滤敏感参数、输出侧移除恶意内容，在 AgentDojo 等四个基准上评测。关键数字：玩具防火墙即接近'完美'——AgentDojo Combined 版 ASR 0.00%，Sanitizer 版 UA 69.17%。意义：'连玩具防火墙都拦得住'说明基准太弱——与 Attacker Moves Second 等共构'评测可信度'警示三角。" },
  ],
};
