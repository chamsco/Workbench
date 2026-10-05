// Backspace, Tauri edition. Two halves, switched at the top of the sidebar:
//
// - Chat (chat.js): threads with a model behind each: a coding CLI on this
//   machine, Ollama, a router, or Backspace Cloud.
// - Code: the agent workbench for a project folder, below.
//
// First run walks through setup (onboard.js); providers.js renders the
// CLI/model list both setup and Settings use.
//
// Tabs on the left of the title bar each hold up to four canvases (2x2 at
// four). A canvas shows an agent session, a worktree, a browser, the diagram
// review or PLAN.md; a new canvas starts as a picker. Tickets open in a drawer
// off the review pill; machines switch from the sidebar foot; settings sit
// behind the gear. Tabs, theme and machines persist in the shared prefs file
// the GPUI build reads too.

const TAURI = window.__TAURI__;
const invoke = (cmd, args) => TAURI.core.invoke(cmd, args);
const $ = (s, el = document) => el.querySelector(s);
const $$ = (s, el = document) => [...el.querySelectorAll(s)];
const esc = s => String(s ?? "").replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

// ------------------------------------------------------------------ icons
const P = {
  folder: '<path d="M2 4.6A1.6 1.6 0 0 1 3.6 3h2.9l1.5 1.5h4.4A1.6 1.6 0 0 1 14 6.1v5.3a1.6 1.6 0 0 1-1.6 1.6H3.6A1.6 1.6 0 0 1 2 11.4z"/>',
  sparkle: '<path d="M8 1.6 9.3 5.9 13.6 7 9.3 8.3 8 12.6 6.7 8.3 2.4 7l4.3-1.1z" fill="currentColor" stroke="none"/><circle cx="12.6" cy="12.4" r="1.2" fill="currentColor" stroke="none"/>',
  globe: '<circle cx="8" cy="8" r="5.8"/><path d="M2.2 8h11.6M8 2.2c1.7 1.6 2.6 3.6 2.6 5.8S9.7 12.2 8 13.8M8 2.2C6.3 3.8 5.4 5.8 5.4 8s.9 4.2 2.6 5.8"/>',
  term: '<rect x="2" y="3" width="12" height="10" rx="1.6"/><path d="m4.8 6.4 2 1.6-2 1.6M8.4 10h2.8"/>',
  plus: '<path d="M8 3.2v9.6M3.2 8h9.6"/>',
  window: '<rect x="2" y="3" width="12" height="10" rx="1.6"/><path d="M2 6h12"/>',
  gear: '<circle cx="8" cy="8" r="2.2"/><path d="M8 1.8v1.8M8 12.4v1.8M1.8 8h1.8M12.4 8h1.8M3.6 3.6l1.3 1.3M11.1 11.1l1.3 1.3M3.6 12.4l1.3-1.3M11.1 4.9l1.3-1.3"/>',
  sidebar: '<rect x="2" y="3" width="12" height="10" rx="1.6"/><path d="M6.2 3v10"/>',
  expand: '<path d="M9.6 2.6h3.8v3.8M13.4 2.6 9.2 6.8M6.4 13.4H2.6V9.6M2.6 13.4l4.2-4.2"/>',
  close: '<path d="m4.4 4.4 7.2 7.2M11.6 4.4l-7.2 7.2"/>',
  ext: '<path d="M9.2 2.6h4.2v4.2M13.4 2.6 7.6 8.4M11.6 9.6v3a.8.8 0 0 1-.8.8H3.4a.8.8 0 0 1-.8-.8V5.2a.8.8 0 0 1 .8-.8h3"/>',
  back: '<path d="M10 3.4 5.4 8l4.6 4.6"/>', fwd: '<path d="M6 3.4 10.6 8 6 12.6"/>',
  reload: '<path d="M13 8a5 5 0 1 1-1.5-3.6M13 2.8v2.6h-2.6"/>',
  chev: '<path d="m6 3.6 4 4.4-4 4.4"/>', chevd: '<path d="m3.6 6 4.4 4 4.4-4"/>',
  ticket: '<path d="M2.4 4.4h11.2v2.2a1.4 1.4 0 0 0 0 2.8v2.2H2.4V9.4a1.4 1.4 0 0 0 0-2.8z"/>',
  diagram: '<rect x="1.8" y="2.6" width="4.6" height="3.4" rx=".8"/><rect x="9.6" y="2.6" width="4.6" height="3.4" rx=".8"/><rect x="5.7" y="10" width="4.6" height="3.4" rx=".8"/><path d="M4.1 6v1.8h7.8V6M8 7.8V10"/>',
  doc: '<path d="M4 2.4h5.2L12 5.2v8.4H4z"/><path d="M9 2.4v3h3M6 8.4h4M6 10.8h4"/>',
  branch: '<circle cx="4.6" cy="3.6" r="1.4"/><circle cx="4.6" cy="12.4" r="1.4"/><circle cx="11.4" cy="5.6" r="1.4"/><path d="M4.6 5v6M11.4 7c0 2.6-6.8 1.8-6.8 4"/>',
  bksp: '<path d="M5.2 3.2h7.6a1 1 0 0 1 1 1v7.6a1 1 0 0 1-1 1H5.2L1.8 8z"/><path d="m7.6 6 4 4M11.6 6l-4 4"/>',
  check: '<path d="m3.4 8.4 3 3 6.2-6.6"/>',
  pc: '<rect x="2.4" y="3" width="11.2" height="7.6" rx="1.2"/><path d="M1.4 13h13.2"/>',
  cloud: '<path d="M4.6 12.4h7a2.8 2.8 0 0 0 .4-5.57A4 4 0 0 0 4.3 6.2a3.1 3.1 0 0 0 .3 6.2z"/>',
  pin: '<path d="M6 2.6h4M6.6 2.6v4L4.6 9h6.8l-2-2.4v-4M8 9v4.4"/>',
  copy: '<rect x="5.4" y="5.4" width="8" height="8" rx="1.4"/><path d="M10.6 5.4V3.4a.8.8 0 0 0-.8-.8H3.4a.8.8 0 0 0-.8.8v6.4a.8.8 0 0 0 .8.8h2"/>',
  down: '<path d="M8 2.6v8M4.6 7.4 8 10.8l3.4-3.4M3 13.4h10"/>',
  compose: '<path d="M7.4 2.8H3.6a1 1 0 0 0-1 1v8.6a1 1 0 0 0 1 1h8.6a1 1 0 0 0 1-1V8.6"/><path d="m11.4 2.4 2.2 2.2-5.8 5.8-2.8.6.6-2.8z"/>',
  bubble: '<path d="M8 2.6c3.2 0 5.6 2.1 5.6 4.8S11.2 12.2 8 12.2c-.7 0-1.4-.1-2-.3L3 13.2l.8-2.4C3 10 2.4 8.8 2.4 7.4 2.4 4.7 4.8 2.6 8 2.6z"/>',
  codei: '<circle cx="8" cy="8" r="6"/><path d="M6.4 6 4.6 8l1.8 2M9.6 6l1.8 2-1.8 2"/>',
  search: '<circle cx="7" cy="7" r="4.2"/><path d="m10.2 10.2 3.4 3.4"/>',
  up: '<path d="M8 13V3.4M3.8 7.4 8 3.2l4.2 4.2"/>',
  stop: '<rect x="4.4" y="4.4" width="7.2" height="7.2" rx="1.4" fill="currentColor" stroke="none"/>',
  attach: '<path d="M8 3.2v9.6M3.2 8h9.6"/>',
  reply: '<path d="M6.4 4 2.6 7.6l3.8 3.6M2.8 7.6h6.6a4 4 0 0 1 4 4v.8"/>',
  more: '<circle cx="3.6" cy="8" r=".9" fill="currentColor"/><circle cx="8" cy="8" r=".9" fill="currentColor"/><circle cx="12.4" cy="8" r=".9" fill="currentColor"/>',
  trash: '<path d="M3 4.4h10M6.4 4.4V3h3.2v1.4M4.4 4.4l.6 8.6h6l.6-8.6"/>',
  edit: '<path d="m10.6 2.8 2.6 2.6-7.4 7.4-3.2.6.6-3.2z"/>',
  bolt: '<path d="M8.8 1.8 3.6 9h4l-.8 5.2L12.4 7h-4z"/>',
  chip: '<rect x="4" y="4" width="8" height="8" rx="1.4"/><path d="M6 1.8V4M10 1.8V4M6 12v2.2M10 12v2.2M1.8 6H4M1.8 10H4M12 6h2.2M12 10h2.2"/>',
  route: '<circle cx="4" cy="12" r="1.6"/><circle cx="12" cy="4" r="1.6"/><path d="M5.6 12h3.6a2 2 0 0 0 0-4H6.8a2 2 0 0 1 0-4h3.6"/>',
  warn: '<path d="M8 2.4 14 13H2z"/><path d="M8 6.6v3M8 11.2v.2"/>',
};
const icon = (n, cls = "") => `<svg class="ic ${cls}" viewBox="0 0 16 16" aria-hidden="true">${P[n]}</svg>`;
$$("[data-icon]").forEach(el => (el.innerHTML = icon(el.dataset.icon)));

// ------------------------------------------------------------------ state
let snap = { name: "", agents: [], approvals: [], tickets: [], total_cost_usd: 0, router_cost_usd: 0, workspace: "" };
let prefs = { theme: "system", tabs: [], active_tab: 0, check_updates: true };
let machines = [], share = null, upd = null, hasProject = true;
const S = {
  mode: "chat",
  focus: 0, max: null, side: "projects", review: null,
  drawer: false, pinned: false, ticket: null, settings: false,
  pane: new Map(),         // per-canvas transient state, by tab:index
};
const MAIN = 0;
const agent = id => snap.agents[id];
const pending = () => snap.approvals.filter(a => a.state.state === "pending");
const subtree = id => snap.agents.filter(a => { let c = a; while (c) { if (c.id === id) return true; c = c.parent == null ? null : snap.agents[c.parent]; } return false; }).reduce((s, a) => s + a.cost_usd, 0);
const route = a => {
  const now = a.decision ? `${a.decision.model} @ ${a.decision.effort}` : "not routed";
  const start = a.escalations[0] && a.escalations[0].split(" → ")[0];
  return start ? `${start} → ${now} ↑${a.escalations.length}` : now;
};
const dotFor = st => ({ running: "run", awaiting_approval: "wait", approved: "done", failed: "fail" })[st] || "";
const tab = () => prefs.tabs[prefs.active_tab];
const pstate = i => { const k = prefs.active_tab + ":" + i; if (!S.pane.has(k)) S.pane.set(k, {}); return S.pane.get(k); };
const machine = () => machines.find(m => m.selected) || { name: "Local", local: true, link: { state: "online" } };

let saveT = null;
function saveTabs() {
  clearTimeout(saveT);
  saveT = setTimeout(() => invoke("set_tabs", { tabs: prefs.tabs, active: prefs.active_tab }), 250);
}


