// Backspace, Tauri edition. The window is a frame that hosts apps:
//
// - frame.js reads extensions/index.json, gives each app a section and loads
//   its script. Chat (extensions/chat/) and Code (extensions/code/) live
//   there; the frame knows them only by manifest, and draws the switcher, the
//   app's name, On and New.
// - This file keeps the window's shared state (prefs, the machine snapshot,
//   agents, projects, settings, updates). Code's engine — tabs, canvases, the
//   grid, the drawer, the wall — is extensions/code/code.js; Chat's face is
//   extensions/chat/chat.js.
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
// The update being downloaded and swapped in: { stage, pct } while it runs.
let installing = null;
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
const machine = () => machines.find(m => m.selected) || { name: "Local", local: true, link: { state: "online" } };

// ------------------------------------------------------------------ agent helpers
// How an agent reads in the sidebar, Home and the workbench. These are about the
// runtime, not about an app's surface, so the frame keeps them.
const tClass = s => ({ proposed: "s-wait", in_review: "s-wait", ready_for_human: "s-wait", needs_info: "s-wait", queued: "s-run", in_progress: "s-run", done: "s-done", failed: "s-fail", wontfix: "s-fail" })[s] || "s-idle";
const tLabel = s => s.replace(/_/g, " ").replace(/^./, c => c.toUpperCase());
const busy = a => !!a && ["running", "queued", "awaiting_approval"].includes(a.status);
const lastLine = a => { const e = [...a.log].reverse().find(e => e.kind !== "user") || a.log[a.log.length - 1]; return e ? e.text.split("\n")[0] : "No activity yet"; };
// Why an agent needs the human, or null.
function needsYou(a) {
  const ap = pending().find(x => x.agent === a.id);
  if (ap) return { why: ap.kind === "plan" ? `Plan · ${ap.tickets.length} tickets to approve` : a.id === MAIN ? "Final deliverable to review" : "Work to review", ap };
  if (a.status === "failed") return { why: "Failed" };
  // A restart cut this one off: its worktree, branch and CLI session are all
  // still on disk, so it can carry on rather than be redone (audit P0-2).
  if (a.status === "interrupted") return { why: "Interrupted by a restart", resume: true };
  const t = a.ticket && snap.tickets.find(x => x.key === a.ticket);
  if (t && (t.state === "needs_info" || t.state === "ready_for_human")) return { why: tLabel(t.state) };
  const last = a.log[a.log.length - 1];
  if (a.id === MAIN && a.status === "idle" && last && last.kind === "assistant") return { why: "Asked you something", reply: true };
  return null;
}


// Backspace Cloud is a nightly build feature: it is a development server with no
// billing, and offering it in a release build reads as a product we do not have.
// `boot()` reports the channel; everything Cloud-facing asks this first.
const nightly = () => !!window.__BOOT && window.__BOOT.channel === "nightly";

