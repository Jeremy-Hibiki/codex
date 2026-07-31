// 报告第 10–14 章图表：政策推理对比 · AgentDoG · I/O 六形态 · I/O 主对照表 · 多轮矩阵 · 512 上下文墙
// 签名图：上下文长度阶梯图（gate ladder，对数 token 轴上的安检闸门视野）
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
  const drill = (node, d) => {
    node.style.cursor = "pointer"; node.setAttribute("data-drill-keep", "1");
    node.addEventListener("click", ev => U.showDrill({ ...d, x: ev.clientX, y: ev.clientY }));
  };
  function frameTable(hostId, { title, sub, src, head, rows }) {
    const host = document.getElementById(hostId);
    if (!host) return;
    const body = U.frame(host, { title, sub, src });
    const t = document.createElement("table");
    t.className = "dt";
    t.innerHTML = `<thead><tr>${head.map(h => `<th>${h}</th>`).join("")}</tr></thead>`;
    const tb = document.createElement("tbody");
    rows.forEach(r => tb.appendChild(r));
    t.appendChild(tb);
    body.appendChild(t);
    if (!REDUCE) {
      const rows_ = tb.querySelectorAll("tr");
      rows_.forEach(r => { r.style.opacity = "0"; r.style.transition = "opacity .5s ease"; });
      const io = new IntersectionObserver(es => es.forEach(e => {
        if (!e.isIntersecting) return; io.disconnect();
        rows_.forEach((r, i) => setTimeout(() => r.style.opacity = "1", i * 55));
      }), { threshold: 0.1 });
      io.observe(body);
    }
    return t;
  }
  function drillRow(d) {
    const tr = document.createElement("tr");
    tr.style.cursor = "pointer";
    tr.setAttribute("data-drill-keep", "1");
    tr.addEventListener("click", ev => U.showDrill({ ...d, x: ev.clientX, y: ev.clientY }));
    return tr;
  }
  const R = window.RPT;

  // ══ §8 政策推理对比表 ══
  (() => {
    const rows = R.policy_compare.map(m => {
      const tr = drillRow({
        title: "定位对比 · " + m.dim, value: m.gptoss,
        sub: `gpt-oss-safeguard：${m.gptoss} ｜ ShieldGemma：${m.sg} ｜ Qwen3Guard-Gen：${m.qwen} ｜ Prompt Guard 2：${m.pg}`,
        source: "报告 §10 [K33][K34][K37]",
      });
      if (m.dim === "适用层") tr.className = "hl";
      tr.innerHTML = `<td style="font-weight:700">${m.dim}</td><td>${m.gptoss}</td>
        <td style="color:var(--ink-md)">${m.sg}</td><td>${m.qwen}</td><td>${m.pg}</td>`;
      return tr;
    });
    frameTable("policy-table", {
      title: "政策推理 vs 内容分类 vs 注入专精：四个家族一次对齐",
      sub: "蓝底行=定位结论：gpt-oss-safeguard 走慢路径深检，ShieldGemma 本场景排除 · 点击行钻取",
      src: "报告 §10 · gpt-oss [K33][K47] · ShieldGemma [K34] · ToxicChat 独立榜 [K37] · 中文 F1 [K31]",
      head: ["维度", "gpt-oss-safeguard 20b/120b", "ShieldGemma（文本）", "Qwen3Guard-Gen", "Prompt Guard 2"],
      rows,
    });
  })();

  // ══ §9 AgentDoG 1.5 Pre-Reply 配对条形（before→after ASR）══
  (() => {
    const host = document.getElementById("agentdog-chart");
    if (!host) return;
    const body = U.frame(host, {
      title: "AgentDoG 1.5 作 Pre-Reply 在线护栏：三个基准的攻击成功率（ASR）压缩",
      sub: "仅在最终交付前审计一次 · 灰条=无护栏 ASR · 蓝条=挂 AgentDoG 1.5-4B 后 · 点击钻取",
      src: "AgentDoG 1.5（arXiv:2605.29801, 2026-05-28）[K36]",
    });
    const W = 880, H = 300;
    const svg = el("svg", { viewBox: `0 0 ${W} ${H}` }, body);
    svg.style.cssText = "width:100%;height:auto;display:block";
    const x0 = 210, x1 = W - 60, y0 = 44, rh = 62, bh = 17;
    const X = v => x0 + v / 100 * (x1 - x0);
    [0, 25, 50, 75, 100].forEach(v => {
      el("line", { x1: X(v), y1: y0 - 12, x2: X(v), y2: y0 + R.agentdog.prerply.length * rh, stroke: PAL.lineLo, "stroke-width": 1 }, svg);
      txt(svg, X(v), y0 + R.agentdog.prerply.length * rh + 20, v + "%", { size: 10, anchor: "middle", fill: PAL.inkLo });
    });
    R.agentdog.prerply.forEach((b, i) => {
      const y = y0 + i * rh;
      const drop = Math.round((1 - b.after / b.before) * 100);
      const g = el("g", { "data-anim": "1" }, svg);
      txt(g, x0 - 12, y + 24, b.bench, { font: SERIF, size: 13.5, anchor: "end", fill: PAL.ink, weight: 700 });
      el("rect", { x: x0, y, width: X(b.before) - x0, height: bh, fill: "#c3ccd8", rx: 2, "data-anim": "1" }, g);
      el("rect", { x: x0, y: y + bh + 5, width: X(b.after) - x0, height: bh, fill: PAL.red, rx: 2, "data-anim": "1" }, g);
      txt(g, X(b.before) + 8, y + 13, b.before + "%", { size: 11, fill: PAL.inkMd, weight: 700 });
      txt(g, X(b.after) + 8, y + bh + 18, b.after + "%", { size: 11, fill: PAL.red, weight: 700 });
      txt(g, x1, y + 24, `−${drop}%`, { size: 13, fill: PAL.green, weight: 700, anchor: "end" });
      drill(g, {
        title: b.bench + " · Pre-Reply 护栏", value: `${b.before}% → ${b.after}%`,
        sub: `AgentDoG 1.5-4B 仅在最终交付前审计一次，ASR 相对降 ${drop}%；TTFT 亚秒级。局限：仅文本轨迹、只覆盖交付前检查点`,
        source: "arXiv:2605.29801, 2026-05-28 [K36] · 报告 §11",
      });
    });
    txt(svg, x0, H - 16, "另：R-Judge 92.2% acc / 92.7 F1 · ATBench 72.4%（较 1.0-4B +8.4）· 三维诊断均值 55.2%（GPT-5.4 为 25.8%）", { font: SERIF, size: 12, fill: PAL.inkMd, style: "font-style:italic" });
    if (!REDUCE) {
      svg.querySelectorAll("[data-anim]").forEach(n => { n.style.opacity = "0"; n.style.transition = "opacity .7s ease"; });
      const io = new IntersectionObserver(es => es.forEach(e => {
        if (!e.isIntersecting) return; io.disconnect();
        svg.querySelectorAll("[data-anim]").forEach((n, i) => setTimeout(() => n.style.opacity = "1", i * 60));
      }), { threshold: 0.15 });
      io.observe(body);
    }
  })();

  // ══ §9 SingGuard-NSFA 分类头插件增强 Llama Guard 3（before→after F1 配对条形）══
  (() => {
    const host = document.getElementById("singuard-chart");
    if (!host) return;
    const body = U.frame(host, {
      title: "SingGuard-NSFA 可插拔分类头作插件：Llama Guard 3 三个基准的 F1 提升",
      sub: "冻结骨干 + 按域分类头可为存量护栏低成本升级 · 灰条=Llama Guard 3 原版 · 蓝条=加 NSFA 分类头后 · 官方自建基准口径，暂无第三方复测 · 点击钻取",
      src: "SingGuard-NSFA（arXiv:2607.13081）[K84] · 开源情况 [K85] · 第三方报道 [K86]",
    });
    const W = 880, H = 330;
    const svg = el("svg", { viewBox: `0 0 ${W} ${H}` }, body);
    svg.style.cssText = "width:100%;height:auto;display:block";
    const x0 = 210, x1 = W - 70, y0 = 44, rh = 62, bh = 17;
    const X = v => x0 + v / 100 * (x1 - x0);
    [0, 25, 50, 75, 100].forEach(v => {
      el("line", { x1: X(v), y1: y0 - 12, x2: X(v), y2: y0 + R.singuard.plugin.length * rh, stroke: PAL.lineLo, "stroke-width": 1 }, svg);
      txt(svg, X(v), y0 + R.singuard.plugin.length * rh + 20, v + "", { size: 10, anchor: "middle", fill: PAL.inkLo });
    });
    R.singuard.plugin.forEach((b, i) => {
      const y = y0 + i * rh;
      const gain = (b.after - b.before).toFixed(1);
      const g = el("g", { "data-anim": "1" }, svg);
      txt(g, x0 - 12, y + 24, b.bench, { font: SERIF, size: 13.5, anchor: "end", fill: PAL.ink, weight: 700 });
      el("rect", { x: x0, y, width: X(b.before) - x0, height: bh, fill: "#c3ccd8", rx: 2, "data-anim": "1" }, g);
      el("rect", { x: x0, y: y + bh + 5, width: X(b.after) - x0, height: bh, fill: PAL.red, rx: 2, "data-anim": "1" }, g);
      txt(g, X(b.before) + 8, y + 13, b.before + "", { size: 11, fill: PAL.inkMd, weight: 700 });
      txt(g, X(b.after) + 8, y + bh + 18, b.after + "", { size: 11, fill: PAL.red, weight: 700 });
      txt(g, x1, y + 24, `+${gain}`, { size: 13, fill: PAL.green, weight: 700, anchor: "end" });
      drill(g, {
        title: "NSFA 分类头插件 · " + b.bench, value: `${b.before} → ${b.after} F1`,
        sub: `Llama Guard 3 加装 NSFA 按域可插拔分类头后 ${b.bench} F1 +${gain}。判别模式单次前向 45–57ms/样本，分类头 5→50,000 个端到端延迟仅 +9ms。口径：蚂蚁自建多语言基准，官方数据，暂无第三方复测`,
        source: "arXiv:2607.13081 [K84] · 报告 §11.6",
      });
    });
    txt(svg, x0, H - 46, "SingGuard 关键数字（官方口径）：35 数据集平均 F1 第一 [K83] · NSFA 四档 F1>94%、133 语言、185 风险变体 [K84]", { font: SERIF, size: 12, fill: PAL.inkMd });
    txt(svg, x0, H - 28, "判别式分类头延迟 45–57ms/样本 · 政策遵循准确率 0.6465→0.7415 · 2026-07-13 开源 Apache-2.0", { font: SERIF, size: 12, fill: PAL.inkMd });
    txt(svg, x0, H - 10, "口径提醒：全部基准均为蚂蚁自建，横向对比仅官方口径、尚无第三方独立复测", { font: SERIF, size: 11.5, fill: PAL.neg, style: "font-style:italic" });
    if (!REDUCE) {
      svg.querySelectorAll("[data-anim]").forEach(n => { n.style.opacity = "0"; n.style.transition = "opacity .7s ease"; });
      const io = new IntersectionObserver(es => es.forEach(e => {
        if (!e.isIntersecting) return; io.disconnect();
        svg.querySelectorAll("[data-anim]").forEach((n, i) => setTimeout(() => n.style.opacity = "1", i * 60));
      }), { threshold: 0.15 });
      io.observe(body);
    }
  })();

  // ══ §9 新基准表 + 工业动态表 ══
  (() => {
    const rows = R.new_benches.map(b => {
      const tr = drillRow({
        title: b.name, value: b.date,
        sub: `对象：${b.obj}。关键发现：${b.find}`,
        source: `报告 §11 [${b.src}]`,
      });
      tr.innerHTML = `<td style="font-weight:700">${b.name}</td><td class="num">${b.date}</td>
        <td>${b.obj}</td><td style="color:var(--ink-md);font-size:12px">${b.find}</td>`;
      return tr;
    });
    frameTable("newbench-table", {
      title: "2025Q4 后的新基准：攻击面从单条 prompt 扩到轨迹、skill 与 OS 行为",
      sub: "共同结论：隐式攻击与多步轨迹是现有检测器的共性盲区 · 点击行钻取",
      src: "报告 §11 · [K41][K42]",
      head: ["基准", "时间", "对象与规模", "关键发现"],
      rows,
    });
  })();
  (() => {
    const rows = R.industry_moves.map(m => {
      const tr = drillRow({
        title: m.who, value: m.date, sub: m.what,
        source: "报告 §11 工业界动态",
      });
      tr.innerHTML = `<td style="font-weight:700">${m.who}</td><td class="num">${m.date}</td><td>${m.what}</td>`;
      return tr;
    });
    frameTable("industry-table", {
      title: "工业界动态：网关护栏标配化，检测组件自身也要过供应链审查",
      sub: "收购与整合密集（Check Point×Lakera、PANW×Protect AI/Portkey、SentinelOne×Prompt Security）· 点击行钻取",
      src: "报告 §11 · 各厂商官方博客/新闻稿（日期见 K 表）",
      head: ["主体", "时间", "动作"],
      rows,
    });
  })();

  // ══ §10 I/O 六形态卡片 ══
  (() => {
    const host = document.getElementById("ioforms-chart");
    if (!host) return;
    const body = U.frame(host, {
      title: "六种 I/O 形态：接入前先认形态，再谈选型",
      sub: "F 轨迹审计是唯一覆盖间接注入/目标劫持的形态 · A/B/C/D 均为'末条判定'家族 · 点击卡片钻取",
      src: "报告 §12 六形态归纳 · LlamaFirewall [K22] · AgentDoG [K35][K36]",
    });
    const grid = document.createElement("div");
    grid.style.cssText = "display:grid;grid-template-columns:repeat(3,1fr);gap:14px;margin-top:10px";
    R.io_forms.forEach(f => {
      const c = document.createElement("div");
      c.style.cssText = `border:1px solid var(--line);border-radius:6px;padding:14px 16px;background:${f.id === "F" ? "rgba(194,47,78,.05)" : "#fff"};cursor:pointer`;
      c.setAttribute("data-drill-keep", "1");
      c.innerHTML = `<div style="display:flex;align-items:baseline;gap:10px">
          <span style="font-family:var(--mono);font-weight:700;font-size:22px;color:${f.id === "F" ? "var(--neg)" : "var(--red)"}">${f.id}</span>
          <span style="font-family:var(--serif);font-weight:700;font-size:15.5px;color:var(--ink)">${f.name}</span></div>
        <p style="font-family:var(--mono);font-size:10.5px;color:var(--ink-md);margin:8px 0 6px">${f.io}</p>
        <p style="font-size:12.5px;color:var(--ink);margin:0 0 6px">${f.reps}</p>
        <p style="font-family:var(--mono);font-size:10px;color:var(--ink-lo);margin:0 0 6px">延迟：${f.lat}</p>
        <p style="font-size:11.5px;color:var(--ink-md);margin:0">${f.note}</p>`;
      c.addEventListener("click", ev => U.showDrill({
        title: `形态 ${f.id} · ${f.name}`, value: f.lat,
        sub: `${f.io}。代表：${f.reps}。${f.note}`,
        source: "报告 §12",
        x: ev.clientX, y: ev.clientY,
      }));
      grid.appendChild(c);
    });
    body.appendChild(grid);
  })();

  // ══ §10 I/O 主对照表（18 方案）══
  (() => {
    const rows = R.io_specs.map(m => {
      const tr = drillRow({
        title: m.name, value: "形态 " + m.form,
        sub: `输入：${m.input} ｜ 上下文：${m.ctx} ｜ 输出：${m.output} ｜ 阈值：${m.thr} ｜ 交付：${m.ship} ｜ 延迟：${m.lat}`,
        source: "报告 §12 主对照表（调用示例见原文）",
      });
      if (m.name.includes("gpt-oss") || m.name.includes("AgentDoG") || m.name.includes("SingGuard")) tr.className = "hl";
      tr.innerHTML = `<td style="font-weight:700">${m.name}</td>
        <td class="num" style="font-weight:700;color:var(--red)">${m.form}</td>
        <td style="font-size:12px">${m.input}</td><td class="num">${m.role}</td>
        <td class="num" style="font-size:12px">${m.ctx}</td>
        <td style="font-size:12px">${m.output}</td>
        <td style="font-size:12px">${m.thr}</td>
        <td class="num" style="font-size:12px">${m.lat}</td>`;
      return tr;
    });
    frameTable("io-table", {
      title: "I/O 规格主对照表：20 个方案/框架的输入、输出、上下文与延迟",
      sub: "蓝底行=2026 年新覆盖方案 · 形态 A–F 定义见上卡 · 点击行钻取完整契约",
      src: "报告 §12 · Lakera/Azure/AWS/Qwen/Llama/OpenAI 官方文档（日期见 K 表）",
      head: ["方案", "形态", "输入形态", "role", "上下文上限", "输出形态", "阈值可调", "典型延迟"],
      rows,
    });
  })();

  // ══ §11 多轮四类方法 + 支持矩阵 ══
  (() => {
    const rows = R.multiturn_methods.map(m => {
      const tr = drillRow({
        title: m.name, value: m.rep,
        sub: `I/O：${m.io}。效果：${m.perf}。定位：${m.role}`,
        source: "报告 §13 [K38][K22][K36]",
      });
      tr.innerHTML = `<td style="font-weight:700">${m.name}</td><td>${m.rep}</td>
        <td style="font-size:12px">${m.io}</td>
        <td style="font-size:12px">${m.perf}</td>
        <td style="color:var(--ink-md);font-size:12px">${m.role}</td>`;
      return tr;
    });
    frameTable("multiturn-methods", {
      title: "四类多轮检测方法：压缩检测 + 会话风险分 + 多轮护栏 + 轨迹审计",
      sub: "组合拳而非单选：首道筛用压缩，会话状态放网关，高权限加轨迹审计 · 点击行钻取",
      src: "报告 §13 · Defensive M2S [K38] · AlignmentCheck [K22] · AgentDoG [K36]",
      head: ["方法", "代表", "I/O", "代表数据", "定位"],
      rows,
    });
  })();
  (() => {
    const rows = R.multiturn_matrix.map(m => {
      const tr = drillRow({
        title: m.name, value: m.hist,
        sub: `判定范围：${m.scope}。多轮语义：${m.sem}`,
        source: "报告 §13 多轮支持矩阵",
      });
      if (m.name.includes("AgentDoG")) tr.className = "hl";
      tr.innerHTML = `<td style="font-weight:700">${m.name}</td><td class="num">${m.hist}</td>
        <td>${m.scope}</td><td style="color:var(--ink-md);font-size:12px">${m.sem}</td>`;
      return tr;
    });
    frameTable("multiturn-table", {
      title: "主流方案多轮支持矩阵：没有产品开箱即做跨轮风险累积",
      sub: "托管 API 至今是会话无感的文本评估器；'接受消息数组'≠'跨轮累积'——这层逻辑要自己在网关实现 · 点击行钻取",
      src: "报告 §13 · 各方案官方文档（日期见 K 表）",
      head: ["方案", "接受对话历史", "判定范围", "多轮语义"],
      rows,
    });
  })();

  // ══ §12 签名图：上下文长度阶梯图（gate ladder）══
  // 对数 token 轴上，每个模型的"闸门视野"条 = 上下文上限；红色带 = Agent 典型输入长度带；
  // 条够不到带右缘的部分画红色斜纹 = 盲区。编码：上限(对数位置) × 形态(颜色) × 盲区(斜纹段)。
  (() => {
    const host = document.getElementById("ctx-chart");
    if (!host) return;
    const body = U.frame(host, {
      title: "512 上下文墙：快路径闸门的视野，只有被保护对象的千分之一",
      sub: "横轴=上下文上限（token，对数刻度）· 横条=该模型一次能看到的范围 · 红色带=Agent 典型输入长度带（RAG+工具返回+对话历史 4K–200K）· 红色斜纹=该模型的盲区 · 点击横条钻取",
      src: "报告 §14 各模型上限核实 · 窗口缝隙武器化 [K43] · Sentinel 8K [K44] · LG4 [K45]",
    });
    const W = 920, H = 560;
    const svg = el("svg", { viewBox: `0 0 ${W} ${H}` }, body);
    svg.style.cssText = "width:100%;height:auto;display:block";
    const x0 = 218, x1 = W - 40;
    const lMin = Math.log10(256), lMax = Math.log10(524288);
    const X = t => x0 + (Math.log10(t) - lMin) / (lMax - lMin) * (x1 - x0);
    const models = R.ctx_limits;
    const y0 = 96, rh = 40, bh = 15;
    const yEnd = y0 + models.length * rh;

    // 斜纹定义
    const defs = el("defs", {}, svg);
    const pat = el("pattern", { id: "blind", width: 7, height: 7, patternUnits: "userSpaceOnUse", patternTransform: "rotate(45)" }, defs);
    el("rect", { width: 7, height: 7, fill: "rgba(194,47,78,.06)" }, pat);
    el("rect", { width: 2.6, height: 7, fill: "rgba(194,47,78,.34)" }, pat);

    // Agent 典型输入带
    const bandX0 = X(4096), bandX1 = X(204800);
    el("rect", { x: bandX0, y: y0 - 34, width: bandX1 - bandX0, height: yEnd - y0 + 48, fill: "rgba(194,47,78,.055)", stroke: "rgba(194,47,78,.4)", "stroke-width": 1, "stroke-dasharray": "5 3" }, svg);
    txt(svg, (bandX0 + bandX1) / 2, y0 - 44, "Agent 典型输入长度带：RAG + 工具返回 + 对话历史 ≈ 4K–200K token", { size: 11.5, anchor: "middle", fill: PAL.neg, weight: 700 });

    // 轴与刻度
    const axY = yEnd + 26;
    el("line", { x1: x0, y1: axY, x2: x1, y2: axY, stroke: PAL.ink, "stroke-width": 1.5 }, svg);
    el("path", { d: `M${x1} ${axY} l-8 -4 l0 8 Z`, fill: PAL.ink }, svg);
    [[512, "512"], [8192, "8K"], [32768, "32K"], [131072, "128K"], [524288, "512K"]].forEach(([v, lab]) => {
      el("line", { x1: X(v), y1: axY - 5, x2: X(v), y2: axY + 5, stroke: PAL.ink, "stroke-width": 1.2 }, svg);
      txt(svg, X(v), axY + 20, lab, { size: 10.5, anchor: "middle", fill: PAL.ink, weight: 700 });
    });
    txt(svg, x1, axY + 38, "上下文上限（token，对数刻度）→", { size: 10, anchor: "end", fill: PAL.inkLo });

    // 行：模型阶梯
    const kindColor = m => m.ctx <= 512 ? PAL.red : (m.name.includes("ShieldGemma") ? PAL.copper : "#42566a");
    models.forEach((m, i) => {
      const y = y0 + i * rh;
      const g = el("g", { "data-anim": "1" }, svg);
      const color = kindColor(m);
      // 名称与底座
      txt(g, x0 - 12, y + 5, m.name, { font: SERIF, size: 12.5, anchor: "end", fill: PAL.ink, weight: 700 });
      txt(g, x0 - 12, y + 19, m.base, { size: 9, anchor: "end", fill: PAL.inkLo });
      // 视野条（含闸头）
      el("rect", { x: x0, y: y - 4, width: X(m.ctx) - x0, height: bh, fill: color, opacity: 0.9, rx: 2 }, g);
      // 闸头小门柱
      el("rect", { x: X(m.ctx) - 3, y: y - 9, width: 5, height: bh + 10, fill: "#051c2c" }, g);
      // 盲区（条右缘 → 带右缘）
      if (m.ctx < 204800) {
        el("rect", { x: X(m.ctx) + 4, y: y - 4, width: bandX1 - X(m.ctx) - 4, height: bh, fill: "url(#blind)" }, g);
      }
      // 上限值标注
      const labFill = m.ctx <= 512 ? PAL.neg : PAL.ink;
      txt(g, X(m.ctx) + 10, y + 9, m.label, { size: 11.5, fill: labFill, weight: 700 });
      drill(g, {
        title: m.name, value: m.label + " token",
        sub: `${m.note}${m.ctx < 204800 ? " —— 红色斜纹段是它看不见的输入区" : ""}`,
        source: "报告 §14 上限核实 [K43][K44][K45]",
      });
    });

    // 注记
    txt(svg, X(512), y0 - 14, "快路径 encoder 全部压在这一格（95% 流量的闸）", { size: 10, anchor: "middle", fill: PAL.neg });
    txt(svg, (x0 + x1) / 2, axY + 56, "红色斜纹 = 被武器化的盲区：护栏只看 512，下游模型有 400K 窗口 [K43]", { font: SERIF, size: 11.5, anchor: "middle", fill: PAL.inkMd, style: "font-style:italic" });

    if (!REDUCE) {
      svg.querySelectorAll("[data-anim]").forEach(n => { n.style.opacity = "0"; n.style.transition = "opacity .7s ease"; });
      const io = new IntersectionObserver(es => es.forEach(e => {
        if (!e.isIntersecting) return; io.disconnect();
        svg.querySelectorAll("[data-anim]").forEach((n, i) => setTimeout(() => n.style.opacity = "1", i * 60));
      }), { threshold: 0.12 });
      io.observe(body);
    }
  })();

  // ══ §12 六种解法 + 按输入类型分流 ══
  (() => {
    const rows = R.ctx_solutions.map(m => {
      const tr = drillRow({
        title: m.name, value: "",
        sub: `机制：${m.mech}。优点：${m.pro}。代价：${m.con}`,
        source: "报告 §14 [K43][K44][K47]",
      });
      if (m.name.includes("两段式")) tr.className = "hl";
      tr.innerHTML = `<td style="font-weight:700">${m.name}</td><td>${m.mech}</td>
        <td style="font-size:12px">${m.pro}</td>
        <td style="color:var(--ink-md);font-size:12px">${m.con}</td>`;
      return tr;
    });
    frameTable("ctx-table", {
      title: "512 墙的六种工程解法：没有银弹，只有按输入类型分流",
      sub: "蓝底行=gpt-oss-safeguard 官方推荐的 hybrid 架构 · 点击行钻取",
      src: "报告 §14 · Sentinel [K44] · Cookbook hybrid [K47]",
      head: ["解法", "机制", "优点", "代价/局限"],
      rows,
    });
  })();
  (() => {
    const rows = R.ctx_routing.map(m => {
      const tr = drillRow({
        title: m.type, value: "",
        sub: m.plan,
        source: "报告 §14 按输入类型实务建议",
      });
      tr.innerHTML = `<td style="font-weight:700">${m.type}</td><td>${m.plan}</td>`;
      return tr;
    });
    frameTable("ctx-routing", {
      title: "按输入类型分流：一句话架构",
      sub: "用户消息→Prompt Guard 2 直过；RAG chunk→逐块 any-hit；工具返回→两段式；对话历史→生成式整段审+增量扫描",
      src: "报告 §14 · 中文换算：1 汉字≈1–2 token，512 token 约容 250–500 汉字",
      head: ["输入类型", "推荐方案"],
      rows,
    });
  })();

  // ══ §13 学术脉络：谱系树表 + 13 篇新增论文表 ══
  (() => {
    const rows = R.lineage.map(m => {
      const tr = drillRow({
        title: "谱系 · " + m.core, value: m.pos,
        sub: `站在谁肩膀上：${m.base} ｜ 被谁改进/打脸/应用：${m.succ}`,
        source: `最终版报告 §15.2 [${m.src}]`,
      });
      tr.innerHTML = `<td style="font-weight:700">${m.core}</td>
        <td style="font-size:12px">${m.base}</td>
        <td style="font-size:12px">${m.succ}</td>
        <td style="color:var(--ink-md);font-size:12px">${m.pos}</td>`;
      return tr;
    });
    frameTable("lineage-table", {
      title: "核心论文谱系树：谁给它铺路，谁继承或打脸它",
      sub: "沿 10 组核心论文的引用网络双向拓展（Semantic Scholar Graph API，2026-07 检索）· 点击行钻取",
      src: "最终版报告 §15.2 · [K3][K6][K13][K16][K22][K36][K48]–[K60]",
      head: ["核心论文", "站在谁肩膀上", "被谁改进 / 打脸 / 应用", "一句话定位"],
      rows,
    });
  })();
  (() => {
    const rows = R.new_papers.map(m => {
      const tr = drillRow({
        title: m.name, value: m.id,
        sub: `${m.gist}。入选理由：${m.why}`,
        source: `最终版报告 §15.3 [${m.src}]`,
      });
      if (m.grp === "打脸作") tr.className = "hl";
      tr.innerHTML = `<td style="font-weight:700">${m.name}</td>
        <td class="num" style="font-size:11.5px">${m.id}</td>
        <td class="num" style="font-size:11.5px">${m.date}</td>
        <td style="font-size:12px">${m.gist}</td>
        <td style="color:var(--ink-md);font-size:12px">${m.why}</td>`;
      return tr;
    });
    frameTable("newpaper-table", {
      title: "扩展论文收录 13 篇：检测与防御 5 · 场景与基准 3 · 打脸作 2 · 其余 3",
      sub: "与前述章节引用不重复（对照约 250 个 arXiv ID 去重）（对照约 250 个 arXiv ID 去重）· 蓝底行=两篇'打脸'作，选型必读 · 点击行钻取",
      src: "最终版报告 §15.3 · 著录见 Sources [K48]–[K60]",
      head: ["论文", "arXiv", "时间/引用", "贡献", "入选理由"],
      rows,
    });
  })();

  // ══ §14 签名图：训练数据规模 × 中文占比（对数规模条形）══
  (() => {
    const host = document.getElementById("train-chart");
    if (!host) return;
    const body = U.frame(host, {
      title: "护栏训练数据：规模差三个数量级，中文占比只有一家够看",
      sub: "横条=训练数据条数（对数刻度）· 颜色=中文占比（红=26.6% 中文 · 铜=中英混合 · 蓝灰=仅英语）· Prompt Guard 1/2 与 Llama Guard 3 未披露数据量不入图 · 点击横条钻取",
      src: "报告 §17.1 · Qwen3Guard [K6] · ProtectAI [K66] · WildGuard [K61] · Aegis 2.0 [K65] · AgentDoG 1.5 [K36] · InjecGuard/MOF [K63]",
    });
    const rows = R.train_data.filter(m => typeof m.size === "number").sort((a, b) => b.size - a.size);
    const W = 920, H = 96 + rows.length * 46 + 76;
    const svg = el("svg", { viewBox: `0 0 ${W} ${H}` }, body);
    svg.style.cssText = "width:100%;height:auto;display:block";
    const x0 = 218, x1 = W - 150;
    const lMin = Math.log10(500), lMax = Math.log10(2000000);
    const X = v => x0 + (Math.log10(v) - lMin) / (lMax - lMin) * (x1 - x0);
    const y0 = 64, rh = 46, bh = 20;
    const yEnd = y0 + rows.length * rh;
    // 轴
    const axY = yEnd + 18;
    [[1000, "1K"], [10000, "10K"], [100000, "100K"], [1000000, "1M"]].forEach(([v, lab]) => {
      el("line", { x1: X(v), y1: y0 - 22, x2: X(v), y2: axY + 6, stroke: PAL.lineLo, "stroke-width": 1 }, svg);
      txt(svg, X(v), axY + 22, lab, { size: 10.5, anchor: "middle", fill: PAL.ink, weight: 700 });
    });
    el("line", { x1: x0, y1: axY, x2: x1, y2: axY, stroke: PAL.ink, "stroke-width": 1.5 }, svg);
    txt(svg, x1, axY + 38, "训练数据条数（对数刻度）→", { size: 10, anchor: "end", fill: PAL.inkLo });
    const colorOf = m => m.zh > 0 ? PAL.neg : (m.zh === -1 ? PAL.copper : "#42566a");
    rows.forEach((m, i) => {
      const y = y0 + i * rh;
      const g = el("g", { "data-anim": "1" }, svg);
      const c = colorOf(m);
      txt(g, x0 - 12, y + 4, m.name, { font: SERIF, size: 13, anchor: "end", fill: PAL.ink, weight: 700 });
      txt(g, x0 - 12, y + 18, m.lang, { size: 9, anchor: "end", fill: PAL.inkLo });
      el("rect", { x: x0, y: y - 6, width: Math.max(3, X(m.size) - x0), height: bh, fill: c, opacity: 0.92, rx: 2 }, g);
      txt(g, X(m.size) + 10, y + 9, m.slabel, { size: 11.5, fill: m.hl ? PAL.neg : PAL.ink, weight: 700 });
      const zhLab = m.zh > 0 ? `中文 ${m.zh}%` : (m.zh === -1 ? "中英混合" : "仅英语");
      txt(g, x1 + 130, y + 9, zhLab, { size: 11, anchor: "end", fill: m.zh !== 0 ? c : PAL.inkLo, weight: m.zh !== 0 ? 700 : 400 });
      drill(g, {
        title: m.name + " · 训练数据", value: m.slabel,
        sub: `构成：${m.comp} ｜ 语言：${m.lang} ｜ 公开：${m.pub} ｜ 标注：${m.label}`,
        source: `报告 §17.1 [${m.src}]`,
      });
    });
    txt(svg, (x0 + x1) / 2, H - 14, "数据质量 > 数量：AgentDoG 1.5 约 1k 影响函数净化样本即击败大模型；MOF 仅 1,000 条纠偏即把平均准确率拉到 83.48% [K36][K63]", { font: SERIF, size: 11.5, anchor: "middle", fill: PAL.inkMd, style: "font-style:italic" });
    if (!REDUCE) {
      svg.querySelectorAll("[data-anim]").forEach(n => { n.style.opacity = "0"; n.style.transition = "opacity .7s ease"; });
      const io = new IntersectionObserver(es => es.forEach(e => {
        if (!e.isIntersecting) return; io.disconnect();
        svg.querySelectorAll("[data-anim]").forEach((n, i) => setTimeout(() => n.style.opacity = "1", i * 70));
      }), { threshold: 0.12 });
      io.observe(body);
    }
  })();

  // ══ §14 九模型训练数据明细表 + 自建/微调指南表 ══
  (() => {
    const rows = R.train_data.map(m => {
      const tr = drillRow({
        title: m.name + " · 训练数据", value: m.slabel,
        sub: `构成：${m.comp} ｜ 语言：${m.lang} ｜ 公开：${m.pub} ｜ 标注：${m.label}`,
        source: `报告 §17.1 [${m.src}]`,
      });
      if (m.hl) tr.className = "hl";
      tr.innerHTML = `<td style="font-weight:700">${m.name}</td>
        <td class="num">${m.slabel}</td>
        <td style="font-size:12px">${m.comp}</td>
        <td style="font-size:12px">${m.lang}</td>
        <td style="font-size:12px">${m.pub}</td>
        <td style="color:var(--ink-md);font-size:11.5px">${m.label}</td>`;
      return tr;
    });
    frameTable("traindata-table", {
      title: "九个代表性护栏的训练数据明细：公开配方的是少数",
      sub: "蓝底行=两个极端：Qwen3Guard 规模最大且中文占比最高 / AgentDoG 1.5 最小但质量取胜 · 点击行钻取",
      src: "报告 §17.1 · 各模型技术报告/模型卡（日期见 K 表）",
      head: ["模型", "数据量", "构成", "语言", "公开否", "标注方式"],
      rows,
    });
  })();
  (() => {
    const rows = R.finetune.map(m => {
      const tr = drillRow({
        title: "自建指南 · " + m.dim, value: m.v, sub: m.note,
        source: "报告 §17.3–17.4",
      });
      if (m.dim === "LoRA 成本") tr.className = "hl";
      tr.innerHTML = `<td style="font-weight:700">${m.dim}</td>
        <td class="num" style="font-size:12px">${m.v}</td>
        <td style="color:var(--ink-md);font-size:12px">${m.note}</td>`;
      return tr;
    });
    frameTable("finetune-table", {
      title: "自建数据集与微调：主粮选型、中文构造、切分纪律与 LoRA 成本",
      sub: "蓝底行=LoRA 成本账目：$1–2/run、单卡 L40S、1 小时内 · 点击行钻取",
      src: "报告 §17.3–17.4 · [K61][K62][K63][K64][K65][K66][K67]",
      head: ["维度", "结论", "细节"],
      rows,
    });
  })();

  // ══ §14 论文精读：三组 tab + 编辑级条目列表（35 篇）══
  (() => {
    const host = document.getElementById("papers-list");
    if (!host) return;
    const body = U.frame(host, {
      title: "35 篇精读卡：攻击 11 · 检测 12 · 防御架构与评测 12",
      sub: "按攻防主线分三组（tab 切换）· 每篇=研究问题 + 方法核心 + 关键数字 + 对输入侧防护的一句话意义 · arXiv 号可点击跳转 · 点击蓝色关键数字钻取来源",
      src: "报告第 16 章 · 著录见 Sources [K3][K5][K6][K11]–[K16][K22][K26][K35][K36][K48]–[K51][K53][K56][K57][K61][K63][K68]–[K82]",
    });
    const GROUPS = [
      { key: "attack",  name: "Ⅰ · 攻击方法与威胁实证", en: "ATTACK" },
      { key: "detect",  name: "Ⅱ · 检测模型与护栏", en: "DETECT" },
      { key: "defense", name: "Ⅲ · 防御架构与评测方法", en: "DEFENSE & EVAL" },
    ];
    const bar = document.createElement("div"); bar.className = "papers-tabs";
    const list = document.createElement("div"); list.className = "papers-list";
    body.appendChild(bar); body.appendChild(list);
    function render(g) {
      bar.querySelectorAll("button").forEach(b => b.classList.toggle("on", b.dataset.g === g.key));
      list.innerHTML = "";
      R.papers.filter(p => p.grp === g.key).forEach((p, i) => {
        const it = document.createElement("article");
        it.className = "paper-item";
        const link = p.arxiv
          ? `<a href="https://arxiv.org/abs/${p.arxiv}" target="_blank" rel="noopener">arXiv:${p.arxiv}</a>`
          : `<a href="${p.url}" target="_blank" rel="noopener">ACM CCS 2024</a>`;
        it.innerHTML = `<p class="p-kick">${g.en} · ${String(i + 1).padStart(2, "0")}</p>
          <h3 class="p-title">${p.title}</h3>
          <p class="p-info">${p.info} · ${link} · [${p.k}]</p>
          <p class="p-sum">${p.sum}</p>`;
        const chip = document.createElement("button");
        chip.className = "p-metric"; chip.setAttribute("data-drill-keep", "1");
        chip.innerHTML = `<span class="pm-lab">关键数字</span>${p.metric}`;
        chip.addEventListener("click", ev => U.showDrill({
          title: p.title, value: p.metric, sub: p.msub,
          source: `报告第 16 章 · [${p.k}]`, x: ev.clientX, y: ev.clientY,
        }));
        it.appendChild(chip);
        list.appendChild(it);
      });
      if (!REDUCE) {
        const items = list.querySelectorAll(".paper-item");
        items.forEach((n, i) => { n.style.opacity = "0"; n.style.transition = "opacity .5s ease"; setTimeout(() => n.style.opacity = "1", i * 55); });
      }
    }
    GROUPS.forEach(g => {
      const b = document.createElement("button");
      b.dataset.g = g.key;
      const n = R.papers.filter(p => p.grp === g.key).length;
      b.innerHTML = `${g.name} <span class="pt-n">${n}</span>`;
      b.addEventListener("click", () => render(g));
      bar.appendChild(b);
    });
    render(GROUPS[0]);
  })();
})();
