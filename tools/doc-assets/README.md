# How the README images are made

Everything under [`docs/assets/`](../../docs/assets) is generated from files in
this repo. Nothing is a hand-taken screenshot that will silently rot.

```text
how-it-works.svg         hand-authored, animated SVG — the hero diagram
tour.gif                 generated: an eight-second tour of design/workbench.html
app-design.png           generated: the design, 2x2 canvases
app-review-plan.png      generated: the design, the Diagram tab on the plan review
app-tickets.png          generated: the design, the Tickets view
app-real-run.png         a real session of the Tauri shell (mock model)
app-real-review.png      a real session, the diagram-first review in light mode
```

## Requirements

- A Chromium-family browser. `shoot.ps1` looks for Playwright's Chromium under
  `%LOCALAPPDATA%\ms-playwright`, then Edge, then Chrome. Override with
  `$env:DOC_ASSETS_CHROME`.
- `ffmpeg` on `PATH` for the GIF (Playwright's bundled build is used as a
  fallback).

## Stills

```powershell
./shoot.ps1 -Html design/workbench.html -Out ../../docs/assets/app-design.png
```

The mock is a fragment with no `</body>`, so `shoot.ps1` appends the injected
driver instead of inserting it. `-Scale 2` renders at 2× for a crisp image.

## Driving a screen

`tour-driver.js` is injected into a copy of `design/workbench.html` (the original
is never modified). It reads the query string and drives the mock's own state:

| Key | Effect |
|---|---|
| `env` | `terminals`, `browser`, `diagram`, `docs`, `new` — same as clicking a tab |
| `side` | `projects`, `agents`, `tickets` — the sidebar segment |
| `review` | `plan`, `api`, `final` — which review the Diagram tab shows |
| `btab` | which preview the Browser tab shows |
| `theme` | `light`, `dark`, `auto` |
| `cursor=x,y` | draws the pointer |
| `ring=0..1` | draws a click ripple at that phase |
| `nospin=1` | freezes the mock's fake spinner |

So this is reproducible from nothing but the repo:

```powershell
./shoot.ps1 -Html design/workbench.html -Out out/plan.png `
  -Inject tour-driver.js -Query "env=diagram&review=plan&theme=dark&nospin=1" -Scale 2
```

## The tour GIF

```powershell
./tour.ps1
```

`tour.ps1` holds the storyboard — Terminals → Tickets → Diagram → `PLAN.md` →
back to Terminals, so the loop is seamless. For each frame it computes which
state the mock should be in and where the pointer is (interpolated with the same
strong ease-out the UI uses, `cubic-bezier(.23, 1, .32, 1)`), renders one frame
through `shoot.ps1`, then encodes the sequence with `ffmpeg` using
`palettegen/stats_mode=diff` and `paletteuse/diff_mode=rectangle`. That last
option is what keeps a mostly-static 1000×625 loop at **under 1 MB**.

Frames land in `target/tour-frames/` and are deleted afterwards; pass
`-KeepFrames` to inspect them.

## The hero SVG

`docs/assets/how-it-works.svg` is written by hand — it has to be, because it is
the argument the README opens with. If you edit it, keep to the rules it was
built with:

- Only `transform` and `opacity` are animated, so it stays cheap.
- The entrance has a 30–80 ms stagger per node and uses one strong ease-out.
- Motion is presentational only: `prefers-reduced-motion` resolves to the
  finished diagram, which carries the same information.
- One shared 7 s cycle, with the fade-out at the same percentage in every
  keyframe, so the loop never drifts out of phase.
