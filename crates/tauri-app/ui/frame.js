// Frame: the window around an app.
//
// Chat and Code are not the shell. They live in extensions/<id>/ with an
// extension.json that says what to load; the frame reads extensions/index.json,
// makes each app a section inside #body, loads its style and script in the
// order the rest of the page expects, and hands it the walls it may use — the
// switcher, On/New and the shared state in prefs.
//
// A native extension is a classic script, like chat.js always was: it shares
// the page's globals with the frame, so it reads prefs/S/snap and calls setMode
// directly, and it registers itself when it loads:
//
//   Frame.register({
//     id: "chat",             // must match extension.json
//     mount(el),              // build inside its section; runs once
//     init(),                 // after the shell and its data are up
//     show(), hide(),         // the app became visible / went away
//     paint(on),              // its section was laid out; `on` is whether it shows
//     elFor(),                // which section shows the app right now (default: section)
//     classes(),              // window classes while the app is active (e.g. pair-on)
//     newInstance(anchor),    // a new instance of the app; anchor is the button
//   });
//
// The frame never names an app. The switcher and On all come from the
// manifests; Settings writes the same prefs.uses list.
//
// A web extension (view.kind = "web") is an iframe in the same section instead
// of a script. It gets its own copy of the page; the app SDK bridge in apps.js
// is not wired to it yet (docs/extensions.md).
var Frame = (() => {
  const ROOT = "extensions/";
  const list = [];              // registered, in manifest order
  const byId = new Map();
  const manifests = new Map();  // id -> manifest, read before the script runs
  let active = null, loading = null, lastClasses = [];

  const ids = () => list.map(e => e.id);
  const has = id => byId.has(id);
  const get = id => byId.get(id);
  const onIds = () => {
    const all = ids();
    const uses = (typeof prefs !== "undefined" && prefs && prefs.uses && prefs.uses.length) ? prefs.uses : all;
    return all.filter(id => uses.includes(id));
  };
  const enabled = id => onIds().includes(id);

  // ---------------------------------------------------------------- loading
  function style(href) {
    return new Promise(done => {
      const l = document.createElement("link");
      l.rel = "stylesheet"; l.href = href;
      l.onload = l.onerror = () => done();
      // The order the app has always had: whirl, style, chat, home, shell.
      // An extension's style belongs where chat.css was, before home.css, so
      // the cascade does not change when the file moves into its folder.
      const before = document.head.querySelector('link[href="home.css"], link[href="shell.css"]');
      document.head.insertBefore(l, before || null);
    });
  }
  function script(src) {
    return new Promise(done => {
      const s = document.createElement("script");
      s.src = src; s.async = false;
      s.onload = () => done();
      s.onerror = () => { console.warn("frame: could not load", src); done(); };
      document.body.append(s);
    });
  }

  async function load() {
    if (loading) return loading;
    loading = (async () => {
      let index;
      try {
        index = await (await fetch(ROOT + "index.json", { cache: "no-store" })).json();
      } catch (e) {
        console.warn("frame: no extensions/index.json", e);
        return;
      }
      const dirs = Array.isArray(index) ? index : index.extensions || [];
      for (const dir of dirs) {
        let m;
        try {
          m = await (await fetch(`${ROOT}${dir}/extension.json`, { cache: "no-store" })).json();
        } catch (e) {
          console.warn("frame: cannot read", dir, e);
          continue;
        }
        if (!m || !m.id) { console.warn("frame: manifest without an id in", dir); continue; }
        m.dir = dir;
        manifests.set(m.id, m);
        if (m.view && m.view.style) await style(`${ROOT}${dir}/${m.view.style}`);
        if (m.view && m.view.kind === "web") {
          register({ id: m.id, mount: el => mountWeb(m, dir, el) });
        } else if (m.view && m.view.script) {
          await script(`${ROOT}${dir}/${m.view.script}`);
          if (!byId.has(m.id)) console.warn("frame: " + dir + " loaded but never registered");
        } else {
          console.warn("frame: " + dir + " has no view");
        }
      }
    })();
    return loading;
  }

  function mountWeb(m, dir, el) {
    el.innerHTML = `<iframe title="${esc(m.name)}" src="${esc(dir + "/" + m.view.entry)}" sandbox="allow-scripts allow-forms allow-popups allow-modals"></iframe>`;
    el.classList.add("ext-web");
  }

  // ---------------------------------------------------------------- registry
  function register(spec) {
    if (!spec || !spec.id) { console.warn("frame: register() without an id"); return null; }
    const m = Object.assign({}, manifests.get(spec.id) || {}, spec);
    if (byId.has(m.id)) { Object.assign(byId.get(m.id), spec); return byId.get(m.id); }
    byId.set(m.id, m);
    list.push(m);
    list.sort((a, b) => (a.order || 100) - (b.order || 100));
    section(m);
    if (m.mount) m.mount(m.el);
    return m;
  }

  function section(m) {
    if (!m.section) return;
    let el = document.getElementById(m.section);
    if (!el) {
      el = document.createElement("section");
      el.id = m.section;
      el.className = m.className || "zone";
      if (!/\bzone\b/.test(el.className)) el.className += " zone";
      el.hidden = true;
      const gut = document.getElementById("splitGut");
      document.getElementById("body").insertBefore(el, gut || null);
    }
    m.el = el;
  }

  async function init() {
    for (const e of list) if (e.init) { try { await e.init(); } catch (err) { console.error("frame: " + e.id + " init", err); } }
  }

  // ---------------------------------------------------------------- showing
  function setActive(id) {
    if (id === active) return;
    const prev = byId.get(active);
    if (prev && prev.hide) { try { prev.hide(); } catch (e) { console.warn("frame: " + prev.id + " hide", e); } }
    active = id;
    const ext = byId.get(id);
    if (ext && ext.show) { try { ext.show(); } catch (e) { console.warn("frame: " + id + " show", e); } }
  }

  // Whether this app's surface (or any of its sections) is on screen now.
  function paint(...shown) {
    shown = shown.filter(Boolean);
    for (const e of list) {
      if (!e.paint) continue;
      const els = [e.section, e.elFor ? e.elFor() : null];
      const on = shown.some(id => els.includes(id));
      try { e.paint(on); } catch (err) { console.warn("frame: " + e.id + " paint", err); }
    }
  }

  function elFor(id) {
    const ext = byId.get(id);
    if (!ext) return id;
    if (ext.elFor) { try { return ext.elFor(); } catch { return ext.section; } }
    return ext.section;
  }

  // Window classes an app owns while it is active (Code's pair-on, Chat's
  // agents-on). The frame remembers what it set so a class never goes stale.
  function syncClasses(w) {
    const ext = byId.get(active);
    let next = [];
    if (ext && ext.classes) { try { next = ext.classes() || []; } catch { next = []; } }
    for (const c of lastClasses) if (!next.includes(c)) w.classList.remove(c);
    for (const c of next) w.classList.add(c);
    lastClasses = next;
  }

  // ---------------------------------------------------------------- On
  async function setOn(id, on, quiet) {
    const all = ids();
    if (!all.includes(id)) return false;
    const cur = (prefs.uses && prefs.uses.length ? prefs.uses : all).slice();
    const next = all.filter(x => (x === id ? on : cur.includes(x)));
    if (!next.length) { if (!quiet) toast("Keep at least one app on", "err"); return false; }
    prefs.uses = next;
    invoke("set_uses", { uses: next }).catch(e => toast(String(e), "err"));
    if (!on && typeof S !== "undefined" && S.mode === id) setMode(next[0]);
    if (window.Shell) Shell.renderRail();
    return true;
  }

  // ---------------------------------------------------------------- New
  function newInstance(id, anchor) {
    const ext = byId.get(id || active);
    if (!ext) return;
    if (ext.newInstance) { try { ext.newInstance(anchor); } catch (e) { console.error("frame: " + ext.id + " new", e); } return; }
    if (ext.action && ext.action.url) invoke("open_url", { url: ext.action.url });
  }

  // ---------------------------------------------------------------- the head
  // The title-bar head is the app's too: Code puts the project's folder there,
  // an app could put its name. `null` means the app has nothing to say.
  function head(id) {
    const ext = byId.get(id || active);
    if (ext && ext.head) { try { return ext.head(); } catch { return null; } }
    return null;
  }
  function windowTitle(id) {
    const ext = byId.get(id || active);
    if (ext && ext.windowTitle) { try { return ext.windowTitle(); } catch { return null; } }
    return null;
  }

  // ---------------------------------------------------------------- the list
  function zones() {
    return list.map(e => ({ id: e.id, name: e.name || e.id, icon: e.icon || "bubble", order: e.order ?? 100, description: e.description || "" }));
  }

  return { load, register, init, ids, zones, has, get, enabled, onIds, setOn, setActive, syncClasses, paint, elFor, head, windowTitle, newInstance };
})();
