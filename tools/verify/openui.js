// tools/verify/openui.js
//
// Acceptance for the OpenUI work: the two blocks that matter are drawn through
// the real chat face (WhirlChat.open -> ThreadView -> RichText).
//   1. the shape that failed — named arguments + invented form parts — must now
//      say WHY, in the notice, instead of "the renderer produced nothing";
//   2. a form written the way the language actually works must draw.
(async () => {
  const H = window.__H;
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const route = { kind: "cli", provider: "claude", model: null };
  const msg = (id, text) => ({ id, role: "assistant", text, at: Date.now(), status: "done", error: null, reply_to: null, reactions: [], attachments: [], model: "claude", cost_usd: 0 });

  const BAD = [
    "Here is how I would break it down.",
    "",
    "```openui",
    'CardForm = Form(onSubmit=tasks_submit, children=[FormInput("Task title (one line)"), FormTextarea("What makes you avoid it?", id=avoids_why)])',
    'root = Card(children=[CardHeader("Untangle that task", subtitle="Pick one thing"), CardForm])',
    "```",
    "",
    "Fill this in and hit Break it down.",
  ].join("\n");

  const GOOD = [
    "Fill this in:",
    "",
    "```openui",
    'title = Input("title", "Task title (one line)", "text")',
    'avoids = TextArea("avoids_why", "What makes you avoid it?", 4)',
    'f1 = FormControl("Task title (one line)", title)',
    'f2 = FormControl("What makes you avoid it?", avoids)',
    'submit = Button("Break it down", Action([@ToAssistant("Help me break down this task")]))',
    'taskForm = Form("taskform", submit, [f1, f2])',
    'root = Card([CardHeader("Untangle that task", "Pick one thing you have been putting off"), taskForm])',
    "```",
  ].join("\n");

  H.mock("chat_list", () => ([
    { id: "t1", title: "Untangle", updated: Date.now(), pinned: false, route, preview: "…", busy: false, unread: false },
  ]));
  H.mock("agents_list", () => []);
  H.mock("chat_thread", (a) => ({
    id: a.id, title: "Untangle", created: 1, updated: 1, pinned: false, route,
    messages: a.id === "t1" ? [msg("m1", BAD)] : [msg("m1", GOOD)],
    life: "keep", project: null, app: null, system: null, agent: null, members: [], goal: null,
  }));

  const read = (label) => {
    const failed = [...document.querySelectorAll(".openui-failed")].map((e) => e.textContent.replace(/\s+/g, " ").trim());
    const blocks = [...document.querySelectorAll(".openui-block")];
    const fields = document.querySelectorAll(".openui-block input, .openui-block textarea, .openui-block select, .openui-block [contenteditable]").length;
    const buttons = [...document.querySelectorAll(".openui-block button")].map((b) => b.textContent.trim()).filter(Boolean);
    const tags = {};
    for (const el of document.querySelectorAll(".openui-block *")) tags[el.tagName.toLowerCase()] = (tags[el.tagName.toLowerCase()] || 0) + 1;
    return {
      label,
      failed,
      blockChildCounts: blocks.map((e) => e.childElementCount),
      fields,
      buttons,
      tags,
      text: blocks.map((e) => e.innerText.replace(/\s+/g, " ").trim().slice(0, 200)),
    };
  };

  window.WhirlChat.open("t1");
  await settle(900);
  const bad = read("named args + invented parts");

  window.WhirlChat.open("t2");
  await settle(900);
  const good = read("positional, real form parts");

  return { bad, good };
})()
