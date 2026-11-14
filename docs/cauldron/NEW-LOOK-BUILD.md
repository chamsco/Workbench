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
- `CAULDRON_OPEN=<thread id>`, `CAULDRON_DRAWER=folded`, `CAULDRON_PANES=<0-5>` (a shape from `Shape::ALL`), `CAULDRON_TERMINAL=1` and `CAULDRON_SETTINGS=1` (debug builds) start the live window in a given state, for real captures.
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

## Finish: terminal card, Settings, thread view (13 Nov, 06:19Z)

Charles, 06:19Z: "lets finish it". The coordinator listed the order: the terminal card, Settings, the thread view, captures, then this report.

**Terminal card** (v3 frame 5):
- **Opening it:** the Terminal button in the top bar opens a second card beside the thread, in the grey Shell's right gutter, 430 wide. It takes the drawer's place, and the drawer folds to its rail. A rail button, or the card's ×, closes the terminal and brings the drawer back. The layout solver has it (`ShellState::terminal_open`, `Shell::terminal`). If the thread would drop under 360px, the terminal closes, after Split and before the drawer.
- **Blocks from core's traces:** every `Bash` tool span in the open thread's replies is one block. A block has a meta line (the model that answered, the start time, the duration, and "ok", "failed" or "no exit recorded"), then `$ command`, then the output. A failed block has the bad colour on its left edge. Core records that a command failed, not its exit code, so the meta line says "failed" rather than a number. Empty states: "No thread open" and "No commands yet".
- **Input line:** "Type a command, or # to ask the agent". A line that starts with `#` is sent to the thread's agent through core, as the composer sends it. Anything else is not run, and the line says why: core has no shell for this terminal yet. Nothing is made up.
- The drawer has no Terminal entry (its panels are Highlights, Usage, Koi preview, Code review and Patches), so the top-bar button is the only way in. The Terminal surface in a split pane is still a placeholder.

**Settings** (v3 frame 7):
- **Layout:** the gear in the user row opens Settings. Its pages replace the sidebar's thread list, under a back row: Agents and subscriptions, General, Appearance, Machines and Shortcuts. The page itself takes the card, whose head reads "Settings". The back row, or the gear again, returns to the threads.
- **Agents and subscriptions:**
  - Subscriptions: one row per account in core's `accounts.json` (name, then provider and who it is signed in as). The usage meter appears only when the CLI reported used and limit; otherwise "No data yet". With no accounts it says so.
  - Agents: Orchestrator and Executor (from the prefs, shown as values). Effort cycles Auto, low, medium, high and max, and saves, as the composer's chip does. "When a subscription hits its limit" cycles Ask me, Switch and resume, and Off, saved to `accounts.json`.
- **Other pages:** "<Page> settings arrive in a later milestone."

**Thread view:** the narrow- and short-pane rules from f8149e9 are kept. Beside the terminal, the thread is 432 wide at 1200x720 and reads normally (`nl6-terminal-*`).

**Tests:**
- Layout: `docs/cauldron/NL6-terminal-red.txt` (the terminal did not exist in the solver), `NL6-terminal-green.txt` (all chrome suites pass, with the new `the_terminal_takes_the_drawers_place`).
- App: `docs/cauldron/NL6-app-green.txt`, 49 of 49. Four tests are new:
  - The meta line's wording.
  - The Terminal button opens the card, folds the drawer, shows the thread's command from its trace, and closing brings the drawer back. A typed command is not run.
  - Settings replaces the list, puts its page in the card, switches pages and goes back.
  - A subscription with no usage says "No data yet".
- The Settings tests read and write their own temporary `accounts.json`, never the user's.

**Captures** (real window captures, 1200x720 at 1x, on the scratch sample data):
- `nl6-terminal-dark.png` and `nl6-terminal-light.png`: the thread beside the terminal, four blocks, one failed.
- `nl6-settings-dark.png` and `nl6-settings-light.png`: Agents and subscriptions, with two sample accounts and no usage.
- The sample trace and the two accounts were written for the shots. They are not anyone's real commands or logins.
- **Hover: not captured.** Pointer moves posted with CGEvent do not reach the app; posting events needs Accessibility permission for the Claude Code process, and it does not have it.

Not verified: typing in the live terminal or clicking through Settings in the live window (the tests cover both); the drawer-to-terminal animation; narrow windows.

## After the merge: Settings without plugins, background A, tabs in the pane header (13 Nov, 06:30Z on)

Charles, 06:30Z, asked for four things. (1) "New look merge into main" is done: PR #42, merged as c99fea5. The other three are on `claude/new-look-next`, in PR #43. Charles had not picked a direction for (3) and (4); the coordinator picked from the board `design/cauldron-background-and-tabs.html` (88454ee on gg3ogp; artifact https://claude.ai/artifact/86aDFBQVa9rkr2GZaiPyY4).

**(2) Settings has no plugins.** Charles: "settings page should not have the addons / right side plugins, it's only for coding". `ShellState::plugins` is false while Settings is open: there is no drawer, no panel strip, no rail and no terminal, and the card runs to the Shell's padding. Leaving Settings brings them all back as they were. Shots: `nl7-settings-no-plugins-*`, then `nl8-after-settings-*`.

