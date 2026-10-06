// Memory: notes every chat and agent gets ("I deploy with fly.io", "the
// staging DB is read-only"). A note is global or tied to one project.
// Chats get the global notes plus their project's; a project's planner and
// workers get the same when it opens. Saved from a chat message too.

var Memory = (() => {
  let notes = [], filter = "all", q = "", editing = null, draft = "", draftScope = "";

  async function load() { notes = (await invoke("memory_list").catch(() => null)) || []; }
  async function init() { await load(); }
  const projects = () => [...new Set([...(prefs.projects || []), ...notes.map(n => n.project).filter(Boolean)])];
  const curProject = () => (hasProject && snap.workspace) || "";

  function shown() {
    const s = q.trim().toLowerCase();
    return notes.filter(n => (filter === "all" || (filter === "global" ? !n.project : n.project === filter)) && (!s || n.text.toLowerCase().includes(s)));
  }

  function renderSide(el) {
    const ps = projects();
    const count = f => notes.filter(n => f === "all" || (f === "global" ? !n.project : n.project === f)).length;
    const row = (f, ic, label, title) => `<button class="row${filter === f ? " on" : ""}" data-mf="${esc(f)}" title="${esc(title || label)}">${icon(ic)}<span class="lab">${esc(label)}</span><span class="n">${count(f) || ""}</span></button>`;
    el.innerHTML = row("all", "memory", "All notes") + row("global", "globe", "Everywhere", "Notes every chat and project gets") +
      (ps.length ? `<div class="sec-t">Projects</div>` + ps.map(p => row(p, "folder", base(p), p)).join("") : "") +
      `<div class="mem-on"><span>Give notes to chats and agents</span><button class="toggle" role="switch" id="memOn" aria-checked="${prefs.memory_on !== false}" aria-label="Give notes to chats and agents"></button></div>`;
    $$("[data-mf]", el).forEach(b => (b.onclick = () => { filter = b.dataset.mf; renderSide(el); render(); }));
    $("#memOn", el).onclick = async () => {
      prefs.memory_on = prefs.memory_on === false; await invoke("set_memory_on", { on: prefs.memory_on });
      toast(prefs.memory_on ? "Chats and agents get your notes" : "Notes are kept, but not given to chats or agents", "ok"); renderSide(el);
    };
  }

  function scopeSelect(id, cur) {
    const ps = projects();
    return `<select class="tx" id="${id}" aria-label="Where this note applies"><option value="">Everywhere</option>${ps.map(p => `<option value="${esc(p)}" ${cur === p ? "selected" : ""}>${esc(base(p))}</option>`).join("")}</select>`;
  }
  function ago(ms) {
    const s = (Date.now() - ms) / 1000;
    if (s < 60) return "just now"; if (s < 3600) return Math.floor(s / 60) + " min ago"; if (s < 86400) return Math.floor(s / 3600) + " h ago";
    return new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });
  }

  function render() {
    const el = $("#memory"); if (!el) return;
    const list = shown();
    const defScope = draftScope || (filter !== "all" && filter !== "global" ? filter : "");
    el.innerHTML = `<div class="mem">
      <div class="mem-h"><h1>Memory</h1><p>Short notes every chat and agent gets. Keep them to facts and preferences: how you like code written, what a project must never do.</p></div>
      <div class="mem-add">
        <textarea class="tx" id="memNew" rows="2" placeholder="e.g. Use pnpm, never npm, in every project" aria-label="New note">${esc(draft)}</textarea>
        <div class="mem-row">${scopeSelect("memScope", defScope)}<span class="sp"></span><button class="btn primary" id="memSave">Save note</button></div>
      </div>
      ${notes.length ? `<label class="mem-q">${icon("search")}<input id="memQ" placeholder="Search notes" value="${esc(q)}" aria-label="Search notes"></label>` : ""}
      ${prefs.memory_on === false ? `<p class="warnp">${icon("warn")}Notes are switched off: chats and agents don't get them. Switch them back on in the sidebar.</p>` : ""}
      <div class="mem-list">${list.length ? list.map(n => editing === n.id
        ? `<div class="note edit"><textarea class="tx" id="memEd" rows="3" aria-label="Edit note">${esc(n.text)}</textarea><div class="mem-row">${scopeSelect("memEdScope", n.project || "")}<span class="sp"></span><button class="btn" id="memCancel">Cancel</button><button class="btn primary" id="memUpd">Save</button></div></div>`
        : `<div class="note${n.on ? "" : " off"}" data-n="${esc(n.id)}">
            <div class="nt">${esc(n.text)}</div>
            <div class="nm"><span class="chip">${n.project ? icon("folder") + esc(base(n.project)) : icon("globe") + "Everywhere"}</span><span>${esc(n.source && n.source !== "you" ? "From " + n.source : "")}</span><span>${ago(n.updated)}</span><span class="sp"></span>
              <button class="toggle sm" role="switch" aria-checked="${n.on}" data-on="${esc(n.id)}" title="${n.on ? "Given to chats and agents" : "Kept, not given"}" aria-label="Use this note"></button>
              <button class="ib" data-ed="${esc(n.id)}" title="Edit" aria-label="Edit note">${icon("edit")}</button>
              <button class="ib" data-del="${esc(n.id)}" title="Delete" aria-label="Delete note">${icon("trash")}</button></div>
          </div>`).join("")
        : `<div class="mem-empty">${icon("memory")}<b>${notes.length ? "No notes match" : "No notes yet"}</b><span>${notes.length ? "Try another search or filter." : "Add one above, or save a chat reply with Remember."}</span></div>`}</div>
    </div>`;
    const nw = $("#memNew", el);
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
