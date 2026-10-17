// tools/verify/agents-ask.js — screenshot for the controls a bot can put in a
// reply: the question card (options + Skip/Answer) and the routine proposal
// (Add routine / Not now), drawn from fenced ask / routine JSON in the text.
//
// Run AFTER agents-projects.js in the same uiharness invocation: that file mocks
// the commands, switches Chat -> Agents, exercises the cards once (which answers
// the question) and leaves the Agents home open. This file resets the mocked
// thread to unanswered, opens Herald's "Morning setup" thread and leaves it for
// --shot.
//
//   node tools/uiharness/run.mjs --wait 900 \
//     --eval-file tools/verify/agents-projects.js \
//     --eval-file tools/verify/agents-ask.js \
//     --shot tools/verify/out/agents-ask.png
(async () => {
  const A = window.__AGENTS;
  if (!A) return { error: "agents-projects.js must run first in the same uiharness run" };
  A.resetAsk();
  const opened = await A.openBot("Herald");
  return { opened, state: A.state() };
})()
