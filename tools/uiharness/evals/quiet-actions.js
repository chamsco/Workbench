// The actions the quiet pages hand back to the engine still call it.
(async () => {
  const q = (s) => document.querySelector(s);
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const steps = [];
  const step = (n, ok, detail) => steps.push({ n, ok: !!ok, detail });

  Quiet.go("canvas");
  await wait(250);
  const before = prefs.tabs[prefs.active_tab].panes.length;
  q("[data-q-add-canvas]")?.click();
  await wait(500);
  const after = prefs.tabs[prefs.active_tab]?.panes.length;
  step("new canvas adds a pane", after === before + 1, `${before} -> ${after}`);
  step("new canvas lands in Code", q("#win").classList.contains("q-code") && q("#qView").hidden, q("#win").className);

  Quiet.go("board");
  await wait(300);
  q("[data-q-ticket]")?.click();
  await wait(400);
  step("ticket opens the drawer", !q("#drawer").hidden, q("#win").className);

  Quiet.go("needs");
  await wait(300);
  q("[data-q-ap]")?.click();
  await wait(400);
  step("approval opens the drawer", !q("#drawer").hidden && S.review != null);

  Quiet.go("home");
  await wait(300);
  q("[data-q-new]")?.click();
  await wait(500);
  step("new session opens the chat face", !!q("#chat") && q("#win").classList.contains("q-code"));

  Quiet.go("home");
  await wait(250);
  q("[data-q-help]")?.click();
  await wait(250);
  step("helper opens", q("#qHelpLayer") && !q("#qHelpLayer").hidden);
  q("[data-q-help-q]")?.click();
  await wait(450);
  step("helper answers a suggestion", q("#qHelpThread")?.querySelectorAll(".qhelp-msg").length === 2);
  q("[data-q-help-close]")?.click();
  await wait(200);
  step("helper closes", q("#qHelpLayer").hidden);

  return JSON.stringify({ pass: steps.every((s) => s.ok), steps });
})()
