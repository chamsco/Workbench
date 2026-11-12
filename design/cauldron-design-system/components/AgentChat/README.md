# AgentChat

One agent's conversation inside a thread, with its work and its changes beside it.

- Header: thread title, branch `Tag`, live `StatusLabel`, split. Under it the `AgentTabs` row: coordinator first, then one tab per agent; the selected tab sits on `inset` and joins the thread.
- Messages: a centred date chip; the user's message right-aligned in an `inset` bubble with name and time under it; each agent turn is the 28px figure, the name, a model `Tag`, then a body column.
- Body order: `StepLedger`, a grouped `ToolCall` list ("Used 2 tools · 1.6s"), the agent's prose in `reading` (14/22, 66ch), and, when it needs a decision, an `ApprovalCard`.
- Foot: a mode switch (Ask, Plan, Act) with one line saying what the mode allows, then the composer: an `inset` shell holding a `panel` input, context tags (files attached), token use and the send button.
- Right column (340px, hairline on its left): Changes, Terminal, Preview as a switch; a checkpoint line ("Checkpoint 4 of 4"); the file list with +/- counts; a diff with Keep and Revert; the context meter.
- Pattern: Tuff UI's Coding Agent screen (MIT) is the base. The checkpoint line, the single approval in the thread and the ledger-first turn order follow Cauldron's chat rules.
