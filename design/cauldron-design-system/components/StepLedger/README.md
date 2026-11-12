# StepLedger

The agent's progress as plain lines: a thin check and grey text per finished step, one live line for the current step.

- Line: `body` in `fg-3`, 14px check at stroke 1.5, no circle, no fill. 4px between lines.
- Current step: spinner and the label in `fg-2` with a slow shimmer (off under reduced motion).
- Facts go in the line ("11 passed", "4 files"), not in a badge.
- A finished group collapses to its first line ("Worked for 3m 12s") with a chevron; tool calls inside open as `ToolCall` rows.
