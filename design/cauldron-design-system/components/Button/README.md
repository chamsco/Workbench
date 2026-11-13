# Button

Use a button for an action that happens now; one `primary` per view, everything else `secondary` (default) or `quiet`.

**Variants**
- `primary`: `accent` fill, `on-accent` text. The one thing this view wants: Approve, Send, Create PR. Two primaries on one screen means one of them is wrong.
- `secondary` (default): `hover` fill, `fg` text. Deny, Open diff, Cancel.
- `quiet`: no fill, `fg-2`. Toolbar actions and actions revealed on hover (Rewind on a checkpoint).
- `danger`: `bad-soft` fill, `bad` text, always with a verb that names what is lost ("Delete thread", never "Confirm").

**Sizes** `size-control` (28) default, `size-control-sm` (24) inside cards and rows. Radius `radius-control`. Label is `body` at 500.

**States** hover darkens one step (`selected`, `accent-hover`); press scales to .98 for `dur-swap`; focus is the 2px `focus` ring at 2px offset; disabled is `hover` fill with `fg-4` text and stays in place (an Apply that waits for a change); loading swaps the leading icon for the spinner, keeps the label and stays focusable.

**Keyboard hints** A `Kbd` inside the button shows the shortcut (Ctrl ↵ on Windows, ⌘↵ on macOS). Show it only on the buttons of the focused card.

**Do not** use pills, gradients, glows or a coloured border on a button.
