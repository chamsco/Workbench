// tools/verify/stages.js — the four-stage strip on the ADE home.
// Mid-Execute (one ticket still running, so the gate is shut), then Commit
// with notes waiting. Run with --eval-file; pass ?stage=commit for the second.
(async () => {
  const H = window.__H;
  const settle = ms => new Promise(r => setTimeout(r, ms));
  const COMMIT = location.search.includes("commit");
  const WS = "/home/me/code/halo";
  const route = { kind: "cli", provider: "claude", model: null };
  H.mock("snapshot", () => ({ name: "halo", local: true, workspace: WS, agents: [{ id: 0, key: "0", parent: null, depth: 0, title: "Main agent", kind: "main", status: "idle", log: [], artifacts: [], escalations: [], cost_usd: 0, input_tokens: 0, output_tokens: 0 }], approvals: [], tickets: [], artifacts: [], options: [], board: [], link: { state: "online", error: null }, index: 0, total_cost_usd: 1.8, router_cost_usd: 0.04 }));
  H.mock("harnesses", () => ([{ id: "claude", name: "Claude Code", version: "2.1", kind: "cli", installed: true, enabled: true, auth: "authenticated", chat: true, detail: "Opus", message: null }]));
  H.mock("git_activity", () => ({ days: [1, 0, 3, 2, 0, 4, 1, 0, 2, 1, 0, 3, 2, 1], added: 900, removed: 300 }));
  H.mock("tabs_for", () => ({ tabs: [{ name: null, panes: [{ kind: "empty", agent: 0, url: null }], layout: null }], active: 0 }));
  let advanced = 0;
  const status = (phase, cur) => ({
    phase, stage: cur,
    stages: [
      { key: "plan", state: cur === "plan" ? "current" : "done", model: route, spent: 0.42, decided: 0.01 },
      { key: "execute", state: cur === "execute" ? "current" : cur === "plan" ? "todo" : "done", model: null, spent: 1.38, decided: 0.03 },
      { key: "ship", state: cur === "ship" ? "current" : cur === "commit" ? "done" : "todo", model: null, spent: 0, decided: 0 },
      { key: "commit", state: cur === "commit" ? "current" : "todo", model: { kind: "local", provider: "ollama", model: "granite3.2:2b" }, spent: 0, decided: 0 },
    ],
    gates: [], gaps: cur === "execute" ? ["T-3 is still in-progress"] : [], advance: cur === "execute" ? "rules" : cur === "ship" ? "human" : "librarian",
    notes: cur === "commit" ? 5 : 2, receipt: null, total: 1.8,
  });
  H.mock("stage_status", () => (COMMIT ? status("commit", "commit") : status("execute", "execute")));
  H.mock("advance_stage", () => { advanced++; return status("done", null); });
  await Code.loadTabs(WS);
  await Providers.load().catch(() => {});
  await refresh(true);
  setMode("code"); setCodeView("pair");
  await refresh(true);
  await settle(900);
  const strip = document.querySelector(".stg:not([hidden])");
  const names = strip ? [...strip.querySelectorAll(".stg-t b")].map(b => b.textContent) : null;
  const models = strip ? [...strip.querySelectorAll(".stg-m")].map(b => b.textContent) : null;
  const go = strip && strip.querySelector("[data-go]");
  const before = { disabled: go && go.disabled, label: go && go.textContent, note: strip && strip.querySelector(".stg-note").textContent };
  if (COMMIT && go) { go.click(); await settle(500); }
  return { names, models, before, advanced, after: strip && strip.querySelector(".stg-note").textContent };
})()
