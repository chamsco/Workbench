// scratch eval: is the screenshot showing a stale frame?
(async () => {
  const g = () => {
    const win = document.querySelector("#win");
    const side = document.querySelector("#side");
    const rail = document.querySelector("#rail");
    return {
      cls: win.className,
      gtc: getComputedStyle(win).gridTemplateColumns,
      sideW: Math.round(side.getBoundingClientRect().width),
      sideDisplay: getComputedStyle(side).display,
      sideVis: getComputedStyle(side).visibility,
      railW: Math.round(rail.getBoundingClientRect().width),
      railDisplay: getComputedStyle(rail).display,
      maincLeft: Math.round(document.querySelector(".mainc").getBoundingClientRect().left),
    };
  };
  const r = { atReady: g() };
  await new Promise((res) => requestAnimationFrame(() => requestAnimationFrame(res)));
  r.afterTwoFrames = g();
  // Force a fresh paint of the whole page before the harness screenshots.
  document.body.offsetHeight;
  await new Promise((res) => setTimeout(res, 250));
  r.afterSettle = g();
  return r;
})()
