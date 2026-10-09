// Chat: threads in the sidebar, an iMessage-style conversation in the main
// column. Each thread talks to one route (a CLI on this machine, Ollama, a
// router, Backspace Cloud), switchable from the composer.
//
// Messages are bubbles grouped by sender, with tapbacks, replies, link
// previews, image attachments, a typing indicator while the reply is on its
// way, and "Not delivered · Retry" when it fails. On Cloud's free tier a
// sponsored card can follow a reply; it is never part of the reply.

var Chat = (() => {
  const C = {
    threads: [], id: null, t: null, q: "",
    reply: null, editing: null, files: [],
    drafts: new Map(), previews: new Map(), fileUrls: new Map(),
    homeRoute: null, html: new Map(), dismissedAds: new Set(),
  };
  const TAPBACKS = ["❤️", "👍", "👎", "😂", "‼️", "❓"];

  // ---------------------------------------------------------------- routes
  function options() {
    const L = Providers.list || [];
    const groups = [];
    const clis = L.filter(h => h.kind === "cli" && h.enabled && h.chat);
    if (clis.length) groups.push({ name: "Your coding CLIs", sub: "Runs on this machine with your own subscription", items: clis.map(h => ({ route: { kind: "cli", provider: h.id, model: null }, id: h.id, label: h.name, sub: [h.version && "v" + h.version, h.detail].filter(Boolean).join(" · ") || "Installed" })) });
    const ol = L.find(h => h.id === "ollama" && h.enabled);
    if (ol) groups.push({ name: "On this machine", sub: "Private and offline", items: ol.models.length ? ol.models.map(m => ({ route: { kind: "local", provider: "ollama", model: m }, id: "ollama", label: m, sub: "Ollama" })) : [{ disabled: true, id: "ollama", label: "No models pulled", sub: "Run `ollama pull llama3.2`" }] });
    const ags = L.filter(h => h.kind === "agent" && h.enabled);
    if (ags.length) groups.push({ name: "Agents from elsewhere", sub: "A2A", items: ags.map(h => ({ route: { kind: "a2a", provider: h.id.replace(/^a2a:/, ""), model: null }, id: h.id, label: h.name, sub: h.detail || "A2A agent" })) });
    L.filter(h => h.kind === "router" && h.enabled).forEach(h => groups.push({ name: h.name, sub: h.path, items: (h.models.length ? h.models.slice(0, 40) : []).map(m => ({ route: { kind: "router", provider: h.id.replace(/^router:/, ""), model: m }, id: h.id, label: m, sub: h.name })) }));
    const acct = Providers.account;
    if (prefs.cloud && prefs.cloud.token && acct) {
      groups.push({ name: "Backspace Cloud", sub: `${acct.plan[0].toUpperCase() + acct.plan.slice(1)} plan`, items: acct.models.map(m => ({ route: { kind: "cloud", provider: "cloud", model: m.id }, id: "cloud", label: m.label, sub: m.allowed ? (m.with_ads ? "With ads" : "Included") : "Upgrade to use", badge: !m.allowed ? "Upgrade" : m.with_ads ? "Ads" : "", disabled: !m.allowed })) });
    } else {
      groups.push({ name: "Backspace Cloud", sub: "Nothing to install", items: [{ setup: true, id: "cloud", label: "Start free with Cloud", sub: "Free with ads, or Plus / Max" }] });
    }
    return groups;
  }
  const flat = () => options().flatMap(g => g.items).filter(i => i.route && !i.disabled);
  const same = (a, b) => a && b && a.kind === b.kind && a.provider === b.provider && (a.model || null) === (b.model || null);
  function usable(r) {
    if (!r) return false;
    if (r.kind === "cloud") return !!(prefs.cloud && prefs.cloud.token);
    if (r.kind === "cli") return (Providers.list || []).some(h => h.id === r.provider && h.enabled);
    if (r.kind === "local") return (Providers.list || []).some(h => h.id === "ollama" && h.enabled);
    if (r.kind === "a2a") return (Providers.list || []).some(h => h.id === "a2a:" + r.provider && h.enabled);
    return (Providers.list || []).some(h => h.id === "router:" + r.provider && h.enabled);
  }
  function providerName(r) {
    if (!r) return "";
    if (r.kind === "cloud") return "Cloud";
    if (r.kind === "local") return "Ollama";
    const h = (Providers.list || []).find(h => h.id === r.provider || h.id === "router:" + r.provider || h.id === "a2a:" + r.provider);
    return h ? h.name : r.provider;
  }
  function routeName(r) {
    if (!r) return "Pick a model";
    if (r.kind === "cli") return providerName(r) + (r.model ? " · " + r.model : "");
    if (r.kind === "cloud") { const m = Providers.account && Providers.account.models.find(m => m.id === r.model); return "Cloud · " + (m ? m.label : r.model || "auto"); }
    if (r.kind === "a2a") return providerName(r);
    return `${providerName(r)} · ${r.model || "default"}`;
  }
  const avId = r => !r ? "cloud" : r.kind === "cli" ? r.provider : r.kind === "local" ? "ollama" : r.kind === "cloud" ? "cloud" : r.kind === "a2a" ? "a2a:" + r.provider : "router:" + r.provider;
  function defaultRoute() {
    if (prefs.default_route && usable(prefs.default_route)) return prefs.default_route;
    const f = flat(); return f.length ? f[0].route : null;
  }

  function routePicker(anchor, cur, cb) {
    const gs = options();
    openPop(anchor, `<div class="rp">${gs.map(g => `<h5>${esc(g.name)}<small>${esc(g.sub || "")}</small></h5>${g.items.map((it, i) => `<button class="pi rpi${same(it.route, cur) ? " on" : ""}" data-g="${esc(g.name)}" data-i="${i}" ${it.disabled ? "disabled" : ""}>${Providers.avatar(it.id, "sm")}<span class="rl"><b>${esc(it.label)}</b><small>${esc(it.sub || "")}</small></span>${it.badge ? `<span class="badge-s ${it.badge === "Ads" ? "ads" : "up"}">${it.badge}</span>` : ""}${same(it.route, cur) ? icon("check") : ""}</button>`).join("")}`).join("")}
      <hr><button class="pi" id="rpManage">${icon("gear")}Manage CLIs and models…</button></div>`, pop => {
      $$(".rpi", pop).forEach(b => (b.onclick = () => {
        const g = gs.find(g => g.name === b.dataset.g), it = g.items[+b.dataset.i];
        closePop();
        if (it.setup) { openSettings("plan"); return; }
        cb(it.route);
      }));
      $("#rpManage", pop).onclick = () => { closePop(); openSettings("providers"); };
    });
  }

  // ---------------------------------------------------------------- markdown
  const linkify = s => s.replace(/(^|[\s(])(https?:\/\/[^\s<)]+[^\s<).,;:!?'"])/g, (m, pre, u) => `${pre}<a data-href="${u}">${u}</a>`);
  function inline(t) {
    const codes = [];
    let s = esc(t).replace(/`([^`]+)`/g, (_, c) => { codes.push(c); return `\u0000${codes.length - 1}\u0000`; });
    s = s.replace(/\[([^\]]+)\]\((https?:[^)\s]+)\)/g, '<a data-href="$2">$1</a>');
    s = linkify(s);
    s = s.replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>").replace(/(^|[^*])\*([^*\s][^*]*)\*/g, "$1<i>$2</i>").replace(/(^|\W)_([^_\s][^_]*)_(?=\W|$)/g, "$1<i>$2</i>").replace(/~~([^~]+)~~/g, "<s>$1</s>");
    return s.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${codes[+i]}</code>`);
  }
  function md(src) {
    const out = []; const lines = src.replace(/\r/g, "").split("\n");
    let i = 0, para = [];
    const flush = () => { if (para.length) { out.push(`<p>${para.map(inline).join("<br>")}</p>`); para = []; } };
    while (i < lines.length) {
      const l = lines[i];
      const fence = l.match(/^\s*```\s*([\w+#.-]*)/);
      if (fence) {
        flush(); const lang = fence[1]; const code = []; i++;
        while (i < lines.length && !/^\s*```\s*$/.test(lines[i])) code.push(lines[i++]);
        i++;
        out.push(`<div class="code"><div class="code-h"><span>${esc(lang || "code")}</span><button class="copyc">${icon("copy")}Copy</button></div><pre><code>${esc(code.join("\n"))}</code></pre></div>`);
        continue;
      }
      if (/^\s*$/.test(l)) { flush(); i++; continue; }
      const h = l.match(/^(#{1,4})\s+(.*)/);
      if (h) { flush(); out.push(`<h${Math.min(4, h[1].length + 2)}>${inline(h[2])}</h${Math.min(4, h[1].length + 2)}>`); i++; continue; }
      if (/^\s*([-*_])\s*\1\s*\1[\s\1]*$/.test(l)) { flush(); out.push("<hr>"); i++; continue; }
      if (/^\s*>/.test(l)) { flush(); const q = []; while (i < lines.length && /^\s*>/.test(lines[i])) q.push(lines[i++].replace(/^\s*>\s?/, "")); out.push(`<blockquote>${md(q.join("\n"))}</blockquote>`); continue; }
      if (/^\s*([-*+]|\d+[.)])\s+/.test(l)) {
        flush(); const ordered = /^\s*\d/.test(l); const items = [];
        while (i < lines.length && /^\s*([-*+]|\d+[.)])\s+/.test(lines[i])) {
          let item = lines[i++].replace(/^\s*([-*+]|\d+[.)])\s+/, "");
          while (i < lines.length && /^\s{2,}\S/.test(lines[i]) && !/^\s*([-*+]|\d+[.)])\s+/.test(lines[i])) item += " " + lines[i++].trim();
          const task = item.match(/^\[([ xX])\]\s+(.*)/);
          items.push(task ? `<li class="task"><span class="cb${task[1] !== " " ? " on" : ""}"></span>${inline(task[2])}</li>` : `<li>${inline(item)}</li>`);
        }
        out.push(ordered ? `<ol>${items.join("")}</ol>` : `<ul>${items.join("")}</ul>`); continue;
      }
      if (/^\s*\|.*\|\s*$/.test(l) && i + 1 < lines.length && /^\s*\|?\s*:?-{2,}/.test(lines[i + 1])) {
        flush(); const row = r => r.trim().replace(/^\||\|$/g, "").split("|").map(c => c.trim());
        const head = row(l); i += 2; const body = [];
        while (i < lines.length && /^\s*\|.*\|\s*$/.test(lines[i])) body.push(row(lines[i++]));
        out.push(`<div class="tbl"><table><thead><tr>${head.map(c => `<th>${inline(c)}</th>`).join("")}</tr></thead><tbody>${body.map(r => `<tr>${r.map(c => `<td>${inline(c)}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`); continue;
      }
      para.push(l); i++;
    }
    flush();
    return out.join("");
  }
  const plainText = t => esc(t).replace(/\n/g, "<br>");
  const firstUrl = t => { const m = t.replace(/```[\s\S]*?```/g, "").match(/https?:\/\/[^\s<>)\]"']+[^\s<>)\]"'.,;:!?]/); return m && m[0]; };

  // ---------------------------------------------------------------- time
  const sameDay = (a, b) => new Date(a).toDateString() === new Date(b).toDateString();
  function stamp(ms) {
    const d = new Date(ms), now = Date.now(), t = d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
    if (sameDay(ms, now)) return `<b>Today</b> ${t}`;
    if (sameDay(ms, now - 864e5)) return `<b>Yesterday</b> ${t}`;
    if (now - ms < 6 * 864e5) return `<b>${d.toLocaleDateString([], { weekday: "long" })}</b> ${t}`;
    return `<b>${d.toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" })}</b> ${t}`;
  }
  function ago(ms) {
    const s = (Date.now() - ms) / 1000;
    if (s < 60) return "now"; if (s < 3600) return Math.floor(s / 60) + "m";
    if (sameDay(ms, Date.now())) return new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
    if (s < 6 * 86400) return new Date(ms).toLocaleDateString([], { weekday: "short" });
    return new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });
  }

  // ---------------------------------------------------------------- Whirl
  // The chat face is Whirl's (web/ → chat-web/whirl.js) when its bundle
  // loaded; the vanilla rendering below stays as the fallback.
  let whirl = false, scope = null, agentsMode = false;
  // Pair shows the chat face scoped to the open project: its threads, and
  // new ones bound to it. Elsewhere, the plain chat list.
  function setScope(s, agents) {
    s = s || null; agents = !!agents && !s;
    if (s === scope && agents === agentsMode) return;
    scope = s; agentsMode = agents;
    if (whirl && window.WhirlChat.setScope) window.WhirlChat.setScope(scope, agentsMode);
  }
  const host = () => ({
    scope: () => scope,
    agentsMode: () => agentsMode,
    onCaptured: (note, who) => window.Shell && Shell.memToast(note, who),
    openComputer: url => window.Shell && Shell.openDesktop(url),
    remember: (text, source) => invoke("memory_add", { text, project: scope, source: source || "chat" }).then(() => { toast("Saved to Memory", "ok"); if (window.Memory) Memory.refresh(); }),
    invoke: (cmd, args) => invoke(cmd, args),
    routes: options,
    routeName,
    providerName,
    defaultRoute,
    usable,
    account: () => Providers.account,
    openSettings: sec => openSettings(sec),
    runSetup: () => Onboard.open(),
    toast,
    openUrl: url => invoke("open_url", { url }),
    // Code chat: the same model and effort picker as the agents' composer,
    // the CLI's permissions and effort (shared with the agents), where it runs.
    codeRun: () => ({ permission: prefs.cli_permission || "edits", effort: prefs.cli_effort || null }),
    setCodeRun: (permission, effort) => {
      prefs.cli_permission = permission; prefs.cli_effort = effort;
      return invoke("set_agent_run", { planner: prefs.planner || null, worker: prefs.worker || null, permission, effort });
    },
    picker: {
      mount: (el, choice, onChange, onClose) => Picker.mount(el, choice, onChange, onClose, { auto: false }),
      close: () => Picker.close(),
      model: cli => Picker.MODEL[cli] || "",
      cli: model => Picker.cliOf(model),
      effortName: (cli, e) => Effort.name(cli, e),
    },
    machineName: () => machine().name,
    projectMenu: anchor => Shell.projectMenu(anchor),
    openPanel: tab => window.RPanel && RPanel.show(tab),
  });
  function mountWhirl() {
    if (whirl || !window.WhirlChat) return false;
    const user = (window.__BOOT && window.__BOOT.user) || "there";
    window.WhirlChat.mount($("#chat"), $("#chatList"), host(), user);
    whirl = true;
    $("#win").classList.add("whirl-on");
    $("#chatList").hidden = false;
    return true;
  }

  // ---------------------------------------------------------------- sidebar
  function renderThreads() {
    if (whirl) return;
    const list = $("#list");
    const q = C.q.trim().toLowerCase();
    const ts = C.threads.filter(t => !q || t.title.toLowerCase().includes(q) || t.preview.toLowerCase().includes(q));
    if (!C.threads.length) {
      list.innerHTML = `<div class="side-empty">${icon("compose")}<b>No chats yet</b>Start one with any model you've connected.<button class="btn sm" id="sideNew">New chat</button></div>`;
      $("#sideNew").onclick = newChat; return;
    }
    if (!ts.length) { list.innerHTML = `<div class="side-empty"><b>No chats match “${esc(C.q)}”</b><button class="btn sm" id="clearQ">Clear search</button></div>`; $("#clearQ").onclick = () => { C.q = ""; $("#search").value = ""; renderThreads(); }; return; }
    const now = Date.now(), groups = [];
    const bucket = t => t.pinned ? "Pinned" : sameDay(t.updated, now) ? "Today" : sameDay(t.updated, now - 864e5) ? "Yesterday" : now - t.updated < 7 * 864e5 ? "Previous 7 days" : now - t.updated < 30 * 864e5 ? "Previous 30 days" : "Older";
    ts.forEach(t => { const b = bucket(t); let g = groups.find(g => g.n === b); if (!g) groups.push(g = { n: b, ts: [] }); g.ts.push(t); });
    list.innerHTML = groups.map(g => `<div class="grp-label">${g.n === "Pinned" ? icon("pin") : ""}${g.n}</div>` + g.ts.map(t => `
      <button class="thr${t.id === C.id ? " sel" : ""}${t.unread ? " unread" : ""}" data-t="${t.id}" title="${esc(t.title)}">
        ${Providers.avatar(avId(t.route), "sm")}
        <span class="tt"><span class="t1"><span class="tn">${esc(t.title)}</span><span class="tm">${t.busy ? `<span class="typing sm"><i></i><i></i><i></i></span>` : ago(t.updated)}</span></span>
        <span class="t2">${t.unread ? '<span class="udot"></span>' : ""}${esc(t.preview || routeName(t.route))}</span></span>
      </button>`).join("")).join("");
    $$("#list [data-t]").forEach(b => {
      b.onclick = () => open(b.dataset.t);
      b.oncontextmenu = e => { e.preventDefault(); threadMenu(b, b.dataset.t); };
    });
  }
  function threadMenu(anchor, id) {
    const t = C.threads.find(t => t.id === id); if (!t) return;
    openPop(anchor, `<button class="pi" id="tmRen">${icon("edit")}Rename</button>
      <button class="pi" id="tmPin">${icon("pin")}${t.pinned ? "Unpin" : "Pin"}</button><hr>
      <button class="pi danger" id="tmDel">${icon("trash")}Delete…</button>`, pop => {
      $("#tmRen", pop).onclick = () => { closePop(); rename(id); };
      $("#tmPin", pop).onclick = async () => { closePop(); await invoke("chat_pin", { id, pinned: !t.pinned }); };
      $("#tmDel", pop).onclick = () => {
        pop.innerHTML = `<div class="note">Delete “${esc(t.title)}” and its attachments? This can't be undone.</div><div class="row-end"><button class="btn" id="dNo">Cancel</button><button class="btn danger" id="dYes">Delete</button></div>`;
        $("#dNo", pop).onclick = closePop;
        $("#dYes", pop).onclick = async () => { closePop(); await invoke("chat_delete", { id }); if (C.id === id) { C.id = null; C.t = null; } await refresh(); render(); };
      };
    });
  }
  function rename(id) {
    const t = C.threads.find(t => t.id === id) || C.t; if (!t) return;
    const anchor = $(`#list [data-t="${id}"]`) || $("#chatHead");
    openPop(anchor, `<h5>Rename chat</h5><label class="field"><input id="rnI" value="${esc(t.title)}" maxlength="80"></label><div class="row-end"><button class="btn primary" id="rnGo">Save</button></div>`, pop => {
      const go = async () => { try { await invoke("chat_rename", { id, title: $("#rnI", pop).value }); closePop(); } catch (e) { toast(String(e), "err"); } };
      $("#rnGo", pop).onclick = go; $("#rnI", pop).onkeydown = e => { if (e.key === "Enter") go(); };
      $("#rnI", pop).select();
    });
  }

  // ---------------------------------------------------------------- header
  function renderHead() {
    if (whirl) return;
    const h = $("#chatHead");
    if (!C.t) { h.innerHTML = `<span class="ch-title">New chat</span>`; return; }
    const r = C.t.route, a = Providers.account;
    const meter = r.kind === "cloud" && a ? `<span class="meterpill" title="${a.used} of ${a.allowance} replies this ${a.period}">${esc(a.plan[0].toUpperCase() + a.plan.slice(1))} · ${a.used}/${a.allowance}</span>` : "";
    h.innerHTML = `${Providers.avatar(avId(r), "md")}<div class="ch-t"><button class="ch-title" id="chRen" title="Rename">${esc(C.t.title)}</button><span class="ch-sub">${esc(routeName(r))}${C.t.branched_from ? " · branch" : ""}</span></div>${meter}
      <button class="ib" id="chPin" title="${C.t.pinned ? "Unpin" : "Pin"}" aria-pressed="${C.t.pinned}">${icon("pin")}</button>
      <button class="ib" id="chMore" title="More" data-popper>${icon("more")}</button>`;
    $("#chRen").onclick = () => rename(C.t.id);
    $("#chPin").onclick = () => invoke("chat_pin", { id: C.t.id, pinned: !C.t.pinned });
    $("#chMore").onclick = e => threadMenu(e.currentTarget, C.t.id);
  }

  // ---------------------------------------------------------------- body
  function render() {
    if (whirl || mountWhirl()) { window.WhirlChat.refresh(); return; }
    const el = $("#chat");
    renderHead();
    if (!el.querySelector(".chat-wrap")) {
      el.innerHTML = `<div class="chat-wrap"><div class="msgs" id="msgs" role="log" aria-live="polite"></div><div class="sugg" id="sugg"></div><div class="composer" id="composer"></div></div>`;
      wireComposer();
    }
    const msgs = $("#msgs");
    const anyRoute = flat().length > 0 || (C.t && usable(C.t.route));
    el.classList.toggle("is-home", !C.t);
    if (!C.t) {
      C.html.clear();
      msgs.innerHTML = anyRoute ? homeHTML() : noModelsHTML();
      wireHome(msgs);
      $("#composer").hidden = !anyRoute;
      $("#sugg").innerHTML = "";
    } else {
      if (msgs.querySelector(".chat-home, .nomodels")) msgs.innerHTML = "";
      $("#composer").hidden = false;
      renderMsgs();
      renderSugg();
    }
    renderComposer();
  }

  function homeHTML() {
    const r = C.homeRoute || defaultRoute();
    const hour = new Date().getHours();
    const hi = hour < 5 ? "Up late?" : hour < 12 ? "Good morning" : hour < 18 ? "Good afternoon" : "Good evening";
    const cards = [
      ["Explain a concept", "Explain how a hash map works, with a tiny example"],
      ["Draft something", "Write a friendly note declining a meeting"],
      ["Debug with me", "Why might a React effect run twice in development?"],
      ["Plan a project", "Plan a weekend project to learn Rust, step by step"],
    ];
    return `<div class="chat-home">
      <div class="hello">${Providers.avatar(avId(r), "lg")}<h1>${hi}</h1><p>Chatting with <button class="lnk" id="homeRoute" data-popper>${esc(routeName(r))}</button>. Your chats stay on this machine.</p></div>
      <div class="starters">${cards.map(([t, p]) => `<button class="starter" data-p="${esc(p)}"><b>${esc(t)}</b><span>${esc(p)}</span></button>`).join("")}</div>
    </div>`;
  }
  function noModelsHTML() {
    return `<div class="nomodels"><div class="home-mark">${icon("chip")}</div><h1>Connect a model to start chatting</h1>
      <p class="home-lead">Use a coding CLI you already pay for (Claude, Codex, Cursor, Grok, OpenCode), models on this machine through Ollama, any OpenAI-compatible router, or Backspace Cloud with nothing to install.</p>
      <div class="home-actions"><button class="btn primary big" id="nmSetup">${icon("bolt")}Run setup</button><button class="btn big" id="nmCloud">${icon("cloud")}Start free with Cloud</button></div></div>`;
  }
  function wireHome(box) {
    const hr = $("#homeRoute", box);
    if (hr) hr.onclick = e => routePicker(e.currentTarget, C.homeRoute || defaultRoute(), r => { C.homeRoute = r; render(); });
    $$(".starter", box).forEach(b => (b.onclick = () => { const ta = $("#cIn"); ta.value = b.dataset.p; grow(ta); ta.focus(); renderComposer(); }));
    const s = $("#nmSetup", box); if (s) s.onclick = () => Onboard.open();
    const c = $("#nmCloud", box); if (c) c.onclick = () => openSettings("plan");
  }

  function msgHTML(m, i, ms) {
    const prev = ms[i - 1], next = ms[i + 1];
    const gap = (a, b) => Math.abs(a.at - b.at) > 10 * 60e3;
    const start = !prev || prev.role !== m.role || gap(prev, m);
    const end = !next || next.role !== m.role || gap(m, next);
    const me = m.role === "user";
    const sep = !prev || Math.abs(m.at - prev.at) > 60 * 60e3 ? `<div class="when">${stamp(m.at)}</div>` : "";
    const lastUser = me && !ms.slice(i + 1).some(x => x.role === "user");
    let quote = "";
    if (m.reply_to) {
      const q = ms.find(x => x.id === m.reply_to);
      if (q) quote = `<div class="quote ${me ? "me" : ""}" data-jump="${q.id}">${icon("reply")}<span>${esc(q.text.slice(0, 140) || "Attachment")}</span></div>`;
    }
    const atts = (m.attachments || []).map(a => a.mime.startsWith("image/")
      ? `<img class="att-img" data-att="${esc(a.id)}" alt="${esc(a.name)}">`
      : `<div class="att-file">${icon("doc")}<span>${esc(a.name)}<small>${(a.size / 1024).toFixed(a.size > 10240 ? 0 : 1)} KB</small></span></div>`).join("");
    const waiting = !me && (m.status === "sending" || (m.status === "streaming" && !m.text));
    let body;
    if (waiting) body = `<div class="bub them typingb${end ? " tail" : ""}"><span class="typing"><i></i><i></i><i></i></span></div>`;
    else if (m.text || !atts) {
      const content = me ? plainText(m.text) : md(m.text);
      const caret = m.status === "streaming" ? '<span class="caret"></span>' : "";
      body = `<div class="bub ${me ? "me" : "them"}${end ? " tail" : ""}${m.status === "error" ? " failed" : ""}${/^\p{Extended_Pictographic}{1,3}$/u.test(m.text.trim()) ? " jumbo" : ""}">${content}${caret}${m.reactions.length ? `<span class="tap">${m.reactions.join("")}</span>` : ""}</div>`;
    } else body = "";
    const attBlock = atts ? `<div class="atts ${me ? "me" : ""}">${atts}${!m.text && m.reactions.length ? `<span class="tap">${m.reactions.join("")}</span>` : ""}</div>` : "";
    const url = m.status === "done" ? firstUrl(m.text) : null;
    const lp = url ? `<div class="lp ${me ? "me" : ""}" data-url="${esc(url)}"></div>` : "";
    const tools = waiting ? "" : `<div class="mtools ${me ? "me" : ""}">
      <button class="mt" data-act="tap" title="Tapback" data-popper>${"♡"}</button>
      <button class="mt" data-act="reply" title="Reply">${icon("reply")}</button>
      <button class="mt" data-act="copy" title="Copy">${icon("copy")}</button>
      ${me ? `<button class="mt" data-act="edit" title="Edit and resend">${icon("edit")}</button>` : `<button class="mt" data-act="retry" title="Ask again">${icon("reload")}</button>`}
      <button class="mt" data-act="branch" title="Branch from here">${icon("branch")}</button></div>`;
    let after = "";
    if (m.status === "error") after = `<div class="fail">${icon("warn")}<span><b>Not delivered.</b> ${esc(m.error || "")}</span><button class="lnk" data-act="retry">Try again</button></div>`;
    else if (m.status === "stopped") after = `<div class="meta them">Stopped · <button class="lnk" data-act="retry">Continue</button></div>`;
    else if (!me && end && m.status === "done") after = `<div class="meta them">${esc(m.model || providerName(C.t.route))}${m.cost_usd ? ` · $${m.cost_usd.toFixed(4)}` : ""}</div>`;
    else if (lastUser && end) after = `<div class="meta me">${next ? "Delivered" : "Sending…"}${m.edited ? " · Edited" : ""}</div>`;
    const ad = m.ad && !C.dismissedAds.has(m.id) ? adHTML(m) : "";
    const av = !me && end ? `<span class="mav">${Providers.avatar(avId(C.t.route), "xs")}</span>` : !me ? `<span class="mav"></span>` : "";
    return `${sep}<div class="msg ${me ? "me" : "them"}${start ? " start" : ""}${end ? " end" : ""}" data-mid="${m.id}">
      ${av}<div class="mcol">${quote}${attBlock}${body}${lp}${tools}${after}${ad}</div></div>`;
  }
  function adHTML(m) {
    const a = m.ad;
    return `<aside class="ad" aria-label="Sponsored"><div class="ad-h"><span class="ad-l">Sponsored</span><span class="ad-adv">${esc(a.advertiser)}</span>
      <button class="ad-x" data-adx="${m.id}" title="Hide this ad" aria-label="Hide ad">${icon("close")}</button></div>
      <div class="ad-t">${esc(a.title)}</div><div class="ad-b">${esc(a.body)}</div>
      <div class="ad-f"><button class="btn sm" data-adgo="${esc(a.url)}">${esc(a.cta)}</button><button class="lnk" data-noads title="You're on a plan with ads. Ads are not chosen from your messages.">Why this ad? · Remove ads</button></div></aside>`;
  }

  function renderMsgs() {
    const box = $("#msgs"); if (!box || !C.t) return;
    const stick = box.scrollHeight - box.scrollTop - box.clientHeight < 60 || box.dataset.tid !== C.t.id;
    if (box.dataset.tid !== C.t.id) { box.innerHTML = ""; C.html.clear(); box.dataset.tid = C.t.id; }
    const ms = C.t.messages;
    if (!ms.length) {
      box.innerHTML = `<div class="thread-empty">${Providers.avatar(avId(C.t.route), "lg")}<b>${esc(routeName(C.t.route))}</b><span>Say hello. Replies arrive here as they're written.</span></div>`;
      C.html.clear(); return;
    }
    const empty = box.querySelector(".thread-empty"); if (empty) empty.remove();
    const seen = new Set();
    let prevEl = null;
    ms.forEach((m, i) => {
      const h = msgHTML(m, i, ms);
      seen.add(m.id);
      let wrap = box.querySelector(`:scope > [data-w="${m.id}"]`);
      if (!wrap) {
        wrap = document.createElement("div"); wrap.dataset.w = m.id; wrap.className = "mw fresh";
        if (prevEl) prevEl.after(wrap); else box.prepend(wrap);
        setTimeout(() => wrap.classList.remove("fresh"), 400);
      }
      if (C.html.get(m.id) !== h) { wrap.innerHTML = h; C.html.set(m.id, h); wireMsg(wrap, m); }
      prevEl = wrap;
    });
    $$(":scope > [data-w]", box).forEach(w => { if (!seen.has(w.dataset.w)) { w.remove(); C.html.delete(w.dataset.w); } });
    fillAttachments(box); fillPreviews(box);
    if (stick) box.scrollTop = box.scrollHeight;
  }

  function wireMsg(wrap, m) {
    $$("[data-act]", wrap).forEach(b => (b.onclick = e => act(b.dataset.act, m, e.currentTarget)));
    $$("a[data-href]", wrap).forEach(a => (a.onclick = e => { e.preventDefault(); invoke("open_url", { url: a.dataset.href }); }));
    $$(".copyc", wrap).forEach(b => (b.onclick = () => { navigator.clipboard.writeText(b.closest(".code").querySelector("code").textContent); b.innerHTML = icon("check") + "Copied"; setTimeout(() => (b.innerHTML = icon("copy") + "Copy"), 1400); }));
    $$("[data-jump]", wrap).forEach(q => (q.onclick = () => { const t = $(`[data-w="${q.dataset.jump}"]`); if (t) { t.scrollIntoView({ block: "center", behavior: "smooth" }); t.classList.add("flash"); setTimeout(() => t.classList.remove("flash"), 1200); } }));
    $$("[data-adx]", wrap).forEach(b => (b.onclick = () => { C.dismissedAds.add(m.id); C.html.delete(m.id); renderMsgs(); }));
    $$("[data-adgo]", wrap).forEach(b => (b.onclick = () => go(b.dataset.adgo)));
    $$("[data-noads]", wrap).forEach(b => (b.onclick = () => openSettings("plan")));
    const bub = $(".bub", wrap);
    if (bub) bub.ondblclick = () => react(m, "❤️");
  }
  function go(url) {
    if (url === "backspace://plans") openSettings("plan");
    else if (url.startsWith("backspace://settings/")) openSettings(url.split("/").pop());
    else invoke("open_url", { url });
  }
  async function react(m, e) { await invoke("chat_react", { id: C.t.id, msg: m.id, emoji: e }).catch(err => toast(String(err), "err")); }
  async function act(a, m, el) {
    const id = C.t.id;
    if (a === "tap") {
      openPop(el, `<div class="taps">${TAPBACKS.map(e => `<button class="tapb${m.reactions.includes(e) ? " on" : ""}" data-e="${e}">${e}</button>`).join("")}</div>`, pop => $$("[data-e]", pop).forEach(b => (b.onclick = () => { closePop(); react(m, b.dataset.e); })));
    } else if (a === "reply") { C.reply = m.id; C.editing = null; renderComposer(); $("#cIn").focus(); }
    else if (a === "copy") { await navigator.clipboard.writeText(m.text); toast("Copied", "ok"); }
    else if (a === "edit") { C.editing = m.id; C.reply = null; const ta = $("#cIn"); ta.value = m.text; grow(ta); ta.focus(); renderComposer(); }
    else if (a === "retry") { await invoke("chat_retry", { id }).catch(e => toast(String(e), "err")); }
    else if (a === "branch") {
      try { const t = await invoke("chat_branch", { id, msg: m.id }); toast("Branched into a new chat", "ok"); await open(t.id); } catch (e) { toast(String(e), "err"); }
    }
  }

  async function fillAttachments(box) {
    for (const img of $$("img[data-att]:not([src])", box)) {
      const key = C.t.id + "/" + img.dataset.att;
      let url = C.fileUrls.get(key);
      if (!url) { url = await invoke("chat_file", { id: C.t.id, file: img.dataset.att }).catch(() => ""); C.fileUrls.set(key, url); }
      if (url) { img.src = url; img.onclick = () => lightbox(url); }
    }
  }
  function lightbox(url) {
    const d = document.createElement("div"); d.className = "lightbox"; d.innerHTML = `<img src="${url}" alt="">`;
    d.onclick = () => d.remove(); document.body.appendChild(d);
  }
  async function fillPreviews(box) {
    for (const el of $$(".lp[data-url]:not(.done)", box)) {
      const url = el.dataset.url; el.classList.add("done");
      let p = C.previews.get(url);
      if (p === undefined) { C.previews.set(url, null); p = await invoke("link_preview", { url }).catch(() => false); C.previews.set(url, p); }
      else if (p === null) { setTimeout(() => { el.classList.remove("done"); fillPreviews(box); }, 600); continue; }
      if (!p || (!p.title && !p.image)) { el.remove(); continue; }
      el.innerHTML = `<button class="lpc">${p.image ? `<span class="lpi" style="background-image:url('${esc(p.image)}')"></span>` : ""}<span class="lpt"><b>${esc(p.title || p.site)}</b>${p.description ? `<span>${esc(p.description.slice(0, 140))}</span>` : ""}<small>${esc(p.site)}</small></span></button>`;
      el.querySelector(".lpc").onclick = () => invoke("open_url", { url });
    }
  }

  // RCS-style suggested replies under the last answer.
  function renderSugg() {
    const s = $("#sugg"); if (!s) return;
    const ms = C.t ? C.t.messages : [];
    const last = ms[ms.length - 1];
    if (!last || last.role !== "assistant" || last.status !== "done" || $("#cIn").value.trim()) { s.innerHTML = ""; return; }
    const t = last.text.trim();
    let chips = /\?\s*$/.test(t) ? ["Yes", "No", "Tell me more"] : ["Tell me more", "Give me an example", "Make it shorter"];
    if (/```/.test(t)) chips = ["Explain this code", "Add tests", "Make it simpler"];
    s.innerHTML = chips.map(c => `<button class="schip">${esc(c)}</button>`).join("");
    $$(".schip", s).forEach(b => (b.onclick = () => send(b.textContent)));
  }

  // ---------------------------------------------------------------- composer
  function grow(ta) { ta.style.height = "auto"; ta.style.height = Math.min(220, ta.scrollHeight) + "px"; }
  function curRoute() { return C.t ? C.t.route : C.homeRoute || defaultRoute(); }
  function renderComposerRoute() {
    if (whirl) { window.WhirlChat.refresh(); return; }
    const b = $("#cRoute"); if (!b) return;
    const r = curRoute();
    b.innerHTML = `${Providers.avatar(avId(r), "xs")}<span>${esc(routeName(r))}</span>${icon("chevd")}`;
    const warn = $("#cWarn");
    if (warn) {
      const off = r && !usable(r);
      warn.hidden = !off;
      if (off) warn.innerHTML = `${icon("warn")}${esc(providerName(r))} is switched off or not set up. <button class="lnk" id="cFix">Pick another</button>`;
      const fx = $("#cFix"); if (fx) fx.onclick = () => $("#cRoute").click();
    }
    const a = Providers.account, u = $("#cUsage");
    if (u) u.innerHTML = r && r.kind === "cloud" && a ? `${esc(a.plan[0].toUpperCase() + a.plan.slice(1))} · ${a.used}/${a.allowance} ${a.period === "day" ? "today" : "this month"}${(a.models.find(m => m.id === r.model) || {}).with_ads ? " · with ads" : ""}` : "";
  }
  function renderComposer() {
    const c = $("#composer"); if (!c) return;
    const busy = C.t && C.threads.some(t => t.id === C.t.id && t.busy);
    const ctx = $("#cCtx");
    if (C.reply && C.t) {
      const m = C.t.messages.find(m => m.id === C.reply);
      ctx.innerHTML = `${icon("reply")}<span>Replying to <b>${m && m.role === "user" ? "yourself" : esc(providerName(C.t.route))}</b>: ${esc((m ? m.text : "").slice(0, 90))}</span><button class="ib" id="cCtxX" aria-label="Cancel reply">${icon("close")}</button>`;
    } else if (C.editing) ctx.innerHTML = `${icon("edit")}<span>Editing: sending asks again from here</span><button class="ib" id="cCtxX" aria-label="Cancel edit">${icon("close")}</button>`;
    else ctx.innerHTML = "";
    ctx.hidden = !ctx.innerHTML;
    const x = $("#cCtxX"); if (x) x.onclick = () => { if (C.editing) $("#cIn").value = ""; C.reply = null; C.editing = null; renderComposer(); };
    $("#cTray").innerHTML = C.files.map((f, i) => `<div class="tray-i">${f.mime.startsWith("image/") ? `<img src="${f.data}" alt="">` : `<span class="trf">${icon("doc")}${esc(f.name)}</span>`}<button class="tray-x" data-rmf="${i}" aria-label="Remove ${esc(f.name)}">${icon("close")}</button></div>`).join("");
    $("#cTray").hidden = !C.files.length;
    $$("[data-rmf]").forEach(b => (b.onclick = () => { C.files.splice(+b.dataset.rmf, 1); renderComposer(); }));
    const ta = $("#cIn"), btn = $("#cSend");
    const has = ta.value.trim() || C.files.length;
    btn.classList.toggle("stop", !!busy);
    btn.classList.toggle("on", !!(has || busy));
    btn.innerHTML = busy ? icon("stop") : icon("up");
    btn.title = busy ? "Stop" : "Send (Enter)";
    btn.setAttribute("aria-label", btn.title);
    ta.placeholder = C.t ? `Message ${providerName(C.t.route)}` : "Ask anything";
    renderComposerRoute();
  }
  function wireComposer() {
    $("#composer").innerHTML = `<div class="c-ctx" id="cCtx" hidden></div><div class="c-warn" id="cWarn" hidden></div><div class="c-tray" id="cTray" hidden></div>
      <div class="c-row"><button class="c-plus" id="cPlus" title="Attach images or files" aria-label="Attach">${icon("attach")}</button>
        <div class="c-box"><textarea id="cIn" rows="1" aria-label="Message" spellcheck="true"></textarea><button class="c-send" id="cSend" aria-label="Send">${icon("up")}</button></div></div>
      <div class="c-under"><button class="c-route" id="cRoute" data-popper title="Who answers"></button><span class="c-usage" id="cUsage"></span><span class="sp"></span><span class="c-hint">Enter to send · Shift+Enter for a new line</span></div>`;
    const ta = $("#cIn");
    ta.oninput = () => { grow(ta); C.drafts.set(C.id || "", ta.value); renderComposer(); renderSugg(); };
    ta.onkeydown = e => {
      if (e.key === "Enter" && !e.shiftKey && !e.isComposing) { e.preventDefault(); send(); }
      if (e.key === "Escape" && (C.reply || C.editing)) { e.stopPropagation(); if (C.editing) ta.value = ""; C.reply = null; C.editing = null; renderComposer(); }
      if (e.key === "ArrowUp" && !ta.value && C.t) { const m = [...C.t.messages].reverse().find(m => m.role === "user"); if (m) { e.preventDefault(); act("edit", m); } }
    };
    ta.onpaste = e => { const fs = [...(e.clipboardData || {}).files || []]; if (fs.length) { e.preventDefault(); addFiles(fs); } };
    $("#cSend").onclick = () => { const busy = C.t && C.threads.some(t => t.id === C.t.id && t.busy); if (busy) invoke("chat_stop", { id: C.t.id }); else send(); };
    $("#cPlus").onclick = () => $("#filePick").click();
    $("#filePick").onchange = e => { addFiles([...e.target.files]); e.target.value = ""; };
    $("#cRoute").onclick = e => routePicker(e.currentTarget, curRoute(), async r => {
      if (C.t) { await invoke("chat_set_route", { id: C.t.id, route: r }).catch(err => toast(String(err), "err")); toast(`Next replies come from ${routeName(r)}`, "ok"); }
      else { C.homeRoute = r; render(); }
    });
    const chat = $("#chat");
    chat.ondragover = e => { e.preventDefault(); chat.classList.add("drop"); };
    chat.ondragleave = e => { if (!chat.contains(e.relatedTarget)) chat.classList.remove("drop"); };
    chat.ondrop = e => { e.preventDefault(); chat.classList.remove("drop"); addFiles([...e.dataTransfer.files]); };
  }
  async function addFiles(fs) {
    for (const f of fs) {
      if (f.size > 10 << 20) { toast(`${f.name} is over 10 MB`, "err"); continue; }
      if (C.files.length >= 6) { toast("Up to 6 attachments per message", "err"); break; }
      const data = await new Promise(r => { const fr = new FileReader(); fr.onload = () => r(fr.result); fr.readAsDataURL(f); });
      C.files.push({ name: f.name, mime: f.type || "application/octet-stream", data, size: f.size });
    }
    renderComposer(); $("#cIn").focus();
  }

  async function send(text) {
    const ta = $("#cIn");
    const body = text != null ? text : ta.value;
    if (!body.trim() && !C.files.length) return;
    const route = curRoute();
    if (!route) { toast("Connect a model first", "err"); return; }
    if (!usable(route)) { toast(`${providerName(route)} isn't available. Pick another model.`, "err"); return; }
    const files = C.files.map(f => ({ name: f.name, mime: f.mime, data: f.data }));
    const reply = C.reply, editing = C.editing;
    ta.value = ""; grow(ta); C.files = []; C.reply = null; C.editing = null; C.drafts.delete(C.id || "");
    renderComposer();
    try {
      if (!C.t) { const t = await invoke("chat_new", { route }); C.id = t.id; C.t = t; render(); }
      if (editing) await invoke("chat_edit", { id: C.t.id, msg: editing, text: body });
      else await invoke("chat_send", { id: C.t.id, text: body, files, replyTo: reply });
    } catch (e) {
      toast(String(e), "err"); ta.value = body; grow(ta); renderComposer();
    }
    await refresh(); render();
  }

  // ---------------------------------------------------------------- data
  async function refresh() {
    if (whirl) {
      const a = await invoke("cloud_cached").catch(() => null);
      if (a) Providers.setAccount(a);
      return window.WhirlChat.refresh();
    }
    C.threads = await invoke("chat_list").catch(() => []);
    if (C.id) {
      C.t = await invoke("chat_thread", { id: C.id }).catch(() => null);
      if (!C.t) C.id = null;
      if (C.t && C.t.route.kind === "cloud") { const a = await invoke("cloud_cached").catch(() => null); if (a) Providers.setAccount(a); }
    }
    if (S.mode === "chat" && !S.settings) { renderThreads(); if ($("#chat .chat-wrap")) { renderHead(); if (C.t) { renderMsgs(); renderSugg(); } renderComposer(); } else render(); }
  }
  async function open(id) {
    if (whirl) { if (S.settings) closeSettings(); if (!Shell.showing("chat")) setMode("chat"); return window.WhirlChat.open(id); }
    C.id = id; C.reply = null; C.editing = null;
    C.t = await invoke("chat_thread", { id }).catch(() => null);
    if (S.settings) closeSettings();
    if (S.mode !== "chat") setMode("chat");
    const ta = $("#cIn");
    render();
    if (ta) { ta.value = C.drafts.get(id) || ""; grow(ta); renderComposer(); ta.focus(); }
    C.threads = await invoke("chat_list").catch(() => C.threads); renderThreads();
    if (narrow()) setSide(false);
  }
  function newChat() {
    if (whirl) { if (S.settings) closeSettings(); if (!Shell.showing("chat")) setMode("chat"); window.WhirlChat.home(); return; }
    C.id = null; C.t = null; C.reply = null; C.editing = null; C.homeRoute = null;
    if (S.settings) closeSettings();
    if (S.mode !== "chat") setMode("chat");
    render(); renderThreads();
    const ta = $("#cIn"); if (ta) { ta.value = C.drafts.get("") || ""; grow(ta); ta.focus(); }
  }
  async function init() {
    $("#newChat").onclick = newChat;
    $("#search").oninput = e => { C.q = e.target.value; renderThreads(); };
    $("#search").onkeydown = e => { if (e.key === "Escape") { C.q = ""; e.target.value = ""; renderThreads(); e.target.blur(); } if (e.key === "Enter") { const f = $("#list [data-t]"); if (f) f.click(); } };
    await Providers.load();
    await Providers.loadAccount();
    C.threads = await invoke("chat_list").catch(() => []);
  }

  return { init, refresh, render, renderThreads, renderComposerRoute, newChat, open, routeName, providerName, routePicker, md, options, flat, defaultRoute, usable, setScope };
})();
