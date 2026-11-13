# ContextMap

What fills an agent's context window, as areas proportional to tokens, with the free space hatched. It answers "why is this agent running out" faster than a bar.

- A 6-row, 8-column treemap in `radius-inset`; tiles `radius-inner`, 4px gaps. Filled tiles use a lightness ramp (`fg-3`, `fg-2`, `fg`) with `bg`-coloured text, so kinds are told apart by lightness and label, not hue; the largest user of context may take `accent`.
- Each tile: name in 500 and mono token count, with the kind after a dot ("14.8k · Tools"). Tiles too small for text show it on hover.
- Free space is a dashed-outline hatched tile. At 80% full the percentage turns `warn`; after compaction a before/after pair of maps is shown.
- Header: "Context", the agent and window size as a `Tag`, used / total in mono and the percentage.
- Props for generation: `window`, `parts[{name,kind,tokens}]`, `compactedFrom?`.
- This replaces `ContextMeter` in the right drawer when there is room; the meter stays for narrow places.