// ------------------------------------------------------------------ layout tree
// Mirrors core::layout: {leaf: i} | {dir: "row"|"col", ratio, a, b}. Rust
// owns the format (prefs.json); this copy draws it and applies drags.
const PH = -1;
const L = {
  leaf: i => ({ leaf: i }),
  split: (dir, ratio, a, b) => ({ dir, ratio, a, b }),
  defaultFor(n) {
    const l = L.leaf, s = L.split;
    if (n <= 1) return l(0);
    if (n === 2) return s("row", 0.5, l(0), l(1));
    if (n === 3) return s("row", 1 / 3, l(0), s("row", 0.5, l(1), l(2)));
    return s("col", 0.56, s("row", 0.5, l(0), l(1)), s("row", 0.5, l(2), l(3)));
  },
  preset(id) {
    const l = L.leaf, s = L.split;
    if (id === "top1") return s("col", 0.5, l(0), s("row", 0.5, l(1), l(2)));
    if (id === "bottom1") return s("col", 0.5, s("row", 0.5, l(0), l(1)), l(2));
    if (id === "left1") return s("row", 0.5, l(0), s("col", 0.5, l(1), l(2)));
    return L.defaultFor(+id || 1);
  },
  leaves: t => ("leaf" in t ? [t.leaf] : [...L.leaves(t.a), ...L.leaves(t.b)]),
  valid(t, n) {
    if (!t || typeof t !== "object") return false;
    const l = L.leaves(t).slice().sort((a, b) => a - b);
    return l.length === n && l.every((v, i) => v === i);
  },
  layout(t, r, gap, path = [], out = { leaves: [], gutters: [] }) {
    if ("leaf" in t) { out.leaves.push({ leaf: t.leaf, r }); return out; }
    let ra, g, rb;
    if (t.dir === "row") {
      const wa = Math.max(0, (r.w - gap) * t.ratio);
      ra = { ...r, w: wa }; g = { ...r, x: r.x + wa, w: gap }; rb = { ...r, x: r.x + wa + gap, w: Math.max(0, r.w - gap - wa) };
    } else {
      const ha = Math.max(0, (r.h - gap) * t.ratio);
      ra = { ...r, h: ha }; g = { ...r, y: r.y + ha, h: gap }; rb = { ...r, y: r.y + ha + gap, h: Math.max(0, r.h - gap - ha) };
    }
    out.gutters.push({ path: path.slice(), dir: t.dir, r: g, span: r });
    L.layout(t.a, ra, gap, [...path, false], out);
    L.layout(t.b, rb, gap, [...path, true], out);
    return out;
  },
  setRatio(t, path, v) {
    if (!path.length) { if (!("leaf" in t)) t.ratio = Math.min(0.9, Math.max(0.1, v)); return; }
    L.setRatio(path[0] ? t.b : t.a, path.slice(1), v);
  },
  remove(t, x) {
    if ("leaf" in t) return t.leaf === x ? null : t;
    const a = L.remove(t.a, x), b = L.remove(t.b, x);
    return a && b ? { ...t, a, b } : a || b;
  },
  insert(t, target, side, nw) {
    if ("leaf" in t) {
      if (t.leaf !== target) return t;
      const T = L.leaf(target), N = L.leaf(nw);
      return { left: L.split("row", 0.5, N, T), right: L.split("row", 0.5, T, N), top: L.split("col", 0.5, N, T), bottom: L.split("col", 0.5, T, N) }[side] || N;
    }
    return { ...t, a: L.insert(t.a, target, side, nw), b: L.insert(t.b, target, side, nw) };
  },
  map: (t, f) => ("leaf" in t ? L.leaf(f(t.leaf)) : { ...t, a: L.map(t.a, f), b: L.map(t.b, f) }),
  rename: (t, from, to) => L.map(t, l => (l === from ? to : l)),
  swap: (t, x, y) => L.map(t, l => (l === x ? y : l === y ? x : l)),
  renumber: (t, removed) => L.map(t, l => (l > removed ? l - 1 : l)),
  // The tree after dropping `d` on `side` of `target`, showing it as `shown`.
  moved(t, d, target, side, shown) {
    if (d === target) return L.rename(t, d, shown);
    if (side === "center") return L.rename(L.swap(t, d, target), d, shown);
    const rest = L.remove(t, d);
    return rest ? L.insert(rest, target, side, shown) : L.rename(t, d, shown);
  },
  // Outer quarter on each side splits there; the middle swaps.
  zone(r, x, y) {
    const dx = (x - r.x) / Math.max(1, r.w), dy = (y - r.y) / Math.max(1, r.h);
    const e = [[dx, "left"], [1 - dx, "right"], [dy, "top"], [1 - dy, "bottom"]].sort((a, b) => a[0] - b[0])[0];
    return e[0] < 0.25 ? e[1] : "center";
  },
};
const PRESETS = [["1", "1", 1], ["2", "2", 2], ["3", "3", 3], ["4", "2×2", 4], ["top1", "1 over 2", 3], ["bottom1", "2 over 1", 3], ["left1", "1 | 2", 3]];
const tree = t => (L.valid(t.layout, t.panes.length) ? t.layout : L.defaultFor(t.panes.length));
// A tab's actual arrangement in 16x12, for unnamed tabs and the presets.
function treeIcon(t, cls = "lay-ic") {
  const { leaves } = L.layout(t, { x: 1, y: 1, w: 14, h: 10 }, 1.6);
  return `<svg class="${cls}" viewBox="0 0 16 12" aria-hidden="true">${leaves.map(({ r }) => `<rect x="${r.x}" y="${r.y}" width="${r.w}" height="${r.h}" rx="1.1"/>`).join("")}</svg>`;
}
// Slide elements from where they were to where a DOM change put them.
function flip(els, mutate) {
  const before = new Map(els.map(e => [e, e.getBoundingClientRect().left]));
  mutate();
  els.forEach(e => {
    const dx = before.get(e) - e.getBoundingClientRect().left;
    if (!dx) return;
    e.style.transition = "none"; e.style.transform = `translateX(${dx}px)`;
    requestAnimationFrame(() => { e.style.transition = "transform .16s ease"; e.style.transform = ""; });
  });
}

// ------------------------------------------------------------------ theme + chrome
function applyTheme() {
  const r = document.documentElement;
  if (prefs.theme === "light" || prefs.theme === "dark") r.dataset.theme = prefs.theme; else delete r.dataset.theme;
}
const narrow = () => matchMedia("(max-width: 760px)").matches;
function setSide(open) {
  const w = $("#win");
  if (narrow()) { w.classList.toggle("side-open", open); w.classList.remove("side-closed"); }
  else { w.classList.toggle("side-closed", !open); w.classList.remove("side-open"); }
  $("#sideOpen").hidden = open && !narrow();
}
$("#sideClose").onclick = () => setSide(false);
$("#sideOpen").onclick = () => setSide(true);
$("#scrim").onclick = () => setSide(false);
$("#gear").onclick = () => openSettings();

// Chat and Code share the window; the sidebar, title bar and body follow.
function setMode(m, save = true) {
  // Only what the user picked in setup: one use hides the switch.
  const uses = (prefs.uses && prefs.uses.length ? prefs.uses : ["chat", "code"]);
  if (!uses.includes(m)) m = uses[0];
  $("#win").classList.toggle("single-use", uses.length === 1);
  S.mode = m === "code" ? "code" : "chat";
  const w = $("#win");
  w.classList.toggle("m-chat", S.mode === "chat"); w.classList.toggle("m-code", S.mode === "code");
  $$("#modes [data-mode]").forEach(b => b.setAttribute("aria-selected", b.dataset.mode === S.mode));
  closeSettings();
  $("#chat").hidden = S.mode !== "chat";
  $("#canvas").hidden = S.mode !== "code";
  if (S.drawer && S.mode === "chat") setDrawer(false);
  if (save) { prefs.mode = S.mode; invoke("set_mode", { mode: S.mode }); }
  renderList();
  if (S.mode === "chat") { if (window.Chat) Chat.render(); } else { renderTabs(); renderGrid(); }
}
$$("#modes [data-mode]").forEach(b => (b.onclick = () => setMode(b.dataset.mode)));

let toastT = null;
function toast(text, kind = "") {
  const t = $("#toast");
  t.textContent = text; t.className = "toast " + kind; t.hidden = false;
  clearTimeout(toastT); toastT = setTimeout(() => (t.hidden = true), kind === "err" ? 6000 : 3200);
}
const base = p => String(p).replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;

// ------------------------------------------------------------------ popovers
function openPop(anchor, html, wire) {
  const pop = $("#pop");
  pop.innerHTML = html; pop.hidden = false;
  const r = anchor.getBoundingClientRect(), pw = pop.offsetWidth;
  pop.style.top = r.bottom + 6 + "px";
  pop.style.left = Math.max(8, Math.min(innerWidth - pw - 8, r.left)) + "px";
  wire && wire(pop);
  const first = pop.querySelector("input, button:not([disabled])"); if (first) first.focus();
}
function closePop() { $("#pop").hidden = true; $("#pop").innerHTML = ""; }
addEventListener("pointerdown", e => { if (!$("#pop").hidden && !e.target.closest("#pop") && !e.target.closest("[data-popper]")) closePop(); }, true);

// ------------------------------------------------------------------ sidebar
function treeOrder() {
  const out = [];
  const walk = id => { out.push(id); snap.agents.filter(a => a.parent === id).forEach(c => walk(c.id)); };
  if (snap.agents.length) walk(MAIN);
  return out;
}
function renderBrand() {
  const m = machine(), st = m.link.state;
  $("#brand").innerHTML = `<span class="mic">${icon(m.local ? "pc" : "cloud")}</span>${esc(m.name)}` +
    (m.local ? "" : `<span class="link ${st === "online" ? "" : st}" title="${st === "offline" ? esc("Offline: " + m.link.error) : st}"></span>`);
}
function renderSeg() {
  const items = [["projects", "folder", "Projects"], ["agents", "sparkle", "Agents"]];
  $("#seg").innerHTML = items.map(([k, ic, l]) => `<button aria-pressed="${S.side === k}" data-side="${k}">${icon(ic)}${l}</button>`).join("");
  $$("#seg button").forEach(b => (b.onclick = () => { S.side = b.dataset.side; renderSeg(); renderList(); }));
}
function agentRow(a, lvl) {
  const p = tab().panes[S.focus];
  const sel = p && p.kind === "agent" && p.agent === a.id ? " sel" : "";
  const esc1 = a.escalations.length ? `<span class="esc">↑${a.escalations.length}</span>` : "";
  return `<button class="row ${lvl}${sel}${a.kind === "triage" ? " muted" : ""}" data-open="${a.id}"><span class="agent">${icon("sparkle")}</span><span class="lab">${esc(a.title)}</span><span class="tail">${esc1}<span class="dot ${dotFor(a.status)}"></span></span></button>`;
}
function offlineNote() {
  const m = machine();
  if (m.local || (m.link.state === "online" && snap.agents.length)) return "";
  return m.link.state === "offline"
    ? `<div class="offline-note"><b>${esc(m.name)} is offline.</b><br>${esc(m.link.error)}. Retrying every few seconds.</div>`
    : `<div class="offline-note"><b>Connecting to ${esc(m.name)}…</b></div>`;
}
function renderList() {
  if (S.mode === "chat") { if (window.Chat) Chat.renderThreads(); return; }
  let h = offlineNote();
  if (S.side === "projects") {
    const m = machine();
    if (m.local) {
      h += `<button class="row add" id="openFolder">${icon("plus")}<span class="lab">Open folder…</span><kbd>⌘O</kbd></button>`;
      const cur = hasProject ? snap.workspace : null;
      const recents = (prefs.projects || []);
      if (!recents.length && !cur) h += `<div class="side-empty">No projects yet. Open a folder to put agents to work on it.</div>`;
      recents.forEach(p => {
        const on = p === cur;
        h += `<button class="row group${on ? " sel" : ""}" data-proj="${esc(p)}" title="${esc(p)}">${icon("folder")}<span class="lab">${esc(base(p))}</span>${on ? `<span class="cnt">${snap.agents.length}</span>` : ""}</button>`;
        if (on) treeOrder().forEach(id => (h += agentRow(agent(id), agent(id).depth ? "l2" : "l1")));
      });
    } else if (snap.agents.length) {
      h += `<button class="row group sel">${icon("folder")}<span class="lab">${esc(snap.name)}</span><span class="cnt">${snap.agents.length}</span></button>`;
      treeOrder().forEach(id => (h += agentRow(agent(id), agent(id).depth ? "l2" : "l1")));
    }
    prefs.tabs.forEach((t, ti) => t.panes.forEach((p, pi) => {
      if (p.kind === "browser" && p.url) h += `<button class="row l1" data-pv="${ti}:${pi}"><span class="web">${icon("globe")}</span><span class="lab">Preview · ${esc(p.url.replace(/^https?:\/\//, ""))}</span></button>`;
    }));
    const wts = snap.agents.filter(a => a.branch && a.id !== MAIN);
    if (wts.length) {
      h += `<button class="row group">${icon("folder")}<span class="lab">worktrees</span><span class="cnt">${wts.length}</span></button>`;
      wts.forEach(a => (h += `<button class="row l1" data-files="${a.id}"><span class="term">${icon("branch")}</span><span class="lab">${esc(a.branch)}</span><span class="tail">${esc(a.ticket || "")}</span></button>`));
    }
  } else if (snap.agents.length) {
    h += `<div class="grp-label">${esc(snap.name)} · ${snap.agents.length} agents</div>`;
    treeOrder().forEach(id => { const a = agent(id); h += agentRow(a, "") + `<span class="sub">${esc(route(a))} · $${subtree(a.id).toFixed(3)}</span>`; });
  }
  $("#list").innerHTML = h;
  const of = $("#openFolder"); if (of) of.onclick = pickProject;
  $$("#list [data-proj]").forEach(b => {
    b.onclick = () => { if (b.dataset.proj !== snap.workspace || !hasProject) openProject(b.dataset.proj); };
    b.oncontextmenu = e => { e.preventDefault(); projMenu(b, b.dataset.proj); };
  });
  $$("#list [data-open]").forEach(b => (b.onclick = () => openView({ kind: "agent", agent: +b.dataset.open })));
  $$("#list [data-files]").forEach(b => (b.onclick = () => openView({ kind: "files", agent: +b.dataset.files })));
  $$("#list [data-pv]").forEach(b => (b.onclick = () => { const [t, p] = b.dataset.pv.split(":").map(Number); switchTab(t); S.focus = p; renderGrid(); }));
}
// ------------------------------------------------------------------ projects
async function pickProject() {
  const path = await invoke("pick_folder").catch(() => null);
  if (path) openProject(path);
}
async function openProject(path) {
  const running = snap.agents.filter(a => a.status === "running").length;
  if (hasProject && running && !confirm(`${running} agent${running > 1 ? "s are" : " is"} still running in ${snap.name}. Switching projects stops them. Switch?`)) return;
  toast(`Opening ${base(path)}…`);
  try {
    await invoke("open_project", { path });
    prefs = await invoke("prefs");
    S.pane.clear(); S.review = null; S.ticket = null;
    if (S.mode !== "code") setMode("code");
    await refresh(true);
    toast(`Opened ${base(path)}`, "ok");
  } catch (e) { toast(String(e), "err"); }
}
function projMenu(anchor, path) {
  const open = hasProject && path === snap.workspace;
  openPop(anchor, `<div class="note mono">${esc(path)}</div>
    ${open ? `<button class="pi" id="pmClose">${icon("close")}Close project</button>` : `<button class="pi" id="pmOpen">${icon("folder")}Open</button>`}
    <button class="pi" id="pmForget">${icon("trash")}Remove from recents</button>`, pop => {
    const c = $("#pmClose", pop); if (c) c.onclick = async () => { closePop(); await invoke("close_project"); await refresh(true); };
    const o = $("#pmOpen", pop); if (o) o.onclick = () => { closePop(); openProject(path); };
    $("#pmForget", pop).onclick = async () => { closePop(); if (open) await invoke("close_project"); await invoke("forget_project", { path }); prefs = await invoke("prefs"); await refresh(true); };
  });
}
// Code with no project open: what a project is, and how to start one.
function projectsHome() {
  const recents = (prefs.projects || []).slice(0, 6);
  return `<div class="home">
    <div class="home-mark">${icon("folder")}</div>
    <h1>Open a project</h1>
    <p class="home-lead">Pick a folder and describe what you want built. A main agent plans it into tickets, you approve the plan, and workers build each ticket on its own git branch. You review every deliverable before it merges.</p>
    <div class="home-actions"><button class="btn primary big" id="hOpen">${icon("folder")}Open folder…</button><button class="btn big" id="hChat">${icon("compose")}Just chat instead</button></div>
    ${recents.length ? `<div class="home-recent"><h5>Recent</h5>${recents.map(p => `<button class="recent" data-proj="${esc(p)}">${icon("folder")}<span><b>${esc(base(p))}</b><small>${esc(p)}</small></span></button>`).join("")}</div>` : ""}
    <p class="home-note">${icon("warn")}Opening a folder makes it a git repository if it isn't one, and commits uncommitted changes as a snapshot so agents can branch from it.</p>
  </div>`;
}
function renderCard() {
  if (!hasProject) { $("#ports").hidden = true; return; }
  $("#ports").hidden = false;
  const run = snap.agents.filter(a => a.status === "running").length;
  const wts = snap.agents.filter(a => a.branch && a.id !== MAIN).length;
  const row = (c, l, v) => `<div class="port"><span class="dot ${c}"></span><span class="pl">${l}</span><span class="pn" style="margin-left:auto;color:var(--fg-3)">${v}</span></div>`;
  $("#ports").innerHTML = `<div class="ports-h">This run · ${esc(snap.name || machine().name)}</div>` +
    row(run ? "run" : "acc", "Agents running", run) + row(pending().length ? "wait" : "", "Waiting on you", pending().length) +
    row("done", "Worktrees", wts) + row("", "Spent", `$${snap.total_cost_usd.toFixed(3)} · router $${snap.router_cost_usd.toFixed(3)}`);
}
function renderMachines() {
  $("#machines").innerHTML = machines.map(m => {
    const st = m.link.state, busy = m.running > 0 || m.pending > 0;
    const md = st === "offline" ? "offline" : st === "connecting" ? "connecting" : busy && !m.selected ? "busy" : "";
    const tip = `${m.name}${m.local ? " (this machine)" : " · " + m.url}${m.project ? " · " + m.project : ""}${st === "offline" ? " · offline" : ""}`;
    return `<button class="mach" aria-pressed="${m.selected}" data-m="${m.index}" title="${esc(tip)}" aria-label="${esc(tip)}">${icon(m.local ? "pc" : "cloud")}${md ? `<span class="md ${md}"></span>` : ""}</button>`;
  }).join("") + `<button class="mach" id="addMachine" title="Add a machine" aria-label="Add a machine">${icon("plus")}</button>`;
  $$("#machines [data-m]").forEach(b => (b.onclick = async () => {
    await invoke("select_machine", { index: +b.dataset.m });
    S.pane.clear(); S.review = null; S.ticket = null;
    await refresh(true);
  }));
  $("#addMachine").onclick = () => openSettings("machines");
}

