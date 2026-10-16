// tools/verify/ade-ide.js — Code's two surfaces, end to end.
//   ADE  (pair):   the switch says ADE, and the home is the project's own screen
//   IDE  (agents): one full-window session canvas with the session card and the
//                  mode/agents status line
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
    tickets: [{ key: "T-3", title: "Rename the settings card", state: "in_progress" }],
    artifacts: [], options: [], board: [], link: { state: "online", error: null },
    index: 0, total_cost_usd: 0.42, router_cost_usd: 0,
  }));
  H.mock("machines", () => ([{ index: 0, name: "This machine", local: true, url: "", selected: true, project: WS, link: { state: "online", error: null }, running: 0, pending: 0 }]));
  H.mock("harnesses", () => ([{ id: "claude", name: "Claude Code", version: "2.1.301", kind: "cli", installed: true, enabled: true, auth: "authenticated", chat: true, detail: "Opus 5.5", message: null }]));
  H.mock("git_changes", () => ({ branch: "backspace/run", fresh: false, files: [
    { status: "M", path: "crates/tauri-app/ui/style.css", added: 12, removed: 3 },
    { status: "A", path: "crates/tauri-app/web/src/app/CodeHome.tsx", added: 180, removed: 0 },
  ] }));
  H.mock("git_activity", () => ({ days: [1, 0, 3, 2, 0, 4, 1, 0, 2, 1, 0, 3, 2, 1], added: 900, removed: 300 }));
  H.mock("worktrees", () => ({ root: WS, worktrees: [{ agent: "T-3", title: "Rename the settings card", branch: "backspace/t-3", path: WS + "/.backspace/worktrees/t-3", exists: true, size_bytes: 1 }] }));
  H.mock("list_files", () => []);
  H.mock("chat_list", () => ([
    { id: "t1", title: "Rename the settings card", updated: Date.now() - 6e5, pinned: false, route, preview: "Done", busy: false, unread: true, project: WS },
    { id: "t2", title: "Why is the loader off-centre?", updated: Date.now() - 9e6, pinned: false, route, preview: "The panel was a sibling", busy: false, unread: false, project: WS },
  ]));
  H.mock("agents_list", () => []);
  // A project's default tab: one agent canvas filling the window
  // (core prefs.rs TabSpec::defaults).
  H.mock("tabs_for", () => ({ tabs: [{ name: null, panes: [{ kind: "agent", agent: 0, url: null }], layout: null }], active: 0 }));
  H.mock("save_tabs", () => null);
  prefs.tabs = [{ name: "", panes: [], layout: null }];
  prefs.active_tab = 0;
  prefs.worker = "claude-code";
  prefs.cli_permission = "edits";
  await useTabsFor(WS);

  await refresh(true);
  setMode("code");
  setCodeView("pair");
  await settle(800);

  const switchLabels = [...document.querySelectorAll("#modes button, #modesSide button")].map((b) => b.textContent.trim()).filter(Boolean);
  const ade = {
    switchLabels: [...new Set(switchLabels)],
    homeHeading: document.querySelector("#chat h1")?.textContent.trim() ?? null,
    homeText: (document.querySelector("#chat")?.innerText || "").replace(/\s+/g, " ").trim().slice(0, 260),
    sessions: [...document.querySelectorAll("#chat button")].map((b) => b.textContent.trim()).filter((t) => /Rename the settings|loader off-centre/.test(t)).length,
  };

  setCodeView("agents");
  // The IDE's view menu: canvases is where the session lives. A saved
  // prefs.wb_view wins over the fresh-install default, so set it the way the
  // menu does rather than reaching into S.
  setWb("canvases");
  await settle(900);
  const show = document.querySelector("#chat") ? null : null;
  const card = document.querySelector(".banner");
  const hints = [...document.querySelectorAll(".tf .hint")].map((e) => e.textContent.replace(/\s+/g, " ").trim());
  const ide = {
    panes: document.querySelectorAll(".pane").length,
    grid: (document.querySelector("#grid")?.innerText || "").replace(/\s+/g, " ").trim().slice(0, 120),
    card: card ? card.innerText.replace(/\s+/g, " ").trim() : null,
    cardTitle: card?.querySelector(".bt > div:nth-child(2)")?.getAttribute("title") ?? null,
    hints,
    paneTitle: document.querySelector(".ph .t")?.textContent.trim() ?? null,
    promptPlaceholder: document.querySelector(".tp input")?.getAttribute("placeholder") ?? null,
    canvasHidden: document.querySelector("#canvas")?.hidden ?? null,
    wb: document.querySelector("#viewBtn")?.textContent.trim() ?? null,
    wbState: { s: S.wb, pref: prefs.wb_view ?? null, hasProject, gridClass: document.querySelector("#grid")?.className ?? null },
  };
  void show;

  return { ade, ide, windowTitle: document.title };
})()
