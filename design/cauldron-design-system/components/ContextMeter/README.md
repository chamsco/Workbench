# ContextMeter

How full an agent's context window is, split by what fills it.

- Bar: 6px, parts in `fg`, `fg-2`, `fg-3`, `fg-4` (a lightness ramp, not hues), 2px gaps, free space as an `inset` well with a `line` ring.
- Header: "Context" in `body-strong`, right "118k of 200k · 59%" in `fg-2`. At 80% the percentage turns `warn`; at 95% `bad` with the word "Full".
- Legend: one row per part, label in `fg-2`, value right-aligned in `fg-3`. Hovering a legend row raises its part to `fg` and the others to `fg-4`.
- Compaction: the bar shimmers once, then redraws; the header says "Compacted 52k".
