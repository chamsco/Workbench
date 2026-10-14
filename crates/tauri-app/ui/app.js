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
  diff: '<path d="M8 2.5v6M5 5.5h6M5 12.5h6"/>',
  gear: '<g transform="scale(.6667)" stroke-width="2"><path d="M10.33 4.32c.43-1.76 2.92-1.76 3.35 0a1.72 1.72 0 0 0 2.57 1.07c1.54-.94 3.31.83 2.37 2.37a1.72 1.72 0 0 0 1.07 2.57c1.76.43 1.76 2.92 0 3.35a1.72 1.72 0 0 0-1.07 2.57c.94 1.54-.83 3.31-2.37 2.37a1.72 1.72 0 0 0-2.57 1.07c-.43 1.76-2.92 1.76-3.35 0a1.72 1.72 0 0 0-2.57-1.07c-1.54.94-3.31-.83-2.37-2.37a1.72 1.72 0 0 0-1.07-2.57c-1.76-.43-1.76-2.92 0-3.35a1.72 1.72 0 0 0 1.07-2.57c-.94-1.54.83-3.31 2.37-2.37 1 .61 2.3.07 2.57-1.07Z"/><circle cx="12" cy="12" r="3"/></g>',
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
  updown: '<path d="m5.4 6 2.6-2.6L10.6 6M5.4 10l2.6 2.6 2.6-2.6"/>',
  panelr: '<rect x="2.6" y="3" width="10.8" height="10" rx="2"/><path d="M9.6 3v10"/>',
  lock: '<rect x="3.6" y="7" width="8.8" height="6" rx="1.4"/><path d="M5.6 7V5.2a2.4 2.4 0 0 1 4.8 0V7"/>',
  home: '<path d="M2.8 7.4 8 3l5.2 4.4M4.4 6.2V13h7.2V6.2"/>',
  trace: '<path d="M2.6 4h5M4.6 8h6M6.6 12h6.8"/>',
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
  moon: '<path d="M12.8 9.6A5.4 5.4 0 0 1 6.4 3.2a5.4 5.4 0 1 0 6.4 6.4z"/>',
  undo: '<path d="M5.4 4.2 2.8 6.8l2.6 2.6"/><path d="M3 6.8h6.4a3.6 3.6 0 0 1 0 7.2H6.6"/>',
  memory: '<path d="M5.6 2.4h4.8a1.8 1.8 0 0 1 1.8 1.8v9.4L8 11.2l-4.2 2.4V4.2a1.8 1.8 0 0 1 1.8-1.8z"/><path d="M6.2 5.6h3.6M6.2 7.8h2.4"/>',
  apps: '<rect x="2.4" y="2.4" width="4.6" height="4.6" rx="1.3"/><rect x="9" y="2.4" width="4.6" height="4.6" rx="1.3"/><rect x="2.4" y="9" width="4.6" height="4.6" rx="1.3"/><rect x="9" y="9" width="4.6" height="4.6" rx="2.3"/>',
  splitv: '<rect x="2" y="3" width="12" height="10" rx="1.6"/><path d="M8.6 3v10"/>',
  pairi: '<circle cx="5.4" cy="5.4" r="2"/><circle cx="10.8" cy="5.4" r="2"/><path d="M2 12.6c.4-2 1.8-3.2 3.4-3.2s3 1.2 3.4 3.2M7.4 12.6c.4-2 1.8-3.2 3.4-3.2s3 1.2 3.4 3.2"/>',
  wmin: '<path d="M3.5 8.5h9"/>',
  team: '<circle cx="8" cy="5.2" r="2.2"/><path d="M4 13.2c.4-2.3 2-3.6 4-3.6s3.6 1.3 4 3.6"/><circle cx="3.4" cy="6.6" r="1.5"/><circle cx="12.6" cy="6.6" r="1.5"/><path d="M1.4 11.6c.3-1.2 1-1.9 2-2.1M14.6 11.6c-.3-1.2-1-1.9-2-2.1"/>',
  motion: '<path d="M2.4 8h3l1.6-3.6 2 7.2 1.6-3.6h3"/>',
  tok: '<path d="M3 4.5h10M3 8h10M3 11.5h6"/>',
  monitor: '<rect x="2" y="2.8" width="12" height="8.4" rx="1.4"/><path d="M6 13.6h4M8 11.2v2.4"/>',
  swaph: '<path d="M2.6 5.4h10.2M10.4 3l2.4 2.4-2.4 2.4M13.4 10.6H3.2M5.6 8.2l-2.4 2.4 2.4 2.4"/>',
  wmax: '<rect x="3.5" y="3.5" width="9" height="9" rx="1"/>',
  wrest: '<rect x="3.5" y="5.5" width="7" height="7" rx="1"/><path d="M5.5 5.5v-1a1 1 0 0 1 1-1h5a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1h-1"/>',
  wclose: '<path d="m4 4 8 8M12 4l-8 8"/>',
  smile: '<circle cx="8" cy="8" r="5.8"/><path d="M5.6 9.6c.6.9 1.4 1.4 2.4 1.4s1.8-.5 2.4-1.4M6 6.4v.2M10 6.4v.2"/>',
  phone: '<rect x="4.4" y="1.8" width="7.2" height="12.4" rx="1.8"/><path d="M7.2 11.8h1.6"/>',
  download: '<path d="M8 2.6v7.6M4.8 7.2 8 10.4l3.2-3.2M3 13.2h10"/>',
  opts: '<rect x="2.2" y="2.6" width="6.4" height="8.2" rx="1.3"/><rect x="7.4" y="5.2" width="6.4" height="8.2" rx="1.3"/>',
  sliders: '<path d="M3 4.6h10M3 8h10M3 11.4h10"/><circle cx="6" cy="4.6" r="1.3"/><circle cx="10.4" cy="8" r="1.3"/><circle cx="5.4" cy="11.4" r="1.3"/>',
  palette: '<path d="M8 2.4a5.6 5.6 0 0 0 0 11.2c.9 0 1.4-.6 1.4-1.3 0-.8-.6-1.2-.6-1.9 0-.6.5-1.1 1.2-1.1h1.2a2.4 2.4 0 0 0 2.4-2.4C13.6 4.3 11.1 2.4 8 2.4z"/><circle cx="6" cy="6" r=".8" fill="currentColor"/><circle cx="8.6" cy="5" r=".8" fill="currentColor"/><circle cx="5.2" cy="9" r=".8" fill="currentColor"/>',
  plug: '<path d="M6 2.6v3.2M10 2.6v3.2M4 5.8h8v2.4a4 4 0 0 1-8 0z"/><path d="M8 12.2v1.4"/>',
  grid: '<rect x="2.4" y="2.4" width="4.6" height="4.6" rx="1.1"/><rect x="9" y="2.4" width="4.6" height="4.6" rx="1.1"/><rect x="2.4" y="9" width="4.6" height="4.6" rx="1.1"/><rect x="9" y="9" width="4.6" height="4.6" rx="1.1"/>',
  book: '<path d="M2.6 3.2h4.2c.8 0 1.2.5 1.2 1.2v8.4c0-.7-.4-1.2-1.2-1.2H2.6zM13.4 3.2H9.2c-.8 0-1.2.5-1.2 1.2v8.4c0-.7.4-1.2 1.2-1.2h4.2z"/>',
  box: '<path d="M2.4 5.6 8 2.6l5.6 3v4.8L8 13.4l-5.6-3z"/><path d="M2.4 5.6 8 8.6l5.6-3M8 8.6v4.8"/>',
  activity: '<path d="M1.8 8.6h3l1.6-4 2.4 7.2 1.6-4h3.8"/>',
  share: '<circle cx="12.4" cy="3.8" r="1.8"/><circle cx="3.6" cy="8" r="1.8"/><circle cx="12.4" cy="12.2" r="1.8"/><path d="m5.2 7.1 5.6-2.4M5.2 8.9l5.6 2.4"/>',
  archive: '<rect x="2.2" y="3" width="11.6" height="3" rx="1"/><path d="M3.4 6v6.4a1 1 0 0 0 1 1h7.2a1 1 0 0 0 1-1V6M6.4 8.8h3.2"/>',
  cpu: '<rect x="4.4" y="4.4" width="7.2" height="7.2" rx="1.2"/><path d="M6.6 2v2.4M9.4 2v2.4M6.6 11.6V14M9.4 11.6V14M2 6.6h2.4M2 9.4h2.4M11.6 6.6H14M11.6 9.4H14"/>',
};
const icon = (n, cls = "") => `<svg class="ic ${cls}" viewBox="0 0 16 16" aria-hidden="true">${P[n]}</svg>`;
$$("[data-icon]").forEach(el => (el.innerHTML = icon(el.dataset.icon)));

