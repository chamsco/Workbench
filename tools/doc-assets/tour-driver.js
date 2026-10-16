/* Injected into design/workbench.html by tools/doc-assets/shoot.ps1.
   It reads the query string and drives the mock's own state, so a headless
   render can reproduce any screen of the design without a click.
   It also draws a pointer (and an optional click ring) so stills and the
   tour GIF read like a real session.

   Query keys: env, side, review, btab, theme, cursor=x,y, ring=r, hide=<css> */
(function () {
  var q = new URLSearchParams(location.search);

  if (q.get("theme")) S.theme = q.get("theme");

  // Keep the mock's fake clock out of the picture unless asked for.
  if (q.get("nospin") === "1") {
    document.querySelectorAll(".spin-g").forEach(function (g) { g.textContent = "\u00b7"; });
    setInterval = function () { return 0; };
  }

  if (q.get("review")) S.review = q.get("review");
  if (q.get("btab")) S.btab = q.get("btab");
  if (q.get("side")) { S.side = q.get("side"); renderSeg(); renderList(); }
  if (q.get("env")) setEnv(q.get("env"));
  applyTheme();

  if (q.get("hide")) {
    q.get("hide").split("|").forEach(function (sel) {
      document.querySelectorAll(sel).forEach(function (el) { el.style.visibility = "hidden"; });
    });
  }

  function layer(styles) {
    var d = document.createElement("div");
    d.setAttribute("aria-hidden", "true");
    d.style.cssText = "position:fixed;z-index:2147483647;pointer-events:none;" + styles;
    document.body.appendChild(d);
    return d;
  }

  var ring = q.get("ring");
  var c = (q.get("cursor") || "").split(",");
  var cx = parseFloat(c[0]), cy = parseFloat(c[1]);
  var hasCursor = c.length === 2 && isFinite(cx) && isFinite(cy);

  if (ring && hasCursor) {
    var r = Math.max(0, Math.min(1, parseFloat(ring)));
    var size = 16 + r * 44;
    var el = layer("left:" + (cx - size / 2) + "px;top:" + (cy - size / 2) + "px;" +
      "width:" + size + "px;height:" + size + "px;border-radius:50%;" +
      "border:2px solid rgba(255,255,255,.85);background:rgba(255,255,255," + (0.22 * (1 - r)) + ");" +
      "box-shadow:0 0 0 1px rgba(0,0,0,.25);opacity:" + (1 - r) + ";");
    el.dataset.ring = "1";
  }

  if (hasCursor) {
    layer("left:" + cx + "px;top:" + cy + "px;filter:drop-shadow(0 2px 4px rgba(0,0,0,.4));").innerHTML =
      '<svg viewBox="0 0 24 24" width="23" height="23">' +
      '<path d="M5.5 2.2 L5.5 18.6 L9.6 14.7 L12.2 20.8 L15.1 19.5 L12.5 13.4 L18.2 13 Z" ' +
      'fill="#ffffff" stroke="#15121a" stroke-width="1.15" stroke-linejoin="round"/></svg>';
  }
})();