**(3) Background A: the painting on the new thread, frosted glass on its composer.**
- **What draws, and where:** the Tauri app's paintings, in its dither, on the new thread's card only, never behind a thread (or, for now, an empty pane). Dark uses the night harbour and light the dawn harbour. Settings > Appearance > Painting cycles Harbour, Lighthouse at dawn, Moorings at night, Open sea, Fog and None (`prefs.backdrop`, which already existed).
- **The dither:** `Art.band` is ported line for line to `cauldron_chrome::dither`, as pure arithmetic: lightness-only Bayer 8x8, 6 levels, an edge-weighted amount, a dot-by-dot dissolve over the foot's 0.42, and a mute toward the card of 0.3 in dark and 0.07 in light. The shell (`shell/backdrop.rs`) decodes the WebP once, draws it covered at 0.6 of the small frame, scales it up, dithers at 2px per dot and scales up with hard pixels. All of that happens once per card size and theme and is cached; nothing runs per frame.
- **Glass:** the composer box (radius 22) and the machine chip (radius 10) each draw the same painting, blurred (fast blur, sigma 14) and cut at their own rect. The cut is cached by rect. Over it goes the tint, panel colour at 74% in dark and 75% in light, and a 1px line-strong ring. Where each element sits is read in layout by a probe and used on the next frame. The usage line stays in the grey stage bar.
- **Measured worst-case contrast on the glass** (every pixel under the composer and the chip, at the 1200x720 card):

  | Theme | fg2 | fg3 |
  |---|---|---|
  | Dark | 9.07:1 | 5.61:1 |
  | Light | 8.21:1 | 5.22:1 |

  The board measured 5.5 and 5.2. Light uses 75% rather than 74% because 74% gave 5.18:1.
- **Licence:** the paintings are bundled in `crates/app/assets/art/` (six, 412 KB). The Berth notice in `THIRD_PARTY_NOTICES.md` now names the Rust port and the bundled paintings.

**(4) Tabs in the pane header.**
- **One pane:** the thread's tabs (the thread, then its agents' 1:1s) sit in the card head. The active tab is the title, so there is no separate title, and the window bar keeps the breadcrumb ("project / thread").
- **Split:** the card head goes. Each pane has a 40px header with its own tabs: the thread's in its pane, and the others their surface's name. The layout button moves into the first pane's header.
- **Narrow panes:** under 320px, a pane keeps only its active tab and a "+N" that steps to the next tab, and expand, split bottom and the drawer toggle make way.
- **The switch:** Settings > Appearance > Tabs ("In the pane header", the default, or "Floating on top") brings the old strip back to compare. It is saved as `prefs.tab_place`, a new prefs field in core.
- **Not built:** dragging a tab onto another pane's header (the board says it can wait), and the board's history button and "Code" mode chip, because core has nothing behind either. The "+" for spawning a 1:1 is on the floating strip only.

**Tests:**
- Dither (`crates/chrome/tests/dither.rs`, 6): the matrix, determinism, no dissolve above the fade and a monotone dissolve toward the foot, lightness-only, the board's settings, and the cover geometry.
- Background (app, 4): made once per size (a cache hit at the same size, made again on a resize or theme change, one cut per rect), a dissolving foot, glass contrast at 4.5:1 or better for fg2 and fg3 in both themes, and the painting on the new thread only.
- Settings without plugins: a red layout test first (`docs/cauldron/NL7-plugins-red.txt`, then `NL7-plugins-green.txt`), and an app test.
- Pane tabs (app, 3): tabs in the card head with the breadcrumb in the bar; split panes with 40px headers, their own tabs, the layout button in the first, and "+N" under 320px; the Settings switch.
- The dither, background and pane-tab tests were written alongside their code, so they have no separate red runs.
- Runs: `docs/cauldron/NL8-app-green.txt` (app, 57 of 57) and `NL8-chrome-green.txt` (all chrome suites).
- `backspace-core` on this Mac: `policy_integrity` fails two tests (`bash_cannot_weaken_the_policy_for_the_next_session` and `the_lock_is_restored_after_a_command_that_rewrites_it`). They fail the same way with this branch's one core change (the new prefs field) taken out, so they fail on this Mac regardless of this branch.

**Shots** (real window captures, 1200x720 at 1x, scratch sample data; "before" is floating tabs with no painting, "after" is tabs in the pane and the harbour), in `docs/screenshots/new-look/`:
- New thread: `nl8-before-new-*`, `nl8-after-new-*`
- Thread: `nl8-before-thread-*`, `nl8-after-thread-*`
- Two panes: `nl8-before-2panes-*`, `nl8-after-2panes-*`
- Settings > Appearance: `nl8-before-settings-*`, `nl8-after-settings-*`

Any lit button in a shot is the real pointer resting there, not a state.

