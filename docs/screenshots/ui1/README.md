# UI1 step 0: the GPUI shell on Windows against the wireframe

Captured on Charles's Windows 11 PC, 11 Nov 2026, at 100% scaling, client area 1240x702 (the window's invisible 8px resize borders cropped). App: `cargo build -p backspace --release`, no compile or link errors (4m30s, reusing an existing `target/`; a clean build was not timed). Wireframe: `design/cauldron-master-layout.html` rendered in headless Edge at the same size.

| File | State |
|---|---|
| `wireframe-master-{dark,light}.png` | The spec, master layout |
| `wireframe-newthread-{dark,light}.png` | The spec, New thread |
| `01-default-highlights.png` | App start: sidebar open, drawer on Highlights |
| `02` to `05` | Drawer on Usage, Koi preview, Code review, Patches |
| `06-drawer-closed.png` | Patches clicked again |
| `07-split.png`, `08-split-and-drawer.png` | Split, then Split with Highlights |
| `09-sidebar-hidden.png` | Sidebar hidden, function icons on the left rail |
| `13`, `14`, `15-agent-tabs-{3,8,14}-fixture.png` | Agent tabs (wireframe round 23) with 3, 8 and 14 agents. **Fixture**: a debug build with `CAULDRON_FIXTURE_AGENTS=n`; real agents come from core in UI4 |
| `16-plus-menu-fixture.png` | The + menu over the 3-agent fixture: Spawn a subagent (wired in UI4), Open 1:1 with |
| `11-tip-right-rail.png`, `12-tip-function-row.png` | UI1 remainder: hover tips, left of the right rail and right of the function row |

Dark only: the app follows the Windows theme and has no switch of its own. The light screenshots need the OS theme changed, which was not done.

## Fixed in this step

1. Nine icons drew as empty boxes (gauge, brain, layout-grid, pin, diff, terminal, columns-2, square, x). They are not in gpui-kit's default icon bundle; `icons.rs` now falls back to the full catalog. Test: `icons::tests::every_shell_icon_loads`.
2. Minimise, maximise, close, the sidebar button, Terminal and Split all answered Windows as the drag strip, so a real click there would have moved the window instead. The buttons in the top band now stop the hit test (`occlude`, only there, so scrolling over buttons in lists is not blocked later). Check: `tools/winshell/hit-test.ps1`, 11 of 11. Its points are hard-coded for 100% scaling.

## Fixed in the UI1 remainder

Tabs show one thread's agents (wireframe round 23). The coordinator is pinned at the left, at most 190px, never scrolls and cannot be closed; on the New thread screen it is a single "New thread" tab with no +. Agent tabs share the room equally between 72 and 210px. When 72px tabs no longer fit, two 24x28 arrows appear left of + and the row scrolls by one tab per click or wheel line. The open tab is scrolled into view when it changes (click, Left, Right, Home, End, Delete, a 1:1 opened from +, a resize). Delete closes the open agent tab; the agent goes back to the + menu. Close buttons show on the open tab and on hover; narrow tabs show the full name as a tip. Spacing inside tabs and the + menu is the wireframe's `.tab`, `.tscroll` and `.tmenu` values (gap 7, padding 10/6, item 6/8), off the 4px scale by design. Every icon button has its name as a tip beside it, as the wireframe's `.fn[data-tip]`: 8px away, centred, after 450ms, at once for a neighbour within 300ms. Tips on the top band and + open below, which the wireframe does not draw. Live tests compare every pane's rendered bounds with `layout::solve` in 8 shell states. Tests in `shell::tests`, red and green runs in `docs/cauldron/UI1-red-output.txt` and `UI1-green-output.txt`.

## Mismatches still open, most visible first

1. Everything inside the panes is a placeholder (thread list, transcript, composer, stage bar, drawer contents). UI3, UI4, UI6.
2. Frame texture and corner glow missing. Planned as one baked image.
3. Selected right-rail button: the wireframe tints the icon with the accent; the app uses plain foreground.
4. Search box: the wireframe fills it with the raised colour; the app draws an outline only.
5. Window corners: the wireframe rounds the window; the app window is square on Windows.
6. Notch: the app uses the 62px Windows notch (no traffic lights), as the plan assumes. The wireframe draws the Mac lights.

## Not checked

- `CAULDRON_LEGACY=1`: the legacy window exits at start because branch `backspace/run` is still checked out by an old worktree under `target/app-demo/ws/`. `main.rs` was not changed in UI1; the worktree was left alone.
- 125% and 150% display scaling: needs a Windows display setting changed.
- Dragging and resizing with a real mouse: hit-testing is verified (drag strip answers caption, edges answer resize), the drag itself was not performed.
- Console window: the release build sets `windows_subsystem = "windows"`, so none; not observed separately.
