// tools/uiharness/evals/computed-style.js
//
// The other half of the point of the harness: what a stylesheet *says* and what
// the browser *computed* are two different things, and only the second one is
// the UI. Everything here is read through getComputedStyle on live elements.
//
// Run: node tools/uiharness/run.mjs --eval-file tools/uiharness/evals/computed-style.js
(() => {
  const pick = (el, props) => {
    const cs = getComputedStyle(el);
    const out = {};
    for (const p of props) out[p] = cs.getPropertyValue(p).trim();
    const r = el.getBoundingClientRect();
    out["_box"] = `${Math.round(r.width)}x${Math.round(r.height)} at ${Math.round(r.left)},${Math.round(r.top)}`;
    return out;
  };
  const q = (sel) => document.querySelector(sel);
  const report = {};
  const targets = {
    "html": "html",
    "#win": "#win",
    "#side": "#side",
    "#side .row:first-child": "#side .row",
    "#chat": "#chat",
    ".tbar": ".tbar",
    "#status": "#status",
  };
  for (const [label, sel] of Object.entries(targets)) {
    const el = q(sel);
    report[label] = el
      ? pick(el, ["display", "position", "width", "height", "grid-template-columns", "background-color", "color", "font-family", "font-size", "border-radius", "transition-property", "visibility", "z-index"])
      : "NOT PRESENT";
  }
  // The CSS custom properties the shell leans on, as resolved on the window.
  const win = q("#win");
  report["#win custom properties"] = win
    ? Object.fromEntries(
        ["--side-w", "--side-w-user", "--rail-w", "--glass", "--glass-side", "--fg", "--fg-3", "--pane-edge"].map((p) => [
          p,
          getComputedStyle(win).getPropertyValue(p).trim(),
        ]),
      )
    : "NOT PRESENT";
  // Which stylesheets actually loaded, and how many rules each contributes.
  report.stylesheets = [...document.styleSheets].map((s) => {
    let rules = "?";
    try {
      rules = s.cssRules.length;
    } catch {
      rules = "blocked";
    }
    return { href: s.href ? s.href.replace(location.origin, "") : "(inline)", rules };
  });
  return report;
})()