**The macOS app workflow:** every `macos.yml` run since at least 10 Nov is "cancelled" within about a second with no runner and no steps. That includes the pushes to main and the workflow_dispatch runs (42887499596, 42890395148, 42946753888, 43282129950, 43379886775, 43380184861). The concurrency group cannot cancel a lone dispatch, so GitHub is not handing out a `macos-14` runner, which points to the account's Actions billing or spending limit for macOS minutes.

## The frame without a top bar, painting X, Windows B, and the Appearance pass (13 Nov, 06:52Z on)

Charles, 06:52Z, asked for these:
- the painting under the grey borders as well;
- more room at the top and bottom of the content, with tabs in each pane ("Variant, two panes");
- no breadcrumbs;
- the right sidebar's button on the right rail;
- split and terminal at the top right of the content, left of a Code pill;
- on Windows, the caption buttons inside the grey border.

At 07:03Z he followed up: the painting must not disappear on the sidebars and panes. At 07:08Z he approved the design and asked that themes and paintings never clash or carry stale state. The coordinator's boards are `design/cauldron-frame-no-topbar.html` (b427b05, then e0fd268, on gg3ogp). Everything here is on `claude/new-look-next` in PR #43.

**Frame (Mac):**
- There is no top bar over the content. The lights, the sidebar button and the name keep a 44px row over the sidebar, and under it sits the app row: message, folder, calendar, bot, gauge, database and grid, with a hairline under it.
- The Shell runs 6px from the window's top and bottom, so the card is 12px from each.
- With the sidebar folded, the Shell starts under that row, so it never meets the lights or the name (`nl9-sidebar-folded-*`).

**Panes:**
- Every pane's 40px header (the card's head when it is one pane) has the tabs on the left, then split (the layout button), the terminal and the Code pill.
- The Code pill is a static label: only coding threads exist, and Charles asked for it by name.
- Under 320px a pane keeps the active tab, "+N", split and Code; the terminal button makes way.
- The breadcrumb, the floating tab strip and its Settings switch are gone; Charles chose the variant.
- The thread strip's "+" (spawn a subagent or open a 1:1) and agent tab closes moved into the pane strip.

**The drawer's toggle:** at the top of the right rail, then a hairline, then the panels. While the drawer is open, it is the first button of the drawer's strip, lit.

**Windows (B, the board e0fd268):**
- The caption buttons (3 × 40 × 24) sit inside the Shell's top right, in a 31px band.
- Each pane's tabs float in a 26px strip in that band, above the pane. The rightmost strip stops 8px short of the caption buttons.
- The card head shows the breadcrumb ("project / thread") on the left and split, terminal and Code on the right. The card starts 38px down.
- `CAULDRON_PLATFORM=windows` draws this layout on a Mac for a preview (`nl10-windows-thread-*`); the Mac's own lights still show there. It has not run on Windows.

**Painting X: behind the whole window.**
- The painting is dithered once at window size, and one blurred copy (sigma 13.5, about 27px) is made from it. Both are cached by painting, theme and size.
- The blurred copy, cut at each surface's rect, lies under every surface's tint: the window and sidebar, the Shell (gutters, drawer, rail, stage bar), every card and pane, the terminal, Settings' page, the composer, its chip and the header pills.
- The new thread's card is the one clear window onto the sharp painting.
- The tints are the board's, as a floor. Each surface's alpha rises only as far as that painting needs for fg2 and fg3 to keep 4.6:1 over its rect, computed once and cached with the cuts.
  - At Harbour the board's values hold in both themes. Measured worst-case fg3, dark / light: sidebar 5.35 / 4.64, drawer 4.69 / 4.62, stage bar 6.39 / 5.39, thread card 5.47 / 5.36, composer 5.76 / 5.23, chip 5.69 / 5.42.
  - The brighter paintings need more frost in light, up to alpha .94 for the drawer over Lighthouse at dawn.

**Appearance pass: what I found and fixed.**
1. The theme came only from the OS; `prefs.theme` was never read. Settings > Appearance now has Theme (System, Light, Dark). System follows the OS live: an appearance observer re-renders, painting included.
2. The board's fixed tints fail on the other paintings. In light, Lighthouse at dawn under the sidebar gave fg3 4.08:1. Fixed by the adaptive alpha above.
3. Core's `Prefs::load` resets the whole file to defaults if any field has the wrong type. An old file with `"tabs": "floating"` would lose its theme and painting. The window now reads theme and painting field by field (`shell/appearance.rs`). The core loader is unchanged.
4. Old painting ids: "plain" and "none" map to None, the Tauri paintings to themselves, and anything else (aurora, the sharks, koi, typos) to the default, Harbour. A missing file, an unreadable one, or wrong types give the defaults.
5. With None, the old painting and its cuts stayed in memory. Now everything is dropped, and None draws the opaque tokens: no window art, no frost, no glass layers.
6. Cache keys were already painting, theme and size. A resize away and back now gives the same painting byte for byte (tested).

Settings follows the same rules: it is frosted, with no drawer and no rail.

