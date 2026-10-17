// tools/verify/agents-projects.js — acceptance for Chat's Agents side after
// docs/AGENTS-PROJECTS-ROUTINES-2026-10-17.md: bots with a name/face/job, the
// Start From sheet, the bot page with "Start its job", routines on a clock,
// projects that collect a folder's bots, chats and routines, and the two
// controls a bot can put in a reply (a question with options, and a routine
// proposal) drawn by ask.tsx from fenced JSON.
//
// Driven through the real UI (the Chat -> Agents switch in #modesSide, the
// sidebar rows, the real dialogs), with the Tauri commands mocked in
// window.__H exactly like tools/verify/ghost-flow.js. Every assertion is a
// returned check, never a throw: a missing feature produces ok:false with the
// text that was actually on screen, not a pageerror.
//
// Run (from the repo root):
//
//   # the whole walk; leaves the Agents home open and screenshots it
//   node tools/uiharness/run.mjs --wait 900 \
//     --eval-file tools/verify/agents-projects.js \
//     --shot tools/verify/out/agents-home.png
//
//   # the same walk, then leaves the busy bot's page open
//   node tools/uiharness/run.mjs --wait 900 \
//     --eval-file tools/verify/agents-projects.js \
//     --eval-file tools/verify/agents-bot-page.js \
//     --shot tools/verify/out/agents-bot-page.png
//
//   # the same walk, then leaves the New bot sheet open (template picked)
//   node tools/uiharness/run.mjs --wait 900 \
//     --eval-file tools/verify/agents-projects.js \
//     --eval-file tools/verify/agents-new-bot.js \
//     --shot tools/verify/out/agents-new-bot.png
//
//   # the same walk, then leaves Herald's thread with the bot's question card
//   # and routine proposal on screen
//   node tools/uiharness/run.mjs --wait 900 \
//     --eval-file tools/verify/agents-projects.js \
//     --eval-file tools/verify/agents-ask.js \
//     --shot tools/verify/out/agents-ask.png
//
// Screenshots: tools/verify/out/agents-home.png,
//              tools/verify/out/agents-bot-page.png,
//              tools/verify/out/agents-new-bot.png,
//              tools/verify/out/agents-ask.png.
// Exit code 0 means the page booted and nothing threw; read the RESULT report
// (`failed` + `checks[].ok`) for the acceptance verdict.
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
  const visible = (el) => {
    if (!el) return false;
    if (el.hidden) return false;
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== "hidden";
  };
  const root = (sel) => document.querySelector(sel);
  const sideRoot = () => root("#chatList");
  const mainRoot = () => root("#chat");
  const sideText = () => txt(sideRoot());
  // One labelled section of the sidebar ("Bots", "Chats", "Projects"); the
  // project rows are only reachable that way because a chat thread can be
  // titled "Almanac …" too.
  const sideSection = (label) => {
    const sections = [...((sideRoot() && sideRoot().querySelectorAll("section")) || [])];
    return sections.find((s) => new RegExp("^" + label + "\\b", "i").test(txt(s))) || null;
  };
  const mainText = () => txt(mainRoot());
  // Dialogs: the whirl components render base-ui popups with this slot; the
  // role fallback keeps it working if that attribute ever changes.
  const dialogEl = () =>
    [...document.querySelectorAll('[data-slot="dialog-content"],[role="dialog"]')].find(visible) || null;
  const dialogText = () => txt(dialogEl());
  const btn = (re, scope) => {
    const nodes = [...(scope || document).querySelectorAll('button,[role="button"],[role="tab"],[role="menuitem"],[role="option"],a')];
    return nodes.find((n) => visible(n) && re.test(strip(n.innerText || n.textContent))) || null;
  };

  const clickBtn = async (re, scope, ms = 450) => {
    const b = btn(re, scope);
    if (!b) return false;
    b.click();
    await waitUI(ms);
    return true;
  };
  const fill = (el, value) => {
    if (!el) return false;
    const proto = el.tagName === "TEXTAREA" ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(proto, "value").set;
    setter.call(el, value);
    el.dispatchEvent(new Event("input", { bubbles: true }));
    return true;
  };
  const calls = (cmd) => H.calls().filter((c) => c.cmd === cmd);
  const lastArgs = (cmd) => {
    const list = calls(cmd);
    return list.length ? list[list.length - 1].args : null;
  };

  const report = { checks: [], failed: 0, calls: [], notes: [] };
  const check = (name, ok, detail) => {
    report.checks.push({
      name,
      ok: !!ok,
      ...(detail === undefined || detail === null ? {} : { detail: String(detail).slice(0, 500) }),
    });
    return !!ok;
  };
  const skip = (name, why) => check(name, false, `not reached: ${why}`);
  // A short tail of what was on screen, for a failing check's detail.
  const tail = (s) => strip(s).slice(-220);

  // ---------------------------------------------------------------- mocks
  // One installed CLI, or the face boots to "connect a model" with no composer
  // and no route for a new bot; the Agents side is still reachable but not the
  // thing under test.
  const ROUTE = { kind: "cli", provider: "claude", model: null };
  const CLI = [{ id: "claude", name: "Claude Code", kind: "cli", enabled: true, chat: true, version: "2.1.301", detail: "local", models: [] }];
  H.mock("harnesses", () => CLI);
  H.mock("rescan", () => CLI);

  const now = Date.now();
  const DAY = 86_400_000;
  // Tomorrow at 08:00 local: nextWakeLabel -> "tomorrow at 08:00".
  const tomorrow8 = (() => {
    const d = new Date(now);
    d.setDate(d.getDate() + 1);
    d.setHours(8, 0, 0, 0);
    return d.getTime();
  })();
  // Nine days out. nextWakeLabel spells 2-6 days out as a weekday ("Mon at
  // 08:00") — localized, so untestable — and only uses "in N days" from a week
  // on; nine days exercises that branch.
  const nineDaysOut = (() => {
    const d = new Date(now + 9 * DAY);
    d.setHours(11, 30, 0, 0);
    return d.getTime();
  })();
  const twoHoursOut = now + 2 * 3_600_000;

  const OFFSET = -new Date().getTimezoneOffset();
  const routine = (o) => ({
    id: "r",
    name: "Morning digest",
    instructions: "Write today's brief: what changed, what's due, the one thing to do first.",
    kind: "daily",
    time: "08:00",
    days: [],
    every_minutes: 0,
    project: "",
    enabled: true,
    last_run: now - DAY,
    next_run: now + DAY,
    runs: 4,
    utc_offset_minutes: OFFSET,
    created: now - 7 * DAY,
    updated: now - DAY,
    ...o,
  });
  const blankComputer = () => ({ kind: "folder", image: "", desktop: false, host: "", user: "", port: 0, key: "", desktop_url: "" });
  const agent = (o) => ({
    id: "a",
    name: "Bot",
    job: "Does a job.",
    avatar: { emoji: "🙂", color: "#64748b" },
    personality: "",
    route: ROUTE,
    shared: [],
    computer: blankComputer(),
    memory: true,
    off: [],
    apps_off: [],
    hook: "",
    routines: [],
    created: now - 30 * DAY,
    updated: now - DAY,
    ...o,
  });

  // Five bots, not four: the four status lines the sidebar must show
  // (Working… / Needs you / Wakes … / Watching) cannot all be carried by four
  // rows when two of them are the two wake variants, so the fifth carries the
  // unread thread's "Needs you".
  const HERALD = agent({
    id: "a-herald",
    name: "Herald",
    job: "Writes me a short morning brief: what changed overnight, what's due today, and the one thing to do first.",
    avatar: { emoji: "🌅", color: "#ca8a04" },
    personality: "Direct and brief. Leads with the answer, skips the pleasantries, and says plainly when something's wrong.",
    // Named "Daily brief", not "Morning digest": the question-card thread later
    // proposes a routine called "Morning digest", and a bot that already has a
    // routine of that name shows the proposal as already Added.
    routines: [routine({ id: "r-herald", name: "Daily brief", kind: "daily", time: "08:00", next_run: nineDaysOut })],
  });
  const COURIER = agent({
    id: "a-courier",
    name: "Courier",
    job: "Triages my inbox: what needs a reply today, what can wait, and what to archive.",
    avatar: { emoji: "✉️", color: "#e11d48" },
    routines: [
      routine({ id: "r-courier", name: "Inbox sweep", kind: "daily", time: "17:00", next_run: tomorrow8, project: "p-almanac" }),
    ],
  });
  // The bot page is driven from Keeper's thread: it has a routine (so the page
  // shows cards) and a busy thread (so its status line is "Working…").
  const KEEPER = agent({
    id: "a-keeper",
    name: "Keeper",
    job: "Watches my builds and test runs, and tells me when one breaks, what broke it, and the smallest fix.",
    avatar: { emoji: "🛠️", color: "#2563eb" },
    routines: [routine({ id: "r-keeper", name: "Build watch", kind: "every", time: "", every_minutes: 120, next_run: twoHoursOut })],
  });
  const SCOUT = agent({
    id: "a-scout",
    name: "Scout",
    job: "Watches the repos and speaks up when something needs me.",
    avatar: { emoji: "🧭", color: "#16a34a" },
  });
  const TALLY = agent({
    id: "a-tally",
    name: "Tally",
    job: "Runs my standup: what I finished, what's next, and what is stuck.",
    avatar: { emoji: "📈", color: "#0891b2" },
  });
  const BOTS = [HERALD, COURIER, KEEPER, SCOUT, TALLY];

  const threadInfo = (o) => ({
    id: "t",
    title: "Chat",
    updated: now - 3_600_000,
    pinned: false,
    route: ROUTE,
    preview: "",
    busy: false,
    unread: false,
    project: null,
    app: null,
    agent: null,
    members: [],
    ...o,
  });
  const ALMANAC = "C:\\Users\\tester\\Documents\\Almanac";
  const KESTREL = "C:\\Users\\tester\\Documents\\Kestrel";
  const T_BUSY = threadInfo({ id: "t-busy", title: "Watch the build", agent: "a-keeper", busy: true, updated: now - 60_000, preview: "Running the test suite" });
  const T_UNREAD = threadInfo({ id: "t-unread", title: "Inbox triage", agent: "a-tally", unread: true, updated: now - 120_000, preview: "Three things need a reply." });
  const T_GROUP = threadInfo({ id: "t-group", title: "Launch week", updated: now - 500_000, preview: "Herald and Courier", members: ["a-herald", "a-courier"] });
  const T_PROJECT = threadInfo({ id: "t-project", title: "Almanac launch notes", agent: "a-courier", project: ALMANAC, updated: now - 300_000, preview: "The brief is in the folder." });
  // The bot asked a question and proposed a routine in its reply (fenced ask /
  // routine JSON, see docs/AGENTS-PROJECTS-ROUTINES-2026-10-17.md and ask.tsx).
  // Herald's own thread, so clicking its sidebar row opens the cards.
  const T_ASK = threadInfo({ id: "t-ask", title: "Morning setup", agent: "a-herald", updated: now - 1_800_000, preview: "I need one thing before I start." });
  // Two agent threads (one busy, one unread), one group, one project thread,
  // and the question-card thread.
  const THREADS = [T_BUSY, T_UNREAD, T_GROUP, T_PROJECT, T_ASK];

  const project = (o) => ({ id: "p", name: "Project", folder: "C:\\", machine: "", agents: [], created: now - 9 * DAY, updated: now - DAY, ...o });
  const ALMANAC_P = project({ id: "p-almanac", name: "Almanac", folder: ALMANAC, agents: ["a-herald", "a-courier"] });
  const KESTREL_P = project({ id: "p-kestrel", name: "Kestrel", folder: KESTREL, agents: [] });
  const PROJECTS = [ALMANAC_P, KESTREL_P];

  // Full threads for chat_thread: the bot page needs an agent thread with no
  // messages, and the project chat needs agent + project.
  const fullThread = (o) => ({
    id: "t",
    title: "Chat",
    created: now - DAY,
    updated: now - 3_600_000,
    pinned: false,
    route: ROUTE,
    messages: [],
    life: "keep",
    expires_at: null,
    ttl_minutes: null,
    branched_from: null,
    project: null,
    app: null,
    system: null,
    agent: null,
    members: [],
    goal: null,
    ...o,
  });
  const FULL = {
    "t-busy": fullThread({ id: "t-busy", title: "Watch the build", agent: "a-keeper" }),
    "t-unread": fullThread({ id: "t-unread", title: "Inbox triage", agent: "a-tally" }),
    "t-group": fullThread({ id: "t-group", title: "Launch week", members: ["a-herald", "a-courier"] }),
    "t-project": fullThread({ id: "t-project", title: "Almanac launch notes", agent: "a-courier", project: ALMANAC }),
  };

  // The two cards the bot can put in a reply: a question with options and a
  // routine proposal, as the fenced JSON the bot is told to write. The message
  // list is mutable so answering the card can append the user turn — that is
  // what makes the card draw itself as answered on the refresh that follows.
  const msg = (o) => ({
    id: "m",
    role: "assistant",
    text: "",
    at: now,
    status: "done",
    reactions: [],
    attachments: [],
    error: null,
    reply_to: null,
    model: null,
    ad: null,
    cost_usd: null,
    via: null,
    ...o,
  });
  const ASK_JSON = {
    question: "Which topics should your digest cover?",
    multiple: true,
    options: ["AI and coding agents", "Apple and the Mac", "Open source", "Startups and funding"],
  };
  const ROUTINE_JSON = { name: "Morning digest", instructions: "Brief me in two minutes.", kind: "daily", time: "08:00" };
  const ASK_TEXT = `I need one thing before I start.\n\n\`\`\`ask\n${JSON.stringify(ASK_JSON)}\n\`\`\``;
  const ROUTINE_TEXT = `When you're ready, I can wake up for this.\n\n\`\`\`routine\n${JSON.stringify(ROUTINE_JSON)}\n\`\`\``;
  const askMsgs = [];
  const resetAsk = () => {
    askMsgs.length = 0;
    askMsgs.push(
      msg({ id: "m-ask-question", text: ASK_TEXT, at: now - 900_000 }),
      msg({ id: "m-ask-routine", text: ROUTINE_TEXT, at: now - 800_000 }),
    );
  };
  resetAsk();

  H.mock("agents_list", () => BOTS);
  H.mock("chat_list", () => THREADS);
  H.mock("agent_projects", () => PROJECTS);
  H.mock("routines_sync", () => BOTS);
  H.mock("chat_thread", (a) => {
    if (!a) return null;
    if (a.id === "t-ask") return fullThread({ id: "t-ask", title: "Morning setup", agent: "a-herald", messages: askMsgs.slice() });
    return FULL[a.id] || null;
  });
  H.mock("chat_new", (a) => {
    const scope = (a && a.scope) || {};
    return fullThread({ id: "t-new", title: "New chat", agent: scope.agent || null, project: scope.project || null });
  });
  H.mock("chat_send", (a) => {
    // A card's answer is a real user turn; appending it is what turns the card
    // into its "Answered" state on the refresh that follows.
    if (a && a.id === "t-ask" && typeof a.text === "string" && a.text) {
      askMsgs.push(msg({ id: `m-ask-user-${askMsgs.length}`, role: "user", text: a.text, at: Date.now() }));
    }
    return null;
  });
  H.mock("agent_save", (a) => (a && a.agent) || BOTS[0]);
  H.mock("routine_save", (a) => BOTS.find((b) => a && b.id === a.agent) || BOTS[0]);
  H.mock("agent_project_assign", (a) => PROJECTS.find((p) => p.id === (a && a.project)) || PROJECTS[0]);

  // Populate the face's store from the mocks (the app boots before an eval can
  // set them), and make sure a chat route exists for the composer.
  try {
    await window.Providers.rescan();
  } catch (e) {
    report.notes.push(`Providers.rescan failed: ${String(e)}`);
  }
  await window.WhirlChat.refresh();
  await waitUI(400);

  // ---------------------------------------------------------------- switch
  const win = root("#win");
  // The sidebar can boot closed; the switch lives in it.
  win.classList.remove("side-closed");
  win.style.setProperty("--side-w-user", "320px");
  await waitUI(500);

  const switcher =
    root('#modesSide [data-view="agents"]') || root('#modes [data-view="agents"]');
  let switchHow = "missing";
  if (switcher) {
    switchHow = "click";
    switcher.click();
  } else if (typeof window.setChatView === "function") {
    switchHow = "setChatView";
    window.setChatView("agents");
  }
  await waitUI(800);
  check("switch.method", switchHow !== "missing", `switch path used: ${switchHow}`);
  check("switch.agentsOn", win.classList.contains("agents-on"), `#win class: ${win.className}`);
  check("switch.sidebarMounted", visible(sideRoot()), visible(sideRoot()) ? "#chatList visible" : "#chatList is not visible/sized");
  const side0 = sideText();
  check("switch.sidebarHasBots", /\bBots\b/.test(side0), tail(side0));

  // ---------------------------------------------------------------- sidebar
  check("sidebar.sections", /\bBots\b/.test(side0) && /\bChats\b/.test(side0) && /\bProjects\b/.test(side0), tail(side0));
  check("sidebar.newBotRow", !!btn(/new bot/i, sideRoot()), `sidebar: ${tail(side0)}`);
  check("sidebar.section.bots", /\bBots\b/.test(side0), tail(side0));
  check("sidebar.section.chats", /\bChats\b/.test(side0), tail(side0));
  check("sidebar.section.projects", /\bProjects\b/.test(side0), tail(side0));

  // Each bot's row: its name, then the status text agentStatus gives it.
  const rowText = (name) => {
    const b = btn(new RegExp(name, "i"), sideRoot());
    return b ? strip(b.innerText) : "";
  };
  const statuses = [
    ["Keeper", "Working", /Working/i],
    ["Tally", "Needs you", /Needs you/i],
    ["Courier", "Wakes tomorrow at 08:00", /Wakes tomorrow at 08:00/i],
    ["Herald", "Wakes in N days", /Wakes in \d+ days/i],
    ["Scout", "Watching", /Watching/i],
  ];
  for (const [name, label, re] of statuses) {
    const row = rowText(name);
    check(`sidebar.status.${name}`, !!row && re.test(row), `expected "${label}" in row: ${row || "(row not found)"}`);
  }

  // ---------------------------------------------------------------- home
  const home = mainText();
  check("home.yourBots", /Your bots/i.test(home), `main pane: ${tail(home)}`);
  const homeBots = BOTS.filter((b) => new RegExp(b.name, "i").test(home)).map((b) => b.name);
  check("home.botCards", homeBots.length === BOTS.length, `cards shown for: ${homeBots.join(", ") || "none"} of ${BOTS.map((b) => b.name).join(", ")}`);
  check("home.upcomingRoutines", /Upcoming routines/i.test(home) && /New routine/i.test(home), `main pane: ${tail(home)}`);
  check("home.upcomingNextWake", /tomorrow at 08:00/i.test(home) && /in \d+ days/i.test(home), `main pane: ${tail(home)}`);
  check(
    "home.projectCards",
    PROJECTS.every((p) => new RegExp(p.name, "i").test(home)),
    `expected ${PROJECTS.map((p) => p.name).join(" + ")}; main pane: ${tail(home)}`,
  );
  check("home.groupsKept", /Launch week/i.test(home), `main pane: ${tail(home)}`);
  const projectsSide = sideSection("Projects");
  check(
    "sidebar.projectsListed",
    !!projectsSide && PROJECTS.every((p) => new RegExp(p.name, "i").test(txt(projectsSide))),
    `projects section: ${txt(projectsSide) || "(section not found)"}`,
  );

  // ---------------------------------------------------------------- new bot sheet
  let stage = "new bot";
  const openedNewBot = await clickBtn(/new bot/i, sideRoot(), 650);
  const sheet = dialogEl();
  check("newBot.opens", openedNewBot && !!sheet, openedNewBot ? "New bot row found but the sheet did not open" : `sidebar: ${tail(sideText())}`);
  const sheetText0 = dialogText();
  if (sheet) {
    check("newBot.title", /New bot/i.test(sheetText0), tail(sheetText0));
    check("newBot.startFrom", /Start From/i.test(sheetText0), tail(sheetText0));
    const TEMPLATE_NAMES = ["Blank", "Morning digest", "Research", "Build watcher", "Inbox triage", "Explainer", "Standup"];
    const tiles = [...sheet.querySelectorAll("button")].filter((b) => visible(b) && TEMPLATE_NAMES.some((n) => new RegExp(n, "i").test(txt(b))));
    check("newBot.startFromTiles", tiles.length >= 6, `${tiles.length} template tiles found (need >= 6)`);
    const picked = await clickBtn(/Morning digest/i, dialogEl(), 500);
    check("newBot.templatePicked", picked, `tiles: ${tiles.map((b) => txt(b).slice(0, 30)).join(" | ")}`);
    const sheet2 = dialogEl();
    const sheetText = dialogText();
    check("newBot.templateSummary", /Change/i.test(sheetText) && /Morning digest/i.test(sheetText), tail(sheetText));
    const nameInput = sheet2 ? sheet2.querySelector("input") : null;
    check("newBot.prefilledName", !!nameInput && nameInput.value === "Herald", `name field: ${nameInput ? JSON.stringify(nameInput.value) : "(no input)"}`);
    const sections = ["Name & job", "Look", "Personality", "Job", "Model", "Lives On"];
    const missing = sections.filter((s) => !new RegExp(s, "i").test(sheetText));
    check("newBot.sections", missing.length === 0, missing.length ? `missing sections: ${missing.join(", ")}; sheet: ${tail(sheetText)}` : "all sections present");
    check("newBot.firstRoutineOffered", /Add its first routine/i.test(sheetText), `sheet: ${tail(sheetText)}`);
    check("newBot.createButton", !!btn(/Create bot/i, sheet2), `sheet: ${tail(sheetText)}`);
    const cancelled = await clickBtn(/^Cancel$/i, dialogEl(), 450);
    check("newBot.cancelCloses", cancelled && !dialogEl(), "Cancel did not close the sheet");
    check("newBot.cancelDoesNotSave", calls("agent_save").length === 0, `agent_save called ${calls("agent_save").length}x`);
    const home2 = mainText();
    check("newBot.returnsHome", /Your bots/i.test(home2), `main pane: ${tail(home2)}`);
  } else {
    for (const n of [
      "newBot.title",
      "newBot.startFrom",
      "newBot.startFromTiles",
      "newBot.templatePicked",
      "newBot.templateSummary",
      "newBot.prefilledName",
      "newBot.sections",
      "newBot.firstRoutineOffered",
      "newBot.createButton",
      "newBot.cancelCloses",
      "newBot.cancelDoesNotSave",
      "newBot.returnsHome",
    ]) {
      skip(n, stage);
    }
  }

  // ---------------------------------------------------------------- bot page
  stage = "bot page";
  const sendsBefore = calls("chat_send").length;
  const openedBot = await clickBtn(/Keeper/i, sideRoot(), 700);
  const botPage = mainText();
  check("botPage.opens", openedBot && /Keeper/i.test(botPage), openedBot ? "Keeper row found but the page did not open" : `sidebar: ${tail(sideText())}`);
  check("botPage.startItsJob", /Start its job/i.test(botPage), `main pane: ${tail(botPage)}`);
  check("botPage.copy", /keeps this conversation, remembers what it learns/i.test(botPage), `main pane: ${tail(botPage)}`);
  check("botPage.status", /Working/i.test(botPage), `main pane: ${tail(botPage)}`);
  check("botPage.routineCard", /Build watch/i.test(botPage) && /Every 2 hours/i.test(botPage), `main pane: ${tail(botPage)}`);
  check("botPage.routineNextWake", /next wake/i.test(botPage), `main pane: ${tail(botPage)}`);
  check("botPage.runNow", /Run now/i.test(botPage), `main pane: ${tail(botPage)}`);
  // "Start its job" has to actually send the kickoff into the thread.
  const startJob = btn(/Start its job/i, mainRoot());
  if (startJob) {
    startJob.click();
    await waitUI(650);
    const sent = lastArgs("chat_send");
    check(
      "botPage.startJobSends",
      calls("chat_send").length > sendsBefore && /Start your job now/i.test(JSON.stringify(sent || {})),
      `chat_send args: ${JSON.stringify(sent)}`,
    );
  } else {
    skip("botPage.startJobSends", "no Start its job button");
  }

  // ---------------------------------------------------------------- routine dialog
  stage = "routine dialog";
  const openedRoutine = await clickBtn(/Add routine/i, mainRoot(), 600);
  check("routine.opens", openedRoutine && !!dialogEl(), openedRoutine ? "Add routine found but no dialog" : `main pane: ${tail(mainText())}`);
  const rd0 = dialogEl();
  const rdText0 = dialogText();
  if (rd0) {
    check("routine.step1", /What it does/i.test(rdText0) && /Step 1 of 3/i.test(rdText0), tail(rdText0));
    check("routine.step1Fields", !!rd0.querySelector("input") && !!rd0.querySelector("textarea"), "name input / instructions textarea missing");
    // Name is required before Next; fill it like a user would.
    fill(rd0.querySelector("input"), "Harness check routine");
    fill(rd0.querySelector("textarea"), "Check the build logs and report the first failure in one line.");
    await waitUI(150);
    const toWhen = await clickBtn(/^Next\b/i, dialogEl(), 500);
    const rdWhen = dialogText();
    check("routine.nextWorks", toWhen && /Step 2 of 3/i.test(rdWhen), toWhen ? `step 2 text: ${tail(rdWhen)}` : "Next button missing/disabled on step 1");
    check("routine.stepWhen", /When/i.test(rdWhen) && /How often/i.test(rdWhen), tail(rdWhen));
    check("routine.whenControls", /Every day/i.test(rdWhen) && /Weekdays/i.test(rdWhen) && /Weekly/i.test(rdWhen) && !!dialogEl()?.querySelector('input[type="time"]'), tail(rdWhen));
    check("routine.whenPreview", /Next wake:/i.test(rdWhen), tail(rdWhen));
    const toWhere = await clickBtn(/^Next\b/i, dialogEl(), 500);
    const rdWhere = dialogText();
    check("routine.stepWhere", toWhere && /Step 3 of 3/i.test(rdWhere) && /Where it works/i.test(rdWhere), toWhere ? tail(rdWhere) : "Next button missing/disabled on step 2");
    check("routine.whereSavedBy", /Save routine/i.test(rdWhere), tail(rdWhere));
    check("routine.whereOwnFolder", /Its own folder/i.test(rdWhere), tail(rdWhere));
    const back = await clickBtn(/^Back\b/i, dialogEl(), 500);
    const rdBack = dialogText();
    check("routine.backWorks", back && /Step 2 of 3/i.test(rdBack) && /How often/i.test(rdBack), back ? tail(rdBack) : "Back button missing");
    const closed = await clickBtn(/^Cancel$/i, dialogEl(), 450);
    check("routine.closes", closed && !dialogEl(), "Cancel did not close the routine dialog");
    check("routine.cancelDoesNotSave", calls("routine_save").length === 0, `routine_save called ${calls("routine_save").length}x`);
  } else {
    for (const n of [
      "routine.step1",
      "routine.step1Fields",
      "routine.nextWorks",
      "routine.stepWhen",
      "routine.whenControls",
      "routine.whenPreview",
      "routine.stepWhere",
      "routine.whereSavedBy",
      "routine.whereOwnFolder",
      "routine.backWorks",
      "routine.closes",
      "routine.cancelDoesNotSave",
    ]) {
      skip(n, stage);
    }
  }

  // ---------------------------------------------------------------- ask + routine cards
  // A bot can put two controls in a reply (ask.tsx): a question with options the
  // reader answers, and a routine it proposes adding. Both ride fenced JSON in
  // the assistant's text, and the answer is a normal user turn in the thread.
  stage = "ask cards";
  const askCard = () => [...document.querySelectorAll('[aria-label="Question from the bot"]')].find(visible) || null;
  const proposalCard = () => [...document.querySelectorAll('[aria-label="Routine the bot proposes"]')].find(visible) || null;
  const askOption = (text) => {
    const card = askCard();
    if (!card) return null;
    return [...card.querySelectorAll('[role="checkbox"],[role="radio"]')].find((b) => visible(b) && new RegExp(text, "i").test(txt(b))) || null;
  };
  const openedAskThread = await clickBtn(/Herald/i, sideRoot(), 800);
  check("ask.threadOpens", openedAskThread && /Herald/i.test(mainText()), openedAskThread ? `main pane: ${tail(mainText())}` : `sidebar: ${tail(sideText())}`);
  check(
    "ask.card",
    !!askCard() && /Which topics should your digest cover\?/i.test(txt(askCard())),
    askCard() ? `card: ${tail(txt(askCard()))}` : `no [aria-label="Question from the bot"]; main pane: ${tail(mainText())}`,
  );
  const askOptions = askCard() ? [...askCard().querySelectorAll('[role="checkbox"],[role="radio"]')].filter(visible) : [];
  const ASK_OPTION_TEXTS = ["AI and coding agents", "Apple and the Mac", "Open source", "Startups and funding"];
  check(
    "ask.options",
    askOptions.length === 4 && ASK_OPTION_TEXTS.every((t) => askOptions.some((b) => new RegExp(t, "i").test(txt(b)))),
    `${askOptions.length} options: ${askOptions.map((b) => txt(b)).join(" | ") || "(none)"}`,
  );
  // The shell stylesheet's unlayered `.grid { height: 100% }` (Code's canvas
  // container) used to beat Tailwind's `.grid` utility inside the chat face and
  // squash this 16px checkbox box to 16x2 — the option rendered as a dash.
  // Scoped to #grid now; keep the box square.
  const firstBox = askOptions[0] ? askOptions[0].querySelector("span") : null;
  const boxRect = firstBox ? firstBox.getBoundingClientRect() : null;
  check(
    "ask.checkboxSquare",
    !!boxRect && Math.abs(boxRect.width - 16) <= 1.5 && Math.abs(boxRect.height - 16) <= 1.5,
    boxRect
      ? `${Math.round(boxRect.width)}x${Math.round(boxRect.height)} px (expected 16x16; the shell .grid height:100% bug made it 16x2)`
      : "no checkbox span on the first option",
  );
  const answerAtStart = btn(/^Answer$/i, askCard());
  check(
    "ask.controls",
    !!btn(/^Skip$/i, askCard()) && !!answerAtStart && answerAtStart.disabled,
    `Skip=${!!btn(/^Skip$/i, askCard())} Answer=${!!answerAtStart} answerDisabled=${answerAtStart ? answerAtStart.disabled : "?"}`,
  );
  const openSource = askOption("Open source");
  if (openSource) {
    openSource.click();
    await waitUI(300);
  }
  const answerAfterPick = btn(/^Answer$/i, askCard());
  const pickedBox = askOption("Open source");
  check(
    "ask.optionSelects",
    !!pickedBox && pickedBox.getAttribute("aria-checked") === "true" && !!answerAfterPick && !answerAfterPick.disabled,
    `openSourceChecked=${pickedBox ? pickedBox.getAttribute("aria-checked") : "?"} answerDisabled=${answerAfterPick ? answerAfterPick.disabled : "?"}`,
  );
  const sendsBeforeAnswer = calls("chat_send").length;
  if (answerAfterPick) {
    answerAfterPick.click();
    await waitUI(800);
  }
  const answerArgs = lastArgs("chat_send");
  check(
    "ask.answers",
    calls("chat_send").length > sendsBeforeAnswer && /Answer: Open source/.test(JSON.stringify(answerArgs || {})),
    `chat_send args: ${JSON.stringify(answerArgs)}`,
  );
  // The refresh after the answer brings the user turn back, and a question card
  // with a user message after it must draw as spent, not as live options.
  const answeredCard = askCard();
  const leftoverOptions = answeredCard ? [...answeredCard.querySelectorAll('[role="checkbox"],[role="radio"]')].filter(visible) : [];
  check(
    "ask.answeredAfterUserturn",
    !!answeredCard && /Answered/i.test(txt(answeredCard)) && leftoverOptions.length === 0,
    answeredCard ? `card: ${tail(txt(answeredCard))} options=${leftoverOptions.length}` : "question card vanished",
  );

  const proposalText = txt(proposalCard());
  check(
    "routineProposal.card",
    !!proposalCard() && /Morning digest/i.test(proposalText) && /Every day at 08:00/i.test(proposalText),
    proposalCard() ? `card: ${tail(proposalText)}` : `no [aria-label="Routine the bot proposes"]; main pane: ${tail(mainText())}`,
  );
  check(
    "routineProposal.controls",
    !!btn(/^Add routine$/i, proposalCard()) && !!btn(/^Not now$/i, proposalCard()),
    `card: ${tail(proposalText)}`,
  );
  const savesBeforeProposal = calls("routine_save").length;
  const addProposal = btn(/^Add routine$/i, proposalCard());
  if (addProposal) {
    addProposal.click();
    await waitUI(800);
  }
  const savedRoutine = lastArgs("routine_save");
  check(
    "routineProposal.adds",
    calls("routine_save").length > savesBeforeProposal &&
      !!savedRoutine &&
      !!savedRoutine.routine &&
      savedRoutine.routine.name === "Morning digest" &&
      savedRoutine.routine.kind === "daily" &&
      savedRoutine.routine.time === "08:00",
    `routine_save args: ${JSON.stringify(savedRoutine)}`,
  );

  // ---------------------------------------------------------------- project page
  stage = "project page";
  const projSide = sideSection("Projects");
  const openedProject = projSide ? await clickBtn(new RegExp(ALMANAC_P.name, "i"), projSide, 750) : false;
  const projectPage = mainText();
  check("project.opens", openedProject && /Almanac/i.test(projectPage), openedProject ? "Almanac row found but the page did not open" : `sidebar: ${tail(sideText())}`);
  check("project.header", /Almanac/i.test(projectPage) && projectPage.includes(ALMANAC), `main pane: ${tail(projectPage)}`);
  check("project.machine", /This Mac/i.test(projectPage), `main pane: ${tail(projectPage)}`);
  check("project.actions", /Edit/i.test(projectPage) && /Remove/i.test(projectPage), `main pane: ${tail(projectPage)}`);
  check("project.botsSection", /Bots on this project/i.test(projectPage), `main pane: ${tail(projectPage)}`);
  check("project.botsListed", /Herald/i.test(projectPage) && /Courier/i.test(projectPage), `main pane: ${tail(projectPage)}`);
  check("project.chatsSection", /\bChats\b/i.test(projectPage), `main pane: ${tail(projectPage)}`);
  check("project.chatsListed", /Almanac launch notes/i.test(projectPage), `main pane: ${tail(projectPage)}`);
  check("project.routinesSection", /Routines on this project/i.test(projectPage), `main pane: ${tail(projectPage)}`);
  check("project.routinesListed", /Inbox sweep/i.test(projectPage), `main pane: ${tail(projectPage)}`);
  check("project.addBot", /Add bot/i.test(projectPage), `main pane: ${tail(projectPage)}`);
  check("project.newChat", /New chat/i.test(projectPage), `main pane: ${tail(projectPage)}`);

  // Back to the Agents home, so the screenshot this run leaves behind is the
  // home (and a tiny follow-up eval can start from it).
  const wentBack = await clickBtn(/^Bots$/i, mainRoot(), 600);
  const backHome = mainText();
  check("project.backToHome", wentBack && /Your bots/i.test(backHome), wentBack ? `main pane: ${tail(backHome)}` : "no back-to-bots row in the project page");

  // ---------------------------------------------------------------- wiring
  const cmds = [...new Set(H.calls().map((c) => c.cmd))];
  check("wiring.agentsList", cmds.includes("agents_list"), `calls: ${cmds.join(", ")}`);
  check("wiring.chatList", cmds.includes("chat_list"), `calls: ${cmds.join(", ")}`);
  check("wiring.agentProjects", cmds.includes("agent_projects"), `calls: ${cmds.join(", ")}`);
  check("wiring.routinesSync", cmds.includes("routines_sync"), `calls: ${cmds.join(", ")}`);

  // ---------------------------------------------------------------- report
  report.failed = report.checks.filter((c) => !c.ok).length;
  report.calls = cmds;

  // The screenshot eval files run after this one in the same uiharness
  // invocation; they drive these helpers from the home state left above.
  const openBot = async (name = "Keeper") => {
    const b = btn(new RegExp(name, "i"), sideRoot());
    if (!b) return false;
    b.click();
    await waitUI(750);
    return /Start its job/i.test(mainText()) || new RegExp(name, "i").test(mainText());
  };
  const openNewBot = async () => {
    const b = btn(/new bot/i, sideRoot());
    if (!b) return false;
    b.click();
    await waitUI(700);
    return !!dialogEl();
  };
  const pickTemplate = async (name = "Morning digest") => {
    const ok = await clickBtn(new RegExp(name, "i"), dialogEl(), 500);
    return ok && !!dialogEl();
  };
  window.__AGENTS = {
    report,
    waitUI,
    frames,
    settle,
    sideText,
    mainText,
    dialogText,
    dialogEl,
    openBot,
    openNewBot,
    pickTemplate,
    // Put the question-card thread back to unanswered, so a screenshot run
    // taken after this one's Answer click still shows the live options.
    resetAsk,
    state: () => ({
      agentsOn: win.classList.contains("agents-on"),
      dialogOpen: !!dialogEl(),
      side: sideText().slice(0, 200),
      main: mainText().slice(0, 240),
    }),
  };
  return report;
})()
