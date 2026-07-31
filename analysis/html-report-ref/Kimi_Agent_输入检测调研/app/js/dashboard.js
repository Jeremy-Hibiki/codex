// 右栏常驻仪表盘（P14）：随章节切换的安全闸门情境面板
(() => {
  const cv = document.getElementById("dash-canvas");
  const rail = document.getElementById("dash-rail");
  if (!cv || !rail) return;
  const PAL = U.PAL, TAU = U.TAU, clamp = U.clamp;
  const SERIF = '"et-book", Palatino, Georgia, serif';
  const MONO = 'Menlo, Consolas, monospace';
  let W = 0, H = 0, ctx;
  const REDUCE = matchMedia("(prefers-reduced-motion: reduce)").matches;

  const WINS = {
    exec:      { no: "§0", title: "摘要 · 六形态方案地图", seg: -1, stats: [
      { k: "PINT 榜首（自家基准）", v: "95.22%", s: "Lakera Guard；利益冲突需打折 [K1]" },
      { k: "AgentDojo 最强小模型", v: "81.2%", s: "Prompt Guard 2 86M [K4]" },
      { k: "流量终结于前两层", v: "95%+", s: "级联成本模型 [K21]" },
      { k: "自适应攻击绕过", v: ">90%", s: "12 种防御全部失守 [K3]" }] },
    threat:    { no: "§1", title: "威胁模型 · 攻击族", seg: -1, stats: [
      { k: "OWASP LLM Top 1", v: "LLM01", s: "Prompt Injection；RAG/微调不能完全缓解" },
      { k: "DAN 类总成功率", v: "31.8%", s: "HackAPrompt 挑战数据 [K27]" },
      { k: "Crescendo 对 GPT-4", v: "98%", s: "多轮渐进，单轮完全良性 [K13]" },
      { k: "emoji 混淆绕过", v: "100%", s: "6 个商用护栏实测 [K2]" }] },
    tools:     { no: "§2", title: "三方案详解", seg: 2, stats: [
      { k: "Lakera 第三方 recall", v: "0.5–0.7", s: "高精确低误报取向，3–5 成漏检 [K11]" },
      { k: "NeMo self-check 延迟", v: "≈1.5s", s: "误报 16.22% [K8]" },
      { k: "Rebuff 误报 / 延迟", v: "0.68 / 29s", s: "已归档，生产不可用 [K10]" },
      { k: "Check Point 收购 Lakera", v: "$3 亿", s: "2025-09，路线图不确定 [K25]" }] },
    landscape: { no: "§3", title: "方案全景 · PINT 榜", seg: 1, stats: [
      { k: "Lakera / AWS / Azure", v: "95.2/89.2/89.1", s: "PINT 官方榜 [K1]" },
      { k: "TPR@0.1%FPR 小 vs 8B", v: "0–12% vs 71%", s: "规模是硬门槛 [K5]" },
      { k: "Qwen3Guard 语言数", v: "119", s: "中英文 SOTA，Apache-2.0 [K6]" },
      { k: "LoRA 微调成本", v: "$1–2/轮", s: "recall 0.53→0.985（RAPIDS）" }] },
    nonmodel:  { no: "§4", title: "非模型技术", seg: 0, stats: [
      { k: "归一化层成本", v: "<1ms", s: "缺它下游全失效 [K2]" },
      { k: "Spotlighting 降 ASR", v: ">50%→<2%", s: "GPT 系列间接注入 [K14]" },
      { k: "CaMeL 攻击成功数", v: "233→0", s: "AgentDojo；utility −7pp [K16]" },
      { k: "canary 默认检测率", v: "0%", s: "加显式指令后部分有效 [K17]" }] },
    bench:     { no: "§5", title: "基准与评估", seg: -1, stats: [
      { k: "PINT 规模", v: "4,314 条", s: "含 1,298 条非英文子集 [K1]" },
      { k: "LLMail-Inject 攻击", v: "208,095", s: "839 名参与者去重后 [K15]" },
      { k: "端到端成功率", v: "0.8%/0.3%", s: "首次成功需数百次尝试 [K15]" },
      { k: "中文数据集", v: "基本空白", s: "三条腿：PINT/CyberSecEval/自建" }] },
    eng:       { no: "§6", title: "工程落地 · 级联", seg: 1, stats: [
      { k: "规则层延迟", v: "<1ms", s: "Aho-Corasick <0.1–0.2ms [K21]" },
      { k: "小分类器延迟", v: "20–100ms", s: "PG2-22M 19ms / 86M 20–92ms [K4]" },
      { k: "judge 延迟", v: "1.2–1.5s", s: "每方向一次 LLM 调用 [K8]" },
      { k: "T4/A10G 容量", v: "数百 req/s", s: "86M + INT4 8B 共池" }] },
    frontier:  { no: "§7", title: "前沿趋势", seg: 2, stats: [
      { k: "防御被自适应绕过", v: ">90%", s: "12 种检测/训练防御 [K3]" },
      { k: "系统提示泄露率", v: ">80%", s: "1200 应用实测 [K23]" },
      { k: "FreoStream 降 ASR", v: "17.9→7.9%", s: "Qwen3Guard-Stream-8B [K32]" },
      { k: "XGuard 动态政策 F1", v: "0.91", s: "超 gpt-oss-safeguard-20B [K31]" }] },
    gptoss:    { no: "§8", title: "政策推理护栏", seg: 2, stats: [
      { k: "gpt-oss 上下文", v: "131K", s: "政策+内容进，判定+理由出 [K33]" },
      { k: "ToxicChat 开源榜首", v: "F1 79.9%", s: "20b，独立榜 2026-03 [K37]" },
      { k: "定位", v: "慢路径深检", s: "秒级 CoT；官方自认注入基准弱于基座 [K33]" },
      { k: "ShieldGemma", v: "排除", s: "词表无注入类、仅英语 [K34]" }] },
    agentdog:  { no: "§9", title: "AgentDoG · SingGuard", seg: -1, stats: [
      { k: "R-Judge（1.5-4B）", v: "92.2%", s: "acc；F1 92.7 [K36]" },
      { k: "ClawSafety ASR", v: "56%→19%", s: "Pre-Reply 在线护栏 [K36]" },
      { k: "NSFA 判别头延迟", v: "45–57ms", s: "F1>94%（自建基准口径）[K84]" },
      { k: "SingGuard 35 数据集", v: "F1 第一", s: "政策自适应多模态护栏 [K83]" }] },
    iospec:    { no: "§10", title: "I/O 六形态", seg: 1, stats: [
      { k: "I/O 形态", v: "A–F", s: "encoder/托管/生成/政策/流式/轨迹" },
      { k: "对照表方案数", v: "20", s: "含框架与编排器" },
      { k: "F 轨迹审计", v: "唯一", s: "覆盖间接注入/目标劫持 [K22]" },
      { k: "Azure 输出", v: "纯布尔", s: "无分数，灰度只能先 log 不拦" }] },
    multiturn: { no: "§11", title: "多轮防护", seg: -1, stats: [
      { k: "Crescendo 对 GPT-4", v: "98%", s: "单轮完全良性 [K13]" },
      { k: "Defensive M2S 召回", v: "93.8%", s: "压缩到 173 token [K38]" },
      { k: "X-Teaming ASR", v: "98.1%", s: "HarmBench [K39]" },
      { k: "会话级护栏对自适应", v: ">90% ASR", s: "SoK 实测，诚实边界" }] },
    ctxwall:   { no: "§12", title: "512 上下文墙", seg: 1, stats: [
      { k: "encoder 上限", v: "512 tok", s: "≈250–500 汉字 [K43]" },
      { k: "生成式护栏上限", v: "32K–164K", s: "Qwen3Guard/LG3/LG4/gpt-oss" },
      { k: "下游 LLM 窗口", v: "400K 级", s: "缝隙已被武器化 [K43]" },
      { k: "解法", v: "6 种", s: "按输入类型分流 [K44][K47]" }] },
    lineage:   { no: "§13", title: "学术脉络 · 引用网络", seg: -1, stats: [
      { k: "核心论文谱系", v: "10 组", s: "双向引用网络，21 次独立检索" },
      { k: "扩展论文", v: "13 篇", s: "与前述章节引用不重复 [K48]–[K60]" },
      { k: "DataSentinel 被引", v: "137", s: "检测侧对训练侧路线的最强回应 [K48]" },
      { k: "打脸作", v: "2 篇", s: "表面启发式 + 基准太弱 [K56][K57]" }] },
    papers:    { no: "§14", title: "论文精读 · 35 篇", seg: -1, stats: [
      { k: "攻击 / 检测 / 防御", v: "11/12/12", s: "三组攻防主线组织 [K12][K48][K16]" },
      { k: "自适应攻击重测", v: ">90%", s: "12 种防御几乎团灭 [K3]" },
      { k: "CaMeL 攻击成功数", v: "233→0", s: "架构隔离唯一幸存 [K16]" },
      { k: "Skill-Inject 最高 ASR", v: "80%", s: "技能文件成新供应链面 [K53]" }] },
    traindata: { no: "§15", title: "训练数据与微调", seg: 1, stats: [
      { k: "Qwen3Guard 训练数据", v: "119 万", s: "中文占 26.6%，公开护栏最高 [K6]" },
      { k: "WildGuard / Aegis 2.0", v: "92K / 34K", s: "CC-BY-4.0 / 可商用 [K61][K65]" },
      { k: "AgentDoG 1.5 净化样本", v: "~1k", s: "影响函数筛选，质量>数量 [K36]" },
      { k: "LoRA 微调成本", v: "$1–2/run", s: "r=8 训 2.3M 参数，单卡 1 小时" }] },
    roadmap:   { no: "§16", title: "选型路线图", seg: 3, stats: [
      { k: "落地周期", v: "4 周", s: "影子→校准→阻断→闭环" },
      { k: "中文 L1 首选", v: "Qwen3Guard-0.6B", s: "Stream 版，Apache-2.0 [K6]" },
      { k: "升级率预算", v: "1–5%", s: "judge 层按升级率计费" },
      { k: "架构兜底三件套", v: "权限/HITL/零机密", s: "OWASP LLM07 [K24]" }] },
    appendix:  { no: "§17", title: "附录 · 冲突口径", seg: -1, stats: [
      { k: "冲突口径项", v: "6 项", s: "全部并列呈现" },
      { k: "高置信结论", v: "9 条", s: "≥2 独立来源一致" },
      { k: "登记来源", v: "K1–K86", s: "五类分组，均带日期" },
      { k: "调研维度", v: "10 维", s: "每维 ≥15 次检索" }] },
  };

  const SEGS = ["L0 规则", "L1 分类器", "L2 judge", "L3 输出侧"];
  let cur = "exec";

  function draw() {
    ctx.clearRect(0, 0, W, H);
    const w = WINS[cur];
    const pad = 34;
    let y = 54;
    // 徽章 + 标题
    ctx.font = `700 12px ${MONO}`; ctx.fillStyle = PAL.red;
    ctx.fillText(w.no, pad, y);
    ctx.font = `700 20px ${SERIF}`; ctx.fillStyle = PAL.ink;
    ctx.fillText(w.title, pad + 44, y + 2);
    y += 22;
    ctx.strokeStyle = PAL.line; ctx.lineWidth = 1;
    ctx.beginPath(); ctx.moveTo(pad, y); ctx.lineTo(W - pad, y); ctx.stroke();
    y += 34;
    // 管线相位条（五段：L0-L3 + 当前高亮）
    ctx.font = `10px ${MONO}`;
    const segW = (W - pad * 2) / 4;
    SEGS.forEach((s, i) => {
      const x = pad + i * segW;
      const on = i === w.seg;
      ctx.fillStyle = on ? PAL.red : PAL.lineLo;
      ctx.fillRect(x, y, segW - 6, on ? 5 : 2.5);
      ctx.fillStyle = on ? PAL.red : PAL.inkLo;
      ctx.fillText(s, x, y + 20);
    });
    y += 44;
    // 迷你闸机图示（站点标记：当前段落闪烁）
    const gx = pad + 30, gy = y + 46;
    ctx.save();
    ctx.strokeStyle = PAL.inkMd; ctx.lineWidth = 1.4;
    for (let i = 0; i < 4; i++) {
      const x = gx + i * ((W - pad * 2 - 60) / 3);
      // 简化闸机：双柱+顶舱
      ctx.fillStyle = i === w.seg ? PAL.red : "#42566a";
      ctx.fillRect(x - 10, gy - 26, 5, 26);
      ctx.fillRect(x + 5, gy - 26, 5, 26);
      ctx.fillRect(x - 14, gy - 36, 28, 10);
      if (i < 3) {
        ctx.strokeStyle = PAL.line;
        ctx.beginPath(); ctx.moveTo(x + 18, gy - 12); ctx.lineTo(x + (W - pad * 2 - 60) / 3 - 18, gy - 12); ctx.stroke();
        ctx.strokeStyle = PAL.inkMd;
      }
    }
    ctx.restore();
    y = gy + 30;
    // 统计块
    ctx.font = `10px ${MONO}`; ctx.fillStyle = PAL.inkLo;
    ctx.fillText("本段关键读数 · 点击钻取", pad, y); y += 14;
    dashStats = [];
    w.stats.forEach((s, i) => {
      const sy = y + i * 86;
      ctx.strokeStyle = PAL.lineLo;
      ctx.beginPath(); ctx.moveTo(pad, sy); ctx.lineTo(W - pad, sy); ctx.stroke();
      ctx.font = `700 21px ${MONO}`; ctx.fillStyle = PAL.red;
      ctx.fillText(s.v, pad, sy + 30);
      ctx.font = `12.5px ${SERIF}`; ctx.fillStyle = PAL.ink;
      ctx.fillText(s.k, pad, sy + 50);
      ctx.font = `10px ${MONO}`; ctx.fillStyle = PAL.inkLo;
      ctx.fillText(s.s.slice(0, 42), pad, sy + 68);
      dashStats.push({ x: pad, y: sy, w: W - pad * 2, h: 84, d: s });
    });
    // 底部：延迟对数尺
    const ly = H - 84;
    ctx.strokeStyle = PAL.line; ctx.beginPath(); ctx.moveTo(pad, ly); ctx.lineTo(W - pad, ly); ctx.stroke();
    ctx.font = `9px ${MONO}`; ctx.fillStyle = PAL.inkLo;
    ctx.fillText("延迟分层（对数）", pad, ly - 10);
    const lMin = Math.log10(0.05), lMax = Math.log10(3000);
    const LX = ms => pad + (Math.log10(ms) - lMin) / (lMax - lMin) * (W - pad * 2);
    [[0.5, "L0 <1ms", "#42566a"], [50, "L1 20–100ms", PAL.red], [1350, "L2 秒级", PAL.redHi]].forEach(([ms, lab, c]) => {
      ctx.fillStyle = c;
      ctx.beginPath(); ctx.arc(LX(ms), ly, 4, 0, TAU); ctx.fill();
      ctx.font = `9px ${MONO}`;
      ctx.fillText(lab, clamp(LX(ms) - 26, pad, W - pad - 66), ly + 18);
    });
    ctx.fillStyle = PAL.inkLo;
    ctx.fillText("数据源：js/data.js · 报告原文锚点 K 表", pad, H - 30);
  }

  let dashStats = [];
  function fit() {
    const dpr = Math.min(devicePixelRatio || 1, 2);
    const r = cv.getBoundingClientRect();
    cv.width = r.width * dpr; cv.height = r.height * dpr;
    ctx = cv.getContext("2d");
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    W = r.width; H = r.height;
  }
  cv.addEventListener("click", ev => {
    const r = cv.getBoundingClientRect();
    const x = ev.clientX - r.left, y = ev.clientY - r.top;
    const hit = dashStats.find(s => x >= s.x && x <= s.x + s.w && y >= s.y && y <= s.y + s.h);
    if (hit) U.showDrill({ title: hit.d.k, value: hit.d.v, sub: hit.d.s, source: "右栏情境面板 · 详见正文对应章节", x: ev.clientX, y: ev.clientY });
  });

  // 章节监听（与 era rail 共用 data-win）
  const io = new IntersectionObserver(es => es.forEach(e => {
    if (!e.isIntersecting) return;
    const win = e.target.dataset.win;
    if (win && WINS[win] && win !== cur) { cur = win; draw(); }
    rail.classList.add("on");
  }), { rootMargin: "-30% 0px -55% 0px", threshold: 0 });
  document.querySelectorAll("[data-win]").forEach(s => io.observe(s));
  // 封面时隐藏
  const coverIO = new IntersectionObserver(es => es.forEach(e => {
    if (e.isIntersecting) rail.classList.remove("on");
  }), { threshold: 0.4 });
  coverIO.observe(document.getElementById("cover"));

  function init() { fit(); draw(); }
  addEventListener("resize", () => { fit(); draw(); });
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(init);
  else init();
})();
