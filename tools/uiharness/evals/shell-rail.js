// shell-rail.js: the interaction ledger for the top rail, Settings' close and
// the six-canvas IDE (docs/SHELL-AUDIT.md). Seeds a project, then clicks
// through and returns what each step left behind.
//   node tools/uiharness/run.mjs --eval-file tools/uiharness/evals/shell-rail.js
(async () => {
  await (async () => {
  const snapF = () => ({ name: "workbench", workspace: "/home/me/workbench", agents: [{id:0,title:"Main agent",status:"idle",log:[],input_tokens:0,output_tokens:0,cost_usd:0,children:[],parent:null}], total_cost_usd: 0, router_cost_usd: 0, approvals: [], tickets: [], link: { state: "off" }, index: 0 });
  window.__H.mock("snapshot", snapF);
  window.__H.mock("git_activity", () => ({days:[1,0,3,2,0,4,1,0,0,2,5,1,0,3], added: 120, removed: 40}));
  window.__H.fire("state", {});
  await window.__H.frames(4);
  const v = window.__VIEW || "home";
  if (v === "chat") setMode("chat");
  if (v === "ade") { setMode("code"); setCodeView("pair"); }
  if (v === "ide") { setMode("code"); setCodeView("agents"); }
  if (v === "settings") openSettings();
  await new Promise(r => setTimeout(r, 900));
  return { mode: S.mode, codeView: S.codeView, settings: S.settings, hasProject };
})();
  const wait = ms => new Promise(r => setTimeout(r, ms));
  const out = {};
  const st = () => ({ mode: S.mode, codeView: S.codeView, chatView: S.chatView, settings: !!S.settings, section: S.setSection });
  out.zones = [...document.querySelectorAll('#zones [data-zz]')].map(b => b.dataset.zz);
  out.chatInNav = out.zones.includes('chat');
  setMode('home'); await wait(200);
  document.querySelector('#newThread').click(); await wait(300); out.newThread = st();
  const w = document.querySelector('#win');
  document.querySelector('#sideClose').click(); await wait(100); out.sideAfter1 = w.classList.contains('side-closed');
  document.querySelector('#sideClose').click(); await wait(100); out.sideAfter2 = w.classList.contains('side-closed');
  document.querySelector('#plugBtn').click(); await wait(300); out.plugins = st();
  document.querySelector('#skillBtn').click(); await wait(300); out.skills = st(); out.skillsPressed = document.querySelector('#skillBtn').getAttribute('aria-pressed');
  document.querySelector('#setX').click(); await wait(200); out.afterClose = st();
  document.querySelector('#agentBtn').click(); await wait(300); out.agents = st();
  setCodeView('pair'); await wait(200);
  dispatchEvent(new KeyboardEvent('keydown', { key: 'j', ctrlKey: true })); await wait(300); out.ctrlJ = st();
  dispatchEvent(new KeyboardEvent('keydown', { key: 'n', ctrlKey: true })); await wait(300); out.ctrlN = st();
  // IDE: fill a tab to six
  setCodeView('agents'); await wait(300);
  const pickerKinds = [];
  for (let i = 0; i < 6; i++) {
    document.querySelector('#addCanvas').click(); await wait(150);
    const pop = document.querySelector('#pop');
    if (i === 0) pickerKinds.push(...[...pop.querySelectorAll('[data-kind]')].map(b => b.dataset.kind));
    const b = pop.querySelector('[data-kind="terminal"]') || pop.querySelector('[data-kind="files"]');
    if (!b) { out.fullNote = pop.textContent.trim().slice(0, 120); closePop(); break; }
    b.click(); await wait(200);
  }
  out.pickerKinds = pickerKinds;
  out.panes = prefs.tabs[prefs.active_tab].panes.length;
  out.cells = document.querySelectorAll('#grid .pane, #grid > [data-slot]').length;
  return out;
})()
