# Motion, batch 1 (in progress): tooltips and menus

Captured on Charles's Windows 11 PC, 12 Nov 2026, 100% scaling, 1240x702, dark theme, debug build, seeded sample data folder. No model was contacted.

| File | State |
|---|---|
| `01-plus-menu-100x-slow-sample.png` | The + menu opening with `CAULDRON_MOTION_SLOW=100` (debug only), at about 1s, 5s and 19s of 15s: the panel fades in and rises, its item follows. Mid-frames can only be shot slowed down; at real speed it takes 150ms. |
| `02-plus-menu-settled-sample.png` | The same menu at real speed, settled: opaque, where it was before motion. |
| `03-machine-menu-highlight-sample.png` | The machine menu at real speed: hover on "This machine", then on "Studio". The highlight lines up with the item and sits behind its text. |

## What moves

- Tokens (`cauldron_chrome::tokens::motion`): menus 150ms, indicators 180ms, panes 200ms, tips 120ms, stagger 25ms, all under the 200ms cap (tested), on cubic-bezier(.32,.72,0,1) (tested: rest to rest, no overshoot).
- Tooltips fade in and settle the last 4px toward their place.
- The + menu and the machine menu fade in and rise 4px; items fade in 25ms apart and all finish together; one hover highlight slides between items.
- Reduced motion: Windows' "Show animations" (SPI_GETCLIENTAREAANIMATION) read at launch sets GPUI's `reduce_motion`, which every tween honours: changes cut, as before.

## Not checked

- Reduced motion with the Windows setting off (changing it is a system setting); the cut path is tested with `set_reduce_motion(true)` in every test window.
- The setting is read at launch only, not when it changes.
- The highlight slide was not shot mid-frame. In my screenshot harness (window driven by posted messages, not focused), slowed runs stop getting frames about 0.3s after the last input: logged at 100x, the machine menu's pop-in reached 0.119 in 58 frames, then nothing for 20s. At real speed, logged with no input, it reaches 1.0 in 10 frames and does not restart over 5s. Why frames stop there is not established.
- The menu and tooltip motion has no unit test of its own; the earlier `Tween` and `lerp` helpers, and their two tests (written with the module, so no red run), were removed as unused. `Motion-green-output.txt` predates that.
- Items 3, 5, 1, 2, 4, 6, 7, 9 are not built yet.
