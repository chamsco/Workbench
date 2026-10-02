// Backspace workbench, Tauri edition. Same layout and styles as
// design/workbench.html, driven by the real harness: every render reads the
// latest ProjectState snapshot; actions go back through Tauri commands.

const TAURI = window.__TAURI__;
const invoke = (cmd, args) => TAURI.core.invoke(cmd, args);
const $ = (s, el = document) => el.querySelector(s);
const esc = s => String(s ?? "").replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const store = {
  get(k) { try { return localStorage.getItem(k); } catch (e) { return null; } },
  set(k, v) { try { localStorage.setItem(k, v); } catch (e) {} },
};

// ------------------------------------------------------------------ icons
const P = {
  folder: '<path d="M2 4.6A1.6 1.6 0 0 1 3.6 3h2.9l1.5 1.5h4.4A1.6 1.6 0 0 1 14 6.1v5.3a1.6 1.6 0 0 1-1.6 1.6H3.6A1.6 1.6 0 0 1 2 11.4z"/>',
  sparkle: '<path d="M8 1.6 9.3 5.9 13.6 7 9.3 8.3 8 12.6 6.7 8.3 2.4 7l4.3-1.1z" fill="currentColor" stroke="none"/><circle cx="12.6" cy="12.4" r="1.2" fill="currentColor" stroke="none"/>',
  globe: '<circle cx="8" cy="8" r="5.8"/><path d="M2.2 8h11.6M8 2.2c1.7 1.6 2.6 3.6 2.6 5.8S9.7 12.2 8 13.8M8 2.2C6.3 3.8 5.4 5.8 5.4 8s.9 4.2 2.6 5.8"/>',
  term: '<rect x="2" y="3" width="12" height="10" rx="1.6"/><path d="m4.8 6.4 2 1.6-2 1.6M8.4 10h2.8"/>',
  plus: '<path d="M8 3.2v9.6M3.2 8h9.6"/>',
  home: '<path d="M2.6 7.2 8 2.8l5.4 4.4v5.6a.8.8 0 0 1-.8.8h-2.8V10H6.2v3.6H3.4a.8.8 0 0 1-.8-.8z"/>',
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
  sun: '<circle cx="8" cy="8" r="2.8"/><path d="M8 1.6v1.6M8 12.8v1.6M1.6 8h1.6M12.8 8h1.6M3.5 3.5l1.1 1.1M11.4 11.4l1.1 1.1M3.5 12.5l1.1-1.1M11.4 4.6l1.1-1.1"/>',
  moon: '<path d="M12.8 10.2A5.4 5.4 0 0 1 5.8 3.2a5.4 5.4 0 1 0 7 7z"/>',
  auto: '<circle cx="8" cy="8" r="5.6"/><path d="M8 2.4v11.2a5.6 5.6 0 0 0 0-11.2z" fill="currentColor"/>',
  bksp: '<path d="M5.2 3.2h7.6a1 1 0 0 1 1 1v7.6a1 1 0 0 1-1 1H5.2L1.8 8z"/><path d="m7.6 6 4 4M11.6 6l-4 4"/>',
  check: '<path d="m3.4 8.4 3 3 6.2-6.6"/>',
};
const icon = (n, cls = "") => `<svg class="ic ${cls}" viewBox="0 0 16 16" aria-hidden="true">${P[n]}</svg>`;
document.querySelectorAll("[data-icon]").forEach(el => (el.innerHTML = icon(el.dataset.icon)));

