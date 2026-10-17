// tools/verify/agents-new-bot.js — screenshot run for the New bot sheet.
//
// Run AFTER agents-projects.js in the same uiharness invocation (it mocks the
// commands, switches to Agents and leaves the home open). This file opens the
// sheet and picks the Morning digest template, so the screenshot shows the
// collapsed Start From row with the prefilled name/face/job/routine sections.
//
//   node tools/uiharness/run.mjs --wait 900 \
//     --eval-file tools/verify/agents-projects.js \
//     --eval-file tools/verify/agents-new-bot.js \
//     --shot tools/verify/out/agents-new-bot.png
(async () => {
  const A = window.__AGENTS;
  if (!A) return { error: "agents-projects.js must run first in the same uiharness run" };
  const opened = await A.openNewBot();
  const picked = opened ? await A.pickTemplate("Morning digest") : false;
  return { opened, picked, state: A.state() };
})()
