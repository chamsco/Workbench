// The Ghostex-style chrome: one centered icon row on top, and a second
// utility rail at the bottom.
(async () => {
  const q = (s) => document.querySelector(s);
  const $$ = (s) => [...document.querySelectorAll(s)];
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const steps = [];
  const step = (n, ok, detail) => steps.push({ n, ok: !!ok, detail });

  const nav = q("#qNav");
  const top = q(".qtop");
  step("the command row lives in the top bar", !!nav && nav.closest(".qtop") === top);
  const nr = nav.getBoundingClientRect(), tr = top.getBoundingClientRect();
  const offset = Math.abs((nr.left + nr.width / 2) - (tr.left + tr.width / 2));
  step("the command row is centered", offset < 3, `${Math.round(offset)}px off`);
  step("the command row is one horizontal row", getComputedStyle(nav).flexDirection === "row");
  step("it carries all four modes", $$("#qNav .qnav-modes [data-q-go]").length === 4);
  step("the five work links start the sidebar", $$("#qWork [data-q-go]").length === 5);

  const dock = q("#qDock");
  step("the bottom rail exists", !!dock && dock.closest(".status") !== null);
  step("the bottom rail carries the three places", $$("#qDock .qd-places [data-q-go]").length === 3);
  step("the bottom rail has help", !!dock?.querySelector("[data-q-help]"));
  step("help sits right of the dock, before the places", (() => { const h = dock.querySelector("[data-q-help]").getBoundingClientRect(), p = dock.querySelector(".qd-places").getBoundingClientRect(); return h.right <= p.left && p.right > h.right; })());
  step("the machine name is on the Machines icon", /Machines · /.test(dock.querySelector('.qd-places [data-q-go="machines"]')?.dataset.qTip || ""));
  step("no machine label in the dock", !dock.querySelector(".qd-mach"));
  const foot = q("#qDock .qd-foot");
  const fr = foot.getBoundingClientRect(), sdr = q(".side").getBoundingClientRect(), stt = q(".status").getBoundingClientRect(), dr = dock.querySelector(".qd-places").getBoundingClientRect();
  step("the foot sits centred on the sidebar column", Math.abs((fr.left + fr.width / 2) - (sdr.left + sdr.width / 2)) < 3, `${Math.round((fr.left + fr.width / 2) - (sdr.left + sdr.width / 2))}px off`);
  step("one bottom row: foot and places share a line", Math.abs((fr.top + fr.height / 2) - (dr.top + dr.height / 2)) < 3);
  step("the bottom row closes the sheet under the sidebar", Math.abs(sdr.bottom - stt.top) < 2, `${Math.round(sdr.bottom)} / ${Math.round(stt.top)}`);
  step("the bottom row has settings", !!foot?.querySelector('[data-q-go="settings"]'));
  step("the bottom row has night mode", !!foot?.querySelector("[data-q-theme]"));
  step("the bottom row has glass", !!foot?.querySelector('[data-q-skin="glass"]'));
  const rail = q("#qRail");
  step("the project rail ends with the account", rail?.lastElementChild?.classList.contains("qr-user"));
  const tb = q(".qtop").getBoundingClientRect(), sd = q(".side").getBoundingClientRect();
  step("the top bar is slim", tb.height <= 36, `${Math.round(tb.height)}px`);
  step("the top bar and sidebar share one sheet", Math.abs(tb.bottom - sd.top) < 2 && Math.abs(tb.left - sd.left) < 2, `${Math.round(tb.bottom)} / ${Math.round(sd.top)}`);
  const wk = [...document.querySelectorAll("#qWork .qwork-ic")].map((b) => b.getBoundingClientRect());
  const wl = wk[0].left - sd.left, wr = sd.right - wk[wk.length - 1].right;
  step("the work rail is centred in the sidebar", Math.abs(wl - wr) < 3, `${Math.round(wl)} / ${Math.round(wr)}`);
  step("each work icon sits in a rounded square", parseFloat(getComputedStyle(q("#qWork .qwork-ic")).borderRadius) >= 6 && parseFloat(getComputedStyle(q("#qWork .qwork-ic")).borderTopWidth) >= 1);

  const theme = foot.querySelector("[data-q-theme]");
  const before = theme.dataset.qTheme;
  theme.click();
  await wait(400);
  step("night mode switches", document.documentElement.dataset.theme === before, document.documentElement.dataset.theme);

  Quiet.setSkin("glass");
  await wait(500);
  step("glass turns on", q("#win").classList.contains("q-glass"));
  step("glass paints a shared scene", /harbour-dusk|night/.test(getComputedStyle(q("#win"), "::before").backgroundImage));
  step("glass keeps the bottom row", !!q("#qDock .qd-foot [data-q-theme]"));
  Quiet.go("home"); await wait(500);
  step("the project page is a command center", !!q(".qh-hero") && !!q(".qh-side") && $$(".qh-panel").length >= 3);
  Quiet.go("code"); await wait(500);
  const modes = $$("[data-q-sgroup]").map((b) => b.dataset.qSgroup);
  step("sessions can be grouped four ways", modes.join() === "activity,where,added,folder", modes.join());
  const labels = {};
  for (const m of modes) { q(`[data-q-sgroup="${m}"]`).click(); await wait(250); labels[m] = $$("#qSide .qs-loc").map((e) => e.textContent.replace(/\d+$/, "").trim()).join("|"); }
  step("activity groups by state", /Working/.test(labels.activity) && /Idle/.test(labels.activity), labels.activity);
  step("where groups by machine", /This Mac/.test(labels.where) && /Cloud/.test(labels.where), labels.where);
  step("added groups by day", /Today/.test(labels.added), labels.added);
  step("folder groups by folder", /Atlas/.test(labels.folder), labels.folder);
  q('[data-q-sgroup="where"]').click(); await wait(200);
  Quiet.setSkin("light");
  await wait(300);
  step("glass turns off", !q("#win").classList.contains("q-glass"));

  return JSON.stringify({ pass: steps.every((s) => s.ok), steps });
})()
