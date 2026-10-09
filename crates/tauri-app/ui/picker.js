// Who runs the agents, and how: the model picker (Auto or a coding CLI, with
// favourites and a search), the effort slider, and the permissions the CLIs
// run with. On Home it unfolds from the composer itself (Picker.mount); in
// Settings it opens as a menu (Picker.open).
//
// The effort slider follows Egoist's mygo effort-slider (MIT): a pink to
// violet fill under a field of shimmering pixels that brighten toward the
// brain, and at the top level a galaxy (rays, sparkles, a glow and a burst)
// drawn over the pixels.

var Effort = (() => {
  // Each CLI's own levels and names; Auto is the router's ladder.
  const SETS = {
    claude: { levels: [["low", "Low"], ["medium", "Medium"], ["high", "High"], ["xhigh", "Extra high"], ["max", "Max"], ["ultra", "Ultracode"]], rec: "high", tip: "How hard Claude Code thinks before it acts (its --effort). Ultracode is its top level." },
    codex: { levels: [["minimal", "Minimal"], ["low", "Low"], ["medium", "Medium"], ["high", "High"], ["xhigh", "Extra high"]], rec: "medium", tip: "Codex's reasoning effort (model_reasoning_effort)." },
    auto: { levels: [["low", "Low"], ["medium", "Medium"], ["high", "High"], ["xhigh", "Extra"], ["max", "Max"], ["ultra", "Galaxy"]], rec: "medium", tip: "Where workers start on the router's ladder; they climb only when a check fails or a review sends work back." },
  };
  const set = cli => SETS[cli] || null;
  // The nearest level a CLI has (its top for max or ultra, else its default).
  function clamp(cli, e) {
    const s = set(cli);
    if (!s) return e;
    if (s.levels.some(([k]) => k === e)) return e;
    return e === "ultra" || e === "max" || e === "xhigh" ? s.levels[s.levels.length - 1][0] : s.rec;
  }
  const name = (cli, e) => { const s = set(cli); if (!s) return ""; const l = s.levels.find(([k]) => k === clamp(cli, e || s.rec)); return l ? l[1] : ""; };

  const BRAIN = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 3.5a3 3 0 0 0-3 3 3 3 0 0 0-2.5 4.6A3.2 3.2 0 0 0 4.5 16a3 3 0 0 0 4.5 3.2V3.5Z"/><path d="M15 3.5a3 3 0 0 1 3 3 3 3 0 0 1 2.5 4.6 3.2 3.2 0 0 1-1 4.9 3 3 0 0 1-4.5 3.2V3.5Z"/><path class="g" d="M9 8.5c-1 0-2 .7-2 2M9 13c-1.2 0-2.2.8-2.2 2M15 8.5c1 0 2 .7 2 2M15 13c1.2 0 2.2.8 2.2 2"/></svg>`;
  const pos = (k, n) => 6 + (k * 88) / Math.max(1, n - 1);

  function html(cli, level) {
    const s = set(cli);
    if (!s) return `<div class="effort none"><div class="eh"><span>Effort</span></div><p class="note">This CLI picks its own effort.</p></div>`;
    const L = s.levels, i = Math.max(0, L.findIndex(([k]) => k === clamp(cli, level || s.rec))), top = i === L.length - 1;
    const r = L.findIndex(([k]) => k === s.rec);
    return `<div class="effort${top ? " top" : ""}">
      <canvas class="fx" aria-hidden="true"></canvas>
      <div class="eh"><span>Effort</span><b class="lvl">${L[i][1]}</b><span class="sp"></span><span class="q" title="${s.tip}">?</span></div>
      <div class="ends"><span>Faster</span><span>Smarter</span></div>
      <div class="track" role="slider" tabindex="0" aria-label="Effort" aria-valuemin="0" aria-valuemax="${L.length - 1}" aria-valuenow="${i}" aria-valuetext="${L[i][1]}">
        ${L.map((_, k) => `<span class="stop${k === r ? " rec" : ""}" style="left:${pos(k, L.length)}%"></span>`).join("")}
        <span class="knob" style="left:${pos(i, L.length)}%">${BRAIN}</span>
      </div>
      <div class="rec" style="left:${pos(r, L.length)}%">Recommended</div>
    </div>`;
  }

  // ---- drawing (canvas over the card, under the brain)
  const hash = (a, b, c) => { const x = Math.sin(a * 127.1 + b * 311.7 + c * 74.7) * 43758.5453; return x - Math.floor(x); };
  const smooth = (a, b, x) => { const t = Math.min(1, Math.max(0, (x - a) / (b - a))); return t * t * (3 - 2 * t); };
  const shimmer = (c, r, s) => 0.5 + 0.5 * Math.sin((s / (0.5 + hash(c, r, 1))) * Math.PI * 2 + hash(c, r, 2) * 6.28);
  const twinkle = (a, b, s) => { const p = 0.7 + 1.1 * hash(a, b, 3), on = 0.35 + 0.4 * hash(a, b, 4); const k = (((s / p) + hash(a, b, 5)) % 1); return k < on ? Math.sin((k / on) * Math.PI) : 0; };
  const reduced = () => document.documentElement.classList.contains("reduce-motion") || matchMedia("(prefers-reduced-motion: reduce)").matches;
  const GAL = ["#67e8f9", "#a78bfa", "#f472b6", "#ffffff", "#8b91fd"], RAYS = ["#5fb4bd", "#9a9aac", "#b3a8f0"], BURST = ["#5b3cf0", "#67e8f9", "#ffffff", "#f472b6", "#a78bfa"];

  function wire(root, cli, level, onChange) {
    const s = set(cli), el = root.querySelector(".effort");
    if (!s || !el || el.classList.contains("none")) return () => {};
    const L = s.levels, track = el.querySelector(".track"), cv = el.querySelector(".fx"), knob = el.querySelector(".knob");
    let i = Math.max(0, L.findIndex(([k]) => k === clamp(cli, level || s.rec)));
    let shown = i, from = i, at = performance.now(), topAt = i === L.length - 1 ? -1e9 : 0, raf = 0, alive = true;
    const isTop = () => i === L.length - 1;

    function frame(now) {
      if (!alive) return;
      const k = Math.min(1, (now - at) / 450), e = 1 - Math.pow(1 - k, 4);
      shown = from + (i - from) * e;
      knob.style.left = pos(shown, L.length) + "%";
      const box = el.getBoundingClientRect(), tr = track.getBoundingClientRect(), dpr = devicePixelRatio || 1;
      const W = Math.round(box.width * dpr), H = Math.round(box.height * dpr);
      if (!W || !H) { raf = requestAnimationFrame(frame); return; }
      if (cv.width !== W || cv.height !== H) { cv.width = W; cv.height = H; }
      const ctx = cv.getContext("2d");
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, box.width, box.height);
      const x0 = tr.left - box.left, y0 = tr.top - box.top, tw = tr.width, th = tr.height;
      const kx = x0 + (pos(shown, L.length) / 100) * tw, ky = y0 + th / 2;
      const secs = (reduced() ? 0 : now) / 1000;
      const g = isTop() ? Math.min(1, (now - topAt) / 160) : 0;
      // 1. The fill: always under the pixels.
      ctx.save();
      ctx.beginPath(); ctx.roundRect(x0, y0, Math.max(0, kx - x0), th, 8); ctx.clip();
      const grad = ctx.createLinearGradient(x0, 0, kx, 0);
      grad.addColorStop(0, g ? "#1c1846" : "#4a2a3e"); grad.addColorStop(1, g ? "#3b2bb0" : "#9f5a7a");
      ctx.globalAlpha = 0.85; ctx.fillStyle = grad; ctx.fillRect(x0, y0, kx - x0, th);
      // 2. Shimmering pixels, denser and brighter toward the brain.
      const pitch = 4, rows = Math.floor(th / pitch);
      for (let c = 0; c * pitch + x0 < kx - 6; c++) {
        const x = x0 + c * pitch + 1, u = (x - x0) / Math.max(1, kx - x0);
        for (let r = 0; r < rows; r++) {
          if (hash(c, r, 0) > 0.45 + u) continue;
          const bright = 0.12 + 0.88 * smooth(0.3, 0.95, u);
          ctx.globalAlpha = bright * (0.45 + 0.55 * shimmer(c, r, secs));
          ctx.fillStyle = g ? GAL[Math.floor(hash(c, r, 7) * GAL.length)] : (u > 0.75 ? "#ffc2dc" : u > 0.4 ? "#e48bb4" : "#b06f9a");
          ctx.fillRect(x, y0 + r * pitch + 1, 3, 3);
        }
      }
      ctx.restore();
      // 3. The galaxy, over the pixels: glow, rays, sparkles, the burst.
      if (g > 0) {
        const glow = ctx.createRadialGradient(kx, ky, 0, kx, ky, 34);
        glow.addColorStop(0, `rgba(124,116,255,${0.55 * g})`); glow.addColorStop(1, "rgba(124,116,255,0)");
        ctx.globalAlpha = 1; ctx.fillStyle = glow; ctx.fillRect(kx - 40, ky - 40, 80, 80);
        for (let n = 0; n < 16; n++) {
          const ang = (n / 16) * Math.PI * 2 + (hash(n, 0, 6) - 0.5) * 0.3, len = 10 + 26 * hash(n, 0, 6) * hash(n, 0, 12);
          const lit = 0.4 + 0.6 * twinkle(n, 1, secs * 1.6);
          ctx.fillStyle = RAYS[n % RAYS.length];
          for (let d = 0; d < len; d += 2) {
            ctx.globalAlpha = (1 - d / len) * 0.65 * lit * g;
            ctx.fillRect(kx + Math.cos(ang) * (19 + d), ky + Math.sin(ang) * (19 + d), 1.2, 1.2);
          }
        }
        for (let n = 0; n < 10; n++) {
          const a = twinkle(n, 99, secs * 1.3) * g;
          if (a < 0.05) continue;
          ctx.globalAlpha = a; ctx.fillStyle = n % 3 ? "#ffffff" : "#7dd3fc";
          ctx.fillRect(kx + (hash(n, 2, 8) - 0.5) * 20, ky + (hash(n, 3, 8) - 0.5) * 18, 1.6, 1.6);
        }
        const bk = (now - topAt) / 650;
        if (bk > 0 && bk < 1) {
          const ease = 1 - Math.pow(1 - bk, 3);
          for (let n = 0; n < 28; n++) {
            const ang = (n / 28) * Math.PI * 2, r = 11 + (6 + 12 * hash(n, 1, 9)) * ease * 1.6;
            ctx.globalAlpha = 1 - bk * bk; ctx.fillStyle = BURST[n % BURST.length];
            ctx.fillRect(kx + Math.cos(ang) * r * 1.05 - 1, ky + Math.sin(ang) * r * 0.95 - 1, 2, 2);
          }
        }
      }
      ctx.globalAlpha = 1;
      if (!reduced() || k < 1) raf = requestAnimationFrame(frame);
    }
    function setI(n, fire = true) {
      n = Math.max(0, Math.min(L.length - 1, n));
      if (n === i) return;
      from = shown; i = n; at = performance.now();
      if (isTop()) topAt = performance.now();
      el.classList.toggle("top", isTop());
      el.querySelector(".lvl").textContent = L[i][1];
      track.setAttribute("aria-valuenow", i); track.setAttribute("aria-valuetext", L[i][1]);
      cancelAnimationFrame(raf); raf = requestAnimationFrame(frame);
      if (fire) onChange(L[i][0]);
    }
    const atX = e => { const r = track.getBoundingClientRect(); return Math.round(((((e.clientX - r.left) / r.width) * 100 - 6) / 88) * (L.length - 1)); };
    track.addEventListener("pointerdown", e => {
      track.setPointerCapture(e.pointerId); setI(atX(e));
      const mv = ev => setI(atX(ev));
      track.addEventListener("pointermove", mv);
      track.addEventListener("pointerup", () => track.removeEventListener("pointermove", mv), { once: true });
    });
    track.addEventListener("keydown", e => {
      const n = { ArrowRight: i + 1, ArrowUp: i + 1, ArrowLeft: i - 1, ArrowDown: i - 1, Home: 0, End: L.length - 1 }[e.key];
      if (n != null) { e.preventDefault(); setI(n); }
    });
    raf = requestAnimationFrame(frame);
    return () => { alive = false; cancelAnimationFrame(raf); };
  }

  return { html, wire, clamp, name, set };
})();