// ------------------------------------------------------------------ theme + chrome
// Settings → Appearance owns these, on top of the light/dark theme.
// Contrast dims or sharpens the secondary text ramp, as a straight linear
// interpolation from the background toward each level's designed colour.
const RAMP = {
  light: { bg: [255, 255, 255], fg: [[20, 19, 23], [78, 73, 85], [125, 119, 133], [168, 162, 174]] },
  dark: { bg: [23, 22, 27], fg: [[255, 255, 255], [222, 217, 228], [176, 170, 186], [124, 118, 132]] },
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
  // Contrast is a position along the ramp, and the neutral is the TOP of that
  // range, not the middle of it. Reading an unset contrast as 1 landed at
  // t = 0.5, so every default install drew its text at half strength toward the
  // background — which is why the UI read as dim no matter what the theme said.
  const c = a.contrast > 0 ? a.contrast : 1.4;
  for (const [k, v] of contrastVars(RAMP[dark ? "dark" : "light"], c)) {
    if (!a.fg || k !== "--fg") set(k, v);
  }
  const trans = a.translucent === false && !!prefs.appearance;
  set("--glass", trans ? (dark ? "rgb(26, 21, 28)" : "rgb(252, 249, 252)") : null);
  set("--glass-side", trans ? (dark ? "rgb(34, 29, 36)" : "rgb(244, 241, 245)") : null);
  set("--glass-main", trans ? (dark ? "rgb(14, 12, 16)" : "rgb(246, 243, 247)") : null);
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
// Both settings buttons, and they toggle. `#gear` is the rail's; `#gearBtn` is
// the title-bar one, which nothing had ever wired. shell.js also binds `#gear`,
// so this is the single binding for both.
for (const sel of ["#gear", "#gearBtn"]) {
  const b = $(sel);
  if (b) b.onclick = () => (S.settings ? closeSettings() : openSettings());
}

// The rail's apps share the window; the sidebar, title bar and body follow.
// Chat and Workbench are extensions (extensions/index.json), so the list of
// modes and the window's m-<id> classes come from the frame, not from here.
// Code has two views: Pair (you and one CLI in the project) and Agents (the
// workbench).
const agentsOn = () => S.mode === "code" && S.codeView === "agents";
const PAGES = ["memory", "apps"];
function setMode(m, save = true) {
  const exts = window.Frame ? Frame.ids() : ["chat", "code"];
  const on = window.Frame ? Frame.onIds() : exts;
  m = m || "";
  if ((!exts.includes(m) && !PAGES.includes(m) && !m.startsWith("app:")) || (exts.includes(m) && !on.includes(m))) m = on[0] || PAGES[0];
  if (m.startsWith("app:") && !(window.Apps && Apps.get(m.slice(4)))) m = on[0] || PAGES[0];
  S.mode = m;
  const w = $("#win");
  for (const z of exts.concat(PAGES)) w.classList.toggle("m-" + z, m === z);
  w.classList.toggle("m-app", m.startsWith("app:"));
  if (window.Frame) { Frame.setActive(m); Frame.syncClasses(w); }
  w.classList.toggle("single-use", on.length === 1);
  if (S.settings) { S.settings = false; $("#settings").hidden = true; }
  if (S.drawer && !agentsOn() && window.Code) Code.setDrawer(false);
  if (save) { prefs.mode = m; invoke("set_mode", { mode: m }); }
  if (window.Shell) Shell.layout();
  renderList();
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
  const t = window.Code ? Code.tab() : null;
  const p = t && t.panes[S.focus];
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
  // The zone's own title and list go first, so a page left behind (Apps,
  // Memory) never keeps its heading over the IDE.
  if (window.Shell) Shell.renderSide();
  if (!agentsOn()) return;
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
  $$("#list [data-open]").forEach(b => (b.onclick = () => Code.openView({ kind: "agent", agent: +b.dataset.open })));
  $$("#list [data-files]").forEach(b => (b.onclick = () => Code.openView({ kind: "files", agent: +b.dataset.files })));
  $$("#list [data-pv]").forEach(b => (b.onclick = () => { const [t, p] = b.dataset.pv.split(":").map(Number); Code.switchTab(t); S.focus = p; Code.renderGrid(); }));
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
    // The new project's own canvas layout, not the one we were just looking at.
    await Code.loadTabs(path);
    S.review = null; S.ticket = null;
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
    const c = $("#pmClose", pop); if (c) c.onclick = async () => { closePop(); await invoke("close_project"); await Code.loadTabs(""); await refresh(true); };
    const o = $("#pmOpen", pop); if (o) o.onclick = () => { closePop(); openProject(path); };
    $("#pmForget", pop).onclick = async () => { closePop(); if (open) { await invoke("close_project"); await Code.loadTabs(""); } await invoke("forget_project", { path }); prefs = await invoke("prefs"); await refresh(true); };
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
  }).join("");
  $$("#machines [data-m]").forEach(b => (b.onclick = async () => {
    await invoke("select_machine", { index: +b.dataset.m });
    S.pane.clear(); S.review = null; S.ticket = null;
    await refresh(true);
  }));
}

