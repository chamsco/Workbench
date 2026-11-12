# ThinkingTrace

What the agent is doing right now, as a rail of short steps. It replaces a spinner on any turn that takes longer than a couple of seconds, and collapses to "Worked for 7s" when done.

- Header: spinner, "Thinking", live timer in `fg-3`. Under it an optional "Searched" row with source chips (28px, `inset`, a 16px favicon tile, "Name – title") wrapping to two lines and a "+N more" chip.
- Steps: a 1px `line-strong` rail with 5px `fg-4` dots; the current step is a 7px `accent` square and its label shimmers once per second. Finished steps stay in `fg-3`. One line per step, present tense, with the object ("Reading 7 sources about the trip").
- Show at most 6 steps; older ones fold into "N earlier steps".
- Replaces the `StepLedger` while a turn is live; the ledger is what remains afterwards.
