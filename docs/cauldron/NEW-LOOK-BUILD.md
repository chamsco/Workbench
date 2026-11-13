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

## Split panes, direction B, three at most (13 Nov)

The board is `design/cauldron-split-panes.html` (published at https://claude.ai/artifact/EAWJMVG33f8ue1f8guXhf1). It still shows both directions, so Charles can overrule B with A. Its four-pane B mock is out of date: the app now stops at three.

Charles, 01:38Z: "go for B, make a pane button that has an animation that shows the different options (add also 2 side by side on top & long vertical at the bottom)".
Charles, 01:49Z: "ok let's go instead with a 3 MAX and make each pane draggable (either 3 vertical or 2 side by side and 1 vertical, or 2 stacked on one side and 1 tall pane), make the elements inside adapt / resize because the text gets jumbled, and when you open a pane, make it look like this" (his screenshot: an empty pane with a centred "Open a surface" list).

What is built (it replaces the four-pane build of ee3d9ba):
- **Six shapes, three panes at most**, in the layout button's order: one pane, two side by side, two stacked, three side by side, two on top with one long pane below, and one tall pane with two stacked beside it. The 2×2 grid is gone. The geometry is `cauldron_chrome::panes` (pure, `crates/chrome/tests/panes.rs`).
- **Layout button** in the card head: a sketch of the shape a click switches to, which morphs to the next shape in 200ms on the settle curve (at once with reduced motion). Each click steps on, and the last shape goes back to one pane. The Split button in the top bar goes from one pane to two side by side, and back.
- **Every divider has a line**: one vertical line, a second one in three side by side, and one horizontal line. Dragging a line resizes only the two panes on its sides, and double-clicking makes them even (all three columns in three side by side). Each line is 4px, with a 12px grab area and an accent mark on hover or drag.
- **Dragging panes**: a pane's name in its header is a handle. Drop it on another pane to swap the two, so the tall pane, or the thread, can sit on either side. The drop target outlines in the accent colour.
- **Limits**: 4px gap and no pane under 200 × 120. A shape that does not fit the card is drawn as the nearest earlier shape that does.
- **Pane header** (from Charles's screenshot, as the coordinator described it): + on the left adds a pane (two side by side → three, two stacked → one tall beside two; nothing once there are three). Next comes the pane's name, which is the drag handle. On the right: Expand (this pane alone), Split bottom (one pane → two stacked, two side by side → two on top with one below), the drawer toggle, and × to close. The thread's pane has no ×.
- **A new pane** is the "Open a surface" list: Browser (with a chevron), Terminal, Files, Diff, Pull request, Linked pull requests and Device, each with its key (B, T, F, D, P, L, M). Diff and both pull-request rows are dimmed because core has nothing behind them yet. Clicking a row, or pressing its key with the pane focused, opens that surface. The surfaces are placeholders ("The terminal surface arrives in a later milestone."). This list replaces the old tiles.
- **Content fits its pane**:
  - The transcript keeps the newest message in view when a thread opens or grows. Before, a short pane showed the oldest messages, cut off by the composer, which is likely the "jumbled" text.
  - Under 360px wide: less padding, and your bubbles may use 90% of the width.
  - Under 260px tall: no cost line.
  - Under 160px tall: the composer becomes one row, the editor and Send, without its chips.
  - The route chip gives way before Send, and pane names and surface labels truncate.
- **Removed**: the solver's half pane. Split lives inside the card.

Interpretations to check:
- "2 side by side and 1 vertical" is built as two on top with one long pane across the bottom, the shape Charles described at 01:38Z.
- "make each pane draggable" is built as both: the lines resize, and a pane's header drags it onto another pane to swap.
- One- and two-pane shapes are kept so you can step down.

Tests:
- Geometry, test-first: `docs/cauldron/NL5-panes-red.txt` (the three-pane API did not exist), `NL5-panes-green.txt` (11 pass). They cover six shapes and three at most, the stepping order, close, +, Split bottom, the 4px gaps, panes filling the card, every divider having a line, 200 × 120 under any drag, a line moving only its two panes, the lines sitting in the gaps, reset, and shapes that do not fit.
- App: `docs/cauldron/NL5-app-green.txt`, 45 of 45. Pane tests cover the button stepping through six shapes; the surface list (seven rows, dimmed rows inert, a click opening one); 4px, 200 × 120, close and +; dragging a header to swap panes; and a real pointer drag of the second line, its 200px stop and its double-click reset.
- New smallest-pane test (`the_thread_fits_the_smallest_pane`): the thread at 200 wide and at 120 tall keeps its composer and Send inside the pane, with the transcript ending above the composer and messages wrapping inside the pane.
- These tests and the code changed together after Charles's 01:49Z message, so the app has no separate red run.

Shots (real window captures, 1200x720 at 1x, a thread open, drawer open), in `docs/screenshots/new-look/`:
- `nl5-panes-1-cols-*`: the thread beside a new pane ("Open a surface").
- `nl5-panes-2-rows-*`, `nl5-panes-3-three-*`, `nl5-panes-4-twoone-*` and `nl5-panes-5-onetwo-*`: the other shapes, each in dark and light.
- Not captured: the morph, a header drag in progress, and a line drag in the live window. The tests cover the drags.

Disk: the Mac's startup volume ran out of space during this build (258 MB free; the link failed with "No space left on device"). I deleted only this worktree's incremental build cache and its old test binaries, which freed about 1.2 GB. This worktree's `target/` is still about 5 GB. Charles may want to free space; `cargo clean` in `~/Documents/code/Workbench-newlook` frees all of it.

## Not done (as asked)

- Top bar A: untouched.
- Nothing merged or pushed anywhere except this branch.
