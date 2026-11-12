// The project's four stages: Plan, Execute, Ship, Commit (core/stages.rs).
// A strip that shows where the project stands, what each stage has cost
// (work, and the decisions on top), which decision model helps it, and what
// stops its gate. A gate is opened by a rule or by you; a stage's model can
// refuse, never open. At Commit the Librarian files the notes the agents and
// their subagents collected, as one memory commit.

var Stages = (() => {
  const NAMES = { plan: "Plan", execute: "Execute", ship: "Ship", commit: "Commit" };
  const WHAT = {
    plan: "Spec and plan: a spec the rules accept, tickets that cover it",
    execute: "Workers build the tickets; done when every one has finished",
    ship: "Your go: nothing publishes without you",
    commit: "The Librarian files what the agents learned, as one memory commit",
  };
  const BTN = { rules: "Finish Execute", human: "Approve ship", librarian: "File notes" };
  const usd = n => (n >= 1 ? "$" + n.toFixed(2) : n > 0 ? "$" + n.toFixed(3) : "$0");
  const esc1 = s => String(s == null ? "" : s).replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  let busy = false, last = null, error = "";

  async function load() {
    try { last = await invoke("stage_status"); error = ""; } catch (e) { last = null; error = String(e); }
    return last;
  }

  function html(st) {
    if (!st) return "";
    const cur = st.stage;
    const gaps = st.gaps || [];
    const act = st.advance === "plan" || st.advance === "none" ? "" : `<button class="stg-go" data-go ${busy || gaps.length ? "disabled" : ""} title="${esc1(gaps.join("; "))}">${busy ? "Working…" : BTN[st.advance] || "Next"}</button>`;
    return `<div class="stg-row" role="list" aria-label="Project stages">${st.stages.map((s, i) => `
      <div class="stg-s ${s.state}" role="listitem" title="${esc1(WHAT[s.key])}">
        <span class="stg-n">${s.state === "done" ? icon("check") : i + 1}</span>
        <span class="stg-t"><b>${NAMES[s.key]}</b><small>${usd(s.spent)}${s.decided ? ` · ${usd(s.decided)} deciding` : ""}</small></span>
        <button class="stg-m" data-model="${s.key}" title="${s.key === "commit" ? "The Librarian's model" : "The decision model for " + NAMES[s.key]}: it may refuse a gate, never open one">${s.model && window.Chat ? esc1(Chat.routeName(s.model)) : "Rules only"}</button>
      </div>`).join("")}${act}</div>
      ${cur ? `<div class="stg-note">${gaps.length ? esc1(gaps[0]) + (gaps.length > 1 ? ` <i>+${gaps.length - 1} more</i>` : "") : esc1(WHAT[cur])}${st.notes ? ` · <b>${st.notes}</b> note${st.notes === 1 ? "" : "s"} waiting for the Librarian` : ""}</div>` : `<div class="stg-note">Done${st.receipt ? ` · ${esc1("Librarian kept " + st.receipt.kept.length + ", merged " + st.receipt.merged.length + ", dropped " + st.receipt.dropped.length)}` : ""}</div>`}
      ${error ? `<div class="stg-err">${esc1(error)}</div>` : ""}`;
  }

  // Draw into `el` and keep it current. Returns the repaint.
  function mount(el) {
    const draw = () => {
      el.innerHTML = html(last);
      el.querySelectorAll("[data-model]").forEach(b => (b.onclick = e => pickModel(e.currentTarget, b.dataset.model, draw)));
      const go = el.querySelector("[data-go]");
      if (go) go.onclick = async () => {
        busy = true; error = ""; draw();
        try {
          last = await invoke("advance_stage");
          if (last.model_note) toast(last.model_note);
        } catch (e) { error = String(e); await load(); }
        busy = false; draw();
        if (typeof refresh === "function") refresh(true);
      };
    };
    load().then(draw);
    return () => load().then(() => el.isConnected && !busy && draw());
  }

  function pickModel(anchor, stage, done) {
    if (!window.Chat) return;
    Chat.routePicker(anchor, (prefs.stage_models || {})[stage] || null, async r => {
      prefs.stage_models = { ...(prefs.stage_models || {}), [stage]: r };
      await invoke("set_stage_model", { stage, route: r });
      await load(); done();
    });
    // "Rules only" is a row of its own above the picker's list.
    const pop = document.querySelector("#pop");
    if (pop && !pop.querySelector("[data-norule]")) {
      const b = document.createElement("button");
      b.className = "pi"; b.dataset.norule = "1"; b.textContent = "Rules only (free)";
      b.onclick = async () => {
        closePop();
        const m = { ...(prefs.stage_models || {}) }; delete m[stage]; prefs.stage_models = m;
        await invoke("set_stage_model", { stage, route: null });
        await load(); done();
      };
      pop.insertBefore(b, pop.firstChild);
    }
  }

  return { mount, load, html };
})();
