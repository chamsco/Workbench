// tools/verify/sidebar-and-title.js
//
// Acceptance: Code's title is the project folder, and the sidebar carries the
// three T3-style actions (new thread, new project, and where the next thread
// runs). Also checks that the Workbench's Chat view lands on the agents home.
(async () => {
  const H = window.__H;
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const frames = (n) => new Promise((r) => { let left = n; const tick = () => (--left <= 0 ? r() : requestAnimationFrame(tick)); requestAnimationFrame(tick); });

  const WS = "C:\\Users\\chamy\\Documents\\code\\ProjectHalo";
  H.mock("snapshot", () => ({
    name: "local", local: true, workspace: WS, agents: [], approvals: [], tickets: [],
    link: { state: "online", error: null }, index: 0, total_cost_usd: 0, router_cost_usd: 0,
  }));
  H.mock("machines", () => ([
    { index: 0, name: "This machine", local: true, url: "", selected: true, project: WS, link: { state: "online", error: null }, running: 0, pending: 0 },
    { index: 1, name: "halo-box", local: false, url: "http://10.0.0.4:7420", selected: false, project: null, link: { state: "online", error: null }, running: 0, pending: 0 },
  ]));
  H.mock("colony_status", () => ({ ready: true, visor: "visor-core", workspaces: [{ name: "sandbox-1", provider: "wsl", os: "ubuntu", state: "running", ip: "172.20.0.2" }] }));
  H.mock("git_changes", () => ({ branch: "main", files: [], added: 0, removed: 0 }));
  H.mock("git_activity", () => ({ days: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], added: 0, removed: 0 }));
  H.mock("chat_list", () => []);
  H.mock("agents_list", () => []);

  // The mock's `prefs` is an empty object; the shell needs its own shape before
  // anything draws a tab.
  prefs.tabs = [{ name: "", panes: [], layout: null }];
  prefs.active_tab = 0;
  prefs.projects = [WS];

  await refresh(true);
  setMode("code");
  setCodeView("pair"); // Code → Chat, the view that is scoped to the project
  await settle(700);

  const out = {};
  const head = document.querySelector("#zoneHead");
  out.zoneHead = head ? head.textContent.trim() : null;
  out.windowTitle = document.title;

  // The sidebar the chat face portals into.
  const sideEl = document.querySelector("#chatList");
  const rows = [...(sideEl ? sideEl.querySelectorAll("button") : [])].map((b) => b.textContent.trim()).filter(Boolean);
  out.sidebarRows = rows.slice(0, 6);

  // Where the next thread runs. The row itself is the tree's root now (it
  // expands to the folder being run); the picker is the ⋯ beside it.
  const machineRow = sideEl ? sideEl.querySelector('button[aria-expanded]') : null;
  out.envTrigger = machineRow ? machineRow.textContent.trim() : null;
  const envMenuBtn = sideEl ? sideEl.querySelector('button[aria-label="Where the next thread runs"]') : null;
  if (envMenuBtn) {
    envMenuBtn.click();
    await frames(3);
    await settle(250);
    const pop = document.querySelector('[data-slot="dropdown-menu-content"]');
    out.envMenu = pop ? pop.textContent.replace(/\s+/g, " ").trim() : null;
    document.body.click();
    await settle(200);
  }

  // The Workbench's Chat view with no thread open.
  const chat = document.querySelector("#chat");
  out.workbenchChatHome = chat ? /Your agents/.test(chat.textContent) : false;
  out.composerPresent = !!(chat && chat.querySelector("textarea"));

  return out;
})()
