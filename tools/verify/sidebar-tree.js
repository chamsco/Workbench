// tools/verify/sidebar-tree.js — Code's sidebar: an icon row, then the project
// tree (the folder, the threads in it, the canvases it is showing), in both of
// Code's surfaces. Also checks the ADE lands on the zone's own home, without a
// second ask line.
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
  H.mock("git_changes", () => ({ branch: "backspace/run", fresh: false, files: [] }));
  H.mock("git_activity", () => ({ days: [0, 1, 0, 2, 1, 0, 0, 1, 0, 0, 2, 0, 1, 0], added: 40, removed: 9 }));
  H.mock("worktrees", () => ({ root: WS, worktrees: [] }));
  H.mock("list_files", () => []);
  H.mock("chat_list", () => ([
    { id: "t1", title: "Rename the settings card", updated: Date.now() - 6e5, pinned: false, route, preview: "Done", busy: false, unread: true, project: WS },
    { id: "t2", title: "Why is the loader off-centre?", updated: Date.now() - 9e6, pinned: false, route, preview: "The panel was a sibling", busy: false, unread: false, project: WS },
    { id: "t3", title: "Ship the ADE and IDE split", updated: Date.now() - 3e7, pinned: false, route, preview: "Named for what they do", busy: false, unread: false, project: "C:\\Users\\chamy\\Documents\\code\\Workbench" },
  ]));
  H.mock("agents_list", () => []);
  // The open project's tab: one session plus a terminal and a browser.
  H.mock("tabs_for", () => ({
    tabs: [{ name: null, panes: [
      { kind: "agent", agent: 0, url: null },
      { kind: "terminal", agent: 0, url: null },
      { kind: "browser", agent: 0, url: "http://localhost:5173" },
    ], layout: null }],
    active: 0,
  }));
  H.mock("save_tabs", () => null);
  prefs.worker = "claude-code";
  prefs.cli_permission = "edits";
  await useTabsFor(WS);
  await Providers.load().catch(() => {});
  await refresh(true);

  const strip = (s) => (s || "").replace(/\s+/g, " ").trim();
  const readSide = () => {
    const el = document.querySelector("#chatList");
    const rows = [...(el ? el.querySelectorAll("button") : [])].map((b) => strip(b.textContent)).filter(Boolean);
    const acts = [...(el ? el.querySelectorAll("button[aria-label]") : [])].map((b) => b.getAttribute("aria-label"));
    return {
      visible: el ? getComputedStyle(el).display !== "none" && !el.hidden : false,
      rows: rows.slice(0, 12),
      actLabels: acts.slice(0, 6),
      hasLabelText: /New thread|New project|Search/.test(strip(el ? el.textContent : "")),
    };
  };

  // IDE, first: the sidebar is the tree even where the vanilla list used to be.
  setMode("code");
  setCodeView("agents");
  setWb("canvases");
  await settle(900);
  const ide = {
    side: readSide(),
    listHidden: getComputedStyle(document.querySelector("#list")).display === "none",
    panes: document.querySelectorAll(".pane").length,
  };

  // ADE: the same tree, and the zone's own home behind its composer.
  setCodeView("pair");
  await settle(900);
  const chat = document.querySelector("#chat");
  const ade = {
    side: readSide(),
    hero: strip(chat?.querySelector("h1")?.textContent),
    pills: [...(chat?.querySelectorAll(".hpill") ?? [])].map((b) => strip(b.textContent)),
    inbox: strip(chat?.querySelector(".home-open")?.textContent).slice(0, 90),
    // The home is drawn without its own ask line: the chat face's composer sits
    // under it, and `.hc` alone is ambiguous — the code composer uses it too.
    homeAskLines: chat ? chat.querySelectorAll(".home .hc textarea").length : -1,
    homeChips: chat ? chat.querySelectorAll(".home .hc-above").length : -1,
    ownComposer: chat ? chat.querySelectorAll(".code-composer textarea").length : -1,
  };

  return { ide, ade, windowTitle: document.title };
})()
