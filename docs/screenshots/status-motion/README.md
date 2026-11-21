# Status marks (option C of the status-motion board)

Plain streams, familiars and the chat status line carry a 5x5 dot mark instead of the old state dot.
Agents keep their head; a familiar row adds the same mark beside it. Stepped at 8 fps; reduced motion shows a still frame.

- `rows-before-after-dark-3x.png`, `rows-before-after-light-3x.png`: sidebar rows, before (left) and after (right), enlarged 3x.
- `before-*.png`, `after-*.png`: full window, 1200x720.

Mapping: Working = rising bars (accent), Needs you = question glyph (amber), Failed = cross (red), Done = one faint dot.
Thinking and Waiting exist in `crates/chrome/src/status.rs`; the chat status line uses Waiting ("Waiting for the reply") and Working ("Writing").

The stream's tab at top left and each side-stream tab (the corner-arrow ones) carry the same mark inside them, left of the title,
showing that stream's state as its sidebar row does. Covered by `a_row_opens_its_thread`; no screenshot, because the
screenshot fixtures have no open stream with messages. Agent tabs keep their head and have no mark yet.
