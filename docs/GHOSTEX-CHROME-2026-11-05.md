# Ghostex chrome, borrowed into the quiet shell

> Superseded in part by `docs/UNSLOP-2026-11-05.md`: the command row and bottom
> rail stay, but the floating pill chrome was flattened into the framed sheet.

Reference: [maddada/Ghostex](https://github.com/maddada/Ghostex) (Rust/GPUI).
The parts worth taking were the chrome, not the engine:

- **One centered icon row** in the titlebar for navigation. Ghostex keeps its
  titlebar actions as a row of small icons; we keep the four modes, five work
  links and three places in one flat row centered over the content, with a
  2px active underline instead of a floating pill.
- **A second rail at the bottom.** Ghostex leaves machine, usage and session
  status along the bottom edge and keeps utilities close to it. Our status
  strip is now a real second rail: machine state on the left, Settings,
  Light/Night, Glass, Help and the account on the right.
- **Glass as one material, not stacked tints.** Ghostex paints a shared
  blurred backdrop, then a separate tint per layer (sidebar vs work area), so
  the tints never double up. Cards and rows are thin washes over that same
  scene. We do the same in CSS:
  - `#win.q-glass::before` paints `art/harbour-dusk.webp`, blurred.
  - `#win.q-glass::after` adds the global depth tint.
  - `.mainc` and `.side` get separate tints with `backdrop-filter`.
  - cards and rows are 4–5% white washes and do not blur again.
  - the window gets a 14px radius and a bright inner edge.
- **Chat rhythm.** The Code session's composer now carries the Ghostex-style
  context strip above the input: needs, tokens, board, workers and task
  progress. The transcript stays a centered 900px column with work rows,
  plain assistant prose and a quiet composer, like Ghostex's chat view.

## Where it lives

| Piece | File |
| --- | --- |
| Centered command row (`#qNav`) | `ui/quiet.js` `renderNav`, `renderTop`; `ui/quiet.css` `.qnav` |
| Bottom utility rail (`#qDock`) | `ui/quiet.js` `renderDock`; `ui/index.html` `.status`; `ui/quiet.css` `.qdock` |
| Glass backdrop and tints | `ui/quiet.css` `/* ---------- glass ---------- */` |
| Chat context strip | `ui/quiet.js` `renderSession`; `ui/quiet.css` `.qs-context` |

## What is deliberately different

- This is the webview shell, not GPUI. Real desktop blur behind the window
  would need the platform window to be transparent and composited by the OS;
  the quiet shell instead paints a shared, blurred scene inside the window and
  keeps every surface translucent over it. That keeps the structure and the
  feel without depending on native blur.
- The command row is centered, as requested, rather than right-aligned like
  Ghostex's titlebar actions.
- The bottom rail keeps the app's own machine and theme actions; it does not
  copy Ghostex's usage counters.

## Verification

```sh
node tools/uiharness/run.mjs --chrome <chrome> \
  --eval-file tools/uiharness/evals/quiet-demo.js \
  --eval-file tools/uiharness/evals/quiet-chrome.js
```

`quiet-chrome.js` checks that the command row is in the top bar, centered, one
horizontal row, with four modes and five links; that the bottom rail has
Settings, night mode, Glass and the account; that night mode and glass switch;
and that the glass scene is painted. Screenshots:
`docs/screenshots/2026-11-05/home.png`, `glass-today.png`, `glass-code.png`,
`code.png` and `today-narrow.png`.