// ------------------------------------------------------------------ state
let snap = { name: "", agents: [], approvals: [], tickets: [], total_cost_usd: 0, router_cost_usd: 0, workspace: "" };
let prefs = { theme: "system", tabs: [], active_tab: 0, check_updates: true };
let machines = [], share = null, upd = null, hasProject = true;
const S = {
  mode: "chat", codeView: "agents", chatView: "chat", wb: "canvases",
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
// Settings → Appearance owns these, on top of the light/dark theme.
// Contrast dims or sharpens the secondary text ramp, as a straight linear
// interpolation from the background toward each level's designed colour.
const RAMP = {
  light: { bg: [255, 255, 255], fg: [[28, 26, 31], [78, 73, 85], [125, 119, 133], [168, 162, 174]] },
  dark: { bg: [23, 22, 27], fg: [[236, 232, 239], [189, 182, 195], [138, 131, 144], [95, 89, 101]] },
};
const mixHex = (from, to, t) => "#" + from.map((c, i) => Math.round(c + (to[i] - c) * t).toString(16).padStart(2, "0")).join("");
function contrastVars(pal, contrast) {
  const t = Math.min(1, Math.max(0, (contrast - 0.6) / 0.8));
  return pal.fg.map((c, i) => [`--fg${i ? "-" + (i + 1) : ""}`, mixHex(pal.bg, c, t)]);
}
function applyAppearance() {
  const r = document.documentElement, a = prefs.appearance || {};
  const dark = r.dataset.theme === "dark" || (!r.dataset.theme && matchMedia("(prefers-color-scheme: dark)").matches);
  const set = (k, v) => { if (v) r.style.setProperty(k, v); else r.style.removeProperty(k); };
  set("--bs-accent", (dark && a.accent_dark) || a.accent);
  set("--page-bg", a.bg);
  set("--fg", a.fg);
  set("--page-fg", a.fg);
  set("--font-ui", a.ui_font);
  set("--font-mono", a.code_font);
  // Contrast only moves the secondary levels; an explicit foreground wins.
  for (const [k, v] of contrastVars(RAMP[dark ? "dark" : "light"], a.contrast > 0 ? a.contrast : 1)) {
    if (!a.fg || k !== "--fg") set(k, v);
  }
  const trans = a.translucent === false && !!prefs.appearance;
  set("--glass", trans ? (dark ? "rgb(36, 30, 38)" : "rgb(252, 249, 252)") : null);
  set("--glass-side", trans ? (dark ? "rgb(48, 42, 50)" : "rgb(244, 241, 245)") : null);
  set("--glass-main", trans ? (dark ? "rgb(20, 17, 22)" : "rgb(246, 243, 247)") : null);
}
function applyTheme() {
  const r = document.documentElement;
  if (prefs.theme === "light" || prefs.theme === "dark") r.dataset.theme = prefs.theme; else delete r.dataset.theme;
  applyAppearance();
  if (window.Art && Art.current()) { Art.apply(Art.current()); }
  if (window.Apps) Apps.sendTheme();
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

// The rail's zones (Chat, Code, Memory, Apps, and each open app) share the
// window; the sidebar, title bar and body follow. Code has two views: Pair
// (you and one CLI in the project) and Agents (the workbench).
const agentsOn = () => S.mode === "code" && S.codeView === "agents";
function setMode(m, save = true) {
  const uses = (prefs.uses && prefs.uses.length ? prefs.uses : ["chat", "code"]);
  if ((m === "chat" || m === "code") && !uses.includes(m)) m = uses[0];
  if (!m || (m.startsWith("app:") && !(window.Apps && Apps.get(m.slice(4))))) m = uses[0];
  if (!["chat", "code", "memory", "apps"].includes(m) && !m.startsWith("app:")) m = uses[0];
  S.mode = m;
  const w = $("#win");
  for (const z of ["chat", "code", "memory", "apps"]) w.classList.toggle("m-" + z, m === z);
  w.classList.toggle("m-app", m.startsWith("app:"));
  w.classList.toggle("pair-on", m === "code" && S.codeView === "pair");
  w.classList.toggle("agents-on", m === "chat" && S.chatView === "agents");
  w.classList.toggle("single-use", uses.length === 1);
  if (S.settings) { S.settings = false; $("#settings").hidden = true; }
  if (S.drawer && !agentsOn()) setDrawer(false);
  if (save) { prefs.mode = m; invoke("set_mode", { mode: m }); }
  if (window.Shell) Shell.layout();
  renderList();
  if (agentsOn() || (window.Shell && Shell.showing("canvas"))) { renderTabs(); renderGrid(); }
  if (window.Chat && window.Shell && Shell.showing("chat")) Chat.render();
}
function setCodeView(v) {
  S.codeView = v === "pair" ? "pair" : "agents";
  prefs.code_view = S.codeView; invoke("set_code_view", { view: S.codeView });
  setMode("code");
}
function setChatView(v) {
  S.chatView = v === "agents" ? "agents" : "chat";
  prefs.chat_view = S.chatView; invoke("set_view_prefs", { chatView: S.chatView, motion: null });
  setMode("chat");
}

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
  const items = [["projects", "folder", "Projects"], ["agents", "sparkle", "Workers"]];
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
  if (!agentsOn()) { if (window.Shell) Shell.renderSide(); return; }
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
    if (window.Shell) Shell.layout();
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
function renderCard() {
  if (!hasProject) { $("#ports").hidden = true; return; }
  $("#ports").hidden = false;
  const run = snap.agents.filter(a => a.status === "running").length;
  const wts = snap.agents.filter(a => a.branch && a.id !== MAIN).length;
  const row = (c, l, v) => `<div class="port"><span class="dot ${c}"></span><span class="pl">${l}</span><span class="pn" style="margin-left:auto;color:var(--fg-3)">${v}</span></div>`;
  $("#ports").innerHTML = `<div class="ports-h">This run · ${esc(snap.name || machine().name)}</div>` +
    row(run ? "run" : "acc", "Workers running", run) + row(pending().length ? "wait" : "", "Waiting on you", pending().length) +
    row("done", "Worktrees", wts) + row("", "Spent", `$${snap.total_cost_usd.toFixed(3)} · router $${snap.router_cost_usd.toFixed(3)}`);
}
// The footer's machines: each by name, with a dot for how it is: green
// while agents work on it, orange when it is idle, red when unreachable.
function machineState(m) {
  if (m.link.state === "offline") return ["red", "Unreachable: " + (m.link.error || "no answer")];
  if (m.link.state === "connecting") return ["orange", "Connecting…"];
  return m.running > 0 || m.pending > 0 ? ["green", `${m.running} working${m.pending ? `, ${m.pending} waiting on you` : ""}`] : ["orange", "Idle: no agents working"];
}
function renderMachines() {
  $("#machines").innerHTML = machines.map(m => {
    const [c, why] = machineState(m);
    const tip = `${m.name}${m.local ? " (this machine)" : " · " + m.url}${m.project ? " · " + m.project : ""} · ${why}`;
    return `<button class="mach" aria-pressed="${m.selected}" data-m="${m.index}" title="${esc(tip)}" aria-label="${esc(tip)}">${icon(m.local ? "pc" : "cloud")}<span class="mn">${esc(m.name)}</span><span class="md ${c}"></span></button>`;
  }).join("") + `<button class="mach add" id="addMachine" title="Add a machine" aria-label="Add a machine">${icon("plus")}</button>`;
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
const VIEWS = [["board", "bubble", "Team chat"], ["conversation", "reply", "Conversation"], ["terminal", "term", "Terminal"], ["files", "folder", "Worktree files"], ["browser", "globe", "Browser"], ["diagram", "diagram", "Diagram review"], ["docs", "doc", "PLAN.md"], ["options", "opts", "Visual options"], ["empty", "window", "Empty canvas"]];
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
  if (e.key === "k") { e.preventDefault(); Shell.search(); return; }
  if (e.key === "o") { e.preventDefault(); pickProject(); return; }
  if (e.key === "j") { e.preventDefault(); setMode(S.mode === "chat" ? "code" : "chat"); return; }
  if (e.key === "/" ) { e.preventDefault(); if (window.Shell) Shell.toggleSplit(); return; }
  if (!agentsOn()) { if (e.key === ",") { e.preventDefault(); openSettings(); } if (e.key === "\\") { e.preventDefault(); setSide($("#win").classList.contains("side-closed")); } return; }
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
    case "options": { const o = (snap.options || []).find(x => x.id === p.option) || (snap.options || []).slice(-1)[0]; return o ? `Options · ${o.title}` : "Visual options"; }
    case "trace": return `Trace · ${a ? (a.id === MAIN ? "main" : a.key) : ""}`;
    default: return "New canvas";
  }
}
const paneIcon = p => ({ board: '<span class="agent">' + icon("bubble") + "</span>", agent: '<span class="agent">' + icon("sparkle") + "</span>", files: '<span class="term">' + icon("folder") + "</span>", browser: '<span class="term">' + icon("globe") + "</span>", diagram: '<span class="term">' + icon("diagram") + "</span>", docs: '<span class="term">' + icon("doc") + "</span>", options: '<span class="term">' + icon("opts") + "</span>" })[p.kind] || '<span class="term">' + icon("window") + "</span>";

function buildPane(p, slot) {
  const el = document.createElement("section");
  el.className = "pane" + (slot === S.focus ? " on" : "");
  el.dataset.slot = slot;
  const ps = pstate(slot);
  el.innerHTML = `<header class="ph">${paneIcon(p)}<span class="t">${esc(paneTitle(p))}</span><span class="sp"></span>
    ${p.kind === "agent" && busy(agent(p.agent)) ? `<button class="ib stopb" data-act="stop" aria-label="Stop this agent" title="Stop">${icon("stop")}</button>` : ""}
    ${p.kind === "agent" ? `<button class="ib" data-act="trace" aria-label="Trace" title="Trace: every tool call, timed">${icon("trace")}</button><button class="ib" data-act="files" aria-label="Open worktree files">${icon("folder")}</button>` : ""}
    ${p.kind !== "empty" ? `<button class="ib" data-act="swap" aria-label="Show something else here" title="Show something else here">${icon("window")}</button>` : ""}
    ${tab().panes.length > 1 ? `<button class="ib" data-act="max" aria-label="Maximize canvas">${icon("expand")}</button>` : ""}
    <button class="ib" data-act="close" aria-label="Close canvas">${icon("close")}</button></header>`;
  el.querySelector(".ph").addEventListener("pointerdown", e => { if (!e.target.closest("button")) paneDrag(e, el, slot); });
  el.addEventListener("pointerdown", () => { if (S.focus !== slot) { S.focus = slot; $$(".pane").forEach(x => x.classList.toggle("on", +x.dataset.slot === slot)); renderList(); } });
  const act = (k, f) => { const b = el.querySelector(`[data-act=${k}]`); if (b) b.onclick = f; };
  act("max", () => { S.max = S.max == null ? slot : null; renderGrid(); });
  act("swap", () => { setPane(slot, { kind: "empty", agent: 0, url: null }); });
  act("files", () => setPane(slot, { kind: "files", agent: p.agent, url: null }));
  act("trace", () => setPane(slot, { kind: "trace", agent: p.agent, url: null }));
  act("stop", () => invoke("stop", { agent: p.agent }));
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
function setPane(slot, spec) {
  // A pane that held a shell must not leak it when it becomes something else.
  const ps = pstate(slot);
  if (ps.cleanup && spec.kind !== tab().panes[slot].kind) ps.cleanup();
  tab().panes[slot] = spec; S.pane.delete(prefs.active_tab + ":" + slot); saveTabs(); renderTabs(); renderGrid();
}

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
  else if (p.kind === "conversation") conversationPane(p, body, ps);
  else if (p.kind === "terminal") terminalPane(body, ps);
  else if (p.kind === "options") optionsPane(p, body, ps);
  else if (p.kind === "trace") tracePane(p, body, ps);
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
    <button class="card" data-k="board">${icon("bubble")}<span class="ct">Team chat</span><span class="cs">What agents tell each other, across models and CLIs</span></button>
    <button class="card" data-k="conversation">${icon("reply")}<span class="ct">Conversation</span><span class="cs">One thread between two agents, or with you</span></button>
    <button class="card" data-k="terminal">${icon("term")}<span class="ct">Terminal</span><span class="cs">A real shell in this project's folder</span></button>
    <button class="card" data-k="options">${icon("opts")}<span class="ct">Visual options</span><span class="cs">Variants an agent wants you to choose between</span></button></div>
    ${agents.length > 1 ? `<h5>Agent sessions</h5><div class="alist">${agents.filter(a => a.id !== MAIN).map(a => `<button class="chip-btn" data-a="${a.id}">${icon("sparkle")}${esc(a.title)}<span class="dot ${dotFor(a.status)}"></span></button>`).join("")}</div>` : ""}</div>`;
  $$("[data-k]", body).forEach(b => (b.onclick = () => setPane(slot, { kind: b.dataset.k, agent: 0, url: null })));
  $$("[data-a]", body).forEach(b => (b.onclick = () => setPane(slot, { kind: "agent", agent: +b.dataset.a, url: null })));
}

// ------------------------------------------------------------------ workbench views
// The same project three ways: Canvases (tabs of split canvases), Wall (every
// agent's session tiled, as many as there are) and Inbox (what needs you,
// then what is running, then what is done). Picked per user, kept in prefs.
const WB = [["home", "home", "Home", "Start work, and what needs you"], ["canvases", "window", "Canvases", "Sessions, files and previews side by side"], ["wall", "monitor", "Wall", "Every agent's session, tiled"]];
const busy = a => !!a && ["running", "queued", "awaiting_approval"].includes(a.status);
const lastLine = a => { const e = [...a.log].reverse().find(e => e.kind !== "user") || a.log[a.log.length - 1]; return e ? e.text.split("\n")[0] : "No activity yet"; };
// Why an agent needs the human, or null.
function needsYou(a) {
  const ap = pending().find(x => x.agent === a.id);
  if (ap) return { why: ap.kind === "plan" ? `Plan · ${ap.tickets.length} tickets to approve` : a.id === MAIN ? "Final deliverable to review" : "Work to review", ap };
  if (a.status === "failed") return { why: "Failed" };
  const t = a.ticket && snap.tickets.find(x => x.key === a.ticket);
  if (t && (t.state === "needs_info" || t.state === "ready_for_human")) return { why: tLabel(t.state) };
  const last = a.log[a.log.length - 1];
  if (a.id === MAIN && a.status === "idle" && last && last.kind === "assistant") return { why: "Asked you something", reply: true };
  return null;
}
// The view menu in the title bar: Home, Canvases, Wall.
function renderWbv() {
  const n = snap.agents.filter(needsYou).length, cur = WB.find(([k]) => k === S.wb) || WB[0];
  const b = $("#viewBtn");
  b.innerHTML = `${icon(cur[1])}<span>${cur[2]}</span>${n && S.wb !== "home" ? `<i class="rb">${n}</i>` : ""}${icon("updown", "ud")}`;
  b.onclick = () => openPop(b, `<div class="vmenu">${WB.map(([k, ic, l, d]) => `<button class="pi${S.wb === k ? " on" : ""}" data-wb="${k}" ${!hasProject && k !== "home" ? "disabled" : ""}>${icon(ic)}<span class="vt"><b>${l}</b><small>${d}</small></span>${k === "home" && n ? `<i class="rb">${n}</i>` : ""}</button>`).join("")}</div>`, pop => {
    $$("[data-wb]", pop).forEach(x => (x.onclick = () => { closePop(); setWb(x.dataset.wb); }));
  });
  $("#win").classList.toggle("wb-alt", S.wb !== "canvases" || !hasProject);
}
function setWb(v) {
  S.wb = WB.some(([k]) => k === v) ? v : "home";
  prefs.wb_view = S.wb; invoke("set_view_prefs", { chatView: null, motion: null, wbView: S.wb });
  renderGrid();
}
// Open an agent in the canvases, from the wall or the inbox.
function openIn(spec) { if (S.wb !== "canvases") setWb("canvases"); openView(spec); }
function wallTile(a) {
  const nd = needsYou(a);
  return `<section class="pane tile${nd ? " needs" : ""}" data-tile="${a.id}"><header class="ph"><span class="agent">${icon("sparkle")}</span><span class="t">${esc(a.id === MAIN ? "Main agent" : a.title)}</span><span class="dot ${dotFor(a.status)}"></span><span class="sp"></span><span class="rt">${esc(a.decision ? a.decision.model : "")}</span>
    ${busy(a) ? `<button class="ib stopb" data-act="stop" aria-label="Stop" title="Stop">${icon("stop")}</button>` : ""}<button class="ib" data-act="open" aria-label="Open in a canvas" title="Open in a canvas">${icon("expand")}</button></header>
    <div class="tb">${a.log.length ? a.log.slice(-80).map(logHTML).join("") : `<div class="ln raw dim">${a.status === "queued" ? "Queued: starts when its blockers are merged" : "No activity yet"}</div>`}${a.status === "running" ? `<div class="ln spin"><span class="spin-g">${SPIN[spinI]}</span> Working…</div>` : ""}</div>
    ${nd ? `<button class="tile-need" data-act="need">${icon("warn")}${esc(nd.why)}</button>` : ""}</section>`;
}
// Tiles update in place (by agent) so each keeps its scroll position.
function renderWall(g) {
  const list = treeOrder().map(agent).filter(a => a.kind !== "triage");
  if (g.className !== "grid wall") { g.className = "grid wall"; g.innerHTML = ""; }
  if (!list.length) { g.innerHTML = `<div class="nothing">No agents yet. Give the main agent a goal from the Inbox or a canvas.</div>`; return; }
  const empty = g.querySelector(".nothing"); if (empty) empty.remove();
  const keep = new Set();
  list.forEach(a => {
    keep.add(String(a.id));
    let el = g.querySelector(`[data-tile="${a.id}"]`);
    const html = wallTile(a);
    if (el && el.__h === html) return;
    const tb = el && el.querySelector(".tb");
    const atBottom = !tb || tb.scrollHeight - tb.scrollTop - tb.clientHeight < 40, top = tb ? tb.scrollTop : 0;
    if (el) el.outerHTML = html; else g.insertAdjacentHTML("beforeend", html);
    el = g.querySelector(`[data-tile="${a.id}"]`); el.__h = html;
    const nb = el.querySelector(".tb"); nb.scrollTop = atBottom ? nb.scrollHeight : top;
    el.querySelector("[data-act=open]").onclick = () => openIn({ kind: "agent", agent: a.id });
    const st = el.querySelector("[data-act=stop]"); if (st) st.onclick = () => invoke("stop", { agent: a.id });
    const need = el.querySelector("[data-act=need]"); if (need) need.onclick = () => actOn(a);
  });
  $$("[data-tile]", g).forEach(el => { if (!keep.has(el.dataset.tile)) el.remove(); });
}
function actOn(a) {
  const nd = needsYou(a);
  if (nd && nd.ap) { S.review = nd.ap.id; if (S.wb !== "canvases") setWb("canvases"); showDiagram(); }
  else if (a.ticket && nd && a.status !== "failed") { S.ticket = a.ticket; setDrawer(true); }
  else openIn({ kind: "agent", agent: a.id });
}
function renderAlt() {
  const g = $("#grid"); S.els = new Map(); S.ph = null;
  if (S.wb === "wall") renderWall(g); else Home.render(g);
}

// Canvases sit absolutely at the rectangles of the tab's layout tree, so a
// new arrangement (a drag preview, a resize) is a style change the browser
// animates rather than a rebuild.
// Rich answers (OpenUI) in Chat: on unless turned off in Settings.
function richOn() { try { return localStorage.getItem("bs.rich") !== "0"; } catch { return true; } }

// The painting is at full strength only on Home (Code's workbench, Home view).
function syncArt() {
  if (window.Art) Art.setVivid(S.mode === "code" && S.codeView !== "pair" && !S.settings && (!hasProject || S.wb === "home"));
}
function renderGrid() {
  syncArt();
  const g = $("#grid"), t = tab();
  if (!hasProject) { renderWbv(); S.els = new Map(); S.ph = null; Home.render(g); return; }
  renderWbv();
  if (S.wb !== "canvases") { renderAlt(); return; }
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
  renderWbv();
  if (S.wb !== "canvases") { renderAlt(); return; }
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
  const i = t.panes.findIndex(p => p.kind === spec.kind && (spec.kind === "options" ? (p.option || "") === (spec.option || "") : !["agent", "files"].includes(spec.kind) || p.agent === spec.agent));
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
    const replyTo = inp.dataset.replyTo ? +inp.dataset.replyTo : null;
    try { await invoke("post_message", { to: to.value, text: inp.value.trim(), replyTo }); inp.value = ""; delete inp.dataset.replyTo; inp.placeholder = "Message the team"; } catch (e) { toast(String(e), "err"); }
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
    const re = m.reply_to != null ? `<span class="bm-re">↩ #${m.reply_to}</span>` : "";
    return `<div class="bm${me ? " me" : ""}" data-mid="${m.id}"><div class="bm-h"><b>${esc(f.t)}</b>${f.run ? `<span class="bm-run">${esc(f.run)}</span>` : ""}${re}<span class="bm-to">→ ${m.to === "all" ? "everyone" : esc(who(m.to).t)}</span><span class="bm-at">${new Date(m.at).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</span><button class="ib bm-reply" data-reply="${m.id}" title="Answer this" aria-label="Answer this">${icon("reply")}</button></div><div class="bm-t">${esc(m.text)}</div></div>`;
  }).join("") : `<div class="nothing">No messages yet. Agents post here to coordinate (interfaces, shared files, questions), whether they run on an API model or in Claude Code, Codex or another CLI.</div>`;
  if (atBottom) box.scrollTop = box.scrollHeight;
  $$("[data-reply]", box).forEach(b => (b.onclick = () => {
    const id = +b.dataset.reply, m = ms.find(x => x.id === id); if (!m) return;
    const inp = body.querySelector(".bin"), sel = body.querySelector(".bto");
    if (sel) sel.value = m.from === "you" ? m.to : m.from;
    inp.value = "";
    inp.placeholder = `Answering #${id} (${who(m.from).t})`;
    inp.focus();
    inp.dataset.replyTo = String(id);
  }));
  const sel = body.querySelector(".bto");
  if (sel && sel.options.length !== snap.agents.length + 1) { const v = sel.value; sel.innerHTML = [["all", "Everyone"], ...snap.agents.map(a => [a.key, `${a.title} (${a.key})`])].map(([k, l]) => `<option value="${esc(k)}">${esc(l)}</option>`).join(""); sel.value = v; }
}

