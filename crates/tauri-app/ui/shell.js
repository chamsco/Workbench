// The window around the zones: the rail on the far left (Chat, Code,
// Memory, Apps, apps pinned from the catalog, Settings at the foot), the
// title bar's window buttons for each OS, and the split view that puts a
// second zone or app beside the first.
//
// Each zone draws into its own <section class="zone"> inside #body. Showing
// one never moves or rebuilds the others, so an app's iframe or a running
// chat keeps its state when it is out of sight.

var Shell = (() => {
  const ZONES = [
    ["chat", "bubble", "Chat"],
    ["code", "codei", "Code"],
    ["memory", "memory", "Memory"],
    ["apps", "apps", "Apps"],
  ];
  let platform = "web", saveT = null;

  const uses = () => (prefs.uses && prefs.uses.length ? prefs.uses : ["chat", "code"]);
  const zoneName = z => z === "desktop" ? "Agent computer" : z.startsWith("app:") ? ((window.Apps && Apps.get(z.slice(4))) || { name: "App" }).name : (ZONES.find(x => x[0] === z) || [0, 0, z])[2];

  // ---------------------------------------------------------------- panes
  // Which section shows a zone: Code is the workbench, or the chat face
  // scoped to the project in Pair (until a project is open).
  function elFor(z) {
    if (z === "code") return S.codeView === "pair" && hasProject && snap.workspace ? "chat" : "canvas";
    if (z === "chat") return "chat";
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

  function layout() {
    const prim = elFor(S.mode), sec = S.settings ? null : secondary();
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
    $("#splitBtn").setAttribute("aria-pressed", !!sec);
    // Chat's scope follows whoever shows it: a project in Pair, else none.
    if (window.Chat && Chat.setScope) Chat.setScope(S.mode === "code" && prim === "chat" ? snap.workspace : null, S.mode === "chat" && S.chatView === "agents");
    renderRail(); renderHead(); renderModes(); renderStatus();
    if ((prim === "chat" || sec === "chat") && window.Chat && !$("#chat").childElementCount) Chat.render();
    if (prim === "memory" || sec === "memory") window.Memory && Memory.refresh();
    if (prim === "apps" || sec === "apps") window.Apps && Apps.renderZone();
  }

  function setSplit(what, ratio) {
    prefs.split = what ? { what, ratio: ratio || (prefs.split && prefs.split.ratio) || 0.5 } : null;
    invoke("set_split", { split: prefs.split });
    layout();
    if (showing("canvas")) { renderTabs(); renderGrid(); }
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
    if (prefs.split) setSplit(null); else splitMenu($("#splitBtn"));
  }
  function splitMenu(anchor) {
    const opts = ZONES.filter(([z]) => (z !== "chat" && z !== "code") || uses().includes(z))
      .map(([z, ic, l]) => [z, icon(ic), l])
      .concat((window.Apps ? Apps.installed() : []).map(a => ["app:" + a.id, Apps.glyph(a, "xs"), a.name]))
      .filter(([z]) => (z === "code" ? "canvas" : elFor(z)) !== elFor(S.mode));
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
    const zz = ZONES.find(x => x[0] === z);
    return zz ? icon(zz[1]) : "";
  }
  function renderRail() {
    const cur = S.settings ? "" : S.mode;
    $("#railZones").innerHTML = ZONES.filter(([z]) => (z !== "chat" && z !== "code") || uses().includes(z)).map(([z, ic, l]) =>
      `<button class="rz" role="tab" data-z="${z}" aria-selected="${cur === z}" aria-label="${l}" data-tip="${l}"><span class="sq">${icon(ic)}${badge(z)}</span></button>`).join("");
    const pinned = window.Apps ? Apps.pinned() : [];
    $("#railApps").innerHTML = pinned.length ? `<div class="rsep"></div>` + pinned.map(a =>
      `<button class="rz rzapp" data-z="app:${esc(a.id)}" aria-selected="${cur === "app:" + a.id}" aria-label="${esc(a.name)}" data-tip="${esc(a.name)}"><span class="sq">${Apps.glyph(a)}</span></button>`).join("") : "";
    $$("#rail [data-z]").forEach(b => (b.onclick = () => { if (S.settings) closeSettings(); setMode(b.dataset.z); }));
    $("#gear").setAttribute("aria-selected", !!S.settings);
  }
  // Something waiting on you in Code: the count of approvals.
  function badge(z) {
    if (z === "code") { const n = typeof pending === "function" ? pending().length : 0; return n ? `<i class="rb">${n}</i>` : ""; }
    return "";
  }

  // ---------------------------------------------------------------- side + head
  // The switch at the top of the sidebar: Chat | Agents in Chat, Pair |
  // Agents in Code. The rail's first square lines up with it.
  const VIEWS = {
    chat: [["chat", "bubble", "Chat", "One model, one conversation"], ["agents", "team", "Agents", "Your named agents, one to one or in groups"]],
    code: [["pair", "pairi", "Pair", "You and one CLI, turn by turn, in the project"], ["agents", "sparkle", "Workbench", "A planner splits the goal into tickets; workers build each one"]],
  };
  function renderModes() {
    const z = S.mode, m = $("#modes"), v = VIEWS[z];
    m.hidden = !v;
    if (!v) { m.innerHTML = ""; return; }
    const cur = z === "chat" ? S.chatView : S.codeView;
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
    if (z === "code" && S.codeView === "pair") renderPairProj();
  }
  // Pair's sidebar: the open project (switch from the recents), then its threads.
  function renderPairProj() {
    const el = $("#pairProj");
    const cur = hasProject && snap.workspace;
    const recents = (prefs.projects || []).filter(p => p !== cur).slice(0, 6);
    el.innerHTML = (cur
      ? `<button class="row proj on" id="ppCur" data-popper title="${esc(cur)}">${icon("folder")}<span class="lab">${esc(base(cur))}</span>${icon("chev", "chev")}</button>`
      : `<button class="row add" id="ppOpen">${icon("plus")}<span class="lab">Open folder…</span><kbd>⌘O</kbd></button>`) +
      (!cur && recents.length ? `<div class="sec-t">Recent</div>` + recents.map(p => `<button class="row proj" data-pp="${esc(p)}" title="${esc(p)}">${icon("folder")}<span class="lab">${esc(base(p))}</span></button>`).join("") : "");
    const o = $("#ppOpen", el); if (o) o.onclick = pickProject;
    $$("[data-pp]", el).forEach(b => (b.onclick = () => openProject(b.dataset.pp)));
    const c = $("#ppCur", el);
    if (c) c.onclick = () => openPop(c, `<div class="ph-t">Projects</div>${recents.map(p => `<button class="pi" data-pp="${esc(p)}">${icon("folder")}${esc(base(p))}</button>`).join("")}
      <div class="psep"></div><button class="pi" id="ppNew">${icon("plus")}Open folder…</button>`, pop => {
      $$("[data-pp]", pop).forEach(b => (b.onclick = () => { closePop(); openProject(b.dataset.pp); }));
      $("#ppNew", pop).onclick = () => { closePop(); pickProject(); };
    });
  }
  function renderHead() {
    const z = S.mode, h = $("#zoneHead");
    if (S.settings) h.innerHTML = `<span class="zt">${icon("gear")}Settings</span>`;
    else if (z === "memory") h.innerHTML = `<span class="zt">${icon("memory")}Memory</span>`;
    else if (z === "apps") h.innerHTML = `<span class="zt">${icon("apps")}Apps</span>`;
    else if (z.startsWith("app:") && window.Apps) h.innerHTML = Apps.head(z.slice(4));
    else if (z === "code" && S.codeView === "pair") h.innerHTML = hasProject && snap.workspace ? `<span class="zt">${icon("pairi")}Pair<span class="sub">${esc(snap.name || base(snap.workspace))}</span></span>` : "";
    else h.innerHTML = "";
    if (z.startsWith("app:") && window.Apps) Apps.wireHead(h, z.slice(4));
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
    document.documentElement.classList.toggle("reduce-motion", prefs.motion === "reduced");
    $("#splitBtn").onclick = e => splitMenu(e.currentTarget);
    $("#gear").onclick = () => (S.settings ? closeSettings() : openSettings());
    if (window.Memory) await Memory.init();
    if (window.Apps) await Apps.init();
  }
  function refresh() { renderStatus(); renderRail(); if (S.mode === "code" && S.codeView === "pair") { renderPairProj(); renderHead(); } }

  // ---------------------------------------------------------------- status bar
  // After an agent control plane's footer: where you are (branch, project),
  // who is working, tokens and spend, then the phone link and Motion.
  let chatBusy = [];
  async function renderStatus() {
    const el = $("#status"); if (!el) return;
    const list = (await invoke("chat_list").catch(() => null)) || [];
    chatBusy = list.filter(t => t.busy);
    const ags = (snap && snap.agents) || [];
    const running = ags.filter(a => a.status && a.status.state === "running");
    const tok = ags.reduce((n, a) => n + (a.input_tokens || 0) + (a.output_tokens || 0), 0);
    const cost = (snap && snap.total_cost_usd) || 0;
    const branch = ags[0] && ags[0].branch;
    const tickets = (snap && snap.tickets) || [];
    const done = tickets.filter(t => /merged|done|approved/i.test(JSON.stringify(t.state || ""))).length;
    const who = running.length
      ? `${running.length === 1 ? esc(running[0].title || "Main agent") : running.length + " agents"} working`
      : chatBusy.length ? `${esc(chatBusy[0].title.slice(0, 28))} replying${chatBusy.length > 1 ? ` +${chatBusy.length - 1}` : ""}` : "Idle";
    const fmt = n => (n >= 1e6 ? (n / 1e6).toFixed(1) + "M" : n >= 1e3 ? (n / 1e3).toFixed(1) + "k" : String(n));
    const motion = prefs.motion === "reduced" ? "Reduced" : "Full";
    const phone = prefs.companion && prefs.companion.enabled;
    el.innerHTML = `
      ${hasProject && snap.workspace ? `<button class="st" data-st="code" title="Open the project">${icon("branch")}${esc(branch || "no branch")}</button>
      <button class="st" data-st="code" title="${esc(snap.workspace)}">${icon("folder")}${esc(snap.name || base(snap.workspace))}${tickets.length ? `<span class="dim">${done}/${tickets.length} tickets</span>` : ""}</button>` : ""}
      <button class="st ${running.length || chatBusy.length ? "live" : ""}" data-st="who"><i class="sd"></i>${who}</button>
      <span class="sp"></span>
      ${prefs.split ? `<button class="st" data-st="split" title="Close the split">${icon("splitv")}Split</button>` : ""}
      <button class="st" data-st="phone" title="Settings → Phone">${icon("phone")}${phone ? "Phone on" : "Phone off"}</button>
      <button class="st" data-st="motion" title="Animations: full or reduced">${icon("motion")}Motion: ${motion}</button>
      <span class="st nb" title="Tokens and spend in this project">${fmt(tok)} tok${cost ? ` · $${cost.toFixed(2)}` : ""}</span>`;
    $$("[data-st]", el).forEach(b => (b.onclick = () => {
      const k = b.dataset.st;
      if (k === "code") setMode("code");
      else if (k === "who") { if (running.length) { S.codeView = "agents"; setMode("code"); } else if (chatBusy[0] && window.Chat) Chat.open(chatBusy[0].id); }
      else if (k === "split") setSplit(null);
      else if (k === "phone") openSettings("phone");
      else if (k === "motion") setMotion(prefs.motion === "reduced" ? "full" : "reduced");
    }));
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

  return { init, layout, showing, renderStatus, memToast, openDesktop, setMotion, renderSide, renderRail, refresh, setSplit, toggleSplit, elFor, glyph };
})();
