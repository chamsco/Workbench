# Motion, batch 1 (in progress): tooltips and menus

Captured on Charles's Windows 11 PC, 12 Nov 2026, 100% scaling, 1240x702, dark theme, debug build, seeded sample data folder. No model was contacted.

| File | State |
|---|---|
| `01-plus-menu-100x-slow-sample.png` | The + menu opening with `CAULDRON_MOTION_SLOW=100` (debug only), at about 1s, 5s and 19s of 15s: the panel fades in and rises, its item follows. Mid-frames can only be shot slowed down; at real speed it takes 150ms. |
| `02-plus-menu-settled-sample.png` | The same menu at real speed, settled: opaque, where it was before motion. |

## What moves

- Tokens (`cauldron_chrome::tokens::motion`): menus 150ms, indicators 180ms, panes 200ms, tips 120ms, stagger 25ms, all under the 200ms cap (tested), on cubic-bezier(.32,.72,0,1) (tested: rest to rest, no overshoot).
- Tooltips fade in and settle the last 4px toward their place.
- The + menu and the machine menu fade in and rise 4px; items fade in 25ms apart and all finish together; one hover highlight slides between items.
- Reduced motion: Windows' "Show animations" (SPI_GETCLIENTAREAANIMATION) read at launch sets GPUI's `reduce_motion`, which every tween honours: changes cut, as before.

## Not checked

- Reduced motion with the Windows setting off (changing it is a system setting); the cut path is tested with `set_reduce_motion(true)` in every test window.
- The setting is read at launch only, not when it changes.
- The highlight slide and the machine menu were not shot mid-frame.
- Items 3, 5, 1, 2, 4, 6, 7, 9 are not built yet.
