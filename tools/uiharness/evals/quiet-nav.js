// Navigation smoke test for the quiet shell: the Home sidebar's quick links,
// the pages we own, and the surfaces we hand back to the engine.
(async () => {
  const q = (s) => document.querySelector(s);
  const pick = (sel, text) => [...document.querySelectorAll(sel)].find((b) => b.textContent.includes(text));
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const steps = [];
  const step = (n, ok) => steps.push({ n, ok: !!ok });

  await wait(200);
  const homeTitle = () => q(".q-page h1")?.textContent;
  step("home", homeTitle() === "Friday demo" || homeTitle() === "Workbench");

  q('#qWork [data-q-go="board"]')?.click();
  await wait(300);
  step("quick board", q(".q-page h1")?.textContent === "Board");

  Quiet.go("needs");
  await wait(250);
  step("needs", q(".q-page h1")?.textContent === "Needs you");

  Quiet.go("work");
  await wait(250);
  step("work", q(".q-page h1")?.textContent === "Work");

  Quiet.go("canvas");
  await wait(250);
  step("canvas", q(".q-page h1")?.textContent === "Canvas");

  Quiet.go("limits");
  await wait(250);
  step("limits", q(".q-page h1")?.textContent === "Limits");

  Quiet.go("settings");
  await wait(250);
  step("settings hub", q(".q-page h1")?.textContent === "Settings");
  q('[data-q-settings-section="appearance"]')?.click();
  await wait(500);
  step("settings opens on appearance", !q("#settings").hidden && /Appearance/.test(q(".setnav .sv.on")?.textContent || ""));
  q("#setClose")?.click();
  await wait(250);

  Quiet.go("crew");
  await wait(700);
  step("crew", !!q("#chat") && !!q("#chatList .whirl-side"));

  Quiet.go("home");
  await wait(250);
  step("back home", homeTitle() === "Friday demo" || homeTitle() === "Workbench");

  return JSON.stringify({ pass: steps.every((s) => s.ok), steps });
})()
