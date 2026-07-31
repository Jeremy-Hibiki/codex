// DOM 表型图表：攻击映射热力矩阵 · 开源模型对比 · 非模型技术 · 推荐管线 · 冲突口径
(() => {
  const REDUCE = matchMedia("(prefers-reduced-motion: reduce)").matches;
  function frameTable(hostId, { title, sub, src, head, rows, hlCol }) {
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
        rows_.forEach((r, i) => setTimeout(() => r.style.opacity = "1", i * 60));
      }), { threshold: 0.12 });
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

  // ── 攻击 → 检测映射热力矩阵（P5 变体：按检测层着色）──
  (() => {
    const layerStyle = {
      norm:    ["归一化层", "background:rgba(34,81,255,.13)"],
      rule:    ["规则层",     "background:rgba(34,81,255,.07)"],
      model:   ["模型判定",   "background:rgba(18,51,184,.10)"],
      session: ["会话级",     "background:rgba(125,155,255,.16)"],
      output:  ["输出侧",     "background:rgba(5,28,44,.06)"],
      arch:    ["架构层",     "background:rgba(194,47,78,.08)"],
    };
    const rows = window.RPT.attack_map.map(m => {
      const tr = drillRow({
        title: m.atk, value: layerStyle[m.cls][0],
        sub: "最有效手段：" + m.det + "。依据：" + m.basis,
        source: "报告 §2.5 攻击→检测映射表",
      });
      tr.innerHTML = `<td style="font-weight:700">${m.atk}</td>
        <td><span style="display:inline-block;padding:1px 8px;border-radius:3px;font-family:var(--mono);font-size:10px;letter-spacing:.06em;${layerStyle[m.cls][1]}">${layerStyle[m.cls][0]}</span> ${m.det}</td>
        <td style="color:var(--ink-md);font-size:12px">${m.basis}</td>`;
      return tr;
    });
    frameTable("attack-matrix", {
      title: "十几个攻击族，六个完全不同的检测面",
      sub: "色块=该攻击最有效的检测层 · 同一层色=可共用同一组件 · 点击行钻取依据",
      src: "报告 §2.5 · 各攻击族来源见 K12–K17, K26, K27",
      head: ["攻击类别", "最有效检测/缓解手段", "关键依据"],
      rows,
    });
  })();

  // ── 开源模型对比矩阵 ──
  (() => {
    const rows = window.RPT.oss_models.map(m => {
      const tr = drillRow({
        title: m.name, value: m.params,
        sub: `${m.license} · 中文：${m.zh} · ${m.note}` +
          (m.pint ? ` · PINT ${m.pint}%` : "") + (m.agentdojo != null ? ` · AgentDojo ${m.agentdojo}%` : ""),
        source: "报告 §5.1 · PINT [K1] · AgentDojo [K4][K30]",
      });
      if (m.name.includes("Qwen3Guard")) tr.className = "hl";
      tr.innerHTML = `<td style="font-weight:700">${m.name}</td><td class="num">${m.params}</td>
        <td>${m.license}</td><td>${m.zh}</td>
        <td class="num">${m.pint != null ? m.pint + "%" : "—"}</td>
        <td class="num">${m.agentdojo != null ? m.agentdojo + "%" : "—"}</td>
        <td class="num">${m.tpr_low}</td><td class="num">${m.lat}</td>`;
      return tr;
    });
    frameTable("oss-matrix", {
      title: "自托管开源分类器：参数、许可、中文能力与基准一次看清",
      sub: "PINT / AgentDojo / TPR@0.1%FPR 三口径并列 · 蓝底行=中文场景首选 · 点击行钻取",
      src: "报告 §5.1/§5.3 · PINT [K1] · PromptShield [K5] · AgentDojo [K4][K30] · Qwen3Guard [K6][K7]",
      head: ["模型", "参数", "许可", "中文", "PINT", "AgentDojo", "TPR@0.1%FPR", "延迟"],
      rows,
    });
  })();

  // ── 非模型技术横向对比 ──
  (() => {
    const rows = window.RPT.nonmodel_tech.map(m => {
      const tr = drillRow({
        title: m.tech, value: m.eff.split("；")[0].split("（")[0],
        sub: `部署时点：${m.when} · 开销：${m.cost} · 有效性：${m.eff} · 主要绕过：${m.bypass}`,
        source: "报告 §6.9 横向对比表 [K14][K16][K17]",
      });
      if (m.tech.includes("归一化") || m.tech.includes("Spotlighting")) tr.className = "hl";
      tr.innerHTML = `<td style="font-weight:700">${m.tech}</td><td>${m.when}</td>
        <td>${m.eff}</td><td class="num">${m.cost}</td>
        <td style="color:var(--ink-md);font-size:12px">${m.bypass}</td>`;
      return tr;
    });
    frameTable("nonmodel-table", {
      title: "不依赖检测模型的十二项技术：有效性、开销与绕过方式",
      sub: "蓝底行=便宜到没有理由不做的两项 · 点击行钻取",
      src: "报告 §6.9 · Spotlighting [K14] · CaMeL [K16] · canary 实测 [K17]",
      head: ["技术", "部署时点", "有效性（代表数据）", "开销", "主要绕过方式"],
      rows,
    });
  })();

  // ── 推荐分层管线表 ──
  (() => {
    const rows = window.RPT.pipeline.map(p => {
      const tr = drillRow({
        title: p.layer + " 层", value: p.lat,
        sub: p.comp + "。职责：" + p.duty,
        source: "报告 §10.2 推荐架构",
      });
      if (p.layer === "L1") tr.className = "hl";
      tr.innerHTML = `<td class="num" style="font-weight:700;color:var(--red)">${p.layer}</td>
        <td>${p.comp}</td><td class="num">${p.lat}</td><td>${p.duty}</td>`;
      return tr;
    });
    frameTable("pipeline-table", {
      title: "推荐架构：网关强制入口的四层检测管线（Go 实现）",
      sub: "L1 行蓝底=中文流量必选 Qwen3Guard-Stream-0.6B · 一张 T4/A10G 承载 L1+L2 共池 · 点击行钻取",
      src: "报告 §10.2 · 容量结论见 §8.6",
      head: ["层", "组件", "延迟", "职责"],
      rows,
    });
  })();

  // ── 冲突口径表 ──
  (() => {
    const rows = window.RPT.conflicts.map(c => {
      const tr = drillRow({
        title: "冲突口径 · " + c.item, value: "A ≠ B",
        sub: `口径 A：${c.a} ｜ 口径 B：${c.b} ｜ 取舍：${c.resolution}`,
        source: "报告 §11.2 冲突口径并列",
      });
      tr.innerHTML = `<td style="font-weight:700">${c.item}</td>
        <td>${c.a}</td><td>${c.b}</td>
        <td style="color:var(--ink-md);font-size:12px">${c.resolution}</td>`;
      return tr;
    });
    frameTable("conflict-table", {
      title: "口径冲突：并列呈现，不取单一叙事",
      sub: "版本 / 硬件 / 评测口径差异 · 点击行钻取完整取舍逻辑",
      src: "报告 §11.2 · 相关来源 [K1][K8][K16][K4]",
      head: ["冲突项", "口径 A", "口径 B", "解释与取舍"],
      rows,
    });
  })();
})();
