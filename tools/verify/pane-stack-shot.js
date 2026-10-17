// tools/verify/pane-stack-shot.js
//
// The fixture for a picture of a stacked tab: three canvases in one column, the
// middle one open, plus a fourth beside them to show that a stack is one region
// of a normal arrangement rather than a mode of its own.
(async () => {
  const H = window.__H;
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const WS = "C:\\Users\\chamy\\Documents\\code\\ProjectStack";
  const route = { kind: "cli", provider: "claude", model: null };
  const mainAgent = {
    id: 0, key: "0", parent: null, depth: 0, title: "Main agent", kind: "main", status: "running",
    branch: "backspace/stack", ticket: null, worktree: null, escalations: [], cost_usd: 1.24,
    input_tokens: 41000, output_tokens: 8200, advisor_calls: 0,
    decision: { model: "claude-opus-4-6", effort: "high", source: "preset", confidence: 0.9, router_cost_usd: 0, note: null },
    log: [], artifacts: [],
  };
  H.mock("snapshot", () => ({
    name: "ProjectStack", local: true, workspace: WS, agents: [mainAgent], approvals: [],
    tickets: [], artifacts: [], board: [], link: { state: "online", error: null },
    options: [], index: 0, total_cost_usd: 1.24, router_cost_usd: 0,
  }));
  H.mock("machines", () => ([{ index: 0, name: "This machine", local: true, url: "", selected: true, project: WS, link: { state: "online", error: null }, running: 0, pending: 0 }]));
  H.mock("harnesses", () => ([{ id: "claude", name: "Claude Code", version: "1", kind: "cli", installed: true, enabled: true, auth: "authenticated", chat: true, detail: "", message: null }]));
  H.mock("git_changes", () => ({ branch: "backspace/stack", fresh: true, files: [{ path: "crates/core/src/layout.rs", status: "M", added: 12, removed: 4 }] }));
  H.mock("git_activity", () => ({ days: [1, 2, 0, 3, 1, 0, 2], added: 120, removed: 30 }));
  H.mock("worktrees", () => ({ root: WS, worktrees: [] }));
  H.mock("list_files", () => []);
  H.mock("chat_list", () => []);
  H.mock("agents_list", () => []);
  H.mock("tabs_for", () => ({ tabs: [{ name: null, panes: [{ kind: "empty", agent: 0, url: null }], layout: null }], active: 0 }));
  H.mock("set_tabs_for", () => null);
  prefs.worker = "claude-code";
  await useTabsFor(WS);
  await Providers.load().catch(() => {});
  await refresh(true);

  setMode("code");
  setCodeView("agents");
  S.wb = "canvases";
  const t = tab();
  t.panes = [
    { kind: "files", agent: 0, url: null },
    { kind: "board", agent: 0, url: null },
    { kind: "empty", agent: 0, url: null },
    { kind: "files", agent: 0, url: null },
  ];
  t.layout = { dir: "row", ratio: 0.52, a: { stack: [0, 1, 2], active: 1 }, b: { leaf: 3 } };
  S.focus = 1;
  renderTabs(); renderGrid();
  await settle(900);
  return {
    folded: document.querySelectorAll(".pane.folded").length,
    open: document.querySelectorAll(".pane.stacked:not(.folded)").length,
    tree: JSON.stringify(tab().layout),
  };
})()
