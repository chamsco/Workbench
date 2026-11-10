// The global sidebar nav: the same five work links and four modes on every page.
(async () => {
  const q = (s) => document.querySelector(s);
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const nav = (k) => q(`#qWork [data-q-go="${k}"]`) || q(`#qNav [data-q-go="${k}"]`);
  const steps = [];
  const step = (n, ok, detail) => steps.push({ n, ok: !!ok, detail });
  const head = () => q(".q-page h1")?.textContent;

  step("global nav is mounted", !!q("#qNav .qnav-modes") && q("#qWork").children.length === 5);
  for (const [key, title] of [["needs", "Needs you"], ["board", "Board"], ["review", "Review"], ["automate", "Automate"], ["limits", "Limits"]]) {
    nav(key)?.click();
    await wait(250);
    step(`${key} from global nav`, head() === title, head());
    step(`${key} is active`, nav(key)?.classList.contains("on"));
  }

  nav("code")?.click();
  await wait(500);
  step("mode Code", !q("#qSession").hidden && nav("code")?.classList.contains("on"));
  nav("work")?.click();
  await wait(300);
  step("mode Work", head() === "Work" && nav("work")?.classList.contains("on"));
  nav("canvas")?.click();
  await wait(300);
  step("mode Canvas", head() === "Canvas" && nav("canvas")?.classList.contains("on"));
  nav("crew")?.click();
  await wait(800);
  step("mode Crew", !!q("#chatList .whirl-side") && nav("crew")?.classList.contains("on"));
  step("nav survives Crew", !!q("#qNav") && !!q("#qWork"));
  nav("limits")?.click();
  await wait(300);
  step("Limits from Crew", head() === "Limits");

  return JSON.stringify({ pass: steps.every((s) => s.ok), steps });
})()