// ------------------------------------------------------------------ state
let snap = { name: "", agents: [], approvals: [], tickets: [], total_cost_usd: 0, router_cost_usd: 0, workspace: "" };
const S = {
  env: "terminals", layout: 2, slots: [{ t: "agent", id: 0 }, { t: "files", id: 0 }, { t: "agent", id: 0 }, { t: "files", id: 0 }],
  focus: 0, max: null, cx: 0.5, ry: 0.56, side: "projects", review: null, viewport: "desktop",
  theme: store.get("bs-theme") || "auto",
  previews: JSON.parse(store.get("bs-previews") || "[]"), btab: 0, preview: null,
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

// ------------------------------------------------------------------ theme + chrome
function applyTheme() {
  const r = document.documentElement;
  if (S.theme === "auto") delete r.dataset.theme; else r.dataset.theme = S.theme;
  $("#themeBtn").innerHTML = icon(S.theme === "auto" ? "auto" : S.theme === "light" ? "sun" : "moon");
  $("#themeBtn").title = "Theme: " + S.theme;
}
$("#themeBtn").onclick = () => { S.theme = { auto: "light", light: "dark", dark: "auto" }[S.theme]; store.set("bs-theme", S.theme); applyTheme(); };
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

// ------------------------------------------------------------------ sidebar
function treeOrder() {
  const out = [];
  const walk = id => { out.push(id); snap.agents.filter(a => a.parent === id).forEach(c => walk(c.id)); };
  if (snap.agents.length) walk(MAIN);
  return out;
}
function renderSeg() {
  const items = [["projects", "folder", "Projects"], ["agents", "sparkle", "Agents"], ["tickets", "ticket", "Tickets"]];
  $("#seg").innerHTML = items.map(([k, ic, l]) => `<button aria-pressed="${S.side === k}" data-side="${k}">${icon(ic)}${l}</button>`).join("");
  $("#seg").querySelectorAll("button").forEach(b => (b.onclick = () => { S.side = b.dataset.side; renderSeg(); renderList(); }));
}
function agentRow(a, lvl) {
  const cur = S.slots[S.focus];
  const sel = S.env === "terminals" && cur.t === "agent" && cur.id === a.id ? " sel" : "";
  const esc1 = a.escalations.length ? `<span class="esc">↑${a.escalations.length}</span>` : "";
  return `<button class="row ${lvl}${sel}${a.kind === "triage" ? " muted" : ""}" data-open="${a.id}"><span class="agent">${icon("sparkle")}</span><span class="lab">${esc(a.title)}</span><span class="tail">${esc1}<span class="dot ${dotFor(a.status)}"></span></span></button>`;
}
function renderList() {
  let h = "";
  if (S.side === "projects") {
    h += `<button class="row group sel">${icon("folder")}<span class="lab">${esc(snap.name)}</span><span class="cnt">${snap.agents.length}</span></button>`;
    treeOrder().forEach(id => (h += agentRow(agent(id), agent(id).depth ? "l2" : "l1")));
    S.previews.forEach((u, i) => (h += `<button class="row l1" data-pv="${i}"><span class="web">${icon("globe")}</span><span class="lab">Preview · ${esc(u.replace(/^https?:\/\//, ""))}</span></button>`));
    const wts = snap.agents.filter(a => a.branch && a.id !== MAIN);
    if (wts.length) {
      h += `<button class="row group">${icon("folder")}<span class="lab">worktrees</span><span class="cnt">${wts.length}</span></button>`;
      wts.forEach(a => (h += `<button class="row l1" data-files="${a.id}"><span class="term">${icon("branch")}</span><span class="lab">${esc(a.branch)}</span><span class="tail">${esc(a.ticket || "")}</span></button>`));
    }
  } else if (S.side === "agents") {
    h += `<div class="grp-label">${esc(snap.name)} · ${snap.agents.length} agents</div>`;
    treeOrder().forEach(id => { const a = agent(id); h += agentRow(a, "") + `<span class="sub">${esc(route(a))} · $${subtree(a.id).toFixed(3)}</span>`; });
  } else {
    const groups = [["Needs you", ["proposed", "in_review", "ready_for_human", "needs_info"]], ["Running", ["queued", "in_progress"]], ["Ready", ["ready_for_agent", "needs_triage"]], ["Done", ["done"]], ["Closed", ["failed", "wontfix"]]];
    groups.forEach(([n, sts]) => {
      const rows = snap.tickets.filter(t => sts.includes(t.state));
      if (!rows.length) return;
      h += `<div class="grp-label">${n} · ${rows.length}</div>`;
      rows.forEach(t => (h += `<button class="row" ${t.assignee != null ? `data-open="${t.assignee}"` : ""}><span class="cnt" style="font-family:var(--font-mono)">${String(t.num).padStart(2, "0")}</span><span class="lab">${esc(t.title)}</span><span class="tail">${t.state.replace(/_/g, "-")}</span></button>`));
    });
    if (!snap.tickets.length) h += `<div class="nothing">No tickets yet. The main agent creates them from your goal.</div>`;
  }
  $("#list").innerHTML = h;
  $("#list").querySelectorAll("[data-open]").forEach(b => (b.onclick = () => openView({ t: "agent", id: +b.dataset.open })));
  $("#list").querySelectorAll("[data-files]").forEach(b => (b.onclick = () => openView({ t: "files", id: +b.dataset.files })));
  $("#list").querySelectorAll("[data-pv]").forEach(b => (b.onclick = () => { S.btab = +b.dataset.pv; setEnv("browser"); }));
}
function renderCard() {
  const run = snap.agents.filter(a => a.status === "running").length;
  const wts = snap.agents.filter(a => a.branch && a.id !== MAIN).length;
  const row = (c, l, v) => `<div class="port"><span class="dot ${c}"></span><span class="pl">${l}</span><span class="pn" style="margin-left:auto;color:var(--fg-3)">${v}</span></div>`;
  $("#ports").innerHTML = `<div class="ports-h">This run · ${esc(snap.name)}</div>` +
    row("run", "Agents running", run) + row(pending().length ? "wait" : "", "Waiting on you", pending().length) +
    row("done", "Worktrees", wts) + row("", "Spent", `$${snap.total_cost_usd.toFixed(3)} · router $${snap.router_cost_usd.toFixed(3)}`);
}

// ------------------------------------------------------------------ tabs
const ENVS = [["terminals", "Terminals"], ["browser", "Browser"], ["diagram", "Diagram"], ["docs", "PLAN.md"]];
function renderTabs() {
  const n = pending().length;
  $("#tabs").innerHTML = ENVS.map(([k, l]) => `<button class="tab" role="tab" aria-selected="${S.env === k}" data-env="${k}">${l}${k === "diagram" && n ? '<span class="badge"></span>' : ""}</button>`).join("") +
    `<button class="tab plus" role="tab" aria-selected="${S.env === "new"}" data-env="new" aria-label="New tab">${icon("plus")}</button>`;
  $("#tabs").querySelectorAll("[data-env]").forEach(b => (b.onclick = () => setEnv(b.dataset.env)));
  const lays = [[2, '<rect x="1" y="1" width="6" height="10" rx="1.5"/><rect x="9" y="1" width="6" height="10" rx="1.5"/>'], [1, '<rect x="1" y="1" width="14" height="10" rx="1.5"/>'], [4, '<rect x="1" y="1" width="6" height="4.2" rx="1"/><rect x="9" y="1" width="6" height="4.2" rx="1"/><rect x="1" y="6.8" width="6" height="4.2" rx="1"/><rect x="9" y="6.8" width="6" height="4.2" rx="1"/>']];
  $("#lay").innerHTML = lays.map(([k, svg]) => `<button aria-pressed="${S.env === "terminals" && S.layout === k && S.max == null}" data-lay="${k}" aria-label="${k === 4 ? "2 by 2 grid" : k + " pane"}"><svg viewBox="0 0 16 12" fill="currentColor" opacity=".85">${svg}</svg></button>`).join("");
  $("#lay").querySelectorAll("[data-lay]").forEach(b => (b.onclick = () => { S.layout = +b.dataset.lay; S.max = null; S.focus = Math.min(S.focus, S.layout - 1); setEnv("terminals"); }));
  $("#cta").innerHTML = n ? `<span class="dot"></span><span class="lbl">${n} to review</span>` : `${icon("check")}<span class="lbl">Nothing to review</span>`;
  $("#cta").onclick = () => setEnv("diagram");
}
function setEnv(e) {
  S.env = e;
  ENVS.concat([["new"]]).forEach(([k]) => ($("#env-" + k).hidden = k !== e));
  renderEnv(); renderTabs(); renderList();
}
function renderEnv() {
  if (S.env === "terminals") renderGrid();
  if (S.env === "browser") renderBrowser();
  if (S.env === "diagram") renderDiagram();
  if (S.env === "docs") renderDocs();
  if (S.env === "new") renderNew();
}
addEventListener("keydown", e => {
  if (!(e.metaKey || e.ctrlKey)) return;
  const i = +e.key;
  if (i >= 1 && i <= 4) { e.preventDefault(); setEnv(ENVS[i - 1][0]); }
  if (e.key === "\\") { e.preventDefault(); setSide($("#win").classList.contains("side-closed")); }
});

// ------------------------------------------------------------------ terminals
const SPIN = ["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];
let spinI = 0;
setInterval(() => { spinI = (spinI + 1) % SPIN.length; document.querySelectorAll(".spin-g").forEach(g => (g.textContent = SPIN[spinI])); }, 130);
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
  const hint = a.id === MAIN ? 'plan approval on <span class="b1">(review in Diagram)</span> · ⌘1–4 switch tabs'
    : t ? `check: ${esc(t.check || "none")} · ${esc(t.state.replace(/_/g, "-"))}` : esc(a.status);
  return `<div class="st">[${esc(a.decision ? `${a.decision.model} @ ${a.decision.effort}` : "not routed")}] <span style="color:var(--blue)">▣ ${esc(snap.name)}</span> | <span style="color:var(--green)">⎇ ${esc(a.branch || "-")}</span></div>
    <div class="st"><span class="bar">${"█".repeat(filled)}${"░".repeat(10 - filled)}</span> ${Math.round(share * 100)}% of spend | <span style="color:var(--amber)">$${subtree(a.id).toFixed(3)}</span> | ${a.input_tokens.toLocaleString()}↓ ${a.output_tokens.toLocaleString()}↑</div>
    <div class="hint">▸▸ ${hint}</div>`;
}
async function filesBody(id, el) {
  const rows = await invoke("list_files", { agent: id });
  const a = agent(id);
  el.querySelector(".ftree").innerHTML = `<div class="ft" style="color:var(--fg-3)">${icon("chevd")}<b style="color:var(--fg)">${esc(a && a.ticket ? a.ticket + " worktree" : "Primary worktree")}</b></div>` +
    rows.map(([d, name, path, dir]) => `<button class="ft" data-name="${esc(name)}" data-path="${esc(path)}" data-dir="${dir}" style="padding-left:${18 + d * 14}px"><span class="${dir ? "dir" : /\.html?$/.test(name) ? "html" : /\.py$/.test(name) ? "py" : /\.sql$/.test(name) ? "sql" : /\.md$/.test(name) ? "md" : ""}">${icon(dir ? "chev" : "doc")}</span>${esc(name)}</button>`).join("");
  el.querySelectorAll(".ft[data-path]").forEach(b => (b.onclick = async () => {
    if (b.dataset.dir === "true") return;
    const text = await invoke("read_file", { path: b.dataset.path }).catch(e => String(e));
    let pv = el.querySelector(".preview");
    if (!pv) { pv = document.createElement("div"); pv.className = "preview"; el.appendChild(pv); }
    pv.textContent = text;
  }));
}
function viewTitle(v) {
  const a = agent(v.id);
  if (!a) return "…";
  return v.t === "files" ? (a.ticket ? `${a.ticket} worktree` : "Primary worktree") : a.id === MAIN ? `Main agent · ${snap.name}` : a.title;
}
function buildPane(v, slot) {
  const el = document.createElement("section");
  el.className = "pane" + (slot === S.focus ? " on" : "");
  el.dataset.slot = slot;
  const a = agent(v.id);
  const ic = v.t === "files" ? `<span class="term">${icon("folder")}</span>` : `<span class="agent">${icon("sparkle")}</span>`;
  el.innerHTML = `<header class="ph">${ic}<span class="t">${esc(viewTitle(v))}</span><span class="sp"></span>
    ${v.t === "files" ? "" : `<button class="ib" data-act="files" aria-label="Open worktree files">${icon("folder")}</button>`}
    <button class="ib" data-act="max" aria-label="Maximize pane">${icon("expand")}</button>
    <button class="ib" data-act="close" aria-label="Close pane">${icon("close")}</button></header>` +
    (v.t === "files"
      ? `<div class="fsearch"><input class="fq" placeholder="Search" aria-label="Search files"><button class="ib" aria-label="Match case">Aa</button></div><div class="ftree"></div>`
      : `<div class="tb" data-agent="${v.id}">${a ? agentBody(a) : ""}</div>
         <footer class="tf"><label class="tp"><span style="color:var(--term-dim)">❯</span><input placeholder="${v.id === MAIN ? "Describe the goal, or answer the main agent" : "Only the main agent takes messages"}" ${v.id === MAIN ? "" : "disabled"} autocomplete="off" spellcheck="false" aria-label="Message"></label>${a ? statusHTML(a) : ""}</footer>`);
  el.addEventListener("pointerdown", () => { if (S.focus !== slot) { S.focus = slot; document.querySelectorAll(".pane").forEach(p => p.classList.toggle("on", +p.dataset.slot === slot)); renderList(); } });
  el.querySelector("[data-act=max]").onclick = () => { S.max = S.max == null ? slot : null; renderGrid(); renderTabs(); };
  el.querySelector("[data-act=close]").onclick = () => { if (S.max != null) S.max = null; else if (S.layout > 1) { const [x] = S.slots.splice(slot, 1); S.slots.push(x); S.layout = S.layout === 4 ? 2 : 1; } S.focus = 0; renderGrid(); renderTabs(); };
  const fb = el.querySelector("[data-act=files]"); if (fb) fb.onclick = () => { S.slots[slot] = { t: "files", id: v.id }; renderGrid(); };
  const inp = el.querySelector(".tp input");
  if (inp) inp.addEventListener("keydown", e => { if (e.key === "Enter" && inp.value.trim()) { invoke("send", { text: inp.value.trim() }); inp.value = ""; } });
  if (v.t === "files") {
    filesBody(v.id, el);
    const fq = el.querySelector(".fq");
    fq.oninput = () => el.querySelectorAll(".ft[data-name]").forEach(r => (r.hidden = !r.dataset.name.toLowerCase().includes(fq.value.toLowerCase())));
  }
  requestAnimationFrame(() => { const tb = el.querySelector(".tb"); if (tb) tb.scrollTop = tb.scrollHeight; });
  return el;
}
function renderGrid() {
  const g = $("#grid");
  const n = S.max != null ? 1 : S.layout;
  if (S.max == null) fillSlots(n);
  const shown = S.max != null ? [[S.slots[S.max], S.max]] : S.slots.slice(0, n).map((v, i) => [v, i]);
  g.className = "grid L" + n;
  g.style.setProperty("--cx", S.cx); g.style.setProperty("--ry", S.ry);
  g.innerHTML = "";
  const areas = { 1: ["1 / 1"], 2: ["1 / 1", "1 / 3"], 4: ["1 / 1", "1 / 3", "3 / 1", "3 / 3"] }[n];
  shown.forEach(([v, slot], i) => { const p = buildPane(v, slot); p.style.gridArea = areas[i]; g.appendChild(p); });
  if (n >= 2) g.appendChild(gutter("v"));
  if (n === 4) g.appendChild(gutter("h"));
}
// A slot that repeats an earlier one takes the busiest agent not on screen yet.
function fillSlots(n) {
  const key = v => v.t + v.id, seen = new Set();
  const rank = a => ({ running: 0, awaiting_approval: 1, failed: 2 })[a.status] ?? 3;
  for (let i = 0; i < n; i++) {
    if (seen.has(key(S.slots[i]))) {
      const a = snap.agents.filter(a => a.id !== MAIN && !seen.has("agent" + a.id)).sort((x, y) => rank(x) - rank(y))[0];
      S.slots[i] = a ? { t: "agent", id: a.id } : { t: "files", id: snap.agents.find(a => a.branch && !seen.has("files" + a.id))?.id ?? MAIN };
      if (seen.has(key(S.slots[i]))) S.slots[i] = { t: "agent", id: MAIN };
    }
    seen.add(key(S.slots[i]));
  }
}
// Live updates touch only transcripts and status lines, so typing and scroll positions survive.
function refreshPanes() {
  document.querySelectorAll(".tb[data-agent]").forEach(tb => {
    const a = agent(+tb.dataset.agent);
    if (!a) return;
    const atBottom = tb.scrollHeight - tb.scrollTop - tb.clientHeight < 40;
    tb.innerHTML = agentBody(a);
    if (atBottom) tb.scrollTop = tb.scrollHeight;
    const foot = tb.parentElement.querySelector(".tf");
    foot.querySelectorAll(".st, .hint").forEach(x => x.remove());
    foot.insertAdjacentHTML("beforeend", statusHTML(a));
  });
}
function gutter(dir) {
  const d = document.createElement("div");
  d.className = "gut " + dir; d.setAttribute("role", "separator");
  d.addEventListener("pointerdown", e => {
    e.preventDefault(); d.setPointerCapture(e.pointerId); d.classList.add("drag");
    const r = $("#grid").getBoundingClientRect();
    const move = ev => {
      if (dir === "v") S.cx = Math.min(0.8, Math.max(0.2, (ev.clientX - r.left) / r.width));
      else S.ry = Math.min(0.8, Math.max(0.2, (ev.clientY - r.top) / r.height));
      $("#grid").style.setProperty("--cx", S.cx); $("#grid").style.setProperty("--ry", S.ry);
    };
    const up = () => { d.classList.remove("drag"); d.removeEventListener("pointermove", move); d.removeEventListener("pointerup", up); };
    d.addEventListener("pointermove", move); d.addEventListener("pointerup", up);
  });
  d.addEventListener("dblclick", () => { S.cx = 0.5; S.ry = 0.56; renderGrid(); });
  return d;
}
function openView(v) {
  const vis = S.slots.slice(0, S.layout).findIndex(x => x.t === v.t && x.id === v.id);
  if (vis >= 0) S.focus = vis; else S.slots[S.focus] = v;
  S.max = null; setEnv("terminals");
  if (narrow()) setSide(false);
}

// ------------------------------------------------------------------ browser (real webview)
function renderBrowser() {
  const tabs = S.previews;
  const cur = tabs[S.btab] || tabs[0];
  const vps = [["desktop", "window", "Desktop"], ["tablet", "doc", "Tablet"], ["phone", "term", "Phone"]];
  $("#env-browser").innerHTML = `<div class="browser">
    <div class="b-tabs" role="tablist">${tabs.map((u, i) => `<button class="btab" role="tab" aria-selected="${u === cur}" data-bt="${i}"><span class="fav"></span><span class="l">${esc(u.replace(/^https?:\/\//, ""))}</span></button>`).join("")}
      <button class="ib" id="newTab" aria-label="New preview tab" style="margin-bottom:2px">${icon("plus")}</button></div>
    <div class="b-bar">
      <button class="ib" id="bBack" aria-label="Back">${icon("back")}</button><button class="ib" id="bFwd" aria-label="Forward">${icon("fwd")}</button><button class="ib" id="bReload" aria-label="Reload">${icon("reload")}</button>
      <label class="addr">${icon("globe")}<input id="url" value="${esc(cur || "")}" placeholder="http://localhost:5173" aria-label="Address"><span class="live"><span class="dot"></span>live</span></label>
      <div class="vp" role="group" aria-label="Viewport">${vps.map(([k, ic, l]) => `<button class="ib ${S.viewport === k ? "on" : ""}" data-vp="${k}" aria-label="${l}">${icon(ic)}</button>`).join("")}</div>
      ${cur ? `<button class="ib" id="bClose" aria-label="Close preview">${icon("close")}</button>` : ""}
    </div>
    <div class="b-view"><div class="frame live ${S.viewport === "desktop" ? "" : S.viewport}">${cur
      ? `<iframe id="iframe" src="${esc(cur)}" title="Preview of ${esc(cur)}"></iframe>`
      : `<div class="blankpage"><b>No previews yet</b><span>Type a dev server address above, for example the URL an agent printed after <code>npm run dev</code>, and press Enter.</span></div>`}</div></div></div>`;
  const go = () => {
    let u = $("#url").value.trim(); if (!u) return;
    if (!/^https?:\/\//.test(u)) u = "http://" + u;
    if (cur && S.btab < tabs.length) tabs[S.btab] = u; else { tabs.push(u); S.btab = tabs.length - 1; }
    store.set("bs-previews", JSON.stringify(tabs)); renderBrowser(); renderList();
  };
  $("#url").addEventListener("keydown", e => { if (e.key === "Enter") go(); });
  $("#newTab").onclick = () => { S.btab = tabs.length; renderBrowserBlank(); };
  $("#env-browser").querySelectorAll("[data-bt]").forEach(b => (b.onclick = () => { S.btab = +b.dataset.bt; renderBrowser(); }));
  $("#env-browser").querySelectorAll("[data-vp]").forEach(b => (b.onclick = () => { S.viewport = b.dataset.vp; renderBrowser(); }));
  const f = $("#iframe");
  $("#bReload").onclick = () => { if (f) f.src = f.src; };
  $("#bBack").onclick = () => { try { f.contentWindow.history.back(); } catch (e) {} };
  $("#bFwd").onclick = () => { try { f.contentWindow.history.forward(); } catch (e) {} };
  const c = $("#bClose"); if (c) c.onclick = () => { tabs.splice(S.btab, 1); S.btab = 0; store.set("bs-previews", JSON.stringify(tabs)); renderBrowser(); renderList(); };
}
function renderBrowserBlank() { const keep = S.previews; S.previews = keep; renderBrowser(); $("#url").value = ""; $("#url").focus(); }

// ------------------------------------------------------------------ diagram review
const NW = 210, NH = 60, GX = 80, GY = 34;
let view = null, lastDiagramKey = "";
async function renderDiagram() {
  const list = snap.approvals.slice().reverse();
  if (S.review == null || !snap.approvals[S.review]) S.review = (pending()[0] || list[0] || {}).id ?? null;
  const ap = S.review != null ? snap.approvals[S.review] : null;
  const d = ap ? await invoke("diagram", { id: ap.id }) : null;
  const label = a => a.kind === "plan" ? `Plan · ${a.tickets.length} tickets` : a.agent === MAIN ? "Final deliverable" : `Ticket · ${a.tickets.join(", ")}`;
  $("#env-diagram").innerHTML = `<div class="dg">
    <div class="dg-list" role="tablist" aria-label="Reviews"><h3>Reviews · diagram first</h3>${list.length ? list.map(a => {
      const st = a.state.state;
      return `<button class="rv" role="tab" aria-selected="${a.id === S.review}" data-rv="${a.id}"><span class="k">${icon("diagram")}${a.kind === "plan" ? "Plan" : a.agent === MAIN ? "Final" : "Ticket"}</span><span class="t">${esc(a.kind === "plan" ? label(a) : agent(a.agent).title)}</span><span class="chip ${st === "pending" ? "pending" : st === "approved" ? "approved" : "locked"}">${st}</span></button>`;
    }).join("") : `<div class="nothing">Nothing has needed your review yet.</div>`}</div>
    <div class="stage" id="stage">${d ? `
      <svg class="dgsvg" id="dgsvg" role="img" aria-label="${esc(d.title)}"></svg>
      <div class="facts"><span class="f title">${esc(d.title)}</span>${d.facts.map(([k, v]) => `<span class="f"><b>${esc(v)}</b> ${esc(k)}</span>`).join("")}</div>
      <div class="zoomc"><button class="ib" data-z="out" aria-label="Zoom out">−</button><button class="ib" data-z="in" aria-label="Zoom in">+</button><button class="ib" data-z="fit" aria-label="Fit">${icon("expand")}</button></div>
      <div class="rcard" id="rcard"></div>` : `<div class="empty"><div class="mark">${icon("diagram")}</div><div class="nothing">Plans and deliverables appear here, drawn by the harness before you read them.</div></div>`}</div></div>`;
  $("#env-diagram").querySelectorAll("[data-rv]").forEach(b => (b.onclick = () => { S.review = +b.dataset.rv; view = null; renderDiagram(); }));
  if (!d) return;
  const key = ap.id + ":" + d.nodes.length;
  if (key !== lastDiagramKey) { view = null; lastDiagramKey = key; }
  drawDiagram(d); renderReviewCard(ap, d, label(ap)); wireStage(d);
}
const pos = n => [40 + n.layer * (NW + GX), 120 + n.row * (NH + GY)];
function drawDiagram(d) {
  const tone = { done: "var(--green)", warn: "var(--amber)", neutral: "var(--fg-4)", active: "var(--blue)", fail: "var(--red)" };
  let h = `<defs><pattern id="dots" width="22" height="22" patternUnits="userSpaceOnUse"><circle cx="1.5" cy="1.5" r="1.1" fill="var(--grid-dot)"/></pattern>
    <marker id="ar" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" class="e-head"/></marker></defs>
    <rect x="-4000" y="-4000" width="9000" height="9000" fill="url(#dots)"/>`;
  const layers = Math.max(0, ...d.nodes.map(n => n.layer)) + 1;
  if (layers > 1) for (let l = 0; l < layers; l++) h += `<text x="${40 + l * (NW + GX)}" y="100" class="n-wave">WAVE ${l + 1}</text>`;
  d.edges.forEach(([a, b]) => {
    const [ax, ay] = pos(d.nodes[a]), [bx, by] = pos(d.nodes[b]);
    const x1 = ax + NW, y1 = ay + NH / 2, x2 = bx, y2 = by + NH / 2, xm = x1 + GX / 2;
    h += `<path class="e-line" d="M${x1},${y1} H${xm} V${y2} H${x2 - 2}" marker-end="url(#ar)"/>`;
  });
  d.nodes.forEach(n => {
    const [x, y] = pos(n);
    h += `<g><rect x="${x}" y="${y}" width="${NW}" height="${NH}" rx="10" fill="var(--node)" stroke="${tone[n.tone] || "var(--fg-4)"}" stroke-width="1.6"/>
      <text x="${x + 14}" y="${y + 25}" class="n-label">${esc(n.label.length > 24 ? n.label.slice(0, 23) + "…" : n.label)}</text><text x="${x + 14}" y="${y + 44}" class="n-sub">${esc(n.sub.length > 30 ? n.sub.slice(0, 29) + "…" : n.sub)}</text></g>`;
  });
  $("#dgsvg").innerHTML = h;
}
function renderReviewCard(ap, d, label) {
  const st = ap.state.state, files = d.files;
  const max = Math.max(1, ...files.map(f => f.added + f.removed));
  $("#rcard").innerHTML = `<h4>${esc(label)}</h4><p>${esc(ap.deliverable.summary)}</p>
    ${d.warnings.length ? `<div class="warns">${d.warnings.map(w => `<div>${esc(w)}</div>`).join("")}</div>` : ""}
    ${files.length ? `<div class="fb">${files.slice(0, 8).map(f => `<div class="r"><span class="p">${esc(f.path)}</span><span class="bars"><span class="ad" style="width:${(f.added / max) * 90}px"></span><span class="rm" style="width:${(f.removed / max) * 90}px"></span></span><span class="n">+${f.added} −${f.removed}</span></div>`).join("")}</div>` : ""}
    ${st === "pending" ? `<textarea id="fbk" placeholder="Feedback for the agent (needed to reject)" aria-label="Feedback"></textarea>
      <div class="acts"><button class="btn primary" id="appr">Approve</button><button class="btn danger" id="rej">Reject</button><button class="btn" id="sess">Session</button><span class="formerr" id="ferr" role="alert"></span></div>`
      : `<div class="acts"><span class="chip ${st === "approved" ? "approved" : "locked"}">${esc(st)}${ap.state.feedback ? ": " + esc(ap.state.feedback) : ""}</span><button class="btn" id="sess">Session</button></div>`}`;
  $("#sess").onclick = () => openView({ t: "agent", id: ap.agent });
  if (st !== "pending") return;
  $("#appr").onclick = () => invoke("approve", { id: ap.id });
  $("#rej").onclick = () => {
    const fb = $("#fbk").value.trim();
    if (!fb) { $("#ferr").textContent = "Write what should change first."; return; }
    invoke("reject", { id: ap.id, feedback: fb });
  };
}
function wireStage(d) {
  const stage = $("#stage"), svg = $("#dgsvg");
  const xs = d.nodes.map(n => pos(n)[0]), ys = d.nodes.map(n => pos(n)[1]);
  const bounds = { x: 0, y: 80, w: Math.max(...xs) + NW + 40, h: Math.max(...ys) + NH - 80 + 40 };
  // Never magnify past 1.25x, and keep the drawing in the band above the review card.
  const fit = () => {
    const sw = svg.clientWidth || 600, sh = svg.clientHeight || 400;
    const card = $("#rcard"), band = card ? Math.max(0.5, 1 - (card.offsetHeight + 32) / sh) : 1;
    const k = Math.min(1.25, sw / (bounds.w + 80), (sh * band) / (bounds.h + 80));
    const w = sw / k, h = sh / k;
    view = { x: bounds.x + bounds.w / 2 - w / 2, y: bounds.y + bounds.h / 2 - (h * band) / 2 - 20 / k, w, h };
  };
  if (!view) fit();
  const apply = () => svg.setAttribute("viewBox", `${view.x} ${view.y} ${view.w} ${view.h}`);
  apply();
  const zoom = (f, cx, cy) => {
    const b = svg.getBoundingClientRect();
    const px = view.x + ((cx - b.left) / b.width) * view.w, py = view.y + ((cy - b.top) / b.height) * view.h;
    const nw = Math.min(8000, Math.max(300, view.w * f)), k = nw / view.w;
    view = { x: px - (px - view.x) * k, y: py - (py - view.y) * k, w: nw, h: view.h * k }; apply();
  };
  stage.querySelectorAll("[data-z]").forEach(b => (b.onclick = () => {
    const bb = svg.getBoundingClientRect();
    if (b.dataset.z === "fit") { fit(); apply(); } else zoom(b.dataset.z === "in" ? 0.8 : 1.25, bb.left + bb.width / 2, bb.top + bb.height / 2);
  }));
  svg.addEventListener("wheel", e => { e.preventDefault(); zoom(Math.exp(e.deltaY * 0.0015), e.clientX, e.clientY); }, { passive: false });
  let last = null;
  svg.addEventListener("pointerdown", e => { last = [e.clientX, e.clientY]; svg.setPointerCapture(e.pointerId); stage.classList.add("drag"); });
  svg.addEventListener("pointermove", e => { if (!last) return; const b = svg.getBoundingClientRect(); view.x -= (e.clientX - last[0]) * view.w / b.width; view.y -= (e.clientY - last[1]) * view.h / b.height; last = [e.clientX, e.clientY]; apply(); });
  const up = () => { last = null; stage.classList.remove("drag"); };
  svg.addEventListener("pointerup", up); svg.addEventListener("pointercancel", up);
}

// ------------------------------------------------------------------ PLAN.md + new tab
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
async function renderDocs() {
  const text = await invoke("read_file", { path: snap.workspace + "/PLAN.md" }).catch(() => null);
  $("#env-docs").innerHTML = `<div class="docs"><article class="doc">${text ? md(text) : `<h1>PLAN.md</h1><div class="by">${esc(snap.name)}</div><p>The main agent writes PLAN.md before it proposes tickets. It will show here once it exists.</p>`}</article></div>`;
}
function renderNew() {
  $("#env-new").innerHTML = `<div class="empty"><div class="mark">${icon("bksp")}</div><div class="cards">
    <button class="card" data-new="goal">${icon("sparkle")}<span class="ct">New goal</span><span class="cs">Tell the main agent what to build</span></button>
    <button class="card" data-new="ticket">${icon("ticket")}<span class="ct">File a ticket</span><span class="cs">Triaged, then scheduled by the main agent</span></button>
    <button class="card" data-new="files">${icon("folder")}<span class="ct">Open worktree</span><span class="cs">Browse the project's files</span></button></div>
    <div id="newForm"></div></div>`;
  $("#env-new").querySelectorAll("[data-new]").forEach(b => (b.onclick = () => {
    const k = b.dataset.new;
    if (k === "goal") { openView({ t: "agent", id: MAIN }); setTimeout(() => { const i = $(`.pane[data-slot="${S.focus}"] .tp input`); if (i) i.focus(); }, 30); }
    if (k === "files") openView({ t: "files", id: MAIN });
    if (k === "ticket") {
      $("#newForm").innerHTML = `<form class="cards" id="tf" style="width:min(520px,90vw)"><input id="tt" class="card" style="width:100%;cursor:text" placeholder="Title: what is wrong or wanted" aria-label="Ticket"><button class="btn primary" type="submit">File ticket</button></form>`;
      $("#tt").focus();
      $("#tf").onsubmit = async e => {
        e.preventDefault();
        const v = $("#tt").value.trim(); if (!v) return;
        const [title, body] = v.includes(":") ? v.split(/:(.*)/s) : [v, v];
        await invoke("file_ticket", { title: title.trim(), body: (body || title).trim() });
        S.side = "tickets"; renderSeg(); setEnv("terminals");
      };
    }
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

let queued = false, lastPending = 0;
async function refresh() {
  queued = false;
  snap = await invoke("snapshot");
  renderList(); renderCard(); renderTabs();
  if (S.env === "terminals") {
    const vis = S.max != null ? [] : S.slots.slice(0, S.layout).map(v => v.t + v.id);
    if (new Set(vis).size < vis.length && snap.agents.length > 1) renderGrid(); else refreshPanes();
  }
  else if (S.env === "diagram") renderDiagram();
  // A new review jumps the queue, the way a notification would.
  const n = pending().length;
  if (n > lastPending && S.env !== "diagram") { const p = pending()[pending().length - 1]; S.review = p.id; }
  lastPending = n;
}
// Coalesce bursts of harness events into one refresh per frame.
TAURI.event.listen("state", () => { if (!queued) { queued = true; requestAnimationFrame(refresh); } });
addEventListener("resize", () => { setSide(!narrow()); if (S.env === "diagram") { view = null; renderDiagram(); } });

(async () => {
  const platform = await invoke("platform");
  document.documentElement.classList.add("native", platform);
  snap = await invoke("snapshot");
  applyTheme(); renderSeg(); renderCard(); setSide(!narrow()); setEnv("terminals");
  window.__bootedAt = performance.now();
})();
