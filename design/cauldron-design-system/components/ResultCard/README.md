# ResultCard

What an agent finished, as facts and a short file list, ending with at most two buttons.

- Card: `panel`, `radius-panel` (16), 1px `line` inset ring, 12px padding. No gradient, no hero number.
- Header: title in `body-strong` that says what changed, `StatusLabel` on the right.
- Facts line in `meta`: files, `+N` in `ok` and `−N` in `bad`, duration, cost. Units once, tabular numerals.
- File list: 28px rows split by `line` hairlines, path in `mono` (middle-truncated), counts on the right. Over 6 files: show 5 and "and 9 more".
- Foot: next steps as plain underlined links on the left, then secondary and primary `Button sm` on the right.
