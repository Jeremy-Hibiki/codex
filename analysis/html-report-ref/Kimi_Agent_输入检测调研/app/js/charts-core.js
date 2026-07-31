// 核心图表：P17 证据对象 · PINT 榜 · 低 FPR 对比 · AgentDojo · 路线赔率板 · 路线图
// 通用约定：固定 viewBox 880 宽，width:100% 自适应；IO 入场一次；全部数字可钻取。
(() => {
  const PAL = U.PAL, clamp = U.clamp;
  const REDUCE = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const SERIF = '"et-book", Palatino, Georgia, serif';
  const MONO = 'Menlo, Consolas, monospace';
  const NS = "http://www.w3.org/2000/svg";
  const el = (tag, attrs, parent) => {
    const e = document.createElementNS(NS, tag);
    for (const k in attrs) e.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(e);
    return e;
  };
  const txt = (parent, x, y, str, { font = MONO, size = 11, fill = PAL.inkMd, anchor = "start", weight = 400, halo = true, style = "" } = {}) => {
    const t = el("text", { x, y, "font-size": size, "text-anchor": anchor, fill, style: `font-family:${font};font-weight:${weight};${style}` }, parent);
    if (halo) { t.setAttribute("paint-order", "stroke"); t.setAttribute("stroke", "#fff"); t.setAttribute("stroke-width", "4"); }
    t.textContent = str;
    return t;
  };
  function enter(svg, body) {
    if (REDUCE) return;
    svg.querySelectorAll("[data-anim]").forEach(n => {
      n.style.transform = "scaleX(0)"; n.style.transformOrigin = "left center";
      n.style.transition = "transform .9s cubic-bezier(.25,.7,.3,1)";
    });
    const io = new IntersectionObserver(es => es.forEach(e => {
      if (!e.isIntersecting) return;
      io.disconnect();
      svg.querySelectorAll("[data-anim]").forEach((n, i) => setTimeout(() => n.style.transform = "scaleX(1)", i * 90));
    }), { threshold: 0.18 });
    io.observe(body);
  }
  function drillOn(node, data) {
    node.style.cursor = "pointer";
    node.setAttribute("data-drill-keep", "1");
    node.addEventListener("click", ev => U.showDrill({ ...data, x: ev.clientX, y: ev.clientY }));
  }

  // ── P17 证据对象：安检闸门侧视图，关键数字挂在语义部位 ──
  (() => {
    const host = document.getElementById("evidence-chart");
    if (!host) return;
    const body = U.frame(host, {
      title: "六个硬数字，挂在一道安检闸门它该在的位置",
      sub: "读数屏=PINT 检测力 · 闸翼=真实拦截率 · 传送带=延迟分层 · 队列=流量终结点 · 闸下漏网卡=混淆绕过 · 裂缝=自适应攻击 · 点击任一数字钻取",
      src: "报告 §1.4 / §11.1 · 分级来源见 K 表（K1–K5, K16, K21）",
    });
    const W = 880, H = 480;
    const svg = el("svg", { viewBox: `0 0 ${W} ${H}` }, body);
    svg.style.cssText = "width:100%;height:auto;display:block";
    const gy = 380; // 地面线
    // 地面
    el("line", { x1: 30, y1: gy, x2: W - 30, y2: gy, stroke: PAL.ink, "stroke-width": 1.5 }, svg);
    // 传送带（L0→L1→L2 三速区）
    const beltY = gy - 26;
    el("rect", { x: 60, y: beltY, width: 740, height: 10, fill: PAL.hi, stroke: PAL.line }, svg);
    [60, 300, 540].forEach((x) => {
      el("line", { x1: x, y1: beltY - 4, x2: x, y2: beltY + 14, stroke: PAL.inkLo, "stroke-width": 1 }, svg);
    });
    // 带速标注（语义部位：传送带速度表 = 延迟分层）
    const beltG = el("g", {}, svg);
    txt(beltG, 180, beltY + 34, "L0 规则 <1ms", { size: 11, anchor: "middle", fill: PAL.ink });
    txt(beltG, 420, beltY + 34, "L1 小分类器 20–100ms", { size: 11, anchor: "middle", fill: PAL.red, weight: 700 });
    txt(beltG, 660, beltY + 34, "L2 judge 秒级", { size: 11, anchor: "middle", fill: PAL.ink });
    el("path", { d: `M60 ${beltY + 46} L800 ${beltY + 46}`, stroke: PAL.inkLo, "stroke-width": 0.8, "stroke-dasharray": "3 3" }, svg);
    txt(beltG, 430, beltY + 62, "延迟跨越三个数量级 —— 级联是唯一可行形态", { size: 10.5, anchor: "middle", fill: PAL.inkLo, font: SERIF, style: "font-style:italic" });
    // 闸门主体：两门柱 + 顶部扫描舱
    const gx1 = 380, gx2 = 560;
    el("rect", { x: gx1, y: 130, width: 30, height: gy - 156, fill: "#42566a" }, svg);
    el("rect", { x: gx2 - 30, y: 130, width: 30, height: gy - 156, fill: "#42566a" }, svg);
    el("rect", { x: gx1 - 18, y: 92, width: gx2 - gx1 + 36, height: 56, rx: 5, fill: "#051c2c" }, svg);
    // 读数屏（语义部位：扫描舱显示屏 = PINT 检测力）
    const scrG = el("g", { "data-anim": "1" }, svg);
    el("rect", { x: gx1 + 16, y: 102, width: 116, height: 36, rx: 3, fill: "#1233b8" }, scrG);
    txt(scrG, gx1 + 74, 127, "95.22%", { size: 18, fill: "#fff", anchor: "middle", weight: 700, halo: false });
    txt(scrG, gx2 + 28, 122, "PINT 检测力读数（自家基准）", { size: 10, fill: PAL.inkMd, anchor: "start" });
    // 闸翼（语义部位：拦截率）
    const flapG = el("g", { "data-anim": "1" }, svg);
    el("path", { d: `M${gx1 + 30} 232 L${gx2 - 34} 196 L${gx2 - 34} 210 L${gx1 + 30} 248 Z`, fill: PAL.red, opacity: 0.88 }, flapG);
    txt(flapG, gx1 + 92, 186, "81.2%", { size: 17, fill: PAL.red, anchor: "middle", weight: 700 });
    txt(flapG, gx1 + 92, 171, "AgentDojo 真实拦截", { size: 9.5, anchor: "middle", fill: PAL.inkMd });
    // prompt 卡队列（语义部位：队列计数 = 95%+ 终结前两层）
    const qG = el("g", { "data-anim": "1" }, svg);
    for (let i = 0; i < 9; i++) {
      const cx = 88 + i * 30;
      el("rect", { x: cx, y: beltY - 20, width: 22, height: 14, rx: 2, fill: "#fff", stroke: PAL.inkLo, "stroke-width": 1.2 }, qG);
      el("line", { x1: cx + 4, y1: beltY - 14, x2: cx + 13, y2: beltY - 14, stroke: PAL.inkLo, "stroke-width": 1.2 }, qG);
      el("line", { x1: cx + 4, y1: beltY - 9, x2: cx + 16, y2: beltY - 9, stroke: PAL.inkLo, "stroke-width": 1 }, qG);
    }
    txt(qG, 208, beltY - 34, "95%+ 流量终结于 L0/L1", { size: 12, fill: PAL.ink, anchor: "middle", weight: 700 });
    // 通过闸门的卡（升级灰区 1–5%）
    const g2 = el("g", {}, svg);
    el("rect", { x: gx2 + 44, y: beltY - 20, width: 22, height: 14, rx: 2, fill: "#fff", stroke: PAL.red, "stroke-width": 1.6 }, g2);
    txt(g2, gx2 + 110, beltY - 24, "1–5% 灰区升级 judge", { size: 10.5, fill: PAL.inkMd });
    // 闸下漏网红卡（语义部位：绕过 = emoji/Unicode 混淆 100%）
    const eG = el("g", { "data-anim": "1" }, svg);
    el("rect", { x: gx1 + 96, y: gy - 13, width: 24, height: 14, rx: 2, fill: "rgba(194,47,78,.92)", transform: `rotate(-9 ${gx1 + 108} ${gy - 6})` }, eG);
    txt(eG, gx1 + 132, gy - 14, "100%：emoji/Unicode 混淆绕过 6 个商用护栏", { size: 11.5, fill: PAL.neg, weight: 700 });
    // 裂缝（语义部位：自适应攻击 >90% 绕过 = 闸门本体裂痕）
    const cG = el("g", { "data-anim": "1" }, svg);
    el("path", { d: `M${gx2 - 15} 160 l8 24 l-10 20 l11 22 l-7 26`, stroke: PAL.neg, "stroke-width": 2.2, fill: "none" }, cG);
    txt(cG, gx2 + 26, 236, ">90%：12 种防御被自适应攻击绕过", { size: 11.5, fill: PAL.neg, weight: 700 });
    txt(cG, gx2 + 26, 252, "The Attacker Moves Second · 2025-10", { size: 9.5, fill: PAL.inkLo });
    // 钻取
    drillOn(scrG, { title: "PINT 官方榜首", value: "95.22%", sub: "Lakera Guard，balanced accuracy；自家基准，利益冲突需打折；旧版口径 92.55% 并列", source: "Lakera PINT Benchmark, 2025-05-02 [K1] · 报告 §7.1" });
    drillOn(flapG, { title: "AgentDojo 真实攻击拦截率", value: "81.2%", sub: "Prompt Guard 2 86M，静态基准最强小模型；22M 版 78.4%；自适应攻击口径另列", source: "Meta Model Card, 2025-04-30 [K4] · 报告 §5.1" });
    drillOn(beltG, { title: "延迟分层", value: "<1ms → 1.2–1.5s", sub: "规则/归一化 <1ms → 小分类器 20–100ms → LLM judge 秒级；三个数量级决定级联架构", source: "GuardChain/Spheron 实测 [K20][K21] · 报告 §8.2" });
    drillOn(qG, { title: "流量终结点", value: "95%+", sub: "95% 以上流量在前两层终结；judge 层按 1–5% 升级率计费，成本低 1–2 个数量级", source: "报告 §8.3 [K21]" });
    drillOn(eG, { title: "混淆绕过率", value: "100%", sub: "Emoji Smuggling 对 6 个护栏（含 Azure Prompt Shield）注入与越狱均 100% ASR —— 归一化层不可省略", source: "arXiv:2504.11168, 2025-04 [K2] · 报告 §6.4" });
    drillOn(cG, { title: "自适应攻击绕过率", value: ">90%", sub: "12 种已发表检测/训练类防御全部被自适应攻击 >90% 绕过；唯一部分例外 CaMeL 架构隔离", source: "arXiv:2510.09023, 2025-10 [K3] · 报告 §9.3" });
    enter(svg, body);
  })();

  // ── PINT 官方榜（横条 + 利益冲突标注）──
  (() => {
    const host = document.getElementById("pint-chart");
    if (!host) return;
    const body = U.frame(host, {
      title: "PINT 官方榜：第一名是 Lakera，裁判也是 Lakera",
      sub: "BALANCED ACCURACY · 4,314 条输入（注入/越狱/硬负例）· 条长=得分 · 点击钻取 · 蓝色=厂商自家基准榜首",
      src: "Lakera PINT Benchmark, 2024-03 发布 / 测试 2025-05~08 [K1] · Aporia 经 hitechnectar 转引 2025-09-19 · 报告 §7.1",
    });
    const D = window.RPT.pint_board;
    const W = 880, rowH = 44, padT = 34, H = padT + D.length * rowH + 70;
    const svg = el("svg", { viewBox: `0 0 ${W} ${H}` }, body);
    svg.style.cssText = "width:100%;height:auto;display:block";
    const x0 = 250, x1 = W - 90;
    const X = v => x0 + (v / 100) * (x1 - x0);
    // 网格
    [60, 70, 80, 90, 100].forEach(v => {
      el("line", { x1: X(v), y1: padT - 8, x2: X(v), y2: H - 40, stroke: PAL.lineLo, "stroke-width": 1 }, svg);
      txt(svg, X(v), H - 24, v + "%", { size: 10, anchor: "middle", fill: PAL.inkLo });
    });
    D.forEach((d, i) => {
      const y = padT + i * rowH + (i > 0 ? 24 : 0) + 8;
      const g = el("g", {}, svg);
      txt(g, x0 - 12, y + 16, d.name, { font: SERIF, size: 13.5, anchor: "end", fill: PAL.ink, weight: d.self ? 700 : 400 });
      txt(g, x0 - 12, y + 30, d.kind + " · " + d.date, { size: 9, anchor: "end", fill: PAL.inkLo });
      const bar = el("rect", { x: x0, y, width: X(d.score) - x0, height: 22, fill: d.self ? PAL.red : (i < 3 ? "#42566a" : "#8595a6"), "data-anim": "1" }, g);
      txt(g, X(d.score) + 8, y + 16, d.score.toFixed(2) + "%", { size: 13, fill: d.self ? PAL.red : PAL.ink, weight: 700 });
      drillOn(g, { title: "PINT · " + d.name, value: d.score.toFixed(2) + "%", sub: d.note + "（测试日期 " + d.date + "）", source: (d.self ? "Lakera PINT Benchmark [K1] — 厂商自建基准，利益冲突已标注" : "Lakera PINT Benchmark [K1]") + " · 报告 §7.1" });
    });
    // 冲突口径标注
    const ay = padT + 6;
    txt(svg, X(95.22), ay - 14, "旧版口径 92.55%（CourtGuard 引）→ 现版 95.22%，并列呈现", { size: 9.5, anchor: "end", fill: PAL.inkLo });
    el("path", { d: `M${X(92.55)} ${padT + 30} L${X(92.55)} ${padT + 2}`, stroke: PAL.neg, "stroke-width": 1.2, "stroke-dasharray": "3 2" }, svg);
    // 利益冲突括注（占 Lakera 行下方专属标注带）
    txt(svg, x0 + 6, padT + 8 + 22 + 18, "↑ 榜首 = 基准建造者：完整数据集不公开（Goodhart 定律），分数需按第三方复现打折", { size: 10, fill: PAL.neg });
    enter(svg, body);
  })();

  // ── 低 FPR 死区（配对条 + 阈值线）──
  (() => {
    const host = document.getElementById("lowfpr-chart");
    if (!host) return;
    const body = U.frame(host, {
      title: "实用的低误报阈值，恰是 encoder 小模型的死区",
      sub: "TPR @ 0.1% FPR（每千条良性仅误伤 1 条的工作点）· 点击钻取",
      src: "PromptShield, arXiv:2501.15145, 2025-04-12 [K5] · 报告 §5.1",
    });
    const W = 800, H = 240, x0 = 230, x1 = W - 80;
    const svg = el("svg", { viewBox: `0 0 ${W} ${H}` }, body);
    svg.style.cssText = "width:100%;height:auto;display:block";
    const X = v => x0 + (v / 80) * (x1 - x0);
    [0, 20, 40, 60].forEach(v => {
      el("line", { x1: X(v), y1: 30, x2: X(v), y2: H - 36, stroke: PAL.lineLo }, svg);
      txt(svg, X(v), H - 20, v + "%", { size: 10, anchor: "middle", fill: PAL.inkLo });
    });
    // 小模型区间条 0–12%
    const g1 = el("g", {}, svg);
    txt(g1, x0 - 12, 62, "DeBERTa 级小模型", { font: SERIF, size: 13, anchor: "end", fill: PAL.ink });
    txt(g1, x0 - 12, 78, "ProtectAI / PromptGuard 档", { size: 9.5, anchor: "end", fill: PAL.inkLo });
    const r1 = el("rect", { x: x0, y: 52, width: X(12) - x0, height: 26, fill: "#8595a6", "data-anim": "1" }, g1);
    txt(g1, X(12) + 8, 70, "0–12%", { size: 14, weight: 700, fill: PAL.inkMd });
    // 8B 级
    const g2 = el("g", {}, svg);
    txt(g2, x0 - 12, 128, "8B 级检测器", { font: SERIF, size: 13, anchor: "end", fill: PAL.ink });
    const r2 = el("rect", { x: x0, y: 112, width: X(71.45) - x0, height: 26, fill: PAL.red, "data-anim": "1" }, g2);
    txt(g2, X(71.45) + 8, 126, "71.45%", { size: 14, weight: 700, fill: PAL.red });
    txt(g2, X(71.45) + 8, 142, "AUC 0.997", { size: 9.5, fill: PAL.inkLo });
    // 结论带
    txt(svg, x0, 176, "规模是硬门槛：要低误报，就要 8B 级复核层 —— 这正是级联架构 L2 的存在理由", { font: SERIF, size: 13, fill: PAL.ink, style: "font-style:italic" });
    drillOn(g1, { title: "TPR@0.1%FPR · 小模型", value: "0–12%", sub: "所有 DeBERTa 级小模型在实用低误报阈值下集体失效；ProtectAI v2 TPR@1%FPR 也仅 1.7%", source: "PromptShield, 2025-04-12 [K5] · 报告 §5.1" });
    drillOn(g2, { title: "TPR@0.1%FPR · 8B 级", value: "71.45%", sub: "8B 级检测器在低误报阈值下仍保七成真阳率（AUC 0.997）——规模是硬门槛", source: "PromptShield, 2025-04-12 [K5] · 报告 §5.1" });
    enter(svg, body);
  })();

  // ── AgentDojo 拦截率 ──
  (() => {
    const host = document.getElementById("agentdojo-chart");
    if (!host) return;
    const body = U.frame(host, {
      title: "AgentDojo 真实攻击：86M 小模型拦下八成，ProtectAI 不到四分之一",
      sub: "静态基准防攻率 · 自适应多轮攻击口径（各 guardrail ASR 仍 >90%）必须同时阅读 · 点击钻取",
      src: "Meta Model Card 2025-04-30 [K4] · sibyllinesoft/clean 复测 2026-02 [K30] · 报告 §5.1/§11.2",
    });
    const D = window.RPT.agentdojo;
    const W = 800, rowH = 42, padT = 24, H = padT + D.length * rowH + 60;
    const svg = el("svg", { viewBox: `0 0 ${W} ${H}` }, body);
    svg.style.cssText = "width:100%;height:auto;display:block";
    const x0 = 250, x1 = W - 90;
    const X = v => x0 + (v / 100) * (x1 - x0);
    [0, 25, 50, 75, 100].forEach(v => {
      el("line", { x1: X(v), y1: padT - 6, x2: X(v), y2: H - 52, stroke: PAL.lineLo }, svg);
      txt(svg, X(v), H - 36, v + "%", { size: 10, anchor: "middle", fill: PAL.inkLo });
    });
    D.forEach((d, i) => {
      const y = padT + i * rowH + 6;
      const g = el("g", {}, svg);
      txt(g, x0 - 12, y + 15, d.name, { font: SERIF, size: 13, anchor: "end", fill: PAL.ink });
      el("rect", { x: x0, y, width: X(d.v) - x0, height: 20, fill: i < 2 ? PAL.red : "#8595a6", "data-anim": "1" }, g);
      txt(g, X(d.v) + 8, y + 15, d.v + "%", { size: 13, weight: 700, fill: i < 2 ? PAL.red : PAL.inkMd });
      drillOn(g, { title: "AgentDojo · " + d.name, value: d.v + "%", sub: d.note + "；注意此为静态基准口径", source: "Meta Model Card [K4] / clean 复测 [K30] · 报告 §5.1" });
    });
    txt(svg, x0, H - 14, "并列口径：独立 SoK 研究显示自适应多轮攻击下各 guardrail ASR 仍 >90% —— 静态榜不代表实战免疫", { size: 10, fill: PAL.neg });
    enter(svg, body);
  })();

  // ── 路线赔率板（P7）──
  (() => {
    const host = document.getElementById("routes-chart");
    if (!host) return;
    const body = U.frame(host, {
      title: "三条技术路线：没有单赢家，胜者是分层组合",
      sub: "主观概率分级（LOW→HIGH 四档，双点=区间）· UNDER WAY=已在生产推进 · 点击行钻取",
      src: "报告 §9.1/§9.5 · LlamaFirewall [K22] · CaMeL [K16] · The Attacker Moves Second [K3]",
    });
    const D = window.RPT.routes;
    const W = 880, rowH = 74, padT = 40, H = padT + D.length * rowH + 30;
    const svg = el("svg", { viewBox: `0 0 ${W} ${H}` }, body);
    svg.style.cssText = "width:100%;height:auto;display:block";
    const sx0 = 330, sx1 = 620, step = (sx1 - sx0) / 4;
    const grades = ["LOW", "MED-LOW", "MED-HIGH", "HIGH"];
    for (let i = 0; i <= 4; i++) {
      el("line", { x1: sx0 + i * step, y1: padT - 10, x2: sx0 + i * step, y2: H - 24, stroke: PAL.lineLo }, svg);
      if (i < 4) txt(svg, sx0 + i * step + step / 2, padT - 16, grades[i], { size: 9, anchor: "middle", fill: PAL.inkLo });
    }
    const GX = v => sx0 + (v / 4) * (sx1 - sx0);
    D.forEach((d, i) => {
      const y = padT + i * rowH + 22;
      const g = el("g", {}, svg);
      if (d.underway) el("rect", { x: 10, y: y - 24, width: W - 20, height: rowH - 12, fill: "rgba(34,81,255,.045)" }, g);
      txt(g, 18, y, d.name, { font: SERIF, size: 14.5, weight: 700, fill: PAL.ink });
      txt(g, 18, y + 18, d.boundary, { size: 9.5, fill: PAL.inkLo });
      // 区间双点 + 连线
      el("line", { x1: GX(d.range[0]), y1: y, x2: GX(d.range[1]), y2: y, stroke: PAL.red, "stroke-width": 2 }, g);
      el("circle", { cx: GX(d.range[0]), cy: y, r: 4, fill: "#fff", stroke: PAL.red, "stroke-width": 2 }, g);
      el("circle", { cx: GX(d.range[1]), cy: y, r: 5.5, fill: PAL.red }, g);
      if (d.underway) {
        const bx = sx1 + 24;
        el("rect", { x: bx, y: y - 11, width: 74, height: 18, rx: 9, fill: "none", stroke: PAL.red, "stroke-width": 1.2 }, g);
        txt(g, bx + 37, y + 2, "UNDER WAY", { size: 8.5, anchor: "middle", fill: PAL.red, halo: false });
      }
      txt(g, sx1 + 24, y + 26, d.evidence.slice(0, 30) + "…", { size: 9, fill: PAL.inkMd });
      drillOn(g, { title: "路线 · " + d.name, value: grades[Math.round(d.grade) - 1] || "HIGH", sub: d.evidence + "。边界：" + d.boundary, source: "报告 §9.1 · 分级为主观判断（非计量）" });
    });
    enter(svg, body);
  })();

  // ── 四周路线图（横向时间带）──
  (() => {
    const host = document.getElementById("roadmap-chart");
    if (!host) return;
    const body = U.frame(host, {
      title: "四周落地：先影子、再校准、后阻断、终闭环",
      sub: "log-only → 阈值校准 → 阻断+升级 → 输出侧+样本回流 · 点击各周钻取",
      src: "报告 §10.4 · 红队工具 [K 表] · held-out 纪律见 §7.6",
    });
    const D = window.RPT.roadmap;
    const W = 880, H = 230, pad = 40;
    const svg = el("svg", { viewBox: `0 0 ${W} ${H}` }, body);
    svg.style.cssText = "width:100%;height:auto;display:block";
    const y = 66;
    el("line", { x1: pad, y1: y, x2: W - pad, y2: y, stroke: PAL.ink, "stroke-width": 1.5 }, svg);
    D.forEach((d, i) => {
      const x = pad + (i + 0.5) * (W - 2 * pad) / 4;
      const g = el("g", { "data-anim": "1" }, svg);
      el("circle", { cx: x, cy: y, r: 7, fill: i === 3 ? PAL.red : "#fff", stroke: i === 3 ? PAL.red : PAL.ink, "stroke-width": 2 }, g);
      txt(g, x, y - 18, d.week, { size: 11, anchor: "middle", weight: 700, fill: PAL.red });
      txt(g, x, y + 30, d.title, { font: SERIF, size: 13.5, anchor: "middle", weight: 700, fill: PAL.ink });
      // 换行正文（避免在 ASCII 词中间断行）
      const words = d.body; let ly = y + 50;
      const maxCh = 15;
      for (let p = 0; p < words.length;) {
        let q = Math.min(p + maxCh, words.length);
        if (q < words.length && /[A-Za-z0-9]/.test(words[q - 1]) && /[A-Za-z0-9]/.test(words[q])) {
          let back = q - 1;
          while (back > p && /[A-Za-z0-9]/.test(words[back])) back--;
          if (back > p) q = back;
        }
        txt(g, x, ly, words.slice(p, q).trim(), { font: SERIF, size: 10.5, anchor: "middle", fill: PAL.inkMd });
        ly += 15; p = q;
      }
      drillOn(g, { title: d.week + " · " + d.title, value: d.title, sub: d.body, source: "报告 §10.4" });
    });
    enter(svg, body);
  })();
})();
