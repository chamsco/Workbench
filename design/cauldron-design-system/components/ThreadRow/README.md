# ThreadRow

One thread in the sidebar: a lead mark, the title, and one trailing fact.

- One line: `size-control` (28) high. Two lines: `size-row-2` (44), second line `meta` `fg-3` with stage · branch · PR.
- Lead mark (14px slot): `fg-4` dot idle, spinner running, `accent` dot needs you, `bad` dot failed. Only needs-you and failed use colour.
- Unread: title in `body-strong`. Selected: `selected` fill. Hover: `hover` fill. `radius-control`.
- Trailing fact: time since last activity, a live timer while running, or the word for needs-you/failed. Tabular numerals.
- Group rows under a `caption` repository label. Hide no state behind hover; reveal only actions (archive, pin) on hover.