// ------------------------------------------------------------------ title bar: tabs
function renderTabs() {
  const n = pending().length;
  $("#tabs").innerHTML = prefs.tabs.map((t, i) => {
    const badge = n && t.panes.some(p => p.kind === "diagram") ? '<span class="badge"></span>' : "";
    const label = t.name ? esc(t.name) : treeIcon(tree(t));
    return `<button class="tab" role="tab" aria-selected="${i === prefs.active_tab}" data-tab="${i}" aria-label="${esc(t.name || t.panes.length + " canvases")}, drag to move, right-click to rename">${label}${badge}</button>`;
  }).join("") + `<button class="tab plus" id="newTab" data-popper aria-label="New tab" title="New tab (⌘T)">${icon("plus")}</button>`;
  $$("#tabs [data-tab]").forEach(b => {
    const i = +b.dataset.tab;
    b.onclick = () => { if (S.dragged) { S.dragged = false; return; } switchTab(i); };
    b.ondblclick = () => renameTab(i);
    b.oncontextmenu = e => { e.preventDefault(); tabMenu(b, i); };
    b.addEventListener("pointerdown", e => tabDrag(e, b, i));
  });
  $("#newTab").onclick = e => newTabPop(e.currentTarget);
  renderCta();
}
// Drag a tab along the bar: the others slide apart around a dotted slot.
function tabDrag(e, el, i) {
  if (e.button !== 0 || el.classList.contains("editing")) return;
  const sx = e.clientX, sy = e.clientY;
  let d = null;
  const begin = () => {
    const others = $$("#tabs [data-tab]").filter(x => x !== el);
    const mids = others.map(x => { const r = x.getBoundingClientRect(); return r.left + r.width / 2; });
    const r = el.getBoundingClientRect();
    const ph = document.createElement("span");
    ph.className = "tab tph"; ph.style.width = r.width + "px"; ph.innerHTML = icon("plus");
    el.before(ph);
    el.classList.add("tdragging");
    const tr = $("#tabs").getBoundingClientRect();
    el.style.width = r.width + "px"; el.style.top = r.top - tr.top + "px";
    document.body.classList.add("dragging-any");
    return { others, mids, ph, tl: tr.left, dx: sx - r.left, idx: others.findIndex((_, k) => k >= i) < 0 ? others.length : i };
  };
  const move = ev => {
    if (!d) { if (Math.hypot(ev.clientX - sx, ev.clientY - sy) < 5) return; d = begin(); }
    el.style.left = ev.clientX - d.tl - d.dx + "px";
    const idx = d.mids.filter(m => m < ev.clientX).length;
    if (idx === d.idx) return;
    d.idx = idx;
    flip(d.others, () => (idx < d.others.length ? d.others[idx].before(d.ph) : $("#newTab").before(d.ph)));
  };
  const up = () => {
    removeEventListener("pointermove", move); removeEventListener("pointerup", up);
    if (!d) return;
    S.dragged = true; setTimeout(() => (S.dragged = false), 0);
    document.body.classList.remove("dragging-any");
    const active = prefs.tabs[prefs.active_tab];
    const [t] = prefs.tabs.splice(i, 1);
    prefs.tabs.splice(d.idx, 0, t);
    prefs.active_tab = prefs.tabs.indexOf(active);
    S.pane.clear(); saveTabs(); renderTabs(); renderGrid();
  };
  addEventListener("pointermove", move); addEventListener("pointerup", up);
}
function switchTab(i) {
  if (i === prefs.active_tab && !S.settings) return;
  prefs.active_tab = i; S.focus = 0; S.max = null; S.pane.clear(); closeSettings();
  saveTabs(); renderTabs(); renderGrid(); renderList();
}
function tabMenu(anchor, i) {
  openPop(anchor, `<button class="pi" data-a="rename">${icon("doc")}Rename<span class="k">double-click</span></button>
    <button class="pi" data-a="dup">${icon("copy")}Duplicate</button><hr>
    <button class="pi" data-a="close" ${prefs.tabs.length < 2 ? "disabled" : ""}>${icon("close")}Close tab</button>`, pop => {
    pop.querySelector("[data-a=rename]").onclick = () => { closePop(); renameTab(i); };
    pop.querySelector("[data-a=dup]").onclick = () => { closePop(); prefs.tabs.splice(i + 1, 0, JSON.parse(JSON.stringify(prefs.tabs[i]))); switchTab(i + 1); };
    pop.querySelector("[data-a=close]").onclick = () => {
      closePop(); if (prefs.tabs.length < 2) return;
      prefs.tabs.splice(i, 1); S.pane.clear();
      prefs.active_tab = Math.min(prefs.active_tab >= i ? Math.max(0, prefs.active_tab - 1) : prefs.active_tab, prefs.tabs.length - 1);
      saveTabs(); renderTabs(); renderGrid();
    };
  });
}
function renameTab(i) {
  const b = $(`#tabs [data-tab="${i}"]`); if (!b) return;
  const t = prefs.tabs[i];
  b.classList.add("editing");
  b.innerHTML = `<input value="${esc(t.name || "")}" placeholder="${t.panes.length} canvases" aria-label="Tab name" maxlength="40">`;
  const inp = b.querySelector("input"); inp.focus(); inp.select();
  let done = false;
  const commit = keep => { if (done) return; done = true; if (keep) { t.name = inp.value.trim() || null; saveTabs(); } renderTabs(); };
  inp.onkeydown = e => { e.stopPropagation(); if (e.key === "Enter") commit(true); if (e.key === "Escape") commit(false); };
  inp.onblur = () => commit(true);
  inp.onclick = e => e.stopPropagation();
}
function newTabPop(anchor) {
  let pick = "2";
  const lays = () => PRESETS.map(([id, label]) => `<button aria-pressed="${id === pick}" data-p="${id}" aria-label="${label}">${treeIcon(L.preset(id), "")}${label}</button>`).join("");
  openPop(anchor, `<h5>New tab</h5><label class="field"><input id="ntName" placeholder="Name (optional)" maxlength="40"></label>
    <h5>Canvases</h5><div class="lays" id="ntLays">${lays()}</div>
    <div class="row-end"><button class="btn primary" id="ntGo">Create</button></div>`, pop => {
    const wireLays = () => $$("[data-p]", pop).forEach(b => (b.onclick = () => { pick = b.dataset.p; $("#ntLays", pop).innerHTML = lays(); wireLays(); }));
    wireLays();
    const go = () => {
      const name = $("#ntName", pop).value.trim() || null;
      const n = PRESETS.find(p => p[0] === pick)[2];
      prefs.tabs.push({ name, panes: Array.from({ length: n }, () => ({ kind: "empty", agent: 0, url: null })), layout: L.preset(pick) });
      closePop(); switchTab(prefs.tabs.length - 1);
    };
    $("#ntGo", pop).onclick = go;
    $("#ntName", pop).onkeydown = e => { if (e.key === "Enter") go(); if (e.key === "Escape") closePop(); };
  });
}
// Right-hand "+": add a canvas to this tab.
const VIEWS = [["board", "bubble", "Team chat"], ["files", "folder", "Worktree files"], ["browser", "globe", "Browser"], ["diagram", "diagram", "Diagram review"], ["docs", "doc", "PLAN.md"], ["empty", "window", "Empty canvas"]];
$("#addCanvas").dataset.popper = "";
$("#addCanvas").onclick = e => {
  const full = tab().panes.length >= 4;
  const agents = treeOrder().map(id => agent(id));
  openPop(e.currentTarget, full ? `<h5>Add a canvas</h5><div class="note">This tab already has four canvases. Close one, or open a new tab with ⌘T.</div>` :
    `<h5>Sessions</h5>${agents.length ? agents.map(a => `<button class="pi" data-agent="${a.id}"><span class="agent">${icon("sparkle")}</span>${esc(a.id === MAIN ? "Main agent" : a.title)}<span class="k">${esc(a.status.replace("_", " "))}</span></button>`).join("") : `<div class="note">No agents on this machine yet.</div>`}
     <hr><h5>Views</h5>${VIEWS.map(([k, ic, l]) => `<button class="pi" data-kind="${k}">${icon(ic)}${l}</button>`).join("")}`, pop => {
    $$("[data-agent]", pop).forEach(b => (b.onclick = () => addCanvas({ kind: "agent", agent: +b.dataset.agent })));
    $$("[data-kind]", pop).forEach(b => (b.onclick = () => addCanvas({ kind: b.dataset.kind, agent: 0, url: null })));
  });
};
function addCanvas(spec) {
  closePop();
  const t = tab(); if (t.panes.length >= 4) return;
  // Split the focused canvas along its longer side.
  const g = $("#grid"), tr = tree(t), n = t.panes.length;
  const { leaves } = L.layout(tr, { x: 0, y: 0, w: g.clientWidth || 1000, h: g.clientHeight || 600 }, 6);
  const f = leaves.find(l => l.leaf === S.focus) || leaves[0];
  t.layout = L.insert(tr, f.leaf, f.r.w >= f.r.h ? "right" : "bottom", n);
  t.panes.push({ agent: 0, url: null, ...spec }); S.focus = n; S.max = null;
  saveTabs(); closeSettings(); renderTabs(); renderGrid();
}

