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
  step("the bottom rail has settings", !!dock?.querySelector('[data-q-go="settings"]'));
  step("the bottom rail has night mode", !!dock?.querySelector("[data-q-theme]"));
  step("the bottom rail has glass", !!dock?.querySelector('[data-q-skin="glass"]'));
  step("the bottom rail has account", !!dock?.querySelector(".qd-user"));

  const theme = dock.querySelector("[data-q-theme]");
  const before = theme.dataset.qTheme;
  theme.click();
  await wait(400);
  step("night mode switches", document.documentElement.dataset.theme === before, document.documentElement.dataset.theme);

  Quiet.setSkin("glass");
  await wait(500);
  step("glass turns on", q("#win").classList.contains("q-glass"));
  step("glass paints a shared scene", /harbour-dusk|night/.test(getComputedStyle(q("#win"), "::before").backgroundImage));
  step("glass keeps the bottom rail", !!q("#qDock [data-q-theme]"));
  Quiet.setSkin("light");
  await wait(300);
  step("glass turns off", !q("#win").classList.contains("q-glass"));

  return JSON.stringify({ pass: steps.every((s) => s.ok), steps });
})()
