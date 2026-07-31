// main.js · 顶部进度轨 + chips 跳转 + 通用入场
(() => {
  // chips
  document.querySelectorAll("[data-goto]").forEach(b => {
    b.addEventListener("click", () => {
      const t = document.querySelector(b.dataset.goto);
      if (t) t.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
    });
  });

  // era rail：章节分段
  const rail = document.getElementById("era-rail");
  const track = document.getElementById("rail-track");
  const year = document.getElementById("rail-year");
  const secs = [...document.querySelectorAll("main section[data-win], footer#sec-sources")];
  const labels = { "sec-exec": "§0 摘要", "sec-threat": "§1 威胁", "sec-tools": "§2 方案", "sec-landscape": "§3 全景", "sec-nonmodel": "§4 非模型", "sec-bench": "§5 基准", "sec-eng": "§6 级联", "sec-frontier": "§7 前沿", "sec-gptoss": "§8 政策推理", "sec-agentdog": "§9 AgentDoG", "sec-iospec": "§10 I/O", "sec-multiturn": "§11 多轮", "sec-ctxwall": "§12 512墙", "sec-lineage": "§13 脉络", "sec-papers": "§14 精读", "sec-traindata": "§15 数据", "sec-roadmap": "§16 选型", "sec-appendix": "§17 附录", "sec-sources": "来源" };
  const segs = secs.map(s => {
    const d = document.createElement("div");
    d.className = "seg";
    d.innerHTML = `<span class="seg-label">${labels[s.id] || s.id}</span>`;
    d.addEventListener("click", () => s.scrollIntoView({ behavior: "smooth" }));
    track.appendChild(d);
    return { s, d };
  });
  function layoutSegs() {
    const doc = document.documentElement;
    const total = doc.scrollHeight - innerHeight;
    segs.forEach(({ s, d }) => {
      const top = s.offsetTop, h = s.offsetHeight;
      d.style.left = (top / (total + innerHeight) * 100) + "%";
      d.style.width = Math.max(2, h / (total + innerHeight) * 100) + "%";
    });
  }
  layoutSegs();
  addEventListener("resize", layoutSegs);
  addEventListener("load", layoutSegs);

  const spy = new IntersectionObserver(es => es.forEach(e => {
    if (!e.isIntersecting) return;
    rail.classList.add("on");
    segs.forEach(({ s, d }) => d.classList.toggle("active", s === e.target));
    year.textContent = (labels[e.target.id] || "§").split(" ")[0];
  }), { rootMargin: "-25% 0px -60% 0px", threshold: 0 });
  secs.forEach(s => spy.observe(s));
  const coverIO = new IntersectionObserver(es => es.forEach(e => {
    if (e.isIntersecting) rail.classList.remove("on");
  }), { threshold: 0.55 });
  coverIO.observe(document.getElementById("cover"));

  // 通用 prose 入场（轻量）
  const REDUCE = matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (!REDUCE) {
    const els = document.querySelectorAll(".prose h2, .prose .dek, .quote-card, .term-mag .term");
    els.forEach(elm => { elm.style.opacity = "0"; elm.style.transform = "translateY(14px)"; elm.style.transition = "opacity .7s ease, transform .7s ease"; });
    const io2 = new IntersectionObserver(es => es.forEach(e => {
      if (!e.isIntersecting) return;
      io2.unobserve(e.target);
      e.target.style.opacity = "1"; e.target.style.transform = "none";
    }), { threshold: 0.1 });
    els.forEach(elm => io2.observe(elm));
  }
})();
