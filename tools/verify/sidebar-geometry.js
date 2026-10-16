// tools/verify/sidebar-geometry.js — what the sidebar is actually laid out as.
(async () => {
  const H = window.__H;
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const WS = "C:\\Users\\chamy\\Documents\\code\\ProjectHalo";
  H.mock("snapshot", () => ({ name: "local", local: true, workspace: WS, agents: [], approvals: [], tickets: [], link: { state: "online", error: null }, index: 0, total_cost_usd: 0, router_cost_usd: 0 }));
  H.mock("machines", () => ([{ index: 0, name: "This machine", local: true, url: "", selected: true, project: WS, link: { state: "online", error: null }, running: 0, pending: 0 }]));
  H.mock("git_changes", () => ({ branch: "main", files: [], added: 0, removed: 0 }));
  H.mock("git_activity", () => ({ days: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], added: 0, removed: 0 }));
  H.mock("chat_list", () => []);
  H.mock("agents_list", () => []);
  prefs.tabs = [{ name: "", panes: [], layout: null }];
  prefs.active_tab = 0;

  await refresh(true);
  setMode("code");
  setCodeView("pair");
  await settle(700);

  const win = document.querySelector("#win");
  const side = document.querySelector("#side");
  const desk = document.querySelector(".desk");
  const box = (el) => { const r = el.getBoundingClientRect(); return { left: Math.round(r.left), width: Math.round(r.width) }; };
  const cs = getComputedStyle(side);
  return {
    win: { class: win.className, box: box(win), grid: getComputedStyle(win).gridTemplateColumns },
    desk: { box: box(desk), padding: getComputedStyle(desk).padding },
    side: { box: box(side), visibility: cs.visibility, overflow: cs.overflow, opacity: cs.opacity, transform: cs.transform, background: cs.backgroundColor },
    sideText: side.textContent.replace(/\s+/g, " ").trim().slice(0, 120),
    chatBox: box(document.querySelector("#chatList")),
    agentsHome: !!document.querySelector(".whirl-side"),
  };
})()
