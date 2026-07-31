// 工程落地 · 级联安检管线（本报告签名图）
// 对数延迟轴上的三道闸：规则(<1ms) → 小分类器(20–100ms) → LLM judge(秒级)。
// 编码：闸位=实测延迟（对数轴）· 流带宽度=流量份额 · 卡片状态=通过/拦截/升级。
(() => {
  const host = document.getElementById("pipeline-chart");
  if (!host) return;
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
  const txt = (parent, x, y, str, { font = MONO, size = 11, fill = PAL.inkMd, anchor = "start", weight = 400, halo = true, italic = false } = {}) => {
    const t = el("text", { x, y, "font-size": size, "text-anchor": anchor, fill, style: `font-family:${font};font-weight:${weight};${italic ? "font-style:italic;" : ""}` }, parent);
    if (halo) { t.setAttribute("paint-order", "stroke"); t.setAttribute("stroke", "#fff"); t.setAttribute("stroke-width", "4"); }
    t.textContent = str;
    return t;
  };
  const body = U.frame(host, {
    title: "级联闸道：三个数量级的延迟分层，95%+ 流量在前两道闸终结",
    sub: "横轴=端到端检测延迟（对数刻度）· 流带宽度=流量份额 · 闸机位置=该层实测延迟带 · 点击闸机/流带钻取",
    src: "GuardChain（arXiv:2604.03598, 2026-04）[K21] · Spheron 生产栈 2026-05 [K20] · Meta LlamaFirewall [K22] · 报告 §8.2–8.3",
  });

  const W = 920, H = 520;
  const svg = el("svg", { viewBox: `0 0 ${W} ${H}` }, body);
  svg.style.cssText = "width:100%;height:auto;display:block";

  // 对数延迟轴：0.05ms → 3000ms
  const axY = 400, x0 = 60, x1 = W - 50;
  const lMin = Math.log10(0.05), lMax = Math.log10(3000);
  const X = ms => x0 + (Math.log10(ms) - lMin) / (lMax - lMin) * (x1 - x0);
  el("line", { x1: x0, y1: axY, x2: x1, y2: axY, stroke: PAL.ink, "stroke-width": 1.5 }, svg);
  el("path", { d: `M${x1} ${axY} l-8 -4 l0 8 Z`, fill: PAL.ink }, svg);
  [[0.1, "0.1ms"], [1, "1ms"], [10, "10ms"], [100, "100ms"], [1000, "1s"]].forEach(([v, lab]) => {
    el("line", { x1: X(v), y1: axY - 5, x2: X(v), y2: axY + 5, stroke: PAL.ink, "stroke-width": 1.2 }, svg);
    txt(svg, X(v), axY + 22, lab, { size: 10.5, anchor: "middle", fill: PAL.ink, weight: 700 });
  });
  txt(svg, x1, axY + 40, "检测延迟（对数刻度）→", { size: 10, anchor: "end", fill: PAL.inkLo });
  // 三个数量级括号
  const brY = axY + 52;
  el("path", { d: `M${X(0.1)} ${brY} L${X(1000)} ${brY}`, stroke: PAL.inkLo, "stroke-width": 1, "stroke-dasharray": "4 3" }, svg);
  txt(svg, (X(0.1) + X(1000)) / 2, brY + 16, "规则 → judge：四个数量级 —— 级联是工程必然，不是风格偏好", { font: SERIF, size: 11.5, anchor: "middle", fill: PAL.inkMd, italic: true });

  // 流带（宽度 ∝ 流量份额）
  const flowY = 210, fullW = 84, escW = 12;
  // 入口 → L0 → L1：全量
  const flow1 = el("path", {
    d: `M${x0 - 30} ${flowY - fullW / 2} L${X(50) - 40} ${flowY - fullW / 2} L${X(50) - 40} ${flowY + fullW / 2} L${x0 - 30} ${flowY + fullW / 2} Z`,
    fill: "rgba(34,81,255,.10)", stroke: "rgba(34,81,255,.35)", "stroke-width": 1, "data-anim": "1",
  }, svg);
  // L1 → L2：升级 ~5%
  const flow2 = el("path", {
    d: `M${X(50) + 40} ${flowY - escW / 2} C ${X(200)} ${flowY - escW / 2}, ${X(500)} ${flowY - escW}, ${X(1300) - 40} ${flowY - escW / 2} L${X(1300) - 40} ${flowY + escW / 2} C ${X(500)} ${flowY + escW}, ${X(200)} ${flowY + escW / 2}, ${X(50) + 40} ${flowY + escW / 2} Z`,
    fill: "rgba(18,51,184,.18)", stroke: "rgba(18,51,184,.5)", "stroke-width": 1, "data-anim": "1",
  }, svg);
  // 终结漏斗（L1 处 95% 向下终结）
  const sink = el("path", {
    d: `M${X(50) - 30} ${flowY + fullW / 2} L${X(50) + 30} ${flowY + fullW / 2} L${X(50) + 12} ${flowY + 120} L${X(50) - 12} ${flowY + 120} Z`,
    fill: "rgba(34,81,255,.07)", stroke: "rgba(34,81,255,.25)", "stroke-width": 1, "stroke-dasharray": "4 3", "data-anim": "1",
  }, svg);
  txt(svg, X(50), flowY + 145, "95%+ 流量在此终结（允许/安全拒答）", { size: 11.5, anchor: "middle", fill: PAL.red, weight: 700 });
  txt(svg, X(50), flowY + 162, "总成本 <$5/百万请求", { size: 9.5, anchor: "middle", fill: PAL.inkLo });

  // 闸机图示（前视：双柱 + 顶舱 + 闸翼 + 屏幕）
  function gate(cx, baseYv, scale, tint, label, sub, band) {
    const g = el("g", { "data-anim": "1" }, svg);
    const s = scale;
    // 延迟带
    if (band) {
      el("rect", { x: X(band[0]), y: baseYv - 150 * s, width: X(band[1]) - X(band[0]), height: 165 * s, fill: "rgba(133,149,166,.10)", stroke: PAL.lineLo, "stroke-dasharray": "3 3" }, g);
    }
    // 双柱
    el("rect", { x: cx - 34 * s, y: baseYv - 96 * s, width: 12 * s, height: 96 * s, fill: "#42566a" }, g);
    el("rect", { x: cx + 22 * s, y: baseYv - 96 * s, width: 12 * s, height: 96 * s, fill: "#42566a" }, g);
    // 顶舱 + 屏
    el("rect", { x: cx - 42 * s, y: baseYv - 128 * s, width: 84 * s, height: 32 * s, rx: 4, fill: tint }, g);
    el("rect", { x: cx - 26 * s, y: baseYv - 122 * s, width: 52 * s, height: 14 * s, rx: 2, fill: "rgba(255,255,255,.22)" }, g);
    // 闸翼
    el("path", { d: `M${cx - 22 * s} ${baseYv - 60 * s} L${cx + 30 * s} ${baseYv - 76 * s} L${cx + 30 * s} ${baseYv - 68 * s} L${cx - 22 * s} ${baseYv - 50 * s} Z`, fill: tint, opacity: 0.85 }, g);
    // 名称
    txt(g, cx, baseYv - 140 * s, label, { font: SERIF, size: 15, anchor: "middle", weight: 700, fill: PAL.ink });
    txt(g, cx, baseYv + 16 * s, sub, { size: 10, anchor: "middle", fill: PAL.inkMd });
    return g;
  }

  // 三道闸（立于流带上沿）
  const g0 = gate(X(0.5), flowY - fullW / 2 + 6, 0.62, "#42566a", "L0 · 规则归一化闸", "<1ms · NFKC/去零宽/Base64 解码", null);
  const g1 = gate(X(50), flowY - fullW / 2 + 6, 1.0, "#2251ff", "L1 · 小分类器闸机", "20–100ms · PromptGuard2 / Qwen3Guard", [19, 100]);
  const g2 = gate(X(1300), flowY - 6, 0.8, "#1233b8", "L2 · LLM judge 复核舱", "1.2–1.5s · 仅 1–5% 灰区升级", null);

  // 恶意卡被 L1 闸口弹开
  const bc = el("g", { "data-anim": "1" }, svg);
  el("rect", { x: X(50) - 66, y: flowY - 58, width: 18, height: 12, rx: 1.5, fill: "rgba(194,47,78,.92)", transform: `rotate(-24 ${X(50) - 57} ${flowY - 52})` }, bc);
  txt(bc, X(50) - 80, flowY - 70, "恶意卡被弹开", { size: 9.5, fill: PAL.neg, anchor: "end", weight: 700 });
  // 通过的良性卡
  for (let i = 0; i < 3; i++) {
    el("rect", { x: X(50) + 60 + i * 26, y: flowY - 6, width: 16, height: 10, rx: 1.5, fill: "#fff", stroke: PAL.inkLo, "stroke-width": 1, "data-anim": "1" }, svg);
  }
  // 升级卡（细流带里）
  el("rect", { x: X(320), y: flowY - 5, width: 12, height: 8, rx: 1, fill: "#fff", stroke: PAL.redHi, "stroke-width": 1.2, "data-anim": "1" }, svg);

  // SaaS 参照（轴下方，虚线）
  const saasG = el("g", {}, svg);
  el("rect", { x: X(200), y: axY - 92, width: X(500) - X(200), height: 34, fill: "none", stroke: PAL.inkLo, "stroke-width": 1, "stroke-dasharray": "5 3" }, saasG);
  txt(saasG, (X(200) + X(500)) / 2, axY - 70, "SaaS 参照：Azure Prompt Shields 同步 200–500ms", { size: 9.5, anchor: "middle", fill: PAL.inkMd });

  // 层职责注记（闸机副标第二行，与副标拉开行距）
  txt(svg, X(0.5), flowY - fullW / 2 + 6 + 44 * 0.62, "先归一化——不做它下游全失效", { size: 9, anchor: "middle", fill: PAL.neg });
  txt(svg, X(50), flowY - fullW / 2 + 6 + 44, "快路径主力 · 毫秒级 · 可共池", { size: 9, anchor: "middle", fill: PAL.inkLo });
  txt(svg, X(1300), flowY - 6 + 30 * 0.8 + 26, "TPR@0.1%FPR 71% vs 小模型 0–12%", { size: 9, anchor: "middle", fill: PAL.inkLo });

  // 钻取
  const drill = (node, d) => {
    node.style.cursor = "pointer"; node.setAttribute("data-drill-keep", "1");
    node.addEventListener("click", ev => U.showDrill({ ...d, x: ev.clientX, y: ev.clientY }));
  };
  drill(g0, { title: "L0 · 规则归一化闸", value: "<1ms", sub: "正则（Aho-Corasick <0.1–0.2ms）+ Unicode NFKC + 去零宽/Bidi + Base64/编码识别解码 + 长度结构异常。不拦截攻击本身，但缺它下游全失效", source: "报告 §8.2 / §6.4 [K2][K21]" });
  drill(g1, { title: "L1 · 小分类器闸机", value: "20–100ms", sub: "PromptGuard2 22M 19ms / 86M 20–92ms；Qwen3Guard-0.6B 89ms（A10G）。95%+ 流量在此终结；一张 T4/A10G 承载 L1+L2 共池", source: "Meta Model Card [K4] · Spheron [K20] · 报告 §8.2" });
  drill(g2, { title: "L2 · LLM judge 复核舱", value: "1.2–1.5s", sub: "每方向一次额外 LLM 调用；仅处理灰区/高危会话（1–5% 升级率）；8B 级 TPR@0.1%FPR 71.45%", source: "arXiv:2605.06669 [K8] · PromptShield [K5] · 报告 §8.2–8.3" });
  drill(flow2, { title: "升级流带", value: "1–5%", sub: "GuardChain 实测仅 ~2% 分布内流量升级到 GPU 级；judge 层按升级率计费，整体成本比'每请求都过 judge'低 1–2 个数量级", source: "GuardChain [K21] · 报告 §8.3" });
  drill(saasG, { title: "SaaS 延迟参照", value: "200–500ms", sub: "Azure Prompt Shields 同步调用（来源为有立场偏向的对比页，已标注）；Lakera 宣称 P95 ~50ms（厂商口径）", source: "报告 §5.2 / §11.3 [K18]" });

  // 入场
  if (!REDUCE) {
    svg.querySelectorAll("[data-anim]").forEach(n => {
      n.style.opacity = "0"; n.style.transition = "opacity .7s ease";
    });
    const io = new IntersectionObserver(es => es.forEach(e => {
      if (!e.isIntersecting) return;
      io.disconnect();
      svg.querySelectorAll("[data-anim]").forEach((n, i) => setTimeout(() => n.style.opacity = "1", i * 70));
    }), { threshold: 0.15 });
    io.observe(body);
  }
})();
