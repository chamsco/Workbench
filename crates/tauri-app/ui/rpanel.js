// The side panel on the right of Code (the title bar's panel button): what
// is uncommitted, the project's git history, its files (with a refresh), and
// agent sessions (this workspace's agents, its chats, or every session). It
// pushes the canvases aside rather than covering them.

var RPanel = (() => {
  const get = (k, d) => { try { return localStorage.getItem(k) || d; } catch { return d; } };
  const put = (k, v) => { try { localStorage.setItem(k, v); } catch {} };
  let open = get("bs.rpanel", "0") === "1", tab = get("bs.rtab", "changes"), scope = get("bs.rscope", "workspace");
  let log = { path: "", at: 0, rows: null, err: null }, files = { at: 0, rows: null }, preview = null, q = "", chats = [];
  let changes = { path: "", at: 0, rows: null, branch: "", err: null };
  const OS = { win: "Windows", mac: "macOS", linux: "Linux" };
  const TABS = [["changes", "diff", "Changes"], ["history", "branch", "History"], ["files", "folder", "Files"], ["sessions", "sparkle", "Sessions"]];
  const dir = () => (agent(MAIN) && agent(MAIN).worktree) || snap.workspace || "";
  // Code chat edits the project folder itself; the agents work in their worktree.
  const workDir = () => (S.codeView === "pair" ? snap.workspace : dir()) || "";
  const showing = () => open && S.mode === "code" && !S.settings && hasProject;

  function toggle(v = !open) {
    open = v; put("bs.rpanel", v ? "1" : "0");
    render();
  }
  function show(t) {
    if (t) { tab = t; put("bs.rtab", tab); q = ""; preview = null; }
    toggle(true);
  }

  async function load(force) {
    const d = dir();
    if (tab === "changes") {
      const w = workDir();
      // ponytail: at most one git status every 3s while agents stream events.
      if (!w || (!force && changes.path === w && Date.now() - changes.at < 3000)) return;
      changes.at = Date.now();
      try { const r = await invoke("git_changes", { path: w }); changes = { path: w, at: changes.at, rows: r.files, branch: r.branch, err: null }; } catch (e) { changes = { path: w, at: changes.at, rows: [], branch: "", err: String(e) }; }
      draw();
    }
    if (tab === "history" && d && (force || log.path !== d || Date.now() - log.at > 30_000)) {
      log = { path: d, at: Date.now(), rows: log.path === d ? log.rows : null, err: null };
      try { log.rows = await invoke("git_log", { path: d }); } catch (e) { log.err = String(e); }
      draw();
    }
    if (tab === "files" && (force || !files.rows || Date.now() - files.at > 30_000)) {
      files = { at: Date.now(), rows: await invoke("list_files", { agent: MAIN }).catch(() => []) };
      draw();
    }
    if (tab === "sessions") {
      chats = ((await invoke("chat_list").catch(() => null)) || []).filter(t => !t.app);
      draw();
    }
  }

  function sessions() {
    const agents = treeOrder().map(agent).filter(a => a && a.kind !== "triage").map(a => ({
      ic: a.kind === "scout" ? "search" : "sparkle", t: a.id === MAIN ? "Main agent" : a.title,
      s: `${a.decision ? a.decision.model : "not routed"} · ${a.status.replace(/_/g, " ")}`, dot: dotFor(a.status), go: () => openIn({ kind: "agent", agent: a.id }),
    }));
    if (scope === "workspace") return agents;
    const ts = chats.filter(t => scope === "all" || t.project === snap.workspace).map(t => ({
      ic: "bubble", t: t.title || "Chat", s: `${t.project ? "Code chat" : "Chat"} · ${ago(t.updated)}`, dot: t.busy ? "run" : t.failed ? "fail" : "",
      warn: t.failed, go: () => { if (t.project) { S.codeView = "pair"; setMode("code"); } else setMode("chat"); if (window.Chat && Chat.open) Chat.open(t.id); },
    }));
    return [...agents, ...ts];
  }
  const ago = ms => { const m = Math.round((Date.now() - ms) / 60000); return m < 1 ? "now" : m < 60 ? `${m}m ago` : m < 1440 ? `${Math.round(m / 60)}h ago` : `${Math.round(m / 1440)}d ago`; };
  // git's two letters, as one: Modified, Added, Deleted, Renamed, Untracked.
  const mark = s => (s === "??" ? "U" : s.replace(/[^MADRCU]/g, "")[0] || "M");
  const diffHtml = t => t.split("\n").map(l => `<span class="${/^\+(?!\+\+)/.test(l) ? "add" : /^-(?!--)/.test(l) ? "del" : /^@@/.test(l) ? "hunk" : ""}">${esc(l)}</span>`).join("\n");
  const prevHtml = () => preview ? `<div class="rp-prev"><div class="rp-prev-h">${esc(base(preview.path))}<button class="ib" data-act="unprev" aria-label="Close preview">${icon("close")}</button></div><pre>${preview.diff ? diffHtml(preview.text) : esc(preview.text)}</pre></div>` : "";

  function body() {
    const match = t => !q || t.toLowerCase().includes(q.toLowerCase());
    if (tab === "changes") {
      if (changes.err) return { n: 0, m: 0, h: `<div class="hw-empty">${esc(changes.err)}</div>` };
      if (!changes.rows) return { n: 0, m: 0, h: `<div class="hw-empty">Reading changes…</div>` };
      const rows = changes.rows.filter(c => match(c.path));
      const folder = p => p.slice(0, -base(p).length).replace(/[\\/]$/, "");
      return { n: rows.length, m: changes.rows.length, h: rows.length ? rows.map(c => `<button class="rp-file rp-chg" data-chg="${esc(c.path)}"><i class="cm cm-${mark(c.status)}">${mark(c.status)}</i><span class="nm">${esc(base(c.path))}<small>${esc(folder(c.path))}</small></span>${c.added ? `<b class="add">+${c.added}</b>` : ""}${c.removed ? `<b class="del">−${c.removed}</b>` : ""}</button>`).join("") + prevHtml()
        : `<div class="rp-none">${icon("diff")}<b>No changes yet</b><span>Changes made on ${esc(changes.branch || "this branch")} appear here.</span></div>` };
    }
    if (tab === "history") {
      if (log.err) return { n: 0, m: 0, h: `<div class="hw-empty">${esc(log.err)}</div>` };
      if (!log.rows) return { n: 0, m: 0, h: `<div class="hw-empty">Reading history…</div>` };
      const rows = log.rows.filter(c => match(c.subject + " " + c.author + " " + c.hash));
      return { n: rows.length, m: log.rows.length, h: rows.map(c => `<div class="rp-row"><span class="rp-dot"></span><span class="tt"><b>${esc(c.subject)}</b><span>${esc(c.hash)} · ${esc(c.author)} · ${esc(c.when)}</span>
        ${c.refs ? `<span class="refs">${c.refs.split(", ").filter(r => r && !r.startsWith("HEAD")).slice(0, 3).map(r => `<i>${esc(r.replace("refs/", ""))}</i>`).join("")}</span>` : ""}</span></div>`).join("") || `<div class="hw-empty">No commits match.</div>` };
    }
    if (tab === "files") {
      if (!files.rows) return { n: 0, m: 0, h: `<div class="hw-empty">Reading files…</div>` };
      const rows = files.rows.filter(f => match(f.name));
      return { n: rows.length, m: files.rows.length, h: rows.map(f => `<button class="rp-file" data-path="${esc(f.path)}" data-dir="${f.dir}" style="padding-left:${8 + (q ? 0 : f.depth * 12)}px">${icon(f.dir ? "folder" : "doc")}<span>${esc(f.name)}</span></button>`).join("") + prevHtml() };
    }
    const rows = sessions().filter(x => match(x.t + " " + x.s));
    return { n: rows.length, m: rows.length, h: rows.map((x, i) => `<button class="rp-row" data-sess="${i}">${icon(x.ic)}<span class="tt"><b>${esc(x.t)}${x.warn ? ` <span class="warnmark" title="${esc(x.warn)}">⚠</span>` : ""}</b><span>${esc(x.s)}</span></span>${x.dot ? `<span class="dot ${x.dot}"></span>` : ""}</button>`).join("") || `<div class="hw-empty">No sessions here yet.</div>`, rows };
  }

  function draw() {
    const el = $("#rpanel");
    if (!showing()) return;
    const b = body(), keep = el.querySelector(".rp-list") ? el.querySelector(".rp-list").scrollTop : 0, focused = document.activeElement && document.activeElement.classList.contains("rp-q");
    const m = machine();
    const count = { changes: "changed", files: "files" }[tab] || "recent";
    el.innerHTML = `<div class="rp-tabs" role="tablist">${TABS.map(([k, ic, l]) => `<button role="tab" aria-selected="${tab === k}" data-tab="${k}">${icon(ic)}${l}</button>`).join("")}</div>
      <div class="rp-head"><span>${b.n} shown · ${b.m} ${count}</span><span class="sp"></span>
        ${tab !== "sessions" ? `<button class="ib" data-act="refresh" title="Refresh" aria-label="Refresh">${icon("reload")}</button>` : ""}
        <button class="rp-opt" data-act="opts" data-popper>View options${icon("chevd")}</button></div>
      <div class="rp-sub">${tab === "changes" ? `${icon("branch")}${esc(changes.branch || "…")} · Uncommitted` : `${icon(m.local ? "pc" : "cloud")}${esc(m.name)} · ${OS[(window.__BOOT || {}).platform] || "Web"}${tab === "sessions" ? ` · ${{ workspace: "Workspace", project: "Project", all: "All" }[scope]}` : ""}`}</div>
      <label class="rp-search">${icon("search")}<input class="rp-q" placeholder="Filter" value="${esc(q)}" aria-label="Filter"></label>
      <div class="rp-list">${b.h}</div>`;
    el.querySelector(".rp-list").scrollTop = keep;
    const inp = el.querySelector(".rp-q");
    inp.oninput = () => { q = inp.value; draw(); };
    if (focused) { inp.focus(); inp.setSelectionRange(q.length, q.length); }
    $$("[data-tab]", el).forEach(x => (x.onclick = () => { tab = x.dataset.tab; put("bs.rtab", tab); q = ""; preview = null; draw(); load(); }));
    const rf = el.querySelector("[data-act=refresh]"); if (rf) rf.onclick = () => load(true);
    el.querySelector("[data-act=opts]").onclick = e => openPop(e.currentTarget, `<div class="ph-t">Show sessions from</div>${[["workspace", "Workspace", "This project's agents"], ["project", "Project", "Its agents and chats"], ["all", "All", "Every agent and chat"]].map(([k, l, d]) => `<button class="pi${scope === k ? " on" : ""}" data-scope="${k}"><span class="vt"><b>${l}</b><small>${d}</small></span></button>`).join("")}`, pop => {
      $$("[data-scope]", pop).forEach(x => (x.onclick = () => { closePop(); scope = x.dataset.scope; put("bs.rscope", scope); tab = "sessions"; put("bs.rtab", tab); draw(); load(); }));
    });
    $$("[data-path]", el).forEach(x => (x.onclick = async () => {
      if (x.dataset.dir === "true") return;
      preview = { path: x.dataset.path, text: await invoke("read_file", { path: x.dataset.path }).catch(e => String(e)) };
      draw();
    }));
    $$("[data-chg]", el).forEach(x => (x.onclick = async () => {
      preview = { path: x.dataset.chg, diff: true, text: await invoke("git_diff", { path: changes.path, file: x.dataset.chg }).catch(e => String(e)) };
      draw();
    }));
    const up = el.querySelector("[data-act=unprev]"); if (up) up.onclick = () => { preview = null; draw(); };
    if (b.rows) $$("[data-sess]", el).forEach(x => (x.onclick = () => b.rows[+x.dataset.sess].go()));
  }

  function render() {
    const el = $("#rpanel"), on = showing();
    el.hidden = !on;
    $("#win").classList.toggle("rpanel-on", on);
    $("#panelBtn").setAttribute("aria-pressed", on);
    if (on) { draw(); load(); }
  }

  $("#panelBtn").onclick = () => toggle();
  // Changes follow every state change (an agent or a chat edits files); the rest on demand.
  return { render, toggle, show, refresh: () => { if (showing()) { draw(); if (tab === "sessions" || tab === "changes") load(); } } };
})();
