# AgentBoard

The fleet view of one thread's agents: who is running, who is waiting for Charles, who failed, and what it costs. It answers "what needs me" before "what is happening".

- Layout: sidebar (thread list by state), a Shell holding one Panel. The Panel is two columns: the agent grid with a summary line on the left, a 300px inbox and usage column on the right.
- Summary line: four big numbers (running, needs you, cost, tokens) at 22/26, unit words in `fg-3`. Numbers use tabular figures.
- Header filter: `SegmentedSwitch` with a count in each label (All, Running, Needs you, Failed). Needs you is waiting plus failed.
- Agent card: `inset` fill, `radius-inset`, 12px padding. Top: the flat figure avatar in the agent's colour, name, role in `fg-3`, a model `Tag`. Then a `StatusLabel`, one task sentence, a progress band and a foot with tokens, cost and the one action.
- Progress band: `panel` fill; a running agent fills `accent-soft` to its percentage from the left; waiting and queued agents get a hairline hatch and a live wait timer, so state reads without colour.
- A card that needs Charles takes the `accent-soft` fill and a primary Review button. Only that card and the inbox use accent.
- Inbox: `ApprovalCard`s in short form with Deny and Approve; the one about to expire shows a `warn` timer.
- Usage: total, a 7-day bar strip in grey (today darker), and one meter per subscription; a meter turns `warn` above 70%.
- Built on Tuff UI's Agent Console layout idea (MIT). The inbox, hatch-for-waiting and per-subscription meters are Cauldron's own.
