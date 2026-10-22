// Code: the project workbench — tabs of canvases, the agent grid, the drawer.
//
// It is an app the frame hosts, like Chat. The engine (tabs in app.js, the
// canvas renderers beside them) still lives in the page for now; this file is
// the app's edge: where its surface is, which window classes it owns, and what
// the title bar's New button does. Moving the engine in here is the next step
// (docs/extensions.md).
if (window.Frame) Frame.register({
  id: "code",
  // In ADE (pair) the chat face shows the project's session; in the IDE the
  // canvas grid does. The manifest names the section; elFor says which one the
  // zone is showing right now.
  mount: el => { el.innerHTML = '<div class="grid" id="grid"></div>'; },
  elFor: () => (S.codeView === "pair" && hasProject && snap.workspace ? "chat" : "canvas"),
  classes: () => (S.codeView === "pair" ? ["pair-on"] : []),
  newInstance: anchor => newTabPop(anchor),
});
