# Toggle

An on/off setting that applies immediately, always in a settings row with a label and a one-line consequence.

- Track 28×16, `radius-round`. Off: 1.5px `line-control` ring and knob (3:1 on every surface). On: `accent` fill, `on-accent` knob. Knob moves over `dur-swap`.
- The row: label in `body`, consequence in `meta` `fg-3` that states what will happen with real values ("Threads idle for 2 days..."). Rows are separated by a `line` hairline.
- If the change needs confirming, use a form with Apply that stays disabled until something changed, not a toggle.