// ------------------------------------------------------------------ conversation canvas
// One conversation between two agents (or you and one), read from the board's
// threads. The same messages Team chat shows, filtered to the pair.
function boardThread(m) {
  if (m.thread) return m.thread;
  const f = m.from, t = m.to;
  return t !== "all" && t !== f ? [f, t].sort().join("|") : f + "~all";
}
function boardThreads() {
  const out = new Map();
  (snap.board || []).forEach(m => {
    const id = boardThread(m);
    let th = out.get(id);
    if (!th) { th = { id, sides: [], messages: [], last: 0 }; out.set(id, th); }
    [m.from, m.to].forEach(s => { if (s !== "all" && !th.sides.includes(s)) th.sides.push(s); });
    th.sides.sort();
    th.messages.push(m);
    th.last = Math.max(th.last, m.at);
  });
  return [...out.values()].sort((a, b) => b.last - a.last || (a.id < b.id ? -1 : 1));
}
function conversationPane(p, body, ps) {
  const all = snap.agents.map(a => a.key);
  const threads = boardThreads();
  // Default to the busiest conversation: that is usually the one that matters.
  if (!ps.with && threads.length) ps.with = threads[0].id;
  const cur = threads.find(t => t.id === ps.with);
  const who = k => (k === "you" ? "You" : (snap.agents.find(a => a.key === k) || {}).title || k);
  const label = t => t.sides.map(who).join(" ↔ ") || t.id;
  body.innerHTML = `<div class="conv">
    <div class="conv-rail">${threads.length ? threads.map(t => `<button class="cv${t.id === ps.with ? " on" : ""}" data-th="${esc(t.id)}"><span class="cv-t">${esc(label(t))}</span><span class="cv-n">${t.messages.length}</span><small>${esc(t.messages[t.messages.length - 1].text.slice(0, 60))}</small></button>`).join("") : `<div class="nothing">No conversations yet. Agents post on the board as they work; each pair of them gets its own thread here.</div>`}</div>
    <div class="conv-main">
      ${cur ? `<header class="conv-h"><b>${esc(label(cur))}</b><span class="sp"></span><span class="bm-to">${cur.messages.length} message${cur.messages.length === 1 ? "" : "s"}</span></header>
      <div class="bmsgs">${cur.messages.map(m => {
        const me = m.from === "you";
        return `<div class="bm${me ? " me" : ""}"><div class="bm-h"><b>${esc(who(m.from))}</b>${m.reply_to != null ? `<span class="bm-re">↩ #${m.reply_to}</span>` : ""}<span class="bm-to">→ ${m.to === "all" ? "everyone" : esc(who(m.to))}</span><span class="bm-at">${new Date(m.at).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</span></div><div class="bm-t">${esc(m.text)}</div></div>`;
      }).join("")}</div>
      <footer class="bfoot"><select class="bto" aria-label="Reply as"></select><input class="bin" placeholder="Reply in this conversation" aria-label="Reply" autocomplete="off"><button class="btn primary sm bsend">Reply</button></footer>`
      : `<div class="nothing">Pick a conversation on the left.</div>`}
    </div></div>`;
  $$("[data-th]", body).forEach(b => (b.onclick = () => { ps.with = b.dataset.th; conversationPane(p, body, ps); }));
  const sel = body.querySelector(".bto"), inp = body.querySelector(".bin");
  if (!sel || !cur) return;
  const targets = cur.sides.filter(k => k !== "you");
  sel.innerHTML = [...targets.map(k => [k, who(k)]), ["all", "Everyone"]].map(([k, l]) => `<option value="${esc(k)}">${esc(l)}</option>`).join("");
  const last = cur.messages[cur.messages.length - 1];
  sel.value = last && targets.includes(last.from) ? last.from : targets[0] || all[0] || "all";
  const go = async () => {
    if (!inp.value.trim()) return;
    try { await invoke("post_message", { to: sel.value, text: inp.value.trim(), replyTo: last && last.from !== "you" ? last.id : null }); inp.value = ""; }
    catch (e) { toast(String(e), "err"); }
  };
  body.querySelector(".bsend").onclick = go;
  inp.onkeydown = e => { if (e.key === "Enter") go(); };
  requestAnimationFrame(() => { const b = body.querySelector(".conv-main .bmsgs"); if (b) b.scrollTop = b.scrollHeight; });
}

