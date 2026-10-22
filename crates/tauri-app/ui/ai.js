// Ask: the AI shortcut. The sparkle at the top right (or ⌘I) floats a chatbox
// over the bottom fifth of the window, the frame blurred behind it. It is the
// window's, not an app's: it never takes a side of the frame, and it closes
// with Escape or a click on the blur.
//
// Typing shows the window's own moves first (open an app, a project, a chat:
// the same list as search) so a manual interaction is one Enter away. Anything
// else is a question: it goes to the default model in a thread of its own,
// scoped to the project when Workbench has one open, and the reply streams in
// above the box. "Open in Chat" carries the thread on in the Chat app.
var Ask = (() => {
  let el = null, thread = null, poll = null, sel = 0, hits = [], chats = [];

  function mount() {
    el = document.createElement("div");
    el.className = "ask"; el.id = "ask"; el.hidden = true;
    el.innerHTML = `<div class="ask-blur"></div>
      <div class="ask-box" role="dialog" aria-label="Ask AI">
        <div class="ask-out" hidden></div>
        <div class="ask-hits" role="listbox" hidden></div>
        <label class="ask-in">${icon("sparkle")}<input placeholder="Ask, or go to an app, project or chat…" aria-label="Ask AI" autocomplete="off"><kbd>Esc</kbd></label>
        <div class="ask-foot"><span class="ask-route"></span><span class="sp"></span><button class="ask-open" hidden>${icon("bubble")}Open in Chat</button></div>
      </div>`;
    $("#win").append(el);
    $(".ask-blur", el).onclick = close;
    const inp = $("input", el);
    inp.oninput = () => { sel = 0; draw(); };
    inp.onkeydown = e => {
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(); }
      else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && hits.length) { e.preventDefault(); sel = (sel + (e.key === "ArrowDown" ? 1 : -1) + hits.length) % hits.length; draw(); }
      else if (e.key === "Enter" && !e.isComposing) { e.preventDefault(); go(); }
    };
    $(".ask-open", el).onclick = () => { const id = thread; close(); setMode("chat"); if (window.Chat && id) Chat.open(id); };
  }

  // The moves: apps first, then projects and chats, matched on what is typed.
  function moves(q) {
    if (!q) return [];
    const zones = (window.Frame ? Frame.zones() : []).map(z => [z.id, z.icon, z.name]).concat([["apps", "apps", "Apps"], ["memory", "memory", "Memory"]]);
    const all = [
      ...zones.map(([k, ic, l]) => ({ ic, t: l, s: "Open", go: () => setMode(k) })),
      ...(prefs.projects || []).map(p => ({ ic: "folder", t: base(p), s: "Project", go: () => openProject(p) })),
      ...chats.map(t => ({ ic: "bubble", t: t.title || "Chat", s: "Chat", go: () => { setMode("chat"); Chat.open(t.id); } })),
    ];
    return all.filter(x => x.t.toLowerCase().includes(q)).slice(0, 4);
  }

  function draw() {
    const q = $("input", el).value.trim().toLowerCase();
    hits = moves(q);
    const box = $(".ask-hits", el);
    box.hidden = !hits.length;
    box.innerHTML = hits.map((x, i) => `<button class="pi${i === sel ? " on" : ""}" data-i="${i}">${icon(x.ic)}<span class="st-t">${esc(x.t)}</span><span class="st-s">${esc(x.s)}</span></button>`).join("")
      + (hits.length ? `<div class="ask-tip">Enter to go · type more to ask instead</div>` : "");
    $$("[data-i]", box).forEach(b => (b.onclick = () => { const h = hits[+b.dataset.i]; close(); h.go(); }));
  }

  async function go() {
    const inp = $("input", el), text = inp.value.trim();
    if (!text) return;
    // A move only wins while the text still reads like its name; a sentence asks.
    if (hits.length && text.split(/\s+/).length <= 3) { const h = hits[sel]; close(); h.go(); return; }
    const route = window.Chat && Chat.defaultRoute();
    if (!route) { show(`<p>Connect a model first: Settings → Agent providers.</p>`); return; }
    inp.value = ""; hits = []; draw();
    try {
      if (!thread) {
        const project = S.mode === "code" && snap && snap.workspace ? snap.workspace : null;
        thread = (await invoke("chat_new", { route, scope: { project } })).id;
      }
      show(`<div class="ask-q">${esc(text)}</div><div class="ask-a"><span class="ask-dots"><i></i><i></i><i></i></span></div>`);
      await invoke("chat_send", { id: thread, text, files: [] });
      watch();
    } catch (e) { show(`<div class="ask-a err">${esc(String(e))}</div>`); }
  }

  // The reply: read the thread until its last answer settles.
  function watch() {
    clearInterval(poll);
    poll = setInterval(async () => {
      const t = await invoke("chat_thread", { id: thread }).catch(() => null);
      if (!t || el.hidden) { clearInterval(poll); return; }
      const msgs = t.messages || [], a = msgs[msgs.length - 1], q = [...msgs].reverse().find(m => m.role === "user");
      if (!a || a.role !== "assistant") return;
      const body = a.status === "error" ? `<div class="ask-a err">${esc(a.error || "The reply failed.")}</div>`
        : `<div class="ask-a">${a.text ? Chat.md(a.text) : `<span class="ask-dots"><i></i><i></i><i></i></span>`}</div>`;
      show(`${q ? `<div class="ask-q">${esc(q.text)}</div>` : ""}${body}`);
      if (a.status !== "streaming" && a.status !== "sending") clearInterval(poll);
    }, 500);
  }

  function show(html) {
    const out = $(".ask-out", el);
    out.hidden = false; out.innerHTML = html; out.scrollTop = out.scrollHeight;
    $(".ask-open", el).hidden = !thread;
  }

  function open() {
    if (!el) mount();
    if (!el.hidden) { $("input", el).focus(); return; }
    closePop();
    el.hidden = false;
    const r = window.Chat && Chat.defaultRoute();
    $(".ask-route", el).textContent = r ? Chat.routeName(r) : "No model connected";
    $("#aiBtn").setAttribute("aria-pressed", "true");
    requestAnimationFrame(() => { el.classList.add("on"); $("input", el).focus(); });
    invoke("chat_list").then(l => { chats = (l || []).filter(t => !t.app && !t.agent); }).catch(() => {});
  }
  // Closing forgets the exchange on screen; the thread stays in Chat.
  function close() {
    if (!el || el.hidden) return;
    clearInterval(poll); thread = null; hits = [];
    el.classList.remove("on"); el.hidden = true;
    $("input", el).value = ""; $(".ask-out", el).hidden = true; $(".ask-hits", el).hidden = true; $(".ask-open", el).hidden = true;
    $("#aiBtn").setAttribute("aria-pressed", "false");
  }
  const toggle = () => (el && !el.hidden ? close() : open());

  return { open, close, toggle };
})();
