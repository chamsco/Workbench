// tools/verify/plan-mode.js — Code chat's Plan | Fast switch and the spec gate.
//   A thread whose last message went in Plan opens in Plan; the reply that
//   wrote SPEC.md shows the gate's verdict (refused, with each gap); Fast
//   switches the placeholder back; a send carries plan=false.
(async () => {
  const H = window.__H;
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const WS = "/home/charles/code/Halo";
  const route = { kind: "cli", provider: "claude", model: null };
  const now = Date.now();
  const spec = "Here is the spec.\n\n```spec\n# Dark mode\n\n## Problem\n\nToo bright at night.\n```";
  const thread = {
    id: "t1", title: "Add dark mode", created: now - 9e5, updated: now - 6e5, pinned: false, route, project: WS,
    messages: [
      { id: "u1", role: "user", text: "Add dark mode", at: now - 9e5, status: "done", reactions: [], attachments: [], plan: true },
      { id: "a1", role: "assistant", text: "❓ **Q1** - **Which shade?**: true black or grey?\n\n➡️ Grey.", at: now - 8e5, status: "done", reactions: [], attachments: [] },
      { id: "u2", role: "user", text: "Grey, and follow the OS", at: now - 7e5, status: "done", reactions: [], attachments: [], plan: true },
      { id: "a2", role: "assistant", text: spec, at: now - 6e5, status: "done", reactions: [], attachments: [],
        spec: { passed: false, at: now - 6e5, gaps: [
          "Open questions: 1 still open; answer them or move them to Out of scope",
          "Acceptance 2 has no command (`…`) or click to check it: It looks nice.",
        ] } },
    ],
  };
  H.mock("snapshot", () => ({ name: "local", local: true, workspace: WS, agents: [], approvals: [], tickets: [], artifacts: [], options: [], board: [], link: { state: "online", error: null }, index: 0, total_cost_usd: 0, router_cost_usd: 0 }));
  H.mock("machines", () => ([{ index: 0, name: "This machine", local: true, url: "", selected: true, project: WS, link: { state: "online", error: null }, running: 0, pending: 0 }]));
  H.mock("harnesses", () => ([{ id: "claude", name: "Claude Code", version: "2.1.301", kind: "cli", installed: true, enabled: true, auth: "authenticated", chat: true, detail: "Opus", message: null }]));
  H.mock("git_changes", () => ({ branch: "main", fresh: false, files: [] }));
  H.mock("git_activity", () => ({ days: [1, 0, 2, 3, 0, 1, 2], added: 120, removed: 40 }));
  H.mock("chat_list", () => ([{ id: "t1", title: thread.title, updated: thread.updated, pinned: false, route, preview: "Here is the spec.", busy: false, unread: false, project: WS }]));
  H.mock("chat_thread", () => thread);
  H.mock("agents_list", () => []);
  H.mock("tabs_for", () => ({ tabs: [{ name: null, panes: [{ kind: "agent", agent: 0, url: null }], layout: null }], active: 0 }));
  H.mock("save_tabs", () => null);
  prefs.tabs = [{ name: "", panes: [], layout: null }];
  prefs.active_tab = 0;
  await useTabsFor(WS);
  await refresh(true);
  setMode("code");
  setCodeView("pair");
  await settle(900);
  const row = [...document.querySelectorAll("#chat button, #side button, [role=button]")].find((b) => b.textContent.trim().startsWith("Add dark mode"));
  if (row) row.click();
  await settle(900);

  const radios = () => [...document.querySelectorAll(".hc-mode [role=radio]")].map((b) => `${b.textContent.trim()}:${b.getAttribute("aria-checked")}`);
  const placeholder = () => document.querySelector(".code-composer textarea")?.placeholder ?? null;
  const card = document.querySelector("[data-spec]");
  const inPlan = { radios: radios(), placeholder: placeholder(), planEdge: !!document.querySelector(".code-composer .hc.is-plan") };
  const verdict = card ? { state: card.dataset.spec, text: card.innerText.replace(/\s+/g, " ").trim() } : null;
  if (window.__keepPlan) return { opened: !!row, inPlan, verdict };

  document.querySelectorAll(".hc-mode [role=radio]")[1]?.click();
  await settle(300);
  const inFast = { radios: radios(), placeholder: placeholder(), planEdge: !!document.querySelector(".code-composer .hc.is-plan") };
  return { opened: !!row, inPlan, verdict, inFast };
})()
