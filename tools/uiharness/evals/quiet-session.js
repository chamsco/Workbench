// Session/Panes switch, filter and send.
(async () => {
  const q = (s) => document.querySelector(s);
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  Quiet.go("code");
  await wait(400);
  const steps = [];
  const step = (n, ok, detail) => steps.push({ n, ok: !!ok, detail });
  step("session is the default", !q("#qSession").hidden && q("#body").hidden);
  step("the footer is full width and visible", !q("#qFooter").hidden && q("#qFooter").getBoundingClientRect().width > 600);
  step("the session uses provider marks", q("#qSession .qs-prov") && q("#qSession .qs-prov").innerHTML.includes("<svg"));
  q('[data-q-sview="panes"]')?.click();
  await wait(400);
  step("panes switch shows the grid", q("#qSession").hidden && !q("#body").hidden && getComputedStyle(q("#appHead")).display === "flex");
  step("panes hides the footer", q("#qFooter").hidden);
  q('[data-q-sview="session"]')?.click();
  await wait(400);
  step("session comes back", !q("#qSession").hidden && q("#body").hidden);
  const inp = q("#qsInput");
  inp.value = "Ship the receipt copy.";
  inp.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  await wait(400);
  step("composer sends to the lead", [...document.querySelectorAll("#qSession .qs-bubble.me")].pop()?.textContent === "Ship the receipt copy.");
  const chip = q('#qSession [data-q-filter]:not([data-q-filter="all"])');
  chip?.click();
  await wait(250);
  step("filter chip narrows the transcript", q(".qs-filter-chip.on")?.textContent.includes("Claude Code"));
  return JSON.stringify({ pass: steps.every((s) => s.ok), steps });
})()