// ------------------------------------------------------------------ update pill
function renderUpd() {
  const b = $("#upd");
  b.hidden = false;
  b.classList.toggle("new", !!(upd && upd.newer) || !!installing);
  if (installing) {
    const pct = installing.stage === "Downloading" ? ` ${installing.pct}%` : "";
    b.innerHTML = `${icon("down")}<span class="lbl">${esc(installing.stage)}${pct}</span>`;
    b.title = "Backspace is fetching the new build; it reopens when it's in place.";
  } else if (upd === "checking") b.innerHTML = `Checking…`;
  else if (upd && upd.error) { b.innerHTML = `${icon("reload")}Check for updates`; b.title = upd.error; }
  else if (upd && upd.newer) { b.innerHTML = `${icon("down")}<span class="lbl">Update to ${esc(upd.latest)}</span>`; b.title = `A newer Backspace is out: build ${upd.latest}. You have ${upd.build || upd.current}.`; }
  else if (upd) { b.innerHTML = `${icon("check")}<span class="lbl">Up to date</span><span class="n">v${esc(upd.current)}</span>`; b.title = `Backspace ${upd.current} · click to check again`; }
  else b.innerHTML = `${icon("reload")}<span class="lbl">Check for updates</span>`;
}
async function checkUpdate() {
  upd = "checking"; renderUpd();
  upd = await invoke("check_update").catch(e => ({ error: String(e) }));
  renderUpd(); if (S.settings) renderSettings();
}
$("#upd").onclick = () => (upd && upd.newer ? installUpdate() : checkUpdate());

// Download the build the check found and let it replace this one. The command
// only returns if something went wrong: on success the app quits into the swap
// script, which reopens it, so there is no "done" state to render.
async function installUpdate() {
  if (installing) return;
  installing = { stage: "Preparing", pct: 0 };
  renderUpd(); syncUpdButton();
  try {
    await invoke("install_update");
  } catch (e) {
    installing = null;
    renderUpd(); syncUpdButton();
    toast(`Couldn't install the update: ${e}`, "err");
  }
}
function syncUpdButton() {
  const g = $("#updGo");
  if (!g) return;
  g.disabled = !!installing;
  if (!installing) { g.innerHTML = "Install & Restart"; return; }
  g.innerHTML = installing.stage === "Downloading" ? `Downloading ${installing.pct}%` : `${esc(installing.stage)}…`;
}
TAURI.event.listen("update:progress", e => {
  installing = e.payload || installing;
  renderUpd(); syncUpdButton();
});

addEventListener("keydown", e => {
  if (e.key === "Escape") { if (window.Ask && $("#ask") && !$("#ask").hidden) Ask.close(); else if (!$("#pop").hidden) closePop(); else if (S.settings) closeSettings(); else if (S.drawer) Code.setDrawer(false); }
  if (!(e.metaKey || e.ctrlKey)) return;
  if (!$("#onboard").hidden) return;
  if (e.key === "n") { e.preventDefault(); setMode("chat"); Chat.newChat(); return; }
  if (e.key === "k") { e.preventDefault(); Shell.search(); return; }
  if (e.key === "i") { e.preventDefault(); Ask.toggle(); return; }
  if (e.key === "o") { e.preventDefault(); pickProject(); return; }
  if (e.key === "j") { e.preventDefault(); setMode(S.mode === "chat" ? "code" : "chat"); return; }
  if (e.key === "/" ) { e.preventDefault(); if (window.Shell) Shell.toggleSplit(); return; }
  if (!agentsOn()) { if (e.key === ",") { e.preventDefault(); openSettings(); } if (e.key === "\\") { e.preventDefault(); setSide($("#win").classList.contains("side-closed")); } return; }
  const i = +e.key;
  if (i >= 1 && i <= 9 && prefs.tabs[i - 1]) { e.preventDefault(); Code.switchTab(i - 1); }
  if (e.key === "t") { e.preventDefault(); Code.newTabPop($("#newTab")); }
  if (e.key === ",") { e.preventDefault(); openSettings(); }
  if (e.key === "\\") { e.preventDefault(); setSide($("#win").classList.contains("side-closed")); }
});

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

// Rich answers (OpenUI) is the model's output mode, so it belongs with the
// runtime's settings; the workbench's session card reads it too.
const richOn = () => { try { return localStorage.getItem("bs.rich") !== "0"; } catch { return true; } };

