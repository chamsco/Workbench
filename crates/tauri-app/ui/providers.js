// The coding CLIs, local models and routers on this machine, and the Cloud
// plan: one list component used by Setup (onboard.js) and Settings.
//
// Each row shows what the scan found (version, signed in or not, plan) and a
// switch. Chat and projects only offer what is switched on.

var Providers = (() => {
  let list = [], scanning = false, account = null, plans = [], accountErr = null, lastJSON = "";
  const mounts = new Set();

  // Monogram avatars, so rows read at a glance without vendor logos.
  const LOOK = {
    codex: ["CX", "#10a37f"], claude: ["CL", "#d97757"], cursor: ["CU", "#4b5563"], grok: ["GK", "#111827"],
    opencode: ["OP", "#6d28d9"], antigravity: ["AG", "#2563eb"], ollama: ["OL", "#0f766e"], cloud: ["☁", "#2a66d9"],
  };
  const look = id => LOOK[id] || [(id.replace(/^router:/, "")[0] || "R").toUpperCase() + (id.replace(/^router:/, "")[1] || "").toUpperCase(), "#7c3aed"];
  const avatar = (id, cls = "") => { const [t, c] = look(id); return `<span class="pav ${cls}" style="--av:${c}">${esc(t)}</span>`; };

  function statusLine(h) {
    if (!h.installed) return `<span class="pst off">Not installed</span>`;
    if (!h.enabled) return `<span class="pst off">Disabled</span>`;
    const a = { authenticated: "Authenticated", unauthenticated: "Not signed in", unknown: "Installed" }[h.auth];
    return `<span class="pst ${h.auth === "authenticated" ? "ok" : h.auth === "unauthenticated" ? "warn" : ""}">${a}${h.detail ? " · " + esc(h.detail) : ""}</span>`;
  }

  function row(h) {
    const ver = h.version ? `<span class="pver">v${esc(h.version)}</span>` : "";
    const msg = h.message && (!h.enabled || h.auth !== "authenticated")
      ? `<div class="pmsg">${esc(h.message)}${!h.installed && h.install && !h.install.startsWith("http") ? ` <button class="copy" data-copy="${esc(h.install)}" title="Copy">${icon("copy")}</button>` : ""}${!h.installed && h.install.startsWith("http") ? ` <button class="lnk" data-url="${esc(h.install)}">Download</button>` : ""}</div>` : "";
    const models = h.kind !== "cli" && h.enabled && h.models.length ? `<div class="pmodels">${h.models.slice(0, 8).map(m => `<span>${esc(m)}</span>`).join("")}${h.models.length > 8 ? `<span>+${h.models.length - 8}</span>` : ""}</div>` : "";
    const rm = h.kind === "router" ? `<button class="ib" data-rmr="${esc(h.id)}" title="Remove router" aria-label="Remove ${esc(h.name)}">${icon("trash")}</button>` : "";
    return `<div class="prow${h.enabled ? "" : " dim"}" data-id="${esc(h.id)}">
      ${avatar(h.id)}
      <div class="pmain"><div class="pname">${esc(h.name)}${ver}</div>${statusLine(h)}${msg}${models}</div>
      ${rm}<button class="toggle" role="switch" aria-checked="${h.enabled}" data-tog="${esc(h.id)}" aria-label="Use ${esc(h.name)}" ${h.installed || h.kind === "router" ? "" : "disabled"}></button>
    </div>`;
  }

  function html(opts = {}) {
    const clis = list.filter(h => h.kind === "cli");
    const local = list.filter(h => h.kind === "local");
    const routers = list.filter(h => h.kind === "router");
    const ol = local[0];
    const busy = scanning ? `<span class="scan-dot"></span>Scanning…` : `${icon("reload")}Rescan`;
    return `
      ${opts.only !== "local" ? `<div class="pgroup"><div class="pgh"><h4>Coding CLIs</h4><button class="btn sm" data-rescan ${scanning ? "disabled" : ""}>${busy}</button></div>
        ${clis.length ? clis.map(row).join("") : `<div class="pempty">${scanning ? "Looking for Codex, Claude, Cursor, Grok, OpenCode and Antigravity…" : "Nothing scanned yet."}</div>`}</div>` : ""}
      ${opts.only !== "cli" ? `<div class="pgroup"><div class="pgh"><h4>On this machine</h4>${opts.only === "local" ? `<button class="btn sm" data-rescan ${scanning ? "disabled" : ""}>${busy}</button>` : ""}</div>
        ${ol ? row(ol) : `<div class="pempty">Checking for Ollama…</div>`}
        <div class="pline"><span>Ollama address</span><input class="tx mono" id="olUrl" value="${esc(prefs.ollama_url || "http://localhost:11434")}" aria-label="Ollama address"><button class="btn sm" id="olSave">Save</button></div></div>
      <div class="pgroup"><div class="pgh"><h4>Routers</h4></div>
        ${routers.length ? routers.map(row).join("") : `<div class="pempty">Any OpenAI-compatible endpoint: OpenRouter, LM Studio, vLLM, LiteLLM, your company's gateway.</div>`}
        <div class="rpresets">${[["OpenRouter", "https://openrouter.ai/api/v1"], ["LM Studio", "http://localhost:1234/v1"], ["vLLM", "http://localhost:8000/v1"], ["LiteLLM", "http://localhost:4000/v1"]].map(([n, u]) => `<button class="chipb" data-preset="${esc(n)}|${esc(u)}">${esc(n)}</button>`).join("")}</div>
        <div class="rform"><input class="tx" id="rName" placeholder="Name" aria-label="Router name"><input class="tx mono" id="rUrl" placeholder="https://…/v1" aria-label="Base URL"><input class="tx mono" id="rKey" placeholder="API key (optional)" type="password" aria-label="API key"><button class="btn primary sm" id="rAdd">Add & test</button></div>
        <div class="err" id="rErr" role="alert"></div></div>` : ""}`;
  }

  function wire(el, opts) {
    $$("[data-tog]", el).forEach(b => (b.onclick = async () => {
      const id = b.dataset.tog, on = b.getAttribute("aria-checked") !== "true";
      list = await invoke("set_harness", { id, on });
      renderAll(); if (window.Chat) Chat.render();
    }));
    $$("[data-rescan]", el).forEach(b => (b.onclick = rescan));
    $$("[data-copy]", el).forEach(b => (b.onclick = () => { navigator.clipboard.writeText(b.dataset.copy); toast("Copied: " + b.dataset.copy, "ok"); }));
    $$("[data-url]", el).forEach(b => (b.onclick = () => invoke("open_url", { url: b.dataset.url })));
    $$("[data-rmr]", el).forEach(b => (b.onclick = async () => { await invoke("remove_router", { id: b.dataset.rmr }); await load(); }));
    $$("[data-preset]", el).forEach(b => (b.onclick = () => { const [n, u] = b.dataset.preset.split("|"); $("#rName", el).value = n; $("#rUrl", el).value = u; $("#rKey", el).focus(); }));
    const os = $("#olSave", el);
    if (os) os.onclick = async () => { os.disabled = true; await invoke("set_ollama_url", { url: $("#olUrl", el).value }); prefs = await invoke("prefs"); await load(); };
    const ra = $("#rAdd", el);
    if (ra) ra.onclick = async () => {
      const err = $("#rErr", el); err.textContent = ""; ra.disabled = true; ra.textContent = "Testing…";
      try {
        await invoke("add_router", { name: $("#rName", el).value, baseUrl: $("#rUrl", el).value, apiKey: $("#rKey", el).value });
        prefs = await invoke("prefs"); await load(); toast("Router added", "ok");
      } catch (e) { err.textContent = String(e); ra.disabled = false; ra.textContent = "Add & test"; }
    };
  }

  function render(el) {
    const opts = JSON.parse(el.dataset.opts || "{}");
    const keep = {}; $$("input.tx", el).forEach(i => (keep[i.id] = i.value));
    el.innerHTML = html(opts);
    Object.entries(keep).forEach(([id, v]) => { const i = $("#" + id, el); if (i && v) i.value = v; });
    wire(el, opts);
  }
  function renderAll() {
    for (const el of [...mounts]) { if (!el.isConnected) { mounts.delete(el); continue; } render(el); }
    for (const el of [...planMounts]) { if (!el.isConnected) { planMounts.delete(el); continue; } renderPlan(el); }
  }

  async function load() {
    list = await invoke("harnesses").catch(() => []);
    lastJSON = JSON.stringify(list);
    renderAll();
    return list;
  }
  async function rescan() {
    scanning = true; renderAll();
    try { list = await invoke("rescan"); } catch (e) { toast(String(e), "err"); }
    scanning = false; lastJSON = JSON.stringify(list); renderAll();
    if (window.Chat) Chat.render();
    return list;
  }
  // On state events: pick up the background scan without stealing focus.
  async function refreshQuiet() {
    const next = await invoke("harnesses").catch(() => list);
    const j = JSON.stringify(next);
    if (j === lastJSON) return;
    list = next; lastJSON = j;
    const typing = document.activeElement && [...mounts].some(m => m.contains(document.activeElement));
    if (!typing) renderAll();
    if (window.Chat) Chat.renderComposerRoute();
  }

  // ---------------------------------------------------------------- plans
  const planMounts = new Set();
  async function loadAccount() {
    if (!plans.length) plans = await invoke("plans").catch(() => []);
    if (!prefs.cloud || !prefs.cloud.token) { account = null; accountErr = null; return null; }
    try { account = await invoke("cloud_account"); accountErr = null; } catch (e) { accountErr = String(e); account = await invoke("cloud_cached").catch(() => null); }
    return account;
  }
  function usage(a) {
    const pct = Math.min(100, Math.round((a.used / Math.max(1, a.allowance)) * 100));
    const reset = new Date(a.resets_at);
    const when = a.period === "day" ? "resets " + reset.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }) : "resets " + reset.toLocaleDateString([], { month: "short", day: "numeric" });
    return `<div class="meter"><div class="bar"><i style="width:${pct}%"></i></div><span>${a.used} / ${a.allowance} replies this ${a.period} · ${when}${a.payg_usd ? ` · $${a.payg_usd.toFixed(2)} pay-as-you-go` : ""}</span></div>`;
  }
  function renderPlan(el) {
    const signed = prefs.cloud && prefs.cloud.token;
    const cur = account && account.plan;
    const cards = plans.map(p => `<div class="plan${cur === p.plan ? " cur" : ""}${p.plan === "plus" ? " pop" : ""}">
        ${p.plan === "plus" ? `<span class="ribbon">Popular</span>` : ""}
        <div class="pn">${esc(p.name)}</div><div class="pp">${esc(p.price)}</div><div class="pb">${esc(p.blurb)}</div>
        <ul>${p.points.map(x => `<li>${icon("check")}${esc(x)}</li>`).join("")}</ul>
        ${p.plan === "max" ? `<div class="over"><span>Past the allowance</span><div class="segc">${[["ads", "Ads"], ["payg", "Pay as you go"]].map(([k, l]) => `<button data-over="${k}" aria-pressed="${(account ? account.overage : "ads") === k}">${l}</button>`).join("")}</div></div>` : ""}
        <button class="btn ${cur === p.plan ? "" : "primary"} wide" data-plan="${p.plan}" ${cur === p.plan && p.plan !== "max" ? "disabled" : ""}>${cur === p.plan ? (p.plan === "max" ? "Update" : "Current plan") : !signed ? (p.plan === "free" ? "Start free" : `Start with ${esc(p.name)}`) : `Switch to ${esc(p.name)}`}</button>
      </div>`).join("");
    el.innerHTML = `${account ? usage(account) : ""}
      <div class="plans">${cards || `<div class="pempty">Loading plans…</div>`}</div>
      ${accountErr ? `<div class="err">${esc(accountErr)}</div>` : ""}
      <p class="fine">Ads are a separate card under a reply, labelled Sponsored. They are never written into an answer and are not picked from what you type. ${signed ? `<button class="lnk" id="cloudOut">Sign out of Cloud</button>` : ""}</p>
      <p class="fine dev">${icon("warn")}This build talks to the development cloud at <code>${esc((prefs.cloud && prefs.cloud.url) || "")}</code> (<code>backspace-cli cloud</code>). Plans switch instantly; nothing is billed.</p>`;
    let over = account ? account.overage : "ads";
    $$("[data-over]", el).forEach(b => (b.onclick = () => { over = b.dataset.over; $$("[data-over]", el).forEach(x => x.setAttribute("aria-pressed", x === b)); }));
    $$("[data-plan]", el).forEach(b => (b.onclick = async () => {
      b.disabled = true; const label = b.textContent; b.textContent = "…";
      try {
        if (!(prefs.cloud && prefs.cloud.token)) { account = await invoke("cloud_signup"); prefs = await invoke("prefs"); }
        if (b.dataset.plan !== "free" || (account && account.plan !== "free")) account = await invoke("cloud_set_plan", { plan: b.dataset.plan, overage: over });
        accountErr = null; toast(`You're on ${account.plan[0].toUpperCase() + account.plan.slice(1)}`, "ok");
      } catch (e) { accountErr = String(e); b.textContent = label; }
      renderAll(); if (window.Chat) Chat.render();
    }));
    const so = $("#cloudOut", el);
    if (so) so.onclick = async () => { await invoke("cloud_sign_out"); prefs = await invoke("prefs"); account = null; renderAll(); if (window.Chat) Chat.render(); };
  }

  return {
    get list() { return list; },
    get account() { return account; },
    avatar, look, load, rescan, refreshQuiet, loadAccount,
    mount(el, opts = {}) { el.dataset.opts = JSON.stringify(opts); mounts.add(el); render(el); if (!list.length) load(); },
    async mountPlan(el) { planMounts.add(el); renderPlan(el); await loadAccount(); renderPlan(el); },
    setAccount(a) { account = a; },
  };
})();
