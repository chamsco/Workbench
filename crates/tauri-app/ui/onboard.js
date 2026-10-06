// First-run setup: what Backspace is for (Chat, Coding or both), which coding CLIs it found (switch
// each on or off), local models and routers, an optional Cloud plan, then
// straight into a chat or a project. Skippable at every step; Settings has
// the same controls and "Run setup again".

var Onboard = (() => {
  const ALL = ["welcome", "clis", "local", "cloud", "done"];
  let step = 0, uses = null;
  // Cloud plans only matter for Chat; everything else serves both.
  const steps = () => ALL.filter(s => s !== "cloud" || !uses || uses.includes("chat"));
  let STEPS = ALL;

  function dots() {
    return `<div class="ob-dots">${STEPS.map((s, i) => `<span class="${i === step ? "on" : i < step ? "past" : ""}"></span>`).join("")}</div>`;
  }
  function frame(inner, { back = true, next = "Continue", skip = true } = {}) {
    return `<div class="ob-card" role="dialog" aria-modal="true" aria-label="Set up Backspace">
      <div class="ob-top">${dots()}<span class="sp"></span>${skip ? `<button class="lnk" id="obSkip">Skip setup</button>` : ""}</div>
      <div class="ob-body">${inner}</div>
      <div class="ob-foot">${back && step > 0 ? `<button class="btn" id="obBack">Back</button>` : ""}<span class="sp"></span>${next ? `<button class="btn primary" id="obNext">${next}</button>` : ""}</div></div>`;
  }

  function render() {
    const el = $("#onboard");
    STEPS = steps();
    const s = STEPS[step];
    if (s === "welcome") {
      // Chat and Code are the two built-in apps; keep at least one.
      const tile = (k, ic, t, d, more) => {
        const on = uses.includes(k), last = on && uses.length === 1;
        return `<button class="appuse" data-use="${k}" aria-pressed="${on}" ${last ? `title="Keep at least one app"` : ""}>
          <span class="au-ic">${icon(ic)}</span>
          <span class="au-t"><b>${t}</b><span>${d}</span><small>${more}</small></span>
          <span class="au-btn">${on ? `${icon("check")}${last ? "Required" : "Added"}` : `${icon("plus")}Add`}</span>
        </button>`;
      };
      el.innerHTML = frame(`<div class="ob-hero"><div class="ob-logo">${icon("bksp")}</div><h1>Welcome to Backspace</h1>
        <p>Pick your apps. You need at least one; add the other any time from Apps.</p></div>
        <div class="appuses">${tile("chat", "bubble", "Chat", "Talk to any model, or to your own named agents, one to one or in groups.", "Your CLIs, local models, routers or Backspace Cloud")}${tile("code", "codei", "Code", "Pair with a CLI in a project, or let agents plan it into tickets and build each on its own branch.", "You review every change before it merges")}</div>`, { back: false, next: "Get started" });
      $$("[data-use]", el).forEach(b => (b.onclick = () => {
        const k = b.dataset.use;
        const next = uses.includes(k) ? uses.filter(u => u !== k) : [...uses, k];
        if (!next.length) { b.classList.add("nope"); setTimeout(() => b.classList.remove("nope"), 400); return; }
        uses = ["chat", "code"].filter(u => next.includes(u)); render();
      }));
    } else if (s === "clis") {
      el.innerHTML = frame(`<h2>Your coding CLIs</h2><p class="ob-lead">Backspace uses the CLIs you already have, with your own subscriptions. Here's what's on this machine; switch off any you don't want offered.</p><div id="obCli" class="ob-prov"></div>`);
      Providers.mount($("#obCli"), { only: "cli" });
      if (!Providers.list.length) Providers.rescan();
    } else if (s === "local") {
      el.innerHTML = frame(`<h2>Local models and routers</h2><p class="ob-lead">Run models on this machine with Ollama (private, offline, free), or add any OpenAI-compatible endpoint. Both are optional.</p><div id="obLocal" class="ob-prov"></div>`);
      Providers.mount($("#obLocal"), { only: "local" });
    } else if (s === "cloud") {
      el.innerHTML = frame(`<h2>Backspace Cloud <span class="opt">optional</span></h2><p class="ob-lead">Chat with nothing to install. Start free with a short sponsored card under each reply, or pick a plan without ads.</p><div id="obPlan"></div>`, { next: "Continue" });
      Providers.mountPlan($("#obPlan"));
    } else {
      const on = Providers.list.filter(h => h.enabled);
      const cloud = prefs.cloud && prefs.cloud.token;
      el.innerHTML = frame(`<div class="ob-hero"><div class="ob-logo ok">${icon("check")}</div><h1>You're set</h1>
        ${on.length || cloud ? `<div class="ob-chips">${on.map(h => `<span class="ob-chip">${Providers.avatar(h.id, "xs")}${esc(h.name)}</span>`).join("")}${cloud ? `<span class="ob-chip">${Providers.avatar("cloud", "xs")}Cloud</span>` : ""}</div>` : `<p class="warnp">${icon("warn")}Nothing is connected yet, so Chat has no model to use. You can add one any time in Settings.</p>`}
        ${uses.includes("chat") ? `<p>New chats go to <button class="lnk" id="obRoute" data-popper>${esc(Chat.routeName(prefs.default_route || null))}</button>.</p>` : ""}
        <div class="ob-more">
          <div>${icon("memory")}<b>Memory</b><span>Notes every chat and agent gets.</span></div>
          <div>${icon("apps")}<b>Apps</b><span>Views with their own agent. Try Prompt Lab.</span></div>
          <div>${icon("phone")}<b>Phone</b><span>Pair it in Settings when the app ships.</span></div>
        </div>
        <div class="home-actions">${uses.includes("chat") ? `<button class="btn primary big" id="obChat">${icon("compose")}Start chatting</button>` : ""}${uses.includes("code") ? `<button class="btn ${uses.includes("chat") ? "" : "primary "}big" id="obCode">${icon("folder")}Open a project</button>` : ""}</div></div>`, { next: null, skip: false });
      const pick = $("#obRoute");
      // No choice yet: new chats use the first usable route.
      if (pick && !prefs.default_route) pick.textContent = "the first model available";
      if (pick) pick.onclick = e => Chat.routePicker(e.currentTarget, prefs.default_route, async r => { prefs.default_route = r; await invoke("set_default_route", { route: r }); render(); });
      const oc = $("#obChat"); if (oc) oc.onclick = () => finish("chat");
      const od = $("#obCode"); if (od) od.onclick = () => finish("code");
    }
    const n = $("#obNext"); if (n) n.onclick = async () => {
      if (s === "welcome") { prefs.uses = uses; await invoke("set_uses", { uses }); }
      step = Math.min(steps().length - 1, step + 1); render();
    };
    const b = $("#obBack"); if (b) b.onclick = () => { step = Math.max(0, step - 1); render(); };
    const k = $("#obSkip"); if (k) k.onclick = () => finish(null);
    const f = el.querySelector("#obNext, .btn.primary"); if (f) f.focus();
  }

  async function finish(then) {
    $("#onboard").hidden = true; $("#win").classList.remove("onboarding");
    prefs.onboarded = true; await invoke("set_onboarded", { done: true });
    if (then === "code" || (prefs.uses || []).join() === "code") { setMode("code"); if (then === "code") pickProject(); }
    else { setMode("chat"); Chat.newChat(); }
  }

  return {
    open() { step = 0; uses = (prefs.uses && prefs.uses.length ? prefs.uses : ["chat", "code"]).slice(); $("#onboard").hidden = false; $("#win").classList.add("onboarding"); closePop(); render(); },
    finish,
  };
})();