// ------------------------------------------------------------------ review pill + tickets drawer
function renderCta() {
  const n = pending().length, t = snap.tickets.length;
  $("#cta").innerHTML = (n ? `<span class="dot wait"></span><span class="lbl">${n} to review</span>` : `${icon("check")}<span class="lbl">Nothing to review</span>`)
    + `<span class="n">${t ? t + (t === 1 ? " ticket" : " tickets") : ""}</span>${icon("chev", "chev")}`;
  $("#cta").classList.toggle("hot", n > 0);
  $("#cta").setAttribute("aria-expanded", S.drawer);
}
(() => {
  const cta = $("#cta"), dr = $("#drawer");
  let openT = null, closeT = null;
  const holding = () => S.pinned || dr.contains(document.activeElement);
  const soon = () => { clearTimeout(closeT); closeT = setTimeout(() => { if (!holding()) setDrawer(false); }, 450); };
  // A short delay before opening, so passing over the pill doesn't flash it.
  cta.onmouseenter = () => { clearTimeout(closeT); clearTimeout(openT); openT = setTimeout(() => setDrawer(true), 320); };
  cta.onmouseleave = () => { clearTimeout(openT); soon(); };
  dr.onmouseenter = () => clearTimeout(closeT);
  dr.onmouseleave = soon;
  cta.onclick = () => { clearTimeout(openT); S.pinned = !S.pinned || !S.drawer; setDrawer(S.pinned); };
})();
function setDrawer(open) {
  S.drawer = open; if (!open) S.pinned = false;
  $("#drawer").hidden = !open;
  if (open) renderDrawer();
  renderCta();
}
const T_GROUPS = [["Needs you", ["proposed", "in_review", "ready_for_human", "needs_info"]], ["Running", ["queued", "in_progress"]], ["Ready", ["ready_for_agent", "needs_triage"]], ["Done", ["done"]], ["Closed", ["failed", "wontfix"]]];
const tClass = s => ({ proposed: "s-wait", in_review: "s-wait", ready_for_human: "s-wait", needs_info: "s-wait", queued: "s-run", in_progress: "s-run", done: "s-done", failed: "s-fail", wontfix: "s-fail" })[s] || "s-idle";
const tLabel = s => s.replace(/_/g, " ").replace(/^./, c => c.toUpperCase());
function renderDrawer() {
  if (!S.drawer) return;
  const dr = $("#drawer");
  const keepInput = $("#tfile", dr) ? $("#tfile", dr).value : "";
  const focused = document.activeElement && document.activeElement.id === "tfile";
  const t = S.ticket && snap.tickets.find(x => x.key === S.ticket);
  let body;
  if (t) body = ticketDetail(t);
  else {
    body = `<div class="dr-file"><input id="tfile" placeholder="File a ticket: title: what is wrong or wanted" aria-label="File a ticket"><button class="btn" id="tfileGo">File</button></div>`;
    const pend = pending();
    if (pend.length) body += `<div class="grp-label">Waiting on you · ${pend.length}</div>` + pend.map(a => `<button class="rvrow" data-rv="${a.id}">${icon("diagram")}<span class="tt">${esc(a.kind === "plan" ? `Plan · ${a.tickets.length} tickets` : a.agent === MAIN ? "Final deliverable" : (agent(a.agent) || {}).title || "Deliverable")}</span><span class="sp"></span><span class="st-pill s-wait">review</span></button>`).join("");
    T_GROUPS.forEach(([n, sts]) => {
      const rows = snap.tickets.filter(x => sts.includes(x.state));
      if (!rows.length) return;
      body += `<div class="grp-label">${n} · ${rows.length}</div>` + rows.map(x => `<button class="trow" data-t="${esc(x.key)}"><span class="num">${String(x.num).padStart(2, "0")}</span><span class="tt">${esc(x.title)}</span><span class="st-pill ${tClass(x.state)}">${esc(x.state.replace(/_/g, "-"))}</span></button>`).join("");
    });
    if (!snap.tickets.length && !pend.length) body += `<div class="nothing">No tickets yet. The main agent creates them from your goal; you can file one above.</div>`;
  }
  dr.innerHTML = `<div class="dr-h">${icon("ticket")}<b>Tickets</b><span>· ${esc(snap.name || machine().name)}</span><span class="sp"></span>
    <button class="ib ${S.pinned ? "on" : ""}" id="drPin" title="${S.pinned ? "Unpin" : "Keep open"}" aria-label="Pin">${icon("pin")}</button>
    <button class="ib" id="drClose" aria-label="Close">${icon("close")}</button></div><div class="dr-b">${body}</div>`;
  $("#drPin").onclick = () => { S.pinned = !S.pinned; renderDrawer(); };
  $("#drClose").onclick = () => setDrawer(false);
  $$("[data-t]", dr).forEach(b => (b.onclick = () => { S.ticket = b.dataset.t; renderDrawer(); }));
  $$("[data-rv]", dr).forEach(b => (b.onclick = () => { S.review = +b.dataset.rv; showDiagram(); }));
  const back = $("#tdBack", dr); if (back) back.onclick = () => { S.ticket = null; renderDrawer(); };
  $$("[data-sess]", dr).forEach(b => (b.onclick = () => openView({ kind: "agent", agent: +b.dataset.sess })));
  const inp = $("#tfile", dr);
  if (inp) {
    inp.value = keepInput; if (focused) inp.focus();
    const go = async () => {
      const v = inp.value.trim(); if (!v) return;
      const [title, body] = v.includes(":") ? v.split(/:(.*)/s) : [v, v];
      inp.value = "";
      await invoke("file_ticket", { title: title.trim(), body: (body || title).trim() }).catch(e => alert(e));
    };
    inp.onkeydown = e => { if (e.key === "Enter") go(); };
    $("#tfileGo", dr).onclick = go;
  }
}
function ticketDetail(t) {
  const a = t.assignee != null ? agent(t.assignee) : null;
  const li = xs => xs.length ? `<ul>${xs.map(x => `<li>${esc(x)}</li>`).join("")}</ul>` : `<div style="color:var(--fg-3)">None</div>`;
  return `<div class="td"><button class="td-back" id="tdBack">${icon("back")}All tickets</button>
    <div class="td-state ${tClass(t.state)}">${icon(t.state === "done" ? "check" : t.state === "in_progress" ? "sparkle" : "ticket")}${esc(tLabel(t.state))}</div>
    <div class="td-title">${esc(t.title)}</div>
    <dl class="kv"><dt>Ticket</dt><dd><code>${esc(String(t.num).padStart(2, "0"))} · ${esc(t.key)}</code></dd>
      <dt>Category</dt><dd>${esc(t.category)}</dd>
      <dt>Agent</dt><dd>${a ? `<button data-sess="${a.id}">${esc(a.title)}</button>` : "unassigned"}</dd>
      ${a && a.branch ? `<dt>Branch</dt><dd><code>${esc(a.branch)}</code></dd>` : ""}
      <dt>Check</dt><dd>${t.check ? `<code>${esc(t.check)}</code>` : "none"}</dd>
      ${t.blocked_by.length ? `<dt>After</dt><dd>${t.blocked_by.map(esc).join(", ")}</dd>` : ""}
      ${t.budget_usd != null ? `<dt>Budget</dt><dd>$${t.budget_usd.toFixed(2)}</dd>` : ""}</dl>
    <details open><summary>What to build</summary><div>${esc(t.what_to_build)}</div></details>
    <details open><summary>Acceptance</summary>${li(t.acceptance)}</details>
    ${t.out_of_scope.length ? `<details><summary>Out of scope</summary>${li(t.out_of_scope)}</details>` : ""}
    ${t.notes.length ? `<details open><summary>Notes</summary>${li(t.notes)}</details>` : ""}</div>`;
}
function showDiagram() {
  // An open diagram canvas anywhere wins; otherwise the focused canvas turns into one.
  for (let ti = 0; ti < prefs.tabs.length; ti++) {
    const pi = prefs.tabs[ti].panes.findIndex(p => p.kind === "diagram");
    if (pi >= 0) { if (ti !== prefs.active_tab) switchTab(ti); S.focus = pi; renderGrid(); return; }
  }
  openView({ kind: "diagram", agent: 0, url: null });
}

// ------------------------------------------------------------------ update pill
function renderUpd() {
  const b = $("#upd");
  b.hidden = false;
  b.classList.toggle("new", !!(upd && upd.newer));
  if (upd === "checking") b.innerHTML = `Checking…`;
  else if (upd && upd.error) { b.innerHTML = `${icon("reload")}Check for updates`; b.title = upd.error; }
  else if (upd && upd.newer) { b.innerHTML = `${icon("down")}<span class="lbl">Update to ${esc(upd.latest)}</span>`; b.title = `Backspace ${upd.latest} is out (you have ${upd.current})`; }
  else if (upd) { b.innerHTML = `${icon("check")}<span class="lbl">Up to date</span><span class="n">v${esc(upd.current)}</span>`; b.title = `Backspace ${upd.current} · click to check again`; }
  else b.innerHTML = `${icon("reload")}<span class="lbl">Check for updates</span>`;
}
async function checkUpdate() {
  upd = "checking"; renderUpd();
  upd = await invoke("check_update").catch(e => ({ error: String(e) }));
  renderUpd(); if (S.settings) renderSettings();
}
$("#upd").onclick = () => (upd && upd.newer ? invoke("open_url", { url: upd.url }) : checkUpdate());

addEventListener("keydown", e => {
  if (e.key === "Escape") { if (!$("#pop").hidden) closePop(); else if (S.settings) closeSettings(); else if (S.drawer) setDrawer(false); }
  if (!(e.metaKey || e.ctrlKey)) return;
  if (!$("#onboard").hidden) return;
  if (e.key === "n") { e.preventDefault(); setMode("chat"); Chat.newChat(); return; }
  if (e.key === "k") { e.preventDefault(); setMode("chat"); setSide(true); $("#search").focus(); return; }
  if (e.key === "o") { e.preventDefault(); pickProject(); return; }
  if (e.key === "j") { e.preventDefault(); setMode(S.mode === "chat" ? "code" : "chat"); return; }
  if (S.mode !== "code") { if (e.key === ",") { e.preventDefault(); openSettings(); } if (e.key === "\\") { e.preventDefault(); setSide($("#win").classList.contains("side-closed")); } return; }
  const i = +e.key;
  if (i >= 1 && i <= 9 && prefs.tabs[i - 1]) { e.preventDefault(); switchTab(i - 1); }
  if (e.key === "t") { e.preventDefault(); newTabPop($("#newTab")); }
  if (e.key === ",") { e.preventDefault(); openSettings(); }
  if (e.key === "\\") { e.preventDefault(); setSide($("#win").classList.contains("side-closed")); }
});

