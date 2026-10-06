// Backspace app SDK. Include it in an app's page:
//   <script src="sdk.js"></script>    (a copy in the app folder), or
//   <script src="/__backspace/sdk.js"></script>  (served by the host; path is
//   relative to the app root, e.g. "./__backspace/sdk.js" from index.html)
//
// The page runs in a sandboxed frame with no access to Backspace itself.
// Every call below is a message to the host, which checks it against the
// "permissions" in backspace-app.json. Format and API: docs/apps.md.
(() => {
  if (window.backspace) return;
  let seq = 0;
  const waiting = new Map();
  const subs = new Map();
  addEventListener("message", e => {
    if (e.source !== window.parent) return;
    const d = e.data || {};
    if (d.__bs !== 1) return;
    if (d.re != null) {
      const w = waiting.get(d.re);
      if (!w) return;
      waiting.delete(d.re);
      if (d.error) w[1](new Error(d.error)); else w[0](d.result);
      return;
    }
    if (d.event) (subs.get(d.event) || []).slice().forEach(f => { try { f(d.data); } catch (err) { console.error(err); } });
  });
  const call = (method, params) => new Promise((ok, no) => {
    const id = ++seq;
    waiting.set(id, [ok, no]);
    window.parent.postMessage({ __bs: 1, id, method, params: params || {} }, "*");
  });
  const on = (event, f) => {
    if (!subs.has(event)) subs.set(event, []);
    subs.get(event).push(f);
    return () => subs.set(event, (subs.get(event) || []).filter(x => x !== f));
  };

  // Follow one thread until its last reply is done.
  function follow(thread, onText) {
    return new Promise((ok, no) => {
      const off = on("thread", t => {
        if (!t || t.id !== thread) return;
        const m = t.messages[t.messages.length - 1];
        if (!m || m.role !== "assistant") return;
        if (onText) onText(m.text, m);
        if (m.status === "done") { off(); ok({ text: m.text, thread, message: m }); }
        else if (m.status === "error") { off(); no(new Error(m.error || "The reply failed")); }
      });
    });
  }

  window.backspace = {
    /** Who and where: { app, user, dark, routes }. */
    ready: () => call("ready"),
    agent: {
      /** Ask the app's agent. Resolves when the reply is complete:
       *  { text, thread, message }. opts: { route, thread, onText(text) }.
       *  Pass the returned thread back to continue the conversation. */
      async ask(prompt, opts = {}) {
        const r = await call("agent.ask", { prompt, route: opts.route || null, thread: opts.thread || null });
        return follow(r.thread, opts.onText);
      },
      stop: thread => call("agent.stop", { thread }),
    },
    /** This app's threads (newest first), and one in full. */
    threads: () => call("threads"),
    thread: id => call("thread", { id }),
    /** Every model the user has switched on: [{ label, sub, route }]. */
    routes: () => call("routes"),
    storage: {
      get: key => call("storage.get", { key }),
      /** null removes the key. An app keeps at most 1 MB. */
      set: (key, value) => call("storage.set", { key, value: value === undefined ? null : value }),
    },
    /** A toast in Backspace: kind "ok" | "err" | "". */
    notify: (text, kind) => call("notify", { text, kind: kind || "" }),
    openUrl: url => call("open_url", { url }),
    /** Events: "theme" ({ dark }), "thread" (a thread of this app changed). */
    on,
  };
  // Follow the host's light/dark before the first paint where possible.
  on("theme", t => { document.documentElement.dataset.theme = t.dark ? "dark" : "light"; document.documentElement.style.colorScheme = t.dark ? "dark" : "light"; });
})();
