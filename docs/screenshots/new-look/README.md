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
| Groups | Needs you, Running, Done | Needs you (empty, NL2d), Unread, Running, Failed, Done | core has no Needs you per thread yet; Unread sits where it does |
| Age column | none | none (removed) | |

- Superseded by NL3a (13 Nov): no row shows a figure now. Figures only for agentic threads (Charles, 12 Nov): a thread with an agent or members shows its agent in the slot, in the row's mood (Running is Working, the cog; Failed is Error; else Idle). A normal thread shows a 6px state dot there: accent Running, bad Failed, fg-3 otherwise. The board draws a figure on every row; this follows Charles over the board.
- At 22px the avatar draws no marks, so an Error figure looks like Idle; the Failed head says it. Figures are still until the motion clock drives them, and every figure is agent 1's colour for now.
- Needs you and the usage line: superseded by NL2d (Charles chose empty states, 12 Nov).
- A selected row is `hover` on the window, as the board's `.th.sel`.
- Runs: `docs/cauldron/NL2-sidebar-red.txt` (group heads 20 high, meta showed the last message), `NL2-rows-red.txt` (figures on normal rows), `NL2-sidebar-green.txt`.

## NL2d: floating tabs (option A) and empty states

Charles retired the folder tabs (12 Nov); the coordinator's answer on placement was option A (tabs in the top bar, the card right under it, no tab on a new thread, the crumbs in the card's head). Charles can still ask for B. On 13 Nov Charles asked for the tabs on the design-system scale rather than the board's raw `.ftab`; the rules came through the coordinator.

`nl2-11-tabs-agentic-dark.png` / `nl2-14-...-light.png`: an agentic thread (`CAULDRON_FIXTURE_AGENTS=3`, the 1:1 fixture open). `nl2-12-tabs-normal-dark.png` / `nl2-15-...-light.png`: a normal thread from the sample data, no figure. `nl2-13-new-no-tabs-dark.png` / `nl2-16-...-light.png`: New thread, no tabs. Light shots come from a build forced to light, then reverted.
`nl2-17-tabs-vs-board.png`: the board's bar and card head (dark, its terminal frame, which has the same bar) over the app's two threads. There is no light render of the board on hand, so the light shots stand alone.

| | Board `.ftab` | App, measured at 1200x720 | Why |
|---|---|---|---|
| Bar | 44 | 44 (was 40 from top bar A; moved to the board's 44 on 13 Nov so the bar and the card head line up) | |
| Tab | y 7, h 30, centred | y 8 to 38, h 30, centred (the window's 1px border adds 1 to every y) | |
| Row x | card + 6 (254) | card's left edge (257) | rule 8; clamped past the logo and name when the sidebar is closed (the board has no such frame) |
| Radius | 7 | 8 (`radius::INNER`) | there is no 7 |
| Text | 12.5px 400 | 12px / 16px, 500 | label style |
| Open tab | card fill + shadow | `selected` fill (9%), no ring or shadow | tabs take no shadow |
| Padding, gaps | 0 10 0 9, gap 7, tabs 4 | 0 8, figure to name 6, tabs 4 | 4px scale |
| Figure | 15x18 on every tab | 14px tall, only on an agentic thread's tabs; none since NL3a | Charles, 12 Nov; 13 Nov |
| Close | fg-4 | fg-3, on hover for closed tabs | fg-4 fails AA |
| + | 28 | 28 | |
| Card | y 44, right under the bar | y 45, right under the bar (no gap) | |
| Sidebar first row (apps) | y 48 | y 49 | the apps row then 16 to New thread, the board's 15 on the 4px scale |
| Card head | 44, title at card + 14, "repo /" fg-3 400 | the same; worktree and branch not drawn | core has no worktree or branch for a thread |

- Agent tabs size to their names within an equal share of the row (72 to 200); past that the row scrolls with arrows, as before.
- Empty states (Charles, 12 Nov): a "Needs you" head with no rows and no count above the groups (hidden while searching), and "No data yet" under the name in the user row. Neither invents a number.
- The thread view's own title row and chips are gone; the card head carries the title.
- Runs: `docs/cauldron/NL2-bar44-red.txt` / `NL2-bar44-green.txt` (the 44px bar, panes under it); `docs/cauldron/NL2-tabs-red.txt` (tabs still under the bar, no `selected` state, no card head, no empty states), `NL2-tabs-green.txt`. The 13 Nov sizes (14px figure, row at the card's edge) changed the tests and the code together; there is no separate red run for them.

## NL3: the v3 board in the app

Spec: `design/cauldron-new-look-v3.html` (the v2 board on the design system, Charles approved 13 Nov). `nl3-01-new-thread-dark.png` / `nl3-04-...-light.png`: New thread with the stage bar. `nl3-02-thread-drawer-dark.png` / `nl3-05-...-light.png`: a thread from the sample data, drawer open. `nl3-03-tabs-dark.png` / `nl3-06-...-light.png`: the agent-tab fixture (`CAULDRON_FIXTURE_AGENTS=3`). `nl3-07-app-vs-board.png`: the board (left) against the app (right), new thread and thread. Light shots come from a build forced to light, then reverted.

| | v3 board | App | Note |
|---|---|---|---|
| Tab | 24 tall, 12/16 500, padding 0 8, radius 8 | the same; + and the scroll arrows 24 too | control-sm; the coordinator asked for 26, which is not a size token |
| Figures in a thread | none: rows, tabs, status line, drawer | none: rows show the state dot, tabs the name, the status line a 6px dot | Charles, 13 Nov: figures belong to the Agents page |
| Drawer | on the frame, the panel strip on top, no rail beside it | 236 wide (the sidebar's width), strip of five 28px buttons, rail only while folded | the strip's open button folds it to the rail |
| New thread bottom bar | on the frame under the card: usage, then Plan, Execute, Ship, Commit | 32 high, card-wide, 11/14 500 fg3 | static: core has no stages or usage, so "No usage data yet" and no current stage |

Limits:
- The bottom bar is on the frame, as the board draws it and as the drawer sits; the coordinator read "the same grey" as the card token, the board does not.
- The drawer panels' bodies are still UI6 placeholders. The board's five panels (Highlights, Patches, Git, Agent trace, Dev) differ from the app's five (Highlights, Usage, Koi preview, Code review, Patches); the app keeps its own until UI6.
- The thread view keeps its stage line inside the card; only New thread got the bar (Charles asked for it there).
- Split panes are unchanged; a proposal board comes first.
- Runs: `docs/cauldron/NL3a-red.txt` / `NL3a-green.txt` (24px tabs, no figures), `NL3b-red.txt` / `NL3b-green.txt` (the drawer takes the rail's place, strip on top), `NL3c-red.txt` / `NL3c-green.txt` (the stage bar). The 24px arrows changed a clamp test after the code; there is no separate red run for that.

