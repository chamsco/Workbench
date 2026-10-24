// tools/verify/harness-tiles.js — two harness tiles in one IDE tab.
// `claude` coordinates (right-click its header); `codex` has answered a
// hand-off. Checks the tiles were sent to the relay, and that typing
// `@codex …` in the claude tile goes to claude's thread for the core to route.
// Run with --eval-file tools/verify/harness-tiles.js --shot out.png
(async () => {
  const H = window.__H;
  const settle = ms => new Promise(r => setTimeout(r, ms));
  const WS = "/home/me/code/halo";
  const claude = { kind: "cli", provider: "claude", model: null };
  const codex = { kind: "cli", provider: "codex", model: null };
  const now = Date.now();
  const msg = (role, text, extra = {}) => ({ id: Math.random().toString(36).slice(2), role, text, at: now, status: "done", reactions: [], attachments: [], ...extra });
  const threads = {
    ta: { id: "ta", title: "Parser", route: claude, project: WS, messages: [
      msg("user", "Split the parser work: you take the lexer tests, codex reviews the grammar."),
      msg("assistant", "I'll write the lexer tests and send the grammar review to `codex`.\n\n- `lexer.rs`: 14 new cases\n- handed **grammar.rs** to @codex"),
      msg("user", "Two issues in grammar.rs: the `else if` chain recurses without a depth limit, and `parse_block` drops trailing comments.", { author: "tile:tb", author_name: "codex" }),
      msg("assistant", "", { status: "streaming" }),
    ] },
    tb: { id: "tb", title: "Review", route: codex, project: WS, messages: [
      msg("user", "Review grammar.rs for unbounded recursion and lost tokens.", { author: "tile:ta", author_name: "claude" }),
      msg("assistant", "Two issues in grammar.rs: the `else if` chain recurses without a depth limit, and `parse_block` drops trailing comments."),
    ] },
  };
  H.mock("snapshot", () => ({ name: "halo", local: true, workspace: WS, agents: [{ id: 0, key: "0", parent: null, depth: 0, title: "Main agent", kind: "main", status: "idle", log: [], artifacts: [], escalations: [], cost_usd: 0, input_tokens: 0, output_tokens: 0 }], approvals: [], tickets: [], artifacts: [], options: [], board: [], link: { state: "online", error: null }, index: 0, total_cost_usd: 0, router_cost_usd: 0 }));
  H.mock("machines", () => ([{ index: 0, name: "This machine", local: true, url: "", selected: true, project: WS, link: { state: "online", error: null }, running: 0, pending: 0 }]));
  H.mock("harnesses", () => ([
    { id: "claude", name: "Claude Code", version: "2.1", kind: "cli", installed: true, enabled: true, auth: "authenticated", chat: true, detail: "Opus", message: null },
    { id: "codex", name: "Codex", version: "0.9", kind: "cli", installed: true, enabled: true, auth: "authenticated", chat: true, detail: "gpt-5", message: null },
  ]));
  H.mock("chat_thread", a => threads[a.id] || null);
  H.mock("chat_list", () => []);
  H.mock("relay_waiting", a => a.threads.map(t => (t === "tb" ? 1 : 0)));
  let relayed = null;
  H.mock("relay_tiles", a => (relayed = a.tiles));
  H.mock("tabs_for", () => ({ tabs: [{ name: "Parser", panes: [
    { kind: "harness", agent: 0, url: null, thread: "ta", label: "claude", coordinator: true },
    { kind: "harness", agent: 0, url: null, thread: "tb", label: "codex" },
  ], layout: null }], active: 0 }));
  await Code.loadTabs(WS);
  await Providers.load().catch(() => {});
  await refresh(true);
  setMode("code");
  setCodeView("agents");
  Code.setWb("canvases");
  await refresh(true);
  await settle(1500);
  const panes = [...document.querySelectorAll(".pane")];
  const ta = panes[0] && panes[0].querySelector("textarea");
  let sent = null;
  H.mock("chat_send", a => { sent = a; return null; });
  if (ta) { ta.value = "@codex check the fix"; ta.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })); }
  await settle(300);
  // Right-click the second header: the menu offers to make it coordinator.
  panes[1].querySelector(".ph").dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, clientX: 900, clientY: 120 }));
  await settle(200);
  const menu = [...document.querySelectorAll("#pop .pi")].map(b => b.textContent.trim());
  document.querySelector("#pop").hidden = true;
  return {
    titles: panes.map(p => p.querySelector(".ph .t").textContent),
    badge: panes.map(p => !!p.querySelector(".hz-badge")),
    chips: panes.map(p => [...p.querySelectorAll(".hz-chip")].map(c => c.textContent)),
    state: panes.map(p => p.querySelector(".hz-state").textContent),
    relayed, sent, menu,
  };
})()
