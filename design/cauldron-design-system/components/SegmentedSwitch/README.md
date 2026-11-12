# SegmentedSwitch

Two to four mutually exclusive views of the same thing, with no panels of their own (Code or Design, Day/Week/Month).

- Track: `inset`, `radius-control`, 2px padding. Segments: 24 high, `radius-inner`, `label` text.
- Selected segment: `card` fill, `fg`, `shadow-tip`. Others: `fg-3`, no fill.
- The selected fill slides between segments over `dur-panel` with `ease-settle`.
- Use `aria-pressed` on each segment. For more than four options use a menu.