// ------------------------------------------------------------------ settings
function openSettings(section) {
  S.settings = true; $("#settings").hidden = false;
  // Settings is a page of its own, so the sidebar and the rail step aside and it
  // spans the window instead of sitting in a column beside them.
  $("#win").classList.add("settings-on");
  renderSettings(); if (window.Shell) Shell.layout();
  refreshCompanion();
  if (section) { const el = $(`#set-${section}`), box = $("#settings"); if (el) box.scrollTop = el.getBoundingClientRect().top - box.getBoundingClientRect().top + box.scrollTop - 12; const f = $(`#set-${section} input`); if (f) f.focus({ preventScroll: true }); }
  refreshShare();
}
function closeSettings() { if (!S.settings) return; S.settings = false; $("#settings").hidden = true; $("#win").classList.remove("settings-on"); if (window.Shell) Shell.layout(); }
let companion = null;
async function refreshCompanion() { companion = await invoke("companion_status").catch(() => null); if (S.settings) renderSettings(); }
async function refreshShare() { share = await invoke("share_status").catch(() => null); if (S.settings) renderSettings(); }
// Settings → Appearance. The panel writes prefs.appearance and hands it to
// the shell command; applyAppearance() turns it into custom properties.
const ACCENTS = ["#cf5a24", "#2a66d9", "#187f45", "#6a4fd6", "#bf3a2f", "#1c1a1f"];
function appearancePanel() {
  const a = prefs.appearance || {}; const c = a.contrast > 0 ? a.contrast : 1.4;
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
      <div class="line"><span class="lab">Apps<small>Off apps stay in the title bar, dimmed, until you turn one back on.</small></span><div class="segc">${(window.Frame ? Frame.zones() : [{ id: "chat", name: "Chat" }, { id: "code", name: "Workbench" }]).map(e => `<button aria-pressed="${(prefs.uses && prefs.uses.length ? prefs.uses : (window.Frame ? Frame.ids() : ["chat", "code"])).includes(e.id)}" data-use="${e.id}">${esc(e.name)}</button>`).join("")}</div></div>
      ${hasProject ? `<div class="line"><span class="lab">Workspace<small class="mono">${esc(snap.workspace || "—")}</small></span></div>
      <div class="line"><span class="lab">Config<small class="mono">${esc(snap.config_source || "built-in defaults")}</small></span></div>` : ""}</section>
    <section id="set-appearance"><h2>Appearance</h2><p class="lead">A theme, then the colours and fonts on top of it.</p>
      ${appearancePanel()}
      <div class="line stack"><span class="lab">Background<small>Behind the window, and across the top of Home.</small></span><div class="bdrops">${Art.BACKDROPS.map(([k, l]) => `<button class="bdrop" aria-pressed="${Art.current() === k}" data-bd="${k}" title="${l}"><span class="sw sw-${k}" ${Art.src(k) ? `style="background-image:url('${Art.src(k)}')"` : ""}></span><span>${l}</span></button>`).join("")}</div></div></section>
    <section id="set-notify"><h2>Notifications</h2><div class="line"><span class="lab">When something needs you<small>A plan to approve, work to review, an agent that failed, a chat that hit a limit.</small></span><div class="segc">${[["system", "System"], ["app", "In the app"], ["off", "Off"]].map(([k, l]) => `<button aria-pressed="${(prefs.notify || "system") === k}" data-nt="${k}">${l}</button>`).join("")}</div></div>
      <div class="line"><span class="lab"></span><button class="btn" id="ntTest">Send a test notification</button></div></section>
    <section id="set-chat"><h2>Chat behavior</h2><div class="line"><span class="lab">New chats go to<small>You can switch per chat from the composer.</small></span><button class="btn" id="setRoute" data-popper>${esc(window.Chat ? Chat.routeName(prefs.default_route) : "Pick")}</button></div>
      <div class="line"><span class="lab">Rich answers<small>Models may answer with charts, tables, steps and forms (OpenUI). Off: Markdown only, and a shorter prompt.</small></span><button class="toggle" role="switch" id="richT" aria-checked="${richOn()}" aria-label="Rich answers"></button></div></section>
    <section id="set-phone"><h2>Phone</h2><p class="lead">The Backspace phone app (coming soon) follows your chats, saves notes to Memory and approves a project's work. Turn this on and scan the code with it to pair. Plain HTTP with a token, so treat it as a network you trust — a paired phone can drive agents that run commands.</p>
      <div class="line"><span class="lab">Let my phone connect<small>${companion && companion.error ? esc(companion.error) : companion && companion.running ? `Listening on ${esc(companion.url || companion.addr)}` : "Off"}</small></span><button class="toggle" role="switch" id="phoneT" aria-checked="${!!(companion && companion.enabled)}" aria-label="Let my phone connect"></button></div>
      <div class="line"><span class="lab">Reachable from my network<small>${companion && companion.allow_lan ? "On: the port answers on your LAN, where a phone can reach it." : "Off (recommended): this machine only, so only the desktop can reach it."}</small></span><button class="toggle" role="switch" id="phoneLan" aria-checked="${!!(companion && companion.allow_lan)}" aria-label="Reachable from my network"></button></div>
      ${companion && companion.enabled && !companion.allow_lan ? `<p class="warnp">${icon("warn")}Turn on "Reachable from my network" before pairing a phone; a phone needs the LAN listener.</p>` : ""}
      ${companion && companion.enabled && companion.allow_lan && companion.pair ? `<div class="pair"><div class="qr" id="phoneQr" aria-label="Pairing code"></div><div class="pair-t"><b>Scan with the Backspace app</b><span>Or enter <code>${esc(companion.url)}</code> and the token by hand.</span><div class="pair-b"><button class="btn" id="phoneCopy">${icon("copy")}Copy pairing link</button><button class="btn" id="phoneNew">Unpair all phones</button></div></div></div>` : companion && companion.enabled && companion.allow_lan && !companion.url ? `<p class="warnp">${icon("warn")}This machine has no address on a local network, so a phone can't reach it.</p>` : ""}</section>
    <section id="set-providers"><h2>Agent providers</h2><p class="lead">What Chat and projects can use. Backspace scans this machine for each CLI, its version and whether it is signed in; switch off any you don't want offered.</p><div id="setProviders"></div>
      ${nightly() ? `<div class="line"><span class="lab">Backspace Cloud</span><span class="lead" style="margin:0">Chat without installing anything. Free with a short ad under each reply, or a paid plan.</span></div><div id="setPlan"></div>` : ""}</section>
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
      <div class="line"><span class="lab">Backspace ${esc(u && u.current || "0.1.0")}<small>${u && u.error ? esc(u.error) : u && u.newer ? `Build ${esc(u.latest)} is available${u.build ? `, you have ${esc(u.build)}` : ""}.` : u ? `You're on the latest build.${u.build ? ` Built from ${esc(u.build)}.` : ""}` : "Not checked yet."}</small></span>
        ${u && u.newer ? `<button class="btn primary" id="updGo">Install & Restart</button>` : `<button class="btn" id="updNow">Check now</button>`}</div></section>
    <section id="set-home"><h2>Archived threads</h2><p class="lead">Threads you archived. They keep their messages and can be restored from Chat.</p><div id="setArchived"></div></section>
  </div></div></div>`;
  Object.entries(keep).forEach(([id, v]) => { const i = $("#" + id, el); if (i && v) i.value = v; });
  // One panel at a time. Bound to the rendered nodes directly, and it refuses
  // to hide everything: an unknown key falls back to General, so the panel can
  // never end up with every section hidden.
  // Aliases: "uses" is a block inside General, and "plan" (Cloud) is a block
  // inside Agent providers. Both used to be their own sections, and callers all
  // over the UI still navigate to them by those names.
  const SET_ALIAS = { uses: "general", plan: "providers" };
  const showSection = k => {
    if (SET_ALIAS[k]) k = SET_ALIAS[k];
    const known = SET_NAV.flatMap(g => g[1]).map(([x]) => x);
    if (k !== "home" && !known.includes(k)) k = "general";
    S.setSection = k;
    let shown = 0;
    $$("[data-setnav]", el).forEach(b => b.classList.toggle("on", b.dataset.setnav === k));
    $$(".set section", el).forEach(s => {
      const mine = s.id === "set-" + k;
      s.hidden = !mine;
      if (mine) shown++;
    });
    if (!shown) {
      // The section this key names was not rendered (a build older than the nav,
      // say). Show General rather than a blank panel, and say so.
      const g = $(".set section", el);
      if (g) { g.hidden = false; }
      console.warn("settings: no section for", k);
    }
    const body = $(".setbody", el);
    if (body) body.scrollTop = 0;
    const nav2 = $(`[data-setnav="${k}"]`, el);
    if (nav2 && nav2.scrollIntoView) nav2.scrollIntoView({ block: "nearest" });
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
  const pl = $("#phoneLan", el);
  if (pl) pl.onclick = async () => {
    const on = !(companion && companion.allow_lan);
    companion = await invoke("set_companion_lan", { allowLan: on }).catch(e => (toast(String(e), "err"), companion));
    setTimeout(refreshCompanion, 400); renderSettings();
  };
  const pq = $("#phoneQr", el);
  if (pq && companion.pair) invoke("qr_svg", { text: companion.pair }).then(svg => { if (pq.isConnected) pq.innerHTML = svg; }).catch(() => {});
  const pc = $("#phoneCopy", el); if (pc) pc.onclick = () => { navigator.clipboard.writeText(companion.pair); toast("Pairing link copied", "ok"); };
  const pn = $("#phoneNew", el); if (pn) pn.onclick = async () => { companion = await invoke("set_companion", { enabled: true, newToken: true }); toast("Every paired phone was signed out", "ok"); renderSettings(); };
  $$("[data-use]", el).forEach(b => (b.onclick = async () => {
    const k = b.dataset.use, ids = window.Frame ? Frame.ids() : ["chat", "code"];
    const cur = prefs.uses && prefs.uses.length ? prefs.uses : ids;
    const next = ids.filter(u => (u === k ? !cur.includes(u) : cur.includes(u)));
    if (!next.length) { toast("Keep at least one on", "err"); return; }
    prefs.uses = next; await invoke("set_uses", { uses: next });
    setMode(S.mode, false); openSettings("general");
  }));
  $("#setRerun").onclick = () => { closeSettings(); Onboard.open(); };
  if (window.Providers) { Providers.mount($("#setProviders")); if (nightly()) Providers.mountPlan($("#setPlan")); }
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
  const go = $("#updGo"); if (go) { go.onclick = installUpdate; syncUpdButton(); }
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

// Settings → Agent skills: every playbook this machine can load, where it came
// from, which roles have it injected, and how to add or find one.
const SKILL_ORIGIN = { "built-in": "Built-in", "yours": "Yours", "workspace": "This project" };
async function renderSkills(el) {
  const box = $("#setSkills", el); if (!box) return;
  if (!box.dataset.loaded) box.innerHTML = `<div class="nothing">Reading skills…</div>`;
  let d;
  try { d = await invoke("skills_catalog"); } catch (e) { box.innerHTML = `<div class="err">${esc(String(e))}</div>`; return; }
  if (!box.isConnected) return;
  box.dataset.loaded = "1";
  box.dataset.catalog = JSON.stringify(d);
  const q = (box.dataset.q || "").trim().toLowerCase();
  const all = d.skills || [];
  const shown = q
    ? all.filter(s => s.name.toLowerCase().includes(q) || (s.description || "").toLowerCase().includes(q))
    : all;
  const role = (label, names) => `<div class="line"><span class="lab">${label}<small>${names.length ? names.map(esc).join(", ") : "none"}</small></span></div>`;
  box.innerHTML = role("Main agent", d.workflow.main_skills) + role("Workers", d.workflow.worker_skills) + role("Triage", d.workflow.triage_skills)
    + `<div class="line"><span class="lab">Search<small>Name, description or body. ${all.length} skill${all.length === 1 ? "" : "s"} on this machine.</small></span>
        <input class="tx" id="skQ" value="${esc(box.dataset.q || "")}" placeholder="e.g. review, triage, tdd" style="width:200px" aria-label="Search skills"></div>`
    + `<div class="skilllist">${shown.length ? shown.map(s => `<div class="skill"><b>${esc(s.name)}</b>
          <span>${esc(s.description || "No description.")}</span>
          <small>${esc(SKILL_ORIGIN[s.origin] || s.origin)}${s.injected && s.injected.length ? ` · injected into ${s.injected.map(esc).join(", ")}` : ""}${s.files.length > 1 ? ` · ${s.files.map(esc).join(", ")}` : ""}</small></div>`).join("")
      : `<div class="nothing">No skill matches “${esc(q)}”.</div>`}</div>`
    + `<div class="line"><span class="lab">Add one<small>Write a folder with a <code>SKILL.md</code> in it. <b>${esc(d.user_dir || d.dirs[0] || "")}</b> is yours; a folder of the same name in <code>&lt;project&gt;/.backspace/skills/</code> overrides a built-in for that project only.</small></span>
        <button class="btn" id="skOpen">Open my skills folder</button><button class="btn" id="skTpl">Copy the template</button></div>`
    + `<div class="line"><span class="lab">Read from<small class="mono">${(d.dirs || []).map(esc).join(" · ")}</small></span></div>`;
  const qi = $("#skQ", box);
  if (qi) {
    qi.oninput = () => { box.dataset.q = qi.value; clearTimeout(skQ); skQ = setTimeout(() => renderSkills(el), 200); };
    if (box.dataset.focusSearch) { qi.focus(); box.dataset.focusSearch = ""; }
  }
  const op = $("#skOpen", box);
  if (op) op.onclick = async () => {
    const dir = d.user_dir || (d.dirs || [])[0];
    if (!dir) { toast("No skills folder on this machine", "err"); return; }
    // Creating it first means the first click does not fail on a folder that
    // does not exist yet, which is exactly the state a new install is in.
    await invoke("ensure_dir", { path: dir }).catch(() => {});
    invoke("reveal_path", { path: dir }).catch(e => toast(String(e), "err"));
  };
  const tp = $("#skTpl", box);
  if (tp) tp.onclick = async () => {
    try { await navigator.clipboard.writeText(d.template || ""); toast("Template copied: paste it into a SKILL.md", "ok"); }
    catch { toast("Copy failed; the template is in docs/SKILLS.md", "err"); }
  };
}
let skQ = null;

// Settings → Archived threads. Chat lists pinned threads at the top; this is
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
    const tasks = (d.tasks && d.tasks.length ? d.tasks : ["scouts"]);
    const runtimes = d.runtimes || [];
    const tiny = runtimes.flatMap(r => r.models.filter(m => m.tiny).map(m => ({ ...m, runtime: r.id, up: r.running })));
    box.innerHTML = `<div class="line"><span class="lab">Small jobs go to<small>Reading, classifying, summarising and extracting, when a subscription model delegates them. One small local model, one task at a time.</small></span>
        <button class="btn" id="mmPick" data-popper>${chosen ? esc(chosen) : "Leave it to the main model"}${icon("chevd")}</button></div>
      <div class="line"><span class="lab">…for these jobs<small>${chosen ? "Scouts are read-only lookups, the safest to hand over. Triage classifies a filed ticket. Workers build a ticket, so pick this only if the small model is up for it." : "Choose a model above first."}</small></span>
        <div class="segc">${[["scouts", "Scouts"], ["triage", "Triage"], ["workers", "Workers"]].map(([k, l]) => `<button aria-pressed="${tasks.includes(k)}" data-lt="${k}" ${chosen ? "" : "disabled"}>${l}</button>`).join("")}</div></div>
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
    // Which jobs the chosen model runs. Always at least one: turning every
    // job off would leave the model chosen and unused, which reads as broken.
    $$("[data-lt]", box).forEach(b => (b.onclick = async () => {
      const k = b.dataset.lt;
      const next = tasks.includes(k) ? tasks.filter(x => x !== k) : [...tasks, k];
      if (!next.length) { toast("Keep at least one job on; or choose no model", "err"); return; }
      await invoke("set_local_tasks", { tasks: next }).catch(e => toast(String(e), "err"));
      prefs.local_tasks = next; draw(box, wtBox);
    }));
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
    const move = ev => { const w = Math.min(380, Math.max(176, ev.clientX - left)); win.style.setProperty("--side-w-user", w + "px"); place(); };
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
  renderBrand(); renderMachines(); renderList(); renderCard();
  if (window.Code) Code.afterData();
  watchAgents();  if (window.Shell) Shell.refresh();
  if (window.RPanel) RPanel.refresh();
  if (!agentsOn() && !(window.Shell && Shell.showing("canvas"))) { lastShape = ""; return; }
  // Rebuild canvases when the machine or its agents appear; otherwise update in place.
  const shape = machine().index + ":" + (snap.agents.length > 0) + ":" + machine().link.state;
  if (window.Code) { if (full || shape !== lastShape) Code.renderGrid(); else Code.refreshPanes(); }
  lastShape = shape;
  const n = pending().length;
  if (n > lastPending) S.review = pending()[n - 1].id;
  lastPending = n;
}
// Coalesce bursts of harness events into one refresh per frame.
TAURI.event.listen("state", () => { if (!queued) { queued = true; requestAnimationFrame(() => refresh(false)); } });
addEventListener("resize", () => { setSide(!narrow()); });

