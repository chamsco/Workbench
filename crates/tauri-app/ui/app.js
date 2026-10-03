// Backspace workbench, Tauri edition, driven by the real harness.
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
};
const icon = (n, cls = "") => `<svg class="ic ${cls}" viewBox="0 0 16 16" aria-hidden="true">${P[n]}</svg>`;
const LAY = {
  1: '<rect x="1" y="1" width="14" height="10" rx="1.5"/>',
  2: '<rect x="1" y="1" width="6" height="10" rx="1.5"/><rect x="9" y="1" width="6" height="10" rx="1.5"/>',
  3: '<rect x="1" y="1" width="4" height="10" rx="1.2"/><rect x="6" y="1" width="4" height="10" rx="1.2"/><rect x="11" y="1" width="4" height="10" rx="1.2"/>',
  4: '<rect x="1" y="1" width="6" height="4.2" rx="1"/><rect x="9" y="1" width="6" height="4.2" rx="1"/><rect x="1" y="6.8" width="6" height="4.2" rx="1"/><rect x="9" y="6.8" width="6" height="4.2" rx="1"/>',
};
const layIcon = n => `<svg class="lay-ic" viewBox="0 0 16 12" aria-hidden="true">${LAY[n]}</svg>`;
$$("[data-icon]").forEach(el => (el.innerHTML = icon(el.dataset.icon)));

// ------------------------------------------------------------------ state
let snap = { name: "", agents: [], approvals: [], tickets: [], total_cost_usd: 0, router_cost_usd: 0, workspace: "" };
let prefs = { theme: "system", tabs: [], active_tab: 0, check_updates: true };
let machines = [], share = null, upd = null;
const S = {
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
  let h = offlineNote();
  if (S.side === "projects") {
    if (snap.agents.length) {
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
  $$("#list [data-open]").forEach(b => (b.onclick = () => openView({ kind: "agent", agent: +b.dataset.open })));
  $$("#list [data-files]").forEach(b => (b.onclick = () => openView({ kind: "files", agent: +b.dataset.files })));
  $$("#list [data-pv]").forEach(b => (b.onclick = () => { const [t, p] = b.dataset.pv.split(":").map(Number); switchTab(t); S.focus = p; renderGrid(); }));
}
function renderCard() {
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
    const label = t.name ? esc(t.name) : layIcon(t.panes.length);
    return `<button class="tab" role="tab" aria-selected="${i === prefs.active_tab}" data-tab="${i}" aria-label="${esc(t.name || t.panes.length + " canvases")}, right-click to rename">${label}${badge}</button>`;
  }).join("") + `<button class="tab plus" id="newTab" data-popper aria-label="New tab" title="New tab (⌘T)">${icon("plus")}</button>`;
  $$("#tabs [data-tab]").forEach(b => {
    const i = +b.dataset.tab;
    b.onclick = () => switchTab(i);
    b.ondblclick = () => renameTab(i);
    b.oncontextmenu = e => { e.preventDefault(); tabMenu(b, i); };
  });
  $("#newTab").onclick = e => newTabPop(e.currentTarget);
  renderCta();
}
function switchTab(i) {
  if (i === prefs.active_tab && !S.settings) return;
  prefs.active_tab = i; S.focus = 0; S.max = null; closeSettings();
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
  let n = 2;
  const lays = () => [1, 2, 3, 4].map(k => `<button aria-pressed="${k === n}" data-n="${k}" aria-label="${k} canvas${k > 1 ? "es" : ""}"><svg viewBox="0 0 16 12">${LAY[k]}</svg>${k === 4 ? "2×2" : k}</button>`).join("");
  openPop(anchor, `<h5>New tab</h5><label class="field"><input id="ntName" placeholder="Name (optional)" maxlength="40"></label>
    <h5>Canvases</h5><div class="lays" id="ntLays">${lays()}</div>
    <div class="row-end"><button class="btn primary" id="ntGo">Create</button></div>`, pop => {
    const wireLays = () => $$("[data-n]", pop).forEach(b => (b.onclick = () => { n = +b.dataset.n; $("#ntLays", pop).innerHTML = lays(); wireLays(); }));
    wireLays();
    const go = () => {
      const name = $("#ntName", pop).value.trim() || null;
      prefs.tabs.push({ name, panes: Array.from({ length: n }, () => ({ kind: "empty", agent: 0, url: null })), cols: [], rows: 0.56 });
      closePop(); switchTab(prefs.tabs.length - 1);
    };
    $("#ntGo", pop).onclick = go;
    $("#ntName", pop).onkeydown = e => { if (e.key === "Enter") go(); if (e.key === "Escape") closePop(); };
  });
}
// Right-hand "+": add a canvas to this tab.
const VIEWS = [["files", "folder", "Worktree files"], ["browser", "globe", "Browser"], ["diagram", "diagram", "Diagram review"], ["docs", "doc", "PLAN.md"], ["empty", "window", "Empty canvas"]];
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
  t.panes.push({ agent: 0, url: null, ...spec }); t.cols = []; S.focus = t.panes.length - 1; S.max = null;
  saveTabs(); closeSettings(); renderTabs(); renderGrid();
}

