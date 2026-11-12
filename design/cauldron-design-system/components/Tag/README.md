# Tag

A mono label for an identifier: model, branch, file, tool, transport. Also covers `Kbd` for shortcuts.

- Tag: `size-tag` (18) high, `radius-inner`, `inset` fill, `mono-tag` text in `fg-2`, optional 12px icon. Truncate in the middle for paths (keep the file name), at the end for everything else; max 220px.
- Kbd: same height, no fill, 1px `line-strong` ring, `fg-3`. Write keys as the OS does: "Ctrl K" on Windows, "⌘K" on macOS.
- Tags are not buttons and not status. If it can be clicked, it is a `Button sm`.