// ------------------------------------------------------------------ terminal canvas
// A real shell (a PTY: ConPTY on Windows, forkpty elsewhere) in the project's
// folder. Not a terminal emulator — the canvas draws plain text — so full-screen
// TUIs are not the point; running a build, a git command or a REPL is.
function terminalPane(body, ps) {
  const dir = snap.workspace || (machine() && machine().local ? "" : "");
  const history = (ps.lines = ps.lines || []);
  const draw = () => { body.querySelector(".tterm").textContent = history.join(""); body.querySelector(".tterm").scrollTop = body.querySelector(".tterm").scrollHeight; };
  body.innerHTML = `<div class="tterm" tabindex="0" aria-label="Terminal output"></div>
    <footer class="bfoot"><span class="tprompt">❯</span><input class="bin tin" placeholder="Run a command in ${esc(dir || "this project")}" aria-label="Command" autocomplete="off" spellcheck="false"><button class="btn sm" id="ttermInt">Ctrl-C</button><button class="btn sm" id="ttermNew">Restart</button></footer>`;
  draw();
  let closed = false, poll = null, sawOutput = false, quietT = null;
  const push = t => { history.push(t); if (history.length > 400) history.splice(0, history.length - 400); draw(); };
  const start = async () => {
    // A repaint reuses the shell this pane already opened: one shell per pane.
    if (ps.term && ps.alive !== false) { poll = watch(); return; }
    push(`$ ${dir || "(no project open)"}\n`);
    try { ps.term = await invoke("terminal_open", { path: dir }); }
    catch (e) { push(`cannot start a shell: ${e}\n`); return; }
    ps.alive = true;
    poll = watch();
    // Some Windows hosts open a ConPTY that never speaks. Say so, rather than
    // showing a blank canvas forever; the rest of the app does not depend on it.
    quietT = setTimeout(() => {
      if (!closed && !sawOutput) push("\n[no output from the shell. This host's PTY may be blocked; an agent canvas or `backspace mcp` runs commands without one.]\n");
    }, 6000);
  };
  const watch = () => setInterval(async () => {
    if (closed || !ps.term) return;
    try {
      const r = await invoke("terminal_read", { id: ps.term });
      if (r.output) { sawOutput = true; push(r.output); }
      if (!r.alive && !r.missing) { ps.alive = false; push("\n[shell exited]\n"); clearInterval(poll); }
    } catch { /* the pane is closing */ }
  }, 400);
  const inp = body.querySelector(".tin");
  const go = async () => {
    const line = inp.value; inp.value = "";
    if (!line.trim() && ps.term) return;
    push(`❯ ${line}\n`);
    if (!ps.term) { await start(); }
    try { await invoke("terminal_write", { id: ps.term, line }); } catch (e) { push(String(e) + "\n"); }
  };
  inp.onkeydown = e => { if (e.key === "Enter") go(); };
  body.querySelector("#ttermInt").onclick = () => { if (ps.term) invoke("terminal_signal", { id: ps.term }); };
  body.querySelector("#ttermNew").onclick = async () => { if (ps.term) invoke("terminal_close", { id: ps.term }).catch(() => {}); clearInterval(poll); closed = false; history.length = 0; ps.term = null; await start(); };
  // Leaving the pane (or the tab) must not leak a shell per visit.
  ps.cleanup = () => { closed = true; clearInterval(poll); clearTimeout(quietT); if (ps.term) invoke("terminal_close", { id: ps.term }).catch(() => {}); ps.term = null; ps.alive = false; };
  start();
  requestAnimationFrame(() => inp.focus());
}

// ------------------------------------------------------------------ visual options
// An agent shows a few variants — screenshots it wrote, or live pages — and
// the user picks one; the pick is that agent's tool result. Two ways to look:
// a row of thumbnails with Choose, and a pair side by side with a split.
function optThumb(x) {
  if (x.image) return `<div class="opt-thumb"><img data-img="${esc(x.image)}" alt="${esc(x.label)}"></div>`;
  if (x.url) return `<div class="opt-thumb"><iframe src="${esc(x.url)}" title="${esc(x.label)}" loading="lazy"></iframe></div>`;
  return `<div class="opt-thumb"><div class="blank">no preview</div></div>`;
}
function loadImgs(box, ps) {
  $$("img[data-img]", box).forEach(async im => {
    const src = im.dataset.img;
    if (/^(data:|https?:)/.test(src)) { im.src = src; return; }
    ps.imgs = ps.imgs || {};
    if (ps.imgs[src]) { im.src = ps.imgs[src]; return; }
    try { im.src = ps.imgs[src] = await invoke("read_image", { path: src }); }
    catch (e) { const d = document.createElement("div"); d.className = "blank"; d.textContent = "can't show " + src; im.replaceWith(d); }
  });
}
function optionsPane(p, body, ps) {
  const sets = snap.options || [];
  const set = sets.find(o => o.id === p.option) || sets[sets.length - 1];
  if (!set) { body.innerHTML = `<div class="nothing">No options yet. An agent shows a few variants here with the show_options tool.</div>`; return; }
  if (p.option !== set.id) p.option = set.id;
  const mode = ps.omode === "compare" ? "compare" : "grid";
  const decided = set.picked != null, picked = set.picked || "";
  const chosen = x => decided && x.id === picked;
  const head = `<header class="opt-h"><div class="ot"><h3>${esc(set.title)}</h3>${set.note ? `<p>${esc(set.note)}</p>` : ""}</div>
    <div class="seg" role="tablist"><button class="sg ${mode === "grid" ? "on" : ""}" data-om="grid">Options</button><button class="sg ${mode === "compare" ? "on" : ""}" data-om="compare">Compare</button></div></header>`;
  if (mode === "grid") {
    const sel = set.variants.some(v => v.id === ps.osel) ? ps.osel : set.variants[0].id;
    body.innerHTML = `<div class="opts">${head}
      <div class="opt-row">${set.variants.map((x, i) => `<div class="opt-card${x.id === sel ? " sel" : ""}${chosen(x) ? " chosen" : ""}" data-ov="${esc(x.id)}">
        <div class="opt-n">${i + 1}</div>${optThumb(x)}
        <div class="opt-lbl">${esc(x.label)}${chosen(x) ? `<span class="chip approved">chosen</span>` : ""}</div>
        <button class="opt-pick" data-pick="${esc(x.id)}">Choose</button></div>`).join("")}</div>
      <div class="opt-foot"><ul class="opt-notes">${set.variants.map(x => `<li><b>${esc(x.label)}.</b> ${esc(x.note)}</li>`).join("")}</ul>
        ${decided ? `<span class="chip approved">${picked ? "You chose " + esc((set.variants.find(v => v.id === picked) || {}).label || picked) : "Dismissed"}</span>`
          : `<button class="btn primary" data-pick="${esc(sel)}">Choose this version</button>`}</div></div>`;
  } else {
    const a = set.variants.find(v => v.id === ps.ovar) || set.variants[0];
    const b = set.variants.find(v => v.id === ps.ovar2) || set.variants.find(v => v.id !== a.id) || set.variants[0];
    const cut = ps.cut == null ? 50 : ps.cut;
    body.innerHTML = `<div class="opts cmp">${head}
      <div class="cmp-wrap">
        <div class="cmp-rail">${set.variants.map(x => `<button class="rv${x.id === a.id ? " on" : ""}" data-ov="${esc(x.id)}">${optThumb(x)}<span class="t">${esc(x.label)}</span>${chosen(x) ? `<span class="chip approved">chosen</span>` : ""}</button>`).join("")}</div>
        <div class="cmp-stage">
          <div class="cmp-top"><span>${esc(a.label)}</span><span class="cmp-vs">vs</span><span>${esc(b.label)}</span></div>
          <div class="cmp-pair">
            <div class="cmp-side" style="flex:0 0 ${cut}%">${optThumb(a)}</div>
            <div class="cmp-grip" title="Drag to compare"></div>
            <div class="cmp-side">${optThumb(b)}</div>
          </div>
          ${decided ? `<div class="cmp-foot"><span class="chip approved">${picked ? "You chose " + esc((set.variants.find(v => v.id === picked) || {}).label || picked) : "Dismissed"}</span></div>`
            : `<div class="cmp-foot">${[a, b].map(x => `<button class="btn${x.id === a.id ? " primary" : ""}" data-pick="${esc(x.id)}">Choose ${esc(x.label)}</button>`).join("")}</div>`}
        </div>
      </div></div>`;
  }
  loadImgs(body, ps);
  $$("[data-om]", body).forEach(x => (x.onclick = () => { ps.omode = x.dataset.om; optionsPane(p, body, ps); }));
  const a0 = mode === "compare" ? (set.variants.find(v => v.id === ps.ovar) || set.variants[0]).id : null;
  const b0 = mode === "compare" ? (set.variants.find(v => v.id === ps.ovar2) || set.variants[1] || set.variants[0]).id : null;
  $$("[data-ov]", body).forEach(el => (el.onclick = e => {
    if (e.target.closest("[data-pick]")) return;
    const id = el.dataset.ov;
    if (mode === "grid") ps.osel = id;
    else if (id === b0) { ps.ovar = b0; ps.ovar2 = a0; }
    else if (id !== a0) ps.ovar = id;
    else return;
    optionsPane(p, body, ps);
  }));
  $$("[data-pick]", body).forEach(el => (el.onclick = () => invoke("pick_option", { id: set.id, variant: el.dataset.pick })));
  const grip = body.querySelector(".cmp-grip");
  if (grip) grip.addEventListener("pointerdown", e => {
    e.preventDefault(); grip.setPointerCapture(e.pointerId);
    const pair = grip.parentElement, side = pair.querySelector(".cmp-side");
    const move = ev => { const r = pair.getBoundingClientRect(); ps.cut = Math.min(96, Math.max(4, ((ev.clientX - r.left) / r.width) * 100)); side.style.flexBasis = ps.cut + "%"; };
    const up = () => { grip.removeEventListener("pointermove", move); grip.removeEventListener("pointerup", up); };
    grip.addEventListener("pointermove", move); grip.addEventListener("pointerup", up);
  });
}