// ------------------------------------------------------------------ canvases
const SPIN = ["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];
let spinI = 0;
setInterval(() => { const g = $$(".spin-g"); if (!g.length) return; spinI = (spinI + 1) % SPIN.length; g.forEach(x => (x.textContent = SPIN[spinI])); }, 130);
function mascot() {
  const rows = ["...XXXXXXXXX", "..XXXXXXXXXX", ".XXXX.XX.XXX", "XXXXXXXXXXXX", ".XXXXXXXXXXX", "..XX.XXXX.XX", "...XXXXXXXXX"];
  let r = "";
  rows.forEach((row, y) => [...row].forEach((c, x) => { if (c === "X") r += `<rect x="${x * 4}" y="${y * 4}" width="4" height="4"/>`; }));
  return `<svg viewBox="0 0 48 28" fill="var(--bs-accent)" aria-hidden="true">${r}</svg>`;
}
function logHTML(e) {
  const t = e.text;
  switch (e.kind) {
    case "user": return `<div class="ln user">${esc(t)}</div>`;
    case "assistant": return `<div class="ln bullet">${esc(t)}</div>`;
    case "tool_call": { const sp = t.indexOf(" "); return `<div class="ln tool"><b>${esc(sp < 0 ? t : t.slice(0, sp))}</b>${esc(sp < 0 ? "" : t.slice(sp))}</div>`; }
    case "tool_result": {
      const lines = t.split("\n");
      const body = lines.length > 6 ? lines.slice(0, 6).join("\n") + `\n… ${lines.length - 6} more lines` : t;
      const cls = /^(CHECK FAILED|REJECTED)/.test(t) ? "err" : /^(Approved|Plan APPROVED|Merged)/.test(t) ? "ok" : "out";
      return `<div class="ln ${cls}">${esc(body)}</div>`;
    }
    case "system": return `<div class="ln ${t.startsWith("escalated") ? "warn" : "note"}">${esc(t)}</div>`;
    case "error": return `<div class="ln err">${esc(t)}</div>`;
    default: return `<div class="ln raw">${esc(t)}</div>`;
  }
}
function agentBody(a) {
  const path = a.worktree || snap.workspace;
  let h = `<div class="banner">${mascot()}<div class="bt"><div>Backspace v0.1.0</div><div>${esc(route(a))}${a.decision && a.decision.confidence > 0 ? ` · routed by ${esc(a.decision.source)} (${Math.round(a.decision.confidence * 100)}% confident)` : a.decision ? ` · routed by ${esc(a.decision.source)}` : ""}</div><div>${esc(path)}</div></div></div>`;
  h += a.log.map(logHTML).join("");
  if (a.status === "running") h += `<div class="ln spin"><span class="spin-g">${SPIN[spinI]}</span> Working… (${esc(a.decision ? a.decision.model : "routing")} · ↓ ${a.input_tokens.toLocaleString()} tokens)</div>`;
  if (a.status === "awaiting_approval") h += `<div class="ln spin"><span class="spin-g">${SPIN[spinI]}</span> Waiting for your review…</div>`;
  if (a.id === MAIN && !a.log.some(e => e.kind === "user")) h += `<div class="ln note">Type a goal below. The main agent interviews you, then proposes tickets for you to approve.</div>`;
  return h;
}
function statusHTML(a) {
  const total = Math.max(snap.total_cost_usd, 0.000001), share = Math.min(1, subtree(a.id) / total);
  const filled = Math.round(share * 10);
  const t = a.ticket && snap.tickets.find(x => x.key === a.ticket);
  const hint = a.id === MAIN ? 'plan approval on <span class="b1">(review in Diagram)</span> · ⌘1–9 switch tabs'
    : t ? `check: ${esc(t.check || "none")} · ${esc(t.state.replace(/_/g, "-"))}` : esc(a.status);
  return `<div class="st">[${esc(a.decision ? `${a.decision.model} @ ${a.decision.effort}` : "not routed")}] <span style="color:var(--blue)">▣ ${esc(snap.name)}</span> | <span style="color:var(--green)">⎇ ${esc(a.branch || "-")}</span></div>
    <div class="st"><span class="bar">${"█".repeat(filled)}${"░".repeat(10 - filled)}</span> ${Math.round(share * 100)}% of spend | <span style="color:var(--amber)">$${subtree(a.id).toFixed(3)}</span> | ${a.input_tokens.toLocaleString()}↓ ${a.output_tokens.toLocaleString()}↑</div>
    <div class="hint">▸▸ ${hint}</div>`;
}
async function filesBody(id, el, ps) {
  const rows = await invoke("list_files", { agent: id }).catch(() => []);
  const a = agent(id);
  const tree = el.querySelector(".ftree"); if (!tree) return;
  tree.innerHTML = `<div class="ft" style="color:var(--fg-3)">${icon("chevd")}<b style="color:var(--fg)">${esc(a && a.ticket ? a.ticket + " worktree" : "Primary worktree")}</b></div>` +
    rows.map(f => `<button class="ft" data-name="${esc(f.name)}" data-path="${esc(f.path)}" data-dir="${f.dir}" style="padding-left:${18 + f.depth * 14}px"><span class="${f.dir ? "dir" : /\.html?$/.test(f.name) ? "html" : /\.py$/.test(f.name) ? "py" : /\.sql$/.test(f.name) ? "sql" : /\.md$/.test(f.name) ? "md" : ""}">${icon(f.dir ? "chev" : "doc")}</span>${esc(f.name)}</button>`).join("");
  const show = text => { let pv = el.querySelector(".preview"); if (!pv) { pv = document.createElement("div"); pv.className = "preview"; el.appendChild(pv); } pv.textContent = text; };
  if (ps.preview) show(ps.preview);
  $$(".ft[data-path]", el).forEach(b => (b.onclick = async () => {
    if (b.dataset.dir === "true") return;
    ps.preview = await invoke("read_file", { path: b.dataset.path }).catch(e => String(e));
    show(ps.preview);
  }));
}
function paneTitle(p) {
  const a = agent(p.agent);
  switch (p.kind) {
    case "agent": return !a ? "Session" : a.id === MAIN ? `Main agent · ${snap.name}` : a.title;
    case "files": return a && a.ticket ? `${a.ticket} worktree` : "Primary worktree";
    case "browser": return p.url ? p.url.replace(/^https?:\/\//, "") : "Browser";
    case "diagram": return "Diagram review";
    case "docs": return "PLAN.md";
    case "board": return `Team chat · ${(snap.board || []).length}`;
    default: return "New canvas";
  }
}
const paneIcon = p => ({ board: '<span class="agent">' + icon("bubble") + "</span>", agent: '<span class="agent">' + icon("sparkle") + "</span>", files: '<span class="term">' + icon("folder") + "</span>", browser: '<span class="term">' + icon("globe") + "</span>", diagram: '<span class="term">' + icon("diagram") + "</span>", docs: '<span class="term">' + icon("doc") + "</span>" })[p.kind] || '<span class="term">' + icon("window") + "</span>";

function buildPane(p, slot) {
  const el = document.createElement("section");
  el.className = "pane" + (slot === S.focus ? " on" : "");
  el.dataset.slot = slot;
  const ps = pstate(slot);
  el.innerHTML = `<header class="ph">${paneIcon(p)}<span class="t">${esc(paneTitle(p))}</span><span class="sp"></span>
    ${p.kind === "agent" ? `<button class="ib" data-act="files" aria-label="Open worktree files">${icon("folder")}</button>` : ""}
    ${p.kind !== "empty" ? `<button class="ib" data-act="swap" aria-label="Show something else here" title="Show something else here">${icon("window")}</button>` : ""}
    ${tab().panes.length > 1 ? `<button class="ib" data-act="max" aria-label="Maximize canvas">${icon("expand")}</button>` : ""}
    <button class="ib" data-act="close" aria-label="Close canvas">${icon("close")}</button></header>`;
  el.querySelector(".ph").addEventListener("pointerdown", e => { if (!e.target.closest("button")) paneDrag(e, el, slot); });
  el.addEventListener("pointerdown", () => { if (S.focus !== slot) { S.focus = slot; $$(".pane").forEach(x => x.classList.toggle("on", +x.dataset.slot === slot)); renderList(); } });
  const act = (k, f) => { const b = el.querySelector(`[data-act=${k}]`); if (b) b.onclick = f; };
  act("max", () => { S.max = S.max == null ? slot : null; renderGrid(); });
  act("swap", () => { setPane(slot, { kind: "empty", agent: 0, url: null }); });
  act("files", () => setPane(slot, { kind: "files", agent: p.agent, url: null }));
  act("close", () => {
    const t = tab();
    if (S.max != null) S.max = null;
    else if (t.panes.length > 1) { t.layout = L.renumber(L.remove(tree(t), slot), slot); t.panes.splice(slot, 1); S.pane.clear(); }
    else t.panes[0] = { kind: "empty", agent: 0, url: null };
    S.focus = Math.min(S.focus, t.panes.length - 1); saveTabs(); renderTabs(); renderGrid();
  });
  const body = document.createElement("div");
  body.style.cssText = "flex:1;min-height:0;display:flex;flex-direction:column";
  el.appendChild(body);
  fillPane(p, slot, body, ps);
  return el;
}
function setPane(slot, spec) { tab().panes[slot] = spec; S.pane.delete(prefs.active_tab + ":" + slot); saveTabs(); renderTabs(); renderGrid(); }

function fillPane(p, slot, body, ps) {
  const note = offlineNote();
  if (note && p.kind !== "empty" && p.kind !== "browser") { body.innerHTML = note; return; }
  if (p.kind === "agent") {
    const a = agent(p.agent) || agent(MAIN);
    if (!a) { body.innerHTML = `<div class="nothing">No such agent on this machine.</div>`; return; }
    body.innerHTML = `<div class="tb" data-agent="${a.id}">${agentBody(a)}</div>
      <footer class="tf"><label class="tp"><span style="color:var(--term-dim)">❯</span><input placeholder="${a.id === MAIN ? "Describe the goal, or answer the main agent" : "Only the main agent takes messages"}" ${a.id === MAIN ? "" : "disabled"} autocomplete="off" spellcheck="false" aria-label="Message"></label>${statusHTML(a)}</footer>`;
    const inp = body.querySelector(".tp input");
    inp.addEventListener("keydown", e => { if (e.key === "Enter" && inp.value.trim()) { invoke("send", { text: inp.value.trim() }); inp.value = ""; } });
    requestAnimationFrame(() => { const tb = body.querySelector(".tb"); if (tb) tb.scrollTop = tb.scrollHeight; });
  } else if (p.kind === "files") {
    body.innerHTML = `<div class="fsearch"><input class="fq" placeholder="Search" aria-label="Search files"><button class="ib" aria-label="Match case">Aa</button></div><div class="ftree"></div>`;
    filesBody(p.agent, body, ps);
    const fq = body.querySelector(".fq");
    fq.oninput = () => $$(".ft[data-name]", body).forEach(r => (r.hidden = !r.dataset.name.toLowerCase().includes(fq.value.toLowerCase())));
  } else if (p.kind === "browser") browserPane(p, body, ps);
  else if (p.kind === "diagram") diagramPane(body, ps);
  else if (p.kind === "docs") docsPane(body);
  else if (p.kind === "board") boardPane(body, ps);
  else chooser(slot, body);
}
function chooser(slot, body) {
  const agents = treeOrder().map(id => agent(id));
  body.innerHTML = `<div class="chooser"><h5>Show in this canvas</h5><div class="cards">
    <button class="card" data-k="agent">${icon("sparkle")}<span class="ct">Main agent</span><span class="cs">Talk to the agent that runs the project</span></button>
    <button class="card" data-k="browser">${icon("globe")}<span class="ct">Browser</span><span class="cs">A dev server preview</span></button>
    <button class="card" data-k="diagram">${icon("diagram")}<span class="ct">Diagram review</span><span class="cs">Plans and deliverables to approve</span></button>
    <button class="card" data-k="files">${icon("folder")}<span class="ct">Worktree</span><span class="cs">Browse the project's files</span></button>
    <button class="card" data-k="docs">${icon("doc")}<span class="ct">PLAN.md</span><span class="cs">The plan the main agent wrote</span></button>
    <button class="card" data-k="board">${icon("bubble")}<span class="ct">Team chat</span><span class="cs">What agents tell each other, across models and CLIs</span></button></div>
    ${agents.length > 1 ? `<h5>Agent sessions</h5><div class="alist">${agents.filter(a => a.id !== MAIN).map(a => `<button class="chip-btn" data-a="${a.id}">${icon("sparkle")}${esc(a.title)}<span class="dot ${dotFor(a.status)}"></span></button>`).join("")}</div>` : ""}</div>`;
  $$("[data-k]", body).forEach(b => (b.onclick = () => setPane(slot, { kind: b.dataset.k, agent: 0, url: null })));
  $$("[data-a]", body).forEach(b => (b.onclick = () => setPane(slot, { kind: "agent", agent: +b.dataset.a, url: null })));
}

// Canvases sit absolutely at the rectangles of the tab's layout tree, so a
// new arrangement (a drag preview, a resize) is a style change the browser
// animates rather than a rebuild.
function renderGrid() {
  const g = $("#grid"), t = tab();
  if (!hasProject) {
    g.className = "grid"; g.innerHTML = projectsHome(); S.els = new Map();
    $("#hOpen").onclick = pickProject; $("#hChat").onclick = () => setMode("chat");
    $$("#grid [data-proj]").forEach(b => (b.onclick = () => openProject(b.dataset.proj)));
    return;
  }
  if (S.focus >= t.panes.length) S.focus = 0;
  g.className = "grid tree still";
  g.innerHTML = "";
  S.els = new Map();
  t.panes.forEach((p, slot) => {
    if (S.max != null && slot !== S.max) return;
    const el = buildPane(p, slot);
    g.appendChild(el); S.els.set(slot, el);
  });
  const ph = document.createElement("div");
  ph.className = "drop-ph"; ph.hidden = true;
  ph.innerHTML = `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 3.2v9.6M3.2 8h9.6"/></svg>`;
  g.appendChild(ph); S.ph = ph;
  position(tree(t));
  requestAnimationFrame(() => g.classList.remove("still"));
}
function position(tr) {
  if (!hasProject || !S.ph) return;
  const g = $("#grid"), W = g.clientWidth, H = g.clientHeight;
  const { leaves, gutters } = S.max != null
    ? { leaves: [{ leaf: S.max, r: { x: 0, y: 0, w: W, h: H } }], gutters: [] }
    : L.layout(tr, { x: 0, y: 0, w: W, h: H }, 6);
  S.ph.hidden = true;
  leaves.forEach(({ leaf, r }) => {
    const el = leaf === PH ? S.ph : S.els.get(leaf);
    if (!el) return;
    el.hidden = false;
    Object.assign(el.style, { left: r.x + "px", top: r.y + "px", width: r.w + "px", height: r.h + "px" });
  });
  $$(".gut", g).forEach(x => x.remove());
  if (!S.drag) gutters.forEach(gt => g.appendChild(gutter(gt)));
}
new ResizeObserver(() => {
  if (!S.els || S.drag) return;
  const g = $("#grid");
  g.classList.add("still"); position(tree(tab()));
  requestAnimationFrame(() => g.classList.remove("still"));
}).observe($("#grid"));

// Live updates touch only transcripts, status lines and review canvases, so
// typing and scroll positions survive.
function refreshPanes() {
  $$(".tb[data-agent]").forEach(tb => {
    const a = agent(+tb.dataset.agent);
    if (!a) return;
    // A canvas waiting for its agent (it did not exist yet) takes it over once it does.
    const el = tb.closest(".pane"), p = tab().panes[+el.dataset.slot];
    if (p && p.kind === "agent" && agent(p.agent) && p.agent !== a.id) { fillPane(p, +el.dataset.slot, el.lastElementChild, pstate(+el.dataset.slot)); return; }
    const atBottom = tb.scrollHeight - tb.scrollTop - tb.clientHeight < 40;
    tb.innerHTML = agentBody(a);
    if (atBottom) tb.scrollTop = tb.scrollHeight;
    const foot = tb.parentElement.querySelector(".tf");
    foot.querySelectorAll(".st, .hint").forEach(x => x.remove());
    foot.insertAdjacentHTML("beforeend", statusHTML(a));
  });
  $$(".pane").forEach(el => {
    const slot = +el.dataset.slot, p = tab().panes[slot];
    if (!p) return;
    const body = el.lastElementChild;
    if (p.kind === "diagram") diagramPane(body, pstate(slot));
    if (p.kind === "board") boardMsgs(body);
    if (p.kind === "empty") chooser(slot, body);
  });
}
function gutter(gt) {
  const d = document.createElement("div");
  d.className = "gut abs " + (gt.dir === "row" ? "v" : "h");
  d.setAttribute("role", "separator");
  Object.assign(d.style, { left: gt.r.x + "px", top: gt.r.y + "px", width: gt.r.w + "px", height: gt.r.h + "px" });
  const t = tab();
  d.addEventListener("pointerdown", e => {
    e.preventDefault(); d.setPointerCapture(e.pointerId); d.classList.add("drag");
    const g = $("#grid"), gr = g.getBoundingClientRect();
    if (!L.valid(t.layout, t.panes.length)) t.layout = tree(t);
    g.classList.add("still");
    const move = ev => {
      const x = ev.clientX - gr.left, y = ev.clientY - gr.top, sp = gt.span;
      L.setRatio(t.layout, gt.path, gt.dir === "row" ? (x - sp.x) / sp.w : (y - sp.y) / sp.h);
      const { leaves } = L.layout(t.layout, { x: 0, y: 0, w: g.clientWidth, h: g.clientHeight }, 6);
      leaves.forEach(({ leaf, r }) => { const el = S.els.get(leaf); if (el) Object.assign(el.style, { left: r.x + "px", top: r.y + "px", width: r.w + "px", height: r.h + "px" }); });
      const me = L.layout(t.layout, { x: 0, y: 0, w: g.clientWidth, h: g.clientHeight }, 6).gutters.find(x => x.path.join() === gt.path.join());
      if (me) Object.assign(d.style, { left: me.r.x + "px", top: me.r.y + "px", width: me.r.w + "px", height: me.r.h + "px" });
    };
    const up = () => { d.removeEventListener("pointermove", move); d.removeEventListener("pointerup", up); saveTabs(); position(t.layout); g.classList.remove("still"); };
    d.addEventListener("pointermove", move); d.addEventListener("pointerup", up);
  });
  d.addEventListener("dblclick", () => { if (!L.valid(t.layout, t.panes.length)) t.layout = tree(t); L.setRatio(t.layout, gt.path, 0.5); saveTabs(); position(t.layout); });
  return d;
}
// Drag a canvas by its header. The others reflow around a dotted "+" slot
// where it will land: the middle of a canvas swaps with it, an edge splits
// it there; dropping on another tab moves the canvas into that tab.
function paneDrag(e, el, slot) {
  if (e.button !== 0 || S.max != null || matchMedia("(max-width: 760px)").matches) return;
  const sx = e.clientX, sy = e.clientY;
  let d = null;
  const begin = () => {
    const g = $("#grid"), gr = g.getBoundingClientRect(), t = tab(), base = tree(t);
    const { leaves } = L.layout(base, { x: 0, y: 0, w: gr.width, h: gr.height }, 6);
    const shield = document.createElement("div");
    shield.className = "drag-shield"; document.body.appendChild(shield);
    const r = el.getBoundingClientRect();
    el.classList.add("dragging");
    // A small card under the pointer, so it doesn't hide where it can land.
    const w = Math.min(r.width, 320), h = Math.min(r.height, 200);
    el.style.width = w + "px"; el.style.height = h + "px";
    S.drag = { slot, base, leaves, gr, key: "", shield, ox: Math.min(sx - r.left, w - 40), oy: Math.min(sy - r.top, 16), tabTo: null };
    position(L.moved(base, slot, slot, "center", PH));
    return S.drag;
  };
  const move = ev => {
    if (!d) { if (Math.hypot(ev.clientX - sx, ev.clientY - sy) < 5) return; d = begin(); }
    el.style.left = ev.clientX - d.gr.left - d.ox + "px"; el.style.top = ev.clientY - d.gr.top - d.oy + "px";
    // Over another tab: drop moves the canvas there.
    const overTab = $$("#tabs [data-tab]").find(b => { const r = b.getBoundingClientRect(); return ev.clientX >= r.left && ev.clientX <= r.right && ev.clientY >= r.top && ev.clientY <= r.bottom; });
    const ti = overTab ? +overTab.dataset.tab : null;
    d.tabTo = ti != null && ti !== prefs.active_tab ? ti : null;
    $$("#tabs [data-tab]").forEach(b => b.classList.toggle("drop-to", +b.dataset.tab === d.tabTo));
    el.classList.toggle("to-tab", d.tabTo != null);
    if (d.tabTo != null) {
      // Tuck the card under the pointer so the tab stays visible, and show
      // this tab without the canvas that is leaving it.
      el.style.left = ev.clientX - d.gr.left - 12 + "px"; el.style.top = ev.clientY - d.gr.top + 14 + "px";
      if (d.key !== "away") { d.key = "away"; d.target = null; position(L.remove(d.base, slot) || d.base); }
      return;
    }
    // Hit-test the arrangement as it was, so the target doesn't move under the pointer.
    const x = ev.clientX - d.gr.left, y = ev.clientY - d.gr.top;
    const hit = d.leaves.find(l => x >= l.r.x && x < l.r.x + l.r.w && y >= l.r.y && y < l.r.y + l.r.h);
    if (!hit) return;
    const side = hit.leaf === slot ? "center" : L.zone(hit.r, x, y);
    const key = hit.leaf + side;
    if (key === d.key) return;
    d.key = key; d.target = hit.leaf; d.side = side;
    position(L.moved(d.base, slot, hit.leaf, side, PH));
  };
  const end = drop => {
    removeEventListener("pointermove", move); removeEventListener("pointerup", up); removeEventListener("keydown", esc_);
    if (!d) return;
    d.shield.remove();
    $$("#tabs .drop-to").forEach(b => b.classList.remove("drop-to"));
    // Land from where the pointer let go: the transition starts there.
    el.classList.remove("dragging", "to-tab");
    S.drag = null;
    const t = tab();
    if (drop && d.tabTo != null) { moveToTab(slot, d.tabTo); return; }
    if (drop && d.target != null) { t.layout = L.moved(d.base, slot, d.target, d.side, slot); S.focus = slot; saveTabs(); renderTabs(); }
    position(tree(t));
  };
  const up = () => end(true);
  const esc_ = ev => { if (ev.key === "Escape") end(false); };
  addEventListener("pointermove", move); addEventListener("pointerup", up); addEventListener("keydown", esc_);
}
function moveToTab(slot, ti) {
  const from = tab(), to = prefs.tabs[ti];
  if (to.panes.length >= 4) { renderGrid(); flash($(`#tabs [data-tab="${ti}"]`)); return; }
  const spec = from.panes[slot];
  if (from.panes.length > 1) { from.layout = L.renumber(L.remove(tree(from), slot), slot); from.panes.splice(slot, 1); }
  else from.panes[0] = { kind: "empty", agent: 0, url: null };
  const n = to.panes.length, tt = tree(to);
  to.panes.push(spec);
  to.layout = L.split("row", n / (n + 1), tt, L.leaf(n));
  S.pane.clear(); S.focus = n;
  prefs.active_tab = ti; S.max = null; saveTabs(); renderTabs(); renderGrid(); renderList();
}
function flash(el) { if (!el) return; el.classList.add("nope"); setTimeout(() => el.classList.remove("nope"), 500); }
// Show something in this tab: focus a canvas already showing it, else take over the focused one.
function openView(spec) {
  closeSettings();
  const t = tab();
  const i = t.panes.findIndex(p => p.kind === spec.kind && (spec.kind !== "agent" && spec.kind !== "files" || p.agent === spec.agent));
  if (i >= 0) S.focus = i;
  else { t.panes[S.focus] = { agent: 0, url: null, ...spec }; S.pane.delete(prefs.active_tab + ":" + S.focus); saveTabs(); }
  S.max = null; renderTabs(); renderGrid(); renderList();
  if (narrow()) setSide(false);
}

// ------------------------------------------------------------------ browser canvas (real webview)
function browserPane(p, body, ps) {
  const vps = [["desktop", "window", "Desktop"], ["tablet", "doc", "Tablet"], ["phone", "term", "Phone"]];
  const vp = ps.viewport || "desktop";
  body.innerHTML = `<div class="browser">
    <div class="b-bar">
      <button class="ib" data-b="back" aria-label="Back">${icon("back")}</button><button class="ib" data-b="fwd" aria-label="Forward">${icon("fwd")}</button><button class="ib" data-b="reload" aria-label="Reload">${icon("reload")}</button>
      <label class="addr">${icon("globe")}<input class="url" value="${esc(p.url || "")}" placeholder="http://localhost:5173" aria-label="Address">${p.url ? '<span class="live"><span class="dot"></span>live</span>' : ""}</label>
      <div class="vp" role="group" aria-label="Viewport">${vps.map(([k, ic, l]) => `<button class="ib ${vp === k ? "on" : ""}" data-vp="${k}" aria-label="${l}">${icon(ic)}</button>`).join("")}</div>
      ${p.url ? `<button class="ib" data-b="ext" aria-label="Open in your browser" title="Open in your browser">${icon("ext")}</button>` : ""}
    </div>
    <div class="b-view"><div class="frame live ${vp === "desktop" ? "" : vp}">${p.url
      ? `<iframe src="${esc(p.url)}" title="Preview of ${esc(p.url)}"></iframe>`
      : `<div class="blankpage"><b>No preview yet</b><span>Type a dev server address above, for example the URL an agent printed after <code>npm run dev</code>, and press Enter.</span></div>`}</div></div></div>`;
  const url = body.querySelector(".url"), f = body.querySelector("iframe");
  url.addEventListener("keydown", e => {
    if (e.key !== "Enter") return;
    let u = url.value.trim(); if (!u) return;
    if (!/^https?:\/\//.test(u)) u = "http://" + u;
    p.url = u; saveTabs(); browserPane(p, body, ps); renderList(); renderPaneTitles();
  });
  body.querySelector("[data-b=reload]").onclick = () => { if (f) f.src = f.src; };
  body.querySelector("[data-b=back]").onclick = () => { try { f.contentWindow.history.back(); } catch (e) {} };
  body.querySelector("[data-b=fwd]").onclick = () => { try { f.contentWindow.history.forward(); } catch (e) {} };
  const ext = body.querySelector("[data-b=ext]"); if (ext) ext.onclick = () => invoke("open_url", { url: p.url });
  $$("[data-vp]", body).forEach(b => (b.onclick = () => { ps.viewport = b.dataset.vp; browserPane(p, body, ps); }));
}
function renderPaneTitles() { $$(".pane").forEach(el => { const p = tab().panes[+el.dataset.slot]; if (p) el.querySelector(".ph .t").textContent = paneTitle(p); }); }

// ------------------------------------------------------------------ diagram canvas
const NW = 210, NH = 60, GX = 80, GY = 34;
async function diagramPane(body, ps) {
  const list = snap.approvals.slice().reverse();
  if (S.review == null || !snap.approvals[S.review]) S.review = (pending()[0] || list[0] || {}).id ?? null;
  const ap = S.review != null ? snap.approvals[S.review] : null;
  const d = ap ? await invoke("diagram", { id: ap.id }).catch(() => null) : null;
  const label = a => a.kind === "plan" ? `Plan · ${a.tickets.length} tickets` : a.agent === MAIN ? "Final deliverable" : `Ticket · ${a.tickets.join(", ")}`;
  const fb = body.querySelector(".fbk") ? body.querySelector(".fbk").value : "";
  body.innerHTML = `<div class="dg">
    <div class="dg-list" role="tablist" aria-label="Reviews"><h3>Reviews · diagram first</h3>${list.length ? list.map(a => {
      const st = a.state.state;
      return `<button class="rv" role="tab" aria-selected="${a.id === S.review}" data-rv="${a.id}"><span class="k">${icon("diagram")}${a.kind === "plan" ? "Plan" : a.agent === MAIN ? "Final" : "Ticket"}</span><span class="t">${esc(a.kind === "plan" ? label(a) : (agent(a.agent) || {}).title)}</span><span class="chip ${st === "pending" ? "pending" : st === "approved" ? "approved" : "locked"}">${st}</span></button>`;
    }).join("") : `<div class="nothing">Nothing has needed your review yet.</div>`}</div>
    <div class="stage">${d ? `
      <svg class="dgsvg" role="img" aria-label="${esc(d.title)}"></svg>
      <div class="facts"><span class="f title">${esc(d.title)}</span>${d.facts.map(([k, v]) => `<span class="f"><b>${esc(v)}</b> ${esc(k)}</span>`).join("")}</div>
      <div class="zoomc"><button class="ib" data-z="out" aria-label="Zoom out">−</button><button class="ib" data-z="in" aria-label="Zoom in">+</button><button class="ib" data-z="fit" aria-label="Fit">${icon("expand")}</button></div>
      <div class="rcard"></div>` : `<div class="empty"><div class="mark">${icon("diagram")}</div><div class="nothing">Plans and deliverables appear here, drawn by the harness before you read them.</div></div>`}</div></div>`;
  $$("[data-rv]", body).forEach(b => (b.onclick = () => { S.review = +b.dataset.rv; ps.view = null; diagramPane(body, ps); }));
  if (!d) return;
  const key = ap.id + ":" + d.nodes.length;
  if (key !== ps.key) { ps.view = null; ps.key = key; }
  drawDiagram(body.querySelector(".dgsvg"), d); reviewCard(body.querySelector(".rcard"), ap, d, label(ap), fb); wireStage(body.querySelector(".stage"), d, ps);
}
const pos = n => [40 + n.layer * (NW + GX), 120 + n.row * (NH + GY)];
function drawDiagram(svg, d) {
  const tone = { done: "var(--green)", warn: "var(--amber)", neutral: "var(--fg-4)", active: "var(--blue)", fail: "var(--red)" };
  const uid = Math.random().toString(36).slice(2, 8);
  let h = `<defs><pattern id="dots${uid}" width="22" height="22" patternUnits="userSpaceOnUse"><circle cx="1.5" cy="1.5" r="1.1" fill="var(--grid-dot)"/></pattern>
    <marker id="ar${uid}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" class="e-head"/></marker></defs>
    <rect x="-4000" y="-4000" width="9000" height="9000" fill="url(#dots${uid})"/>`;
  const layers = Math.max(0, ...d.nodes.map(n => n.layer)) + 1;
  if (layers > 1) for (let l = 0; l < layers; l++) h += `<text x="${40 + l * (NW + GX)}" y="100" class="n-wave">WAVE ${l + 1}</text>`;
  d.edges.forEach(([a, b]) => {
    const [ax, ay] = pos(d.nodes[a]), [bx, by] = pos(d.nodes[b]);
    const x1 = ax + NW, y1 = ay + NH / 2, x2 = bx, y2 = by + NH / 2, xm = x1 + GX / 2;
    h += `<path class="e-line" d="M${x1},${y1} H${xm} V${y2} H${x2 - 2}" marker-end="url(#ar${uid})"/>`;
  });
  d.nodes.forEach(n => {
    const [x, y] = pos(n);
    h += `<g><rect x="${x}" y="${y}" width="${NW}" height="${NH}" rx="10" fill="var(--node)" stroke="${tone[n.tone] || "var(--fg-4)"}" stroke-width="1.6"/>
      <text x="${x + 14}" y="${y + 25}" class="n-label">${esc(n.label.length > 24 ? n.label.slice(0, 23) + "…" : n.label)}</text><text x="${x + 14}" y="${y + 44}" class="n-sub">${esc(n.sub.length > 30 ? n.sub.slice(0, 29) + "…" : n.sub)}</text></g>`;
  });
  svg.innerHTML = h;
}
function reviewCard(card, ap, d, label, fbText) {
  const st = ap.state.state, files = d.files;
  const max = Math.max(1, ...files.map(f => f.added + f.removed));
  card.innerHTML = `<h4>${esc(label)}</h4><p>${esc(ap.deliverable.summary)}</p>
    ${d.warnings.length ? `<div class="warns">${d.warnings.map(w => `<div>${esc(w)}</div>`).join("")}</div>` : ""}
    ${files.length ? `<div class="fb">${files.slice(0, 8).map(f => `<div class="r"><span class="p">${esc(f.path)}</span><span class="bars"><span class="ad" style="width:${(f.added / max) * 90}px"></span><span class="rm" style="width:${(f.removed / max) * 90}px"></span></span><span class="n">+${f.added} −${f.removed}</span></div>`).join("")}</div>` : ""}
    ${st === "pending" ? `<textarea class="fbk" placeholder="Feedback for the agent (needed to reject)" aria-label="Feedback"></textarea>
      <div class="acts"><button class="btn primary" data-r="appr">Approve</button><button class="btn danger" data-r="rej">Reject</button><button class="btn" data-r="sess">Session</button><span class="formerr" role="alert"></span></div>`
      : `<div class="acts"><span class="chip ${st === "approved" ? "approved" : "locked"}">${esc(st)}${ap.state.feedback ? ": " + esc(ap.state.feedback) : ""}</span><button class="btn" data-r="sess">Session</button></div>`}`;
  card.querySelector("[data-r=sess]").onclick = () => openView({ kind: "agent", agent: ap.agent });
  if (st !== "pending") return;
  card.querySelector(".fbk").value = fbText || "";
  card.querySelector("[data-r=appr]").onclick = () => invoke("approve", { id: ap.id });
  card.querySelector("[data-r=rej]").onclick = () => {
    const fb = card.querySelector(".fbk").value.trim();
    if (!fb) { card.querySelector(".formerr").textContent = "Write what should change first."; return; }
    invoke("reject", { id: ap.id, feedback: fb });
  };
}
function wireStage(stage, d, ps) {
  const svg = stage.querySelector(".dgsvg");
  const xs = d.nodes.map(n => pos(n)[0]), ys = d.nodes.map(n => pos(n)[1]);
  const bounds = { x: 0, y: 80, w: Math.max(...xs) + NW + 40, h: Math.max(...ys) + NH - 80 + 40 };
  // Never magnify past 1.25x, and keep the drawing in the band above the review card.
  const fit = () => {
    const sw = svg.clientWidth || 600, sh = svg.clientHeight || 400;
    const card = stage.querySelector(".rcard"), band = card ? Math.max(0.5, 1 - (card.offsetHeight + 32) / sh) : 1;
    const k = Math.min(1.25, sw / (bounds.w + 80), (sh * band) / (bounds.h + 80));
    const w = sw / k, h = sh / k;
    ps.view = { x: bounds.x + bounds.w / 2 - w / 2, y: bounds.y + bounds.h / 2 - (h * band) / 2 - 20 / k, w, h };
  };
  if (!ps.view) fit();
  const apply = () => svg.setAttribute("viewBox", `${ps.view.x} ${ps.view.y} ${ps.view.w} ${ps.view.h}`);
  apply();
  const zoom = (f, cx, cy) => {
    const b = svg.getBoundingClientRect(), v = ps.view;
    const px = v.x + ((cx - b.left) / b.width) * v.w, py = v.y + ((cy - b.top) / b.height) * v.h;
    const nw = Math.min(8000, Math.max(300, v.w * f)), k = nw / v.w;
    ps.view = { x: px - (px - v.x) * k, y: py - (py - v.y) * k, w: nw, h: v.h * k }; apply();
  };
  $$("[data-z]", stage).forEach(b => (b.onclick = () => {
    const bb = svg.getBoundingClientRect();
    if (b.dataset.z === "fit") { fit(); apply(); } else zoom(b.dataset.z === "in" ? 0.8 : 1.25, bb.left + bb.width / 2, bb.top + bb.height / 2);
  }));
  svg.addEventListener("wheel", e => { e.preventDefault(); zoom(Math.exp(e.deltaY * 0.0015), e.clientX, e.clientY); }, { passive: false });
  let last = null;
  svg.addEventListener("pointerdown", e => { last = [e.clientX, e.clientY]; svg.setPointerCapture(e.pointerId); stage.classList.add("drag"); });
  svg.addEventListener("pointermove", e => { if (!last) return; const b = svg.getBoundingClientRect(); ps.view.x -= (e.clientX - last[0]) * ps.view.w / b.width; ps.view.y -= (e.clientY - last[1]) * ps.view.h / b.height; last = [e.clientX, e.clientY]; apply(); });
  const up = () => { last = null; stage.classList.remove("drag"); };
  svg.addEventListener("pointerup", up); svg.addEventListener("pointercancel", up);
}

// ------------------------------------------------------------------ team chat canvas
// The agents' message board: who told whom what, whichever model or CLI
// runs them. You can post too, to one agent or everyone.
function boardPane(body, ps) {
  body.innerHTML = `<div class="board"><div class="bmsgs"></div>
    <footer class="bfoot"><select class="bto" aria-label="Send to"></select><input class="bin" placeholder="Message the team" aria-label="Message" autocomplete="off"><button class="btn primary sm bsend">Send</button></footer></div>`;
  const to = body.querySelector(".bto"), inp = body.querySelector(".bin");
  const opts = () => [["all", "Everyone"], ...snap.agents.map(a => [a.key, `${a.title} (${a.key})`])];
  to.innerHTML = opts().map(([k, l]) => `<option value="${esc(k)}">${esc(l)}</option>`).join("");
  to.value = ps.to || "all"; to.onchange = () => (ps.to = to.value);
  const go = async () => {
    if (!inp.value.trim()) return;
    try { await invoke("post_message", { to: to.value, text: inp.value.trim() }); inp.value = ""; } catch (e) { toast(String(e), "err"); }
  };
  body.querySelector(".bsend").onclick = go;
  inp.onkeydown = e => { if (e.key === "Enter") go(); };
  boardMsgs(body);
}
function boardMsgs(body) {
  const box = body.querySelector(".bmsgs"); if (!box) return;
  const ms = snap.board || [];
  const who = k => { if (k === "you") return { t: "You", run: "" }; const a = snap.agents.find(a => a.key === k); return { t: a ? a.title : k, run: a && a.decision ? a.decision.model : "" }; };
  const atBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 40;
  box.innerHTML = ms.length ? ms.map(m => {
    const f = who(m.from), me = m.from === "you";
    return `<div class="bm${me ? " me" : ""}"><div class="bm-h"><b>${esc(f.t)}</b>${f.run ? `<span class="bm-run">${esc(f.run)}</span>` : ""}<span class="bm-to">→ ${m.to === "all" ? "everyone" : esc(who(m.to).t)}</span><span class="bm-at">${new Date(m.at).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</span></div><div class="bm-t">${esc(m.text)}</div></div>`;
  }).join("") : `<div class="nothing">No messages yet. Agents post here to coordinate (interfaces, shared files, questions), whether they run on an API model or in Claude Code, Codex or another CLI.</div>`;
  if (atBottom) box.scrollTop = box.scrollHeight;
  const sel = body.querySelector(".bto");
  if (sel && sel.options.length !== snap.agents.length + 1) { const v = sel.value; sel.innerHTML = [["all", "Everyone"], ...snap.agents.map(a => [a.key, `${a.title} (${a.key})`])].map(([k, l]) => `<option value="${esc(k)}">${esc(l)}</option>`).join(""); sel.value = v; }
}

// ------------------------------------------------------------------ PLAN.md canvas
function md(src) {
  let out = "", code = false, list = false;
  for (const raw of src.split("\n")) {
    const l = raw.replace(/\s+$/, "");
    if (l.startsWith("```")) { if (list) { out += "</ul>"; list = false; } out += code ? "</pre>" : "<pre>"; code = !code; continue; }
    if (code) { out += esc(l) + "\n"; continue; }
    const inline = t => esc(t).replace(/`([^`]+)`/g, "<code>$1</code>").replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>");
    if (/^[-*] /.test(l)) { if (!list) { out += "<ul>"; list = true; } out += `<li>${inline(l.slice(2))}</li>`; continue; }
    if (list) { out += "</ul>"; list = false; }
    const h = l.match(/^(#{1,3}) (.*)/);
    if (h) out += `<h${h[1].length === 1 ? 1 : 2}>${inline(h[2])}</h${h[1].length === 1 ? 1 : 2}>`;
    else if (l.trim()) out += `<p>${inline(l)}</p>`;
  }
  return out + (list ? "</ul>" : "") + (code ? "</pre>" : "");
}
async function docsPane(body) {
  const text = snap.workspace ? await invoke("read_file", { path: snap.workspace + "/PLAN.md" }).catch(() => null) : null;
  body.innerHTML = `<div class="docs"><article class="doc">${text ? md(text) : `<h1>PLAN.md</h1><div class="by">${esc(snap.name)}</div><p>The main agent writes PLAN.md before it proposes tickets. It will show here once it exists.</p>`}</article></div>`;
}

// ------------------------------------------------------------------ settings
function openSettings(section) {
  S.settings = true; $("#settings").hidden = false; $("#canvas").hidden = true; $("#chat").hidden = true; renderSettings();
  if (section) { const el = $(`#set-${section}`); if (el) el.scrollIntoView(); const f = $(`#set-${section} input`); if (f) f.focus(); }
  refreshShare();
}
function closeSettings() { if (!S.settings) return; S.settings = false; $("#settings").hidden = true; $("#canvas").hidden = S.mode !== "code"; $("#chat").hidden = S.mode !== "chat"; }
async function refreshShare() { share = await invoke("share_status").catch(() => null); if (S.settings) renderSettings(); }
function renderSettings() {
  const el = $("#settings");
  const keep = {}; $$("input.tx", el).forEach(i => (keep[i.id] = i.value));
  const th = prefs.theme || "system";
  const sh = share || { enabled: false, addr: "127.0.0.1:7420", token: "", error: null, sharing: false };
  const u = upd && upd !== "checking" ? upd : null;
  el.innerHTML = `<div class="set">
    <div class="set-h"><h1>Settings</h1><span class="sp"></span><button class="btn" id="setRerun">Run setup again</button><button class="ib" id="setClose" aria-label="Close settings">${icon("close")}</button></div>
    <section id="set-providers"><h2>Coding CLIs and models</h2><p class="lead">What Chat and projects can use. Backspace scans this machine for each CLI, its version and whether it is signed in; switch off any you don't want offered.</p><div id="setProviders"></div></section>
    <section id="set-plan"><h2>Backspace Cloud</h2><p class="lead">Chat without installing anything. Free with a short ad under each reply, or a paid plan.</p><div id="setPlan"></div></section>
    <section id="set-coding"><h2>Coding</h2><p class="lead">A project's planner runs on an API model (Anthropic or OpenRouter, keys below). Each ticket's worker can run on the router's pick, or be handed to a coding CLI you're signed in to: it works in the ticket's own git worktree, then goes through the same checks and your review.</p>
      <div class="line"><span class="lab">Workers run on<small>Applies when a project opens.${hasProject ? ` <button class="lnk" id="reopen">Reopen ${esc(snap.name)} now</button>` : ""}</small></span><div class="segc wrap" id="workerSeg"></div></div>
      <div id="keyRows"></div></section>
    <section id="set-chat"><h2>Chat</h2><div class="line"><span class="lab">New chats go to<small>You can switch per chat from the composer.</small></span><button class="btn" id="setRoute" data-popper>${esc(window.Chat ? Chat.routeName(prefs.default_route) : "Pick")}</button></div></section>
    <section id="set-projects"><h2>Projects</h2>
      ${(prefs.projects || []).length ? prefs.projects.map(p => `<div class="mrow"><span class="mi">${icon("folder")}</span><span class="lab">${esc(base(p))}${hasProject && p === snap.workspace ? " · open" : ""}<small class="mono">${esc(p)}</small></span><button class="btn" data-popen="${esc(p)}">Open</button><button class="btn danger" data-pforget="${esc(p)}">Forget</button></div>`).join("") : `<p class="lead">No projects yet.</p>`}
      <div class="line"><span class="lab"></span><button class="btn primary" id="setOpenFolder">Open folder…</button></div></section>
    <section id="set-uses"><h2>Use Backspace for</h2><div class="line"><span class="lab">What shows in the app<small>With both on, switch from the top of the sidebar.</small></span><div class="segc">${[["chat", "Chat"], ["code", "Coding"]].map(([k, l]) => `<button aria-pressed="${(prefs.uses || ["chat", "code"]).includes(k)}" data-use="${k}">${l}</button>`).join("")}</div></div></section>
    <section><h2>Appearance</h2><div class="line"><span class="lab">Theme</span><div class="segc">${[["system", "System"], ["light", "Light"], ["dark", "Dark"]].map(([k, l]) => `<button aria-pressed="${th === k}" data-theme="${k}">${l}</button>`).join("")}</div></div></section>
    <section id="set-machines"><h2>Machines</h2><p class="lead">Follow and drive the harness on other machines. Each one runs <code>backspace-cli serve</code> (or shares from its own Backspace, below); add it with its address and token. Switch machines from the sidebar foot.</p>
      ${machines.map(m => `<div class="mrow"><span class="mi">${icon(m.local ? "pc" : "cloud")}</span><span class="lab">${esc(m.name)}${m.selected ? " · showing" : ""}<small>${m.local ? "This machine" : esc(m.url)} · ${m.link.state === "offline" ? "offline: " + esc(m.link.error) : m.link.state}${m.project ? " · " + esc(m.project) : ""}</small></span>
        ${m.selected ? "" : `<button class="btn" data-show="${m.index}">Show</button>`}${m.local ? "" : `<button class="btn danger" data-rm="${m.index}">Remove</button>`}</div>`).join("")}
      <div class="form3"><input class="tx" id="mName" placeholder="Name, e.g. Cloud" aria-label="Machine name"><input class="tx mono" id="mUrl" placeholder="http://10.0.0.5:7420" aria-label="Address"><input class="tx mono" id="mTok" placeholder="Token" aria-label="Token"><button class="btn primary" id="mAdd">Add</button></div>
      <div class="err" id="mErr" role="alert"></div></section>
    <section id="set-share"><h2>Share this machine</h2><p class="lead">Let other machines follow this harness and approve its work. Plain HTTP with a token: keep it on localhost and use an SSH tunnel, or a private network such as Tailscale.</p>
      <div class="line"><span class="lab">Sharing<small>${sh.sharing ? "On at http://" + esc(sh.addr) : "Off"}</small></span><button class="toggle" role="switch" aria-checked="${sh.enabled}" id="shareT" aria-label="Share this machine"></button></div>
      <div class="line"><span class="lab">Address</span><input class="tx mono" id="shAddr" value="${esc(sh.addr)}" style="width:200px" aria-label="Listen address"><button class="btn" id="shApply">Apply</button></div>
      <div class="line"><span class="lab">Token</span><code class="mono" style="user-select:all">${esc(sh.token)}</code><button class="btn" id="shNew">New token</button></div>
      ${sh.error ? `<div class="err">${esc(sh.error)}</div>` : ""}
      <pre class="cmd">ssh -L 7420:${esc(sh.addr)} you@this-machine   # then add http://127.0.0.1:7420 elsewhere</pre></section>
    <section id="set-updates"><h2>Updates</h2>
      <div class="line"><span class="lab">Check for updates automatically</span><button class="toggle" role="switch" aria-checked="${prefs.check_updates}" id="updT" aria-label="Check for updates automatically"></button></div>
      <div class="line"><span class="lab">Backspace ${esc(u && u.current || "0.1.0")}<small>${u && u.error ? esc(u.error) : u && u.newer ? `Version ${esc(u.latest)} is available.` : u ? "You're on the latest release." : "Not checked yet."}</small></span>
        ${u && u.newer ? `<button class="btn primary" id="updGo">Download & Update</button>` : `<button class="btn" id="updNow">Check now</button>`}</div></section>
    ${hasProject ? `<section><h2>Open project</h2>
      <div class="line"><span class="lab">Workspace<small class="mono">${esc(snap.workspace || "—")}</small></span></div>
      <div class="line"><span class="lab">Config<small class="mono">${esc(snap.config_source || "built-in defaults")}</small></span></div></section>` : ""}</div>`;
  Object.entries(keep).forEach(([id, v]) => { const i = $("#" + id, el); if (i && v) i.value = v; });
  $("#setClose").onclick = closeSettings;
  $$("[data-use]", el).forEach(b => (b.onclick = async () => {
    const cur = prefs.uses && prefs.uses.length ? prefs.uses : ["chat", "code"], k = b.dataset.use;
    const next = ["chat", "code"].filter(u => (u === k ? !cur.includes(u) : cur.includes(u)));
    if (!next.length) { toast("Keep at least one on", "err"); return; }
    prefs.uses = next; await invoke("set_uses", { uses: next });
    setMode(S.mode, false); openSettings("uses");
  }));
  $("#setRerun").onclick = () => { closeSettings(); Onboard.open(); };
  if (window.Providers) { Providers.mount($("#setProviders")); Providers.mountPlan($("#setPlan")); }
  $("#setRoute").onclick = e => Chat.routePicker(e.currentTarget, prefs.default_route, async r => { prefs.default_route = r; await invoke("set_default_route", { route: r }); renderSettings(); });
  $("#setOpenFolder").onclick = pickProject;
  renderCoding(el);
  $$("[data-popen]", el).forEach(b => (b.onclick = () => { closeSettings(); openProject(b.dataset.popen); }));
  $$("[data-pforget]", el).forEach(b => (b.onclick = async () => { await invoke("forget_project", { path: b.dataset.pforget }); prefs = await invoke("prefs"); renderSettings(); renderList(); }));
  $$("[data-theme]", el).forEach(b => (b.onclick = async () => { prefs.theme = b.dataset.theme; applyTheme(); renderSettings(); await invoke("set_theme", { theme: prefs.theme }); }));
  $$("[data-show]", el).forEach(b => (b.onclick = async () => { await invoke("select_machine", { index: +b.dataset.show }); S.pane.clear(); S.review = null; await refresh(true); renderSettings(); }));
  $$("[data-rm]", el).forEach(b => (b.onclick = async () => { await invoke("remove_machine", { index: +b.dataset.rm }); await refresh(true); renderSettings(); }));
  $("#mAdd").onclick = async () => {
    const err = $("#mErr"); err.textContent = ""; $("#mAdd").disabled = true; $("#mAdd").textContent = "Checking…";
    try {
      await invoke("add_machine", { name: $("#mName").value, url: $("#mUrl").value, token: $("#mTok").value });
      ["mName", "mUrl", "mTok"].forEach(id => ($("#" + id).value = ""));
      S.pane.clear(); S.review = null; await refresh(true); renderSettings();
    } catch (e) { err.textContent = String(e); $("#mAdd").disabled = false; $("#mAdd").textContent = "Add"; }
  };
  const setShare = async (enabled, newToken) => {
    await invoke("set_share", { enabled, addr: $("#shAddr").value, newToken }).catch(() => {});
    await refreshShare();
  };
  $("#shareT").onclick = () => setShare(!sh.enabled, false);
  $("#shApply").onclick = () => setShare(sh.enabled, false);
  $("#shNew").onclick = () => setShare(sh.enabled, true);
  $("#updT").onclick = async () => { prefs.check_updates = !prefs.check_updates; await invoke("set_check_updates", { on: prefs.check_updates }); renderUpd(); renderSettings(); };
  const now = $("#updNow"); if (now) now.onclick = checkUpdate;
  const go = $("#updGo"); if (go) go.onclick = () => invoke("open_url", { url: u.url });
}

// Settings → Coding: which harness runs workers, and the API keys.
const WORKERS = [["", "Auto"], ["claude-code", "Claude Code", "claude"], ["codex", "Codex", "codex"], ["cursor", "Cursor", "cursor"], ["grok", "Grok", "grok"], ["opencode", "OpenCode", "opencode"]];
async function renderCoding(el) {
  const seg = $("#workerSeg", el); if (!seg) return;
  const hs = window.Providers ? Providers.list : [];
  const ok = id => !id || hs.some(h => h.id === id && h.installed && h.enabled);
  seg.innerHTML = WORKERS.map(([m, l, h]) => `<button data-w="${m}" aria-pressed="${(prefs.worker || "") === m}" ${ok(h) ? "" : `disabled title="${l} isn't installed or is switched off"`}>${l}</button>`).join("");
  $$("[data-w]", seg).forEach(b => (b.onclick = async () => { prefs.worker = b.dataset.w || null; await invoke("set_worker", { worker: prefs.worker }); renderSettings(); toast(b.dataset.w ? `New tickets go to ${b.textContent}` : "The router picks a model per ticket", "ok"); }));
  const ro = $("#reopen", el); if (ro) ro.onclick = () => { const p = snap.workspace; closeSettings(); openProject(p); };
  const keys = await invoke("api_keys").catch(() => []);
  const names = { ANTHROPIC_API_KEY: ["Anthropic", "Claude models for the planner and workers"], OPENROUTER_API_KEY: ["OpenRouter", "Any model through one key"], TYPESAFE_API_KEY: ["Jev router", "Optional: smarter model choice per agent"] };
  $("#keyRows", el).innerHTML = keys.map(([k, inApp, env]) => `<div class="line"><span class="lab">${names[k][0]}<small>${names[k][1]} · ${inApp ? "saved in Backspace" : env ? "from your environment" : "not set"}</small></span>
    <input class="tx mono" type="password" data-key="${k}" placeholder="${inApp || env ? "••••••••" : k}" style="width:220px" aria-label="${names[k][0]} key"><button class="btn" data-savekey="${k}">Save</button></div>`).join("");
  $$("[data-savekey]", el).forEach(b => (b.onclick = async () => {
    const i = $(`[data-key="${b.dataset.savekey}"]`, el);
    try { await invoke("set_api_key", { name: b.dataset.savekey, value: i.value }); toast(i.value ? "Key saved" : "Key removed", "ok"); renderCoding(el); } catch (e) { toast(String(e), "err"); }
  }));
}

// ------------------------------------------------------------------ sidebar resize, live updates, boot
(() => {
  const g = $("#sideGut"), win = $("#win");
  const place = () => (g.style.left = getComputedStyle(win).getPropertyValue("--side-w"));
  g.addEventListener("pointerdown", e => {
    e.preventDefault(); g.setPointerCapture(e.pointerId);
    const left = win.getBoundingClientRect().left;
    const move = ev => { const w = Math.min(380, Math.max(220, ev.clientX - left)); win.style.setProperty("--side-w", w + "px"); place(); };
    const upf = () => { g.removeEventListener("pointermove", move); g.removeEventListener("pointerup", upf); };
    g.addEventListener("pointermove", move); g.addEventListener("pointerup", upf);
  });
  place();
})();

let queued = false, lastPending = 0, lastShape = "";
async function refresh(full) {
  queued = false;
  [snap, machines] = await Promise.all([invoke("snapshot"), invoke("machines")]);
  hasProject = !machine().local || !!snap.workspace;
  $("#win").classList.toggle("no-project", !hasProject);
  if (window.Chat) await Chat.refresh();
  if (window.Providers) Providers.refreshQuiet();
  renderBrand(); renderMachines(); renderList(); renderCard(); renderTabs(); renderDrawer();
  if (S.mode === "chat") { lastShape = ""; return; }
  // Rebuild canvases when the machine or its agents appear; otherwise update in place.
  const shape = machine().index + ":" + (snap.agents.length > 0) + ":" + machine().link.state;
  if (full || shape !== lastShape) renderGrid(); else refreshPanes();
  lastShape = shape;
  const n = pending().length;
  if (n > lastPending) S.review = pending()[n - 1].id;
  lastPending = n;
}
// Coalesce bursts of harness events into one refresh per frame.
TAURI.event.listen("state", () => { if (!queued) { queued = true; requestAnimationFrame(() => refresh(false)); } });
addEventListener("resize", () => { setSide(!narrow()); });

// After every script has run (chat.js and friends load after this one).
addEventListener("DOMContentLoaded", async () => {
  const boot = await invoke("boot");
  window.__BOOT = boot;
  document.documentElement.classList.add("native", boot.platform);
  prefs = await invoke("prefs");
  if ([1, 2, 3, 4].includes(boot.layout)) {
    // bench/: a fixed starting tab with that many canvases.
    prefs.tabs.unshift({ name: "Bench", panes: [{ kind: "agent", agent: 0, url: null }, { kind: "files", agent: 0, url: null }, { kind: "agent", agent: 1, url: null }, { kind: "agent", agent: 2, url: null }].slice(0, boot.layout), layout: null });
    prefs.active_tab = 0;
  }
  applyTheme(); renderSeg(); setSide(!narrow());
  S.mode = prefs.mode === "code" ? "code" : "chat";
  // A folder given on the command line, or a bench run: start in Code.
  const first = await invoke("snapshot");
  if (first.workspace && (boot.bench || !prefs.onboarded)) S.mode = "code";
  if (window.Chat) await Chat.init();
  setMode(S.mode, false);
  await refresh(true);
  if (!prefs.onboarded && !boot.bench) Onboard.open();
  if (prefs.check_updates && !boot.bench) checkUpdate(); else renderUpd();
  requestAnimationFrame(() => requestAnimationFrame(() => invoke("ready")));
});