**Tests:**
- App: `docs/cauldron/NL11-app-green.txt`, 61 of 61.
- Chrome: `NL11-chrome-green.txt`, all suites.
- New tests:
  - The frame solver, written red first (`NL9-frame-red.txt`; the Windows band `NL10-windows-red.txt`).
  - The pane header, plus the Windows tab strip and breadcrumb.
  - fg2 and fg3 at 4.5:1 or better on six surfaces, for all five paintings in both themes.
  - The resize round trip and "None keeps nothing".
  - The render matrix: 3 themes × 6 backdrops (None included) × 5 views (new thread, thread, two panes, drawer folded, Settings), each rendered with the painting present or absent as expected and no drawer in Settings.
  - Appearance parsing: defaults, Tauri mappings, files core cannot parse, the theme cycle.
- The appearance tests read their own prefs file per window, never the user's.

**Shots:**
- Real window captures at 1200x720 on Mac: `nl9-*` (round 1) and `nl10-*` (round 2: new thread, thread, two panes, Settings, and the Windows preview).
- The contact sheets, real captures through the prefs file (not the debug theme override): `nl11-sheet-new.png` and `nl11-sheet-thread.png`, light and dark × Harbour, Lighthouse at dawn, Moorings at night, Open sea, Fog and None.

**Limits:**
- Windows has not been run.
- Hover and drags are not captured.
- The blurred copy is static: a pane resize or a drag cuts new rects, and the cache holds 64 of them.
- `backspace-core`'s two `policy_integrity` failures on this Mac are as before and unrelated.

## Round 3: flush corners, apps, streams, folders, the rails, pages (13 Nov, 07:34Z and 13:40Z)

These are Charles's 07:34Z list and 13:40Z note ("not as tabs, they need to replace the main content area... streams should belong to a folder"). The coordinator's boards are `design/cauldron-pages-switch.html` (5b7662c on gg3ogp). The work is on `claude/new-look-round3`, in PR #44 on top of #43.

**The corner gap.** The window-wide frost was already under everything, but at the Shell's rounded left corners it showed the sidebar's frost, a different shade, so the corners still read as a notch. The Shell now meets the sidebar (and the folded rail) flush, with square left corners and rounded right ones. Solver test red first: `NL12-flush-red.txt` / `NL12-flush-green.txt`.

**Apps.** The 7-icon row and its hairline are out of the sidebar and the folded rail. Settings > Apps lists them; Streams opens, and the others say they arrive later.

**Streams.** UI text only: New stream, Search streams, This stream, and the empty and error texts. Core's ids, types and files are unchanged.

**Streams, Agents and Memory.**
- A three-way switch at the top of the sidebar replaces only the card. Memory also drops the drawer; Agents gets its own drawer mockup (Context, Traces, Routines, Settings).
- The sidebar's streams stay put. Opening a stream returns to Streams.
- Agents and Memory are labelled mockups:
  - Agents: a roster with figures (they belong here) and a detail (Route, Computer, Memory, Switched off, Routines). It shows core's real agents when there are any, otherwise examples marked "Example content".
  - Memory: Proposals, Notes, Dreams and Skills, scope chips, and proposal cards with Reject and Accept, the global one marked "Needs your approval". All example content, marked as such.

**Folders (direction B).**
- The sidebar shows Needs you (flat, each row with its folder chip, not repeated under the folder), then Folders with a +, then each folder: chevron, icon, name, count, and an accent dot when it is closed over a stream that needs you. Unfiled comes last.
- A folder is open when it holds a running, waiting or open stream; a click toggles it for the session.
- Rows are a state dot (needs you filled accent, running a ring, failed bad, done grey), the title and the age.
- Search lists hits flat, each with its folder chip.
- The "+" for a new folder is shown but arrives later.
- Move to folder is not built. A stream's project is where its CLI runs (`chat.rs`: "the project folder a CLI works in"), so moving one would change where it runs.

**User row and folded rail.**
- Unfolded: the name with the usage ring beside it (with no data, "No data yet" is in its tip: core has no per-subscription usage), the version button (an icon; it opens Settings > General, which shows the version and says updates come from the release workflow), and the cog.
- Folded rail, top to bottom: a usage ring (empty), the three apps, search (opens the sidebar), New stream, a square per folder (its letter, a dot over a waiting stream; it opens the sidebar on that folder), then the version, the cog and the avatar at the foot.

**The right rail.** It always stands at the Shell's right edge, its icons stacked: the drawer's toggle (lit while open), a hairline, then the panels (the open one lit). The open drawer sits beside it; there is no horizontal strip. The card is about 50px narrower with the drawer open, and the minimum-width rule closes Split and then the drawer a little sooner. Solver test red first: `NL12-rail-red.txt` / `NL12-rail-green.txt`.

**New stream.**
- The composer sits about 40px under the card's middle.
- Under it are the folder chip, the machine, and Worktree, which is dimmed because core has nothing behind it.
- The folder chip suggests the working directory when it is a known folder. Its popover lists your streams' folders (the working directory's tagged "current directory"), No folder, and New folder (arrives later).
- The chosen folder becomes the new stream's project, the folder it runs in. A test checks the created stream's project.

