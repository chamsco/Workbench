# Menu

A short list of actions on one object, opened from a button or a right click. Also covers `Tooltip`.

- Menu: 240px wide, `raised` fill, `radius-card`, 4px padding, `shadow-menu` (its ring is the border). Items 28px, `radius-control`, 16px icon in `fg-2`, label in `body`, shortcut `Kbd` on the right.
- Hover and keyboard focus: `hover` fill. Separator: 1px `line` across the full width. Destructive items last, in `bad`, after a separator.
- Opens over `dur-menu` (scale .96 to 1 and fade, items staggered 25ms), closes over `dur-exit` without stagger.
- Tooltip: `raised`, `radius-control`, `shadow-tip`, `meta` text, 4×8 padding, the shortcut as a `Kbd`. Appears after 500ms hover, instantly when moving between tooltipped controls.
