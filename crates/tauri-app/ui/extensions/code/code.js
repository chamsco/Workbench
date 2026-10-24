// Code: the project workbench — tabs of canvases, the agent grid, the drawer.
//
// The app the frame hosts, and the engine behind it. Everything the workbench
// draws lives here: the tab strip in the title bar, the per-project layout, the
// canvases and their panes (session, files, terminal, browser, diagram, docs,
// options, trace), the wall, the tickets drawer and the view menu. app.js keeps
// the runtime (agents, projects, machines, settings); shell.js keeps the chrome.
//
// It is a native extension: a classic script that shares the page's globals
// with the frame, registers itself, and receives the section it builds in.
var Code = (() => {
// ------------------------------------------------------------------ per-project canvases
// The canvas layout belongs to the project you are in: tabs (and which one was
// showing) are remembered per folder, so opening another project cannot
// overwrite the one you were working in. `prefs.tabs`/`prefs.active_tab` stay
// the *shape* every existing call site uses — they are redefined below as
// accessors over whatever the current project's layout is, so nothing else in
// this file had to change. The store side is `Prefs::tabs_for`.
let tabsNow = null;      // { tabs, active } for the project that is open
let appliedTo = null;    // which `prefs` object the accessors were installed on
function activateTabs() {
  if (appliedTo === prefs) return;
  appliedTo = prefs;
  Object.defineProperty(prefs, "tabs", {
    configurable: true, enumerable: true,
    get: () => (tabsNow ? tabsNow.tabs : (prefs.__tabs || [])),
    set: v => { if (tabsNow) tabsNow.tabs = v; else prefs.__tabs = v; },
  });
  Object.defineProperty(prefs, "active_tab", {
    configurable: true, enumerable: true,
    get: () => (tabsNow ? tabsNow.active : (prefs.__active || 0)),
    set: v => { if (tabsNow) tabsNow.active = v; else prefs.__active = v; },
  });
}
/// Adopt the layout that belongs to `workspace` (its own, or the global one).
async function useTabsFor(workspace) {
  activateTabs();
  let t = null;
  try { t = await invoke("tabs_for", { workspace: workspace || "" }); } catch { t = null; }
  const tabs = (t && Array.isArray(t.tabs) && t.tabs.length) ? t.tabs : (prefs.__tabs || []);
  tabsNow = { tabs, active: Math.min((t && t.active) || 0, Math.max(0, tabs.length - 1)) };
  S.pane.clear();
  return tabsNow;
}

const tab = () => prefs.tabs[prefs.active_tab];
const pstate = i => { const k = prefs.active_tab + ":" + i; if (!S.pane.has(k)) S.pane.set(k, {}); return S.pane.get(k); };

let saveT = null;
function saveTabs() {
  clearTimeout(saveT);
  // The layout is filed under the project it belongs to, so a save from one
  // project can never land in another's. Empty workspace: the global layout.
  saveT = setTimeout(() => invoke("set_tabs_for", {
    workspace: (snap && snap.workspace) || "",
    tabs: prefs.tabs,
    active: prefs.active_tab,
  }), 250);
  // The sidebar's tree lists these canvases. It lives in the chat face's store,
  // which only re-reads when something tells it to.
  if (window.WhirlChat && window.WhirlChat.shell) window.WhirlChat.shell();
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
// Team chat is not offered while the four-phase flow is being built: the
// canvas type and its renderer stay, so a tab that already holds one keeps
// working and the view comes back by adding it here again.
const VIEWS = [["conversation", "reply", "Conversation"], ["terminal", "term", "Terminal"], ["files", "folder", "Worktree files"], ["browser", "globe", "Browser"], ["diagram", "diagram", "Diagram review"], ["docs", "doc", "PLAN.md"], ["options", "opts", "Visual options"], ["empty", "window", "Empty canvas"]];
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

// The session card: what this canvas is actually talking to.
//
// The IDE opens on a session, and the three things you would otherwise go to
// Settings to check are which CLI, on which model and effort, and on whose
// account — plus the folder it is working in. An IDE shows that at the top of
// the session rather than in a preference pane.
function sessionCard(a, path) {
  const cli = (window.Picker && Picker.cliOf(prefs.worker || "")) || "auto";
  const h = (Providers.list || []).find(x => x.id === cli);
  const auto = cli === "auto" || !h;
  const name = auto ? "Backspace" : h.name || cli;
  const version = !auto && h.version ? ` v${esc(h.version)}` : "";
  // A plan we can name is the Cloud one; a CLI's own subscription is not
  // something this app can read, so it says signed in or not and stops there.
  const account = Providers.account && Providers.account.plan
    ? Providers.account.plan[0].toUpperCase() + Providers.account.plan.slice(1)
    : !auto && h.installed
      ? (h.auth === "authenticated" ? "signed in" : esc(h.auth || "not signed in"))
      : "";
  const routed = a.decision
    ? `${esc(a.decision.model)} with ${esc(Effort.name(cli, a.decision.effort))} effort`
    : "not routed yet";
  // How it got there is a hover, not a line: it matters when a route looks
  // wrong, and it is noise the rest of the time.
  const why = a.decision
    ? `routed by ${a.decision.source}${a.decision.confidence > 0 ? ` (${Math.round(a.decision.confidence * 100)}% confident)` : ""}`
    : "";
  return `<div class="banner">${auto ? mascot() : Providers.avatar(cli, "sm")}<div class="bt">
    <div class="bt-h">${esc(name)}${version}</div>
    <div title="${esc(why)}">${routed}${account ? ` <span class="bt-plan">· ${esc(account)}</span>` : ""}</div>
    <div class="bt-p">${esc(path)}</div>
  </div></div>`;
}

// The permission mode the CLI runs under, in the words the picker uses, and the
// cycle shift+tab walks — the IDE's status line, and the one keystroke an IDE
// user already expects to change it.
const PERM_NAME = { supervised: "supervised", edits: "auto-accept edits", auto: "auto", full: "full access" };
const permName = () => PERM_NAME[prefs.cli_permission || "edits"] || "auto-accept edits";
function cyclePerm() {
  const order = ["edits", "auto", "full"];
  const cur = prefs.cli_permission || "edits";
  const next = order[(order.indexOf(cur) + 1) % order.length];
  prefs.cli_permission = next;
  invoke("set_agent_run", { planner: prefs.planner || null, worker: prefs.worker || null, permission: next, effort: prefs.cli_effort || null }).catch(() => {});
  toast(`CLI permissions: ${PERM_NAME[next]}`, "ok");
  renderGrid();
}
// Shift+Tab cycles it in the IDE, where there is no composer to mean plan mode.
addEventListener("keydown", e => {
  if (e.key !== "Tab" || !e.shiftKey) return;
  if (S.mode !== "code" || S.codeView !== "agents" || S.settings) return;
  if (e.target.closest("textarea")) return;
  e.preventDefault();
  cyclePerm();
});
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
  let h = sessionCard(a, path);
  h += a.log.map(logHTML).join("");
  h += artifactHTML(a);
  if (a.status === "running") h += `<div class="ln spin"><span class="spin-g">${SPIN[spinI]}</span> Working… (${esc(a.decision ? a.decision.model : "routing")} · ↓ ${a.input_tokens.toLocaleString()} tokens)</div>`;
  if (a.status === "awaiting_approval") h += `<div class="ln spin"><span class="spin-g">${SPIN[spinI]}</span> Waiting for your review…</div>`;
  if (a.id === MAIN && !a.log.some(e => e.kind === "user")) h += `<div class="ln note">Type a goal below. The main agent interviews you, then proposes tickets for you to approve.</div>`;
  return h;
}

// ------------------------------------------------------------------ artifacts
// What an agent produced, as things rather than prose: a diff, an image, a
// document, a table, a trace, or a set of options to decide. One renderer per
// kind; a kind this build does not know is listed by name rather than dropped.
// The record is `core::artifact`, on the project state so it arrives in the
// same snapshot as the agents.
const ART_KINDS = { diff: ["diff", "Diff"], image: ["doc", "Image"], options: ["opts", "Options"], trace: ["trace", "Trace"], document: ["doc", "Document"], table: ["grid", "Table"] };
function artifactHTML(a) {
  const mine = (snap.artifacts || []).filter(x => x.by === a.key || x.by === String(a.id));
  if (!mine.length) return "";
  return `<div class="arts"><div class="arts-h">${icon("archive")}Produced (${mine.length})</div>`
    + mine.slice(-12).map(x => artifactCard(x)).join("") + `</div>`;
}
function artifactCard(x) {
  const [ic, label] = ART_KINDS[x.kind] || ["doc", x.kind];
  const files = (x.stat || "").split("\n").filter(l => l.trim()).length;
  const meta = x.kind === "diff" && files ? `${files} file${files === 1 ? "" : "s"}`
    : x.summary || x.kind;
  return `<div class="art" data-art="${esc(x.id)}" data-kind="${esc(x.kind)}">
    <div class="art-h"><span class="ic">${icon(ic)}</span><b>${esc(x.title || label)}</b>
      <span class="art-k">${esc(label)}</span><span class="sp"></span>
      <span class="art-m">${esc(meta)}</span>
      <button class="ib art-more" title="Show or hide" aria-label="Show or hide ${esc(x.title || label)}">${icon("expand")}</button></div>
    <div class="art-body" hidden></div></div>`;
}
/// The body of one artifact, filled in the first time it is opened.
function artifactOpen(el, x) {
  const body = $(".art-body", el);
  if (!body) return;
  if (!body.hidden) { body.hidden = true; return; }
  if (!body.dataset.filled) {
    body.dataset.filled = "1";
    body.innerHTML = artifactBodyHTML(x);
  }
  body.hidden = false;
}
function artifactBodyHTML(x) {
  const pre = t => `<pre class="mono art-pre">${esc(t)}</pre>`;
  switch (x.kind) {
    case "diff": return (x.path ? `<div class="ln note mono">${esc(x.path)}</div>` : "") + (x.stat ? pre(x.stat) : "") + (x.body ? diffHTML(x.body) : "") + (!x.body && x.path ? `<div class="art-act"><button class="btn" data-artcat="${esc(x.path)}">Read the file</button></div>` : "");
    case "table": return tableHTML(x);
    case "options": return `<div class="art-act"><button class="btn primary" data-artopts="${esc(x.options)}">Open the options</button></div>`;
    case "trace": return `<div class="art-act"><button class="btn" data-artcat="${esc(x.trace)}">Read the trace</button></div>`;
    case "image": return x.data
      ? `<img class="art-img" src="${esc(x.data)}" alt="${esc(x.title)}">`
      : `<div class="art-act"><button class="btn" data-artimg="${esc(x.path)}">Show the image</button><button class="btn" data-artreveal="${esc(x.path)}">Reveal in folder</button></div>`;
    case "document": return x.body ? pre(x.body) : `<div class="art-act"><button class="btn" data-artcat="${esc(x.path)}">Read the document</button></div>`;
    default: return x.body ? pre(x.body) : `<div class="ln note">Nothing to show for <code>${esc(x.kind)}</code>.</div>`;
  }
}
/// A unified diff, coloured by line. Not a diff engine: the agent already knows
/// what changed, so this only has to read.
function diffHTML(text) {
  return `<pre class="mono art-pre diff">` + text.split("\n").map(l => {
    const c = l.startsWith("+++") || l.startsWith("---") ? "dh" : l.startsWith("+") ? "da" : l.startsWith("-") ? "dd" : l.startsWith("@@") ? "dm" : "";
    return `<span class="${c}">${esc(l)}</span>`;
  }).join("\n") + `</pre>`;
}
function tableHTML(x) {
  const cols = x.columns || [];
  return `<div class="art-tablewrap"><table class="art-table"><thead><tr>${cols.map(c => `<th>${esc(c)}</th>`).join("")}</tr></thead>`
    + `<tbody>${(x.rows || []).map(r => `<tr>${r.map(c => `<td>${esc(c)}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`;
}
/// Read a file an artifact points at, into the artifact's own body. Reading is
/// the `read_file` command, so nothing here touches disk itself.
async function artifactRead(into, path, asImage) {
  if (!into || !path) return;
  if (!into.hidden) { into.hidden = true; return; }
  into.innerHTML = `<div class="nothing">Reading ${esc(base(path))}…</div>`;
  into.hidden = false;
  try {
    const text = await invoke("read_file", { path });
    into.innerHTML = asImage
      ? `<img class="art-img" src="${esc(text)}" alt="${esc(base(path))}">`
      : `<pre class="mono art-pre">${esc(text)}</pre>`;
  } catch (e) { into.innerHTML = `<div class="err">${esc(String(e))}</div>`; }
}
/// One artifact kind opens a canvas of its own: the options set it refers to.
function artifactOptions(id) {
  if (!id) return;
  S.mode = "code";
  setCodeView("agents");
  S.options = id;
  if (window.Chat && Chat.render) Chat.render();
}
/// Click handling for every artifact card in the window.
function bindArtifacts() {
  $$(".art").forEach(card => {
    const x = (snap.artifacts || []).find(a => a.id === card.dataset.art);
    if (!x) return;
    const body = $(".art-body", card);
    $(".art-more", card).onclick = e => { e.stopPropagation(); artifactOpen(card, x); };
    card.onclick = e => {
      const t = e.target.closest("[data-artcat],[data-artreveal],[data-artimg],[data-artopts],[data-arttrace]") || e.target;
      if (t.dataset && t.dataset.artcat) return artifactRead(body, t.dataset.artcat);
      if (t.dataset && t.dataset.artreveal) return invoke("reveal_path", { path: t.dataset.artreveal }).catch(e2 => toast(String(e2), "err"));
      if (t.dataset && t.dataset.artimg) return artifactRead(body, t.dataset.artimg, true);
      if (t.dataset && t.dataset.artopts) return artifactOptions(t.dataset.artopts);
      if (t.dataset && t.dataset.arttrace) return artifactRead(body, t.dataset.arttrace);
      if (!t.closest("button")) artifactOpen(card, x);
    };
  });
}
function statusHTML(a) {
  const total = Math.max(snap.total_cost_usd, 0.000001), share = Math.min(1, subtree(a.id) / total);
  const filled = Math.round(share * 10);
  const t = a.ticket && snap.tickets.find(x => x.key === a.ticket);
  const hint = a.id === MAIN ? 'plan approval on <span class="b1">(review in Diagram)</span> · ⌘1–9 switch tabs'
    : t ? `check: ${esc(t.check || "none")} · ${esc(t.state.replace(/_/g, "-"))}` : esc(a.status);
  return `<div class="st">[${esc(a.decision ? `${a.decision.model} @ ${a.decision.effort}` : "not routed")}] <span style="color:var(--blue)">▣ ${esc(snap.name)}</span> | <span style="color:var(--green)">⎇ ${esc(a.branch || "-")}</span></div>
    <div class="st"><span class="bar">${"█".repeat(filled)}${"░".repeat(10 - filled)}</span> ${Math.round(share * 100)}% of spend | <span style="color:var(--amber)">$${subtree(a.id).toFixed(3)}</span> | ${a.input_tokens.toLocaleString()}↓ ${a.output_tokens.toLocaleString()}↑</div>
    <div class="hint">▸▸ <span class="b1">${esc(permName())}</span> on <span class="dim">(shift+tab to cycle)</span> · ${snap.agents.length} agent${snap.agents.length === 1 ? "" : "s"}</div>
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
    body.innerHTML = `<div class="tb" data-agent="${a.id}">${agentBody(a)}</div><footer class="tf"><label class="tp"><span style="color:var(--term-dim)">❯</span><input placeholder="${a.id === MAIN ? "Describe the goal, or answer the main agent" : "Only the main agent takes messages"}" ${a.id === MAIN ? "" : "disabled"} autocomplete="off" spellcheck="false" aria-label="Message"></label>${statusHTML(a)}</footer>`;
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
    <button class="card" data-k="conversation">${icon("reply")}<span class="ct">Conversation</span><span class="cs">One thread between two agents, or with you</span></button>
    <button class="card" data-k="terminal">${icon("term")}<span class="ct">Terminal</span><span class="cs">A real shell in this project's folder</span></button>
    <button class="card" data-k="options">${icon("opts")}<span class="ct">Visual options</span><span class="cs">Variants an agent wants you to choose between</span></button></div>
    ${agents.length > 1 ? `<h5>Agent sessions</h5><div class="alist">${agents.filter(a => a.id !== MAIN).map(a => `<button class="chip-btn" data-a="${a.id}">${icon("sparkle")}${esc(a.title)}<span class="dot ${dotFor(a.status)}"></span></button>`).join("")}</div>` : ""}</div>`;
  $$("[data-k]", body).forEach(b => (b.onclick = () => setPane(slot, { kind: b.dataset.k, agent: 0, url: null })));
  $$("[data-a]", body).forEach(b => (b.onclick = () => setPane(slot, { kind: "agent", agent: +b.dataset.a, url: null })));
}

// ------------------------------------------------------------------ workbench views
// The IDE's two views: Canvases (tabs of split canvases) and Wall (every agent's
// session tiled, as many as there are). Picked per user, kept in prefs. The
// Inbox — what needs you — rides beside them in the footer, not here.
//
// Start is not one of them any more: it is the ADE's landing — the home with
// the composer — and the IDE is the project's workspace, whose canvases hold no
// chat (docs/ADE-AND-IDE.md). Keeping it here let a stored `wb_view: "home"`
// open the IDE on a chatbox, which is exactly what the IDE must not do.
const WB = [["canvases", "window", "Canvases", "Sessions, files and previews side by side"], ["wall", "monitor", "Wall", "Every agent's session, tiled"]];

// The view menu in the title bar: Canvases, Wall.
function renderWbv() {
  const n = snap.agents.filter(needsYou).length, cur = WB.find(([k]) => k === S.wb) || WB[0];
  const b = $("#viewBtn");
  b.innerHTML = `${icon(cur[1])}<span>${cur[2]}</span>${n ? `<i class="rb">${n}</i>` : ""}${icon("updown", "ud")}`;
  b.onclick = () => openPop(b, `<div class="vmenu">${WB.map(([k, ic, l, d]) => `<button class="pi${S.wb === k ? " on" : ""}" data-wb="${k}" ${!hasProject ? "disabled" : ""}>${icon(ic)}<span class="vt"><b>${l}</b><small>${d}</small></span></button>`).join("")}</div>`, pop => {
    $$("[data-wb]", pop).forEach(x => (x.onclick = () => { closePop(); setWb(x.dataset.wb); }));
  });
  $("#win").classList.toggle("wb-alt", S.wb !== "canvases" || !hasProject);
}
function setWb(v) {
  S.wb = WB.some(([k]) => k === v) ? v : "canvases";
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
  renderWall(g);
}

// Canvases sit absolutely at the rectangles of the tab's layout tree, so a
// new arrangement (a drag preview, a resize) is a style change the browser
// animates rather than a rebuild.
// Rich answers (OpenUI) in Chat: on unless turned off in Settings.

// The painting is at full strength on Home (Code's workbench, Home view) and on
// the two chat surfaces, which sit on it the same way. Everywhere else it is
// muted toward the page, so panes full of text stay readable.
function syncArt() {
  const chatSurface =
    S.mode === "chat" ||
    (S.mode === "code" && (S.codeView === "pair" || !hasProject));
  // Settings is a frosted card: it needs the painting behind it, or the frost has
  // nothing to show and it reads as the flat slab it used to be.
  if (window.Art) Art.setVivid(chatSurface || S.settings);
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
const gridRO = new ResizeObserver(() => {
  if (!S.els || S.drag) return;
  const g = $("#grid");
  if (!g) return;
  if (!tab()) return;
  g.classList.add("still"); position(tree(tab()));
  requestAnimationFrame(() => g.classList.remove("still"));
});

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
    bindArtifacts();
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
// The preview is an <iframe>, which is the right tool for the thing this canvas
// is for: a dev server on this machine. It is the wrong tool for the wider web,
// and it fails in a way that reads as "refused to connect": almost every public
// site sends `X-Frame-Options: DENY` or a CSP `frame-ancestors` rule, and the
// browser then refuses to render it *inside* a frame. That is not a network
// problem and nothing here can override it, so an external address is opened in
// the real browser instead, and a preview that never loads says so rather than
// sitting on a blank rectangle.
const isLocalUrl = u => /^https?:\/\/(localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1\]|10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.)/i.test(u);
function browserPane(p, body, ps) {
  const vps = [["desktop", "window", "Desktop"], ["tablet", "doc", "Tablet"], ["phone", "term", "Phone"]];
  const vp = ps.viewport || "desktop";
  const url = p.url || "";
  const local = isLocalUrl(url);
  const check = (p.check && p.check.url === url) ? p.check : null;
  // What the canvas will do with this address. A local address is what the
  // canvas is for and is framed without asking; a public one is asked about
  // first, because most of the web sends a header that makes framing
  // impossible and the engine's refusal is indistinguishable from a dead
  // server until you know which it is.
  const show = !url ? "blank" : local || (check && check.frameable !== false) || p.force ? "frame" : check ? "opened" : "checking";
  body.innerHTML = `<div class="browser">
    <div class="b-bar">
      <button class="ib" data-b="back" aria-label="Back">${icon("back")}</button><button class="ib" data-b="fwd" aria-label="Forward">${icon("fwd")}</button><button class="ib" data-b="reload" aria-label="Reload">${icon("reload")}</button>
      <label class="addr">${icon("globe")}<input class="url" value="${esc(url)}" placeholder="http://localhost:5173" aria-label="Address">${local ? '<span class="live"><span class="dot"></span>live</span>' : ""}</label>
      <div class="vp" role="group" aria-label="Viewport">${vps.map(([k, ic, l]) => `<button class="ib ${vp === k ? "on" : ""}" data-vp="${k}" aria-label="${l}">${icon(ic)}</button>`).join("")}</div>
      ${url ? `<button class="ib" data-b="ext" aria-label="Open in your browser" title="Open in your browser">${icon("ext")}</button>` : ""}
    </div>
    <div class="b-view"><div class="frame live ${vp === "desktop" ? "" : vp}">${show === "frame"
      ? `<iframe src="${esc(url)}" title="Preview of ${esc(url)}" sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-modals"></iframe>`
      : show === "blank"
        ? `<div class="blankpage"><b>No preview yet</b><span>Type a dev server address above, for example the URL an agent printed after <code>npm run dev</code>, and press Enter.</span></div>`
        : show === "checking"
          ? `<div class="blankpage"><b>Checking ${esc(url)}…</b><span>Seeing whether the site allows being shown in a frame.</span></div>`
          : `<div class="blankpage"><b>Opened in your browser</b><span>${esc((check && check.reason) || "")}</span><span class="b-hint">This is not a limit of Backspace: a page that sends <code>X-Frame-Options</code> or a <code>frame-ancestors</code> rule refuses to render inside any other app, which is the point of the header. The canvas is for servers on this machine.</span></div>`}</div>
      ${show === "opened" ? `<div class="b-note">${icon("warn")}<span>Shown in your browser instead.</span><button class="btn sm" data-b="force">Try in the canvas anyway</button><button class="btn sm" data-b="ext">Open in browser</button></div>` : ""}
      ${url && !local && show === "frame" && check ? `<div class="b-note">${icon("check")}<span>${esc(check.reason)}</span><button class="btn sm" data-b="ext">Open in browser</button></div>` : ""}
      <div class="b-note b-fail" hidden>${icon("warn")}<span></span><button class="btn sm" data-b="ext">Open in browser</button></div>
    </div></div>`;
  const inp = body.querySelector(".url"), f = body.querySelector("iframe");
  const ext = () => { if (p.url) invoke("open_url", { url: p.url }).catch(e => toast(String(e), "err")); };
  const forceBtn = body.querySelector("[data-b=force]");
  if (forceBtn) forceBtn.onclick = () => { p.force = true; browserPane(p, body, ps); };
  if (show === "frame" && !local) ext();
  // A public address: ask the site what it says about framing before it fails.
  if (url && !local && !check && !p.force) {
    invoke("frame_check", { url }).then(r => {
      if (!r || p.url !== url) return;
      p.check = r;
      // A site that allows framing is shown; one that does not is opened, so
      // the user sees the page rather than a refusal they cannot act on.
      browserPane(p, body, ps);
      renderList(); renderPaneTitles();
    }).catch(() => { p.force = true; browserPane(p, body, ps); });
  }
  // A frame that never fires `load` is a refusal we cannot see into (cross-origin
  // frames do not report their status), so report the likeliest cause.
  if (f) {
    let loaded = false;
    f.addEventListener("load", () => (loaded = true));
    setTimeout(() => {
      if (loaded || !f.isConnected) return;
      const note = body.querySelector(".b-fail");
      if (!note) return;
      note.querySelector("span").textContent = `${p.url} did not load in the canvas. A site that forbids framing, or a server that is not running, both look like this.`;
      note.hidden = false;
    }, 4000);
  }
  inp.addEventListener("keydown", e => {
    if (e.key !== "Enter") return;
    let u = inp.value.trim(); if (!u) return;
    if (!/^https?:\/\//.test(u)) u = "http://" + u;
    p.url = u; p.check = null; p.force = false; saveTabs(); browserPane(p, body, ps); renderList(); renderPaneTitles();
  });
  const rb = body.querySelector("[data-b=reload]");
  if (rb) rb.onclick = () => { if (f) f.src = f.src; else { p.check = null; browserPane(p, body, ps); } };
  const bb = body.querySelector("[data-b=back]");
  if (bb) bb.onclick = () => { try { f.contentWindow.history.back(); } catch (e) {} };
  const fb2 = body.querySelector("[data-b=fwd]");
  if (fb2) fb2.onclick = () => { try { f.contentWindow.history.forward(); } catch (e) {} };
  $$("[data-b=ext]", body).forEach(b => (b.onclick = ext));
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
  // ---------------------------------------------------------------- the app
  // Mounted once, when the manifest is read; everything else is lazy.
  function mount(el) { el.innerHTML = '<div class="grid" id="grid"></div>'; }
  function init() { const g = $("#grid"); if (g) gridRO.observe(g); }

  // A layout pass from the shell: the bar and the grid follow the project.
  function layout() {
    if (!window.Shell) return;
    renderTabs();
    if (Shell.showing("canvas")) renderGrid();
  }

  // One refresh of the runtime's data: the tab strip, the drawer (even when
  // closed, so it is right when opened) and any canvas an agent opened.
  function afterData() {
    renderTabs();
    renderDrawer();
    applyCanvasOps();
  }

  // What the title bar says this app is holding. The frame asks; the shell no
  // longer names Code's project.
  function head() {
    const p = hasProject && snap.workspace ? snap.workspace : null;
    return p
      ? `<button class="zt proj" id="zhProj" data-popper title="${esc(p)} — switch project">${icon("folder")}<b>${esc(base(p))}</b>${icon("chevd")}</button>`
      : `<button class="zt proj quiet" id="zhProj" data-popper title="No folder open — open one">${icon("folder")}<b>No folder</b>${icon("chevd")}</button>`;
  }
  // The window title (taskbar, alt-tab) follows the same rule.
  function windowTitle() {
    const p = S.mode === "code" && hasProject && snap.workspace ? base(snap.workspace) : "";
    return p ? `${p} · Backspace` : "Backspace";
  }
  // A stored wb_view is a view we still have.
  const wbView = v => (WB.some(([k]) => k === v) ? v : "canvases");

  function elFor() { return S.codeView === "pair" && hasProject && snap.workspace ? "chat" : "canvas"; }
  function classes() { return S.codeView === "pair" ? ["pair-on"] : []; }
  function paint(on) {
    if (!on || !window.Shell || !Shell.showing("canvas")) return;
    renderTabs(); renderGrid();
  }

  // ---------------------------------------------------------------- terminal dock
  // The header's terminal button: one shell in the project, docked under the
  // page or to its right, whichever its menu is told.
  const dock = { ps: {}, at: null };
  function termMenu(anchor) {
    const on = !!dock.at, pick = at => `<button class="pi" data-at="${at}">${icon(at === "under" ? "panelb" : "panelr")}${at === "under" ? "Under the page" : "To the right"}${dock.at === at ? `<span class="meta on">${icon("check")}</span>` : ""}</button>`;
    openPop(anchor, `<div class="ph-t">Terminal</div>${pick("under")}${pick("right")}${on ? `<div class="psep"></div><button class="pi" data-at="">${icon("close")}Close the terminal</button>` : ""}`, pop => {
      pop.querySelectorAll("[data-at]").forEach(b => (b.onclick = () => { closePop(); setDock(b.dataset.at || null); }));
    });
  }
  function setDock(at) {
    const mainc = $("#win > .mainc"); let el = $("#tdock");
    mainc.classList.remove("term-under", "term-right");
    $("#termBtn").setAttribute("aria-pressed", String(!!at));
    if (!at) { if (dock.ps.cleanup) dock.ps.cleanup(); dock.ps = {}; if (el) el.remove(); dock.at = null; return; }
    if (at !== "under" && at !== "right") return;
    mainc.classList.add("term-" + at);
    if (!el) { el = document.createElement("section"); el.id = "tdock"; el.className = "tdock"; el.setAttribute("aria-label", "Terminal"); mainc.append(el); }
    if (!dock.at) terminalPane(el, dock.ps);
    dock.at = at;
  }
  document.addEventListener("click", e => { const b = e.target.closest("#termBtn"); if (b) termMenu(b); });

  return {
    // state the frame's sidebar and the other apps read
    loadTabs: useTabsFor, tab, paneTitle, wbView, setDock,
    // the app's own surface
    mount, init, paint, elFor, classes, layout, afterData, head, windowTitle,
    // public verbs (the frame, the sidebar, Home and Chat call these)
    renderTabs, renderGrid, refreshPanes, renderDrawer, applyCanvasOps, syncArt,
    openView, openIn, actOn, switchTab, setDrawer, setWb, newTabPop, newInstance: anchor => newTabPop(anchor),
  };
})();

if (window.Frame) Frame.register({
  id: "code",
  mount: Code.mount,
  init: Code.init,
  paint: Code.paint,
  elFor: Code.elFor,
  classes: Code.classes,
  newInstance: Code.newInstance,
  head: Code.head,
  windowTitle: Code.windowTitle,
});
