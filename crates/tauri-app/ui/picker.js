// Who runs the agents, and how: the model menu (a dropdown of coding CLIs
// and Auto, with favourites, a search, the effort slider and the
// permissions they run with) and the effort slider itself.
//
// Picker.open(anchor, { model, permission, effort }, onChange) shows the menu
// under `anchor`; onChange gets the whole choice after every change.

var Effort = (() => {
  const LEVELS = ["low", "medium", "high", "xhigh", "max", "ultra"];
  const NAMES = { low: "Low", medium: "Medium", high: "High", xhigh: "Extra", max: "Max", ultra: "Galaxy" };
  const REC = "high";
  const BAYER4 = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map(v => (v + 0.5) / 16);
  const reduced = () => document.documentElement.classList.contains("reduced") || matchMedia("(prefers-reduced-motion: reduce)").matches;
  const BRAIN = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 3.5a3 3 0 0 0-3 3 3 3 0 0 0-2.5 4.6A3.2 3.2 0 0 0 4.5 16a3 3 0 0 0 4.5 3.2V3.5Z"/><path d="M15 3.5a3 3 0 0 1 3 3 3 3 0 0 1 2.5 4.6 3.2 3.2 0 0 1-1 4.9 3 3 0 0 1-4.5 3.2V3.5Z"/><path class="g" d="M9 8.5c-1 0-2 .7-2 2M9 13c-1.2 0-2.2.8-2.2 2M15 8.5c1 0 2 .7 2 2M15 13c1.2 0 2.2.8 2.2 2"/></svg>`;

  function html(level) {
    const i = Math.max(0, LEVELS.indexOf(level || REC));
    return `<div class="effort${LEVELS[i] === "ultra" ? " galaxy" : ""}">
      <div class="eh"><span>Effort</span><b class="lvl">${NAMES[LEVELS[i]]}</b><span class="sp"></span><span class="q" title="How hard the agents think before acting. Higher is slower, uses more of your plan, and gets harder problems right.">?</span></div>
      <div class="ends"><span>Faster</span><span>Smarter</span></div>
      <div class="track" role="slider" tabindex="0" aria-label="Effort" aria-valuemin="0" aria-valuemax="${LEVELS.length - 1}" aria-valuenow="${i}" aria-valuetext="${NAMES[LEVELS[i]]}">
        <canvas class="fill"></canvas>
        ${LEVELS.map((_, k) => `<span class="stop" style="left:${pos(k)}%"></span>`).join("")}
        <span class="knob" style="left:${pos(i)}%">${BRAIN}<span class="rays"></span></span>
      </div>
      <div class="rec" style="left:${pos(LEVELS.indexOf(REC))}%">Recommended</div>
    </div>`;
  }
  const pos = k => 6 + (k * 88) / (LEVELS.length - 1);

  // Pixels up to the knob: a dithered ramp that brightens toward it; at
  // Galaxy, a field of coloured stars that twinkles.
  function paint(cv, i, t = 0) {
    const r = cv.getBoundingClientRect(), cell = 4;
    const w = Math.max(1, Math.round(r.width / cell)), h = Math.max(1, Math.round(r.height / cell));
    if (cv.width !== w || cv.height !== h) { cv.width = w; cv.height = h; }
    const ctx = cv.getContext("2d");
    ctx.clearRect(0, 0, w, h);
    const end = Math.round((pos(i) / 100) * w);
    const galaxy = LEVELS[i] === "ultra";
    const STAR = ["#5b7cff", "#8f6bff", "#c86bff", "#ff6ad5", "#9fd0ff", "#ffffff"];
    for (let x = 0; x < end; x++) {
      const p = x / Math.max(1, end);
      for (let y = 0; y < h; y++) {
        const th = BAYER4[(y & 3) * 4 + (x & 3)];
        if (galaxy) {
          const seed = Math.sin((x * 12.9898 + y * 78.233) * 43758.5453) % 1;
          const tw = 0.5 + 0.5 * Math.sin(t / 380 + Math.abs(seed) * 40);
          if (Math.abs(seed) * (1.1 - p) > 0.55 * tw) continue;
          ctx.globalAlpha = 0.35 + 0.65 * p * tw;
          ctx.fillStyle = STAR[Math.floor(Math.abs(seed) * 977) % STAR.length];
        } else {
          // Every cell lit: three colours dithered along the ramp.
          const k = p * 2 + th * 0.9 - 0.45;
          ctx.globalAlpha = 0.55 + 0.45 * p;
          ctx.fillStyle = k > 1.35 ? "#ff7ab8" : k > 0.7 ? "#c25aa6" : "#6b4a8c";
        }
        ctx.fillRect(x, y, 1, 1);
      }
    }
    ctx.globalAlpha = 1;
  }

  function wire(root, level, onChange) {
    const el = root.querySelector(".effort"), track = el.querySelector(".track"), cv = el.querySelector(".fill");
    let i = Math.max(0, LEVELS.indexOf(level || REC)), raf = 0;
    const loop = t => { paint(cv, i, t); raf = requestAnimationFrame(loop); };
    const set = (k, fire = true) => {
      i = Math.max(0, Math.min(LEVELS.length - 1, k));
      el.classList.toggle("galaxy", LEVELS[i] === "ultra");
      el.querySelector(".lvl").textContent = NAMES[LEVELS[i]];
      el.querySelector(".knob").style.left = pos(i) + "%";
      track.setAttribute("aria-valuenow", i); track.setAttribute("aria-valuetext", NAMES[LEVELS[i]]);
      cancelAnimationFrame(raf);
      if (LEVELS[i] === "ultra" && !reduced()) raf = requestAnimationFrame(loop); else paint(cv, i);
      if (fire) onChange(LEVELS[i]);
    };
    const at = e => { const r = track.getBoundingClientRect(); const pct = ((e.clientX - r.left) / r.width) * 100; return Math.round(((pct - 6) / 88) * (LEVELS.length - 1)); };
    track.addEventListener("pointerdown", e => {
      track.setPointerCapture(e.pointerId); set(at(e));
      const mv = ev => { const k = at(ev); if (k !== i) set(k); };
      track.addEventListener("pointermove", mv);
      track.addEventListener("pointerup", () => track.removeEventListener("pointermove", mv), { once: true });
    });
    track.addEventListener("keydown", e => {
      const k = { ArrowRight: i + 1, ArrowUp: i + 1, ArrowLeft: i - 1, ArrowDown: i - 1, Home: 0, End: LEVELS.length - 1 }[e.key];
      if (k != null) { e.preventDefault(); set(k); }
    });
    requestAnimationFrame(() => set(i, false));
    return () => cancelAnimationFrame(raf);
  }

  return { html, wire, LEVELS, NAMES };
})();

