// Home: where work starts. The window's painting runs behind it at full
// strength (art.js); on it, one composer (what should your agents work on,
// in which project, on whom), and a row of pills for what's going on: who is
// working, what needs you, tickets, git activity, recent projects. A pill
// opens its details underneath. After Berth's home and Zeron's new-session
// canvas (both MIT; THIRD_PARTY_NOTICES.md).
//
// Home.render(grid) builds it once and then only refreshes what changes, so
// typing survives state updates.

var Home = (() => {
  let proj = null, open = null, activity = { path: "", at: 0, data: null };

  const working = () => treeOrder().map(agent).filter(a => a && a.kind !== "triage" && (a.status === "running" || a.status === "queued"));
  const project = () => proj || (hasProject && snap.workspace) || (prefs.projects || [])[0] || null;
  const choice = () => ({ model: prefs.worker || "", permission: prefs.cli_permission || "edits", effort: prefs.cli_effort || null });
  const name = a => (a.id === MAIN ? "Main agent" : a.title);

  function shell() {
    return `<div class="home">
      <div class="home-hero">
        <h1>What should your agents work on?</h1>
        <div class="hc-above">
          <button class="hc-chip" data-act="proj" data-popper>${icon("folder")}<span class="v"></span>${icon("chevd")}</button>
          <span class="hc-chip quiet">${icon("pc")}<span class="mach"></span></span>
        </div>
        <div class="hc">
          <textarea rows="2" placeholder="Describe a task, a bug to fix, an idea to try…" aria-label="What should your agents work on?"></textarea>
          <div class="hc-bar">
            <span class="sp"></span>
            <button class="hc-model" data-act="model" data-mpick><span class="v"></span><span class="eff"></span>${icon("chevd")}</button>
            <button class="hc-send" data-act="send" aria-label="Start">${icon("up")}</button>
          </div>
        </div>
        <div class="hc-below"><span class="hc-chip quiet">${icon("branch")}backspace/run</span></div>
        <div class="home-pills" role="tablist" aria-label="What's going on"></div>
        <div class="home-open"></div>
      </div>
    </div>`;
  }

  // A sparkline of commits a day, for the git pill.
  function spark(days) {
    const max = Math.max(1, ...days), w = 3, gap = 1.5;
    return `<svg class="spark" viewBox="0 0 ${days.length * (w + gap)} 14" aria-hidden="true">${days.map((n, i) => { const h = Math.max(n ? 2.5 : 1, (n / max) * 14); return `<rect x="${i * (w + gap)}" y="${14 - h}" width="${w}" height="${h}" rx="0.8"/>`; }).join("")}</svg>`;
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
    if (activity.path === p) pills();
  }

  function pills() {
    const el = document.querySelector(".home-pills");
    if (!el) return;
    const w = working(), needs = snap.agents.map(a => [a, needsYou(a)]).filter(([, n]) => n), t = snap.tickets || [], act = activity.data;
    const commits = act ? act.days.reduce((a, b) => a + b, 0) : 0;
    const ps = (prefs.projects || []).slice(0, 3).map(base);
    const P = [
      ["work", `<span class="dot ${w.length ? "run" : ""}"></span>Working now<b>${w.length}</b>`],
      ["needs", `<span class="dot ${needs.length ? "wait" : ""}"></span>Needs you<b>${needs.length}</b>`],
      ["tix", `${icon("ticket")}Tickets<b>${t.length}</b>`],
      ["git", act ? `${icon("branch")}${spark(act.days)}<b>${commits}</b><span class="dim">commits · 14d</span><span class="add">+${fmtK(act.added)}</span><span class="del">−${fmtK(act.removed)}</span>` : `${icon("branch")}Git activity`],
      ["proj", `${icon("folder")}${ps.length ? esc(ps.join(" · ")) : "Projects"}`],
    ];
    el.innerHTML = P.map(([k, h]) => `<button class="hpill" role="tab" aria-selected="${open === k}" data-pill="${k}">${h}</button>`).join("");
    el.querySelectorAll("[data-pill]").forEach(b => (b.onclick = () => { open = open === b.dataset.pill ? null : b.dataset.pill; pills(); }));
    details(w, needs, t, act);
  }
  const fmtK = n => (n >= 1000 ? (n / 1000).toFixed(1) + "k" : String(n));

  // What the open pill holds.
  function details(w, needs, t, act) {
    const el = document.querySelector(".home-open");
    if (!open) { el.innerHTML = ""; el.hidden = true; return; }
    el.hidden = false;
    const by = sts => t.filter(x => sts.includes(x.state)).length;
    const row = (a, right) => `<button class="hw-row" data-open="${a.id}"><span class="dot ${dotFor(a.status)}"></span><span class="tt"><b>${esc(name(a))}</b><span>${esc(lastLine(a))}</span></span>${right}</button>`;
    el.innerHTML = {
      work: w.length ? w.map(a => row(a, `<span class="meta">${esc(a.decision ? a.decision.model : "queued")}</span><span class="go" data-stop="${a.id}" title="Stop">${icon("stop")}</span>`)).join("") : `<div class="hw-empty">No agents running. Describe a task above to start some.</div>`,
      needs: needs.length ? needs.map(([a, n]) => `<button class="hw-row" data-act-on="${a.id}"><span class="dot wait"></span><span class="tt"><b>${esc(name(a))}</b><span>${esc(n.why)}</span></span><span class="go">${n.ap ? "Review" : "Open"}</span></button>`).join("") : `<div class="hw-empty">Nothing waiting on you.</div>`,
      tix: t.length ? `<div class="tix">${[["Needs you", ["proposed", "in_review", "ready_for_human", "needs_info"], "wait"], ["Running", ["queued", "in_progress"], "run"], ["Ready", ["ready_for_agent", "needs_triage"], ""], ["Done", ["done"], "done"], ["Failed", ["failed"], "fail"]].map(([l, sts, d]) => `<button class="tix-c" data-tix><span class="dot ${d}"></span><b>${by(sts)}</b><span>${l}</span></button>`).join("")}</div>` : `<div class="hw-empty">The main agent turns your goal into tickets.</div>`,
      git: act ? `<div class="big">${act.days.reduce((a, b) => a + b, 0)}<span>commits in 14 days on every branch of ${esc(base(activity.path))}</span><span class="sp"></span><span class="add">+${act.added.toLocaleString()}</span><span class="del">−${act.removed.toLocaleString()}</span></div>${bars(act.days)}` : `<div class="hw-empty">${project() ? "Not a git repository yet." : "Open a project to see its activity."}</div>`,
      proj: (prefs.projects || []).slice(0, 8).map(p => `<button class="hw-row" data-proj="${esc(p)}">${icon("folder")}<span class="tt"><b>${esc(base(p))}</b><span>${esc(p)}</span></span>${hasProject && p === snap.workspace ? `<span class="meta on">open</span>` : p === project() ? `<span class="meta">chosen</span>` : ""}</button>`).join("") + `<button class="hw-row" data-act="openf">${icon("plus")}<span class="tt"><b>Open folder…</b></span></button>`,
    }[open];
    el.querySelectorAll("[data-open]").forEach(b => (b.onclick = e => {
      const s = e.target.closest("[data-stop]");
      if (s) { e.stopPropagation(); invoke("stop", { agent: +s.dataset.stop }); return; }
      openIn({ kind: "agent", agent: +b.dataset.open });
    }));
    el.querySelectorAll("[data-act-on]").forEach(b => (b.onclick = () => actOn(agent(+b.dataset.actOn))));
    el.querySelectorAll("[data-proj]").forEach(b => (b.onclick = () => { proj = b.dataset.proj; bar(); activity.path = ""; loadActivity(); pills(); }));
    el.querySelectorAll("[data-tix]").forEach(b => (b.onclick = () => setDrawer(true)));
    const of = el.querySelector("[data-act=openf]"); if (of) of.onclick = pickProject;
  }

  async function bar() {
    const p = project();
    document.querySelector(".hc-above [data-act=proj] .v").textContent = p ? base(p) : "Choose a project";
    document.querySelector(".hc-above .mach").textContent = machine().name;
    const c = choice();
    document.querySelector(".hc-model .v").textContent = await Picker.label(c.model);
    document.querySelector(".hc-model .eff").textContent = Effort.NAMES[c.effort || "high"];
  }

  function projMenu(anchor) {
    const ps = prefs.projects || [];
    openPop(anchor, `${ps.map(p => `<button class="pi" data-p="${esc(p)}">${icon("folder")}${esc(base(p))}</button>`).join("")}<div class="psep"></div><button class="pi" data-p="">${icon("plus")}Open folder…</button>`, pop => {
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
    const ta = document.querySelector(".hc textarea"), text = ta && ta.value.trim();
    if (!text) return;
    const p = project();
    if (!p) { pickProject(); return; }
    if (!hasProject || p !== snap.workspace) {
      await openProject(p);
      if (!hasProject || snap.workspace !== p) return;
    }
    await invoke("send", { text });
    ta.value = "";
    setWb("inbox");
  }

  function render(g) {
    let home = g.querySelector(".home");
    if (!home) {
      g.className = "grid home-wrap";
      g.innerHTML = shell();
      home = g.querySelector(".home");
      home.querySelector("[data-act=proj]").onclick = e => projMenu(e.currentTarget);
      // The menu opens under the composer, as wide as it.
      home.querySelector("[data-act=model]").onclick = e => Picker.open(e.currentTarget, choice(), choose, { under: home.querySelector(".hc") });
      home.querySelector("[data-act=send]").onclick = send;
      const ta = home.querySelector("textarea");
      ta.onkeydown = e => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); send(); } };
      ta.oninput = () => { ta.style.height = "auto"; ta.style.height = Math.min(220, ta.scrollHeight) + "px"; };
      requestAnimationFrame(() => ta.focus());
    }
    bar();
    pills();
    loadActivity();
  }

  return { render };
})();
