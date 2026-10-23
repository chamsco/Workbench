// The window around the zones: the rail on the far left (Apps, Agents,
// Code, Memory, apps pinned from the catalog, Settings at the foot), the
// title bar's window buttons for each OS, and the split view that puts a
// second zone or app beside the first.
//
// Each zone draws into its own <section class="zone"> inside #body. Showing
// one never moves or rebuilds the others, so an app's iframe or a running
// chat keeps its state when it is out of sight.

var Shell = (() => {
  // Pages the frame owns. The apps come from extensions/index.json; order is
  // the reading order the window has always had — Apps, Agents, Code,
  // Memory — and each app's manifest carries its own number.
  const PAGES = [
    ["apps", "apps", "Apps", 0],
    ["memory", "memory", "Memory", 30],
  ];
  const zones = () => PAGES.concat((window.Frame ? Frame.zones() : [
    { id: "chat", name: "Chat", icon: "bubble", order: 10 },
    { id: "code", name: "Code", icon: "codei", order: 20 },
  ]).map(e => [e.id, e.icon, e.name, e.order])).sort((a, b) => a[3] - b[3]);
  let platform = "web", saveT = null;

  const isExt = z => !!(window.Frame && Frame.has(z));
  const isOff = z => isExt(z) && !Frame.enabled(z);
  const zoneName = z => z === "desktop" ? "Agent computer" : z.startsWith("app:") ? ((window.Apps && Apps.get(z.slice(4))) || { name: "App" }).name : (zones().find(x => x[0] === z) || [0, 0, z])[2];

  // ---------------------------------------------------------------- panes
  // Which section shows a zone: Code is the workbench, or the chat face
  // scoped to the project in Pair (until a project is open).
  function elFor(z) {
    // An app says which section is its face right now (Workbench is the canvas
    // grid in the IDE, the chat face in the ADE).
    if (isExt(z)) return Frame.elFor(z);
    if (z === "memory") return "memory";
    if (z === "apps") return "apps";
    if (z === "desktop") return $("#desktop") ? "desktop" : "chat";
    if (z.startsWith("app:") && window.Apps) return Apps.frame(z.slice(4)) || "apps";
    return "chat";
  }
  const secondary = () => {
    const sp = prefs.split;
    if (!sp || !sp.what) return null;
    if (sp.what.startsWith("app:") && !(window.Apps && Apps.get(sp.what.slice(4)))) return null;
    // The chat face is one view: it can't sit on both sides.
    const id = sp.what === "code" ? "canvas" : elFor(sp.what);
    return id === elFor(S.mode) ? null : id;
  };
  function showing(id) {
    if (S.settings) return false;
    return elFor(S.mode) === id || secondary() === id;
  }

  let laid = null;
  function layout() {
    const prim = elFor(S.mode), sec = S.settings ? null : secondary();
    laid = prim;
    const r = sec ? Math.min(0.8, Math.max(0.2, prefs.split.ratio || 0.5)) : 1;
    for (const el of $$("#body > .zone")) {
      if (el.id === prim && !S.settings) { el.hidden = false; el.style.left = "0"; el.style.right = (1 - r) * 100 + "%"; el.classList.remove("second"); }
      else if (el.id === sec) { el.hidden = false; el.style.left = r * 100 + "%"; el.style.right = "0"; el.classList.add("second"); }
      else el.hidden = true;
    }
    const gut = $("#splitGut"), head = $("#splitHead");
    gut.hidden = head.hidden = !sec;
    $("#win").classList.toggle("split-on", !!sec);
    if (sec) {
      gut.style.left = `calc(${r * 100}% - 3px)`;
      head.style.left = r * 100 + "%";
      const what = prefs.split.what;
      head.innerHTML = `<span class="sh-t">${glyph(what, "xs")}${esc(zoneName(what))}</span><span class="sp"></span>
        <button class="ib" id="shSwap" title="Swap sides" aria-label="Swap sides">${icon("swaph")}</button>
        <button class="ib" id="shClose" title="Close split" aria-label="Close split">${icon("close")}</button>`;
      $("#shSwap").onclick = swap;
      $("#shClose").onclick = () => setSplit(null);
    }

    // Chat's scope follows whoever shows it: a project in Pair, else none.
    if (window.Chat && Chat.setScope) Chat.setScope(S.mode === "code" && prim === "chat" ? snap.workspace : null, S.mode === "chat" && S.chatView === "agents");
    // The face's sidebar is a tree of the project and the zone it is in, and it
    // reads both from here: tell it the shell moved. A nudge, not a fetch.
    if (window.WhirlChat && window.WhirlChat.shell) window.WhirlChat.shell();
    renderRail(); renderZones(); renderHead(); renderModes(); renderStatus();
    if (window.Code) Code.syncArt();
    if (window.RPanel) RPanel.render();
    // Whatever app owns a visible section gets to draw into it; the frame does
    // not know how. Chat mounts lazily here.
    if (window.Frame) Frame.paint(prim, sec);
    if (prim === "memory" || sec === "memory") window.Memory && Memory.refresh();
    if (prim === "apps" || sec === "apps") window.Apps && Apps.renderZone();
  }

  function setSplit(what, ratio) {
    prefs.split = what ? { what, ratio: ratio || (prefs.split && prefs.split.ratio) || 0.5 } : null;
    invoke("set_split", { split: prefs.split });
    layout();
    if (showing("canvas") && window.Code) Code.layout();
    if (showing("chat") && window.Chat) Chat.render();
  }
  function swap() {
    const sp = prefs.split; if (!sp) return;
    const was = S.mode;
    prefs.split = { what: was, ratio: 1 - sp.ratio };
    invoke("set_split", { split: prefs.split });
    setMode(sp.what);
  }
  function toggleSplit() {
    if (prefs.split) setSplit(null); else splitMenu($("#searchBtn"));
  }
  function splitMenu(anchor) {
    const opts = zones()
      .map(([z, ic, l]) => [z, icon(ic), l])
      .concat((window.Apps ? Apps.installed() : []).map(a => ["app:" + a.id, Apps.glyph(a, "xs"), a.name]))
      .filter(([z]) => elFor(z) !== elFor(S.mode));
    openPop(anchor, `<div class="ph-t">Open beside ${esc(zoneName(S.mode))}</div>
      ${opts.map(([z, ic, l]) => `<button class="pi" data-sp="${esc(z)}">${ic}${esc(l)}</button>`).join("")}
      ${prefs.split ? `<div class="psep"></div><button class="pi" data-sp="">${icon("close")}Close split</button>` : ""}`, pop => {
      $$("[data-sp]", pop).forEach(b => (b.onclick = () => { closePop(); setSplit(b.dataset.sp || null); }));
    });
  }

  // Drag the gap; double-click evens it out.
  function wireGutter() {
    const gut = $("#splitGut");
    gut.onpointerdown = e => {
      e.preventDefault(); gut.setPointerCapture(e.pointerId);
      const box = $("#body").getBoundingClientRect();
      $("#win").classList.add("dragging-split");
      gut.onpointermove = ev => { prefs.split.ratio = Math.min(0.8, Math.max(0.2, (ev.clientX - box.left) / box.width)); layout(); };
      gut.onpointerup = () => {
        gut.onpointermove = gut.onpointerup = null; $("#win").classList.remove("dragging-split");
        clearTimeout(saveT); saveT = setTimeout(() => invoke("set_split", { split: prefs.split }), 200);
      };
    };
    gut.ondblclick = () => { prefs.split.ratio = 0.5; layout(); invoke("set_split", { split: prefs.split }); };
  }

  // ---------------------------------------------------------------- rail
  function glyph(z, cls = "") {
    if (z.startsWith("app:") && window.Apps) { const a = Apps.get(z.slice(4)); if (a) return Apps.glyph(a, cls); }
    const zz = zones().find(x => x[0] === z);
    return zz ? icon(zz[1]) : "";
  }
  // Going to an app that is off turns it on: the dimmed icon is a way back,
  // not a dead end.
  function openZone(z) {
    if (S.settings) closeSettings();
    if (isOff(z)) Frame.setOn(z, true, true);
    setMode(z);
  }
  function renderRail() {
    const cur = S.settings ? "" : S.mode;
    $("#railZones").innerHTML = zones().map(([z, ic, l]) =>
      `<button class="rz${isOff(z) ? " off" : ""}" role="tab" data-z="${z}" aria-selected="${cur === z}" aria-label="${l}${isOff(z) ? " (off)" : ""}" data-tip="${l}"><span class="sq">${icon(ic)}${badge(z)}</span></button>`).join("");
    const pinned = window.Apps ? Apps.pinned() : [];
    $("#railApps").innerHTML = pinned.length ? `<div class="rsep"></div>` + pinned.map(a =>
      `<button class="rz rzapp" data-z="app:${esc(a.id)}" aria-selected="${cur === "app:" + a.id}" aria-label="${esc(a.name)}" data-tip="${esc(a.name)}"><span class="sq">${Apps.glyph(a)}</span></button>`).join("") : "";
    $$("#rail [data-z]").forEach(b => (b.onclick = () => openZone(b.dataset.z)));
    $("#gear").setAttribute("aria-selected", !!S.settings);
  }
  // The title bar's switcher: every app, side by side as icons; the open one
  // also shows its name. An app that is
  // off stays here, dimmed — click it to turn it back on. Then pinned apps.
  // Settings is the gear on the right.
  function renderZones() {
    const el = $("#zones"); if (!el) return;
    const z = S.settings ? "" : S.mode;
    const apps = window.Apps ? Apps.pinned() : [];
    el.innerHTML = zones().map(([k, ic, l]) => `<button role="tab" aria-selected="${z === k}" data-zz="${k}" title="${l}${isOff(k) ? " — off" : ""}" aria-label="${l}${isOff(k) ? " (off)" : ""}" class="${isOff(k) ? "off" : ""}">${icon(ic)}<span class="zl">${l}</span>${badge(k)}</button>`).join("")
      + apps.map(a => `<button role="tab" aria-selected="${z === "app:" + a.id}" data-zz="app:${esc(a.id)}" title="${esc(a.name)}" aria-label="${esc(a.name)}">${Apps.glyph(a, "xs")}<span class="zl">${esc(a.name)}</span></button>`).join("");
    $$("[data-zz]", el).forEach(x => (x.onclick = () => openZone(x.dataset.zz)));
    $("#gearBtn").setAttribute("aria-pressed", !!S.settings);
  }

  // Search: zones, projects, agents and chats, from the title bar or Ctrl K.
  // Threads with a project behind them belong to that project (Code → Pair) and
  // are reached from there; listing them here would put the same thread in both
  // zones, and open it in whichever one you happened to be in.
  async function search() {
    const anchor = $("#searchBtn");
    const chats = ((await invoke("chat_list").catch(() => null)) || []).filter(t => !t.app && !t.project && !t.agent && !t.members.length);
    const all = [
      ...zones().map(([k, ic, l]) => ({ ic, t: l, s: "Go to", go: () => openZone(k) })),
      ...(prefs.projects || []).map(p => ({ ic: "folder", t: base(p), s: p, go: () => openProject(p) })),
      ...((snap && snap.agents) || []).filter(a => a.kind !== "triage").map(a => ({ ic: "sparkle", t: a.id === 0 ? "Main agent" : a.title, s: `${snap.name} · ${a.status.replace(/_/g, " ")}`, go: () => { S.codeView = "agents"; setMode("code"); Code.openIn({ kind: "agent", agent: a.id }); } })),
      ...chats.map(t => ({ ic: "bubble", t: t.title || "Chat", s: "Chat", go: () => { setMode("chat"); if (window.Chat && Chat.open) Chat.open(t.id); } })),
    ];
    openPop(anchor, `<div class="srch"><label>${icon("search")}<input placeholder="Search projects, agents and chats" aria-label="Search"></label><div class="srch-r"></div></div>`, pop => {
      const inp = $("input", pop), out = $(".srch-r", pop);
      let hits = [], sel = 0;
      const draw = () => {
        const q = inp.value.trim().toLowerCase();
        hits = all.filter(x => !q || (x.t + " " + x.s).toLowerCase().includes(q)).slice(0, 12);
        sel = Math.min(sel, Math.max(0, hits.length - 1));
        out.innerHTML = hits.length ? hits.map((x, i) => `<button class="pi${i === sel ? " on" : ""}" data-i="${i}">${icon(x.ic)}<span class="st-t">${esc(x.t)}</span><span class="st-s">${esc(x.s)}</span></button>`).join("") : `<div class="note">Nothing matches.</div>`;
        $$("[data-i]", out).forEach(b => (b.onclick = () => { closePop(); hits[+b.dataset.i].go(); }));
      };
      inp.oninput = () => { sel = 0; draw(); };
      inp.onkeydown = e => {
        if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); sel = (sel + (e.key === "ArrowDown" ? 1 : -1) + hits.length) % Math.max(1, hits.length); draw(); }
        else if (e.key === "Enter" && hits[sel]) { closePop(); hits[sel].go(); }
        else if (e.key === "Escape") closePop();
      };
      draw();
    });
    const pop = $("#pop"), r = anchor.getBoundingClientRect();
    pop.style.left = Math.max(8, r.right - pop.offsetWidth) + "px";
  }

  // Something waiting on you in Code: the count of approvals.
  function badge(z) {
    if (z === "code") { const n = typeof pending === "function" ? pending().length : 0; return n ? `<i class="rb">${n}</i>` : ""; }
    return "";
  }

  // ---------------------------------------------------------------- side + head
  // The switch in the title bar, the same in both zones: Chat (you and one
  // model, turn by turn) | Agents (named agents in Chat; the planner and its
  // workers in Code).
  //
  // Code's two surfaces are named for what they are (docs/ADE-AND-IDE.md): the
  // **ADE** is the agent development environment — one session with the
  // project's CLI — and the **IDE** is the project's own workspace, the
  // canvases, worktrees and tickets. "Chat | Agents" was Chat's axis, borrowed.
  // The stored values stay `pair` and `agents`, so no prefs file needs migrating.
  const VIEWS = {
    chat: [["chat", "bubble", "Chat", "One model, one conversation"], ["agents", "team", "Agents", "Your named agents, one to one or in groups"]],
    code: [["pair", "bubble", "ADE", "Agent development environment: you and one CLI, one session, in the project"], ["agents", "grid", "IDE", "The project: canvases, worktrees, tickets"]],
  };
  function renderModes() {
    const z = S.mode, v = VIEWS[z];
    const cur = z === "chat" ? S.chatView : S.codeView;
    const m = $("#modes");
    if (!m) return;
    m.hidden = !v;
    if (!v) { m.innerHTML = ""; return; }
    m.setAttribute("aria-label", v.map(x => x[2]).join(" or "));
    m.innerHTML = v.map(([k, ic, l, t]) => `<button role="tab" data-view="${k}" aria-selected="${cur === k}" title="${t}">${icon(ic)}${l}</button>`).join("");
    $$("[data-view]", m).forEach(b => (b.onclick = () => (z === "chat" ? setChatView(b.dataset.view) : setCodeView(b.dataset.view))));
  }
  function renderSide() {
    const z = S.mode, zl = $("#zoneList");
    renderModes();
    $("#zoneTitle").innerHTML = z === "code" || z === "chat" ? "" : `<h2>${esc(z.startsWith("app:") ? "Apps" : zoneName(z))}</h2>`;
    if (z === "memory") { window.Memory && Memory.renderSide(zl); }
    else if (z === "apps" || z.startsWith("app:")) { window.Apps && Apps.renderSide(zl); }
    else zl.innerHTML = "";
    if (z === "chat" || (z === "code" && S.codeView === "pair")) { if (window.Chat) Chat.renderThreads(); }
  }
  // Switch project: the recents, or another folder. One menu, and one way in —
  // the folder icon at the top of the sidebar. There used to be a project row
  // above it that said the same thing in a quarter of the width.
  function projectMenu(anchor) {
    const recents = (prefs.projects || []).filter(p => p !== (hasProject && snap.workspace)).slice(0, 6);
    openPop(anchor, `<div class="ph-t">Projects</div>${recents.map(p => `<button class="pi" data-pp="${esc(p)}">${icon("folder")}${esc(base(p))}</button>`).join("")}
      ${recents.length ? `<div class="psep"></div>` : ""}<button class="pi" id="ppNew">${icon("plus")}Open folder…</button>`, pop => {
      $$("[data-pp]", pop).forEach(b => (b.onclick = () => { closePop(); openProject(b.dataset.pp); }));
      $("#ppNew", pop).onclick = () => { closePop(); pickProject(); };
    });
  }
  // The window title (taskbar, alt-tab) and the head are the app's to say;
  // Code names the project, everything else is just the window.
  function syncWindowTitle() {
    const t = window.Frame ? Frame.windowTitle() : null;
    document.title = t || "Backspace";
  }
  function renderHead() {
    const z = S.mode, h = $("#zoneHead");
    // Apps and Memory carry their own H1 at the top of the sheet. Naming them
    // again here, a line above the page, was the title said twice.
    if (S.settings) h.innerHTML = `<span class="zt">${icon("gear")}Settings</span>`;
    else if (z.startsWith("app:") && window.Apps) h.innerHTML = Apps.head(z.slice(4));
    else if (window.Frame && Frame.head(z) != null) h.innerHTML = Frame.head(z) || "";
    else h.innerHTML = "";
    if (z.startsWith("app:") && window.Apps) Apps.wireHead(h, z.slice(4));
    const zh = $("#zhProj");
    if (zh) zh.onclick = () => projectMenu(zh);
    syncWindowTitle();
  }

  // ---------------------------------------------------------------- window buttons
  // macOS draws its own lights over the rail (titleBarStyle Overlay). Windows
  // and Linux run undecorated: Windows gets its caption buttons flush in the
  // top-right corner, Linux the round GNOME/KDE-style ones.
  function renderCaps() {
    const caps = $("#caps");
    if (platform !== "win" && platform !== "linux") { caps.innerHTML = ""; return; }
    caps.innerHTML = `<button data-cap="min" aria-label="Minimize" title="Minimize">${icon("wmin")}</button>` +
      `<button data-cap="max" aria-label="Maximize" title="Maximize">${icon("wmax")}</button>` +
      `<button data-cap="close" aria-label="Close" title="Close">${icon("wclose")}</button>`;
    const W = () => window.__TAURI__ && window.__TAURI__.window && window.__TAURI__.window.getCurrentWindow();
    $$("[data-cap]", caps).forEach(b => (b.onclick = async () => {
      const w = W(); if (!w) return;
      if (b.dataset.cap === "min") w.minimize();
      else if (b.dataset.cap === "max") { await w.toggleMaximize(); syncMax(); }
      else w.close();
    }));
    const syncMax = async () => {
      const w = W(); if (!w) return;
      const max = await w.isMaximized().catch(() => false);
      const b = $('[data-cap="max"]', caps);
      b.innerHTML = icon(max ? "wrest" : "wmax");
      b.title = b.ariaLabel = max ? "Restore" : "Maximize";
      document.documentElement.classList.toggle("maxed", max);
    };
    addEventListener("resize", () => requestAnimationFrame(syncMax));
    syncMax();
    // Undecorated GTK windows have no resize border of their own.
    if (platform === "linux") {
      const E = { n: "North", s: "South", e: "East", w: "West", ne: "NorthEast", nw: "NorthWest", se: "SouthEast", sw: "SouthWest" };
      const box = document.createElement("div"); box.className = "edges";
      box.innerHTML = Object.keys(E).map(k => `<div class="e-${k}" data-edge="${k}"></div>`).join("");
      document.body.append(box);
      $$("[data-edge]", box).forEach(d => (d.onpointerdown = e => { e.preventDefault(); const w = W(); if (w) w.startResizeDragging(E[d.dataset.edge]); }));
    }
  }

  async function init(boot) {
    platform = boot.platform || "web";
    document.documentElement.classList.add("os-" + platform);
    renderCaps(); wireGutter();
    $("#searchBtn kbd").textContent = platform === "mac" ? "⌘K" : "Ctrl K";
    document.documentElement.classList.toggle("reduce-motion", prefs.motion === "reduced");
    $("#searchBtn").onclick = search;
    $("#aiBtn").onclick = () => Ask.toggle();
    $("#gearBtn").onclick = () => (S.settings ? closeSettings() : openSettings());
    $("#gear").onclick = () => (S.settings ? closeSettings() : openSettings());
    if (window.Memory) await Memory.init();
    if (window.Apps) await Apps.init();
  }
  // The project arriving (at launch, or opened elsewhere) can change which
  // section Code shows; lay out again when it does.
  function refresh() { if (elFor(S.mode) !== laid) layout(); renderStatus(); renderRail(); renderZones(); if (S.mode === "code") renderHead(); }

  // ---------------------------------------------------------------- status bar
  // After an agent control plane's footer: where you are (branch, project),
  // who is working (only while something is), then the phone link, Motion and
  // what you have left to spend.
  let chatBusy = [];
  async function renderStatus() {
    const el = $("#status"); if (!el) return;
    const list = (await invoke("chat_list").catch(() => null)) || [];
    if (typeof watchChats === "function") watchChats(list);
    chatBusy = list.filter(t => t.busy);
    const ags = (snap && snap.agents) || [];
    const running = ags.filter(a => a.status && a.status.state === "running");
    const branch = ags[0] && ags[0].branch;
    const tickets = (snap && snap.tickets) || [];
    const done = tickets.filter(t => /merged|done|approved/i.test(JSON.stringify(t.state || ""))).length;
    const who = running.length
      ? `${running.length === 1 ? esc(running[0].title || "Main agent") : running.length + " agents"} working`
      : chatBusy.length ? `${esc(chatBusy[0].title.slice(0, 28))} replying${chatBusy.length > 1 ? ` +${chatBusy.length - 1}` : ""}` : "";
    const motion = prefs.motion === "reduced" ? "Reduced" : "Full";
    const phone = prefs.companion && prefs.companion.enabled;
    const u = await usage();
    el.innerHTML = `
      ${hasProject && snap.workspace ? `<button class="st" data-st="code" title="Open the project">${icon("branch")}${esc(branch || "no branch")}</button>
      <button class="st" data-st="code" title="${esc(snap.workspace)}">${icon("folder")}${esc(snap.name || base(snap.workspace))}${tickets.length ? `<span class="dim">${done}/${tickets.length} tickets</span>` : ""}</button>` : ""}
      ${who ? `<button class="st live" data-st="who"><i class="sd"></i>${who}</button>` : ""}
      <span class="sp"></span>
      ${prefs.split ? `<button class="st" data-st="split" title="Close the split">${icon("splitv")}Split</button>` : ""}
      <button class="st" data-st="phone" title="Settings → Phone">${icon("phone")}${phone ? "Phone on" : "Phone off"}</button>
      <button class="st" data-st="motion" title="Animations: full or reduced">${icon("motion")}Motion: ${motion}</button>
      <button class="st nb usage${u.low ? " low" : ""}" data-st="usage" title="${esc(u.tip)}">${u.pct != null ? `<span class="ubar"><i style="width:${Math.round(u.pct * 100)}%"></i></span>` : ""}${esc(u.txt)}</button>`;
    $$("[data-st]", el).forEach(b => (b.onclick = () => {
      const k = b.dataset.st;
      if (k === "code") setMode("code");
      else if (k === "who") { if (running.length) { S.codeView = "agents"; setMode("code"); } else if (chatBusy[0] && window.Chat) Chat.open(chatBusy[0].id); }
      else if (k === "split") setSplit(null);
      else if (k === "phone") openSettings("phone");
      else if (k === "motion") setMotion(prefs.motion === "reduced" ? "full" : "reduced");
      else if (k === "usage") openSettings(u.go);
    }));
  }

  // What is left, from whoever meters it: Backspace Cloud's allowance first,
  // then a CLI login's own quota report, else what this project has spent.
  // Nothing here is estimated — a quota only shows when its owner reported it.
  let accts = null, acctsAt = 0;
  async function usage() {
    const fmt = n => (n >= 1e6 ? (n / 1e6).toFixed(1) + "M" : n >= 1e3 ? (n / 1e3).toFixed(1) + "k" : String(n));
    const resets = ms => { const d = new Date(ms); return d.toDateString() === new Date().toDateString() ? d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }) : d.toLocaleDateString([], { month: "short", day: "numeric" }); };
    const cloud = window.Providers && Providers.account;
    if (cloud && cloud.allowance) {
      const left = Math.max(0, cloud.allowance - cloud.used);
      return { txt: `${fmt(left)} left`, pct: left / cloud.allowance, low: left / cloud.allowance < 0.15, go: "providers",
        tip: `Backspace Cloud: ${cloud.used} of ${cloud.allowance} replies used this ${cloud.period}, resets ${resets(cloud.resets_at)}${cloud.payg_usd ? ` · $${cloud.payg_usd.toFixed(2)} pay-as-you-go` : ""}` };
    }
    if (Date.now() - acctsAt > 30000) { acctsAt = Date.now(); accts = await invoke("accounts").then(d => d.accounts || []).catch(() => accts); }
    const live = (accts || []).filter(a => a.enabled && a.usage);
    const metered = live.find(a => a.usage.limit);
    if (metered) {
      const { used = 0, limit, resets_at } = metered.usage, left = Math.max(0, limit - (used || 0));
      return { txt: `${fmt(left)} left`, pct: left / limit, low: left / limit < 0.15, go: "accounts",
        tip: `${metered.name}: ${used || 0} of ${limit} used${resets_at ? `, resets ${resets(resets_at)}` : ""}${metered.usage.report ? ` · ${metered.usage.report}` : ""}` };
    }
    const capped = live.find(a => a.health && a.health.cooldown_until > Date.now());
    if (capped) return { txt: "Limit reached", pct: 0, low: true, go: "accounts", tip: `${capped.name}: ${capped.usage.report || "usage limit"}, usable again ${resets(capped.health.cooldown_until)}` };
    const ags = (snap && snap.agents) || [];
    const tok = ags.reduce((n, a) => n + (a.input_tokens || 0) + (a.output_tokens || 0), 0);
    const cost = (snap && snap.total_cost_usd) || 0;
    return { txt: cost ? `$${cost.toFixed(2)} used` : "Usage", pct: null, go: "accounts",
      tip: `No provider has reported a quota yet. This project: ${fmt(tok)} tokens${cost ? `, $${cost.toFixed(2)}` : ""}.` };
  }
  function setMotion(m) {
    prefs.motion = m;
    document.documentElement.classList.toggle("reduce-motion", m === "reduced");
    invoke("set_view_prefs", { chatView: null, motion: m });
    renderStatus();
  }

  // ---------------------------------------------------------------- memory toast
  // "Kira will remember that": dismissing keeps it (and teaches the scorer
  // it was right); "Don't remember that" deletes it and teaches it no.
  let memT = null;
  function memToast(note, who) {
    const el = $("#memToast"); if (!el || !note) return;
    clearTimeout(memT);
    el.innerHTML = `<span class="mt-ic">${icon("memory")}</span>
      <div class="mt-b"><b>${esc(who || "Backspace")} will remember that</b><span>${esc(note.text)}</span></div>
      <button class="mt-no" id="mtNo">Don't remember that</button>
      <button class="ib" id="mtX" aria-label="Dismiss">${icon("close")}</button>`;
    el.hidden = false;
    const close = () => { el.hidden = true; clearTimeout(memT); };
    $("#mtNo", el).onclick = async () => { close(); await invoke("memory_forget", { id: note.id }).catch(e => toast(String(e), "err")); toast("Forgotten. It will keep fewer like that.", "ok"); if (window.Memory) Memory.refresh(); };
    $("#mtX", el).onclick = () => { close(); invoke("memory_confirm", { id: note.id }).catch(() => {}); if (window.Memory) Memory.refresh(); };
    // Left alone, it stays saved; no feedback either way. Hovering holds it.
    const arm = () => { clearTimeout(memT); memT = setTimeout(() => { el.hidden = true; if (window.Memory) Memory.refresh(); }, 15000); };
    el.onmouseenter = () => clearTimeout(memT);
    el.onmouseleave = arm;
    arm();
  }

  // ---------------------------------------------------------------- agent desktops
  // An agent's computer with a web desktop opens beside the chat.
  function openDesktop(url) {
    let el = $("#desktop");
    if (!el) {
      el = document.createElement("section");
      el.className = "zone appz"; el.id = "desktop"; el.hidden = true;
      $("#body").insertBefore(el, $("#splitGut"));
    }
    el.innerHTML = `<iframe title="Agent computer" src="${esc(url)}" allow="clipboard-read; clipboard-write"></iframe>`;
    setSplit("desktop", 0.45);
  }

  return { projectMenu, search, init, layout, showing, renderStatus, memToast, openDesktop, setMotion, renderSide, renderRail, refresh, setSplit, toggleSplit, elFor, glyph };
})();
