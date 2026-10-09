// Home: where work starts and where you watch it. The window's painting
// runs behind it at full strength (art.js); on it, one composer (what should
// your agents work on, in which project, on whom) whose model and effort
// picker unfolds from the composer itself, and under it the inbox: what
// needs you, who is working, who is idle, then tickets, git activity and
// projects. After Berth's home and Zeron's new-session canvas (MIT;
// THIRD_PARTY_NOTICES.md).
//
// Home.render(grid) builds it once and then only refreshes what changes, so
// typing survives state updates. It never scrolls as a page; the inbox
// scrolls inside itself.

var Home = (() => {
  let proj = null, open = "agents", picking = false, activity = { path: "", at: 0, data: null };

  const project = () => proj || (hasProject && snap.workspace) || (prefs.projects || [])[0] || null;
  const choice = () => ({ model: prefs.worker || "", permission: prefs.cli_permission || "edits", effort: prefs.cli_effort || null });
  const name = a => (a.id === MAIN ? "Main agent" : a.title);
  const visible = () => treeOrder().map(agent).filter(a => a && a.kind !== "triage");

  function shell() {
    return `<div class="home">
      <div class="home-hero">
        <h1>What should your agents work on?</h1>
        <div class="hc-above">
          <button class="hc-chip" data-act="proj" data-popper>${icon("folder")}<span class="v"></span>${icon("chevd")}</button>
          <span class="hc-chip quiet">${icon("branch")}<span class="br">backspace/run</span></span>
          <span class="hc-chip quiet">${icon("pc")}<span class="mach"></span></span>
        </div>
        <div class="hc">
          <textarea rows="2" placeholder="Describe a task, a bug to fix, an idea to try…" aria-label="What should your agents work on?"></textarea>
          <div class="hc-bar">
            <span class="hc-hint"><kbd>↵</kbd> start <kbd>Ctrl ↵</kbd> in parallel</span>
            <span class="sp"></span>
            <button class="hc-par" data-act="par" title="Start another task in parallel, on its own branch">${icon("branch")}Parallel</button>
            <button class="hc-model" data-act="model" aria-expanded="false"><span class="lg"></span><span class="v"></span><span class="eff"></span>${icon("chevd")}</button>
            <button class="hc-send" data-act="send" aria-label="Start">${icon("up")}</button>
          </div>
          <div class="hc-panel" hidden></div>
        </div>
        <div class="home-dock">
          <div class="home-pills" role="tablist" aria-label="What's going on"></div>
          <div class="home-open"></div>
        </div>
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
  const fmtK = n => (n >= 1000 ? (n / 1000).toFixed(1) + "k" : String(n));

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
    const all = visible(), needs = all.filter(needsYou), working = all.filter(a => !needsYou(a) && busy(a));
    const t = snap.tickets || [], act = activity.data, consults = all.reduce((n, a) => n + (a.advisor_calls || 0), 0);
    const ps = (prefs.projects || []).slice(0, 3).map(base);
    const P = [
      ["agents", `<span class="dot ${needs.length ? "wait" : working.length ? "run" : ""}"></span>Agents<b>${all.length}</b>${needs.length ? `<span class="need">${needs.length} need you</span>` : ""}`],
      ["tix", `${icon("ticket")}Tickets<b>${t.length}</b>`],
      ["git", act ? `${icon("branch")}${spark(act.days)}<b>${act.days.reduce((a, b) => a + b, 0)}</b><span class="dim">commits · 14d</span><span class="add">+${fmtK(act.added)}</span><span class="del">−${fmtK(act.removed)}</span>` : `${icon("branch")}Git activity`],
      ["proj", `${icon("folder")}${ps.length ? esc(ps.join(" · ")) : "Projects"}`],
    ];
    if (consults) P.push(["adv", `${icon("sparkle")}Advisor<b>${consults}</b><span class="dim">consult${consults === 1 ? "" : "s"}</span>`]);
    el.innerHTML = P.map(([k, h]) => `<button class="hpill" role="tab" aria-selected="${open === k}" data-pill="${k}">${h}</button>`).join("");
    el.querySelectorAll("[data-pill]").forEach(b => (b.onclick = () => { open = b.dataset.pill; pills(); }));
    details();
  }

  // The inbox, fused into Home: what needs you, who is working, who is idle.
  function inbox() {
    const all = visible();
    if (!all.length || (all.length === 1 && !all[0].log.some(e => e.kind === "user"))) {
      return `<div class="hw-empty">${hasProject ? "No agents yet. Describe a task above: the main agent plans it into tickets and workers build them." : "Pick a project above, then describe a task."}</div>`;
    }
    const needs = all.filter(needsYou), working = all.filter(a => !needsYou(a) && busy(a));
    const idle = all.filter(a => !needsYou(a) && !busy(a) && a.kind !== "scout");
    const row = (a, right) => `<div class="hw-row" data-open="${a.id}" role="button" tabindex="0"><span class="dot ${dotFor(a.status)}"></span><span class="tt"><b>${esc(name(a))}${a.kind === "scout" ? ` <i class="tag">scout</i>` : ""}</b><span>${esc(right.sub || lastLine(a))}</span></span>${right.html || ""}</div>`;
    const sec = (title, rows) => rows.length ? `<div class="grp-label">${title} · ${rows.length}</div>${rows.join("")}` : "";
    return sec("Needs you", needs.map(a => { const n = needsYou(a); return row(a, { sub: n.why, html: `<button class="btn sm${n.ap ? " primary" : ""}" data-act-on="${a.id}">${n.ap ? "Review" : "Open"}</button>` }); }))
      + sec("Working", working.map(a => row(a, { html: `<span class="meta">${esc(a.decision ? a.decision.model : "queued")}</span><button class="ib stopb" data-stop="${a.id}" aria-label="Stop" title="Stop">${icon("stop")}</button>` })))
      + sec("Idle", idle.map(a => row(a, { html: `<span class="meta">${esc(a.status.replace(/_/g, " "))}</span>` })));
  }

  function details() {
    const el = document.querySelector(".home-open");
    if (!el) return;
    const t = snap.tickets || [], act = activity.data;
    const by = sts => t.filter(x => sts.includes(x.state)).length;
    const keep = el.scrollTop;
    el.innerHTML = {
      agents: inbox,
      tix: () => t.length ? `<div class="tix">${[["Needs you", ["proposed", "in_review", "ready_for_human", "needs_info"], "wait"], ["Running", ["queued", "in_progress"], "run"], ["Ready", ["ready_for_agent", "needs_triage"], ""], ["Done", ["done"], "done"], ["Failed", ["failed"], "fail"]].map(([l, sts, d]) => `<button class="tix-c" data-tix><span class="dot ${d}"></span><b>${by(sts)}</b><span>${l}</span></button>`).join("")}</div>` : `<div class="hw-empty">The main agent turns your goal into tickets.</div>`,
      git: () => act ? `<div class="big">${act.days.reduce((a, b) => a + b, 0)}<span>commits in 14 days on every branch of ${esc(base(activity.path))}</span><span class="sp"></span><span class="add">+${act.added.toLocaleString()}</span><span class="del">−${act.removed.toLocaleString()}</span></div>${bars(act.days)}` : `<div class="hw-empty">${project() ? "Not a git repository yet." : "Open a project to see its activity."}</div>`,
      proj: () => (prefs.projects || []).slice(0, 8).map(p => `<div class="hw-row" data-proj="${esc(p)}" role="button" tabindex="0">${icon("folder")}<span class="tt"><b>${esc(base(p))}</b><span>${esc(p)}</span></span>${hasProject && p === snap.workspace ? `<span class="meta on">open</span>` : p === project() ? `<span class="meta">chosen</span>` : ""}</div>`).join("") + `<div class="hw-row" data-act="openf" role="button" tabindex="0">${icon("plus")}<span class="tt"><b>Open folder…</b></span></div>`,
      adv: () => visible().filter(a => a.advisor_calls).map(a => `<div class="hw-row" data-open="${a.id}" role="button" tabindex="0">${icon("sparkle")}<span class="tt"><b>${esc(name(a))}</b><span>consulted the advisor ${a.advisor_calls} time${a.advisor_calls === 1 ? "" : "s"}</span></span></div>`).join(""),
    }[open]();
    el.scrollTop = keep;
    el.querySelectorAll("[data-open]").forEach(b => (b.onclick = e => {
      const s = e.target.closest("[data-stop]"), r = e.target.closest("[data-act-on]");
      if (s) { e.stopPropagation(); invoke("stop", { agent: +s.dataset.stop }); return; }
      if (r) { e.stopPropagation(); actOn(agent(+r.dataset.actOn)); return; }
      openIn({ kind: "agent", agent: +b.dataset.open });
    }));
    el.querySelectorAll("[data-proj]").forEach(b => (b.onclick = () => { proj = b.dataset.proj; bar(); activity.path = ""; loadActivity(); pills(); }));
    el.querySelectorAll("[data-tix]").forEach(b => (b.onclick = () => setDrawer(true)));
    const of = el.querySelector("[data-act=openf]"); if (of) of.onclick = pickProject;
  }

  async function bar() {
    const p = project(), c = choice();
    document.querySelector(".hc-above [data-act=proj] .v").textContent = p ? base(p) : "Choose a project";
    document.querySelector(".hc-above .mach").textContent = machine().name;
    const main = agent(MAIN);
    document.querySelector(".hc-above .br").textContent = (hasProject && main && main.branch) || "backspace/run";
    document.querySelector(".hc-model .lg").innerHTML = Logos.mark(Picker.cliOf(c.model));
    document.querySelector(".hc-model .v").textContent = await Picker.label(c.model);
    document.querySelector(".hc-model .eff").textContent = Effort.name(Picker.cliOf(c.model), c.effort);
  }

  function projMenu(anchor) {
    const ps = prefs.projects || [];
    openPop(anchor, `${ps.map(p => `<button class="pi" data-p="${esc(p)}">${icon("folder")}${esc(base(p))}</button>`).join("")}<div class="psep"></div><button class="pi" data-p="">${icon("plus")}Open folder…</button>`, pop => {
      pop.querySelectorAll("[data-p]").forEach(b => (b.onclick = () => { closePop(); if (!b.dataset.p) pickProject(); else { proj = b.dataset.p; bar(); activity.path = ""; loadActivity(); } }));
    });
  }

  // A new choice applies when the project is next opened; reopen it now if
  // nothing is running.
  let reopenT = null;
  async function choose(c) {
    prefs.worker = c.model || null; prefs.planner = c.model || null;
    prefs.cli_permission = c.permission; prefs.cli_effort = c.effort;
    await invoke("set_agent_run", { planner: prefs.planner, worker: prefs.worker, permission: c.permission, effort: c.effort });
    bar();
    clearTimeout(reopenT);
    reopenT = setTimeout(async () => {
      if (!hasProject || !machine().local) return;
      if (snap.agents.some(busy)) toast("Applies when you next open the project; agents are working.");
      else await invoke("open_project", { path: snap.workspace }).catch(e => toast(String(e), "err"));
    }, 900);
  }

  // The picker unfolds from the composer; the inbox steps aside meanwhile.
  function togglePicker() {
    const home = document.querySelector(".home"), panel = home.querySelector(".hc-panel"), btn = home.querySelector("[data-act=model]");
    if (picking) { Picker.close(); return; }
    picking = true; panel.hidden = false; home.classList.add("picking"); btn.setAttribute("aria-expanded", "true");
    Picker.mount(panel, choice(), choose, () => { picking = false; panel.hidden = true; home.classList.remove("picking"); btn.setAttribute("aria-expanded", "false"); });
  }

  async function ensureProject() {
    const p = project();
    if (!p) { pickProject(); return false; }
    if (!hasProject || p !== snap.workspace) {
      await openProject(p);
      if (!hasProject || snap.workspace !== p) return false;
    }
    return true;
  }

  async function send(parallel) {
    const ta = document.querySelector(".hc textarea"), text = ta && ta.value.trim();
    if (!text || !(await ensureProject())) return;
    if (parallel) {
      try {
        const key = await invoke("start_task", { text });
        toast(`Started ${key} in parallel, on its own branch`, "ok");
      } catch (e) { toast(String(e), "err"); return; }
    } else await invoke("send", { text });
    ta.value = ""; ta.style.height = "";
    open = "agents"; pills();
  }

  function render(g) {
    let home = g.querySelector(".home");
    if (!home) {
      g.className = "grid home-wrap";
      g.innerHTML = shell();
      home = g.querySelector(".home");
      home.querySelector("[data-act=proj]").onclick = e => projMenu(e.currentTarget);
      home.querySelector("[data-act=model]").onclick = togglePicker;
      home.querySelector("[data-act=send]").onclick = () => send(false);
      home.querySelector("[data-act=par]").onclick = () => send(true);
      const ta = home.querySelector("textarea");
      ta.onkeydown = e => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); send(e.ctrlKey || e.metaKey); } };
      ta.oninput = () => { ta.style.height = "auto"; ta.style.height = Math.min(200, ta.scrollHeight) + "px"; };
      requestAnimationFrame(() => ta.focus());
      picking = false;
    }
    bar();
    pills();
    loadActivity();
  }

  return { render };
})();
