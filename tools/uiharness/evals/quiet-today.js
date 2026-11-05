// The per-project day view, the fleet page, and the team sheet: navigation,
// grouping, the agent drawer with fallbacks, and the doctor.
(async () => {
  const q = (s) => document.querySelector(s);
  const $$ = (s) => [...document.querySelectorAll(s)];
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const steps = [];
  const step = (n, ok, detail) => steps.push({ n, ok: !!ok, detail });

  const rail = q(".rail");
  const rw = rail ? rail.getBoundingClientRect().width : 999;
  step("project rail is slim", rw > 0 && rw < 50, `${Math.round(rw)}px`);
  step("project rail is flush with the frame", parseFloat(getComputedStyle(rail).borderTopLeftRadius) < 2 && parseFloat(getComputedStyle(rail).marginBottom) < 2);

  Quiet.go("today");
  await wait(350);
  const first = q(".qt-date")?.textContent;
  step("today opens", !!first && !!q(".qt-head") && !!q(".qt-sub"));
  step("today is grouped", $$(".qt-group").length >= 2);

  q('[data-q-day="-1"]')?.click();
  await wait(300);
  const prev = q(".qt-date")?.textContent;
  step("day goes back", !!prev && prev !== first, `${first} -> ${prev}`);
  q("[data-q-go='today']")?.click();
  await wait(300);
  step("today comes back", q(".qt-date")?.textContent === first);

  const pick = q("#qDayPick");
  if (pick) {
    pick.value = new Date(Date.now() - 864e5).toISOString().slice(0, 10);
    pick.dispatchEvent(new Event("change", { bubbles: true }));
    await wait(300);
    step("date picker changes the day", q(".qt-date")?.textContent !== first, q(".qt-date")?.textContent);
  }

  const group = q("#qDayGroup");
  if (group) {
    group.value = "project";
    group.dispatchEvent(new Event("change", { bubbles: true }));
    await wait(300);
    step("chats regroup by project", group.value === "project" && !!q(".qt-group-h"));
  }

  Quiet.go("machines");
  await wait(400);
  step("fleet lists machines", $$(".qt-machine").length >= 5, `${$$(".qt-machine").length} cards`);
  step("harnesses table is live", $$(".qt-htable tbody tr").length >= 4, `${$$(".qt-htable tbody tr").length} rows`);
  step("harness updates are offered", $$("[data-q-harness-update]").length > 0);

  Quiet.go("team");
  await wait(500);
  step("team is listed", $$(".qt-botcard").length >= 3, `${$$(".qt-botcard").length} bots`);
  step("doctor is present", !!q(".qt-doctor"));
  step("doctor state is visible", /All chats healthy|need repair/.test(q(".qt-doctor small")?.textContent || ""), q(".qt-doctor small")?.textContent);

  q(".qt-botcard")?.click();
  await wait(300);
  step("agent sheet opens", !!q(".qt-drawer"));
  step("instructions are editable", !!q("#qaInstr"));
  step("runs-on is a host picker", $$("#qaComputer option").length >= 5);
  step("fallbacks are editable", !!q("[data-q-fb-add]"));
  const before = $$(".qt-fb-row").length;
  q("[data-q-fb-add]")?.click();
  await wait(300);
  step("a fallback can be added", $$(".qt-fb-row").length === before + 1);

  const name = q("#qaName");
  if (name) name.value = "Claude Prime";
  q("[data-q-agent-save]")?.click();
  await wait(600);
  step("the sheet saves", !q(".qt-drawer"));
  step("the card takes the new name", $$(".qt-botcard b").some((b) => b.textContent === "Claude Prime"));

  q("[data-q-doctor]")?.click();
  await wait(700);
  step("doctor runs", /checked (now|\d+m)|healthy/.test(q(".qt-doctor small")?.textContent || ""), q(".qt-doctor small")?.textContent);

  return JSON.stringify({ pass: steps.every((s) => s.ok), steps });
})()
