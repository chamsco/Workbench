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
- The main card keeps its colour and its 1px ring, with radius 16, so it nests inside the 22 Shell.
- `motion::Frame` carries the Shell, so it moves with the sidebar like the card does.

At 1200x720 (Mac, sidebar and drawer open): the Shell is x 257 to 1187 and y 45 to 707. The card is x 263 to 939 and y 51 to 701. The drawer is x 945 to 1181, 236 wide.
Folded, the 44px rail stands in the same gutter (`nl3d-folded-*.png`).

## Gate 3: the bottom bar in the Shell grey

No new code beyond gate 2. The new thread's 32px bar sits under the card, inside the Shell's bottom gutter. Usage ("No usage data yet") is on the left; Plan, Execute, Ship and Commit are on the right in 11/14 500 fg3. The bar's bottom edge is 6px above the Shell's bottom edge, so it sits on the Shell grey, not on the frame (`nl3d-new-*.png`; the before shots are `nl3d-before-new-*.png`).
Core still has no stages and no per-subscription usage, so the bar is static: no current stage and no numbers.

## Tests

- Red: `docs/cauldron/NL3d-red.txt`. The new Shell geometry fails the two wireframe tests that still expected the drawer on the frame (the drawer ending at the side margin, and the panes starting right under the bar).
- Green: `docs/cauldron/NL3d-green.txt` (chrome wireframe, 6 pass, 1 ignored as before) and `NL3d-app-green.txt` (app, 39 of 39).
- New assertions: every pane sits 6px inside the Shell. In the app, the `shell` element matches the solver in every state, the stage bar's bottom edge is 6px above the Shell's bottom edge, and the drawer ends 6px inside the Shell's right edge.
- `cargo test -p cauldron-chrome`: all suites pass.

## What I verified on the Mac, and what I did not

Verified:
- The build and both test suites on the Mac (macOS 27, Apple toolchain, cargo 1.99.1).
- The real app window opens on the display at 1200x720 (`CAULDRON_SIZE`), on sample data in a scratch folder rather than Charles's own threads.
- Screenshots: all eight shots (four states, dark and light) are real GPUI Metal renders of the shell at 1200x720, saved at 2x (2400x1440). I measured the tab and the drawer edges in them pixel by pixel.

Not verified:
- **No screenshot of the live window.** macOS refused to capture the screen ("could not create image from display") because the terminal does not have Screen Recording permission. The shots come from GPUI's headless Metal renderer, which draws the same view. They do not include the native traffic lights, the window shadow, or real hover and click states.
- I did not look at the live window by eye, and I did not drag it, resize it, or click through the open and fold animations. Only the tests cover those.
- I did not check the narrow width or a Retina display at other sizes.
- Light shots use `CAULDRON_THEME=light` rather than a system theme switch.

## Tools added (debug and feature-gated, not in a release build)

- `CAULDRON_THEME=light|dark` (debug builds) forces the theme.
- `CAULDRON_SIZE=1200x720` (debug builds) sets the window's starting size.
- `cargo run -p backspace --features shots` with `CAULDRON_SHOTS=<dir> BACKSPACE_DATA=<sample data>` renders New thread, a thread, the drawer folded, and agent tabs in dark and light to PNG, then exits (`crates/app/src/shell/shots.rs`, macOS only). It runs on the main thread because AppKit requires it, so it cannot be a test.

## Open point for Charles

The v3 board draws the drawer and the bottom bar on the window frame. This build follows the brief instead and puts both on the Shell grey (`card`), inside a rounded Shell that also holds the card. In dark, the card and the Shell are the same grey and only the card's 1px ring separates them. In light, the Shell is a touch darker than the frame and the sidebar.
If the card should stand apart more, two options are a `panel` card inside the Shell, or a frame-coloured Shell. Either is one change in `shell.rs`.

## Not done (as asked)

- Split panes: unchanged. The prototype board with two directions comes first.
- Top bar A: untouched.
- Nothing merged or pushed anywhere except this branch.