// ------------------------------------------------------------------ agents' canvases
// Agents open and close canvases (open_canvas / close_canvas): a browser on
// the dev server they started, their files, a session. Each op applies once;
// the ones from before this window opened are history, not news.
let canvasSeen = null;
const sameCanvas = (p, s) => p.kind === s.kind && (s.kind === "browser" ? (p.url || "") === (s.url || "") : s.kind === "options" ? (p.option || "") === (s.option || "") : !["agent", "files"].includes(s.kind) || p.agent === s.agent);
function applyCanvasOps() {
  const ops = snap.canvas_ops || [];
  if (!canvasSeen || canvasSeen.ws !== snap.workspace) { canvasSeen = { ws: snap.workspace, id: ops.length ? ops[ops.length - 1].id : 0 }; return; }
  let changed = false;
  for (const op of ops.filter(o => o.id > canvasSeen.id)) {
    canvasSeen.id = op.id;
    const who = snap.agents.find(x => x.key === op.agent);
    const spec = { kind: op.kind, agent: who ? who.id : 0, url: op.url || null, option: op.option || null };
    const label = op.kind === "browser" ? (op.url || "a page").replace(/^https?:\/\//, "") : { files: "files", agent: "a session", diagram: "the diagram", docs: "PLAN.md", board: "team chat", options: "visual options" }[op.kind];
    if (op.op === "close") {
      prefs.tabs.forEach(t => {
        for (let i = t.panes.length - 1; i >= 0; i--) {
          if (!sameCanvas(t.panes[i], spec)) continue;
          if (t.panes.length > 1) { t.layout = L.renumber(L.remove(tree(t), i), i); t.panes.splice(i, 1); }
          else t.panes[0] = { kind: "agent", agent: MAIN, url: null };
          changed = true;
        }
      });
      continue;
    }
    if (prefs.tabs.some(t => t.panes.some(p => sameCanvas(p, spec)))) continue;
    const t = tab();
    if (t.panes.length < 4) {
      const g = $("#grid"), tr = tree(t), n = t.panes.length;
      const { leaves } = L.layout(tr, { x: 0, y: 0, w: g.clientWidth || 1000, h: g.clientHeight || 600 }, 6);
      const f = leaves[leaves.length - 1];
      t.layout = L.insert(tr, f.leaf, f.r.w >= f.r.h ? "right" : "bottom", n);
      t.panes.push(spec);
    } else prefs.tabs.push({ name: null, panes: [spec], layout: null });
    changed = true;
    toast(`${op.by === "main" ? "The main agent" : op.by} opened ${label} in Canvases`, "ok");
  }
  if (changed) { S.pane.clear(); saveTabs(); if (S.wb === "canvases") { renderTabs(); renderGrid(); } }
}

// ------------------------------------------------------------------ notifications
// What needs you, told the way Settings → Notifications says: the system's
// notifications while the window is in the background, a toast otherwise.
function notifyUser(title, body, kind = "") {
  const mode = prefs.notify || "system";
  if (mode === "off") return;
  if (mode === "system" && !document.hasFocus()) invoke("notify", { title, body }).catch(() => toast(`${title}: ${body}`, kind));
  else toast(body ? `${title}: ${body}` : title, kind);
}
let seenAgents = null, seenPending = null, seenWs = null;
function watchAgents() {
  const st = new Map(snap.agents.map(a => [a.id, a.status]));
  const pend = new Set(pending().map(a => a.id));
  if (seenAgents && seenWs === snap.workspace) {
    for (const a of snap.agents) {
      const was = seenAgents.get(a.id), who = a.id === MAIN ? "The main agent" : a.title;
      if (!was || was === a.status || a.kind === "scout") continue;
      if (a.status === "failed") notifyUser(`${who} failed`, lastLine(a), "err");
      else if (a.id === MAIN && a.status === "approved") notifyUser(`${snap.name} is done`, "The final deliverable was accepted.", "ok");
      else if (a.id === MAIN && a.status === "idle" && was === "running") notifyUser("The main agent replied", lastLine(a));
    }
    for (const id of pend) if (!seenPending.has(id)) {
      const ap = snap.approvals[id];
      notifyUser(ap.kind === "plan" ? "A plan needs your approval" : "Work is ready for review", ap.kind === "plan" ? `${ap.tickets.length} tickets in ${snap.name}` : (agent(ap.agent) || {}).title || "", "ok");
    }
  }
  seenAgents = st; seenPending = pend; seenWs = snap.workspace;
}
// Chats: a reply that failed (a usage limit, a crash) or finished while you were away.
let seenChats = null;
function watchChats(list) {
  const now = new Map(list.map(t => [t.id, t]));
  if (seenChats) for (const t of list) {
    const was = seenChats.get(t.id);
    if (!was) continue;
    if (t.failed && !was.failed) notifyUser(`⚠ ${t.title || "A chat"} failed`, t.failed.slice(0, 140), "err");
    else if (was.busy && !t.busy && !t.failed && !document.hasFocus()) notifyUser(t.title || "Reply ready", t.preview.slice(0, 140));
  }
  seenChats = now;
}

// ------------------------------------------------------------------ trace canvas
// An agent's runs as waterfalls: one row per tool call on a shared clock,
// its input and output a click away. Traces live in <data>/traces.
async function tracePane(p, body, ps) {
  const a = agent(p.agent);
  const ids = a ? a.traces || [] : [];
  if (!ids.length) { body.innerHTML = `<div class="nothing">No traces yet. ${a && busy(a) ? "This run's trace is saved when it ends." : "Each run of this agent leaves one."}</div>`; return; }
  const id = ps.trace && ids.includes(ps.trace) ? ps.trace : ids[ids.length - 1];
  const t = await invoke("trace_get", { id }).catch(() => null);
  if (!t) { body.innerHTML = `<div class="nothing">That trace is gone.</div>`; return; }
  const t0 = Math.min(...t.spans.map(s => s.start)), t1 = Math.max(...t.spans.map(s => s.end || s.start)), span = Math.max(1, t1 - t0);
  const ms = n => (n < 1000 ? `${n} ms` : n < 60000 ? `${(n / 1000).toFixed(1)} s` : `${Math.floor(n / 60000)}m ${Math.round((n % 60000) / 1000)}s`);
  const root = t.spans[0], at = root.attrs || {};
  body.innerHTML = `<div class="trace">
    <div class="tr-h"><select class="tx" aria-label="Run">${ids.map((x, i) => `<option value="${x}" ${x === id ? "selected" : ""}>Run ${i + 1}${x === ids[ids.length - 1] ? " (latest)" : ""}</option>`).join("")}</select>
      <span>${esc(at["gen_ai.response.model"] || at["gen_ai.request.model"] || "")}</span><span>${ms(span)}</span>
      ${at["gen_ai.usage.input_tokens"] != null ? `<span>${Number(at["gen_ai.usage.input_tokens"]).toLocaleString()}↓ ${Number(at["gen_ai.usage.output_tokens"] || 0).toLocaleString()}↑</span>` : ""}
      ${at["cost.usd"] != null ? `<span>$${Number(at["cost.usd"]).toFixed(3)}</span>` : ""}${root.error ? `<span class="err">${esc(root.error)}</span>` : ""}</div>
    <div class="tr-rows">${t.spans.map((s, i) => `<details class="tr-row${s.error ? " bad" : ""}"><summary><span class="tn">${esc(i ? s.name : root.name)}</span><span class="tbar"><i style="left:${((s.start - t0) / span) * 100}%;width:${Math.max(0.6, (((s.end || s.start) - s.start) / span) * 100)}%"></i></span><span class="td">${ms((s.end || s.start) - s.start)}</span></summary>
      ${s.input ? `<pre>${esc(s.input)}</pre>` : ""}${s.output ? `<pre class="out">${esc(s.output)}</pre>` : ""}</details>`).join("")}</div></div>`;
  body.querySelector("select").onchange = e => { ps.trace = e.target.value; tracePane(p, body, ps); };
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
  const dir = (agent(MAIN) && agent(MAIN).worktree) || snap.workspace;
  const text = dir ? await invoke("read_file", { path: dir + "/PLAN.md" }).catch(() => null) : null;
  body.innerHTML = `<div class="docs"><article class="doc">${text ? md(text) : `<h1>PLAN.md</h1><div class="by">${esc(snap.name)}</div><p>The main agent writes PLAN.md before it proposes tickets. It will show here once it exists.</p>`}</article></div>`;
}

// ------------------------------------------------------------------ settings
function openSettings(section) {
  S.settings = true; $("#settings").hidden = false; renderSettings(); if (window.Shell) Shell.layout();
  refreshCompanion();
  if (section) { const el = $(`#set-${section}`), box = $("#settings"); if (el) box.scrollTop = el.getBoundingClientRect().top - box.getBoundingClientRect().top + box.scrollTop - 12; const f = $(`#set-${section} input`); if (f) f.focus({ preventScroll: true }); }
  refreshShare();
}
function closeSettings() { if (!S.settings) return; S.settings = false; $("#settings").hidden = true; if (window.Shell) Shell.layout(); }
let companion = null;
async function refreshCompanion() { companion = await invoke("companion_status").catch(() => null); if (S.settings) renderSettings(); }
async function refreshShare() { share = await invoke("share_status").catch(() => null); if (S.settings) renderSettings(); }
// Settings → Appearance. The panel writes prefs.appearance and hands it to
// the shell command; applyAppearance() turns it into custom properties.
const ACCENTS = ["#cf5a24", "#2a66d9", "#187f45", "#6a4fd6", "#bf3a2f", "#1c1a1f"];
function appearancePanel() {
  const a = prefs.appearance || {}; const c = a.contrast > 0 ? a.contrast : 1;
  const sw = (k, v, cls = "") => `<label class="swatch ${cls}"><input type="color" data-ap="${k}" value="${esc(v || "#ffffff")}" aria-label="${k}"><span style="background:${esc(v || "#ffffff")}"></span></label>`;
  const font = (k, cur, ph) => `<input class="tx" style="width:190px" data-apv="${k}" value="${esc(cur || "")}" placeholder="${ph}" aria-label="${k}">`;
  return `<div class="ap">
    <div class="line"><span class="lab">Theme</span><div class="segc">${[["system", "System"], ["light", "Light"], ["dark", "Dark"]].map(([k, l]) => `<button aria-pressed="${(prefs.theme || "system") === k}" data-theme="${k}">${l}</button>`).join("")}</div></div>
    <div class="line"><span class="lab">Accent<small>Tabs, selection, the Choose pill. One colour per theme.</small></span><div class="ap-row">
      ${ACCENTS.map(h => `<button class="ap-chip${(a.accent || "#cf5a24") === h ? " on" : ""}" data-apset="accent" data-aphex="${h}" style="background:${h}" aria-label="Accent ${h}"></button>`).join("")}
      ${sw("accent", a.accent || "#cf5a24")}${sw("accent_dark", a.accent_dark || a.accent || "#f08a5d", "dark")}
      <button class="btn" data-apreset="all">Reset</button></div></div>
    <div class="line"><span class="lab">Background<small>Pages, docs and the canvases' paper.</small></span><div class="ap-row">${sw("bg", a.bg || "#ffffff")}<button class="btn" data-apreset="bg">Reset</button></div></div>
    <div class="line"><span class="lab">Foreground<small>Body text, on the background above.</small></span><div class="ap-row">${sw("fg", a.fg || "#1b1a1f")}<button class="btn" data-apreset="fg">Reset</button></div></div>
    <div class="line"><span class="lab">UI font<small>Empty uses Inter, then the system face.</small></span>${font("ui_font", a.ui_font, "Inter")}</div>
    <div class="line"><span class="lab">Code font<small>Logs, diffs and monospace blocks.</small></span>${font("code_font", a.code_font, "JetBrains Mono")}</div>
    <div class="line"><span class="lab">Translucent window<small>Off makes the panes solid, for a busy backdrop or a screenshot.</small></span><button class="toggle" role="switch" id="apTrans" aria-checked="${a.translucent !== false}" aria-label="Translucent window"></button></div>
    <div class="line"><span class="lab">Contrast<small>${c.toFixed(2)}× — dims the secondary text, or sharpens it.</small></span><input type="range" class="ap-range" id="apCon" min="0.6" max="1.4" step="0.05" value="${c}" aria-label="Contrast"></div>
  </div>`;
}
const APPEARANCE_KEYS = ["accent", "accent_dark", "bg", "fg", "ui_font", "code_font", "translucent", "contrast"];
function bindAppearance(el) {
  const read = () => ({ ...(prefs.appearance || {}) });
  const save = async a => { prefs.appearance = a; applyTheme(); await invoke("set_appearance", { appearance: a }); renderSettings(); };
  $$("[data-ap]", el).forEach(i => (i.oninput = () => { const a = read(); a[i.dataset.ap] = i.value; prefs.appearance = a; applyTheme(); }));
  $$("[data-ap]", el).forEach(i => (i.onchange = () => save(read())));
  $$("[data-apv]", el).forEach(i => (i.onchange = () => { const a = read(); a[i.dataset.apv] = i.value.trim(); save(a); }));
  $$("[data-aphex]", el).forEach(b => (b.onclick = () => { const a = read(); a[b.dataset.apset] = b.dataset.aphex; save(a); }));
  $$("[data-apreset]", el).forEach(b => (b.onclick = () => {
    const a = read(), k = b.dataset.apreset;
    if (k === "all") APPEARANCE_KEYS.forEach(x => delete a[x]); else if (k === "bg") a.bg = ""; else a.fg = "";
    save(a);
  }));
  const t = $("#apTrans", el); if (t) t.onclick = () => { const a = read(); a.translucent = t.getAttribute("aria-checked") !== "true"; save(a); };
  const cr = $("#apCon", el); if (cr) {
    cr.oninput = () => { const a = read(); a.contrast = +cr.value; prefs.appearance = a; applyAppearance(); el.querySelector("#apCon").previousElementSibling.querySelector("small").textContent = (+cr.value).toFixed(2) + "× — dims the secondary text, or sharpens it."; };
    cr.onchange = () => save(read());
  }
}
// Settings → what each pane is, and where the left nav points.
const SET_NAV = [
  ["Personal", [["general", "General", "sliders"], ["appearance", "Appearance", "palette"], ["notify", "Notifications", "bubble"], ["chat", "Chat behavior", "compose"], ["phone", "Phone", "phone"]]],
  ["Integrations", [["providers", "Agent providers", "plug"], ["mcp", "MCP and connections", "grid"], ["machines", "Machines", "cloud"]]],
  ["Coding", [["coding", "Models and writing", "codei"], ["accounts", "Accounts", "route"], ["models", "Local models", "cpu"], ["skills", "Agent skills", "book"], ["worktrees", "Managed worktrees", "branch"], ["colony", "Workspaces and Colony", "box"]]],
  ["System", [["tracing", "Tracing", "activity"], ["projects", "Projects", "folder"], ["share", "Share this machine", "share"], ["updates", "Updates", "download"]]],
];
function renderSettings() {
  const el = $("#settings");
  const keep = {}; $$("input.tx[id]", el).forEach(i => (keep[i.id] = i.value));
  const th = prefs.theme || "system";
  const sh = share || { enabled: false, addr: "127.0.0.1:7420", token: "", error: null, sharing: false };
  const u = upd && upd !== "checking" ? upd : null;
  if (!S.setSection) S.setSection = "general";
  const on = k => (k === S.setSection ? " on" : "");
  const nav = SET_NAV.map(([g, items]) => `<div class="sg"><span>${g}</span>${items.map(([k, l, ic]) => `<button class="sv${on(k)}" data-setnav="${k}">${icon(ic)}${l}</button>`).join("")}</div>`).join("")
    + `<div class="sg"><span>Archived</span><button class="sv${on("home")}" data-setnav="home">${icon("archive")}Archived threads</button></div>`;
  el.innerHTML = `<div class="setwrap"><nav class="setnav" aria-label="Settings sections">${nav}</nav><div class="setbody"><div class="set">
    <div class="set-h"><h1>Settings</h1><span class="sp"></span><button class="btn" id="setRerun">Run setup again</button><button class="ib" id="setClose" aria-label="Close settings">${icon("close")}</button></div>
    <section id="set-general"><h2>General</h2><p class="lead">What Backspace is for, where it looks for projects, and the open project.</p>
      <div class="line"><span class="lab">What shows in the app<small>With both on, switch from the top of the sidebar.</small></span><div class="segc">${[["chat", "Chat"], ["code", "Coding"]].map(([k, l]) => `<button aria-pressed="${(prefs.uses || ["chat", "code"]).includes(k)}" data-use="${k}">${l}</button>`).join("")}</div></div>
      ${hasProject ? `<div class="line"><span class="lab">Workspace<small class="mono">${esc(snap.workspace || "—")}</small></span></div>
      <div class="line"><span class="lab">Config<small class="mono">${esc(snap.config_source || "built-in defaults")}</small></span></div>` : ""}</section>
    <section id="set-appearance"><h2>Appearance</h2><p class="lead">A theme, then the colours and fonts on top of it.</p>
      ${appearancePanel()}
      <div class="line stack"><span class="lab">Background<small>Behind the window, and across the top of Home.</small></span><div class="bdrops">${Art.BACKDROPS.map(([k, l]) => `<button class="bdrop" aria-pressed="${Art.current() === k}" data-bd="${k}" title="${l}"><span class="sw sw-${k}" ${Art.src(k) ? `style="background-image:url('${Art.src(k)}')"` : ""}></span><span>${l}</span></button>`).join("")}</div></div></section>
    <section id="set-notify"><h2>Notifications</h2><div class="line"><span class="lab">When something needs you<small>A plan to approve, work to review, an agent that failed, a chat that hit a limit.</small></span><div class="segc">${[["system", "System"], ["app", "In the app"], ["off", "Off"]].map(([k, l]) => `<button aria-pressed="${(prefs.notify || "system") === k}" data-nt="${k}">${l}</button>`).join("")}</div></div>
      <div class="line"><span class="lab"></span><button class="btn" id="ntTest">Send a test notification</button></div></section>
    <section id="set-chat"><h2>Chat behavior</h2><div class="line"><span class="lab">New chats go to<small>You can switch per chat from the composer.</small></span><button class="btn" id="setRoute" data-popper>${esc(window.Chat ? Chat.routeName(prefs.default_route) : "Pick")}</button></div>
      <div class="line"><span class="lab">Rich answers<small>Models may answer with charts, tables, steps and forms (OpenUI). Off: Markdown only, and a shorter prompt.</small></span><button class="toggle" role="switch" id="richT" aria-checked="${richOn()}" aria-label="Rich answers"></button></div></section>
    <section id="set-phone"><h2>Phone</h2><p class="lead">The Backspace phone app (coming soon) follows your chats, saves notes to Memory and approves a project's work from anywhere on the same network. Turn this on and scan the code with it to pair; plain HTTP with a token, so use it on a network you trust (or Tailscale).</p>
      <div class="line"><span class="lab">Let my phone connect<small>${companion && companion.error ? esc(companion.error) : companion && companion.running ? `Listening on ${esc(companion.url || companion.addr)}` : "Off"}</small></span><button class="toggle" role="switch" id="phoneT" aria-checked="${!!(companion && companion.enabled)}" aria-label="Let my phone connect"></button></div>
      ${companion && companion.enabled && companion.pair ? `<div class="pair"><div class="qr" id="phoneQr" aria-label="Pairing code"></div><div class="pair-t"><b>Scan with the Backspace app</b><span>Or enter <code>${esc(companion.url)}</code> and the token by hand.</span><div class="pair-b"><button class="btn" id="phoneCopy">${icon("copy")}Copy pairing link</button><button class="btn" id="phoneNew">Unpair all phones</button></div></div></div>` : companion && companion.enabled && !companion.url ? `<p class="warnp">${icon("warn")}This machine has no address on a local network, so a phone can't reach it.</p>` : ""}</section>
    <section id="set-providers"><h2>Agent providers</h2><p class="lead">What Chat and projects can use. Backspace scans this machine for each CLI, its version and whether it is signed in; switch off any you don't want offered.</p><div id="setProviders"></div>
      <div class="line"><span class="lab">Backspace Cloud</span><span class="lead" style="margin:0">Chat without installing anything. Free with a short ad under each reply, or a paid plan.</span></div><div id="setPlan"></div></section>
    <section id="set-mcp"><h2>MCP and connections</h2><p class="lead">Outside tools a bot can be given through the Model Context Protocol. Add a server by command or URL; every bot you give it to can call its tools.</p><div id="setConnections"></div></section>
    <section id="set-machines"><h2>Machines</h2><p class="lead">Follow and drive the harness on other machines. Each one runs <code>backspace-cli serve</code> (or shares from its own Backspace, below); add it with its address and token. Switch machines from the sidebar foot.</p>
      ${machines.map(m => `<div class="mrow"><span class="mi">${icon(m.local ? "pc" : "cloud")}</span><span class="lab">${esc(m.name)}${m.selected ? " · showing" : ""}<small>${m.local ? "This machine" : esc(m.url)} · ${m.link.state === "offline" ? "offline: " + esc(m.link.error) : m.link.state}${m.project ? " · " + esc(m.project) : ""}</small></span>
        ${m.selected ? "" : `<button class="btn" data-show="${m.index}">Show</button>`}${m.local ? "" : `<button class="btn danger" data-rm="${m.index}">Remove</button>`}</div>`).join("")}
      <div class="form3"><input class="tx" id="mName" placeholder="Name, e.g. Cloud" aria-label="Machine name"><input class="tx mono" id="mUrl" placeholder="http://10.0.0.5:7420" aria-label="Address"><input class="tx mono" id="mTok" placeholder="Token" aria-label="Token"><button class="btn primary" id="mAdd">Add</button></div>
      <div class="err" id="mErr" role="alert"></div></section>
    <section id="set-coding"><h2>Models and writing</h2><p class="lead">A project's planner runs on an API model (Anthropic or OpenRouter, keys below). Each ticket's worker can run on the router's pick, or be handed to a coding CLI you're signed in to: it works in the ticket's own git worktree, then goes through the same checks and your review.</p>
      <div class="line stack"><span class="lab">Agents run on<small>The planner and the workers, what they may do, and how hard they think. Applies when a project opens.${hasProject ? ` <button class="lnk" id="reopen">Reopen ${esc(snap.name)} now</button>` : ""}</small></span><div class="segc" id="workerSeg"></div></div>
      <div id="keyRows"></div></section>
    <section id="set-accounts"><h2>Accounts</h2><p class="lead">Several logins per coding CLI, each in its own profile directory, kept apart. When one hits its usage limit it is parked and the next turn picks another — never on a plain rate limit, only when the CLI itself reports a limit. Backspace never holds the credentials: the CLI signs in to its own profile directory.</p><div id="setAccounts"></div></section>
    <section id="set-models"><h2>Local models</h2><p class="lead">Small models on this machine, for the small jobs a big agent delegates: reading, classifying, summarising, extracting. Ollama, LM Studio, llama.cpp and MLX are all found where they run.</p><div id="setLocal"></div></section>
    <section id="set-skills"><h2>Agent skills</h2><p class="lead">Markdown playbooks an agent loads on demand. Each role gets some injected; every agent can load any of them. Drop a folder with a <code>SKILL.md</code> into <code>~/.config/backspace/skills/</code> or <code>&lt;workspace&gt;/.backspace/skills/</code> to replace one or add your own.</p><div id="setSkills"></div></section>
    <section id="set-worktrees"><h2>Managed worktrees</h2><p class="lead">Each ticket's agent gets its own git branch and worktree, forked from its parent's branch. Worktrees are kept after a run so you can reopen a ticket; each one carries its own copy of dependencies.</p><div id="setWorktrees"></div></section>
    <section id="set-colony"><h2>Workspaces and Colony</h2><p class="lead">Colony gives an agent a real machine of its own: a WSL2 distro or a Hyper-V VM, started on demand, addressable by name, thrown away when the task is done. Backspace drives visor-core's JSON CLI.</p><div id="setColony"></div></section>
    <section id="set-tracing"><h2>Tracing</h2><p class="lead">Every reply keeps a trace on this machine: each tool call, how long it took, tokens and cost (the timeline button under a reply). To watch them elsewhere, send them to an OpenTelemetry collector such as treg, Jaeger or Langfuse.</p>
      <div class="line"><span class="lab">Collector<small>OTLP over HTTP; <code>/v1/traces</code> is added for you. Empty keeps traces here only.</small></span><input class="tx mono" id="trEp" value="${esc((prefs.tracing || {}).endpoint || "")}" placeholder="https://collector.example.com" style="width:260px" aria-label="Collector address"></div>
      <div class="line"><span class="lab">Header<small>For sign-in, e.g. <code>authorization: Bearer …</code></small></span><input class="tx mono" id="trHdr" value="${esc(Object.entries((prefs.tracing || {}).headers || {}).map(([k, v]) => k + ": " + v).join("; "))}" placeholder="authorization: Bearer …" style="width:260px" aria-label="Header"></div>
      <div class="line"><span class="lab">Send prompts and output too<small>Off: timings, models, token counts and tool names only.</small></span><button class="toggle" role="switch" id="trContent" aria-checked="${!!(prefs.tracing || {}).content}" aria-label="Send prompts and output too"></button></div>
      <div class="line"><span class="lab"></span><button class="btn" id="trTest">Send a test trace</button><button class="btn primary" id="trSave">Save</button></div></section>
    <section id="set-projects"><h2>Projects</h2>
      ${(prefs.projects || []).length ? prefs.projects.map(p => `<div class="mrow"><span class="mi">${icon("folder")}</span><span class="lab">${esc(base(p))}${hasProject && p === snap.workspace ? " · open" : ""}<small class="mono">${esc(p)}</small></span><button class="btn" data-popen="${esc(p)}">Open</button><button class="btn danger" data-pforget="${esc(p)}">Forget</button></div>`).join("") : `<p class="lead">No projects yet.</p>`}
      <div class="line"><span class="lab"></span><button class="btn primary" id="setOpenFolder">Open folder…</button></div></section>
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
    <section id="set-home"><h2>Archived threads</h2><p class="lead">Threads you archived. They keep their messages and can be restored from Chat.</p><div id="setArchived"></div></section>
  </div></div></div>`;
  Object.entries(keep).forEach(([id, v]) => { const i = $("#" + id, el); if (i && v) i.value = v; });
  const showSection = k => {
    if (!SET_NAV.flatMap(g => g[1]).some(([x]) => x === k) && k !== "home") k = "general";
    S.setSection = k;
    $$("[data-setnav]", el).forEach(b => b.classList.toggle("on", b.dataset.setnav === k));
    $$(".setbody > .set > section", el).forEach(s => (s.hidden = s.id !== "set-" + k));
    $(".setbody", el).scrollTop = 0;
    const nav2 = $(`[data-setnav="${k}"]`, el); if (nav2) nav2.scrollIntoView({ block: "nearest" });
  };
  $$("[data-setnav]", el).forEach(b => (b.onclick = () => showSection(b.dataset.setnav)));
  showSection(S.setSection);
  bindAppearance(el);
  $("#setClose").onclick = closeSettings;
  const pt = $("#phoneT", el);
  const trCfg = () => {
    const headers = {};
    $("#trHdr").value.split(";").map(h => h.trim()).filter(Boolean).forEach(h => { const i = h.indexOf(":"); if (i > 0) headers[h.slice(0, i).trim().toLowerCase()] = h.slice(i + 1).trim(); });
    return { endpoint: $("#trEp").value.trim(), headers, content: $("#trContent").getAttribute("aria-checked") === "true" };
  };
  $("#trContent").onclick = e => e.currentTarget.setAttribute("aria-checked", String(e.currentTarget.getAttribute("aria-checked") !== "true"));
  $("#trSave").onclick = async () => { const cfg = trCfg(); await invoke("set_tracing", { cfg }).catch(e => toast(String(e), "err")); prefs.tracing = cfg; toast(cfg.endpoint ? "Traces will also go to " + cfg.endpoint : "Traces stay on this machine", "ok"); };
  $("#trTest").onclick = async () => { try { await invoke("trace_test", { cfg: trCfg() }); toast("The collector accepted a test trace", "ok"); } catch (e) { toast(String(e), "err"); } };
  if (pt) pt.onclick = async () => { companion = await invoke("set_companion", { enabled: !(companion && companion.enabled), newToken: false }).catch(e => (toast(String(e), "err"), companion)); setTimeout(refreshCompanion, 400); renderSettings(); };
  const pq = $("#phoneQr", el);
  if (pq && companion.pair) invoke("qr_svg", { text: companion.pair }).then(svg => { if (pq.isConnected) pq.innerHTML = svg; }).catch(() => {});
  const pc = $("#phoneCopy", el); if (pc) pc.onclick = () => { navigator.clipboard.writeText(companion.pair); toast("Pairing link copied", "ok"); };
  const pn = $("#phoneNew", el); if (pn) pn.onclick = async () => { companion = await invoke("set_companion", { enabled: true, newToken: true }); toast("Every paired phone was signed out", "ok"); renderSettings(); };
  $$("[data-use]", el).forEach(b => (b.onclick = async () => {
    const cur = prefs.uses && prefs.uses.length ? prefs.uses : ["chat", "code"], k = b.dataset.use;
    const next = ["chat", "code"].filter(u => (u === k ? !cur.includes(u) : cur.includes(u)));
    if (!next.length) { toast("Keep at least one on", "err"); return; }
    prefs.uses = next; await invoke("set_uses", { uses: next });
    setMode(S.mode, false); openSettings("uses");
  }));
  $("#setRerun").onclick = () => { closeSettings(); Onboard.open(); };
  if (window.Providers) { Providers.mount($("#setProviders")); Providers.mountPlan($("#setPlan")); }
  renderConnections(el); renderSkills(el); renderColony(el); renderArchived(el); renderAccounts(el);
  if (Models.mount) Models.mount($("#setLocal"), $("#setWorktrees"));
  $("#setRoute").onclick = e => Chat.routePicker(e.currentTarget, prefs.default_route, async r => { prefs.default_route = r; await invoke("set_default_route", { route: r }); renderSettings(); });
  $("#setOpenFolder").onclick = pickProject;
  renderCoding(el);
  $$("[data-popen]", el).forEach(b => (b.onclick = () => { closeSettings(); openProject(b.dataset.popen); }));
  $$("[data-pforget]", el).forEach(b => (b.onclick = async () => { await invoke("forget_project", { path: b.dataset.pforget }); prefs = await invoke("prefs"); renderSettings(); renderList(); }));
  $$("[data-theme]", el).forEach(b => (b.onclick = async () => { prefs.theme = b.dataset.theme; applyTheme(); renderSettings(); await invoke("set_theme", { theme: prefs.theme }); }));
  $$("[data-nt]", el).forEach(b => (b.onclick = async () => { prefs.notify = b.dataset.nt; await invoke("set_notify", { notify: prefs.notify }); renderSettings(); }));
  const nt = $("#ntTest", el); if (nt) nt.onclick = () => (prefs.notify === "app" ? toast("Backspace: notifications work.", "ok") : invoke("notify", { title: "Backspace", body: "Notifications work. You'll hear from your agents here." }).catch(e => toast(String(e), "err")));
  const rt = $("#richT", el); if (rt) rt.onclick = () => { const on = !richOn(); try { localStorage.setItem("bs.rich", on ? "1" : "0"); } catch {} if (window.setRich) window.setRich(on); renderSettings(); };
  $$("[data-bd]", el).forEach(b => (b.onclick = async () => { prefs.backdrop = b.dataset.bd; Art.apply(prefs.backdrop); renderSettings(); await invoke("set_backdrop", { backdrop: prefs.backdrop }); }));
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
  const perm = { edits: "Auto-accept edits", auto: "Auto", full: "Full access" }[prefs.cli_permission || "edits"];
  const cli = Picker.cliOf(prefs.worker), eff = Effort.name(cli, prefs.cli_effort);
  seg.innerHTML = `<button class="btn" data-mpick>${Logos.mark(cli)}${esc(await Picker.label(prefs.worker))} · ${perm}${eff ? ` · ${eff}` : ""}${icon("chevd")}</button>`;
  seg.querySelector("button").onclick = e => Picker.open(e.currentTarget, { model: prefs.worker || "", permission: prefs.cli_permission || "edits", effort: prefs.cli_effort || null }, async c => {
    prefs.worker = prefs.planner = c.model || null; prefs.cli_permission = c.permission; prefs.cli_effort = c.effort;
    await invoke("set_agent_run", { planner: prefs.planner, worker: prefs.worker, permission: c.permission, effort: c.effort });
  });
  const ro = $("#reopen", el); if (ro) ro.onclick = () => { const p = snap.workspace; closeSettings(); openProject(p); };
  const keys = (await invoke("api_keys").catch(() => null)) || [];
  const names = { ANTHROPIC_API_KEY: ["Anthropic", "Claude models for the planner and workers"], OPENROUTER_API_KEY: ["OpenRouter", "Any model through one key"], TYPESAFE_API_KEY: ["Jev router", "Optional: smarter model choice per agent"] };
  $("#keyRows", el).innerHTML = keys.map(([k, inApp, env]) => `<div class="line"><span class="lab">${names[k][0]}<small>${names[k][1]} · ${inApp ? "saved in Backspace" : env ? "from your environment" : "not set"}</small></span>
    <input class="tx mono" type="password" data-key="${k}" placeholder="${inApp || env ? "••••••••" : k}" style="width:220px" aria-label="${names[k][0]} key"><button class="btn" data-savekey="${k}">Save</button></div>`).join("");
  $$("[data-savekey]", el).forEach(b => (b.onclick = async () => {
    const i = $(`[data-key="${b.dataset.savekey}"]`, el);
    try { await invoke("set_api_key", { name: b.dataset.savekey, value: i.value }); toast(i.value ? "Key saved" : "Key removed", "ok"); renderCoding(el); } catch (e) { toast(String(e), "err"); }
  }));
}

// Settings → Accounts: several logins per CLI, kept apart (accounts.rs).
// "Switch now" walks to the next usable login; Automatic only fails over when
// the CLI itself reports a usage limit.
const ACCOUNT_CLIS = [["claude", "Claude Code"], ["codex", "Codex"], ["cursor", "Cursor"], ["opencode", "OpenCode"], ["gemini", "Gemini"], ["copilot", "Copilot"]];
async function renderAccounts(el) {
  const box = $("#setAccounts", el); if (!box) return;
  let d;
  try { d = await invoke("accounts"); } catch (e) { box.innerHTML = `<div class="err">${esc(String(e))}</div>`; return; }
  if (!box.isConnected) return;
  const hs = window.Providers ? Providers.list : [];
  const known = id => hs.some(h => h.id === id && h.installed);
  const st = a => (a.health && a.health.state) || "unknown";
  const cool = a => a.health && a.health.cooldown_until > Date.now();
  const when = ms => new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  box.innerHTML = `<div class="line"><span class="lab">When a limit is hit<small>Automatic switches to the next login and carries on; Manual leaves it to you; Off turns the whole registry off.</small></span>
      <div class="segc">${[["automatic", "Automatic"], ["manual", "Manual"], ["off", "Off"]].map(([k, l]) => `<button aria-pressed="${d.policy === k}" data-pol="${k}">${l}</button>`).join("")}</div></div>
    ${d.accounts.length ? d.accounts.map(a => `<div class="prow${a.enabled ? "" : " dim"}"><span class="mi">${icon("route")}</span>
      <div class="pmain"><div class="pname">${esc(a.name)} <span class="pver">${esc(a.provider)}</span></div>
        <span class="pst ${cool(a) ? "warn" : st(a) === "healthy" ? "ok" : "off"}">${cool(a) ? `parked until ${esc(when(a.health.cooldown_until))}` : esc(st(a))}${a.health.fails_24h ? ` · ${a.health.fails_24h} failure${a.health.fails_24h === 1 ? "" : "s"} in 24h` : ""}</span>
        <div class="pmsg mono">${esc(a.profile_dir || "the CLI's own login")}</div>
        ${a.usage && a.usage.report ? `<div class="pmsg">${esc(a.usage.report)}</div>` : ""}</div>
      <button class="toggle" role="switch" aria-checked="${a.enabled}" data-aon="${esc(a.id)}" aria-label="Use ${esc(a.name)}"></button>
      <button class="ib" data-arm="${esc(a.id)}" title="Remove" aria-label="Remove">${icon("trash")}</button></div>`).join("")
      : `<p class="lead">No extra logins yet. Add one below and sign that CLI in inside its own profile directory (the CLI's own sign-in command; Backspace never sees the credential).</p>`}
    <div class="form3"><select class="tx" id="aCli" aria-label="CLI">${ACCOUNT_CLIS.map(([k, l]) => `<option value="${k}" ${known(k) ? "" : ""}>${l}${known(k) ? "" : " (not installed)"}</option>`).join("")}</select><input class="tx" id="aName" placeholder="Name, e.g. Work Claude" aria-label="Account name"><span></span><button class="btn primary" id="aAdd">Add account</button></div>
    <div class="err" id="aErr" role="alert"></div>
    <div class="line"><span class="lab">Switch now<small>${d.accounts.length ? "Moves to the next usable login and marks it used." : ""}</small></span><button class="btn" id="aNext">Next account</button></div>
    <div class="line"><span class="lab">Registry<small class="mono">${esc(d.path)}</small></span></div>`;
  $$("[data-pol]", box).forEach(b => (b.onclick = async () => { await invoke("set_accounts_policy", { policy: b.dataset.pol }).catch(e => toast(String(e), "err")); renderAccounts(el); }));
  $$("[data-aon]", box).forEach(b => (b.onclick = async () => { await invoke("account_set", { id: b.dataset.aon, enabled: b.getAttribute("aria-checked") !== "true", priority: null }).catch(e => toast(String(e), "err")); renderAccounts(el); }));
  $$("[data-arm]", box).forEach(b => (b.onclick = async () => { await invoke("account_remove", { id: b.dataset.arm }).catch(e => toast(String(e), "err")); renderAccounts(el); }));
  const add = async () => {
    const err = $("#aErr", box); err.textContent = "";
    try { await invoke("account_add", { provider: $("#aCli", box).value, name: $("#aName", box).value }); toast("Account added; sign that CLI in to its profile directory", "ok"); renderAccounts(el); }
    catch (e) { err.textContent = String(e); }
  };
  $("#aAdd", box).onclick = add;
  $("#aNext", box).onclick = async () => {
    try { const r = await invoke("account_next"); toast(r.next ? `Next turn runs as ${r.next}` : "No other login is usable right now", r.next ? "ok" : "err"); renderAccounts(el); }
    catch (e) { toast(String(e), "err"); }
  };
}

// Settings → MCP and connections: outside tool servers a bot can be given.
// A bot's Connected apps decides which of these it actually gets.
function renderConnections(el) {
  const box = $("#setConnections", el); if (!box) return;
  const draw = list => {
    box.innerHTML = (list.length ? list.map(c => `<div class="mrow"><span class="mi">${icon("grid")}</span><span class="lab">${esc(c.name)}<small class="mono">${esc(c.target)}</small></span><button class="ib" data-crm="${esc(c.id)}" title="Remove" aria-label="Remove">${icon("trash")}</button></div>`).join("")
      : `<p class="lead">No servers yet. Add one by command or URL; bots you give it to can call its tools.</p>`)
      + `<div class="form3"><input class="tx" id="cName" placeholder="Name, e.g. GitHub" aria-label="Server name"><input class="tx mono" id="cTarget" placeholder="npx -y @modelcontextprotocol/server-github" aria-label="Command or URL"><button class="btn primary" id="cAdd">Add</button></div><div class="err" id="cErr" role="alert"></div>`;
    const add = async () => {
      const err = $("#cErr", box), name = $("#cName", box).value.trim(), target = $("#cTarget", box).value.trim();
      err.textContent = "";
      if (!name || !target) { err.textContent = "A server needs a name and a command or URL."; return; }
      try { list = await invoke("set_connections", { list: [...list, { id: "", name, target }] }); draw(list); }
      catch (e) { err.textContent = String(e); }
    };
    $("#cAdd", box).onclick = add;
    $$("[data-crm]", box).forEach(b => (b.onclick = async () => { draw(await invoke("set_connections", { list: list.filter(x => x.id !== b.dataset.crm) })); }));
  };
  draw(prefs.connections || []);
}

// Settings → Agent skills: every playbook this machine can load, and which
// role has which injected (the config decides; this is what it resolved to).
async function renderSkills(el) {
  const box = $("#setSkills", el); if (!box) return;
  box.innerHTML = `<div class="nothing">Reading skills…</div>`;
  let d;
  try { d = await invoke("skills_catalog"); } catch (e) { box.innerHTML = `<div class="err">${esc(String(e))}</div>`; return; }
  if (!box.isConnected) return;
  const role = (label, names) => `<div class="line"><span class="lab">${label}<small>${names.length ? names.map(esc).join(", ") : "none"}</small></span></div>`;
  box.innerHTML = role("Main agent", d.workflow.main_skills) + role("Workers", d.workflow.worker_skills) + role("Triage", d.workflow.triage_skills)
    + `<div class="line"><span class="lab">Every skill here<small>Any agent can load one with the skill tool, whether or not it is injected.</small></span></div>`
    + `<div class="skilllist">${d.skills.map(s => `<div class="skill"><b>${esc(s.name)}</b><span>${esc(s.description || "No description.")}</span>${s.files.length > 1 ? `<small class="mono">${s.files.map(esc).join(", ")}</small>` : ""}</div>`).join("")}</div>`
    + `<div class="line"><span class="lab">Your own<small class="mono">${d.dirs.map(esc).join(" · ")}</small></span></div>`;
}

// Settings → Managed worktrees: what each agent's ticket left on disk.
async function renderWorktrees(el) {
  const box = $("#setWorktrees", el); if (!box) return;
  let d;
  try { d = await invoke("worktrees"); } catch { box.innerHTML = ""; return; }
  if (!box.isConnected) return;
  if (!d.worktrees.length) { box.innerHTML = `<p class="lead">No worktrees on this machine yet. Each ticket's agent gets one while it works.</p>`; return; }
  const total = d.worktrees.reduce((n, w) => n + (w.size_bytes || 0), 0);
  box.innerHTML = `<p class="lead">${d.worktrees.length} worktree${d.worktrees.length === 1 ? "" : "s"} · ${Models.size(total)} under <code class="mono">${esc((d.root || "") + "/.backspace/worktrees")}</code></p>`
    + d.worktrees.map(w => `<div class="mrow"><span class="mi">${icon("branch")}</span><span class="lab">${esc(w.agent)} · ${esc(w.title)}<small class="mono">${esc(w.branch || "")} · ${esc(w.path)}${w.exists ? "" : " · missing"}</small></span><span class="chip">${Models.size(w.size_bytes || 0)}</span><button class="btn" data-reveal="${esc(w.path)}">Reveal</button></div>`).join("");
  $$("[data-reveal]", box).forEach(b => (b.onclick = () => invoke("reveal_path", { path: b.dataset.reveal }).catch(e => toast(String(e), "err"))));
}

// Settings → Workspaces and Colony: the durable little machines an agent can
// be given (docs/COLONY.md). visor-core is optional; this says what is
// missing rather than failing.
async function renderColony(el) {
  const box = $("#setColony", el); if (!box) return;
  box.innerHTML = `<div class="nothing">Checking visor-core…</div>`;
  let d;
  try { d = await invoke("colony_status"); } catch (e) { box.innerHTML = `<div class="err">${esc(String(e))}</div>`; return; }
  if (!box.isConnected) return;
  const caps = d.capabilities;
  const capLine = caps ? `<div class="line"><span class="lab">This machine<small>${[caps.wsl ? "WSL2" : null, caps.hyperv ? "Hyper-V" : null, caps.elevated ? "elevated" : "not elevated", (caps.providers || []).join(", ")].filter(Boolean).map(esc).join(" · ") || "nothing reported"}</small></span></div>` : "";
  box.innerHTML = `<div class="line"><span class="lab">visor-core<small class="mono">${esc(d.visor || "not found")}</small></span></div>`
    + capLine
    + (d.message ? `<p class="warnp">${icon("warn")}${esc(d.message)}</p>` : "")
    + `<div class="line"><span class="lab">Workspaces<small>Named, disposable machines. An agent creates one, runs commands in it, and destroys it; see the colony_* tools.</small></span></div>`
    + (d.workspaces && d.workspaces.length
      ? d.workspaces.map(w => `<div class="mrow"><span class="mi">${icon("box")}</span><span class="lab">${esc(w.name)}<small>${esc(w.provider)} · ${esc(w.os)} · ${esc(w.state)}${w.ip ? " · " + esc(w.ip) : ""}</small></span></div>`).join("")
      : `<p class="lead">None yet.</p>`)
    + `<pre class="cmd">cargo build --manifest-path capsule/core/Cargo.toml   # build visor-core${"\n"}cargo test -p backspace-colony                        # the registry, no VM needed</pre>`;
}

// Settings → Archived threads. Chat lists pinned threads at the top; this is
// the place to see and restore them.
async function renderArchived(el) {
  const box = $("#setArchived", el); if (!box) return;
  let list = [];
  try { list = await invoke("chat_list"); } catch { }
  const arc = list.filter(t => t.pinned);
  box.innerHTML = arc.length
    ? `<p class="lead">Pinned threads (${arc.length}). Unpin one to drop it back into the list.</p>`
      + arc.map(t => `<div class="mrow"><span class="mi">${icon("compose")}</span><span class="lab">${esc(t.title || "Untitled")}<small>${esc(t.preview || "")}</small></span><button class="btn" data-unarc="${esc(t.id)}">Unpin</button></div>`).join("")
    : `<p class="lead">No pinned threads.</p>`;
  $$("[data-unarc]", box).forEach(b => (b.onclick = async () => { await invoke("chat_pin", { id: b.dataset.unarc, pinned: false }).catch(e => toast(String(e), "err")); renderArchived(el); }));
}

// Settings → Local models, and the sizes the worktree list shows.
var Models = (() => {
  const size = b => { const u = ["B", "KiB", "MiB", "GiB", "TiB"]; let i = 0, n = b || 0; while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; } return (i ? n.toFixed(1) : n) + " " + u[i]; };
  const KIND = { ollama: "Ollama", mlx: "MLX", lmstudio: "LM Studio", llamacpp: "llama.cpp", custom: "Custom" };
  let last = null;
  async function draw(box, wtBox) {
    box.innerHTML = `<div class="nothing">Looking for Ollama, LM Studio, llama.cpp and MLX…</div>`;
    let d;
    try { d = await invoke("local_models"); } catch (e) { box.innerHTML = `<div class="err">${esc(String(e))}</div>`; return; }
    last = d;
    if (!box.isConnected) return;
    const chosen = d.chosen || "";
    const runtimes = d.runtimes || [];
    const tiny = runtimes.flatMap(r => r.models.filter(m => m.tiny).map(m => ({ ...m, runtime: r.id, up: r.running })));
    box.innerHTML = `<div class="line"><span class="lab">Small jobs go to<small>Reading, classifying, summarising and extracting, when a subscription model delegates them. One small local model, one task at a time.</small></span>
        <button class="btn" id="mmPick" data-popper>${chosen ? esc(chosen) : "Leave it to the main model"}${icon("chevd")}</button></div>
      ${tiny.length ? `<div class="pmodels">${tiny.slice(0, 10).map(m => `<span>${esc(m.id)}${m.up ? "" : " (stopped)"}</span>`).join("")}</div>` : ""}
      ${runtimes.map(r => `<div class="prow${r.running ? "" : " dim"}"><span class="mi">${icon("cpu")}</span>
        <div class="pmain"><div class="pname">${esc(r.name)} <span class="pver">${esc(KIND[r.kind] || r.kind)}</span></div>
          <span class="pst ${r.running ? "ok" : "off"}">${r.running ? `${r.models.length} model${r.models.length === 1 ? "" : "s"}` : r.installed ? "Not running" : "Not installed"}</span>
          ${r.message ? `<div class="pmsg">${esc(r.message)}</div>` : ""}
          <div class="pmodels">${r.models.slice(0, 12).map(m => `<span title="${esc(m.id)}">${esc(m.id)}${m.tiny ? " ·" : ""}</span>`).join("")}</div>
          <div class="pmsg mono">${esc(r.base_url)}${r.version ? " · v" + esc(r.version) : ""}</div></div></div>`).join("")}
      ${runtimes.length ? "" : `<p class="lead">Nothing found. Install <a href="#" data-url="https://ollama.com/download">Ollama</a>, or start LM Studio, <code>llama-server</code> or <code>mlx_lm.server</code>, then rescan.</p>`}
      <div class="line"><span class="lab">${d.endpoint ? `An OpenAI-compatible endpoint is up at <code class="mono">${esc(d.endpoint)}</code>` : "No local endpoint is up."}${d.recommend ? `<small>Suggested right now: <b>${esc(d.recommend.model)}</b> on ${esc(d.recommend.base_url)}</small>` : ""}</span><button class="btn" id="mmScan">Rescan</button></div>`;
    const pick = $("#mmPick", box);
    if (pick) pick.onclick = e => openPop(e.currentTarget, `<h5>Small jobs run on</h5>
      <button class="pi" data-m=""><span class="agent">${icon("sparkle")}</span>Leave it to the main model<span class="k">default</span></button>
      ${tiny.length ? tiny.map(m => `<button class="pi" data-m="${esc(m.runtime + ":" + m.id)}"><span class="agent">${icon("cpu")}</span>${esc(m.id)}<span class="k">${esc(m.runtime)}${m.up ? "" : " · stopped"}</span></button>`).join("") : `<div class="note">No tiny model is available. Pull one — for example <code>ollama pull lfm2.5:latest</code> — or start a runtime.</div>`}`, pop => {
      $$("[data-m]", pop).forEach(b => (b.onclick = async () => { closePop(); await invoke("set_subagent_model", { model: b.dataset.m || null }); prefs.subagent_model = b.dataset.m || null; draw(box, wtBox); toast(b.dataset.m ? `Small jobs will run on ${b.dataset.m}` : "Small jobs stay on the main model", "ok"); }));
    });
    $("#mmScan", box).onclick = () => draw(box, wtBox);
    $$("[data-url]", box).forEach(b => (b.onclick = e => { e.preventDefault(); invoke("open_url", { url: b.dataset.url }); }));
    if (wtBox) renderWorktrees(wtBox);
  }
  return { mount: (box, wtBox) => { if (box) draw(box, wtBox); }, size, get last() { return last; } };
})();

