// tools/verify/ghost-real.js
//
// Acceptance: a ghost chat collapses the sidebar on the way in and puts it back
// on the way out, driven through the chat face's own public API
// (window.WhirlChat.open / .home) rather than by adding the class by hand — the
// wiring is half of the bug.
//
// It also covers the case that made the collapse stop working for a whole
// session: once the resize gutter has been dragged, --side-w is an inline style,
// and an inline custom property beats every selector. A stale inline --side-w is
// simulated too, so the fix cannot regress into "works until you drag".
(async () => {
  const H = window.__H;
  const win = document.querySelector("#win");
  const side = document.querySelector("#side");
  const mainc = document.querySelector(".mainc");
  const frames = (n) => new Promise((r) => { let left = n; const tick = () => (--left <= 0 ? r() : requestAnimationFrame(tick)); requestAnimationFrame(tick); });
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const px = (v) => Math.round(v * 1000) / 1000;

  const ghost = {
    id: "g1", title: "Ghost", created: 1, updated: 1, pinned: false,
    route: { kind: "cli", provider: "claude", model: null }, messages: [],
    life: "ghost", project: null, app: null, system: null, agent: null,
    members: [], goal: null, expires_at: null, ttl_minutes: null,
    branched_from: null, cli_session: null,
  };
  H.mock("chat_thread", (a) => (a && a.id === "g1" ? ghost : null));
  H.mock("chat_list", () => []);
  H.mock("agents_list", () => []);

  const read = () => {
    const w = getComputedStyle(win);
    return {
      class: win.className.split(" ").filter((c) => c === "ghost-side" || c === "side-closed").join(" ") || "-",
      grid: w.gridTemplateColumns,
      sideW: px(side.getBoundingClientRect().width),
      maincLeft: px(mainc.getBoundingClientRect().left),
      sideVisible: getComputedStyle(side).visibility,
    };
  };
  const collapsed = (r) => r.grid.startsWith("0px") && r.sideW === 0 && r.sideVisible === "hidden";

  const out = {};

  // The app boots with the sidebar closed (app.js: setSide(false)); open it, or
  // there is nothing to collapse, and drag it wide as the gutter would.
  win.classList.remove("side-closed");
  win.style.setProperty("--side-w-user", "320px");
  await settle(450);
  out.opened = read();

  // Open a ghost through the real path: WhirlChat.open -> chat_thread -> the
  // ChatApp effect -> host.ghostSide(true).
  window.WhirlChat.open("g1");
  await frames(2);
  out.ghostMidTransition = read();
  await settle(500);
  out.ghostOpen = read();
  out.ghostCollapsed = collapsed(out.ghostOpen);

  // A stale inline --side-w: what the old gutter wrote. The track must still go
  // to zero, which is why the collapse rules set grid-template-columns directly.
  win.style.setProperty("--side-w", "320px");
  await settle(400);
  out.ghostOpenStaleInline = read();
  out.ghostCollapsedWithStaleInline = collapsed(out.ghostOpenStaleInline);
  win.style.removeProperty("--side-w");

  // Leaving: the app's own GhostToggle calls remove() + goHome().
  window.WhirlChat.home();
  await settle(500);
  out.afterLeave = read();
  out.leaveRestored = out.afterLeave.grid.startsWith("320px") && out.afterLeave.sideW > 0 && out.afterLeave.sideVisible === "visible";

  out.calls = H.calls().map((c) => c.cmd).filter((c) => c === "chat_thread");
  return out;
})()