var Picker = (() => {
  // Model ids the harness knows a CLI by, and back.
  const MODEL = { claude: "claude-code", codex: "codex", cursor: "cursor", grok: "grok", opencode: "opencode" };
  const cliOf = model => (model ? Object.keys(MODEL).find(k => MODEL[k] === model) || "auto" : "auto");
  const PLANS = ["claude", "codex"]; // CLIs the planner can run in
  const av = id => Logos.tile(id, "sm");
  const PERMS = [
    ["supervised", "Supervised", "Ask before commands and file changes.", "lock", "Needs approval prompts inside Backspace; coming next."],
    ["edits", "Auto-accept edits", "Edit files and run tests in the project; nothing else.", "edit"],
    ["auto", "Auto", "The CLI's own reviewer approves or denies each action.", "sparkle"],
    ["full", "Full access", "Anything, with no sandbox. Only on a machine you can throw away.", "bolt"],
  ];
  let favs = [];
  try { favs = JSON.parse(localStorage.getItem("bs.favs") || "[]"); } catch { favs = []; }
  const saveFavs = () => { try { localStorage.setItem("bs.favs", JSON.stringify(favs)); } catch {} };
  const esc1 = s => String(s).replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

  // What the picker lists: Auto, then every coding CLI the scan knows.
  async function items() {
    const hs = await invoke("harnesses").catch(() => []);
    const out = [{ id: "", cli: "auto", name: "Auto", sub: "A model per ticket on your API key; Sonnet leads, Opus advises", ok: true }];
    hs.filter(h => h.kind === "cli" && MODEL[h.id]).forEach(h => {
      const ok = h.installed && h.auth !== "unauthenticated";
      const sub = !h.installed ? "Not installed" : h.auth === "unauthenticated" ? "Not signed in" : (h.detail ? h.detail + " · " : "") + (PLANS.includes(h.id) ? "plans and builds" : "builds; plans on Claude Code or Codex");
      out.push({ id: MODEL[h.id], cli: h.id, name: h.name === "Claude" ? "Claude Code" : h.name, sub, ok, detail: !h.installed ? "Not installed" : h.auth === "unauthenticated" ? "Not signed in" : h.detail || "Signed in" });
    });
    return out;
  }
  const label = async id => ((await items()).find(i => i.id === (id || "")) || { name: "Auto" }).name;

  // Render the picker into `host` and keep it live; returns a function that
  // tears it down. Code chat passes { auto: false }: it talks to one CLI.
  async function render(host, choice, onChange, opts = {}) {
    const chat = opts.auto === false;
    const all = (await items()).filter(i => !chat || i.cli !== "auto").map(i => (chat ? { ...i, sub: i.detail } : i));
    let tab = "all", q = "", stopEffort = null;
    const tabs = [["all", `<span class="ic-all">All</span>`], ["fav", icon("pin")], ...all.map(i => [i.cli, av(i.cli)])];
    const draw = () => {
      const cur = choice.model || "", cli = cliOf(cur);
      const list = all.filter(i => (tab === "all" || (tab === "fav" ? favs.includes(i.cli) : i.cli === tab)) && i.name.toLowerCase().includes(q.toLowerCase()));
      host.innerHTML = `<div class="mp-tabs" role="tablist">${tabs.map(([k, h]) => `<button role="tab" aria-selected="${tab === k}" data-tab="${k}" title="${k === "fav" ? "Favourites" : k === "all" ? "All" : esc1(all.find(i => i.cli === k).name)}">${h}</button>`).join("")}</div>
        <div class="mp-body">
          <div class="mp-left">
            <label class="mp-search">${icon("search")}<input placeholder="Search models…" value="${esc1(q)}" aria-label="Search models"></label>
            <div class="mp-list" role="listbox">${list.map(i => `<div class="mp-item${i.id === cur ? " on" : ""}${i.ok ? "" : " off"}" role="option" aria-selected="${i.id === cur}" aria-disabled="${!i.ok}" data-id="${esc1(i.id)}" tabindex="0">
                ${av(i.cli)}<span class="mp-t"><b>${esc1(i.name)}</b><span>${esc1(i.sub)}</span></span>
                ${i.id === cur ? `<span class="mp-check">${icon("check")}</span>` : ""}
                <button class="mp-star${favs.includes(i.cli) ? " on" : ""}" data-star="${i.cli}" aria-label="${favs.includes(i.cli) ? "Unfavourite" : "Favourite"} ${esc1(i.name)}">★</button></div>`).join("") || `<div class="mp-none">Nothing matches.</div>`}</div>
            ${Effort.html(cli, choice.effort)}
          </div>
          <div class="mp-right"><h6>Permissions</h6>${PERMS.map(([k, t, d, ic, off]) => `<button class="mp-perm${(choice.permission || "edits") === k ? " on" : ""}" data-perm="${k}" ${off ? `disabled title="${esc1(off)}"` : ""}>
              <span class="pi">${icon(ic)}</span><span class="pt"><b>${t}</b><span>${d}</span></span>${(choice.permission || "edits") === k ? `<span class="mp-check">${icon("check")}</span>` : ""}</button>`).join("")}</div>
        </div>`;
      const inp = host.querySelector(".mp-search input");
      inp.oninput = () => { q = inp.value; draw(); const n = host.querySelector(".mp-search input"); n.focus(); n.setSelectionRange(q.length, q.length); };
      host.querySelectorAll("[data-tab]").forEach(b => (b.onclick = () => { tab = b.dataset.tab; draw(); }));
      host.querySelectorAll("[data-star]").forEach(b => (b.onclick = e => {
        e.stopPropagation();
        const c = b.dataset.star; favs = favs.includes(c) ? favs.filter(x => x !== c) : [...favs, c]; saveFavs(); draw();
      }));
      host.querySelectorAll(".mp-item").forEach(r => {
        const pick = () => {
          if (r.getAttribute("aria-disabled") === "true") return;
          const model = r.dataset.id || null;
          // Each CLI has its own levels: keep the nearest one.
          choice = { ...choice, model, effort: Effort.clamp(cliOf(model), choice.effort || (Effort.set(cliOf(model)) || {}).rec || null) };
          onChange(choice); draw();
        };
        r.onclick = pick;
        r.onkeydown = e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pick(); } };
      });
      host.querySelectorAll("[data-perm]").forEach(b => (b.onclick = () => { choice = { ...choice, permission: b.dataset.perm }; onChange(choice); draw(); }));
      if (stopEffort) stopEffort();
      stopEffort = Effort.wire(host, cli, choice.effort, e => { choice = { ...choice, effort: e }; onChange(choice); });
    };
    draw();
    return () => { if (stopEffort) stopEffort(); host.innerHTML = ""; };
  }

  // Unfold inside `host` (Home's composer). Escape or a click outside the
  // composer folds it back; onClose runs then.
  let undo = null;
  async function mount(host, choice, onChange, onClose, opts) {
    if (undo) return close();
    const teardown = await render(host, choice, onChange, opts);
    const box = host.closest(".hc") || host;
    const outside = e => { if (!box.contains(e.target) && !e.target.closest(".pop")) close(); };
    const esc = e => { if (e.key === "Escape") { e.stopPropagation(); close(); } };
    addEventListener("pointerdown", outside, true);
    addEventListener("keydown", esc, true);
    undo = () => { teardown(); removeEventListener("pointerdown", outside, true); removeEventListener("keydown", esc, true); undo = null; if (onClose) onClose(); };
    const s = host.querySelector(".mp-search input"); if (s) s.focus();
  }
  function close() { if (undo) undo(); }

  // As a menu under `anchor` (Settings).
  async function open(anchor, choice, onChange) {
    if (undo) return close();
    const pop = document.createElement("div");
    pop.className = "mpick";
    pop.setAttribute("role", "dialog"); pop.setAttribute("aria-label", "Who runs the agents");
    document.body.appendChild(pop);
    const teardown = await render(pop, choice, onChange);
    const r = anchor.getBoundingClientRect(), pw = pop.offsetWidth, ph = pop.offsetHeight;
    pop.style.left = Math.max(8, Math.min(innerWidth - pw - 8, r.right - pw)) + "px";
    pop.style.top = (r.bottom + 8 + ph < innerHeight ? r.bottom + 8 : Math.max(8, r.top - ph - 8)) + "px";
    const outside = e => { if (!pop.contains(e.target) && !e.target.closest("[data-mpick]")) close(); };
    const esc = e => { if (e.key === "Escape") { e.stopPropagation(); close(); } };
    addEventListener("pointerdown", outside, true);
    addEventListener("keydown", esc, true);
    undo = () => { teardown(); pop.remove(); removeEventListener("pointerdown", outside, true); removeEventListener("keydown", esc, true); undo = null; };
  }

  return { open, mount, close, label, cliOf, MODEL };
})();
