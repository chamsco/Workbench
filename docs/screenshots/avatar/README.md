# Avatars, early frame (not in the app yet)

`01-early-frame-idle.png`: six agents (seeds 11, 42, 7, 99, 1234, 5) at 96px and 32px, idle face. Rendered from the SVG that `cauldron_chrome::avatar::svg` writes, by headless Edge on Charles's Windows 11 PC, 12 Nov 2026. GPUI renders SVG through resvg, so the in-app look still has to be checked.

- A textured head sphere over a body sphere, generated in code from a seed: the body colour is the agent's dot (`AGENT_DOTS`), paint patches come from seeded noise, then light and shade. No image files; the reference photos were not copied.
- The rounded face is the only part that will change with mood; the moods follow orbling (github.com/kochjar/orbling, MIT). Only Idle exists so far, waiting on the owner's look at this frame.
- Colours come only from tokens (`tokens::avatar` plus `AGENT_DOTS`), tested.
- Red and green runs: `docs/cauldron/Avatar-red-output.txt`, `Avatar-green-output.txt`.
