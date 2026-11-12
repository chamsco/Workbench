# Surface

The three layers every card is made of: a Shell (tray), a Panel (content) and an Inset (well). This is the structure Tuff UI's Surface uses, on Cauldron's radii.

- Shell: `card` fill, `radius-window` (10), 4px padding. Header and footer sit in the shell's padding, outside the panel: an optional 28px icon tile, a `body-strong` title, a `meta` subtitle, an aside on the right.
- Panel: `panel` fill, `radius-card` (6), 12px padding, a 1px `line` ring. In light it is whiter than the shell, in dark it is darker (recessed). Radii nest: window 10 = card 6 + 4px padding.
- Inset: `inset` fill (foreground at 4-5%), `radius-control`, `mono` 12px. Code, payloads, details.
- Depth comes from these tone steps. No drop shadow on a shell or a panel; only overlays and the composer lift.
- Do not put a panel inside a panel; go to an inset instead.