// ------------------------------------------------------------------ review pill + tickets drawer
function renderCta() {
  const n = pending().length, t = snap.tickets.length;
  $("#cta").innerHTML = n ? `<span class="dot"></span><span class="lbl">${n} to review</span>${t ? `<span class="n">· ${t} tickets</span>` : ""}`
    : `${icon("check")}<span class="lbl">Nothing to review</span>${t ? `<span class="n">· ${t} tickets</span>` : ""}`;
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
  if (!prefs.check_updates && !upd) { b.hidden = true; return; }
  b.hidden = false;
  b.classList.toggle("new", !!(upd && upd.newer));
  if (upd === "checking") b.innerHTML = `Checking…`;
  else if (upd && upd.error) { b.innerHTML = `${icon("reload")}Check for updates`; b.title = upd.error; }
  else if (upd && upd.newer) { b.innerHTML = `${icon("down")}Download & Update`; b.title = `Backspace ${upd.latest} is out (you have ${upd.current})`; }
  else if (upd) { b.innerHTML = `${icon("check")}Up to date`; b.title = `Backspace ${upd.current}`; }
  else b.innerHTML = `${icon("reload")}Check for updates`;
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
  return `<svg viewBox="0 0 48 28" fill="var(--accent)" aria-hidden="true">${r}</svg>`;
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
    default: return "New canvas";
  }
}
const paneIcon = p => ({ agent: '<span class="agent">' + icon("sparkle") + "</span>", files: '<span class="term">' + icon("folder") + "</span>", browser: '<span class="term">' + icon("globe") + "</span>", diagram: '<span class="term">' + icon("diagram") + "</span>", docs: '<span class="term">' + icon("doc") + "</span>" })[p.kind] || '<span class="term">' + icon("window") + "</span>";

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
  el.addEventListener("pointerdown", () => { if (S.focus !== slot) { S.focus = slot; $$(".pane").forEach(x => x.classList.toggle("on", +x.dataset.slot === slot)); renderList(); } });
  const act = (k, f) => { const b = el.querySelector(`[data-act=${k}]`); if (b) b.onclick = f; };
  act("max", () => { S.max = S.max == null ? slot : null; renderGrid(); });
  act("swap", () => { setPane(slot, { kind: "empty", agent: 0, url: null }); });
  act("files", () => setPane(slot, { kind: "files", agent: p.agent, url: null }));
  act("close", () => {
    const t = tab();
    if (S.max != null) S.max = null;
    else if (t.panes.length > 1) { t.panes.splice(slot, 1); t.cols = []; S.pane.clear(); }
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
  else chooser(slot, body);
}
function chooser(slot, body) {
  const agents = treeOrder().map(id => agent(id));
  body.innerHTML = `<div class="chooser"><h5>Show in this canvas</h5><div class="cards">
    <button class="card" data-k="agent">${icon("sparkle")}<span class="ct">Main agent</span><span class="cs">Talk to the agent that runs the project</span></button>
    <button class="card" data-k="browser">${icon("globe")}<span class="ct">Browser</span><span class="cs">A dev server preview</span></button>
    <button class="card" data-k="diagram">${icon("diagram")}<span class="ct">Diagram review</span><span class="cs">Plans and deliverables to approve</span></button>
    <button class="card" data-k="files">${icon("folder")}<span class="ct">Worktree</span><span class="cs">Browse the project's files</span></button>
    <button class="card" data-k="docs">${icon("doc")}<span class="ct">PLAN.md</span><span class="cs">The plan the main agent wrote</span></button></div>
    ${agents.length > 1 ? `<h5>Agent sessions</h5><div class="alist">${agents.filter(a => a.id !== MAIN).map(a => `<button class="chip-btn" data-a="${a.id}">${icon("sparkle")}${esc(a.title)}<span class="dot ${dotFor(a.status)}"></span></button>`).join("")}</div>` : ""}</div>`;
  $$("[data-k]", body).forEach(b => (b.onclick = () => setPane(slot, { kind: b.dataset.k, agent: 0, url: null })));
  $$("[data-a]", body).forEach(b => (b.onclick = () => setPane(slot, { kind: "agent", agent: +b.dataset.a, url: null })));
}

function renderGrid() {
  const g = $("#grid"), t = tab();
  if (S.focus >= t.panes.length) S.focus = 0;
  const n = S.max != null ? 1 : t.panes.length;
  const shown = S.max != null ? [[t.panes[S.max], S.max]] : t.panes.map((p, i) => [p, i]);
  const cols = t.cols && t.cols.length ? t.cols : n === 3 ? [1 / 3, 2 / 3] : [0.5];
  g.className = "grid L" + n;
  g.style.setProperty("--cx", cols[0]); g.style.setProperty("--ry", t.rows || 0.56);
  g.style.setProperty("--c1", cols[0]); g.style.setProperty("--c2", cols[1] ?? 2 / 3);
  g.innerHTML = "";
  const areas = { 1: ["1 / 1"], 2: ["1 / 1", "1 / 3"], 3: ["1 / 1", "1 / 3", "1 / 5"], 4: ["1 / 1", "1 / 3", "3 / 1", "3 / 3"] }[n];
  shown.forEach(([p, slot], i) => { const el = buildPane(p, slot); el.style.gridArea = areas[i]; g.appendChild(el); });
  if (n >= 2) g.appendChild(gutter("v"));
  if (n === 3) g.appendChild(gutter("v2"));
  if (n === 4) g.appendChild(gutter("h"));
}
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
    if (p.kind === "empty") chooser(slot, body);
  });
}
function gutter(dir) {
  const d = document.createElement("div");
  d.className = "gut " + (dir === "v2" ? "v v2" : dir); d.setAttribute("role", "separator");
  const t = tab();
  d.addEventListener("pointerdown", e => {
    e.preventDefault(); d.setPointerCapture(e.pointerId); d.classList.add("drag");
    const r = $("#grid").getBoundingClientRect(), n = t.panes.length;
    if (!t.cols || !t.cols.length) t.cols = n === 3 ? [1 / 3, 2 / 3] : [0.5];
    const move = ev => {
      const fx = (ev.clientX - r.left) / r.width, fy = (ev.clientY - r.top) / r.height;
      if (dir === "v") t.cols[0] = Math.min(n === 3 ? t.cols[1] - 0.12 : 0.8, Math.max(0.15, fx));
      else if (dir === "v2") t.cols[1] = Math.min(0.85, Math.max(t.cols[0] + 0.12, fx));
      else t.rows = Math.min(0.8, Math.max(0.2, fy));
      const g = $("#grid");
      g.style.setProperty("--cx", t.cols[0]); g.style.setProperty("--c1", t.cols[0]); g.style.setProperty("--c2", t.cols[1] ?? 2 / 3); g.style.setProperty("--ry", t.rows);
    };
    const up = () => { d.classList.remove("drag"); d.removeEventListener("pointermove", move); d.removeEventListener("pointerup", up); saveTabs(); };
    d.addEventListener("pointermove", move); d.addEventListener("pointerup", up);
  });
  d.addEventListener("dblclick", () => { t.cols = []; t.rows = 0.56; saveTabs(); renderGrid(); });
  return d;
}
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
  S.settings = true; $("#settings").hidden = false; $("#canvas").hidden = true; renderSettings();
  if (section) { const el = $(`#set-${section}`); if (el) el.scrollIntoView(); const f = $(`#set-${section} input`); if (f) f.focus(); }
  refreshShare();
}
function closeSettings() { if (!S.settings) return; S.settings = false; $("#settings").hidden = true; $("#canvas").hidden = false; }
async function refreshShare() { share = await invoke("share_status").catch(() => null); if (S.settings) renderSettings(); }
function renderSettings() {
  const el = $("#settings");
  const keep = {}; $$("input.tx", el).forEach(i => (keep[i.id] = i.value));
  const th = prefs.theme || "system";
  const sh = share || { enabled: false, addr: "127.0.0.1:7420", token: "", error: null, sharing: false };
  const u = upd && upd !== "checking" ? upd : null;
  el.innerHTML = `<div class="set">
    <div class="set-h"><h1>Settings</h1><span class="sp"></span><button class="ib" id="setClose" aria-label="Close settings">${icon("close")}</button></div>
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
    <section><h2>Project</h2>
      <div class="line"><span class="lab">Workspace<small class="mono">${esc(snap.workspace || "—")}</small></span></div>
      <div class="line"><span class="lab">Config<small class="mono">${esc(snap.config_source || "built-in defaults")}</small></span></div></section></div>`;
  Object.entries(keep).forEach(([id, v]) => { const i = $("#" + id, el); if (i && v) i.value = v; });
  $("#setClose").onclick = closeSettings;
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
  renderBrand(); renderMachines(); renderList(); renderCard(); renderTabs(); renderDrawer();
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

(async () => {
  const boot = await invoke("boot");
  document.documentElement.classList.add("native", boot.platform);
  prefs = await invoke("prefs");
  if ([1, 2, 3, 4].includes(boot.layout)) {
    // bench/: a fixed starting tab with that many canvases.
    prefs.tabs.unshift({ name: "Bench", panes: [{ kind: "agent", agent: 0, url: null }, { kind: "files", agent: 0, url: null }, { kind: "agent", agent: 1, url: null }, { kind: "agent", agent: 2, url: null }].slice(0, boot.layout), cols: [], rows: 0.56 });
    prefs.active_tab = 0;
  }
  applyTheme(); renderSeg(); setSide(!narrow());
  await refresh(true);
  if (prefs.check_updates && !boot.bench) checkUpdate(); else renderUpd();
  requestAnimationFrame(() => requestAnimationFrame(() => invoke("ready")));
})();
