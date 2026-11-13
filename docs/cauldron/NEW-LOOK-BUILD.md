# New look: build report (Mac, 13 Nov 2026)

The Windows PC went offline in the middle of the v3 build, so this run continues it on Charles's Mac.
Branch `claude/project-thread-sa3a1p`, started from 38c24b7 (the Windows tab and figure work and the v3 updates).
Reference: `design/cauldron-new-look-v3.html` and the v3 artifact (M5XJsDxGmSQY8PtDrLSnWD).
Nothing is merged, top bar A is unchanged, and the split panes are unchanged (a prototype board with two directions comes first).

## Starting point

- Head 38c24b7. The app suite (`cargo test -p backspace`) passed 39 of 39 on the Mac before any change. The brief expected 38; 38c24b7 added `the_drawer_switches_panels_from_its_own_strip`, which accounts for the extra test.
- From 38c24b7: 24px tabs, no figures in threads, the drawer with its panel strip, and the new thread's bottom bar. The drawer and the bar both sat on the window frame.

## Gate 1: tabs

No code change; checked on the Mac render (`nl3d-thread-*.png`, `nl3d-tabs-*.png`).

| | Wanted | Measured at 1200x720 |
|---|---|---|
| Tab height | 24 | 24 (y 11 to 35) |
| Centred in the 44 bar | yes | yes: the bar is y 1 to 45, with 10px above the tab and 10 below |
| Agent figures on a normal thread | none | none: the sidebar rows show state dots and the tabs show names only |
| Agent tab fixture (3 agents) | names only | names only |

## Gate 2: the drawer inside the grey Shell gutter

The brief asks for "the right drawer inside the grey shell gutter" and the bottom bar "in the SHELL grey (the same grey as the right gutter), not on the window frame". In `tokens.rs` the Shell is the `card` surface ("a Shell (`card`) holds a Panel"), and radius SHELL 22 is PANEL 16 plus SHELL_PAD 6. I built it that way:

