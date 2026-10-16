// tools/uiharness/evals/ghost-collapse.js
//
// Acceptance B: measure the sidebar and the window's grid tracks before and
// after .win gets `ghost-side` — the class the app's ghost-chat toggle sets
// (chat.js host.ghostSide) and style.css/shell.css use to collapse the sidebar.
//
// The app boots with the sidebar closed at a narrow-ish width, and a closed
// sidebar is already 0px wide, so this opens it first: measuring a collapse
// needs something to collapse. Everything is read off the live box tree and
// getComputedStyle, never off the CSS source.
//
// Run with --eval-file: the whole thing is one expression that awaits frames.
(async () => {
  const win = document.querySelector("#win");
  const side = document.querySelector("#side");
  const rail = document.querySelector("#rail");
  const gut = document.querySelector("#sideGut");
  const mainc = document.querySelector(".mainc");
  const frames = (n) => new Promise((r) => {
    let left = n;
    const tick = () => (--left <= 0 ? r() : requestAnimationFrame(tick));
    requestAnimationFrame(tick);
  });
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const px = (v) => (v == null ? null : Math.round(v * 1000) / 1000);
  const read = () => {
    const w = getComputedStyle(win);
    return {
      "win.className": win.className,
      "grid-template-columns": w.gridTemplateColumns,
      "win.--side-w": w.getPropertyValue("--side-w").trim(),
      "win.--side-w-user": w.getPropertyValue("--side-w-user").trim() || null,
      "win.width": px(win.getBoundingClientRect().width),
      "side.width": px(side.getBoundingClientRect().width),
      "side.computed.width": getComputedStyle(side).width,
      "side.visibility": getComputedStyle(side).visibility,
      "side.transform": getComputedStyle(side).transform,
      "side.transition-property": getComputedStyle(side).transitionProperty,
      "win.transition-property": w.transitionProperty,
      "rail.width": px(rail.getBoundingClientRect().width),
      "gutter.display": getComputedStyle(gut).display,
      "mainc.left": px(mainc.getBoundingClientRect().left),
      "mainc.width": px(mainc.getBoundingClientRect().width),
    };
  };

  // Start from a genuinely open sidebar.
  win.classList.remove("side-closed");
  await settle(450); // past the 0.28s open transition
  const open = read();

  win.classList.add("ghost-side");
  await frames(2); // mid-transition: this is the point of the exercise
  const ghostAfterTwoFrames = read();
  await settle(450); // past the 0.28s collapse
  const ghostSettled = read();

  win.classList.remove("ghost-side");
  await frames(2);
  await settle(450);
  const restored = read();

  return { open, ghostAfterTwoFrames, ghostSettled, restored };
})()