// ------------------------------------------------------------------ sidebar resize, live updates, boot
(() => {
  const g = $("#sideGut"), win = $("#win");
  const place = () => (g.style.left = getComputedStyle(win).getPropertyValue("--side-w"));
  g.addEventListener("pointerdown", e => {
    e.preventDefault(); g.setPointerCapture(e.pointerId);
    const left = win.getBoundingClientRect().left;
    const move = ev => { const w = Math.min(380, Math.max(176, ev.clientX - left)); win.style.setProperty("--side-w", w + "px"); place(); };
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
  applyCanvasOps(); watchAgents();
  if (window.Shell) Shell.refresh();
  if (window.RPanel) RPanel.refresh();
  if (!agentsOn() && !(window.Shell && Shell.showing("canvas"))) { lastShape = ""; return; }
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
  // "os-<platform>" comes from shell.js; a bare "win" class would match ".win" (the window) and hide Code's title bar on Windows.
  document.documentElement.classList.add("native", "os-" + boot.platform);
  prefs = await invoke("prefs");
  if ([1, 2, 3, 4].includes(boot.layout)) {
    // bench/: a fixed starting tab with that many canvases.
    prefs.tabs.unshift({ name: "Bench", panes: [{ kind: "agent", agent: 0, url: null }, { kind: "files", agent: 0, url: null }, { kind: "agent", agent: 1, url: null }, { kind: "agent", agent: 2, url: null }].slice(0, boot.layout), layout: null });
    prefs.active_tab = 0;
  }
  applyTheme(); renderSeg(); setSide(!narrow());
  S.mode = prefs.mode || "chat";
  S.codeView = prefs.code_view === "pair" ? "pair" : "agents";
  S.chatView = prefs.chat_view === "agents" ? "agents" : "chat";
  // The old Inbox lives on Home now.
  S.wb = WB.some(([k]) => k === prefs.wb_view) ? prefs.wb_view : "home";
  Art.apply(prefs.backdrop);
  // A folder given on the command line, or a bench run: start in Code.
  const first = await invoke("snapshot");
  if (first.workspace && (boot.bench || !prefs.onboarded)) { S.mode = "code"; S.codeView = "agents"; }
  if (window.Shell) await Shell.init(boot);
  if (window.Chat) await Chat.init();
  setMode(S.mode, false);
  await refresh(true);
  // Starting in Code: reopen the last project, as editors do.
  if (S.mode === "code" && !snap.workspace && machine().local && (prefs.projects || [])[0] && !boot.bench) openProject(prefs.projects[0]);
  if (!prefs.onboarded && !boot.bench) Onboard.open();
  if (prefs.check_updates && !boot.bench) checkUpdate(); else renderUpd();
  requestAnimationFrame(() => requestAnimationFrame(() => invoke("ready")));
});