- `layout::Shell` has a new `shell` rect. It starts right under the bar and runs from 8px after the sidebar (or rail) to the 12px side margin. The card, Split and the drawer (or its rail) sit inside it, 6px (SHELL_PAD) in from each edge.
- The view draws the Shell in `card` grey with radius 22, behind everything else. The drawer and the rail have no surface of their own, so they sit on that grey in the Shell's right gutter. The strip of five panel buttons stays on top of the drawer.
- The main card is a Panel in the Shell: the `panel` token (#151515 in dark, darker than the Shell's #1d1d1d; white in light on the Shell's #eeeef0), with its 1px ring and radius 16, nesting in the 22 Shell (coordinator, 13 Nov 01:29Z).
- `motion::Frame` carries the Shell, so it moves with the sidebar like the card does.

At 1200x720 (Mac, sidebar and drawer open): the Shell is x 257 to 1187 and y 45 to 707. The card is x 263 to 939 and y 51 to 701. The drawer is x 945 to 1181, 236 wide.
Folded, the 44px rail stands in the same gutter (`nl3d-folded-*.png`).

## Gate 3: the bottom bar in the Shell grey

No new code beyond gate 2. The new thread's 32px bar sits under the card, inside the Shell's bottom gutter. Usage ("No usage data yet") is on the left; Plan, Execute, Ship and Commit are on the right in 11/14 500 fg3. The bar's bottom edge is 6px above the Shell's bottom edge, so it sits on the Shell grey, not on the frame (`nl3d-new-*.png`; the before shots are `nl3d-before-new-*.png`).
Core still has no stages and no per-subscription usage, so the bar is static: no current stage and no numbers.

## Tests

- Red: `docs/cauldron/NL3d-red.txt`. The new Shell geometry fails the two wireframe tests that still expected the drawer on the frame (the drawer ending at the side margin, and the panes starting right under the bar).
- Green: `docs/cauldron/NL3d-green.txt` (chrome wireframe, 6 pass, 1 ignored as before) and `NL3d-app-green.txt` (app, 39 of 39, rerun after the panel change).
- New assertions: every pane sits 6px inside the Shell. In the app, the `shell` element matches the solver in every state, the stage bar's bottom edge is 6px above the Shell's bottom edge, and the drawer ends 6px inside the Shell's right edge.
- `cargo test -p cauldron-chrome`: all suites pass.

## What I verified on the Mac, and what I did not

Verified:
- The build and both test suites on the Mac (macOS 27, Apple toolchain, cargo 1.99.1).
- The real app window on the display at 1200x720 (`CAULDRON_SIZE`), on sample data in a scratch folder rather than Charles's own threads.

Which shots are real window captures (13 Nov, after Charles granted Screen Recording; the first retry at 01:31Z still failed with `could not create image from display`, the next one worked):
- **Real captures** (`screencapture -l` of the live window, native traffic lights and window corners included): `nl3d-new-*`, `nl3d-thread-*`, `nl3d-folded-*`, dark and light, and the six pane shapes `nl4-panes-*` (below).
- **Headless Metal renders**: `nl3d-tabs-*-render.png` (the agent-tab fixture has no live start-up option) and `nl3d-before-new-*` (the state before the Shell).
- The real captures are 1200x720 at 1x. This Mac's display is 1920x1080 at scale 1, so a capture of the live window cannot be 2x. The headless renders are 2x.
- "Thread beside terminal" is not shot: the Terminal button in the top bar does nothing yet. The old Split shots (`nl3d-split-*`) are removed, because Split now opens the panes below.
- Hover states are not in the shots. I moved the pointer with a posted CGEvent, but no hover showed. Posting events most likely needs Accessibility permission for the Claude Code process, which it does not have.

Not verified:
- I did not drag or resize the live window, or click through the open and fold animations. Only the tests cover those.
- I did not check the narrow width or a Retina display.
- Light shots use `CAULDRON_THEME=light` rather than a system theme switch.

## Tools added (debug and feature-gated, not in a release build)

- `CAULDRON_THEME=light|dark` (debug builds) forces the theme.
- `CAULDRON_SIZE=1200x720` (debug builds) sets the window's starting size.
- `CAULDRON_OPEN=<thread id>`, `CAULDRON_DRAWER=folded` and `CAULDRON_PANES=<0-5>` (a shape from `Shape::ALL`) (debug builds) start the live window in a given state, for real captures.
- `cargo run -p backspace --features shots` with `CAULDRON_SHOTS=<dir> BACKSPACE_DATA=<sample data>` renders New thread, a thread, the drawer folded, and agent tabs in dark and light to PNG, then exits (`crates/app/src/shell/shots.rs`, macOS only). It runs on the main thread because AppKit requires it, so it cannot be a test.

## Note for Charles

The v3 board draws the drawer and the bottom bar on the window frame. This build follows the brief instead and puts both on the Shell grey (`card`), inside a rounded Shell that holds the card. The card is now the `panel` token, so it stands apart from the Shell in both themes: darker in dark, white in light.

## Split panes, direction B (13 Nov)

The board is `design/cauldron-split-panes.html` (published at https://claude.ai/artifact/EAWJMVG33f8ue1f8guXhf1). It keeps both directions, so Charles can still overrule B with A.
Charles, through the coordinator: "go for B, make a pane button that has an animation that shows the different options (add also 2 side by side on top & long vertical at the bottom)".

What is built:
- **Six shapes**, in the layout button's order: one pane, two side by side, two stacked, one beside two, two on top with one long pane below (new), and a 2×2 grid. The geometry is `cauldron_chrome::panes`: pure, tested in `crates/chrome/tests/panes.rs`.
- **Layout button** in the card head, at the right, 28px. It draws a small sketch of the shape a click switches to, and its tooltip and name say it ("Layout: One beside two"). Each click steps to the next shape, and the grid goes back to one pane. On a click the sketch morphs from the old shape to the new one in 200ms on the settle curve; with reduced motion it switches at once. The Split button in the top bar goes from one pane to two side by side, and back to one.
- **Two lines**: a vertical one and a horizontal one, each over its 4px gap with a 12px grab area. The accent mark shows on hover and while dragging. Dragging moves the split with the pointer, and double-clicking makes it even. In One beside two the horizontal line spans the right column; in Two on top, one below the vertical line spans the top row.
- **Limits**: 4px between panes (Split was 8px). No pane gets smaller than 200 × 120, so a line stops there. A shape that no longer fits the card (a narrow window, or the drawer open) is drawn as the nearest earlier shape that fits, as the shell already closes Split when the window narrows. Four panes at most, since no shape has more.
- **Close**: every pane after the first has an ×. Closing one steps back one pane: the grid goes to one beside two, either three-pane shape goes to two side by side, and a two-pane shape goes to one pane.
- **Panes**: pane 0 holds the thread, or the new-thread screen. The others say "Open in this pane", with the old half's tiles as placeholders (Highlights, Files, Changes, Terminal, Browser, Add an app); none of them opens anything yet. Split, the panes are the Panels and the card head sits on the Shell's grey, as on the board.
- **Removed**: the solver's half pane is no longer drawn (`split_open` is always false). Split lives inside the card now.
- **Thread in a narrow pane**: the route chip ("Answered by …") now shrinks first, so Send stays in view.

Interpretation to check: "long vertical at the bottom" is built as one full-width pane under two side by side (the coordinator's reading). If Charles meant a tall pane instead, it is one shape in `panes.rs`.

Tests:
- Geometry, test-first: `docs/cauldron/NL4-panes-red.txt` (the module did not exist yet) and `NL4-panes-green.txt` (8 pass). They cover six shapes holding 1 to 4 panes, the stepping order and wrap, close stepping back, the 4px gaps, panes filling the area, 200 × 120 under any drag, lines lining up, reset, and shapes that do not fit.
- App: `docs/cauldron/NL4-app-green.txt`, 42 of 42 (three new). The new tests check that the button sits in the card head and steps through all six shapes; that panes are 4px apart, keep 200 × 120, and closing steps back; and that a real pointer drag moves the line, stops at 200, and a double-click makes it even. The sixth shape changed tests and code together, so there is no separate red run for it.
- The solver-match test now covers four states (sidebar and drawer), since Split no longer uses the solver.

Shots (real window captures, 1200x720 at 1x, a thread open, drawer open): `docs/screenshots/new-look/nl4-panes-<n>-<shape>-<dark|light>.png` for all six shapes.
Not captured: the morph itself (a still shows one frame), hover on the lines, and a drag in the live window. The tests cover the drag.

## Not done (as asked)

- Top bar A: untouched.
- Nothing merged or pushed anywhere except this branch.