var Picker = (() => {
  // Model ids the harness knows a CLI by.
  const MODEL = { claude: "claude-code", codex: "codex", cursor: "cursor", grok: "grok", opencode: "opencode" };
  const PLANS = ["claude", "codex"]; // CLIs the planner can run in
  const LOOK = { claude: ["CL", "#d97757"], codex: ["CX", "#10a37f"], cursor: ["CU", "#4b5563"], grok: ["GK", "#111827"], opencode: ["OP", "#6d28d9"], auto: ["A", "#2a66d9"] };
  const av = id => { const [t, c] = LOOK[id] || ["?", "#555"]; return `<span class="pav sm" style="--av:${c}">${t}</span>`; };
  const PERMS = [
    ["supervised", "Supervised", "Ask before commands and file changes.", "lock", "Needs approval prompts inside Backspace; coming next."],
    ["edits", "Auto-accept edits", "Edit files and run tests in its own worktree; nothing else.", "edit"],
    ["auto", "Auto", "The CLI's own reviewer approves or denies each action.", "sparkle"],
    ["full", "Full access", "Anything, with no sandbox. Only on a machine you can throw away.", "bolt"],
  ];
  let favs = [];
  try { favs = JSON.parse(localStorage.getItem("bs.favs") || "[]"); } catch { favs = []; }
  const saveFavs = () => { try { localStorage.setItem("bs.favs", JSON.stringify(favs)); } catch {} };

  // What the menu lists: Auto, then every coding CLI the scan knows.
  async function items() {
    const hs = await invoke("harnesses").catch(() => []);
    const out = [{ id: "", cli: "auto", name: "Auto", sub: "A model per ticket on your API key; starts cheap, climbs on failure", ok: true }];
    hs.filter(h => h.kind === "cli" && MODEL[h.id]).forEach(h => {
      const ok = h.installed && h.auth !== "unauthenticated";
      const sub = !h.installed ? "Not installed" : h.auth === "unauthenticated" ? "Not signed in" : (h.detail ? h.detail + " · " : "") + (PLANS.includes(h.id) ? "plans and builds" : "builds; plans on Claude Code or Codex");
      out.push({ id: MODEL[h.id], cli: h.id, name: h.name === "Claude" ? "Claude Code" : h.name, sub, ok });
    });
    return out;
  }
  const label = async id => ((await items()).find(i => i.id === (id || "")) || { name: "Auto" }).name;

  let pop = null, stopEffort = null;
  function close() {
    if (stopEffort) stopEffort();
    if (pop) pop.remove();
    pop = null;
    removeEventListener("pointerdown", outside, true);
    removeEventListener("keydown", esc, true);
  }
  const outside = e => { if (pop && !pop.contains(e.target) && !e.target.closest("[data-mpick]")) close(); };
  const esc = e => { if (e.key === "Escape") { e.stopPropagation(); close(); } };

  // opts.under: an element (Home's composer) to sit under, as wide as it.
  async function open(anchor, choice, onChange, opts = {}) {
    if (pop) { close(); return; }
    const all = await items();
    let tab = "all", q = "";
    pop = document.createElement("div");
    pop.className = "mpick";
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-label", "Who runs the agents");
    document.body.appendChild(pop);
    const tabs = [["all", `<span class="ic-all">All</span>`], ["fav", icon("pin")], ...all.map(i => [i.cli, av(i.cli)])];
    const render = () => {
      const list = all.filter(i => (tab === "all" || (tab === "fav" ? favs.includes(i.cli) : i.cli === tab)) && i.name.toLowerCase().includes(q.toLowerCase()));
      pop.innerHTML = `<div class="mp-tabs" role="tablist">${tabs.map(([k, h]) => `<button role="tab" aria-selected="${tab === k}" data-tab="${k}" title="${k === "fav" ? "Favourites" : k === "all" ? "All" : esc1(all.find(i => i.cli === k).name)}">${h}</button>`).join("")}</div>
        <div class="mp-body">
          <div class="mp-left">
            <label class="mp-search">${icon("search")}<input placeholder="Search models…" value="${esc1(q)}" aria-label="Search models"></label>
            <div class="mp-list" role="listbox">${list.map(i => `<div class="mp-item${i.id === (choice.model || "") ? " on" : ""}${i.ok ? "" : " off"}" role="option" aria-selected="${i.id === (choice.model || "")}" aria-disabled="${!i.ok}" data-id="${esc1(i.id)}" tabindex="0">
                ${av(i.cli)}<span class="mp-t"><b>${esc1(i.name)}</b><span>${esc1(i.sub)}</span></span>
                ${i.id === (choice.model || "") ? `<span class="mp-check">${icon("check")}</span>` : ""}
                <button class="mp-star${favs.includes(i.cli) ? " on" : ""}" data-star="${i.cli}" aria-label="${favs.includes(i.cli) ? "Unfavourite" : "Favourite"} ${esc1(i.name)}">★</button></div>`).join("") || `<div class="mp-none">Nothing matches.</div>`}</div>
            ${Effort.html(choice.effort)}
          </div>
          <div class="mp-right"><h6>Permissions</h6>${PERMS.map(([k, t, d, ic, off]) => `<button class="mp-perm${(choice.permission || "edits") === k ? " on" : ""}" data-perm="${k}" ${off ? `disabled title="${esc1(off)}"` : ""}>
              <span class="pi">${icon(ic)}</span><span class="pt"><b>${t}</b><span>${d}</span></span>${(choice.permission || "edits") === k ? `<span class="mp-check">${icon("check")}</span>` : ""}</button>`).join("")}</div>
        </div>`;
      const inp = pop.querySelector(".mp-search input");
      inp.oninput = () => { q = inp.value; render(); const n = pop.querySelector(".mp-search input"); n.focus(); n.setSelectionRange(q.length, q.length); };
      pop.querySelectorAll("[data-tab]").forEach(b => (b.onclick = () => { tab = b.dataset.tab; render(); }));
      pop.querySelectorAll("[data-star]").forEach(b => (b.onclick = e => {
        e.stopPropagation();
        const c = b.dataset.star; favs = favs.includes(c) ? favs.filter(x => x !== c) : [...favs, c]; saveFavs(); render();
      }));
      pop.querySelectorAll(".mp-item").forEach(r => {
        const pick = () => { if (r.getAttribute("aria-disabled") === "true") return; choice = { ...choice, model: r.dataset.id || null }; onChange(choice); render(); };
        r.onclick = pick;
        r.onkeydown = e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pick(); } };
      });
      pop.querySelectorAll("[data-perm]").forEach(b => (b.onclick = () => { choice = { ...choice, permission: b.dataset.perm }; onChange(choice); render(); }));
      if (stopEffort) stopEffort();
      stopEffort = Effort.wire(pop, choice.effort, e => { choice = { ...choice, effort: e }; onChange(choice); });
    };
    render();
    if (opts.under) {
      // Under the composer: scroll it up if the menu would not fit below.
      const room = 470, sc = opts.under.closest(".home-wrap");
      let u = opts.under.getBoundingClientRect();
      if (sc && innerHeight - u.bottom < room) { sc.scrollTop += room - (innerHeight - u.bottom); u = opts.under.getBoundingClientRect(); }
      const w = Math.min(innerWidth - 16, Math.max(u.width, 560));
      pop.style.width = w + "px";
      pop.style.left = Math.max(8, Math.min(innerWidth - w - 8, u.left + (u.width - w) / 2)) + "px";
      pop.style.top = u.bottom + 8 + "px";
      pop.style.maxHeight = Math.max(260, innerHeight - u.bottom - 16) + "px";
    } else {
      const r = anchor.getBoundingClientRect(), pw = pop.offsetWidth, ph = pop.offsetHeight;
      const below = r.bottom + 8 + ph < innerHeight;
      pop.style.left = Math.max(8, Math.min(innerWidth - pw - 8, r.right - pw)) + "px";
      pop.style.top = (below ? r.bottom + 8 : Math.max(8, r.top - ph - 8)) + "px";
    }
    addEventListener("pointerdown", outside, true);
    addEventListener("keydown", esc, true);
    pop.querySelector(".mp-search input").focus();
  }
  const esc1 = s => String(s).replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

  return { open, close, label, MODEL };
})();
