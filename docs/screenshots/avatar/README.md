# Avatars

## 04 to 07: the bust in the thread's status line

`04-thread-writing.png` (Writing: three small heads), `05-thread-waiting.png` (Waiting for the reply: head and shoulders), `06-thread-streaming-text.png` (text arriving, the status line stays under it), `07-status-line-zoom-8x.png` (04 and 05 enlarged). Real window, debug build, 1240x702, a copy of the sample data folder with a running reply added. For the shot only, core was kept from turning that stale reply into Stopped on load (reverted, not committed).

- Crop v2 (`docs/cauldron/BUST-CROP-V2.md` on gg3ogp): slot 18 7 64 64, so the head is a little smaller and the shoulders show to the slot edge; marks keep their on-screen size.
- A reply that is Sending or Streaming draws the bust (outline, the palette's `fg3`) in a 16px slot, then its label in the step lines' 12.5px grey, under any text so far. Step checks now sit in the same 16px slot with the same 8px gap, so the label starts where the step text does (the step text moved 3px right); the test checks the x.
- At 16px the typing mood's three heads read as a dotted bar (07).
- Still frames: the cog spin and typing hop come with the motion clock.
- Runs: `docs/cauldron/Avatar-crop2-red.txt`, `Avatar-crop2-green.txt`, `Thread-bust-red.txt` (no status line yet), `Thread-bust-green.txt`.

## 03: the chat status line agent, a bust

`03-status-line-bust-in-app-sample.png`: two step lines, then status lines with the agent as a bust (sheet 5742272): head plus the top of the shoulders in a 16px square slot (the sheet's 22 9 56 56, since cropped wider; see 04), outline about 1.4px in the step lines' grey (`DARK.fg3` via `Look.ink`), 6px gap (now a shared 16px slot and 8px gap, see 04) so the label starts where the step text does. Marks are about 1.5x bigger and overhang the slot up and right (`avatar::bust_frame` gives the image size and offset). Real window, placed in the Highlights panel for the shot only (reverted, not committed). The status line itself is not built yet; still frames.

- The bust tests were written with the code; the red run (`Avatar-bust-red-output.txt`) is from the first bust try, before `bust` and `ink` existed.

## 02: flat figure A, in the app

`02-flat-moods-in-app-sample.png`: the eleven moods at 48px, five as outlines, then asking at 40, 28, 22 and 16px. Shot from the real window (debug build, sample data folder, 1240x702, 12 Nov 2026) with the strip placed in the empty Highlights panel for the shot only; that placement was reverted and is not committed. GPUI draws each figure from the SVG that `cauldron_chrome::avatar::svg` writes.

- Ported from the sheet Charles picked A from (`design/cauldron-avatars-and-thread.html` on `claude/project-thread-gg3ogp`): flat head over body in the agent's dot colour at every size, bigger head and smaller body, the cog head for working, three small heads for typing, marks (?, !, z) from 28px up, the outline agent for the chat status line. Mood faces and the cog follow orbling (MIT).
- Still poses only: the cog angle and the typing hop are tested functions of the motion clock, but nothing animates on screen yet; the eye wander, blink and mark draw-on come with the status line.
- `px` is the figure height on the sheet's size ladder (its 96-unit box). From 28px up the drawn box grows upward by 16 units so the marks fit, keeping the sheet's scale; below 28px it is the sheet's box exactly (`avatar::size`).
- Eyes are 0.24 of the head diameter (sheet update 0f766b8).
- Not in the app yet: tabs, rows and the + menu still draw the agent dot. Those hunks sit where top bar A rewrites the tabs, so the figure goes in after that branch merges. The status line outline (17px tall, 14px wide, step-line grey) comes with the status line.
- The size test was written with the size fix (no red run).
- Departures from the sheet: the drawn box is taller (y from -12) so marks are not clipped; marks show whole under reduced motion; the z's are paths, not text.
- Radii: head, eyes and minis are circles or pills, under the circles exception.
- Red and green runs: `docs/cauldron/Avatar-flat-red-output.txt`, `Avatar-flat-green-output.txt` (red is the compile failure before the module existed).

## 01: textured spheres (superseded)

`01-early-frame-idle.png`: the first try, seeded textured spheres rendered by headless Edge. Replaced by figure A at Charles's request; the texture code is gone.
