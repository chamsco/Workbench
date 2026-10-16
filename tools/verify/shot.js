// tools/verify/shot.js — a settled frame of Code's two surfaces.
//   ?shot=ade  (default)  Code -> ADE: the project's own home
//   ?shot=ide             Code -> IDE: one full-window session canvas
// Run with --url "/__harness/index.html?shot=ide".
//
// Transitions are frozen first: a full-page capture re-lays the page out, and a
// frame taken mid-animation is a frame that lies.
(async () => {
  const H = window.__H;
  const IDE = location.search.includes("ide");
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const WS = "C:\\Users\\chamy\\Documents\\code\\ProjectHalo";
  const route = { kind: "cli", provider: "claude", model: null };

  const mainAgent = {
    id: 0, key: "0", parent: null, depth: 0, title: "Main agent", kind: "main", status: "idle",
    branch: "backspace/run", ticket: null, worktree: null, escalations: [], cost_usd: 0.42,
    input_tokens: 12000, output_tokens: 3400, advisor_calls: 0,
    decision: { model: "claude-opus-4-6", effort: "high", source: "preset", confidence: 0.9, router_cost_usd: 0, note: null },
    log: IDE ? [
      { kind: "user", text: "The settings card is one column on a wide window — make it two." },
      { kind: "tool_call", text: "Read crates/tauri-app/ui/style.css" },
      { kind: "tool_call", text: "Edit crates/tauri-app/ui/style.css  +12 −3" },
      { kind: "assistant", text: "The card was pinned to a single column at every width. It is a two-column grid above 720px now, and the padding scales with it." },
    ] : [],
    artifacts: [],
  };

  H.mock("snapshot", () => ({
    name: "local", local: true, workspace: WS, agents: [mainAgent], approvals: [],
    tickets: [
      { key: "T-3", title: "Rename the settings card", state: "in_progress" },
      { key: "T-4", title: "The loader is off-centre", state: "ready_for_human" },
    ],
    artifacts: [], options: [], board: [], link: { state: "online", error: null },
    index: 0, total_cost_usd: 0.42, router_cost_usd: 0,
  }));
  H.mock("machines", () => ([{ index: 0, name: "This machine", local: true, url: "", selected: true, project: WS, link: { state: "online", error: null }, running: 0, pending: 0 }]));
  H.mock("harnesses", () => ([{ id: "claude", name: "Claude Code", version: "2.1.301", kind: "cli", installed: true, enabled: true, auth: "authenticated", chat: true, detail: "Opus 5.5", message: null }]));
  H.mock("git_changes", () => ({ branch: "backspace/run", fresh: false, files: [
    { status: "M", path: "crates/tauri-app/ui/style.css", added: 12, removed: 3 },
    { status: "A", path: "crates/tauri-app/web/src/app/CodeHome.tsx", added: 180, removed: 0 },
    { status: "M", path: "crates/tauri-app/ui/shell.css", added: 9, removed: 0 },
  ] }));
  H.mock("git_activity", () => ({ days: [1, 0, 3, 2, 0, 4, 1, 0, 2, 1, 0, 3, 2, 1], added: 900, removed: 300 }));
  H.mock("worktrees", () => ({ root: WS, worktrees: [{ agent: "T-3", title: "Rename the settings card", branch: "backspace/t-3", path: WS + "/.backspace/worktrees/t-3", exists: true, size_bytes: 1 }] }));
  H.mock("list_files", () => []);
  H.mock("chat_list", () => ([
    { id: "t1", title: "Rename the settings card", updated: Date.now() - 6e5, pinned: false, route, preview: "Done — the card is one column now", busy: false, unread: true, project: WS },
    { id: "t2", title: "Why is the loader off-centre?", updated: Date.now() - 9e6, pinned: false, route, preview: "The panel was a sibling of the dock", busy: false, unread: false, project: WS },
  ]));
  H.mock("agents_list", () => []);
  H.mock("tabs_for", () => ({ tabs: [{ name: null, panes: [{ kind: "agent", agent: 0, url: null }], layout: null }], active: 0 }));
  H.mock("save_tabs", () => null);
  prefs.worker = "claude-code";
  prefs.cli_permission = "edits";
  await useTabsFor(WS);

  // Providers are loaded by Chat.init() at boot, before this fixture installs
  // the harness scan; without them the face shows "Connect a model" over
  // everything. Load them, then make the face read them again.
  await Providers.load().catch(() => {});
  await refresh(true);
  setMode("code");
  setCodeView(IDE ? "agents" : "pair");
  if (IDE) setWb("canvases");
  await refresh(true);
  document.querySelector("#win").classList.remove("side-closed"); // the app boots closed
  await settle(900);

  const noMotion = document.createElement("style");
  noMotion.textContent = "*,*::before,*::after{transition:none!important;animation:none!important}";
  document.head.appendChild(noMotion);
  // A full-page capture resizes the viewport, and the app's own resize listener
  // then collapses the sidebar. Pin narrow() to false so the frame shows the
  // layout that was measured.
  const realMM = window.matchMedia.bind(window);
  window.matchMedia = (q) =>
    /max-width/.test(q)
      ? { matches: false, media: q, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, onchange: null, dispatchEvent: () => false }
      : realMM(q);
  await settle(300);
  document.querySelector("#win").classList.remove("side-closed");
  await settle(400);

  const side = document.querySelector("#side").getBoundingClientRect();
  return {
    shot: IDE ? "ide" : "ade",
    sidebar: { left: Math.round(side.left), width: Math.round(side.width) },
    title: document.title,
    view: document.querySelector("#modes button[aria-selected=true]")?.textContent.trim() ?? null,
    panes: document.querySelectorAll(".pane").length,
  };
})()
