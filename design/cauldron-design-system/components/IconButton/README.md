# IconButton

A square button holding one Lucide icon, for chrome actions whose meaning the icon carries (sidebar, split, terminal, copy).

- 28×28 (`size-control`) or 24×24 (`size-control-sm`); icon `size-icon` at stroke 1.75; `radius-control`.
- Rest: no fill, `fg-2`. Hover: `hover` fill, `fg`. On (a toggled panel is open): `selected` fill.
- Every icon button needs an accessible name and a `Tooltip` with its shortcut.
- `send` is the only round icon button (`radius-round`): `accent` when there is text to send, `hover` with `fg-4` when empty. It swaps to Stop (square icon) while the agent runs, over `dur-swap`.
