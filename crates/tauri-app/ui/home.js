// Home: where work starts. A painting across the top dissolving into the
// page, one composer (what should your agents work on, in which project, on
// whom), and a grid of what's going on: who is working, what needs you, the
// project's git activity, tickets, recent projects and machines. The layout
// follows Berth's home (MIT; THIRD_PARTY_NOTICES.md).
//
// Home.render(grid) builds it once and then only refreshes the widgets, so
// typing and the painting survive state updates.

var Home = (() => {
  let stopBand = null, bandSrc = "", tab = "new", proj = null, activity = { path: "", at: 0, data: null };

  const working = () => treeOrder().map(agent).filter(a => a && a.kind !== "triage" && (a.status === "running" || a.status === "queued"));
  const project = () => proj || (hasProject && snap.workspace) || (prefs.projects || [])[0] || null;
  const choice = () => ({ model: prefs.worker || "", permission: prefs.cli_permission || "edits", effort: prefs.cli_effort || null });

  function shell() {
    return `<div class="home">
      <div class="home-band" aria-hidden="true"></div>
      <div class="home-hero">
        <h1>What should your agents work on?</h1>
        <div class="hc">
          <div class="hc-tabs" role="tablist"></div>
          <div class="hc-body"></div>
          <div class="hc-bar">
            <button class="hc-chip" data-act="proj" data-popper>${icon("folder")}<span class="v"></span>${icon("chevd")}</button>
            <span class="hc-chip quiet"><span class="dot done"></span><span class="mach"></span></span>
            <span class="hc-chip quiet">${icon("branch")}backspace/run</span>
            <span class="sp"></span>
            <button class="hc-chip" data-act="model" data-mpick>${icon("sparkle")}<span class="v"></span>${icon("chevd")}</button>
            <button class="hc-send" data-act="send" aria-label="Start">${icon("up")}</button>
          </div>
        </div>
      </div>
      <div class="home-grid"></div>
    </div>`;
  }

  function card(title, ic, count, body, extra = "") {
    return `<section class="hw">${`<header>${icon(ic)}<b>${title}</b>${count != null ? `<span class="n">${count}</span>` : ""}<span class="sp"></span>${extra}</header>`}<div class="hw-b">${body}</div></section>`;
  }

  function bars(days) {
    const max = Math.max(1, ...days);
    const names = ["S", "M", "T", "W", "T", "F", "S"], today = new Date().getDay();
    return `<div class="bars">${days.map((n, i) => `<div class="bar${i === days.length - 1 ? " now" : ""}" title="${n} commit${n === 1 ? "" : "s"}"><i style="height:${Math.max(n ? 6 : 2, (n / max) * 100)}%"></i><span>${names[(today - (days.length - 1 - i) + 70) % 7]}</span></div>`).join("")}</div>`;
  }

  async function loadActivity() {
    const p = project();
    if (!p || (activity.path === p && Date.now() - activity.at < 60_000)) return;
    activity = { path: p, at: Date.now(), data: null };
    activity.data = await invoke("git_activity", { path: p }).catch(() => null);
    if (activity.path === p) widgets();
  }

  function widgets() {
    const el = document.querySelector(".home-grid");
    if (!el) return;
    const w = working(), needs = snap.agents.map(a => [a, needsYou(a)]).filter(([, n]) => n);
    const t = snap.tickets || [];
    const by = sts => t.filter(x => sts.includes(x.state)).length;
    const act = activity.data;
    const projects = (prefs.projects || []).slice(0, 6);
    el.innerHTML = [
      card("Working now", "bolt", w.length, w.length ? w.map(a => `<button class="hw-row" data-open="${a.id}"><span class="spin-g">${a.status === "running" ? SPIN[spinI] : "·"}</span><span class="tt"><b>${esc(a.id === MAIN ? "Main agent" : a.title)}</b><span>${esc(lastLine(a))}</span></span><span class="meta">${esc(a.decision ? a.decision.model : "queued")}</span></button>`).join("") : `<div class="hw-empty">No agents running.</div>`),
      card("Needs you", "warn", needs.length || null, needs.length ? needs.map(([a, n]) => `<button class="hw-row" data-act-on="${a.id}"><span class="dot wait"></span><span class="tt"><b>${esc(a.id === MAIN ? "Main agent" : a.title)}</b><span>${esc(n.why)}</span></span><span class="go">${n.ap ? "Review" : "Open"}</span></button>`).join("") : `<div class="hw-empty">Nothing waiting on you.</div>`),
      card("Git activity", "branch", null, act ? `<div class="big">${act.days.reduce((a, b) => a + b, 0)}<span>commits · 14 days</span><span class="sp"></span><span class="add">+${act.added.toLocaleString()}</span><span class="del">−${act.removed.toLocaleString()}</span></div>${bars(act.days)}` : `<div class="hw-empty">${project() ? "Not a git repository yet." : "Open a project to see its activity."}</div>`),
      card("Tickets", "ticket", t.length || null, t.length ? `<div class="tix">${[["Needs you", ["proposed", "in_review", "ready_for_human", "needs_info"], "wait"], ["Running", ["queued", "in_progress"], "run"], ["Ready", ["ready_for_agent", "needs_triage"], ""], ["Done", ["done"], "done"], ["Failed", ["failed"], "fail"]].map(([l, sts, d]) => `<button class="tix-c" data-tix><span class="dot ${d}"></span><b>${by(sts)}</b><span>${l}</span></button>`).join("")}</div>` : `<div class="hw-empty">The main agent turns your goal into tickets.</div>`),
      card("Recent projects", "folder", null, projects.length ? projects.map(p => `<button class="hw-row" data-proj="${esc(p)}">${icon("folder")}<span class="tt"><b>${esc(base(p))}</b><span>${esc(p)}</span></span>${hasProject && p === snap.workspace ? `<span class="meta on">open</span>` : ""}</button>`).join("") : `<div class="hw-empty">No projects yet.</div>`, `<button class="lnk" data-act="openf">Open folder…</button>`),
      card("Machines", "pc", machines.length || 1, (machines.length ? machines : [machine()]).map(m => `<div class="hw-row static"><span class="dot ${m.local || m.link.state === "online" ? "done" : "fail"}"></span><span class="tt"><b>${esc(m.name)}</b><span>${m.local ? "This machine" : m.link.state}</span></span>${m.selected ? `<span class="meta on">showing</span>` : ""}</div>`).join("")),
    ].join("");
    el.querySelectorAll("[data-open]").forEach(b => (b.onclick = () => openIn({ kind: "agent", agent: +b.dataset.open })));
    el.querySelectorAll("[data-act-on]").forEach(b => (b.onclick = () => actOn(agent(+b.dataset.actOn))));
    el.querySelectorAll("[data-proj]").forEach(b => (b.onclick = () => { proj = b.dataset.proj; bar(); loadActivity(); }));
    el.querySelectorAll("[data-tix]").forEach(b => (b.onclick = () => setDrawer(true)));
    const of = el.querySelector("[data-act=openf]"); if (of) of.onclick = pickProject;
  }

  function tabs() {
    const n = working().length;
    document.querySelector(".hc-tabs").innerHTML = `<button role="tab" aria-selected="${tab === "new"}" data-tab="new">New task</button><button role="tab" aria-selected="${tab === "run"}" data-tab="run">Running agents${n ? `<span class="n">${n}</span>` : ""}</button>`;
    document.querySelectorAll(".hc-tabs [data-tab]").forEach(b => (b.onclick = () => { tab = b.dataset.tab; body(); tabs(); }));
  }

  function body() {
    const el = document.querySelector(".hc-body");
    if (tab === "new") {
      if (!el.querySelector("textarea")) {
        el.innerHTML = `<textarea rows="3" placeholder="Describe a task, a bug to fix, an idea to try…" aria-label="What should your agents work on?"></textarea>`;
        const ta = el.querySelector("textarea");
        ta.onkeydown = e => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey || !e.shiftKey)) { e.preventDefault(); send(); } };
      }
      return;
    }
    const w = working();
    el.innerHTML = `<div class="hc-run">${w.length ? w.map(a => `<button class="hw-row" data-open="${a.id}"><span class="spin-g">${SPIN[spinI]}</span><span class="tt"><b>${esc(a.id === MAIN ? "Main agent" : a.title)}</b><span>${esc(lastLine(a))}</span></span><span class="go" data-stop="${a.id}" title="Stop">${icon("stop")}</span></button>`).join("") : `<div class="hw-empty">Nothing running.</div>`}</div>`;
    el.querySelectorAll("[data-open]").forEach(b => (b.onclick = e => {
      const s = e.target.closest("[data-stop]");
      if (s) { e.stopPropagation(); invoke("stop", { agent: +s.dataset.stop }); return; }
      openIn({ kind: "agent", agent: +b.dataset.open });
    }));
  }

  async function bar() {
    const p = project();
    document.querySelector(".hc-bar [data-act=proj] .v").textContent = p ? base(p) : "Choose a project";
    document.querySelector(".hc-bar .mach").textContent = machine().name;
    document.querySelector(".hc-bar [data-act=model] .v").textContent = await Picker.label(choice().model);
  }

  function projMenu(anchor) {
    const ps = prefs.projects || [];
    openPop(anchor, `<div class="menu">${ps.map(p => `<button data-p="${esc(p)}">${icon("folder")}${esc(base(p))}</button>`).join("")}<button data-p="">${icon("plus")}Open folder…</button></div>`, pop => {
      pop.querySelectorAll("[data-p]").forEach(b => (b.onclick = () => { closePop(); if (!b.dataset.p) pickProject(); else { proj = b.dataset.p; bar(); activity.path = ""; loadActivity(); } }));
    });
  }

  // Applying a new choice needs the project reopened; only when nothing runs.
  async function choose(c) {
    prefs.worker = c.model || null; prefs.planner = c.model || null;
    prefs.cli_permission = c.permission; prefs.cli_effort = c.effort;
    await invoke("set_agent_run", { planner: prefs.planner, worker: prefs.worker, permission: c.permission, effort: c.effort });
    bar();
    if (hasProject && machine().local) {
      if (snap.agents.some(a => a.status === "running" || a.status === "awaiting_approval")) toast("Applies when you next open the project; agents are working.");
      else await invoke("open_project", { path: snap.workspace }).catch(e => toast(String(e), "err"));
    }
  }

  async function send() {
    const ta = document.querySelector(".hc-body textarea"), text = ta && ta.value.trim();
    if (!text) return;
    const p = project();
    if (!p) { pickProject(); return; }
    if (!hasProject || p !== snap.workspace) {
      try { await openProject(p); } catch (e) { toast(String(e), "err"); return; }
    }
    await invoke("send", { text });
    ta.value = "";
    setWb("inbox");
  }

  function band(home) {
    const bd = Art.current();
    const src = Art.src(["harbour", "dawn", "night", "open-sea", "fog"].includes(bd) ? bd : "harbour");
    if (src === bandSrc && home.querySelector(".home-band canvas")) return;
    if (stopBand) stopBand();
    bandSrc = src;
    stopBand = Art.band(home.querySelector(".home-band"), src, { position: 0.42, fade: 0.45, mute: Art.muteFor(src), bg: "--home-bg" });
  }

  function render(g) {
    let home = g.querySelector(".home");
    if (!home) {
      if (stopBand) { stopBand(); stopBand = null; bandSrc = ""; }
      g.className = "grid home-wrap";
      g.innerHTML = shell();
      home = g.querySelector(".home");
      home.querySelector("[data-act=proj]").onclick = e => projMenu(e.currentTarget);
      home.querySelector("[data-act=model]").onclick = e => Picker.open(e.currentTarget, choice(), choose);
      home.querySelector("[data-act=send]").onclick = send;
      body();
      requestAnimationFrame(() => { const ta = home.querySelector("textarea"); if (ta) ta.focus(); });
    }
    band(home);
    tabs();
    if (tab === "run") body();
    bar();
    widgets();
    loadActivity();
  }

  return { render, restyle: () => { bandSrc = ""; const h = document.querySelector(".home"); if (h) band(h); } };
})();
