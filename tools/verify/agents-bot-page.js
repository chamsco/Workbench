// tools/verify/agents-bot-page.js — screenshot run for the bot page.
//
// Run AFTER agents-projects.js in the same uiharness invocation: that file sets
// up the mocks, switches Chat -> Agents and leaves the Agents home open, then
// exposes window.__AGENTS. This file only opens the mocked busy bot's thread
// (Keeper: no messages, one routine) and leaves it on screen for --shot.
//
//   node tools/uiharness/run.mjs --wait 900 \
//     --eval-file tools/verify/agents-projects.js \
//     --eval-file tools/verify/agents-bot-page.js \
//     --shot tools/verify/out/agents-bot-page.png
(async () => {
  const A = window.__AGENTS;
  if (!A) return { error: "agents-projects.js must run first in the same uiharness run" };
  const opened = await A.openBot("Keeper");
  return { opened, state: A.state() };
})()
