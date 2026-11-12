# New look

The app moves to the design-system tokens (design/cauldron-design-system/tokens.json at 45d2432 on
`claude/project-thread-92mzld`) and the v2 board's layout (design/cauldron-new-look.html at 2096c8f on
`claude/project-thread-gg3ogp`), gate by gate. Shots are the real window at 1200x720 with the
sample data folder.

## NL1: tokens

`nl1-01-new-thread.png`, `nl1-02-thread.png` (dark), `nl1-03-thread-light.png` (light; the theme follows the OS, so that shot came from a build forced to light, reverted and not committed): the current layout on the new tokens. The layout changes from NL2 on;
`board-frame1.png` is the board's frame 1 for comparison.

- Palette: Tuff surfaces (bg #111111, card #1d1d1d, panel #151515, raised #282828 in dark), purple accent
  #a08cff dark / #6236d6 light. `inset`, `hover`, `selected`, `line` and `line-strong` are overlays (`Rgba`)
  that GPUI composites over whatever is beneath. `run`, `info`, `design` and `mem` are gone (the Code and project chips and the
  code glyphs are fg-2; your message bubble is `raised`).
- Radii: SHELL 22, PANEL 16, INSET 12, CONTROL 10, INNER 8, TAG 6, replacing 6/4/3. For now the old uses map
  R to PANEL, R1 to CONTROL and R2 to INNER; each screen takes its own radii as it is ported.
- Type: Geist and Geist Mono are bundled and registered on every OS. The shell sets ss01 and tnum (tabular figures) on its root. The bundled TTFs have no cv11 (GSUB checked with fontTools), so that set is not on.
- Found by the tests: light fg-3 on `selected` over a card is 4.37:1, so selected fills go on bg or a panel, never on a card. Nothing uses `selected` yet; selected rows still use `raised` until NL2 moves the sidebar onto bg.
- Not changed: motion stays Cauldron's (200ms cap); the design system's 240 and 300ms durations break that rule. The
  shadows keep their dark values in both themes, and the 1px rings from `shadow-menu`/`shadow-tip` are not added yet.
- Runs: `docs/cauldron/NL1-tokens-red.txt` (the new tests against the old tokens: no `Rgba`, no new radii), `NL1-tokens-green.txt`.

## NL2a: the new thread box

`nl2-01-new-thread.png` against `board-frame1.png`: the board's centred column, the machine above a box,
and in the box the editor, Add a file, effort and a round Start. The brand, the Code/Design tabs and the
cards under the box are gone.

| | Board | App | Why |
|---|---|---|---|
| Box height (one line) | 84 | 84 | |
| Box width | 780 | 628 | the card is 676 wide here, 944 on the board: the drawer is open here and not on the board (see NL2b) |
| Ctx row above the box | 34 | 34 | |
| Box radius / dd radius | 10 / 6 | 10 / 6 | CONTROL, TAG |
| Start | 28 round, raised | 28 round, raised (accent once there is text) | |
| Placeholder | "Ask the coordinator, @ an agent..." | "Describe a task, a bug or an idea..." | there is no coordinator in core yet |
| Trust, Coordinator, mic, Worktree/Isolated, usage bars, stage strip | shown | not drawn | core has none of them; no fakes |
| Card header and close | shown | not drawn | still to do: NL3, with the thread view |

- The editor starts at one row (was two) so the box is the board's 84 with no text.
- Runs: `docs/cauldron/NL2-newthread-red.txt` (the centred-box test fails on the old screen), `NL2-newthread-green.txt`.
- `nl2-02-machine-menu.png`: the machine menu opens under the machine and draws over the box (deferred, since the box now comes after it). `nl2-03-cant-reach.png`: the error state, a sample machine that is not running; the line sits under the box and the column re-centres around it.

## NL2b: top bar A merged

Merged `claude/project-thread-gg3ogp` (top bar A, 81698a0, and its motion clock) as MERGE-NOTES says: the + menu keeps this branch's pop-in, gg3ogp's `menu_since`, `menu_shown` and the `k` argument are gone, every other motion field stays. The tab flare is `radius::CONTROL` (10, the board's).
`nl2-04-top-bar-new-thread.png`, `nl2-05-top-bar-thread.png`, `nl2-06-top-bar-plus-menu.png`: one bar across the window (sidebar button, logo and name, breadcrumbs, Terminal, Split, window controls); the sidebar, card and rail start under it. The notch is kept for later: see `docs/cauldron/NOTCH.md`.
Width: the board's frame 1 has no open drawer, so its card is 944 wide; here Highlights is open and the card is 676. The new thread test holds the box at 780, or the card's inside less 48 when narrower.

## NL2c: the sidebar

`nl2-07-sidebar.png` (made-up threads, `CAULDRON_FIXTURE_THREADS=list`) against `board-frame1.png`;
`nl2-08-sidebar-states.png`: list, loading, empty and error, left to right. `nl2-09-sidebar-light.png`: light (a build forced to light, reverted).
`nl2-10-sidebar-folded.png`: the sidebar folded to the rail; the box then reaches the board's 780. "Opening your threads" under the box is the fixture mode (no core), not part of the screen.

| | Board | App | Why |
|---|---|---|---|
| Sidebar | on the window, no card | on the window, no card | |
| Content left / right (apps row, rows) | 10 / 240 | 13 / 240 | the layout keeps the card era's 12px margin |
| New thread, Search | 32px rows, Ctrl N / Ctrl K hints | 32px rows, no hints | the app has no Ctrl N or Ctrl K binding yet |
| Group head | 36 (12px, fg-3, count right) | 36 | |
| Row | 30, radius 6, slot 18x22, title 13px, repo 11px fg-4 max 54, 6px ping | the same, repo in fg-3 | fg-4 is 2.9:1 on the window, under AA for text |
| Figure drawn | 15x20, 12.6px into the row | 15x20, 13px in | |
| Groups | Needs you, Running, Done | Unread, Running, Failed, Done | core has no Needs you per thread yet; Unread sits where it does |
| Age column | none | none (removed) | |

- Figures only for agentic threads (Charles, 12 Nov): a thread with an agent or members shows its agent in the slot, in the row's mood (Running is Working, the cog; Failed is Error; else Idle). A normal thread shows a 6px state dot there: accent Running, bad Failed, fg-3 otherwise. The board draws a figure on every row; this follows Charles over the board.
- At 22px the avatar draws no marks, so an Error figure looks like Idle; the Failed head says it. Figures are still until the motion clock drives them, and every figure is agent 1's colour for now.
- Needs you and the usage line in the user row are not drawn: core has neither, and the brief says draw nothing there.
- A selected row is `hover` on the window, as the board's `.th.sel`.
- Runs: `docs/cauldron/NL2-sidebar-red.txt` (group heads 20 high, meta showed the last message), `NL2-rows-red.txt` (figures on normal rows), `NL2-sidebar-green.txt`.
