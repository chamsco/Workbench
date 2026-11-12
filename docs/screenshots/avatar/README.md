# Avatars

## 02: flat figure A, in the app

`02-flat-moods-in-app-sample.png`: the eleven moods at 52px, five as outlines, then asking at 40, 28, 22 and 16px. Shot from the real window (debug build, sample data folder, 1240x702, 12 Nov 2026) with the strip placed in the empty Highlights panel for the shot only; that placement was reverted and is not committed. GPUI draws each figure from the SVG that `cauldron_chrome::avatar::svg` writes.

- Ported from the sheet Charles picked A from (`design/cauldron-avatars-and-thread.html` on `claude/project-thread-gg3ogp`): flat head over body in the agent's dot colour at every size, bigger head and smaller body, the cog head for working, three small heads for typing, marks (?, !, z) from 28px up, the outline agent for the chat status line. Mood faces and the cog follow orbling (MIT).
- Still poses only: the cog angle and the typing hop are tested functions of the motion clock, but nothing animates on screen yet; the eye wander, blink and mark draw-on come with the status line.
- In the app today: the agent tabs and the + menu draw the figure (idle, 16px) in place of the dot. The sample data has no agents, so those two spots were not shot.
- Departures from the sheet: the drawn box is taller (y from -12) so marks are not clipped; marks show whole under reduced motion; the z's are paths, not text.
- Radii: head, eyes and minis are circles or pills, under the circles exception.
- Red and green runs: `docs/cauldron/Avatar-flat-red-output.txt`, `Avatar-flat-green-output.txt` (red is the compile failure before the module existed).

## 01: textured spheres (superseded)

`01-early-frame-idle.png`: the first try, seeded textured spheres rendered by headless Edge. Replaced by figure A at Charles's request; the texture code is gone.
