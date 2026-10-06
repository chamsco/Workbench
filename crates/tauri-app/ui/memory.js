// Memory: notes every chat and agent gets ("I deploy with fly.io", "the
// staging DB is read-only"). A note is global or tied to one project.
// Chats get the global notes plus their project's; a project's planner and
// workers get the same when it opens. Saved from a chat message too.

var Memory = (() => {
  let notes = [], filter = "all", q = "", editing = null, draft = "", draftScope = "";

  let agents = [], dreams = [], dreaming = false, repo = "", showAll = false;
  const agentName = id => (agents.find(a => a.id === id) || { name: "An agent" }).name;
  async function load() {
    notes = (await invoke("memory_list").catch(() => null)) || [];
    agents = (await invoke("agents_list").catch(() => null)) || [];
    const d = (await invoke("memory_dreams").catch(() => null)) || {};
    dreams = d.dreams || []; dreaming = !!d.dreaming; repo = d.repo || "";
  }
  async function init() { await load(); }
  const projects = () => [...new Set([...(prefs.projects || []), ...notes.map(n => n.project).filter(p => p && !p.startsWith("agent:"))])];
  const curProject = () => (hasProject && snap.workspace) || "";

  function shown() {
    const s = q.trim().toLowerCase();
    return notes.filter(n => (filter === "all" || (filter === "global" ? !n.project : n.project === filter)) && (!s || n.text.toLowerCase().includes(s)));
  }

  function renderSide(el) {
    const ps = projects();
    const count = f => notes.filter(n => f === "all" || (f === "global" ? !n.project : n.project === f)).length;
    const row = (f, ic, label, title) => `<button class="row${filter === f ? " on" : ""}" data-mf="${esc(f)}" title="${esc(title || label)}">${icon(ic)}<span class="lab">${esc(label)}</span><span class="n">${count(f) || ""}</span></button>`;
    const hist = `<button class="row${filter === "dreams" ? " on" : ""}" data-mf="dreams" title="What each tidy changed, with Undo">${icon("moon")}<span class="lab">Tidy history</span><span class="n">${dreams.length || ""}</span></button>`;
    el.innerHTML = row("all", "memory", "All notes") + row("global", "globe", "Everywhere", "Notes every chat and project gets") + hist +
      (ps.length ? `<div class="sec-t">Projects</div>` + ps.map(p => row(p, "folder", base(p), p)).join("") : "") +
      (agents.length ? `<div class="sec-t">Agents</div>` + agents.map(a => row("agent:" + a.id, "team", a.name, `What ${a.name} keeps`)).join("") : "") +
      `<div class="mem-on"><span>Give notes to chats and agents</span><button class="toggle" role="switch" id="memOn" aria-checked="${prefs.memory_on !== false}" aria-label="Give notes to chats and agents"></button></div>
      <div class="mem-on second"><span>Notice things to remember<small>Decided on this machine; learns from "Don't remember that"</small></span><button class="toggle" role="switch" id="memAuto" aria-checked="${prefs.memory_auto !== false}" aria-label="Notice things to remember"></button></div>
      <div class="mem-on second"><span>Tidy once a day<small>Your chat model merges, updates and adds notes; every tidy can be undone</small></span><button class="toggle" role="switch" id="memDream" aria-checked="${prefs.memory_dream !== false}" aria-label="Tidy once a day"></button></div>`;
    $$("[data-mf]", el).forEach(b => (b.onclick = () => { filter = b.dataset.mf; renderSide(el); render(); }));
    $("#memAuto", el).onclick = async () => {
      prefs.memory_auto = prefs.memory_auto === false; await invoke("set_memory_auto", { on: prefs.memory_auto });
      toast(prefs.memory_auto ? "Backspace will notice things worth remembering" : "Only notes you add or Remember are kept", "ok"); renderSide(el);
    };
    $("#memDream", el).onclick = async () => {
      prefs.memory_dream = prefs.memory_dream === false; await invoke("set_memory_dream", { on: prefs.memory_dream });
      toast(prefs.memory_dream ? "Memory will tidy itself about once a day" : "Memory tidies only when you ask", "ok"); renderSide(el);
    };
    $("#memOn", el).onclick = async () => {
      prefs.memory_on = prefs.memory_on === false; await invoke("set_memory_on", { on: prefs.memory_on });
      toast(prefs.memory_on ? "Chats and agents get your notes" : "Notes are kept, but not given to chats or agents", "ok"); renderSide(el);
    };
  }

  function scopeSelect(id, cur) {
    const ps = projects();
    return `<select class="tx" id="${id}" aria-label="Where this note applies"><option value="">Everywhere</option>${ps.map(p => `<option value="${esc(p)}" ${cur === p ? "selected" : ""}>${esc(base(p))}</option>`).join("")}${agents.map(a => `<option value="agent:${esc(a.id)}" ${cur === "agent:" + a.id ? "selected" : ""}>${esc(a.name)} only</option>`).join("")}</select>`;
  }
  function ago(ms) {
    const s = (Date.now() - ms) / 1000;
    if (s < 60) return "just now"; if (s < 3600) return Math.floor(s / 60) + " min ago"; if (s < 86400) return Math.floor(s / 3600) + " h ago";
    return new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });
  }

  function sourceLabel(n) {
    const what = { chat: "From chat", phone: "From phone", dream: "From a tidy", agent: "Saved by the agent", file: "Added in the repo" }[n.source] || (n.source && n.source !== "you" ? "From " + n.source : "");
    if (!what) return "<span></span>";
    return n.thread ? `<button class="lnk" data-th="${esc(n.thread)}" title="Open the chat it came from">${esc(what)}</button>` : `<span>${esc(what)}</span>`;
  }
  function day(ms) { return new Date(ms).toLocaleString([], { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" }); }
  function dreamCard(d, latest) {
    const ch = d.changes || [], more = !showAll && ch.length > 5 && latest;
    const items = (more ? ch.slice(0, 5) : ch).map(c => `<li>${esc(c)}</li>`).join("");
    const state = d.error ? `<span class="bad">Failed</span>` : d.undone ? `<span>Undone</span>` : `<span>${ch.length && d.commit ? ch.length + " change" + (ch.length === 1 ? "" : "s") : "No changes"}</span>`;
    return `<div class="dream${d.undone ? " off" : ""}">
      <div class="dh">${icon("moon")}<b>${latest ? "Last tidy" : "Tidy"}</b><span>${day(d.at)}</span><span>Read ${d.read} message${d.read === 1 ? "" : "s"}</span>${state}<span class="sp"></span>
        ${d.commit && !d.undone && !d.error ? `<button class="btn sm" data-undo="${d.at}" title="Revert this tidy (a new commit in the memory repo)">${icon("undo")}Undo</button>` : ""}</div>
      ${d.error ? `<p class="de">${esc(d.error)}</p>` : d.commit ? `<ul>${items}</ul>${more ? `<button class="lnk" id="memMore">Show all ${ch.length}</button>` : ""}` : ""}
    </div>`;
  }

  function render() {
    const el = $("#memory"); if (!el) return;
    const list = shown();
    const defScope = draftScope || (filter !== "all" && filter !== "global" && filter !== "dreams" ? filter : "");
    el.innerHTML = `<div class="mem">
      <div class="mem-h"><div class="mem-ht"><h1>Memory</h1><span class="sp"></span><button class="btn" id="memDreamNow" ${dreaming ? "disabled" : ""} title="Read your recent chats and tidy memory now">${icon("moon")}${dreaming ? "Tidying…" : "Tidy now"}</button></div><p>Short notes every chat and agent gets. Keep them to facts and preferences: how you like code written, what a project must never do.</p>
        ${repo ? `<p class="mem-repo">${icon("branch")}Kept as a git repo in the Agent Memory Repo format, so you and other agents can read, edit and sync it. <button class="lnk" id="memReveal">Show folder</button></p>` : ""}</div>
      <div class="mem-add">
        <textarea class="tx" id="memNew" rows="2" placeholder="e.g. Use pnpm, never npm, in every project" aria-label="New note">${esc(draft)}</textarea>
        <div class="mem-row">${scopeSelect("memScope", defScope)}<span class="sp"></span><button class="btn primary" id="memSave">Save note</button></div>
      </div>
      ${notes.length && filter !== "dreams" ? `<label class="mem-q">${icon("search")}<input id="memQ" placeholder="Search notes" value="${esc(q)}" aria-label="Search notes"></label>` : ""}
      ${prefs.memory_on === false ? `<p class="warnp">${icon("warn")}Notes are switched off: chats and agents don't get them. Switch them back on in the sidebar.</p>` : ""}
      ${filter === "dreams" ? `<div class="mem-list">${dreams.length ? dreams.map((d, i) => dreamCard(d, i === 0)).join("") : `<div class="mem-empty">${icon("moon")}<b>No tidies yet</b><span>Tidy now reads what you said in chats since the last tidy and merges, updates and adds notes.</span></div>`}</div>` :
      `<div class="mem-list">${filter === "all" && !q && dreams[0] && !dreams[0].undone && Date.now() - dreams[0].at < 3 * 864e5 ? dreamCard(dreams[0], true) : ""}${list.length ? list.map(n => editing === n.id
        ? `<div class="note edit"><textarea class="tx" id="memEd" rows="3" aria-label="Edit note">${esc(n.text)}</textarea><div class="mem-row">${scopeSelect("memEdScope", n.project || "")}<span class="sp"></span><button class="btn" id="memCancel">Cancel</button><button class="btn primary" id="memUpd">Save</button></div></div>`
        : `<div class="note${n.on ? "" : " off"}" data-n="${esc(n.id)}">
            <div class="nt">${esc(n.text)}</div>
            <div class="nm"><span class="chip">${n.project ? (n.project.startsWith("agent:") ? icon("team") + esc(agentName(n.project.slice(6))) : icon("folder") + esc(base(n.project))) : icon("globe") + "Everywhere"}</span><span>${esc(n.kind ? n.kind[0].toUpperCase() + n.kind.slice(1) : "")}</span>${sourceLabel(n)}<span>${ago(n.updated)}</span><span class="sp"></span>
              <button class="toggle sm" role="switch" aria-checked="${n.on}" data-on="${esc(n.id)}" title="${n.on ? "Given to chats and agents" : "Kept, not given"}" aria-label="Use this note"></button>
              <button class="ib" data-ed="${esc(n.id)}" title="Edit" aria-label="Edit note">${icon("edit")}</button>
              <button class="ib" data-del="${esc(n.id)}" title="Delete" aria-label="Delete note">${icon("trash")}</button></div>
          </div>`).join("")
        : `<div class="mem-empty">${icon("memory")}<b>${notes.length ? "No notes match" : "No notes yet"}</b><span>${notes.length ? "Try another search or filter." : "Add one above, or save a chat reply with Remember."}</span></div>`}</div>`}
    </div>`;
    $("#memDreamNow", el).onclick = async () => {
      dreaming = true; render();
      try {
        const d = await invoke("memory_dream");
        await load(); render(); Shell.renderSide();
        toast(d.error ? "Tidy failed: " + d.error : d.commit ? `Tidied: ${d.changes.length} change${d.changes.length === 1 ? "" : "s"}` : d.changes[0] || "Nothing to change", d.error ? "err" : "ok");
      } catch (e) { dreaming = false; render(); toast(String(e), "err"); }
    };
    const rv = $("#memReveal", el); if (rv) rv.onclick = () => invoke("memory_reveal").catch(e => toast(String(e), "err"));
    const mm = $("#memMore", el); if (mm) mm.onclick = () => { showAll = true; render(); };
    $$("[data-undo]", el).forEach(b => (b.onclick = async () => {
      try { await invoke("memory_undo_dream", { at: Number(b.dataset.undo) }); await load(); render(); Shell.renderSide(); toast("Tidy undone", "ok"); }
      catch (e) { toast(String(e), "err"); }
    }));
    $$("[data-th]", el).forEach(b => (b.onclick = () => window.Chat && Chat.open(b.dataset.th)));
    const nw = $("#memNew", el);
    if (!nw) return;
    nw.oninput = () => (draft = nw.value);
    $("#memScope", el).onchange = e => (draftScope = e.target.value);
    const save = async () => {
      const text = nw.value.trim(); if (!text) { nw.focus(); return; }
      try { await invoke("memory_add", { text, project: $("#memScope", el).value || null, source: "you" }); draft = ""; draftScope = ""; await load(); render(); Shell.renderSide(); toast("Saved", "ok"); }
      catch (e) { toast(String(e), "err"); }
    };
    $("#memSave", el).onclick = save;
    nw.onkeydown = e => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) save(); };
    const qi = $("#memQ", el);
    if (qi) qi.oninput = () => { q = qi.value; const pos = qi.selectionStart; render(); const n = $("#memQ"); if (n) { n.focus(); n.setSelectionRange(pos, pos); } };
    $$("[data-ed]", el).forEach(b => (b.onclick = () => { editing = b.dataset.ed; render(); const t = $("#memEd"); if (t) { t.focus(); t.setSelectionRange(t.value.length, t.value.length); } }));
    $$("[data-del]", el).forEach(b => (b.onclick = async () => {
      if (!b.dataset.armed) { b.dataset.armed = "1"; b.classList.add("armed"); b.title = "Click again to delete"; setTimeout(() => { if (b.isConnected) { delete b.dataset.armed; b.classList.remove("armed"); } }, 3000); return; }
      await invoke("memory_delete", { id: b.dataset.del }).catch(e => toast(String(e), "err")); await load(); render(); Shell.renderSide();
    }));
    $$("[data-on]", el).forEach(b => (b.onclick = async () => {
      await invoke("memory_update", { id: b.dataset.on, on: b.getAttribute("aria-checked") !== "true" }).catch(e => toast(String(e), "err")); await load(); render();
    }));
    const up = $("#memUpd", el);
    if (up) {
      up.onclick = async () => {
        try { await invoke("memory_update", { id: editing, text: $("#memEd", el).value, project: $("#memEdScope", el).value }); editing = null; await load(); render(); Shell.renderSide(); }
        catch (e) { toast(String(e), "err"); }
      };
      $("#memCancel", el).onclick = () => { editing = null; render(); };
    }
  }
  async function refresh() { await load(); if (Shell.showing("memory")) render(); if (S.mode === "memory") Shell.renderSide(); }

  return { init, render, renderSide, refresh, curProject };
})();
