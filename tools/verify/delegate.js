// tools/verify/delegate.js — acceptance for the Delegate sheet
// (docs/DELEGATION-2026-10-18.md): the mockup's board, driven through the real
// chat face. Unassigned tasks, the fleet with each bot's model and minute
// price, drag-to-assign, add-a-task, and the Auto switch.
//
// The Tauri commands are mocked in window.__H exactly like
// tools/verify/agents-projects.js; the board, its grouping and its prices are
// computed here to mirror crates/core/src/delegate.rs. Every assertion is a
// returned check, never a throw.
//
// Run (from the repo root):
//
//   node tools/uiharness/run.mjs --wait 900 \
//     --eval-file tools/verify/delegate.js \
//     --shot tools/verify/out/delegate.png
(async () => {
  const H = window.__H;
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const frames = (n = 3) => H.frames(n);
  const waitUI = async (ms = 350) => {
    await frames(3);
    await settle(ms);
  };
  const strip = (s) => (s || "").replace(/\s+/g, " ").trim();
  const txt = (el) => strip(el ? el.innerText || el.textContent : "");
  const visible = (el) => !!el && el.getBoundingClientRect().width > 0 && el.getBoundingClientRect().height > 0;
  const root = (sel) => document.querySelector(sel);
  const dialogEl = () => [...document.querySelectorAll('[data-slot="dialog-content"],[role="dialog"]')].find(visible) || null;
  const dialogText = () => txt(dialogEl());
  const btn = (re, scope) => [...(scope || document).querySelectorAll("button,[role=button],a")].find((n) => visible(n) && re.test(txt(n))) || null;
  const calls = (cmd) => H.calls().filter((c) => c.cmd === cmd);
  const lastArgs = (cmd) => {
    const l = calls(cmd);
    return l.length ? l[l.length - 1].args : null;
  };

  const report = { checks: [], failed: 0, calls: [], notes: [] };
  const check = (name, ok, detail) => {
    report.checks.push({ name, ok: !!ok, ...(detail === undefined || detail === null ? {} : { detail: String(detail).slice(0, 500) }) });
    return !!ok;
  };
  const tail = (s) => strip(s).slice(-240);

  // ---------------------------------------------------------------- mocks
  const CLI = [{ id: "claude", name: "Claude Code", kind: "cli", enabled: true, chat: true, version: "2.1.301", detail: "local", models: [] }];
  H.mock("harnesses", () => CLI);
  H.mock("rescan", () => CLI);
  H.mock("chat_list", () => []);

  const now = Date.now();
  const route = (model) => ({ kind: "router", provider: "openrouter", model });
  const computer = () => ({ kind: "folder", image: "", desktop: false, host: "", user: "", port: 0, key: "", desktop_url: "" });
  const agent = (id, name, emoji, color, model) => ({
    id,
    name,
    job: `${name} helps.\n\nLonger backstory.`,
    avatar: { emoji, color },
    personality: "",
    route: route(model),
    shared: [],
    computer: computer(),
    memory: true,
    off: [],
    apps_off: [],
    hook: "",
    routines: [],
    created: now - 30 * 86_400_000,
    updated: now - 86_400_000,
  });
  const BOTS = [
    agent("a-researcher", "Researcher", "🔬", "#f97316", "claude-opus-5-5"),
    agent("a-writer", "Writer", "✍️", "#e11d48", "claude-sonnet-5-5"),
    agent("a-coder", "Coder", "🛠️", "#2563eb", "gpt-6-sol"),
    agent("a-ops", "Ops", "📈", "#16a34a", "gemini-3.8-flash"),
  ];
  H.mock("agents_list", () => BOTS);
  H.mock("agent_projects", () => []);

  // The board, computed the way delegate.rs groups and prices it.
  const PRICE = { "a-researcher": 0.08, "a-writer": 0.03, "a-coder": 0.06, "a-ops": 0.01 };
  let seq = 100;
  const task = (title, est, agentId, note = "") => ({
    id: `t${seq++}`,
    title,
    detail: "",
    est_min: est,
    agent: agentId || null,
    source: agentId ? "auto" : "",
    note,
    ticket: "",
    created: now + seq,
    updated: now + seq,
  });
  let board = {
    auto: false,
    tasks: [
      task("Update pricing page", 10, null),
      task("Clean up CRM duplicates", 18, null),
      task("Competitor scan", 12, "a-researcher", "auto · Researcher had the shortest queue (0 min)"),
      task("Fix flaky checkout test", 8, "a-writer", "auto · Writer had the shortest queue (0 min)"),
      task("Refactor auth", 20, "a-coder", "auto · Coder had the shortest queue (0 min)"),
      task("Upgrade Stripe SDK", 15, "a-coder", "auto · Coder had the shortest queue (20 min)"),
      task("Summarize churn calls", 25, "a-coder", "auto · Coder had the shortest queue (35 min)"),
    ],
  };

  const INITIAL = JSON.parse(JSON.stringify(board));
  const snapshot = () => {
    const agents = BOTS.map((b) => {
      const tasks = board.tasks.filter((t) => t.agent === b.id);
      const queued = tasks.reduce((n, t) => n + t.est_min, 0);
      const usd = PRICE[b.id] || 0;
      return {
        id: b.id,
        name: b.name,
        job: b.job.split("\n")[0],
        emoji: b.avatar.emoji,
        color: b.avatar.color,
        model: b.route.model,
        provider: b.route.provider,
        usd_per_min: usd,
        free: false,
        queued_min: queued,
        cost_usd: queued * usd,
        busy_until: queued ? now + queued * 60_000 : 0,
        tasks,
      };
    });
    return {
      auto: board.auto,
      unassigned: board.tasks.filter((t) => !t.agent),
      agents,
      queues_usd: agents.reduce((n, a) => n + a.cost_usd, 0),
      queued_min: board.tasks.reduce((n, t) => n + t.est_min, 0),
    };
  };
  const autoAssign = () => {
    const order = [...board.tasks].filter((t) => !t.agent).sort((a, b) => a.created - b.created);
    for (const t of order) {
      const pick = [...BOTS]
        .map((b) => ({
          b,
          load: board.tasks.filter((x) => x.agent === b.id).reduce((n, x) => n + x.est_min, 0),
        }))
        .sort((x, y) => x.load - y.load || (PRICE[x.b.id] || 0) - (PRICE[y.b.id] || 0))[0];
      t.agent = pick.b.id;
      t.source = "auto";
      t.note = `auto · ${pick.b.name} had the shortest queue (${pick.load} min)`;
    }
  };

  H.mock("delegate_state", () => snapshot());
  H.mock("delegate_add", (a) => {
    board.tasks.push(task(a.title, a.estMin || 10, null));
    if (board.auto) autoAssign();
    return snapshot();
  });
  H.mock("delegate_delete", (a) => {
    board.tasks = board.tasks.filter((t) => t.id !== a.id);
    return snapshot();
  });
  H.mock("delegate_assign", (a) => {
    const t = board.tasks.find((x) => x.id === a.id);
    if (t) {
      t.agent = a.agent || null;
      t.source = a.agent ? "manual" : "";
      t.note = a.agent ? "you moved it" : "";
    }
    return snapshot();
  });
  H.mock("delegate_auto", (a) => {
    board.auto = a.on;
    if (a.on) autoAssign();
    return snapshot();
  });
  H.mock("delegate_import", () => snapshot());

  // ---------------------------------------------------------------- boot
  await H.frames(4);
  try {
    await window.Providers.rescan();
  } catch (e) {
    report.notes.push(`Providers.rescan failed: ${String(e)}`);
  }
  await window.WhirlChat.refresh();
  await waitUI(400);

  const win = root("#win");
  win.classList.remove("side-closed");
  win.style.setProperty("--side-w-user", "320px");
  await waitUI(450);

  const switcher = root('#modesSide [data-view="agents"]') || root('#modes [data-view="agents"]');
  if (switcher) switcher.click();
  else if (typeof window.setChatView === "function") window.setChatView("agents");
  await waitUI(800);
  check("switch.agentsOn", win.classList.contains("agents-on"), win.className);

  const side = root("#chatList");
  check("sidebar.delegateRow", !!btn(/^Delegate\b/, side), tail(txt(side)));

  // ---------------------------------------------------------------- open
  const opened = (() => {
    const b = btn(/^Delegate\b/, side);
    if (!b) return false;
    b.click();
    return true;
  })();
  await waitUI(700);
  check("sheet.open", opened && !!dialogEl(), opened ? tail(dialogText()) : "no Delegate row in the sidebar");
  const d = dialogText();
  check("sheet.title", /^Delegate\b/.test(d), tail(d));
  check("sheet.subtitle", /Drag a task onto an agent/.test(d), tail(d));
  check("sheet.autoSwitch", !!root("[data-delegate-auto]"), "no [data-delegate-auto]");
  check("sheet.unassigned", /Unassigned/.test(d), tail(d));
  check("sheet.agentsHeading", /\bAgents\b/.test(d), tail(d));

  const unassignedCards = () => [...document.querySelectorAll("[data-delegate-drop='unassigned'] [data-delegate-task]")];
  const agentRow = (id) => document.querySelector(`[data-delegate-agent="${id}"]`);

  check("sheet.twoUnassigned", unassignedCards().length === 2, `${unassignedCards().length} cards`);
  check(
    "sheet.unassignedTitles",
    unassignedCards().some((c) => /Update pricing page/.test(txt(c))) && unassignedCards().some((c) => /Clean up CRM duplicates/.test(txt(c))),
    unassignedCards().map(txt).join(" | "),
  );
  check("sheet.countBadge", /Unassigned\s*2/.test(d), tail(d));

  check("sheet.fourAgents", BOTS.every((b) => new RegExp(`\\b${b.name}\\b`).test(d)), tail(d));
  check("sheet.models", /Opus 5\.5/.test(d) && /Sonnet 5\.5/.test(d) && /GPT 6 Sol/.test(d) && /Gemini 3\.8 Flash/.test(d), tail(d));
  check("sheet.prices", /~\$0\.08\/min/.test(d) && /~\$0\.03\/min/.test(d), tail(d));
  check("sheet.busy", (d.match(/Busy until \d{2}:\d{2}/g) || []).length >= 3, tail(d));
  check("sheet.free", /\bFree\b/.test(d), tail(d));
  check("sheet.queues", /\$[\d.]+ in queues/.test(d), tail(d));
  check("sheet.chips", ["Competitor scan", "Fix flaky checkout test", "Refactor auth", "Upgrade Stripe SDK", "Summarize churn calls"].every((t) => new RegExp(t).test(d)), tail(d));

  // ---------------------------------------------------------------- drag
  const dt = new DataTransfer();
  const fire = (el, type) => el && el.dispatchEvent(new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer: dt }));
  const first = unassignedCards()[0];
  const beforeAgent = "a-writer";
  let dragged = false;
  if (first && agentRow(beforeAgent)) {
    fire(first, "dragstart");
    fire(agentRow(beforeAgent), "dragover");
    fire(agentRow(beforeAgent), "drop");
    fire(first, "dragend");
    dragged = true;
  }
  await waitUI(700);
  const assignedArgs = lastArgs("delegate_assign");
  check("drag.callsAssign", dragged && !!assignedArgs && assignedArgs.agent === beforeAgent, JSON.stringify(assignedArgs));
  check("drag.cardMoved", unassignedCards().length === 1, `${unassignedCards().length} remain unassigned`);
  const writerText = txt(agentRow(beforeAgent));
  check("drag.chipOnAgent", /Update pricing page/.test(writerText), tail(writerText));
  check("drag.whyRecorded", (() => {
    const c = [...document.querySelectorAll("[data-delegate-chip]")].find((n) => /Update pricing page/.test(txt(n)));
    return !!c && /you moved it/.test(c.getAttribute("title") || "");
  })(), "chip title should say who moved it");

  // ---------------------------------------------------------------- add
  const input = dialogEl() && dialogEl().querySelector("input[aria-label='New task']");
  const setter = input && Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
  if (input) {
    setter.call(input, "Draft launch note");
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }
  await waitUI(250);
  const addBtn = dialogEl() && dialogEl().querySelector("button[aria-label='Add task']");
  if (addBtn) addBtn.click();
  await waitUI(600);
  check("add.callsAdd", !!lastArgs("delegate_add") && /Draft launch note/.test(JSON.stringify(lastArgs("delegate_add"))), JSON.stringify(lastArgs("delegate_add")));
  check("add.cardAdded", unassignedCards().some((c) => /Draft launch note/.test(txt(c))), unassignedCards().map(txt).join(" | "));

  // Keyboard path: the card's select assigns without a drag.
  const sel = dialogEl() && dialogEl().querySelector("select[aria-label^='Assign ']");
  if (sel) {
    const setSel = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value").set;
    setSel.call(sel, "a-ops");
    sel.dispatchEvent(new Event("change", { bubbles: true }));
  }
  await waitUI(600);
  check("select.callsAssign", !!lastArgs("delegate_assign") && lastArgs("delegate_assign").agent === "a-ops", JSON.stringify(lastArgs("delegate_assign")));

  // ---------------------------------------------------------------- auto
  const sw = root("[data-delegate-auto]");
  if (sw) sw.click();
  await waitUI(700);
  const autoArgs = lastArgs("delegate_auto");
  check("auto.callsOn", !!autoArgs && autoArgs.on === true, JSON.stringify(autoArgs));
  check("auto.emptiesUnassigned", unassignedCards().length === 0, `${unassignedCards().length} still waiting`);
  check("auto.switchOn", (root("[data-delegate-auto]") || {}).getAttribute?.("aria-checked") === "true", "aria-checked should be true");

  // Leave the sheet as the mockup draws it: two waiting, everyone with work,
  // Auto off. Close and reopen so the face reads the restored board.
  board = JSON.parse(JSON.stringify(INITIAL));
  // The reference art is light; the harness boots dark, so draw the shot the
  // way the mockup is drawn.
  document.documentElement.dataset.theme = "light";
  document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  await waitUI(500);
  const again = btn(/^Delegate\b/, side);
  if (again) again.click();
  await waitUI(800);

  check("sheet.reopenedForShot", unassignedCards().length === 2, `${unassignedCards().length} unassigned after the reset`);

  report.calls = H.calls().filter((c) => c.cmd.startsWith("delegate_")).map((c) => c.cmd);
  report.failed = report.checks.filter((c) => !c.ok).length;
  window.__DELEGATE = { report, dialogText, unassignedCards, agentRow };
  return report;
})()
