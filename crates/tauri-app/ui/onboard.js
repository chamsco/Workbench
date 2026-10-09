// First-run setup: what Backspace is for (Chat, Coding or both), which coding CLIs it found (switch
// each on or off), a first agent (brought from elsewhere or made here),
// local models and routers, an optional Cloud plan, then straight into a
// chat or a project. Skippable at every step; Settings has
// the same controls and "Run setup again".

var Onboard = (() => {
  const ALL = ["welcome", "clis", "notify", "agent", "local", "cloud", "done"];
  const NOTIFY = [["system", "System notifications", "Your computer's own alerts when Backspace is in the background, a note in the app when it's in front."], ["app", "Inside Backspace only", "Notes in the corner of the app; nothing outside it."], ["off", "Off", "Check Home when you want; nothing pops up."]];
  let step = 0, uses = null;
  // Cloud plans and agents live in Chat; everything else serves both.
  const steps = () => ALL.filter(s => !["cloud", "agent"].includes(s) || !uses || uses.includes("chat"));

  // Bring your agent: what can be connected today. Consumer agents with no
  // outside API (OpenAI Dots, Grok Bot, Tencent WorkBuddy) aren't listed as
  // if they were; the footnote says what to bring from them instead.
  const PICKS = {
    sidekick: { t: "Backspace sidekick", d: "Make one here: a name, a job, and the model it runs on.", name: "Sidekick", emoji: "🤖", color: "#7c3aed" },
    claude: { t: "Claude Code", d: "Anthropic's coding agent, on your own subscription.", name: "Claude", emoji: "🧭", color: "#d97757", cli: "claude" },
    codex: { t: "OpenAI Codex", d: "The open-source agent harness OpenAI's Dots run on.", name: "Codex", emoji: "⚙️", color: "#10a37f", cli: "codex" },
    xai: { t: "Grok", d: "xAI's models with your API key.", name: "Grok", emoji: "⚡", color: "#111827", base: "https://api.x.ai/v1", router: "xAI", prefer: /grok/i },
    meta: { t: "Meta Muse", d: "Meta's Model API with your API key.", name: "Muse", emoji: "🎨", color: "#0866ff", base: "https://api.meta.ai/v1", router: "Meta", prefer: /muse/i },
    a2a: { t: "Any A2A agent", d: "An agent that lives elsewhere and speaks A2A.", name: "", emoji: "🔗", color: "#b45309" },
  };
  let pick = null, added = null, sideRoute = null;
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
        <div class="appuses">${tile("chat", "bubble", "Chat", "Talk to any model, or to your own named agents, one to one or in groups.", "Your CLIs, local models, routers or Backspace Cloud")}${tile("code", "codei", "Code", "Chat with a CLI in a project, or let agents plan it into tickets and build each on its own branch.", "You review every change before it merges")}</div>`, { back: false, next: "Get started" });
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
    } else if (s === "agent") {
      const P = pick && PICKS[pick];
      const found = id => (Providers.list || []).find(h => h.id === id);
      const st = k => {
        const p = PICKS[k];
        if (!p.cli) return "";
        const h = found(p.cli);
        return h && h.installed ? (h.enabled ? "On this machine" : "Switched off") : "Not installed";
      };
      sideRoute = sideRoute || Chat.defaultRoute();
      el.innerHTML = frame(`<h2>Your first agent</h2><p class="ob-lead">Bring an agent you already use, or make one here. It gets a name, its own folder and memory, a trace of everything it does, and a seat in groups later.</p>
        ${added ? `<p class="okp">${icon("check")}${esc(added)} is ready in Chat → Agents. Add another, or continue.</p>` : ""}
        <div class="bya">${Object.entries(PICKS).map(([k, p]) => `<button class="bya-t" data-pick="${k}" aria-pressed="${pick === k}"><span class="bya-av" style="--av:${p.color}">${p.emoji}</span><span class="bya-x"><b>${esc(p.t)}</b><span>${esc(p.d)}</span>${st(k) ? `<small>${st(k)}</small>` : ""}</span></button>`).join("")}</div>
        ${P ? `<div class="bya-form">
          ${pick === "a2a" ? `<div class="bya-row"><input class="tx mono" id="byUrl" placeholder="https://agent.example.com" aria-label="Agent address"><input class="tx mono" id="byTok" type="password" placeholder="Token (optional)" aria-label="Agent token"></div>` : ""}
          ${P.base ? `<div class="bya-row"><input class="tx mono" id="byKey" type="password" placeholder="${esc(P.router)} API key" aria-label="API key"></div>` : ""}
          <div class="bya-row"><input class="tx" id="byName" value="${esc(P.name)}" placeholder="${pick === "a2a" ? "Name (from its card if empty)" : "Name"}" aria-label="Agent name"></div>
          <textarea class="tx" id="byJob" rows="2" placeholder="${pick === "sidekick" ? "Its job, e.g. Plans my week and keeps my notes tidy" : "Its job here (optional)"}" aria-label="Agent's job"></textarea>
          ${pick === "sidekick" ? `<div class="bya-row"><span class="lab">Runs on</span><button class="btn" id="byRoute" data-popper>${esc(sideRoute ? Chat.routeName(sideRoute) : "Pick a model")}</button></div>` : ""}
          <div class="bya-row"><span class="err" id="byErr" role="alert"></span><span class="sp"></span><button class="btn primary" id="byAdd">Add agent</button></div>
        </div>` : ""}
        <p class="ob-note">OpenAI Dots, Grok Bot and Tencent WorkBuddy don't let other apps drive them yet. From them you can bring Codex and xAI's API today.</p>`, { next: added ? "Continue" : "Skip for now" });
      $$("[data-pick]", el).forEach(b => (b.onclick = () => { pick = pick === b.dataset.pick ? null : b.dataset.pick; render(); const f = $("#byUrl, #byKey, #byJob"); if (f) f.focus(); }));
      const rt = $("#byRoute"); if (rt) rt.onclick = e => Chat.routePicker(e.currentTarget, sideRoute, r => { sideRoute = r; render(); });
      const add = $("#byAdd");
      if (add) add.onclick = async () => {
        const err = $("#byErr"); err.textContent = "";
        const val = id => ($("#" + id) || {}).value || "";
        let name = val("byName").trim(), job = val("byJob").trim(), route = null;
        add.disabled = true; add.textContent = "Adding…";
        try {
          if (pick === "sidekick") {
            if (!sideRoute) throw "Connect a CLI or a model first (the step before), then pick it here.";
            if (!job) throw "Give it a job: one line on what it does.";
            route = sideRoute;
          } else if (P.cli) {
            const h = found(P.cli);
            if (!h || !h.installed) throw `${P.t} isn't installed${h && h.install ? ": " + h.install : ""}.`;
            if (!h.enabled) await invoke("set_harness", { id: P.cli, on: true });
            route = { kind: "cli", provider: P.cli, model: null };
          } else if (P.base) {
            if (!val("byKey").trim()) throw `Paste your ${P.router} API key.`;
            const info = await invoke("add_router", { name: P.router, baseUrl: P.base, apiKey: val("byKey") });
            const ms = (info.models || []).filter(m => P.prefer.test(m) && !/image|vision|embed|audio/i.test(m));
            route = { kind: "router", provider: info.id.replace(/^router:/, ""), model: (ms.length ? ms[ms.length - 1] : (info.models || [])[0]) || null };
          } else {
            if (!val("byUrl").trim()) throw "Paste the agent's address.";
            const a = await invoke("connect_agent", { url: val("byUrl"), token: val("byTok") });
            route = { kind: "a2a", provider: a.id, model: null };
            name = name || a.name; job = job || a.description;
            // Your notes would leave this machine: off until you turn it on.
          }
          const ag = await invoke("agent_save", { agent: { id: "", name: name || P.name || "Agent", job, avatar: { emoji: P.emoji, color: P.color }, route, shared: [], memory: pick !== "a2a" } });
          added = ag.name; pick = null;
          prefs = await invoke("prefs"); await Providers.load(); if (window.Chat) Chat.refresh();
          render();
        } catch (e) { err.textContent = String(e); add.disabled = false; add.textContent = "Add agent"; }
      };
    } else if (s === "notify") {
      const cur = prefs.notify || "system";
      el.innerHTML = frame(`<h2>Notifications</h2><p class="ob-lead">How should Backspace tell you that a plan needs approval, work is ready to review, an agent failed, or a chat hit a limit?</p>
        <div class="ob-notify">${NOTIFY.map(([k, t, d]) => `<button class="ob-n${cur === k ? " on" : ""}" data-n="${k}"><b>${t}</b><span>${d}</span></button>`).join("")}</div>
        <button class="lnk" id="obTest">Send a test notification</button>`);
      $$("[data-n]", el).forEach(b => (b.onclick = async () => { prefs.notify = b.dataset.n; await invoke("set_notify", { notify: prefs.notify }); render(); }));
      $("#obTest").onclick = () => invoke("notify", { title: "Backspace", body: "Notifications work. You'll hear from your agents here." }).catch(e => toast(String(e), "err"));
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
    open() { step = 0; pick = null; added = null; sideRoute = null; uses = (prefs.uses && prefs.uses.length ? prefs.uses : ["chat", "code"]).slice(); $("#onboard").hidden = false; $("#win").classList.add("onboarding"); closePop(); render(); },
    finish,
  };
})();
