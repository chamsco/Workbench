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
| `10-tabs.png` | UI1 remainder: five tabs on the frame, the open one joining the card with curved feet |
| `11-tip-right-rail.png`, `12-tip-function-row.png` | UI1 remainder: hover tips, left of the right rail and right of the function row |

Dark only: the app follows the Windows theme and has no switch of its own. The light screenshots need the OS theme changed, which was not done.

## Fixed in this step

1. Nine icons drew as empty boxes (gauge, brain, layout-grid, pin, diff, terminal, columns-2, square, x). They are not in gpui-kit's default icon bundle; `icons.rs` now falls back to the full catalog. Test: `icons::tests::every_shell_icon_loads`.
2. Minimise, maximise, close, the sidebar button, Terminal and Split all answered Windows as the drag strip, so a real click there would have moved the window instead. The buttons in the top band now stop the hit test (`occlude`, only there, so scrolling over buttons in lists is not blocked later). Check: `tools/winshell/hit-test.ps1`, 11 of 11. Its points are hard-coded for 100% scaling.

## Fixed in the UI1 remainder

Folder tabs (open, switch, close, + ; closing the last tab leaves one New thread) with concave feet joining the card; with 12 tabs they shrink to fit and + stays in the row (past about 14 they get too thin to read; scrolling is not built). Every icon button has its name as a tip beside it, as the wireframe's `.fn[data-tip]`: 8px away, centred, after 450ms, at once for a neighbour within 300ms. Tips on the top band and + open below, which the wireframe does not draw. Live tests compare every pane's rendered bounds with `layout::solve` in 8 shell states. Tests in `shell::tests`, red and green runs in `docs/cauldron/UI1-red-output.txt` and `UI1-green-output.txt`.

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
