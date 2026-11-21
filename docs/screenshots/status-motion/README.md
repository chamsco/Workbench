# Status marks (option C of the status-motion board)

Plain streams, familiars and the chat status line carry a 5x5 dot mark instead of the old state dot.
Agents keep their head; a familiar row adds the same mark beside it. Stepped at 8 fps; reduced motion shows a still frame.

- `rows-before-after-dark-3x.png`, `rows-before-after-light-3x.png`: sidebar rows, before (left) and after (right), enlarged 3x.
- `before-*.png`, `after-*.png`: full window, 1200x720.

Mapping: Working = rising bars (accent), Needs you = question glyph (amber), Failed = cross (red), Done = one faint dot.
Thinking and Waiting exist in `crates/chrome/src/status.rs`; the chat status line uses Waiting ("Waiting for the reply") and Working ("Writing").

The newest bubble of yours in an open stream also carries the mark, inside it at the right (Charles, 21 Nov):
waiting for the reply, working while it streams, a cross if it failed, the faint dot once done. It is covered by
`a_row_opens_its_thread`; no screenshot, because the screenshot fixtures have no messages to open.
