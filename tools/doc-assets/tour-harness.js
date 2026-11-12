// tour-harness.js: the README stills and tour.gif, from the UI harness on
// mocked commands. It shows window.__STEP (home, ade, files, review, logs,
// wall, folded; default home): set it as the first line inside the function.
(async () => {
  const wait = ms => new Promise(r => setTimeout(r, ms));
  const ws = "/home/me/workbench";
  __H.mock("git_activity", () => ({days:[1,0,3,2,0,4,1,0,0,2,5,1,0,3], added: 120, removed: 40}));
  __H.mock("list_files", () => [
    {path: ws+"/src", name:"src", dir:true, depth:0}, {path: ws+"/src/main.rs", name:"main.rs", dir:false, depth:1},
    {path: ws+"/src/ui.rs", name:"ui.rs", dir:false, depth:1}, {path: ws+"/docs", name:"docs", dir:true, depth:0},
    {path: ws+"/docs/shot.png", name:"shot.png", dir:false, depth:1}, {path: ws+"/README.md", name:"README.md", dir:false, depth:0}]);
  __H.mock("read_file", a => a.path.endsWith("main.rs") ? 'fn main() {\n    println!("hello");\n    ui::run();\n}\n' : "# workbench\n");
  const png = "data:image/svg+xml;utf8," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="320" height="200"><rect width="320" height="200" fill="#2a66d9"/><text x="20" y="110" font-size="28" fill="white" font-family="sans-serif">shot.png</text></svg>');
  __H.mock("read_image", () => png);
  __H.mock("git_changes", () => ({ branch: "feat/rail", fresh: false, files: [{path:"src/main.rs", status:" M", added: 2, removed: 1}, {path:"src/ui.rs", status:"??", added: 14, removed: 0}] }));
  __H.mock("git_diff", () => "@@ -1,3 +1,4 @@\n fn main() {\n-    println!(\"hi\");\n+    println!(\"hello\");\n+    ui::run();\n }\n");
  __H.mock("git_log", () => [{subject:"Shell: top rail", hash:"b4334b8", author:"Claude", when:"1 hour ago", refs:""}, {subject:"Windows build", hash:"e669e8d", author:"Charles", when:"4 days ago", refs:""}]);
  __H.mock("chat_list", () => [{id:"t1", title:"Add the work area", project: ws, updated: Date.now()-120000, members: []}]);
  __H.mock("chat_thread", () => ({id:"t1", title:"Add the work area", messages:[{role:"user", text:"go"}, {role:"assistant", text:"done", trace:"tr1"}]}));
  __H.mock("trace_get", () => ({ trace_id:"tr1", spans:[{name:"reply · claude", start:0, end:4200, attrs:{"gen_ai.request.model":"claude-code", "gen_ai.usage.input_tokens": 1830, "gen_ai.usage.output_tokens": 412}}, {name:"tool · read_file", start:300, end:900, input:"src/main.rs"}, {name:"tool · edit", start:1200, end:2600, input:"src/main.rs"}, {name:"model", start:2700, end:4100}]}));
  __H.mock("terminal_open", () => "term1");
  __H.mock("terminal_read", () => ({ output: "", alive: true }));
  const W = (id, title, key, status, log, model) => ({ id, title, key, ticket: key, branch: "backspace/" + key, status, kind: "worker", parent: 0, children: [], input_tokens: 18200 + id * 3100, output_tokens: 2400 + id * 400, cost_usd: 0.04 * id, decision: { model, effort: "medium" }, log });
  const snap = { name: "workbench", workspace: ws, total_cost_usd: 0.31, router_cost_usd: 0.002, approvals: [], link: { state: "off" }, index: 0,
    tickets: [{ key: "auth-api", title: "Sign-up and login endpoints", state: "in_progress" }, { key: "auth-ui", title: "Login screen", state: "in_progress" }, { key: "auth-tests", title: "Auth tests", state: "queued" }],
    agents: [
      { id: 0, title: "Main agent", status: "running", kind: "main", parent: null, children: [1, 2, 3], input_tokens: 24100, output_tokens: 3100, cost_usd: 0.12, decision: { model: "claude-sonnet", effort: "high" }, log: [{ kind: "user", text: "Users should be able to sign up and log in." }, { kind: "assistant", text: "Plan approved: 3 tickets. Dispatching auth-api and auth-ui in parallel." }] },
      W(1, "Sign-up and login endpoints", "auth-api", "running", [{ kind: "tool_call", text: "read src/server.rs" }, { kind: "tool_call", text: "edit src/auth.rs" }, { kind: "assistant", text: "Added /signup and /login with argon2 hashing." }, { kind: "tool_call", text: "bash cargo test auth" }], "codex"),
      W(2, "Login screen", "auth-ui", "running", [{ kind: "tool_call", text: "read web/src/App.tsx" }, { kind: "assistant", text: "Building the form with inline validation." }, { kind: "tool_call", text: "write web/src/Login.tsx" }], "claude-code"),
      W(3, "Auth tests", "auth-tests", "queued", [], "qwen3-4b")] };
  const now = Date.now(), D = 864e5, provs = ["claude-code", "codex", "ollama"];
  const prompts = []; for (let i = 0; i < 30; i++) { const k = (i * 7 % 5) + (i % 6 === 0 ? 4 : 1); for (let j = 0; j < k; j++) prompts.push({ at: now - (29 - i) * D + j * 6e5, provider: provs[(i + j) % 3], agent: j % 4 === 3 ? "kira" : null }); }
  __H.mock("chat_activity", () => ({ prompts, hours: Array.from({ length: 41 }, (_, i) => i), sessions: 63 }));
  __H.mock("memory_list", () => [
    { text: "Use pnpm, never npm, in this repo.", project: ws, source: "you", created: now - 3e6 },
    { text: "Auth tokens live 15 minutes; refresh tokens 30 days.", project: ws, source: "agent", created: now - 9e6 },
    { text: "Every UI change ships with before and after screenshots.", project: null, source: "chat", created: now - 2 * D },
    { text: "The staging database is read-only on Fridays.", project: null, source: "you", created: now - 4 * D }]);
  __H.mock("agents_list", () => [{ id: "kira", name: "Kira", avatar: { color: "#ff7f95" } }]);
  __H.mock("snapshot", () => snap);
  __H.mock("chat_list", () => [{id:"t1", title:"Sign-up and login", project: ws, updated: Date.now()-120000, members: []}, {id:"t2", title:"Fix the flaky upload test", project: ws, updated: Date.now()-3600e3, members: []}]);
  __H.fire("state", {}); await __H.frames(4);
  const q = s => document.querySelector(s), sleep = ms => new Promise(r => setTimeout(r, ms));
  const STEP = window.__STEP || "home";
  if (STEP === "home") { try { prefs.projects = [ws, "/home/me/brainpowr-site"]; } catch (e) {} Dash.stale(); await sleep(900); return STEP; }
  setMode("code"); setCodeView("pair"); await sleep(500);
  if (STEP === "ade") return STEP;
  const open = async t => { q('#work [data-act="new"]').click(); await sleep(150); q(`#work [data-open="${t}"]`).click(); await sleep(400); };
  if (STEP === "files") { await open("files"); q('#work [data-file$="main.rs"]').click(); await sleep(400); return STEP; }
  if (STEP === "review") { await open("files"); await open("diffs"); q('#work [data-chg="src/main.rs"]').click(); await sleep(400); return STEP; }
  if (STEP === "logs") { await open("files"); await open("diffs"); await open("logs"); await sleep(600); return STEP; }
  if (STEP === "ide") { setCodeView("agents"); await sleep(800); return STEP; }
  if (STEP === "wall") { setCodeView("agents"); await sleep(400); if (window.Code && Code.setWb) Code.setWb("wall"); await sleep(800); return STEP; }
  if (STEP === "folded") { q("#sideClose").click(); await sleep(600); return STEP; }
  return STEP;
})()
