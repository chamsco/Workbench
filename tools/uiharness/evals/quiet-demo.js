// A settled Direction 04 frame with a real-looking project, bots, tickets and
// routines. Mirrors the shapes `tools/verify/shot.js` uses; adds the Agents
// (bots/groups) the Crew page draws. Not a check — a screenshot fixture.
(async () => {
  const H = window.__H;
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const WS = "C:\\Users\\chamy\\Documents\\code\\Friday demo";
  const route = { kind: "cli", provider: "claude", model: null };
  const now = Date.now();
  const agent = (id, title, status, extra = {}) => ({
    id, key: String(id), parent: null, depth: 0, title, kind: id === 0 ? "main" : "worker", status,
    branch: "main", ticket: null, worktree: null, escalations: [], cost_usd: id === 0 ? 0.62 : 0.2,
    input_tokens: 12000 + id * 2400, output_tokens: 3400 + id * 700, advisor_calls: 0,
    decision: { model: "claude-opus-4-6", effort: "high", source: "preset", confidence: 0.9, router_cost_usd: 0, note: null },
    log: [], artifacts: [], ...extra,
  });
  H.mock("snapshot", () => ({
    name: "Friday demo", local: true, workspace: WS,
    agents: [
      agent(0, "Main agent", "running", { log: [
        { kind: "user", text: "Ship the receipt flow for Friday. Keep the money tests strict." },
        { kind: "assistant", text: "I split the job on the shared board. Codex has the receipt total and the boundary tests; Cursor is checking the interface copy." },
        { kind: "tool_call", text: "Read crates/tauri-app/ui/app.js" },
        { kind: "tool_call", text: "Edit crates/tauri-app/ui/quiet.js  +42 −6" },
        { kind: "tool_result", text: "2 files changed, 42 insertions(+), 6 deletions(-)" },
        { kind: "assistant", text: "The receipt total is green. Cursor still has one label to fix; I’ll read the diff once the preview settles." },
      ] }),
      agent(1, "Codex", "running", { ticket: "T-2", log: [
        { kind: "assistant", text: "I have the receipt total and the money tests. The boundary cases are the next thing I’m proving." },
        { kind: "tool_call", text: "Run npm test -- money" },
        { kind: "tool_result", text: "5 tests passed, 0 failed" },
        { kind: "assistant", text: "Done. Both pass at 0 and at a negative total; the diff is on the board." },
      ] }),
      agent(2, "Cursor", "awaiting_approval", { ticket: "T-3", log: [
        { kind: "assistant", text: "The preview is up. I’m checking the receipt copy and the total layout at the small size." },
        { kind: "tool_call", text: "Open http://localhost:5173/receipt" },
        { kind: "assistant", text: "One label still says “subtotal”. I’m fixing the copy, then I’ll read Codex’s diff." },
      ] }),
    ],
    approvals: [
      { id: 1, kind: "plan", agent: 0, deliverable: {}, tickets: ["T-1", "T-2", "T-3"], state: { state: "pending" } },
      { id: 2, kind: "deliverable", agent: 2, deliverable: {}, tickets: ["T-3"], state: { state: "approved" } },
    ],
    tickets: [
      { num: 1, key: "T-1", title: "Split the receipt total out of the view", category: "feature", state: "done", what_to_build: "", acceptance: [], check: "npm test -- receipt", out_of_scope: [], blocked_by: [], owner: 0, assignee: 1, budget_usd: null, notes: [] },
      { num: 2, key: "T-2", title: "Strict rounding at 0 and negative totals", category: "test", state: "in_progress", what_to_build: "", acceptance: [], check: "npm test -- money", out_of_scope: [], blocked_by: [], owner: 0, assignee: 1, budget_usd: null, notes: [] },
      { num: 3, key: "T-3", title: "Quieter receipt copy", category: "feature", state: "ready_for_human", what_to_build: "", acceptance: [], check: "npm test -- copy", out_of_scope: [], blocked_by: [], owner: 0, assignee: 2, budget_usd: null, notes: [] },
      { num: 4, key: "T-4", title: "Preview harness", category: "test", state: "queued", what_to_build: "", acceptance: [], check: null, out_of_scope: [], blocked_by: [], owner: 0, assignee: 2, budget_usd: null, notes: [] },
    ],
    total_cost_usd: 1.23, router_cost_usd: 0.04, config_source: null, board: [], options: [], artifacts: [],
    link: { state: "online", error: null }, index: 0,
  }));
  H.mock("machines", () => ([
    { index: 0, name: "This Mac", local: true, url: "", selected: true, project: WS, os: "windows", kind: "local", detail: "This machine · primary bench", paths: [WS], tailscale: true, link: { state: "online", error: null }, running: 2, pending: 1 },
    { index: 1, name: "Studio Mac", local: false, url: "ssh://studio", selected: false, project: "C:\\Users\\chamy\\Documents\\code\\Studio", os: "macos", kind: "ssh", detail: "Apple M3 Ultra · 128 GB", paths: ["C:\\Users\\chamy\\Documents\\code\\Studio", "C:\\Users\\chamy\\Documents\\code\\Atlas"], tailscale: true, link: { state: "online", error: null }, running: 1, pending: 0 },
    { index: 2, name: "Build Mini", local: false, url: "cloud://build-mini", selected: false, project: "C:\\Users\\chamy\\Documents\\code\\Pixel kit", os: "macos", kind: "cloud", detail: "Offline since 08:12 · last seen overnight", paths: ["C:\\Users\\chamy\\Documents\\code\\Pixel kit"], tailscale: false, link: { state: "offline", error: "not seen for 9h" }, running: 0, pending: 0 },
    { index: 3, name: "Beast Proxmox", local: false, url: "ssh://beast", selected: false, project: "C:\\Users\\chamy\\Documents\\code\\Atlas", os: "linux", kind: "ssh", detail: "Proxmox host · VM and container admin", paths: ["/srv/atlas"], tailscale: true, link: { state: "online", error: null }, running: 3, pending: 2 },
    { index: 4, name: "Hetzner VPS", local: false, url: "ssh://hetzner", selected: false, project: "C:\\Users\\chamy\\Documents\\code\\Friday demo", os: "linux", kind: "ssh", detail: "2 vCPU · 8 GB · Frankfurt", paths: ["/srv/friday"], tailscale: true, link: { state: "online", error: null }, running: 1, pending: 0 },
  ]));
  H.mock("harnesses", () => ([
    { id: "claude", name: "Claude Code", kind: "cli", installed: true, enabled: true, auth: "authenticated", version: "2.1.299", latest: "2.1.299", hosts: { "Studio Mac": "2.1.287", "Beast Proxmox": "2.1.287", "Hetzner VPS": "2.1.299" } },
    { id: "codex", name: "Codex", kind: "cli", installed: true, enabled: true, auth: "authenticated", version: "0.162.1", latest: "0.162.1", hosts: { "Studio Mac": "0.160.0", "Beast Proxmox": "0.159.3", "Hetzner VPS": "0.162.1" } },
    { id: "cursor", name: "Cursor", kind: "cli", installed: true, enabled: true, auth: "authenticated", version: "2026.10.14-0f9edde", latest: "2026.10.14-0f9edde", hosts: { "Studio Mac": "2026.10.01-e373342", "Beast Proxmox": "2026.10.01-e373342", "Hetzner VPS": "2026.10.01-14929f9" } },
    { id: "grok", name: "Grok CLI", kind: "cli", installed: true, enabled: true, auth: "authenticated", version: "0.64.0", latest: "0.64.0", hosts: { "Studio Mac": "0.62.0", "Hetzner VPS": "0.64.0" } },
    { id: "opencode", name: "OpenCode", kind: "cli", installed: true, enabled: true, auth: "authenticated", version: "0.87.0", latest: "0.87.0", hosts: { "Studio Mac": "0.87.0", "Beast Proxmox": "0.87.0", "Hetzner VPS": "0.87.0" } },
  ]));
  H.mock("chat_retry", () => true);
  H.mock("chat_purge_ghosts", () => []);
  H.mock("agent_save", (args) => {
    const a = args.agent;
    const i = demoAgents.findIndex((x) => x.id === a.id);
    if (i >= 0) demoAgents[i] = { ...demoAgents[i], ...a };
    else demoAgents.push(a);
    return demoAgents.find((x) => x.id === a.id) || a;
  });
  H.mock("agent_projects", () => ([{ id: "friday", name: "Friday demo", folder: WS, agents: ["claude", "codex"] }]));
  H.mock("chat_list", () => ([
    { id: "t1", title: "Launch crew", updated: now - 6e5, pinned: false, route, preview: "Preview harness replied", busy: false, unread: true, project: WS, members: ["claude", "codex", "cursor"], goal: "Preview the receipt flow" },
    { id: "t2", title: "Design room", updated: now - 2.4e6, pinned: false, route, preview: "Cursor: copy is ready", busy: false, unread: false, project: WS, members: ["claude", "cursor"], goal: "Quieter receipt copy" },
    { id: "t3", title: "Friday demo", updated: now - 3.6e6, pinned: false, route, preview: "Total is in and the tests are green.", busy: true, unread: false, project: WS, agent: "codex" },
    { id: "t4", title: "Q4 launch checklist", updated: now - 5.2e6, pinned: false, route: { kind: "cloud", provider: "cloud", model: null }, preview: "Checklist is ready for review.", busy: false, unread: false, project: WS, agent: "claude" },
    { id: "t5", title: "Receipt edge cases", updated: now - 6.8e6, pinned: false, route, preview: "Boundary tests are green at zero and negative totals.", busy: false, unread: false, project: WS, agent: "codex" },
    { id: "t6", title: "Release notes", updated: now - 26 * 36e5, pinned: false, route, preview: "Drafted the Friday release notes.", busy: false, unread: false, project: WS, agent: "claude" },
    { id: "t7", title: "Old dashboard cleanup", updated: now - 30 * 36e5, pinned: false, route: { kind: "cli", provider: "cursor", model: null }, preview: "Removed the stale widgets.", busy: false, unread: false, project: "C:\\Users\\chamy\\Documents\\code\\Atlas", agent: "cursor" },
    { id: "t8", title: "Flaky upload test", updated: now - 2e5, pinned: false, route, preview: "The upload test timed out, retrying.", busy: false, unread: false, project: WS, agent: "codex", failed: "usage limit on the model" },
  ]));
  H.mock("git_changes", () => ({ branch: "main", files: [
    { status: "M", path: "crates/tauri-app/ui/quiet.css", added: 159, removed: 0 },
    { status: "M", path: "crates/tauri-app/ui/quiet.js", added: 42, removed: 3 },
    { status: "M", path: "crates/tauri-app/ui/index.html", added: 3, removed: 0 },
  ] }));
  const demoAgents = [
    {
      id: "claude", name: "Claude Code", job: "Lead · plans and checks",
      avatar: { emoji: "🧠", color: "#8b5cf6" }, personality: "Direct and brief. Leads with the answer, splits work before starting, and checks the diff before calling it done.",
      route: { kind: "cli", provider: "claude", model: "claude-opus-4-6" },
      fallbacks: [{ kind: "cli", provider: "codex", model: "gpt-5.1" }],
      shared: [], computer: { kind: "local", host: null, desktop: null }, memory: true, off: [], apps_off: [], hook: "",
      routines: [{ id: "r1", name: "Friday review", instructions: "Check the diff", kind: "weekdays", time: "09:00", days: [], every_minutes: 0, project: "", enabled: true, last_run: now - 864e5, next_run: now + 15e5, runs: 3, utc_offset_minutes: 0, created: now, updated: now }],
      created: now, updated: now,
    },
    {
      id: "codex", name: "Codex", job: "Engineer · builds and tests",
      avatar: { emoji: "🐱", color: "#14b8a6" }, personality: "Pragmatic. Writes the test first, keeps the diff small, and says plainly when something is still broken.",
      route: { kind: "cli", provider: "codex", model: "gpt-5.1-codex" },
      shared: [], computer: { kind: "local", host: null, desktop: null }, memory: true, off: [], apps_off: [], hook: "",
      routines: [], created: now, updated: now,
    },
    {
      id: "cursor", name: "Cursor", job: "Designer · interface and copy",
      avatar: { emoji: "⭐", color: "#ef4444" }, personality: "Sweats the details. Notices the rough edge in the copy, fixes it, and cares how the result reads.",
      route: { kind: "cli", provider: "cursor", model: "cursor" },
      shared: [], computer: { kind: "local", host: null, desktop: null }, memory: true, off: [], apps_off: [], hook: "",
      routines: [], created: now, updated: now,
    },
    {
      id: "medic", name: "Medic", job: "Doctor · repairs stuck chats",
      avatar: { emoji: "🩺", color: "#14b8a6" }, personality: "Calm and methodical. Finds the chat that stopped, retries it on a fallback, and reports the one thing that was wrong.",
      route: { kind: "cli", provider: "claude", model: "claude-haiku-4-5" },
      shared: [], computer: { kind: "local", host: null, desktop: null }, memory: true, off: [], apps_off: [], hook: "",
      routines: [], created: now, updated: now,
    },
  ];
  H.mock("agents_list", () => demoAgents);
  H.mock("routines_sync", () => [
    { id: "claude", name: "Claude Code", job: "Lead · plans and checks", avatar: { emoji: "🧠", color: "#8b5cf6" }, personality: "", route, shared: [], computer: { kind: "local", host: null, desktop: null }, memory: true, off: [], apps_off: [], hook: "", routines: [{ id: "r1", name: "Friday review", instructions: "Check the diff", kind: "weekdays", time: "09:00", days: [], every_minutes: 0, project: "", enabled: true, last_run: now - 864e5, next_run: now + 15e5, runs: 3, utc_offset_minutes: 0, created: now, updated: now }], created: now, updated: now },
    { id: "codex", name: "Codex", job: "Engineer · builds and tests", avatar: { emoji: "🐱", color: "#14b8a6" }, personality: "", route, shared: [], computer: { kind: "local", host: null, desktop: null }, memory: true, off: [], apps_off: [], hook: "", routines: [], created: now, updated: now },
    { id: "cursor", name: "Cursor", job: "Designer · interface and copy", avatar: { emoji: "⭐", color: "#ef4444" }, personality: "", route, shared: [], computer: { kind: "local", host: null, desktop: null }, memory: true, off: [], apps_off: [], hook: "", routines: [], created: now, updated: now },
  ]);
  prefs.projects = [WS, "C:\\Users\\chamy\\Documents\\code\\Atlas", "C:\\Users\\chamy\\Documents\\code\\Pixel kit"];
  try {
    localStorage.setItem("bs.q_agent_extra", JSON.stringify({
      claude: { fallbacks: [{ provider: "codex", model: "gpt-5.1" }], effort: "high", fast: false, rotation: true, kind: "agent" },
      medic: { kind: "doctor" },
    }));
  } catch {}
  await refresh(true);
  await Quiet.load(true);
  Quiet.go("home");
  await settle(500);
  return "quiet-demo";
})()
