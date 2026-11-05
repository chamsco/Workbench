// First-run setup, in the shape of the app's four-step tour: Agent, Projects,
// Plugins, Devices. Each step is optional and skippable; everything here also
// lives in Settings ("Run setup again" reopens this).
//
// Agent: the coding CLIs found on this machine, with their sign-in state and a
//        sign-in that opens the CLI's own terminal.
// Projects: the folders in recents; pick one to open when setup finishes.
// Plugins: the built-in surfaces (Agents, Workbench, Memory) and the bundled
//          Prompt Lab app, each switched on or off.
// Devices: the ways to reach this machine from elsewhere (sharing, another
//          machine, a phone).

var Onboard = (() => {
  const STEPS = [
    ["agent", "Agent"],
    ["projects", "Projects"],
    ["plugins", "Plugins"],
    ["devices", "Devices"],
  ];
  let step = 0, signin = null, picked = new Set(), busy = "";
  const stepIndex = key => STEPS.findIndex(s => s[0] === key);
  const host = () => "this computer";

  function frame(inner, { next = "Continue", back = true } = {}) {
    return `<div class="ob-page">
      <header class="ob-head">
        <span class="ob-mark">${icon("bksp")}</span>
        <nav class="ob-steps" aria-label="Setup steps">
          ${STEPS.map(([key, label], i) => `<button type="button" class="ob-step${i === step ? " on" : ""}${i < step ? " done" : ""}" data-step="${key}" ${i > step ? "disabled" : ""} aria-current="${i === step ? "step" : "false"}">
            <span class="n">${i < step ? icon("check") : i + 1}</span><span class="t">${label}</span>
          </button>`).join("")}
        </nav>
        <button type="button" class="ob-skip" id="obSkip">Skip setup</button>
      </header>
      <main class="ob-main"><div class="ob-inner">${inner}</div></main>
      <footer class="ob-foot">
        ${back && step > 0 ? `<button type="button" class="ob-back" id="obBack"><span aria-hidden="true">&larr;</span>Back</button>` : `<span></span>`}
        <button type="button" class="ob-primary" id="obNext">${next}<span aria-hidden="true">&rarr;</span></button>
      </footer>
    </div>`;
  }

  // ---------------------------------------------------------------- agent
  function cliRows() {
    const list = (Providers.list || []).filter(h => h.kind === "cli");
    if (!list.length) {
      return `<div class="ob-empty"><span class="ob-spin"></span>Looking for coding agents on this computer…</div>`;
    }
    return list.map(h => {
      const authed = h.auth === "authenticated";
      const state = !h.installed ? "missing" : authed ? "ready" : "signin";
      const sub = !h.installed
        ? h.install ? `Install: ${esc(h.install)}` : "Not installed"
        : h.version ? `v${esc(h.version)}${h.detail ? " · " + esc(h.detail) : ""}` : h.detail || (authed ? "Signed in" : "Installed");
      const right = state === "ready"
        ? `<span class="ob-ready">${icon("check")}Ready</span>`
        : state === "signin"
          ? `<button type="button" class="ob-small acc" data-login="${esc(h.id)}">Sign in</button>`
          : `<span class="ob-muted">Not installed</span>`;
      const panel = signin === h.id ? signinPanel(h) : "";
      return `<div class="ob-arow${state === "missing" ? " dim" : ""}">
          <span class="ob-av">${Providers.avatar(h.id, "sm")}</span>
          <span class="ob-amain"><b>${esc(h.name)}</b><span>${sub}</span></span>
          ${right}
        </div>${panel}`;
    }).join("");
  }
  function signinPanel(h) {
    return `<div class="ob-signin-panel">
      <div class="ob-signin-h"><b>Sign in to ${esc(h.name)}</b><button type="button" class="lnk" id="obSigninCancel">Cancel</button></div>
      <p>A terminal window opened for ${esc(h.name)}. Finish signing in there; Backspace checks again when you come back.</p>
      <div class="ob-signin-row"><button type="button" class="ob-small" id="obSigninAgain">Check again</button><span class="ob-muted">${busy === "signin" ? "Checking…" : "Waiting for sign-in"}</span></div>
    </div>`;
  }
  function agentStep() {
    return `<h1>Connect a coding agent</h1>
      <p class="ob-sub">Backspace runs the agents you already use. You need one that is installed and signed in on this computer.</p>
      <div class="ob-rowline"><span>${icon("monitor")}On ${host()}</span><button type="button" class="lnk" id="obAgain">${icon("reload")}Check again</button></div>
      <div class="ob-list">${cliRows()}</div>
      <p class="ob-note">Don't see your agent? Backspace picks up other ACP agents, such as opencode, once they are installed.</p>`;
  }

  // ---------------------------------------------------------------- projects
  function projectRows() {
    const list = prefs.projects || [];
    if (!list.length) {
      return `<div class="ob-empty">No folders yet. Choose one below, and Backspace remembers it here.</div>`;
    }
    return list.map(p => {
      const open = snap && snap.workspace === p;
      return `<label class="ob-arow ob-pick">
        <input type="checkbox" class="ob-check" data-project="${esc(p)}" ${picked.has(p) ? "checked" : ""}>
        <span class="ob-amain"><b>${esc(base(p))}</b><span class="mono">${esc(p)}</span></span>
        <span class="ob-muted">${open ? "Open now" : "Already added"}</span>
      </label>`;
    }).join("");
  }
  function projectsStep() {
    const list = prefs.projects || [];
    return `<h1>Add your projects</h1>
      <p class="ob-sub">Git repos on this computer that you've worked in over the last 30 days.</p>
      <div class="ob-rowline"><span>${list.length ? `${picked.size} of ${list.length} selected` : "No projects yet"}</span>
        <span class="ob-rowacts">${list.length ? `<button type="button" class="lnk" id="obAll">Select all</button><button type="button" class="lnk" id="obNone">Select none</button>` : ""}<button type="button" class="lnk" id="obPickFolder">Choose a folder…</button></span></div>
      <div class="ob-list">${projectRows()}</div>`;
  }

  // ---------------------------------------------------------------- plugins
  function usesNow() {
    return prefs.uses && prefs.uses.length ? prefs.uses.slice() : ["chat", "code"];
  }
  function pluginCard(id, glyph, name, blurb, on, canOff = true) {
    return `<div class="ob-pcard${on ? "" : " off"}">
      <div class="ob-pc-top"><span class="ob-pc-ic">${glyph}</span><div><b>${name}</b></div></div>
      <p>${blurb}</p>
      <div class="ob-pc-foot"><span class="ob-muted">${on ? "On" : "Off"}</span><button type="button" class="ob-toggle" role="switch" aria-checked="${on}" data-plugin="${id}" ${!canOff && on ? 'title="Keep at least one app"' : ""}></button></div>
    </div>`;
  }
  function pluginsStep() {
    const uses = usesNow();
    const hasLab = !!(window.Apps && Apps.installed().some(a => a.id === "prompt-lab"));
    return `<h1>Make Backspace yours</h1>
      <p class="ob-sub">Most of Backspace is optional. Everything can be changed later in Settings.</p>
      <div class="ob-pgrid">
        ${pluginCard("chat", icon("bubble"), "Agents", "One thread with any model or your own named agents: Claude Code, Codex, a model on this machine, a router, groups.", uses.includes("chat"), uses.length > 1)}
        ${pluginCard("code", icon("codei"), "Workbench", "The project: canvases, worktrees and tickets, with agents planning it and building each ticket on its own branch.", uses.includes("code"), uses.length > 1)}
        ${pluginCard("memory", icon("memory"), "Memory", "Notes every chat and agent gets. Say something worth keeping once; it comes back when it matters.", prefs.memory_on !== false)}
        ${pluginCard("prompt-lab", icon("grid"), "Prompt Lab", "Ask two models the same thing and read the answers side by side. A small app with its own agent.", hasLab)}
      </div>
      <p class="ob-note">${icon("grid")}Want something that isn't here? Install an app from a folder or a URL in Apps.</p>`;
  }

  // ---------------------------------------------------------------- devices
  let devices = null;
  function deviceRows() {
    const share = devices && devices.share;
    const machines = (devices && devices.machines) || [];
    return `
      <div class="ob-list">
        <div class="ob-arow ob-device">
          <span class="ob-dev-ic">${icon("monitor")}</span>
          <span class="ob-amain"><b>Open Backspace from any browser</b><span>${share && share.enabled ? `Sharing on ${esc(share.addr)}. ${share.error ? esc(share.error) : "Reachable from your network."}` : "Share this machine so another browser or machine can follow it."}</span></span>
          <button type="button" class="ob-small" data-device="share">${share && share.enabled ? "Change" : "Set up"}</button>
        </div>
        <div class="ob-arow ob-device">
          <span class="ob-dev-ic">${icon("pc")}</span>
          <span class="ob-amain"><b>Add another machine</b><span>${machines.length > 1 ? `${machines.length} machines paired. ` : ""}Run agents on a server, a desktop, or a cloud box and steer them all from here.</span></span>
          <button type="button" class="ob-small" data-device="machines">Add machine</button>
        </div>
        <div class="ob-arow ob-device">
          <span class="ob-dev-ic">${icon("phone")}</span>
          <span class="ob-amain"><b>Get the mobile app</b><span>Get a push when a thread needs you.</span></span>
          <span class="ob-dev-btns"><button type="button" class="ob-small" data-device="phone">iOS</button><button type="button" class="ob-small" data-device="phone">Android</button></span>
        </div>
      </div>`;
  }
  function devicesStep() {
    return `<h1>Use Backspace from anywhere</h1>
      <p class="ob-sub">All optional. Everything here also lives in Settings → Machines.</p>
      ${deviceRows()}`;
  }
  // ---------------------------------------------------------------- render
  function render() {
    const el = $("#onboard");
    const key = STEPS[step][0];
    const body = key === "agent" ? agentStep() : key === "projects" ? projectsStep() : key === "plugins" ? pluginsStep() : devicesStep();
    const next = key === "devices" ? "Start using Backspace" : key === "projects" && picked.size ? `Import ${picked.size} project${picked.size === 1 ? "" : "s"}` : "Continue";
    el.innerHTML = frame(body, { next });
    wire();
  }

  function wire() {
    const el = $("#onboard");
    $$("[data-step]", el).forEach(b => (b.onclick = () => { step = stepIndex(b.dataset.step); signin = null; render(); }));
    const sk = $("#obSkip"); if (sk) sk.onclick = () => finish(null);
    const bk = $("#obBack"); if (bk) bk.onclick = () => { step = Math.max(0, step - 1); signin = null; render(); };
    $("#obNext").onclick = next;

    if (step === 0) {
      const again = $("#obAgain"); if (again) again.onclick = async () => { busy = "scan"; render(); await Providers.rescan(); busy = ""; render(); };
      $$("[data-login]", el).forEach(b => (b.onclick = async () => {
        try { await invoke("cli_login", { id: b.dataset.login }); } catch (e) { toast(String(e), "err"); return; }
        signin = b.dataset.login; busy = ""; render();
        addEventListener("focus", () => { if (signin) Providers.rescan().then(() => render()); }, { once: true });
      }));
      const cancel = $("#obSigninCancel"); if (cancel) cancel.onclick = () => { signin = null; render(); };
      const check = $("#obSigninAgain"); if (check) check.onclick = async () => { busy = "signin"; render(); await Providers.rescan(); busy = ""; render(); };
    }
    if (step === 1) {
      $$("[data-project]", el).forEach(i => (i.onchange = () => { const p = i.dataset.project; if (i.checked) picked.add(p); else picked.delete(p); render(); }));
      const all = $("#obAll"); if (all) all.onclick = () => { picked = new Set(prefs.projects || []); render(); };
      const none = $("#obNone"); if (none) none.onclick = () => { picked = new Set(); render(); };
      const pf = $("#obPickFolder"); if (pf) pf.onclick = async () => {
        const path = await invoke("pick_folder").catch(() => null);
        if (!path) return;
        await invoke("open_project", { path }).catch(e => toast(String(e), "err"));
        prefs = await invoke("prefs").catch(() => prefs);
        picked = new Set([path]);
        render();
      };
    }
    if (step === 2) {
      $$("[data-plugin]", el).forEach(b => (b.onclick = async () => {
        const id = b.dataset.plugin, on = b.getAttribute("aria-checked") !== "true";
        if (id === "chat" || id === "code") {
          const uses = usesNow();
          const nextUses = on ? [...uses, id].filter((v, i, a) => a.indexOf(v) === i) : uses.filter(u => u !== id);
          if (!nextUses.length) return;
          prefs.uses = ["chat", "code"].filter(u => nextUses.includes(u));
          await invoke("set_uses", { uses: prefs.uses });
        } else if (id === "memory") {
          prefs.memory_on = on; await invoke("set_memory_on", { on });
        } else if (id === "prompt-lab") {
          if (on) await invoke("app_install_example", { id: "prompt-lab" }).catch(e => toast(String(e), "err"));
          else await invoke("app_remove", { id: "prompt-lab" }).catch(e => toast(String(e), "err"));
          if (window.Apps) await Apps.reload();
        }
        render();
      }));
    }
    if (step === 3) {
      $$("[data-device]", el).forEach(b => (b.onclick = () => {
        const what = b.dataset.device;
        finish(null, what === "share" ? "share" : what === "machines" ? "machines" : "phone");
      }));
    }
  }

  async function next() {
    const key = STEPS[step][0];
    if (key === "projects" && picked.size) {
      const list = prefs.projects || [];
      const path = list.find(p => picked.has(p)) || [...picked][0];
      if (path && path !== (snap && snap.workspace)) {
        await invoke("open_project", { path }).catch(e => toast(String(e), "err"));
        prefs = await invoke("prefs").catch(() => prefs);
      }
    }
    if (step >= STEPS.length - 1) { finish("chat"); return; }
    step += 1; signin = null; render();
  }

  async function finish(then, settings) {
    $("#onboard").hidden = true; $("#win").classList.remove("onboarding");
    prefs.onboarded = true; await invoke("set_onboarded", { done: true });
    if (settings) { openSettings(settings); return; }
    const uses = usesNow();
    if (then === "code" || uses.join() === "code") { setMode("code"); Chat.newChat(); }
    else { setMode("chat"); Chat.newChat(); }
  }

  return {
    async open() {
      step = 0; signin = null; busy = "";
      picked = new Set((prefs.projects || []).slice(0, 1));
      closeSettings(); closePop();
      $("#onboard").hidden = false; $("#win").classList.add("onboarding");
      render();
      devices = await Promise.all([
        invoke("share_status").catch(() => null),
        invoke("machines").catch(() => []),
      ]).then(([share, machines]) => ({ share, machines })).catch(() => null);
      if (step === 3) render();
      await Providers.load().catch(() => {});
      if (!picked.size && (prefs.projects || []).length) picked = new Set([prefs.projects[0]]);
      if (step < 2) render();
      if (window.Apps && !Apps.installed().length) Apps.reload().then(() => { if (step === 2) render(); });
    },
    finish,
  };
})();
