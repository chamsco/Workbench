// tools/verify/pane-stack.js
//
// Acceptance: a stack of canvases is a column of cards — one open, the rest
// folded to their header — and it is reachable from the shell, not just from
// the layout model:
//   1. dragging a canvas by its header onto the middle of another stacks them
//   2. the folded card shows only its header; clicking it opens that card
//   3. the open card's unstack button gives it back a rectangle of its own
//   4. a stack saved in prefs draws instead of throwing (the JS mirror used to
//      know only leaf and split, so a stack from the GPUI shell took the tab
//      down with it)
//   5. a tab holds more than four canvases now that a stack can carry them
(async () => {
  const H = window.__H;
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const WS = "C:\\Users\\chamy\\Documents\\code\\ProjectStack";
  const route = { kind: "cli", provider: "claude", model: null };
  const mainAgent = {
    id: 0, key: "0", parent: null, depth: 0, title: "Main agent", kind: "main", status: "idle",
    branch: "backspace/run", ticket: null, worktree: null, escalations: [], cost_usd: 0.1,
    input_tokens: 100, output_tokens: 20, advisor_calls: 0,
    decision: { model: "claude-opus-4-6", effort: "high", source: "preset", confidence: 0.9, router_cost_usd: 0, note: null },
    log: [], artifacts: [],
  };
  H.mock("snapshot", () => ({
    name: "local", local: true, workspace: WS, agents: [mainAgent], approvals: [],
    tickets: [], artifacts: [], options: [], board: [], link: { state: "online", error: null },
    index: 0, total_cost_usd: 0.1, router_cost_usd: 0,
  }));
  H.mock("machines", () => ([{ index: 0, name: "This machine", local: true, url: "", selected: true, project: WS, link: { state: "online", error: null }, running: 0, pending: 0 }]));
  H.mock("harnesses", () => ([{ id: "claude", name: "Claude Code", version: "1", kind: "cli", installed: true, enabled: true, auth: "authenticated", chat: true, detail: "", message: null }]));
  H.mock("git_changes", () => ({ branch: "backspace/run", fresh: false, files: [] }));
  H.mock("git_activity", () => ({ days: [], added: 0, removed: 0 }));
  H.mock("worktrees", () => ({ root: WS, worktrees: [] }));
  H.mock("list_files", () => []);
  H.mock("chat_list", () => []);
  H.mock("agents_list", () => []);
  H.mock("tabs_for", () => ({ tabs: [{ name: null, panes: [{ kind: "empty", agent: 0, url: null }], layout: null }], active: 0 }));
  H.mock("set_tabs_for", () => null);
  H.mock("set_tabs", () => null);
  prefs.worker = "claude-code";
  prefs.cli_permission = "edits";
  await useTabsFor(WS);
  await Providers.load().catch(() => {});
  await refresh(true);

  const panes = (n) => Array.from({ length: n }, () => ({ kind: "empty", agent: 0, url: null }));
  const box = (slot) => {
    const el = document.querySelector(`.pane[data-slot="${slot}"]`);
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return {
      top: Math.round(r.top), h: Math.round(r.height), w: Math.round(r.width),
      folded: el.classList.contains("folded"),
      stacked: el.classList.contains("stacked"),
      bodyHidden: !!el.lastElementChild.hidden,
      title: (el.querySelector(".ph .t") || {}).textContent || "",
    };
  };
  const read = () => ({
    grid: Math.round(document.querySelector("#grid").getBoundingClientRect().height),
    panes: [0, 1, 2].map(box).filter(Boolean),
  });
  const ptr = (type, x, y) => new PointerEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, pointerId: 1, button: 0, buttons: type === "pointerup" ? 0 : 1 });

  setMode("code");
  setCodeView("agents");
  S.wb = "canvases";
  const t = tab();
  t.panes = panes(3);
  t.layout = L.defaultFor(3);
  renderTabs(); renderGrid();
  await settle(500);
  const out = { three: read() };

  // 1. Drag canvas 1 onto the middle of canvas 0. The middle shares the spot.
  const p0 = document.querySelector('.pane[data-slot="0"]').getBoundingClientRect();
  const p1 = document.querySelector('.pane[data-slot="1"]').getBoundingClientRect();
  const head = document.querySelector('.pane[data-slot="1"] .ph');
  head.dispatchEvent(ptr("pointerdown", p1.left + 40, p1.top + 8));
  window.dispatchEvent(ptr("pointermove", p1.left + 80, p1.top + 40));
  await settle(60);
  window.dispatchEvent(ptr("pointermove", p0.left + p0.width / 2, p0.top + p0.height / 2));
  await settle(60);
  out.duringDrag = {
    preview: document.querySelectorAll(".drop-ph.card").length,
    phHeight: Math.round(document.querySelector(".drop-ph").getBoundingClientRect().height),
  };
  window.dispatchEvent(ptr("pointerup", p0.left + p0.width / 2, p0.top + p0.height / 2));
  await settle(400);
  out.afterDrop = read();
  out.tree = JSON.stringify(tab().layout);
  out.stacked = !!L.stackOf(tab().layout, 0);

  // 2. Click the folded header: that card opens and the other folds.
  const folded = out.afterDrop.panes.find((p) => p.folded);
  const foldedSlot = out.afterDrop.panes.findIndex((p) => p.folded);
  if (folded) document.querySelector(`.pane[data-slot="${foldedSlot}"] .ph`).click();
  await settle(400);
  out.afterClick = read();
  out.openSlotChanged = foldedSlot >= 0 && !out.afterClick.panes[foldedSlot].folded;

  // 3. The unstack button on the open card gives it its own rectangle.
  const openSlot = out.afterClick.panes.findIndex((p) => !p.folded);
  const popBtn = document.querySelector(`.pane[data-slot="${openSlot}"] [data-act="pop"]`);
  out.popPresent = !!popBtn;
  if (popBtn) popBtn.click();
  await settle(400);
  out.afterPop = read();
  out.unstacked = !L.stackOf(tab().layout, 0);

  // 4. A stack straight from prefs draws, and folds the cards it is not on.
  t.panes = panes(3);
  t.layout = { stack: [0, 1, 2], active: 1 };
  renderGrid();
  await settle(400);
  out.fromPrefs = read();
  out.prefsFolded = out.fromPrefs.panes.filter((p) => p.folded).length;

  // 5. More than four canvases in one tab.
  t.panes = panes(3);
  t.layout = null;
  renderGrid();
  S.focus = 2;
  for (let i = 0; i < 5; i++) addCanvas({ kind: "empty", agent: 0, url: null });
  await settle(300);
  out.canvasesInTab = tab().panes.length;
  out.maxPanes = MAX_PANES;

  // The model's own geometry, against core::layout's numbers.
  const two = L.moved(L.defaultFor(2), 1, 0, "stack", 1);
  const { leaves } = L.layout(two, { x: 0, y: 0, w: 1000, h: 600 }, 0);
  const rr = (i) => leaves.find((l) => l.leaf === i).r;
  out.geometry = { head: rr(0).h, openTop: rr(1).y, openH: rr(1).h, headConst: STACK_HEAD };
  return out;
})()
