// Ask: the chat beside the page. A column on the right of the window holds one
// thread and its composer at the foot; the sparkle in the page header (or ⌘I)
// folds it away and back. It is the window's, not an app's: whatever page is
// open, the column stays the same thread.
//
// Typing shows the window's own moves first (open an app, a project, a chat:
// the same list as search) so a manual interaction is one Enter away. Anything
// else is a question: it goes to the default model, scoped to the project when
// Code has one open, and the reply streams into the column. The thread lives
// in Chat like any other; "Open in Chat" carries it on there, and New starts
// another.
var Ask = (() => {
  const KEY = "bs.ask", WKEY = "bs.chatcol";
  let el = null, thread = null, msgs = [], poll = null, sel = 0, hits = [], chats = [];
  const store = {
    get: k => { try { return localStorage.getItem(k); } catch { return null; } },
    set: (k, v) => { try { v == null ? localStorage.removeItem(k) : localStorage.setItem(k, v); } catch {} },
  };

  function mount() {
    el = $("#chatcol");
    el.innerHTML = `<div class="cc-top" data-tauri-drag-region>
        <span class="cc-route" data-tauri-drag-region></span><span class="sp" data-tauri-drag-region></span>
        <button class="ib" data-cc="open" title="Open in Chat" aria-label="Open in Chat" hidden>${icon("ext")}</button>
        <button class="ib" data-cc="new" title="New chat" aria-label="New chat">${icon("compose")}</button>
      </div>
      <div class="cc-out" role="log" aria-live="polite"></div>
      <div class="cc-hits" role="listbox" hidden></div>
      <div class="cc-box">
        <textarea rows="2" placeholder="Ask anything, or change this page…" aria-label="Ask"></textarea>
        <div class="cc-bar">
          <button class="ib" data-cc="moves" title="Go to an app, project or chat" aria-label="Go to">${icon("plus")}</button>
          <span class="cc-model"></span><span class="sp"></span>
          <button class="cc-send" data-cc="send" aria-label="Send">${icon("up")}</button>
        </div>
      </div>`;
    const inp = $("textarea", el);
    inp.oninput = () => { sel = 0; draw(); };
    inp.onkeydown = e => {
      if (e.key === "Escape") { e.preventDefault(); inp.value = ""; draw(); }
      else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && hits.length) { e.preventDefault(); sel = (sel + (e.key === "ArrowDown" ? 1 : -1) + hits.length) % hits.length; draw(); }
      else if (e.key === "Enter" && !e.shiftKey && !e.isComposing) { e.preventDefault(); go(); }
    };
    $('[data-cc="send"]', el).onclick = go;
    $('[data-cc="moves"]', el).onclick = () => Shell.search();
    $('[data-cc="new"]', el).onclick = () => { thread = null; msgs = []; store.set(KEY, null); paint(); inp.focus(); };
    $('[data-cc="open"]', el).onclick = () => { if (!thread) return; setMode("chat"); if (window.Chat) Chat.open(thread); };
    wireGutter();
  }

  // The column's width: drag its left edge; double-click puts it back.
  function wireGutter() {
    const gut = $("#chatGut"), w = $("#win");
    const saved = +store.get(WKEY + ".w");
    if (saved) w.style.setProperty("--chat-w", saved + "px");
    gut.onpointerdown = e => {
      e.preventDefault(); gut.setPointerCapture(e.pointerId);
      w.classList.add("dragging-chat");
      gut.onpointermove = ev => w.style.setProperty("--chat-w", Math.round(Math.min(560, Math.max(280, innerWidth - ev.clientX))) + "px");
      gut.onpointerup = () => {
        gut.onpointermove = gut.onpointerup = null; w.classList.remove("dragging-chat");
        store.set(WKEY + ".w", parseInt(getComputedStyle(w).getPropertyValue("--chat-w")) || null);
      };
    };
    gut.ondblclick = () => { w.style.removeProperty("--chat-w"); store.set(WKEY + ".w", null); };
  }

  // The moves: apps first, then projects and chats, matched on what is typed.
  function moves(q) {
    if (!q) return [];
    const zones = (window.Frame ? Frame.zones() : []).map(z => [z.id, z.icon, z.name]).concat([["home", "home", "Home"], ["apps", "apps", "Apps"], ["memory", "memory", "Memory"]]);
    const all = [
      ...zones.map(([k, ic, l]) => ({ ic, t: l, s: "Open", go: () => setMode(k) })),
      ...(prefs.projects || []).map(p => ({ ic: "folder", t: base(p), s: "Project", go: () => openProject(p) })),
      ...chats.map(t => ({ ic: "bubble", t: t.title || "Chat", s: "Chat", go: () => { setMode("chat"); Chat.open(t.id); } })),
    ];
    return all.filter(x => x.t.toLowerCase().includes(q)).slice(0, 4);
  }

  function draw() {
    const q = $("textarea", el).value.trim().toLowerCase();
    hits = q.split(/\s+/).length <= 3 ? moves(q) : [];
    const box = $(".cc-hits", el);
    box.hidden = !hits.length;
    box.innerHTML = hits.map((x, i) => `<button class="pi${i === sel ? " on" : ""}" data-i="${i}">${icon(x.ic)}<span class="st-t">${esc(x.t)}</span><span class="st-s">${esc(x.s)}</span></button>`).join("")
      + (hits.length ? `<div class="ask-tip">Enter to go · type more to ask instead</div>` : "");
    $$("[data-i]", box).forEach(b => (b.onclick = () => { const h = hits[+b.dataset.i]; clear(); h.go(); }));
  }
  function clear() { $("textarea", el).value = ""; hits = []; draw(); }

  async function go() {
    const text = $("textarea", el).value.trim();
    if (!text) return;
    // A move only wins while the text still reads like its name; a sentence asks.
    if (hits.length) { const h = hits[sel]; clear(); h.go(); return; }
    const route = window.Chat && Chat.defaultRoute();
    if (!route) { note("Connect a model first: Settings → Agent providers."); return; }
    clear();
    try {
      if (!thread) {
        const project = S.mode === "code" && snap && snap.workspace ? snap.workspace : null;
        thread = (await invoke("chat_new", { route, scope: { project } })).id;
        store.set(KEY, thread);
      }
      msgs = msgs.concat([{ role: "user", text, at: Date.now() }, { role: "assistant", text: "", status: "sending" }]);
      paint();
      await invoke("chat_send", { id: thread, text, files: [] });
      watch();
    } catch (e) { note(String(e)); }
  }

  // Read the thread until its last answer settles.
  async function load() {
    const t = thread && await invoke("chat_thread", { id: thread }).catch(() => null);
    if (thread && !t) { thread = null; store.set(KEY, null); }
    msgs = (t && t.messages) || [];
    paint();
    return msgs[msgs.length - 1];
  }
  function watch() {
    clearInterval(poll);
    poll = setInterval(async () => {
      const a = await load();
      if (!a || (a.role === "assistant" && a.status !== "streaming" && a.status !== "sending")) clearInterval(poll);
    }, 500);
  }

  function paint() {
    if (!el) return;
    const out = $(".cc-out", el);
    const dots = `<span class="ask-dots"><i></i><i></i><i></i></span>`;
    out.innerHTML = msgs.length ? msgs.map(m => m.role === "user"
      ? `<div class="cc-q">${esc(m.text)}</div>`
      : m.status === "error" ? `<div class="cc-a err">${esc(m.error || "The reply failed.")}</div>`
      : `${m.status === "done" ? `<div class="cc-done">${icon("check")}Worked${m.model ? `<span>${esc(m.model)}</span>` : ""}</div>` : ""}<div class="cc-a">${m.text ? (window.Chat ? Chat.md(m.text) : esc(m.text)) : dots}</div>`).join("")
      : `<div class="cc-empty">Ask about this page, or anything else. The thread is kept in Chat.</div>`;
    out.scrollTop = out.scrollHeight;
    $('[data-cc="open"]', el).hidden = !thread;
    const r = window.Chat && Chat.defaultRoute();
    $(".cc-model", el).textContent = r ? Chat.routeName(r) : "No model connected";
    $(".cc-route", el).textContent = "";
    if (window.Shell) Shell.renderLine();
  }
  function note(t) { msgs = msgs.filter(m => m.status !== "sending"); paint(); $(".cc-out", el).insertAdjacentHTML("beforeend", `<div class="cc-a err">${esc(t)}</div>`); }

  // The last thing you asked, for the page header's middle line.
  function last() {
    const q = [...msgs].reverse().find(m => m.role === "user");
    return q ? { text: q.text, at: q.at } : null;
  }

  function setOpen(on) {
    if (!el) mount();
    $("#win").classList.toggle("chat-off", !on);
    $("#aiBtn").setAttribute("aria-pressed", String(on));
    store.set(WKEY, on ? "on" : null);
    if (on) {
      invoke("chat_list").then(l => { chats = (l || []).filter(t => !t.app && !t.agent); }).catch(() => {});
      requestAnimationFrame(() => $("textarea", el).focus());
    }
    if (window.Code && S.mode === "code") Code.layout();
  }
  const isOpen = () => !$("#win").classList.contains("chat-off");
  const toggle = () => setOpen(!isOpen());
  const open = () => setOpen(true);
  const close = () => setOpen(false);

  // At boot: the column as you left it, and the thread it held.
  async function init() {
    if (!el) mount();
    thread = store.get(KEY);
    // Left alone, it starts folded: the app opens on the sidebar, and the
    // column is one click away.
    const was = store.get(WKEY);
    $("#win").classList.toggle("chat-off", was !== "on");
    $("#aiBtn").setAttribute("aria-pressed", String(isOpen()));
    paint();
    if (thread) { const a = await load(); if (a && a.role === "assistant" && (a.status === "streaming" || a.status === "sending")) watch(); }
  }

  return { init, open, close, toggle, last };
})();
