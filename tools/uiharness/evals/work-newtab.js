// work-newtab.js: the ADE work area starts on the New tab page; each Open adds
// a closable tab and closing the last one returns to New tab.
(async () => {
  const wait = ms => new Promise(r => setTimeout(r, ms));
  const ws = "/home/me/workbench";
  __H.mock("snapshot", () => ({ name: "workbench", workspace: ws, agents: [{id:0,title:"Main agent",status:"idle",log:[],input_tokens:0,output_tokens:0,cost_usd:0,children:[],parent:null}], approvals: [], tickets: [], link: { state: "off" }, index: 0, total_cost_usd: 0, router_cost_usd: 0 }));
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
  __H.fire("state", {}); await __H.frames(4);
  setCodeView("pair"); await wait(400);
  const out = {};
  const q = s => document.querySelector(s);
  out.startOpen = !q("#work").hidden;
  out.startTitle = q("#work .wk-new h3") && q("#work .wk-new h3").textContent;
  out.cards = [...document.querySelectorAll("#work .wk-card b")].map(b => b.textContent);
  out.startTabs = [...document.querySelectorAll("#work [data-wt]")].map(b => b.textContent.trim());
  if (window.__NT === "start") return out;
  q('#work [data-open="files"]').click(); await wait(300);
  q('#work [data-file$="main.rs"]').click(); await wait(300);
  out.filesTab = q('#work [data-wt="files"]').getAttribute("aria-selected");
  q('#work [data-act="new"]').click(); await wait(200);
  q('#work [data-open="diffs"]').click(); await wait(300);
  out.diffsRows = document.querySelectorAll("#work [data-chg]").length;
  q('#work [data-act="new"]').click(); await wait(200);
  q('#work [data-open="terminal"]').click(); await wait(400);
  out.terminal = !!q("#work .tin");
  out.tabs = [...document.querySelectorAll("#work [data-wt]")].map(b => b.dataset.wt);
  if (window.__NT === "tabs") return out;
  q('#work [data-wx="terminal"]').click(); await wait(200);
  q('#work [data-wx="diffs"]').click(); await wait(200);
  q('#work [data-wx="files"]').click(); await wait(200);
  out.afterClose = [...document.querySelectorAll("#work [data-wt]")].map(b => b.dataset.wt);
  out.backToNew = !!q("#work .wk-new");
  return out;
})()
