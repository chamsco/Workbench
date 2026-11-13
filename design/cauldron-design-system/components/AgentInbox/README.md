# AgentInbox

Every agent's work as three columns: Needs you, Running, Done. It is the board for people who start many tasks and come back later.

- Columns are `inset` with `radius-panel`; the header is the column name in `caption` and a count tag. Cards are `panel` with `radius-inset` and a 1px `line` ring.
- Card: title in `body-strong`, a two-line summary in `fg-2`, then a foot with the `StatusLabel` and two mono counters (searches, tool calls) at the right.
- Needs you holds waiting and failed together; a waiting card shows how long it has waited in `warn`. Done holds the last 24 hours.
- Dragging is not offered: state is the agent's, not the person's. Click opens the thread.
- Board or List via a `SegmentedSwitch`; the list is `ThreadRow`s grouped the same way.
- Props for generation: `items[{id,title,summary,status,elapsed,counts:{searches,tools}}]`.
