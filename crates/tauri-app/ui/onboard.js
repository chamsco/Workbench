// First-run setup: what Backspace is, which coding CLIs it found (switch
// each on or off), local models and routers, an optional Cloud plan, then
// straight into a chat or a project. Skippable at every step; Settings has
// the same controls and "Run setup again".

var Onboard = (() => {
  const STEPS = ["welcome", "clis", "local", "cloud", "done"];
  let step = 0;

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
    const s = STEPS[step];
    if (s === "welcome") {
      el.innerHTML = frame(`<div class="ob-hero"><div class="ob-logo">${icon("bksp")}</div><h1>Welcome to Backspace</h1>
        <p>One place to chat with any model and to put coding agents to work on your projects.</p>
        <div class="ob-two"><div>${icon("compose")}<b>Chat</b><span>Threads with Claude, Codex, local models, routers or Backspace Cloud.</span></div>
        <div>${icon("folder")}<b>Code</b><span>Agents plan a project into tickets and build each one on its own branch. You approve every step.</span></div></div></div>`, { back: false, next: "Get started" });
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
        <p>New chats go to <button class="lnk" id="obRoute" data-popper>${esc(Chat.routeName(prefs.default_route || null))}</button>.</p>
        <div class="home-actions"><button class="btn primary big" id="obChat">${icon("compose")}Start chatting</button><button class="btn big" id="obCode">${icon("folder")}Open a project</button></div></div>`, { next: null, skip: false });
      const pick = $("#obRoute");
      // No choice yet: new chats use the first usable route.
      if (!prefs.default_route) pick.textContent = "the first model available";
      pick.onclick = e => Chat.routePicker(e.currentTarget, prefs.default_route, async r => { prefs.default_route = r; await invoke("set_default_route", { route: r }); render(); });
      $("#obChat").onclick = () => finish("chat");
      $("#obCode").onclick = () => finish("code");
    }
    const n = $("#obNext"); if (n) n.onclick = () => { step = Math.min(STEPS.length - 1, step + 1); render(); };
    const b = $("#obBack"); if (b) b.onclick = () => { step = Math.max(0, step - 1); render(); };
    const k = $("#obSkip"); if (k) k.onclick = () => finish(null);
    const f = el.querySelector("#obNext, .btn.primary"); if (f) f.focus();
  }

  async function finish(then) {
    $("#onboard").hidden = true; $("#win").classList.remove("onboarding");
    prefs.onboarded = true; await invoke("set_onboarded", { done: true });
    if (then === "code") { setMode("code"); pickProject(); }
    else { setMode("chat"); Chat.newChat(); }
  }

  return {
    open() { step = 0; $("#onboard").hidden = false; $("#win").classList.add("onboarding"); closePop(); render(); },
    finish,
  };
})();
