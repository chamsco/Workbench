// tools/verify/sidebar-and-canvas.js
//
// The three things the owner asked for after seeing it running:
//   1. the sidebar is the ADE's, in both surfaces, with no project row above the
//      icons
//   2. the folder icon opens the Projects menu, not the OS folder dialog
//   3. the IDE lands on an empty canvas that asks what to open; the ADE keeps the
//      zone's home
(async () => {
  const H = window.__H;
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const WS = "C:\\Users\\chamy\\Documents\\code\\ProjectHalo";
  const route = { kind: "cli", provider: "claude", model: null };
  const mainAgent = {
    id: 0, key: "0", parent: null, depth: 0, title: "Main agent", kind: "main", status: "idle",
    branch: "backspace/run", ticket: null, worktree: null, escalations: [], cost_usd: 0.42,
    input_tokens: 12000, output_tokens: 3400, advisor_calls: 0,
    decision: { model: "claude-opus-4-6", effort: "high", source: "preset", confidence: 0.9, router_cost_usd: 0, note: null },
    log: [], artifacts: [],
  };
  H.mock("snapshot", () => ({
    name: "local", local: true, workspace: WS, agents: [mainAgent], approvals: [],
    tickets: [], artifacts: [], options: [], board: [], link: { state: "online", error: null },
    index: 0, total_cost_usd: 0.42, router_cost_usd: 0,
  }));
  H.mock("machines", () => ([{ index: 0, name: "This machine", local: true, url: "", selected: true, project: WS, link: { state: "online", error: null }, running: 0, pending: 0 }]));
  H.mock("harnesses", () => ([{ id: "claude", name: "Claude Code", version: "2.1.301", kind: "cli", installed: true, enabled: true, auth: "authenticated", chat: true, detail: "Opus 5.5", message: null }]));
  H.mock("git_changes", () => ({ branch: "backspace/run", fresh: false, files: [] }));
  H.mock("git_activity", () => ({ days: [0, 1, 0, 2, 1, 0, 0, 1, 0, 0, 2, 0, 1, 0], added: 40, removed: 9 }));
  H.mock("worktrees", () => ({ root: WS, worktrees: [] }));
  H.mock("list_files", () => []);
  H.mock("chat_list", () => ([
    { id: "t1", title: "Rename the settings card", updated: Date.now() - 6e5, pinned: false, route, preview: "Done", busy: false, unread: false, project: WS },
  ]));
  H.mock("agents_list", () => []);
  // Exactly what `TabSpec::defaults()` hands a project seen for the first time.
  H.mock("tabs_for", () => ({ tabs: [{ name: null, panes: [{ kind: "empty", agent: 0, url: null }], layout: null }], active: 0 }));
  H.mock("save_tabs", () => null);
  prefs.worker = "claude-code";
  prefs.cli_permission = "edits";
  delete prefs.wb_view;
  await useTabsFor(WS);
  await Providers.load().catch(() => {});
  await refresh(true);

  const strip = (s) => (s || "").replace(/\s+/g, " ").trim();
  const side = () => {
    const el = document.querySelector("#chatList");
    return {
      topRow: strip(document.querySelector("#pairProj")?.textContent) || null,
      pairProjGone: !document.querySelector("#pairProj"),
      // The icons are the first row: three buttons with aria-labels, no text.
      labels: [...(el?.querySelectorAll("button[aria-label]") ?? [])].map((b) => b.getAttribute("aria-label")).slice(0, 3),
      rows: [...(el?.querySelectorAll("button") ?? [])].map((b) => strip(b.textContent)).filter(Boolean).slice(0, 8),
    };
  };

  setMode("code");
  setCodeView("agents");
  await settle(800);
  const ide = {
    view: strip(document.querySelector("#viewBtn")?.textContent),
    canvas: strip(document.querySelector("#grid")?.textContent).slice(0, 120),
    chooserHeading: strip(document.querySelector(".chooser h5")?.textContent),
    chooserCards: [...document.querySelectorAll(".chooser .card .ct")].map((e) => strip(e.textContent)),
    panes: document.querySelectorAll(".pane").length,
    side: side(),
  };

  // The folder icon: the Projects menu, not the folder dialog.
  const folderBtn = [...document.querySelectorAll("#chatList button[aria-label]")].find((b) => b.getAttribute("aria-label") === "Projects");
  let projectsMenu = null;
  let dialogCalled = false;
  H.mock("pick_folder", () => { dialogCalled = true; return null; });
  if (folderBtn) {
    folderBtn.click();
    await settle(300);
    projectsMenu = strip(document.querySelector("#pop")?.textContent) || null;
    document.body.click();
    await settle(200);
  }

  setCodeView("pair");
  await settle(800);
  const chat = document.querySelector("#chat");
  const ade = {
    hero: strip(chat?.querySelector("h1")?.textContent),
    homeAskLines: chat ? chat.querySelectorAll(".home .hc textarea").length : -1,
    ownComposer: chat ? chat.querySelectorAll(".code-composer textarea").length : -1,
    side: side(),
  };

  return { ide, ade, projectsMenu, openedFolderDialog: dialogCalled };
})()