**Tests.**
- `docs/cauldron/NL12-app-green.txt`: 66 of 66.
- `NL12-chrome-green.txt`: all suites.
- New tests cover the sidebar model (Needs you, folders, toggles, search, the open stream's folder) and ages, the folding UI and its dot, the folded rail's order, the rail beside the drawer, the switch keeping the sidebar and a stream bringing Streams back, the Memory mockup, the folder chip and Settings > General.

**Shots.** Real window captures at 1200x720, dark and light, in `docs/screenshots/new-look/`:
- New stream: `nl12-new-*`
- Stream: `nl12-thread-*`
- Two panes: `nl12-2panes-*`
- Agents: `nl12-agents-*`
- Memory: `nl12-memory-*`
- Settings > Apps: `nl12-settings-apps-*`
- Folded sidebar: `nl12-sidebar-folded-*`

The sample data's streams were given recent times for the ages.

## The destinations and the Agents demo (13 Nov, 15:54Z and 16:05Z)

Charles, 15:54Z: go back to the buttons, and bring Automations back. Usage goes at the bottom of the collapsed sidebar, with a tracker on hover and a page on click. He also asked to reuse the premade agents designs and keep the Librarian, in the order Agents, Folders, Library.

16:05Z: he accepted M3 (`docs/cauldron/M3-SIGNOFF.md`) and said: "Continue the Agents demo next. Follow UI-PLAN decision 5: don't present missing engine behaviour as functional."

The coordinator's board is `design/cauldron-destinations.html` (ac5ef7a on gg3ogp).

**Six buttons**, under the sidebar's top row:
- Streams, Folders, Automations, Agents, Usage and Library, with tooltips. They are the old icon row, back in place of the switch.
- Each replaces the card; the sidebar's streams, search, New stream and footer stay. Apps stay in Settings.
- Folded, the same six stack on the rail.

**Usage:**
- An empty ring sits beside the name; core has no usage per subscription. With no data the ring stands alone, so the name keeps its room: "No data yet" is in its tip and accessible name. A click opens the Usage page.
- The agent drawer's ring captions and the room's composer text truncate inside their box, so nothing runs into the rail or cuts off the send button.
- On the folded rail the ring sits at the foot, above version, cog and avatar.

**The Agents demo:**
- It uses the approved Cauldron figure from `crates/chrome` (flat colour, moods, a bust in the room), not the old brain mascot.
- On the left: the roster (core's own agents first and unmarked, then four examples), and group chats.
  - The four examples are Claude Code (asking), Codex (working), Cursor (watching) and the Librarian (amber, reading, "Filing 2 notes").
- On the right, the open room: a figure stack, its name, an example conversation in busts in which the Librarian files a note, and the composer.
- Agents' own drawer, beside the standing rail: the agent with weekly and 5-hour rings, its current task, a Context treemap, Traces, Routines and Settings rows.

**Demo marking (UI-PLAN decision 5):**
- The header says "Demo".
- Every example agent, room and message, and the drawer, say "Example".
- The composer is disabled and says "Not connected: messages to … are not sent yet".
- Room info, New group and the drawer's settings rows look disabled and say "Demo: not built yet".
- Real core agents would show without a mark.

**Folders, Automations, Usage and Library** are placeholder pages marked Demo, each saying what it will hold and that it is not built yet.

**Tests:** `docs/cauldron/NL13-app-green.txt`, 68 of 68. New ones check:
- the six buttons replace the card, keep the sidebar and return to Streams;
- the Agents demo is marked throughout (Demo, Example on each example, "Not connected", the drawer marked), and the Librarian opens its own lines;
- the folded rail's order, with the usage ring at the foot.

**Shots** (real captures, dark and light): `nl13-agents-*`, `nl13-thread-*`, `nl13-library-*`, `nl13-folded-*`.

## Not done (as asked)

- Top bar A: untouched.
- Nothing merged or pushed anywhere except this branch.

## The drawer at 200 (13 Nov, Charles 18:49Z)

- The right drawer is 200 wide, UI-PLAN's width, down from the sidebar's 236 (`layout::DRAWER_W`). The 44 rail beside it is unchanged.
- Agents' drawer follows the destinations board: the agent, the two rings with their captions under them, the current task, then tabs Context, Traces, Routines and a cog for Settings (its name in the tip). Tabs have 5px padding and the drawer 6px sides, so the row fits the 188 inside; in a wider font the text tabs end in "…" rather than run into the rail. The drawer scrolls if the window is short.
- Test: `the_agents_demo_is_marked_as_a_demo` checks the tab row ends inside the drawer and that Traces replaces Context.
- **Shots:** before `nl13-agents-*`; after `nl14-agents-*`, `nl14-agents-folded-*`, `nl14-stream-*` (dark and light).

## The moving background, S1 Flow (13 Nov, Charles 18:49Z)

- **What:** the shader board's S1 (b2328bb), ported from its JavaScript to `cauldron_chrome::flow`: 3D value noise in three octaves, warped by itself, under a glow from the bottom right, six levels of the theme's violet ramp (dark #0e0e12 to #e6e0ff, light #f7f7f8 to #6236d6; the accents #a08cff and #6236d6 sit on it), the 8x8 Bayer pattern, 3px cells fixed to the window. The noise is worked out every 4th cell and blended between (its finest wave is about 100px across), which cut the cost about 6 times with no visible change.
- **Where it moves:** only behind the new stream's card, with the window in front, and motion allowed by the system. Any stream, page, Settings, a split or an inactive or minimised window holds the last frame still. Every other surface shows the frosted copy: the same field on a 100x60 grid, dimmed, stretched smooth, remade twice a second while it moves (no blur pass).
- **Settings > Appearance > Background:** Moving / Still / Off, and under Still a Picture: Flow or one of the paintings. All in the saved `backdrop` id, so core's prefs have no new field: `flow` is Moving, `flow-still` or a painting is Still, `plain` is Off. Old values carry over: a saved painting is Still with that painting.
- **Frames:** made off the main thread (GPUI's background executor), handed to the card as a BGRA picture one texel per point; the picture it replaces is freed from the GPU a frame later (`Window::drop_image`), so memory stays flat (150 MB over a minute of motion, no growth).
- **Measured on this Mac** (release build, display 1920x1080 at 1x, so a 2560x1600 window cannot open here; median of 30 one-second samples, window in front):

| Window | Background | App CPU | Making a frame |
|---|---|---|---|
| 1280x800 | Still | 1.2% | none |
| 1280x800 | Moving, 15 fps | 12.9% | 2.1 ms (750x743 card) |
| 1280x800 | Moving, 10 fps | 10.0% | 2.1 ms |
| 1920x1055 | Still | 0.9% | none |
| 1920x1055 | Moving, 15 fps | 16.2% | 4.1 ms (1390x934 card) |
| 1920x1055 | Moving, 10 fps | 13.4% | 3.5 ms |
| 2560x1600 | (bench only) | | 1.4 ms for the cells, `cargo run --release -p cauldron-chrome --example flow_bench` |

- **The upload, measured:** the same run with the picture not replaced (the window still redrawn 15 times a second) used 9.1% (1280) and 10.4% (1920). So handing GPUI a new picture costs about 3.8% and 5.8% of a core: **about 2.5 ms and 3.9 ms a frame**, over the 2 ms the board allowed. Redrawing the whole view costs about 3 ms a frame (GPUI redraws the window, not just the card).
- **So:** frames run at 10 fps, and the default **Moving plays, then rests** (the coordinator, 19:38Z): it plays for 8 seconds on launch, when the window comes back to the front, and each time New stream is shown, then holds its last frame; nothing else wakes it, typing included (`appearance::Burst`). Settings > Background: Moving (default), Always moving (the old Moving), Still, Off; saved as `flow`, `flow-live`, `flow-still` (or a painting), `plain`.
- **Measured, Moving (1440x900 default window, release):** while it plays, 13.4% CPU (seconds 2 to 8 after launch); resting, 0.0 to 0.4% (seconds 15 to 30), the same as Still: no frames made or uploaded. At launch it played to about 13 s: it starts again when the window first comes to the front, a few seconds into launch. GPUI samples pictures smoothly (its Metal sampler is linear), so a smaller picture would blur the grain: no saving there. What would bring Always moving under 5%: draw Flow as its own cached GPUI layer so a frame does not redraw the window (about 3 ms saved), at 10 fps. Not built.
- **Tests:** `flow::tests` (range, the ramp's levels only, glow at the bottom right, a rect matches the window under it, it moves, the frost never reaches the brightest level); `flow_moves_only_behind_the_new_stream`; `the_background_reads_from_the_saved_id`; `moving_plays_then_rests` (fake clock: launch, focus and New stream start it, it stops at 8 s, typing does not restart it); the contrast test now covers Flow too (16 cases).
- **Shots:** `nl15-flow-*` (Moving, new stream), `nl15-flow-stream-*`, `nl15-flow-settings-light`.

## Three layout fixes (13 Nov, Charles 19:29Z)

- **The right side starts folded:** at launch no drawer is open, whatever it was last time.
- **No right side on a new stream:** the rail and drawer stand only on a started stream, and on Agents (its own 200 drawer, opened with the page). Not on the new stream, Folders, Automations, Usage, Library or Settings (`Cauldron::has_rail`). Nothing else depended on the rail: the terminal still opens on a started stream, in the drawer's place.
- **One frame colour:** the separately tinted Shell behind the card is gone, so there is no second slab with square corners and no edge between the sidebar and the content. The window's frosted tint now holds 4.5:1 over the whole window, since the drawer and rail sit on it. The card is the only rounded surface.
- **No usage strip under the card:** usage is in the sidebar's footer; the bar under the new stream keeps only the stages.
- **Shots:** before `nl15-flow-dark` (drawer open, the slab, the strip); after `nl16-launch-*` (Flow still), `nl16-launch-harbour-*`, `nl16-stream-*`.

## The sidebar follows the destination, and global search (13 Nov, Charles 18:57Z)

From the context-sidebar board (direction A, sections 1 to 5).

**Sidebar.** The top row, the six buttons and the footer stay; the zone between them is the open destination's (`shell/context.rs`):

- **Streams:** unchanged.
- **Agents:** New chat and New group (disabled: "Demo: not built yet"), Agents (core's, unmarked, then the examples marked Example), Crew (the two example crews, marked). The Agents page drops its in-page roster: the chat is wider, the 200 drawer stays. Picking a crew shows that crew's room.
- **Folders:** New folder (disabled), All folders, the folders the streams are in with their colours (real: from the streams' folders), Proposed by the Librarian (one row, Example).
- **Automations:** New automation (disabled), Templates (Example), Yours (core's agents' routines, real), Recent runs (from the routines' last run, real; "Nothing has run yet" when none).
- **Usage:** Overview, Tools (the coding tools core reads usage from and finds on this machine, real, each "No data yet": core has no per-tool numbers yet), Alerts (Example). No create action.
- **Library:** New note (disabled), Proposals (Example), Notes (the count from memory), Graph, Librarian, Notes by folder (memory's notes grouped by place, real; nothing when memory has none). Memory is only read if it already exists: opening it would create it.
- Outside Streams, a one-line pill "N need you in Streams" (the same count as Streams' Needs you: an unopened reply), shown only when N > 0; a click goes to Streams.
- A picked row is highlighted as a stream is; until one is picked, the destination's main view is (All folders, Templates, Overview, Notes). The folded rail does not change per destination.

**Search** (`shell/search.rs`):

- A magnifier immediately right of the sidebar button (Charles, 13 Nov 19:29Z: "at the top left next to the sidebar retract button"; the coordinator corrected the board): the top row is window buttons, sidebar button, search, then the name, which truncates first in a narrow sidebar. It is in the layout contract (`Shell::search_button`, folded: none) and the golden top-row test, which says it follows that instruction, not the old wireframe; the sidebar button did not move. Folded, search is on top of the rail. Its tip names the shortcut. Shots: `nl18-toprow-*`.
- Cmd+K (Mac), Ctrl+K (Windows, Linux) or the icon opens a centred menu (560 wide, scrim, input focused); Esc or the shortcut again closes it, and focus returns where it was. Up, Down and Enter move and open. Cmd+K and Ctrl+K were bound nowhere before (the app and gpui-component). On Windows and Linux, Ctrl+K goes to the terminal when its input has focus, and Ctrl+Shift+K opens search there.
- **Ranking, not filtering:** a case-insensitive substring on the title, then the context and text; a title that starts with the query first, then one containing it, then a text match. The open destination's group first, then everything else under Elsewhere. Nothing typed: Recent (the three newest streams), Actions in this destination (only ones that work), Go to.
- **What it finds:** streams (title and last words), core's agents, folders, notes in memory, the destinations, Settings and its pages, New stream and Hide/Show sidebar. The example agents and crews carry "Example". Not searched: automation runs and the Librarian's proposals (not in core). Opening a result goes to its destination; the sidebar follows.
- **Not done:** the 120ms list crossfade (optional on the board). **Accessibility:** the menu, its field and each result carry names and the selected state GPUI exposes (`aria_label`, `aria_selected`, what the tests read); GPUI has no platform accessibility tree on macOS, so VoiceOver cannot announce a dialog or listbox role. That needs GPUI support, not app code.
- **Tests:** `the_current_destination_ranks_first`, `nothing_typed_gives_recent_actions_and_places`, `the_shortcut_by_platform`, `search_opens_ranks_and_closes` (icon, focus, ranking, Esc, the shortcut opens and closes), `the_pill_counts_streams_that_need_you`, `notes_group_by_place`; the destinations test now checks Library's own list and a picked row's highlight.
- **Shots:** `nl17-agents-*`, `nl17-library-*`, `nl17-usage-*`, `nl17-search-agents-*` (nothing typed), `nl17-search-streams-*` ("m" on Streams).

## Resizable panels that learn your width (13 Nov, Charles 19:29Z, chose A: the learned peak)

- **Model** (`cauldron_chrome::snap`, pure, from the prototype's reference JS): the last 40 settled widths per panel, recorded on release (never on open); weight `0.5^(k/5)` by later resizes; density of Gaussians (sigma 14) over the panel's range, peak in 1px steps; snaps only when the total weight is at least 2 and the peak holds at least 34%; radius 8 to 16 by confidence, lock zone 0.4 of it, a linear ease between with no jump at either edge.
- **Layout:** `layout::solve_with` takes the widths (`Widths`); `solve` keeps the defaults, so the goldens are unchanged.
- **Panels and ranges:** sidebar 200 to 360 (default 236, the app's width; the spec's 248 was not the app's), the drawer beside a started stream 280 to 560 (default 340), Agents' drawer 168 to 280 (default 200). The Library note panel is not built yet. Each keeps its own history in `panels.json` in the data folder (core's prefs untouched); folded and unfolded, the sidebar comes back at its learned width.
- **GPUI:** a handle on the sidebar's right edge and the drawer's left edge (col-resize cursor, a line on hover, accent while dragging, with the width and "your usual" when locked or "exact" with Alt or Shift: GPUI gives the modifiers on every mouse move, Alt included). Double-click goes back to the usual width without recording. A handle takes focus: arrows 8px, Alt+arrows 1px, Home and End, recorded after 700ms. A release outside the window ends the drag on the next move. A width the window forced (the card keeps its 360) is held back and not recorded. Tip: "Drag to resize. Hold Alt or Shift for exact. Double-click for your usual width." Settings > Appearance > Panels: "Reset learned widths".
- **Tests:** the model's (peak on a seeded history, magnet continuity at R0 and R within 0.01, drift from 248 to 300, one odd resize makes no magnet, Alt/Shift unlock, the cap of 40); `dragging_learns_and_snaps` (the handle on the edge, learning, snapping, exact, a forced width not recorded, double-click not recorded, saved and read back, reset).
- **Shots** (`nl19-panel-*`, light and dark): before; mid-drag locked on the usual 280; the same with Alt ("exact"); past the magnet. Synthetic mouse events do not reach the app on this Mac (posting them needs Accessibility for the terminal), so these hold the handle mid-drag through the same code path (`CAULDRON_DRAG=dx[,exact]`, debug builds only).

## Glass Folders (14 Nov, the destinations board section 3; Charles chose "UI plus real folders, rest Example")

- **A, All folders** (`shell/folders.rs`): one glass card per folder, the old app's Memory folder (`legacy/tauri-app/ui/dash.css`, `.fold-art`) redrawn at the board's size (148x108): a coloured back with its tab, two sheets that tip further and rise on hover, a frosted front with "Updated … ago" and a dot when a stream there needs you. Under it the name, the stream count, and the dots: running, needs you, notes. The Librarian's proposal is an outlined, see-through folder (Example), New folder an empty outlined tile (Demo: not built yet), and a Librarian stripe on top (Example, Review disabled).
- **B, a folder:** a card (or the sidebar's folder row) opens it: the same glass folder as its header with the counts, a Librarian proposal stripe (Example), its streams (each opens in Streams), its agents (Example: not per folder in core yet), and its notes from memory ("Open in Library").
- **The glass:** each folder's art is one SVG per colour and hover: the sheets drawn again under the front, blurred (feGaussianBlur), under the folder colour's gradient, an inset highlight and a soft shadow. GPUI keeps the rasterised picture, so it is made once per colour, not per frame. Colours are the board's (violet, green, amber, blue, orange; Unfiled grey), shared with the sidebar's folder icons.
- **Real now:** the folders (a stream's folder is the folder it works in, `threads::folder`), their streams, running and needs-you counts, last update, notes per place. **Waiting on core's `folder` field** (being written in parallel): folders separate from the working directory, New folder and filing; then this page and the sidebar list switch to it.
- **Tests:** `folders_count_streams_and_notes_by_folder`, `the_art_is_a_glass_folder`.
- **Shots:** `nl20-folders-*` (A), `nl20-folder-detail-*` (B), light and dark.

## Folders on core's folder field (14 Nov; core 9732acc, 3c26462, 578a2cd cherry-picked from claude/project-thread-gg3ogp)

- **The field:** a stream's folder is core's `Thread.folder`, read through `folders::Folders` (`list`, `threads_in`, `unfiled`); where it runs is `Thread.project` and filing never touches it. The app keeps a `threads::Filing` (folders in creation order, each filed stream's folder), read again when the streams or the folders change. Every list goes through it: the Streams sidebar, the Folders sidebar and page, search, the folded rail. `threads::folder` is now only the working directory's name.
- **Existing streams start Unfiled.** Empty folders list (count 0).
- **Actions, all real now:** New folder (you name it: the Folders page header, its tile, the sidebar's New folder and the + by Folders in Streams, and the New stream chip's menu); Rename and Delete in a folder's detail (Delete asks once; its streams become Unfiled); filing a stream: right-click its row in the sidebar, or "File in…" on a row in a folder's detail, into any folder or Unfiled, or a new one.
- **"File by working directory":** one explicit button on the Folders page header and its empty state: a folder per working-directory name (made if missing) and each stream filed in its own. It runs only when clicked, and says what it does; it is not a Librarian suggestion.
- **The New stream chip files the new stream:** its menu lists core's folders, No folder and New folder (which names one and chooses it). The stream still runs where it would have (the working directory, when that is a known one); after it starts it is filed in the chosen folder.
- **Still Example:** the Librarian's proposals and agents per folder (milestone 10).
- **The tests' folders registry is their own** (the fixture passes its data folder); nothing writes to the user's data folder from a test.
- **Tests:** `folders_file_streams_without_moving_them` (empty state, File by working directory, the menu, rename, delete to Unfiled, `project` unchanged throughout), `the_folder_chip_files_the_new_stream` (New folder from the chip, filed, working directory untouched), `folders_come_from_filing_not_the_working_directory` (filing over working directory, empty folders, a folder that is gone is Unfiled).
- **Shots:** `nl21-folders-*`, `nl21-folder-detail-*`, `nl21-streams-*`, `nl21-folders-empty-*`.
