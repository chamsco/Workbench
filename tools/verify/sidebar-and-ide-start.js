// tools/verify/sidebar-and-ide-start.js
//
// The two things the owner asked for after seeing it running:
//   1. one sidebar in Code, whichever surface you are in: the same project tree,
//      the same rows, in the same place
//   2. the IDE starts on a canvas that asks what to open — never on the home
//      ("a chatbox"), even on a machine whose prefs still carry the old
//      `wb_view: "home"` from the build that landed there.
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
  ]));
  H.mock("agents_list", () => []);
  // Exactly what `TabSpec::defaults()` hands a project seen for the first time:
  // one empty canvas, whose whole face is the chooser.
  H.mock("tabs_for", () => ({ tabs: [{ name: null, panes: [{ kind: "empty", agent: 0, url: null }], layout: null }], active: 0 }));
  H.mock("save_tabs", () => null);
  prefs.worker = "claude-code";
  prefs.cli_permission = "edits";
  prefs.code_view = "agents";
  // The pref a machine that ran the previous build still has. It must not decide
  // what the IDE opens on.
  prefs.wb_view = "home";
  await useTabsFor(WS);
  await Providers.load().catch(() => {});
  await refresh(true);

  const strip = (s) => (s || "").replace(/\s+/g, " ").trim();
  const el = (sel) => document.querySelector(sel);
  const shown = (sel) => { const e = el(sel); return e ? getComputedStyle(e).display !== "none" && !e.hidden : null; };
  const side = () => {
    const e = el("#chatList");
    return {
      there: !!e,
      visible: shown("#chatList"),
      acts: [...(e?.querySelectorAll("button[aria-label]") ?? [])].map((b) => b.getAttribute("aria-label")).slice(0, 3),
      rows: [...(e?.querySelectorAll("button") ?? [])].map((b) => strip(b.textContent)).filter(Boolean),
      // The vanilla Code list, which used to be one surface's sidebar only.
      vanilla: shown("#list"),
    };
  };

  // What the app does with a stored view on boot (app.js, the boot line).
  S.wb = WB.some(([k]) => k === prefs.wb_view) ? prefs.wb_view : "canvases";

  setMode("code");
  setCodeView("agents");
  setSide(true);
  await settle(900);
  const ide = {
    views: WB.map(([k]) => k),
    viewChip: strip(el("#viewBtn")?.textContent),
    menu: (() => { el("#viewBtn").click(); const t = strip(el("#pop")?.textContent); closePop(); return t; })(),
    chooser: !!el("#grid .chooser"),
    chooserHeading: strip(el("#grid .chooser h5")?.textContent),
    homeInGrid: !!el("#grid .home"),
    heroInGrid: strip(el("#grid h1")?.textContent) || null,
    side: side(),
  };

  setCodeView("pair");
  await settle(900);
  const ade = {
    hero: strip(el("#chat h1")?.textContent),
    wb: S.wb,
    side: side(),
  };

  return {
    ide,
    ade,
    sameSidebar: JSON.stringify(ide.side) === JSON.stringify(ade.side),
    windowTitle: document.title,
  };
})()
