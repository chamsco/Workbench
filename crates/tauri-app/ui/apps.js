// Apps: the catalog (installed, examples, install from a URL or a folder),
// each app's view in a sandboxed frame, and the host side of the app SDK
// (apps/sdk.js). Format: docs/apps.md.
//
// The frame has no IPC of its own. It posts {__bs, id, method, params}; we
// check the method against the manifest's permissions, do it, and answer
// {__bs, re, result | error}. Threads an app's agent writes are pushed to
// it as "thread" events while they change.

var Apps = (() => {
  let list = [], examples = [], busy = "", err = "";
  const frames = new Map();   // app id -> section id
  const watched = new Map();  // thread id -> app id
  const seen = new Map();     // thread id -> last JSON pushed

  const get = id => list.find(a => a.id === id && a.enabled);
  const installed = () => list.filter(a => a.enabled);
  const pinned = () => (prefs.pinned_apps || []).map(get).filter(Boolean);
  const isPinned = id => (prefs.pinned_apps || []).includes(id);
  const os = () => (window.__BOOT && window.__BOOT.platform) || "web";
  // Custom schemes are http://<scheme>.localhost on Windows' WebView2.
  const base = id => (os() === "win" ? `http://bsapp.localhost/${id}/` : `bsapp://localhost/${id}/`);
  const dark = () => { const t = document.documentElement.dataset.theme; return t ? t === "dark" : matchMedia("(prefers-color-scheme: dark)").matches; };

  function glyph(a, cls = "") {
    const ic = a.icon || {};
    if (ic.file) return `<img class="ag ${cls}" src="${esc(base(a.id) + ic.file)}" alt="">`;
    const g = (ic.glyph || a.name.slice(0, 2)).slice(0, 2);
    return `<span class="ag ${cls}" style="--ag:${esc(ic.color || "#6b6475")}">${esc(g)}</span>`;
  }

  async function load() {
    list = (await invoke("apps_list").catch(() => null)) || [];
    examples = (await invoke("app_examples").catch(() => null)) || [];
  }
  async function init() {
    await load();
    addEventListener("message", onMessage);
    matchMedia("(prefers-color-scheme: dark)").addEventListener("change", sendTheme);
  }
  async function reload(then) {
    await load();
    // Drop frames of apps that went away.
    for (const [id, sec] of frames) if (!get(id)) { const el = document.getElementById(sec); if (el) el.remove(); frames.delete(id); }
    if (then) then();
    if (S.mode.startsWith("app:") && !get(S.mode.slice(4))) setMode("apps"); else Shell.layout();
    renderList();
  }

  // ---------------------------------------------------------------- frames
  function frame(id) {
    const a = get(id); if (!a) return null;
    if (frames.has(id)) return frames.get(id);
    const sec = "app-" + id;
    const el = document.createElement("section");
    el.className = "zone appz"; el.id = sec; el.hidden = true;
    const src = a.view.url || base(id) + a.view.entry;
    el.innerHTML = `<iframe title="${esc(a.name)}" src="${esc(src)}" sandbox="allow-scripts allow-forms allow-popups allow-modals allow-downloads" allow="clipboard-write" referrerpolicy="no-referrer"></iframe>`;
    $("#body").insertBefore(el, $("#splitGut"));
    el.querySelector("iframe").addEventListener("load", () => sendTheme());
    frames.set(id, sec);
    return sec;
  }
  const frameOf = id => { const s = frames.get(id); const el = s && document.getElementById(s); return el && el.querySelector("iframe"); };
  function post(id, msg) { const f = frameOf(id); if (f && f.contentWindow) f.contentWindow.postMessage(Object.assign({ __bs: 1 }, msg), "*"); }
  function sendTheme() { for (const id of frames.keys()) post(id, { event: "theme", data: { dark: dark() } }); }

  // ---------------------------------------------------------------- SDK host
  const NEEDS = { "agent.ask": "agent", "agent.stop": "agent", threads: "agent", thread: "agent", routes: "routes", "storage.get": "storage", "storage.set": "storage", notify: "notify", open_url: "open_url" };
  async function onMessage(e) {
    const d = e.data || {};
    if (d.__bs !== 1 || d.id == null) return;
    let appId = null;
    for (const id of frames.keys()) { const f = frameOf(id); if (f && f.contentWindow === e.source) appId = id; }
    if (!appId) return;
    const reply = r => e.source.postMessage(Object.assign({ __bs: 1, re: d.id }, r), "*");
    try { reply({ result: await call(appId, d.method, d.params || {}) }); }
    catch (x) { reply({ error: String(x && x.message || x) }); }
  }
  async function call(appId, method, p) {
    const a = get(appId); if (!a) throw new Error("This app is not installed");
    const need = NEEDS[method];
    if (need && !(a.permissions || []).includes(need)) throw new Error(`${a.name} did not ask for "${need}" in backspace-app.json`);
    switch (method) {
      case "ready": return { app: { id: a.id, name: a.name, version: a.version }, user: (window.__BOOT && window.__BOOT.user) || "", dark: dark() };
      case "routes": return Chat.flat().map(i => ({ label: Chat.routeName(i.route), sub: i.sub || "", route: i.route }));
      case "agent.ask": {
        const text = String(p.prompt || "").trim(); if (!text) throw new Error("Empty prompt");
        let tid = p.thread && (await invoke("chat_thread", { id: p.thread }).catch(() => null)) ? p.thread : null;
        const route = p.route || (a.agent && a.agent.route) || Chat.defaultRoute();
        if (!route) throw new Error("No model is switched on in Backspace");
        if (!Chat.usable(route)) throw new Error(`${Chat.routeName(route)} isn't available`);
        if (!tid) {
          const t = await invoke("chat_new", { route, scope: { app: a.id, system: (a.agent && a.agent.instructions) || null } });
          tid = t.id;
        } else if (p.route) await invoke("chat_set_route", { id: tid, route: p.route });
        watched.set(tid, a.id);
        await invoke("chat_send", { id: tid, text, files: [], replyTo: null });
        pump();
        return { thread: tid };
      }
      case "agent.stop": await invoke("chat_stop", { id: p.thread }); return null;
      case "threads": return ((await invoke("chat_list").catch(() => [])) || []).filter(t => t.app === a.id);
      case "thread": { const t = await invoke("chat_thread", { id: p.id }); if (!t || t.app !== a.id) throw new Error("Not this app's thread"); return t; }
      case "storage.get": return await invoke("app_storage_get", { id: a.id, key: String(p.key) });
      case "storage.set": await invoke("app_storage_set", { id: a.id, key: String(p.key), value: p.value === undefined ? null : p.value }); return null;
      case "notify": toast(`${a.name}: ${String(p.text || "").slice(0, 200)}`, p.kind === "err" ? "err" : p.kind === "ok" ? "ok" : ""); return null;
      case "open_url": { const u = String(p.url || ""); if (!/^https?:\/\//.test(u)) throw new Error("Only http(s) links"); await invoke("open_url", { url: u }); return null; }
    }
    throw new Error("Unknown call: " + method);
  }
  // Push threads an app is waiting on while they change.
  let pumping = false;
  async function pump() {
    if (pumping || !watched.size) return;
    pumping = true;
    try {
      for (const [tid, appId] of [...watched]) {
        const t = await invoke("chat_thread", { id: tid }).catch(() => null);
        if (!t) { watched.delete(tid); continue; }
        const j = JSON.stringify(t);
        if (seen.get(tid) !== j) { seen.set(tid, j); post(appId, { event: "thread", data: t }); }
        const last = t.messages[t.messages.length - 1];
        if (last && (last.status === "done" || last.status === "error")) { watched.delete(tid); seen.delete(tid); }
      }
    } finally { pumping = false; }
    if (watched.size) setTimeout(pump, 250);
  }

  // ---------------------------------------------------------------- side + head
  function renderSide(el) {
    const cur = S.mode.startsWith("app:") ? S.mode.slice(4) : "";
    const apps = installed();
    el.innerHTML = `<button class="row${S.mode === "apps" ? " on" : ""}" id="apBrowse">${icon("apps")}<span class="lab">All apps</span></button>
      ${apps.length ? `<div class="sec-t">Installed</div>` + apps.map(a => `<button class="row approw${cur === a.id ? " on" : ""}" data-app="${esc(a.id)}">${glyph(a, "xs")}<span class="lab">${esc(a.name)}</span>${isPinned(a.id) ? icon("pin", "pinned") : ""}</button>`).join("") : `<div class="nothing">No apps yet. Install one from the catalog.</div>`}`;
    $("#apBrowse", el).onclick = () => setMode("apps");
    $$("[data-app]", el).forEach(b => (b.onclick = () => setMode("app:" + b.dataset.app)));
  }
  function head(id) {
    const a = get(id); if (!a) return "";
    return `<span class="zt">${glyph(a, "xs")}${esc(a.name)}${a.version ? `<span class="sub">${esc(a.version)}</span>` : ""}</span>
      <button class="ib" data-ah="reload" title="Reload" aria-label="Reload ${esc(a.name)}">${icon("reload")}</button>
      <button class="ib" data-ah="pin" aria-pressed="${isPinned(id)}" title="${isPinned(id) ? "Unpin from the rail" : "Pin to the rail"}" aria-label="Pin">${icon("pin")}</button>`;
  }
  function wireHead(h, id) {
    $$("[data-ah]", h).forEach(b => (b.onclick = () => {
      if (b.dataset.ah === "reload") { const f = frameOf(id); if (f) f.src = f.src; }
      else togglePin(id);
    }));
  }
  function togglePin(id) {
    const cur = prefs.pinned_apps || [];
    prefs.pinned_apps = cur.includes(id) ? cur.filter(x => x !== id) : [...cur, id];
    invoke("set_pinned_apps", { ids: prefs.pinned_apps });
    Shell.layout(); renderList();
  }

  // ---------------------------------------------------------------- catalog
  function card(a, inst) {
    const perms = (a.permissions || []).filter(p => PERM[p]).map(p => PERM[p]);
    return `<div class="acard">
      <div class="ac-top">${glyph(a)}<div class="ac-n"><b>${esc(a.name)}</b><small>${esc([a.version && "v" + a.version, a.author].filter(Boolean).join(" · "))}</small></div></div>
      <p>${esc(a.description || "")}</p>
      ${perms.length ? `<div class="ac-perm">${perms.map(p => `<span>${esc(p)}</span>`).join("")}</div>` : ""}
      ${(a.tools || []).length ? `<div class="ac-perm"><span>${a.tools.length} tool${a.tools.length > 1 ? "s" : ""} for coding CLIs</span></div>` : ""}
      <div class="ac-act">${inst
        ? `<button class="btn primary sm" data-open="${esc(a.id)}">Open</button><button class="btn sm" data-pin="${esc(a.id)}">${isPinned(a.id) ? "Unpin" : "Pin to rail"}</button><span class="sp"></span><button class="ib" data-rm="${esc(a.id)}" title="Remove" aria-label="Remove ${esc(a.name)}">${icon("trash")}</button>`
        : `<button class="btn primary sm" data-ex="${esc(a.id)}" ${busy ? "disabled" : ""}>${busy === a.id ? "Installing…" : `${icon("download")}Install`}</button>`}</div>
    </div>`;
  }
  const PERM = { agent: "Asks models you've switched on", routes: "Sees which models are on", storage: "Keeps its own data", notify: "Shows notifications", open_url: "Opens links in your browser" };

  function renderZone() {
    const el = $("#apps");
    const inst = installed();
    const ex = examples.filter(m => !list.some(a => a.id === m.id));
    el.innerHTML = `<div class="apz">
      <div class="apz-h"><h1>Apps</h1><p>Views with their own agent, made by anyone: install one from a link to its <code>backspace-app.json</code>, or from a folder while you build it.</p></div>
      <div class="apz-inst">
        <input class="tx" id="apUrl" placeholder="https://…/backspace-app.json" aria-label="App manifest URL" autocomplete="off">
        <button class="btn primary" id="apGo" ${busy ? "disabled" : ""}>${busy === "url" ? "Installing…" : "Install"}</button>
        <button class="btn" id="apDir" ${busy ? "disabled" : ""}>${icon("folder")}From folder…</button>
      </div>
      <div class="err" role="alert">${esc(err)}</div>
      <h2>Installed</h2>
      ${inst.length ? `<div class="agrid">${inst.map(a => card(a, true)).join("")}</div>` : `<div class="apz-empty">Nothing installed yet. Try an example below, or install your own.</div>`}
      ${ex.length ? `<h2>Examples</h2><div class="agrid">${ex.map(a => card(a, false)).join("")}</div>` : ""}
      <h2>Make one</h2>
      <p class="apz-make">An app is a folder with <code>backspace-app.json</code> and a page. The page includes <code>__backspace/sdk.js</code> and calls <code>backspace.agent.ask(prompt)</code>, <code>backspace.storage</code>, <code>backspace.notify</code>. The full format is in <code>docs/apps.md</code>.</p>
    </div>`;
    const install = async (key, fn) => {
      busy = key; err = ""; renderZone();
      try { const a = await fn(); toast(`Installed ${a.name}`, "ok"); busy = ""; await reload(); setMode("app:" + a.id); }
      catch (e) { err = String(e); busy = ""; renderZone(); }
    };
    $("#apGo", el).onclick = () => { const u = $("#apUrl", el).value.trim(); if (!u) { err = "Paste the link to an app's backspace-app.json."; renderZone(); return; } install("url", () => invoke("app_install_url", { url: u })); };
    $("#apUrl", el).onkeydown = e => { if (e.key === "Enter") $("#apGo", el).click(); };
    $("#apDir", el).onclick = async () => { const p = await invoke("pick_app").catch(() => null); if (p) install("dir", () => invoke("app_install_dir", { path: p })); };
    $$("[data-ex]", el).forEach(b => (b.onclick = () => install(b.dataset.ex, () => invoke("app_install_example", { id: b.dataset.ex }))));
    $$("[data-open]", el).forEach(b => (b.onclick = () => setMode("app:" + b.dataset.open)));
    $$("[data-pin]", el).forEach(b => (b.onclick = () => { togglePin(b.dataset.pin); renderZone(); }));
    // Two clicks to remove: the first asks.
    $$("[data-rm]", el).forEach(b => (b.onclick = async () => {
      const a = get(b.dataset.rm); if (!a) return;
      if (!b.dataset.armed) { b.dataset.armed = "1"; b.classList.add("armed"); b.innerHTML = `Remove ${esc(a.name)} and its data?`; setTimeout(() => { if (b.isConnected) renderZone(); }, 4000); return; }
      try { await invoke("app_remove", { id: a.id }); prefs = await invoke("prefs"); toast(`Removed ${a.name}`, "ok"); await reload(renderZone); } catch (e) { toast(String(e), "err"); }
    }));
  }

  return { init, get, installed, pinned, glyph, frame, renderSide, renderZone, head, wireHead, pump, sendTheme, reload };
})();
