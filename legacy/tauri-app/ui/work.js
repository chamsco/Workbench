// The ADE's work area: a split beside the agent chat, with what the session is
// working on. It opens on a New tab page (after ZCode's) that lists what it
// can show; each one opened becomes a tab you can close. Files, Review
// (uncommitted changes and the history), Browser, Terminal, Images and Logs
// (the traces of the project's ADE replies). Every
// tab reads real state: the project folder, git, a real shell, the traces the
// replies left. Nothing is drawn that the machine did not report.
//
// The IDE keeps its own side panel (rpanel.js); in the ADE the header's panel
// button opens this instead, and the terminal button opens its Terminal tab.

var Work = (() => {
  const get = (k, d) => { try { return localStorage.getItem(k) || d; } catch { return d; } };
  const put = (k, v) => { try { localStorage.setItem(k, v); } catch {} };
  const TABS = [["files", "folder", "Files"], ["browser", "globe", "Browser"], ["terminal", "term", "Terminal"], ["diffs", "diff", "Review"], ["images", "image", "Images"], ["logs", "trace", "Logs"]];
  // What each one is, on the New tab page.
  const ABOUT = { files: "Browse the project folder and read a file.", browser: "Browse websites and preview apps.", terminal: "Run commands and view output.", diffs: "Review code changes and diffs.", images: "See the images in the project.", logs: "Read each reply's trace: model, tools, timings." };
  const IMG = /\.(png|jpe?g|gif|webp|svg|bmp|ico|avif)$/i;
  const known = k => TABS.some(([t]) => t === k);
  // Open by default: the New tab page is where the ADE starts.
  let open = get("bs.work", "1") === "1", tab = get("bs.wtab", "new");
  // The tabs opened from the New tab page, in the order they were opened.
  let opened = get("bs.wopen", "").split(",").filter(known);
  if (tab !== "new" && !opened.includes(tab)) tab = opened[0] || "new";
  // Per tab: what was read, and when. Kept across tab switches so coming back
  // is instant; refreshed on demand and as the session changes things.
  const st = {
    files: { rows: null, at: 0, sel: null, text: null, q: "" },
    diffs: { mode: "changes", path: "", rows: null, branch: "", fresh: false, err: null, at: 0, sel: null, text: null, log: null },
    images: { rows: null, at: 0, sel: null, urls: {} },
    logs: { threads: null, tid: null, thread: null, ps: {} },
    browser: { p: { kind: "browser", url: get("bs.wurl", "") || null }, ps: {} },
    terminal: { ps: {}, mounted: false },
  };
  const ws = () => (hasProject && snap.workspace) || "";
  const showing = () => open && S.mode === "code" && S.codeView === "pair" && !S.settings && !!ws();

  function toggle(v = !open) { open = v; put("bs.work", v ? "1" : "0"); render(); }
  function show(t) { if (t) setTab(t); toggle(true); }
  function setTab(t) {
    if (t !== "new" && !known(t)) return;
    if (t !== "new" && !opened.includes(t)) { opened.push(t); put("bs.wopen", opened.join(",")); }
    tab = t; put("bs.wtab", t);
    if (open) draw();
  }
  function closeTab(t) {
    opened = opened.filter(k => k !== t); put("bs.wopen", opened.join(","));
    // A closed terminal ends its shell; a closed browser forgets its frame.
    if (t === "terminal") { if (st.terminal.ps.cleanup) st.terminal.ps.cleanup(); st.terminal = { ps: {}, mounted: false }; const l = $('#work [data-live="terminal"]'); if (l) l.innerHTML = ""; }
    if (t === "browser") { const l = $('#work [data-live="browser"]'); if (l) l.innerHTML = ""; }
    if (tab === t) { tab = opened[opened.length - 1] || "new"; put("bs.wtab", tab); }
    draw(); load();
  }

  // ---------------------------------------------------------------- data
  async function load(force) {
    const w = ws(); if (!w) return;
    if (tab === "files" || tab === "images") {
      const s = st[tab === "files" ? "files" : "images"];
      if (!force && s.rows && Date.now() - s.at < 30_000) return;
      const rows = await invoke("list_files", { agent: MAIN }).catch(() => []);
      st.files.rows = rows; st.files.at = Date.now();
      st.images.rows = rows.filter(f => !f.dir && IMG.test(f.name)); st.images.at = Date.now();
      draw();
    }
    if (tab === "diffs") {
      const d = st.diffs;
      if (d.mode === "history") {
        if (!force && d.log && d.path === w) return;
        d.path = w;
        try { d.log = await invoke("git_log", { path: w }); } catch (e) { d.log = { err: String(e) }; }
        draw(); return;
      }
      // At most one git status every 3s while a reply streams events.
      if (!force && d.path === w && Date.now() - d.at < 3000) return;
      d.at = Date.now(); d.path = w;
      try { const r = await invoke("git_changes", { path: w }); Object.assign(d, { rows: r.files || [], branch: r.branch || "", fresh: !!r.fresh, err: null }); }
      catch (e) { Object.assign(d, { rows: [], err: String(e) }); }
      if (d.sel && !d.rows.some(r => r.path === d.sel)) { d.sel = null; d.text = null; }
      draw();
    }
    if (tab === "logs") {
      const l = st.logs;
      // The project's ADE threads, newest first.
      l.threads = ((await invoke("chat_list").catch(() => null)) || []).filter(t => t.project === w && !t.app).sort((a, b) => b.updated - a.updated);
      if (!l.tid || !l.threads.some(t => t.id === l.tid)) l.tid = l.threads.length ? l.threads[0].id : null;
      l.thread = l.tid ? await invoke("chat_thread", { id: l.tid }).catch(() => null) : null;
      draw();
    }
  }

  // ---------------------------------------------------------------- views
  const ago = ms => { const m = Math.round((Date.now() - ms) / 60000); return m < 1 ? "now" : m < 60 ? `${m}m ago` : m < 1440 ? `${Math.round(m / 60)}h ago` : `${Math.round(m / 1440)}d ago`; };
  const mark = s => (s === "??" ? "U" : String(s || "").replace(/[^MADRCU]/g, "")[0] || "M");
  const diffHtml = t => t.split("\n").map(l => `<span class="${/^\+(?!\+\+)/.test(l) ? "add" : /^-(?!--)/.test(l) ? "del" : /^@@/.test(l) ? "hunk" : ""}">${esc(l)}</span>`).join("\n");
  const none = (ic, b, s) => `<div class="wk-none">${icon(ic)}<b>${esc(b)}</b><span>${s}</span></div>`;
  const rel = p => { const w = ws(); return w && p.startsWith(w) ? p.slice(w.length).replace(/^[\\/]/, "") : p; };

  function filesView() {
    const s = st.files;
    if (!s.rows) return `<div class="wk-wait">Reading files…</div>`;
    const q = s.q.toLowerCase();
    const rows = s.rows.filter(f => !q || f.name.toLowerCase().includes(q));
    return `<div class="wk-split"><div class="wk-list">
        <label class="rp-search">${icon("search")}<input class="wk-q" placeholder="Filter files" value="${esc(s.q)}" aria-label="Filter files"></label>
        ${rows.map(f => `<button class="rp-file${s.sel === f.path ? " sel" : ""}" data-file="${esc(f.path)}" data-dir="${f.dir}" style="padding-left:${8 + (q ? 0 : f.depth * 12)}px">${icon(f.dir ? "folder" : IMG.test(f.name) ? "image" : "doc")}<span>${esc(f.name)}</span></button>`).join("") || `<div class="wk-wait">No files match.</div>`}
      </div><div class="wk-view">${s.sel ? `<div class="wk-vh">${esc(rel(s.sel))}</div><pre class="wk-pre">${s.text == null ? "Reading…" : esc(s.text)}</pre>` : none("doc", "Pick a file", "Its contents show here, read from the project folder.")}</div></div>`;
  }

  function diffsView() {
    const d = st.diffs;
    const seg = `<div class="wk-seg" role="group" aria-label="Diffs"><button data-dm="changes" aria-pressed="${d.mode === "changes"}">Uncommitted</button><button data-dm="history" aria-pressed="${d.mode === "history"}">History</button></div>`;
    if (d.mode === "history") {
      if (!d.log) return seg + `<div class="wk-wait">Reading history…</div>`;
      if (d.log.err) return seg + `<div class="wk-wait">${esc(d.log.err)}</div>`;
      return seg + `<div class="wk-list wk-full">${d.log.map(c => `<div class="rp-row"><span class="rp-dot"></span><span class="tt"><b>${esc(c.subject)}</b><span>${esc(c.hash)} · ${esc(c.author)} · ${esc(c.when)}</span></span></div>`).join("") || `<div class="wk-wait">No commits yet.</div>`}</div>`;
    }
    if (d.err) return seg + `<div class="wk-wait">${esc(d.err)}</div>`;
    if (!d.rows) return seg + `<div class="wk-wait">Reading changes…</div>`;
    if (!d.rows.length) return seg + none("diff", "No changes yet", `What the session changes on ${esc(d.branch || "this branch")} shows here, file by file.`);
    const add = d.rows.reduce((n, r) => n + (r.added || 0), 0), del = d.rows.reduce((n, r) => n + (r.removed || 0), 0);
    return seg + `<div class="wk-sum">${icon("branch")}${esc(d.branch || "…")} · ${d.rows.length} file${d.rows.length === 1 ? "" : "s"} <b class="add">+${add}</b> <b class="del">−${del}</b>${d.fresh ? " · no commits yet" : ""}</div>
      <div class="wk-split"><div class="wk-list">${d.rows.map(c => `<button class="rp-file rp-chg${d.sel === c.path ? " sel" : ""}" data-chg="${esc(c.path)}"><i class="cm cm-${mark(c.status)}">${mark(c.status)}</i><span class="nm">${esc(c.path)}</span>${c.added ? `<b class="add">+${c.added}</b>` : ""}${c.removed ? `<b class="del">−${c.removed}</b>` : ""}</button>`).join("")}</div>
      <div class="wk-view">${d.sel ? `<div class="wk-vh">${esc(d.sel)}</div><pre class="wk-pre wk-diff">${d.text == null ? "Reading…" : diffHtml(d.text)}</pre>` : none("diff", "Pick a file", "Its diff against the last commit shows here.")}</div></div>`;
  }

  function imagesView() {
    const s = st.images;
    if (!s.rows) return `<div class="wk-wait">Looking for images…</div>`;
    if (!s.rows.length) return none("image", "No images in this project", "Screenshots and images the session writes into the project show here.");
    if (s.sel) return `<div class="wk-vh"><button class="ib" data-act="imgback" aria-label="All images">${icon("back")}</button>${esc(rel(s.sel))}</div><div class="wk-big"><img data-src="${esc(s.sel)}" alt="${esc(base(s.sel))}"></div>`;
    return `<div class="wk-grid">${s.rows.slice(0, 120).map(f => `<button class="wk-thumb" data-img="${esc(f.path)}" title="${esc(rel(f.path))}"><img data-src="${esc(f.path)}" alt="" loading="lazy"><span>${esc(f.name)}</span></button>`).join("")}</div>${s.rows.length > 120 ? `<div class="wk-wait">Showing 120 of ${s.rows.length}.</div>` : ""}`;
  }

  function newTabView() {
    return `<div class="wk-new"><h3>New tab</h3>${TABS.map(([k, ic, l]) => `<div class="wk-card"><span class="wk-ci">${icon(ic)}</span><span class="wk-ct"><b>${l}</b><span>${ABOUT[k]}</span></span><button class="btn sm" data-open="${k}">${opened.includes(k) ? "Show" : "Open"}</button></div>`).join("")}</div>`;
  }

  function logsView() {
    const l = st.logs;
    if (!l.threads) return `<div class="wk-wait">Reading the project's threads…</div>`;
    if (!l.threads.length) return none("trace", "No ADE threads in this project yet", "Each reply leaves a trace: the model, its tool calls, how long each took. Start a thread and they show here.");
    const pick = `<div class="wk-bar"><select class="tx wk-thread" aria-label="Thread">${l.threads.map(t => `<option value="${esc(t.id)}"${t.id === l.tid ? " selected" : ""}>${esc(t.title || "Thread")} · ${ago(t.updated)}</option>`).join("")}</select></div>`;
    return pick + `<div class="wk-trace"></div>`;
  }

  // ---------------------------------------------------------------- frame
  function draw() {
    const el = $("#work");
    if (!el || !showing()) return;
    const keep = el.querySelector(".wk-list") ? el.querySelector(".wk-list").scrollTop : 0;
    const focused = document.activeElement && document.activeElement.classList.contains("wk-q");
    // Browser and Terminal keep their DOM (an iframe, a running shell), so the
    // body is rebuilt only for the tabs that are plain reads.
    let body = el.querySelector(".wk-body"), live = el.querySelector(".wk-live");
    if (!body) {
      el.innerHTML = `<div class="wk-gut" role="separator" aria-orientation="vertical" aria-label="Resize work area"></div>
        <div class="wk-tabs" role="tablist" aria-label="Work area"></div><div class="wk-body"></div>
        <div class="wk-live" data-live="browser" hidden></div><div class="wk-live" data-live="terminal" hidden></div>`;
      body = el.querySelector(".wk-body");
      wireGutter(el.querySelector(".wk-gut"));
    }
    const lab = k => TABS.find(([t]) => t === k);
    const tabs = opened.map(k => { const [, ic, l] = lab(k); return `<span class="wk-tab" role="tab" aria-selected="${tab === k}" data-wt="${k}">${icon(ic)}<span>${l}</span>${k === "diffs" && st.diffs.rows && st.diffs.rows.length ? `<i class="wk-n">${st.diffs.rows.length}</i>` : ""}<button class="wk-x" data-wx="${k}" aria-label="Close ${l}" title="Close ${l}">${icon("close")}</button></span>`; });
    if (tab === "new") tabs.push(`<span class="wk-tab" role="tab" aria-selected="true" data-wt="new">${icon("doc")}<span>New tab</span>${opened.length ? `<button class="wk-x" data-wx="new" aria-label="Close New tab" title="Close New tab">${icon("close")}</button>` : ""}</span>`);
    el.querySelector(".wk-tabs").innerHTML = `<div class="wk-strip">${tabs.join("")}<button class="ib" data-act="new" title="New tab" aria-label="New tab">${icon("plus")}</button></div>`
      + `<span class="sp"></span>${["files", "diffs", "images", "logs"].includes(tab) ? `<button class="ib" data-act="refresh" title="Refresh" aria-label="Refresh">${icon("reload")}</button>` : ""}<button class="ib" data-act="close" title="Close the work area" aria-label="Close the work area">${icon("close")}</button>`;
    $$("[data-wt]", el).forEach(b => (b.onclick = e => { if (e.target.closest("[data-wx]")) return; setTab(b.dataset.wt); load(); }));
    $$("[data-wx]", el).forEach(b => (b.onclick = () => (b.dataset.wx === "new" ? setTab(opened[opened.length - 1]) : closeTab(b.dataset.wx))));
    el.querySelector("[data-act=new]").onclick = () => setTab("new");
    el.querySelector("[data-act=close]").onclick = () => toggle(false);
    const rf = el.querySelector("[data-act=refresh]"); if (rf) rf.onclick = () => load(true);

    const isLive = tab === "browser" || tab === "terminal";
    body.hidden = isLive;
    $$(".wk-live", el).forEach(x => (x.hidden = x.dataset.live !== tab));
    if (tab === "browser") {
      live = el.querySelector('[data-live="browser"]');
      if (!live.childElementCount && window.Code && Code.browserPane) Code.browserPane(st.browser.p, live, st.browser.ps);
      return;
    }
    if (tab === "terminal") {
      live = el.querySelector('[data-live="terminal"]');
      if (!st.terminal.mounted && window.Code && Code.terminalPane) { st.terminal.mounted = true; Code.terminalPane(live, st.terminal.ps); }
      return;
    }
    body.innerHTML = tab === "new" ? newTabView() : tab === "files" ? filesView() : tab === "diffs" ? diffsView() : tab === "images" ? imagesView() : logsView();
    const list = body.querySelector(".wk-list"); if (list) list.scrollTop = keep;
    wire(body);
    if (focused) { const q = body.querySelector(".wk-q"); if (q) { q.focus(); q.setSelectionRange(q.value.length, q.value.length); } }
  }

  function wire(body) {
    $$("[data-open]", body).forEach(b => (b.onclick = () => { setTab(b.dataset.open); load(); }));
    const q = body.querySelector(".wk-q");
    if (q) q.oninput = () => { st.files.q = q.value; draw(); };
    $$("[data-file]", body).forEach(b => (b.onclick = async () => {
      if (b.dataset.dir === "true") return;
      const s = st.files, p = b.dataset.file;
      s.sel = p; s.text = null; draw();
      const t = await invoke("read_file", { path: p }).catch(e => String(e));
      if (s.sel === p) { s.text = t; draw(); }
    }));
    $$("[data-dm]", body).forEach(b => (b.onclick = () => { st.diffs.mode = b.dataset.dm; draw(); load(); }));
    $$("[data-chg]", body).forEach(b => (b.onclick = async () => {
      const d = st.diffs, f = b.dataset.chg;
      d.sel = f; d.text = null; draw();
      const t = await invoke("git_diff", { path: d.path, file: f }).catch(e => String(e));
      if (d.sel === f) { d.text = t; draw(); }
    }));
    $$("[data-img]", body).forEach(b => (b.onclick = () => { st.images.sel = b.dataset.img; draw(); }));
    const ib = body.querySelector("[data-act=imgback]"); if (ib) ib.onclick = () => { st.images.sel = null; draw(); };
    $$("img[data-src]", body).forEach(async im => {
      const p = im.dataset.src, u = st.images.urls;
      try { im.src = u[p] || (u[p] = await invoke("read_image", { path: p })); }
      catch (e) { im.replaceWith(Object.assign(document.createElement("span"), { className: "wk-wait", textContent: "can't show " + base(p) })); }
    });
    const sel = body.querySelector(".wk-thread");
    if (sel) sel.onchange = () => { st.logs.tid = sel.value; st.logs.ps = {}; load(); };
    const tr = body.querySelector(".wk-trace");
    if (tr) {
      const msgs = (st.logs.thread && st.logs.thread.messages) || [];
      const ids = msgs.map(m => m.trace).filter(Boolean);
      if (!ids.length) tr.innerHTML = `<div class="wk-wait">No traces on this thread yet. Each reply saves one when it ends.</div>`;
      else if (window.Code && Code.traceList) Code.traceList(tr, ids, st.logs.ps, "Reply");
    }
  }

  // Drag the left edge to resize; the width is remembered.
  function wireGutter(g) {
    g.onpointerdown = e => {
      e.preventDefault();
      const card = $("#win > .mainc").getBoundingClientRect();
      const move = ev => { const w = Math.round(Math.min(card.width - 320, Math.max(300, card.right - ev.clientX - 8))); $("#win").style.setProperty("--work-w", w + "px"); };
      const up = () => { removeEventListener("pointermove", move); removeEventListener("pointerup", up); $("#win").classList.remove("work-drag"); put("bs.work-w", getComputedStyle($("#win")).getPropertyValue("--work-w").trim()); };
      $("#win").classList.add("work-drag");
      addEventListener("pointermove", move); addEventListener("pointerup", up);
    };
  }

  function render() {
    const el = $("#work"); if (!el) return;
    const on = showing();
    el.hidden = !on;
    $("#win").classList.toggle("work-on", on);
    if (S.mode === "code" && S.codeView === "pair") $("#panelBtn").setAttribute("aria-pressed", String(on));
    if (on) { draw(); load(); }
  }
  // A new project means new files, changes and threads: forget what was read.
  let lastWs = null;
  function refresh() {
    if (ws() !== lastWs) {
      lastWs = ws();
      st.files.rows = st.images.rows = null; st.files.sel = st.images.sel = null;
      Object.assign(st.diffs, { rows: null, log: null, sel: null, text: null, path: "" });
      Object.assign(st.logs, { threads: null, tid: null, thread: null, ps: {} });
      if (st.terminal.ps.cleanup) st.terminal.ps.cleanup();
      st.terminal = { ps: {}, mounted: false };
      const el = $("#work"); if (el) el.innerHTML = "";
    }
    if (showing()) { draw(); if (tab === "diffs" || tab === "logs") load(); }
  }

  const w0 = get("bs.work-w", ""); if (w0) $("#win").style.setProperty("--work-w", w0);
  return { render, toggle, show, refresh, showing, setTab };
})();
