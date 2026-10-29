# Shell audit against the ADE/IDE goal (29 Oct 2026)

The goal Charles set (written for "Cauldron", applied to this app): Chat leaves
the top navigation and agent chat lives in the ADE; one shared shell with a top
global rail; each app owns its sidebars; an ADE modelled on BB and T3 Code; an
IDE that is a multiplexer canvas of up to six panels; real Plugins and Skills
management. Base: PR #31's branch (`main` is a week behind the open stack).

## Executive summary

Round 1 builds the shell: a top rail across the window (New thread | sidebar
pill, back/forward, the app switcher centred with Plugins, Skills and Agents,
then search, settings and the window buttons). Chat is out of the switcher;
New thread, Ctrl/⌘N and the end of onboarding start a thread in Code's ADE;
named agents are behind the rail's Agents entry (Charles's pick). Settings
closes with a large round button at the card's top right, back to whatever was
open. The IDE takes six canvases (UI and `core::layout`), with 5 and 6 presets,
and the canvas picker no longer offers "Empty canvas". Not done yet: the ADE's
split work area, and enable/disable/remove for plugins and skills.

## Interaction ledger

Evidence: `tools/uiharness/evals/shell-rail.js` (run with the harness, see
`tools/uiharness/README.md`), screenshots in `docs/screenshots/shell-rail/`.

| # | Interaction | Before | After | Evidence |
|---|---|---|---|---|
| 1 | Chat absent from top navigation | FAIL (tile in sidebar) | PASS | eval `chatInNav: false`; zones = home, code |
| 2 | Top global rail with centred app switcher | NOT IMPLEMENTED | PASS | `*-before-after.png` |
| 3 | Plugins entry opens the plugin catalog | NOT IMPLEMENTED (a + tile) | PASS | eval `plugins.mode: "apps"` |
| 4 | Skills entry opens skills | NOT IMPLEMENTED | PASS | eval `skills.section: "skills"`, button pressed |
| 5 | Agents entry opens named agents | n/a | PASS | eval `agents: chat/agents` |
| 6 | Leading pill: New thread starts an ADE thread | NOT IMPLEMENTED (search button there) | PASS | eval `newThread: code/pair` |
| 7 | Leading pill: sidebar toggle folds and unfolds | PARTIAL (two buttons in two places) | PASS | eval `sideAfter1: true`, `sideAfter2: false`; `ade-sidebar-folded.png` |
| 8 | Ctrl/⌘N starts a thread | FAIL (opened Chat) | PASS | eval `ctrlN: code/pair` |
| 9 | Ctrl/⌘J | Chat ↔ Code | ADE ↔ IDE | eval `ctrlJ: code/agents` |
| 10 | ADE/IDE switch in the app toolbar | PASS | PASS | card header |
| 11 | Settings has no app sidebar | PASS | PASS | `settings-before-after.png` |
| 12 | Settings: large round close, back to the previous page | FAIL (small ×) | PASS | eval `afterClose.mode: "apps"` (was Plugins) |
| 13 | IDE: up to six panels | FAIL (4) | PASS | eval `panes: 6`, picker says the tab is full at 6; `ide-six-panels-before-after.png` |
| 14 | IDE picker lists only real panel types | FAIL ("Empty canvas") | PASS | eval `pickerKinds` |
| 15 | IDE add / remove / resize / maximize | PASS | PASS | existing pane actions and gutters |
| 16 | IDE saved canvases persist | PASS | PASS | tabs filed per project (`set_tabs_for`) |
| 17 | ADE split work area (files, diffs, browser, terminal, images, logs) | PARTIAL | PASS (round 2) | see rows 23–31 |
| 18 | Plugins lifecycle: install, remove | PASS | PASS | `app_install_*`, `app_remove` |
| 19 | Plugins lifecycle: enable / disable | NOT IMPLEMENTED | NOT IMPLEMENTED | `Installed.enabled` has no setter in the UI |
| 20 | Skills lifecycle: list, search, add (folder) | PASS | PASS | `skills_catalog` |
| 21 | Skills lifecycle: enable / disable / remove | NOT IMPLEMENTED | NOT IMPLEMENTED | |
| 22 | Chat › Agents view renders in the harness | BLOCKED | BLOCKED | crashes on the harness's stub data on the base branch too; needs a real backend or a fuller mock |

## Round 2: the ADE work area

`ui/work.js`: a split beside the ADE chat, inside the card, with six tabs on
real state. The header's panel button opens it in the ADE (the IDE keeps its
side panel), the terminal button opens its Terminal tab, the left edge resizes
it, and open/tab/width are remembered per viewer.

| # | Interaction | Result | Evidence (`tools/uiharness/evals/ade-work.js`) |
|---|---|---|---|
| 23 | Panel button opens the work area in the ADE | PASS | `open: true`, pressed |
| 24 | Files: list (`list_files`), filter, read a file (`read_file`) | PASS | `files` shows main.rs contents; `files.png` |
| 25 | Diffs: uncommitted files (`git_changes`), a file's diff (`git_diff`), History (`git_log`) | PASS | `diffs: 2` added lines; `diffs-before-after.png` |
| 26 | Browser: the IDE's browser pane, framing a local dev server | PASS | `browser: true` |
| 27 | Terminal: a real shell in the project (`terminal_*`) | PASS | `terminal: true` |
| 28 | Images: project images as thumbnails (`read_image`), one enlarged | PASS | `images: 1`; `logs-and-images.png` |
| 29 | Logs: the project's ADE threads, each reply's trace (`chat_thread`, `trace_get`) | PASS | `logs: 4` spans; `logs-and-images.png` |
| 30 | Terminal button in the ADE opens the Terminal tab | PASS | `termBtn: "true"` |
| 31 | IDE keeps its side panel, no work area | PASS | `ideWorkHidden: true` |

The ADE side panel's Sessions tab is not carried over: the ADE's own sidebar
lists the project's threads. Also fixed: trace rows drew their bar with the
card header's styles (they share the `.tbar` class), in the IDE too.

## Round 3: back to the full-height sidebar (Charles, 29 Oct)

Charles reversed the top rail after seeing it: the sidebar gets its full
height back (more room on the left), the apps move to a **vertical pill app
rail** on the window's left edge (Home, Code, pinned apps, then Plugins,
Skills, Agents, then live sessions and Agents' bots), and the sidebar's own
row carries the leading pill (New thread | sidebar toggle), back/forward,
search and settings. In the ADE the card header is only **ADE | IDE**,
centred, and the panel buttons; the folder button is gone (the composer and
the sidebar pick the project). In the IDE the Canvases/Wall switch shows only
while a session runs (a worker exists, or the main agent is busy or waiting on
you), and stays on the Wall so the way back is there. References: ZCode
(empty state, terminal drawer, tabbed right panel), Zuse and Nova (sidebar,
chat beside a browser).

Rows 2, 6 and 7 now read: the switcher is the vertical rail, not a top bar;
New thread and the toggle are the sidebar row's pill. Evals re-run: both pass.

## Next, in priority order

1. Plugins: enable/disable (core setter + command + toggle) on the catalog.
2. Skills: a page of its own with enable/disable per role and remove for
   user skills.
3. A fuller harness mock for the Agents view, so row 22 can be tested.

## Tests run

- `cargo test -p backspace-core --lib layout`: 13 passed (defaults now cover 1–6 panes).
- `node --check` on app.js, shell.js, code.js.
- uiharness eval `shell-rail.js`: rows 1–16 PASS; `ade-work.js`: rows 23–31 PASS.