// After every script has run. The frame loads the apps' scripts during boot.
addEventListener("DOMContentLoaded", async () => {
  const boot = await invoke("boot");
  window.__BOOT = boot;
  // "os-<platform>" comes from shell.js; a bare "win" class would match ".win" (the window) and hide Code's title bar on Windows.
  document.documentElement.classList.add("native", "os-" + boot.platform);
  prefs = await invoke("prefs");
  // The frame reads extensions/index.json, makes the apps' sections and loads
  // their scripts. Nothing below can draw a chat or a canvas before this.
  await Frame.load();
  // Before anything draws a tab, adopt the layout of the project that is
  // already open on this machine (or the global one when none is).
  await Code.loadTabs((await invoke("snapshot").catch(() => null) || {}).workspace || "");
  if ([1, 2, 3, 4].includes(boot.layout)) {
    // bench/: a fixed starting tab with that many canvases.
    prefs.tabs.unshift({ name: "Bench", panes: [{ kind: "agent", agent: 0, url: null }, { kind: "files", agent: 0, url: null }, { kind: "agent", agent: 1, url: null }, { kind: "agent", agent: 2, url: null }].slice(0, boot.layout), layout: null });
    prefs.active_tab = 0;
  }
  applyTheme(); renderSeg(); setSide(false);
  // Every launch lands on Apps; the zone you were last in is a click away.
  S.mode = "apps";
  S.codeView = prefs.code_view === "pair" ? "pair" : "agents";
  S.chatView = prefs.chat_view === "agents" ? "agents" : "chat";
  // The IDE opens on its canvases — one empty canvas, which asks what to put
  // here. Start is the ADE's landing (it draws the same home behind its own
  // composer), and it stays in the menu for anyone who wants it over here too.
  S.wb = Code.wbView(prefs.wb_view);
  Art.apply(prefs.backdrop);
  // A folder given on the command line, or a bench run: start in Code.
  const first = await invoke("snapshot");
  if (first.workspace && (boot.bench || !prefs.onboarded)) { S.mode = "code"; S.codeView = "agents"; }
  if (window.Shell) await Shell.init(boot);
  await Frame.init();
  setMode(S.mode, false);
  await refresh(true);
  // Starting in Code: reopen the last project, as editors do.
  if (S.mode === "code" && !snap.workspace && machine().local && (prefs.projects || [])[0] && !boot.bench) openProject(prefs.projects[0]);
  if (!prefs.onboarded && !boot.bench) Onboard.open();
  if (prefs.check_updates && !boot.bench) checkUpdate(); else renderUpd();
  requestAnimationFrame(() => requestAnimationFrame(() => invoke("ready")));
});
