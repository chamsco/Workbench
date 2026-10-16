// tools/verify/shot.js — a settled frame: Code -> Chat, sidebar open, agents on
// the home. Transitions are switched off first, or a full-page capture repaints
// mid-animation and the frame lies.
(async () => {
  const H = window.__H;
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const WS = "C:\\Users\\chamy\\Documents\\code\\ProjectHalo";
  const route = { kind: "cli", provider: "claude", model: null };
  const agent = (id, name, job, emoji, color) => ({
    id, name, job, avatar: { emoji, color }, route, shared: [], memory: true,
    computer: { kind: "folder", image: "", desktop: false, host: "", user: "", port: 0, key: "", desktop_url: "" },
    off: [], apps_off: [], hook: "", created: 1, updated: 1,
  });

  H.mock("snapshot", () => ({ name: "local", local: true, workspace: WS, agents: [], approvals: [], tickets: [], link: { state: "online", error: null }, index: 0, total_cost_usd: 0, router_cost_usd: 0 }));
  H.mock("machines", () => ([{ index: 0, name: "This machine", local: true, url: "", selected: true, project: WS, link: { state: "online", error: null }, running: 0, pending: 0 }]));
  H.mock("git_activity", () => ({ days: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], added: 0, removed: 0 }));
  H.mock("git_changes", () => ({ branch: "main", files: [], added: 0, removed: 0 }));
  H.mock("chat_list", () => ([
    { id: "t1", title: "Rename the settings card", updated: Date.now() - 6e5, pinned: false, route, preview: "Done — the card is one column now", busy: false, unread: false, project: WS },
    { id: "t2", title: "Why is the loader off-centre?", updated: Date.now() - 9e6, pinned: false, route, preview: "The panel was a sibling of the dock", busy: false, unread: false, project: WS },
  ]));
  H.mock("agents_list", () => ([
    agent("a1", "Scout", "Reads the repo and reports what changed", "🔭", "#7c9cf5"),
    agent("a2", "Builder", "Implements a ticket on its own branch", "🔨", "#f0a040"),
    agent("a3", "Reviewer", "Reviews the diff before it merges", "🔍", "#5ec48a"),
  ]));
  prefs.tabs = [{ name: "", panes: [], layout: null }];
  prefs.active_tab = 0;

  await refresh(true);
  setMode("code");
  setCodeView("pair");
  document.querySelector("#win").classList.remove("side-closed"); // the app boots closed
  await settle(800);

  const noMotion = document.createElement("style");
  noMotion.textContent = "*,*::before,*::after{transition:none!important;animation:none!important}";
  document.head.appendChild(noMotion);
  // A full-page capture resizes the viewport, and the app's own resize listener
  // then collapses the sidebar (`setSide(!narrow())`). Pin narrow() to false so
  // the frame shows the layout that was measured.
  const realMM = window.matchMedia.bind(window);
  window.matchMedia = (q) =>
    /max-width/.test(q)
      ? { matches: false, media: q, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, onchange: null, dispatchEvent: () => false }
      : realMM(q);
  await settle(300);
  document.querySelector("#win").classList.remove("side-closed");
  await settle(400);

  const side = document.querySelector("#side").getBoundingClientRect();
  return { sidebar: { left: Math.round(side.left), width: Math.round(side.width) }, title: document.title };
})()
