# PlanVsActual

A run's plan against what happened, one row per step on a shared time axis, so slippage is visible before it is a problem.

- Header: scale icon, "Plan vs actual", the task in `fg-3`, then time and cost as `actual / plan` in mono, and one status `Tag`: "+17s behind" in `warn`, or "On plan" in `fg-2`. Never green for on plan.
- Rows 32px. Left: step state (check, spinner, hollow dot) and name; future steps in `fg-3`. Right: a hatched outlined bar for the plan, a solid `accent` bar for actual, and the part beyond the plan in `warn`. After the bar: duration in mono and a delta tag (`diff-add` background for faster, `warn-soft` for slower).
- A 1px `accent` line marks now; the axis has mono ticks every 30s (scale to the run's length).
- Footer legend and an ETA tag. The legend uses the same fills as the bars.
- Props for generation: `title`, `steps[{name,state,plan:{start,dur},actual?:{start,dur}}]`, `now`, `budget:{time,cost}`.
