// tools/verify/ghost-flow.js
//
// Acceptance: a ghost chat is a one-off. Whirl's incognito (MIT,
// whirlchat/whirl: apps/legacy/app/lib/incognito.tsx) hides the shell the moment
// the mode is on, creates nothing until the first message, and destroys the
// ephemeral thread when you leave — by any route, not just its own button.
//
// Drives the real chat face through window.WhirlChat and the real composer, so
// the wiring is under test and not a class added by hand.
(async () => {
  const H = window.__H;
  const win = document.querySelector("#win");
  const side = document.querySelector("#side");
  const frames = (n) => new Promise((r) => { let left = n; const tick = () => (--left <= 0 ? r() : requestAnimationFrame(tick)); requestAnimationFrame(tick); });
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const px = (v) => Math.round(v * 1000) / 1000;
  const calls = (cmd) => H.calls().filter((c) => c.cmd === cmd);

  window.confirm = () => true;

  const thread = (over) => ({
    id: "g1", title: "New chat", created: 1, updated: 1, pinned: false,
    route: { kind: "cli", provider: "claude", model: null }, messages: [],
    life: "ghost", project: null, app: null, system: null, agent: null,
    members: [], goal: null, expires_at: null, ttl_minutes: null,
    branched_from: null, cli_session: null, ...over,
  });
  H.mock("chat_new", (a) => thread({ life: (a && a.scope && a.scope.life) || "keep" }));
  H.mock("chat_thread", (a) => (a && a.id === "g1" ? thread() : null));
  H.mock("chat_list", () => []);
  H.mock("agents_list", () => []);

  // One installed CLI, or the face shows "connect a model" instead of a
  // composer and there is nothing to send from.
  const CLI = [{ id: "claude", name: "Claude Code", kind: "cli", enabled: true, chat: true, version: "1.0.0", detail: "local", models: [] }];
  H.mock("harnesses", () => CLI);
  H.mock("rescan", () => CLI);
  await window.Providers.rescan();
  await window.WhirlChat.refresh();
  await settle(400);

  const read = () => {
    const w = getComputedStyle(win);
    const btn = document.querySelector(".ghost-toggle");
    return {
      class: win.className.split(" ").filter((c) => c === "ghost-side" || c === "side-closed").join(" ") || "-",
      grid: w.gridTemplateColumns,
      sideW: px(side.getBoundingClientRect().width),
      sideVisible: getComputedStyle(side).visibility,
      toggle: !!btn,
      pressed: btn ? btn.getAttribute("aria-pressed") : null,
    };
  };
  const collapsed = (r) => r.grid.startsWith("0px") && r.sideW === 0 && r.sideVisible === "hidden";

  const out = {};

  // The app boots with the sidebar closed; open it so there is something to
  // collapse, and drag it wide as the gutter would.
  win.classList.remove("side-closed");
  win.style.setProperty("--side-w-user", "320px");
  await settle(450);
  out.opened = read();

  // 1. Turning the mode on hides the shell at once — before anything is typed.
  document.querySelector(".ghost-toggle").click();
  await settle(450);
  out.modeOn = read();
  out.modeOnCollapsed = collapsed(out.modeOn);
  out.modeOnPressed = out.modeOn.pressed === "true";

  // 2. Type and send. The thread must be created as a ghost, and must not be
  //    listed (chat_list drops it) once it exists.
  const ta = document.querySelector('textarea[aria-label="Message"]');
  if (!ta) {
    out.noComposer = true;
    out.body = document.body.innerText.slice(0, 300);
    return out;
  }
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value").set;
  setter.call(ta, "what is a semaphore");
  ta.dispatchEvent(new Event("input", { bubbles: true }));
  await frames(2);
  ta.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  await settle(600);
  out.sent = read();
  out.createdScope = (calls("chat_new")[0] || {}).args || null;
  out.sentCollapsed = collapsed(out.sent);
  out.inThread = !!document.querySelector(".ghost-toggle[aria-pressed='true']");

  // 3. Leaving through the app's own home() — the path a sidebar click takes —
  //    must destroy the ghost, not merely stop showing it.
  const before = calls("chat_delete").length;
  window.WhirlChat.home();
  await settle(600);
  out.left = read();
  out.deletesOnLeave = calls("chat_delete").length - before;
  out.leftRestored = out.left.grid.startsWith("320px") && out.left.sideW > 0 && out.left.sideVisible === "visible";
  out.ghostGone = !win.classList.contains("ghost-side");

  // 4. Any ghost left behind in the core is forgotten when the mode is entered
  //    again — Whirl's purgeIncognito on enter.
  document.querySelector(".ghost-toggle").click();
  await settle(150);
  out.purgeOnEnter = calls("chat_purge_ghosts").length;
  out.toggleCalls = calls("chat_new").length + calls("chat_delete").length;
  return out;
})()
