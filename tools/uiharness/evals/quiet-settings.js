// Clicking a Settings card opens the existing sheet on that section.
(async () => {
  const q = (s) => document.querySelector(s);
  Quiet.go("settings");
  await new Promise((r) => setTimeout(r, 300));
  q('[data-q-settings-section="appearance"]')?.click();
  await new Promise((r) => setTimeout(r, 600));
  return JSON.stringify({
    open: !q("#settings").hidden,
    active: q(".setnav .sv.on")?.textContent,
    appearanceShown: !q("#set-appearance")?.hidden,
    win: q("#win").className,
    settingsWidth: q("#settings").getBoundingClientRect().width,
  });
})()
