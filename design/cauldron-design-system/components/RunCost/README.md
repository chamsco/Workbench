# RunCost

Where a run's money and time went: one row per model call, sub-agent or tool, in run order.

- Header line: "Run cost", the task in `fg-3`, total cost and tokens on the right.
- Rows 28px, split by `line`. Columns: step (sub-agent children indent 16px), kind word in `fg-3`, a 4px share bar in `fg-4` (the largest in `fg-2`), cost and time right-aligned with tabular numerals.
- Failed rows say "failed" in `bad` in the time column; running rows show a live timer.
- Costs keep two decimals above $0.10 and three below ("$0.042"). Below 520px wide the Kind and Share columns hide.
