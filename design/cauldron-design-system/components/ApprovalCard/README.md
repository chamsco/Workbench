# ApprovalCard

An agent asking before it does something it may not do alone. It is the only card in the thread with an `accent` mark.

- Card: `card` fill, `radius-panel` (16), 1px `line-strong` inset ring, 12px padding.
- Top: `StatusLabel` needs-you and the asking agent as a `Tag`. Title in `body-strong` that names the action in plain words. One line of why, in `fg-2`, with its real consequence.
- Details: a key/value well (`inset`, `mono` values) with the exact command and what it touches.
- Foot: an optional "Always allow ..." toggle scoped to this thread, then Deny (secondary, Esc) and Approve (primary, Ctrl ↵).
- After a decision the card collapses over `dur-panel` to one `meta` line: glyph, who decided, time, the command.
- If the request expires, show "Expires in 0:42" as a `warn` StatusLabel in the top row for the last 10 seconds only.
