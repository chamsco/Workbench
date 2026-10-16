# How the README images are made

Everything under [`docs/assets/`](../../docs/assets) is generated from files in
this repo. Nothing is a hand-taken screenshot that will silently rot.

| Image | Where it comes from |
|---|---|
| `how-it-works.svg` | hand-authored, animated SVG — the hero diagram |
| `tour.gif` | the **real app**: four beats captured during one scripted run |
| `app-canvases.png` | the real app — four agents at once, mid-run |
| `app-wall.png` | the real app — the Wall, every agent tiled |
| `app-picker.png` | the real app — what a canvas can show |
| `app-review-plan.png` | the real app — a plan awaiting approval, diagram first |

The stills are 1440×900 at 1× because that is the app's own window; the GIF is
downscaled to 1000 px wide on the way out.

## Requirements

- The release Tauri shell: `cargo build --release -p backspace-tauri`
- Python (for `bench/mock.py`) and Node 22+ (for the CDP client)
- `ffmpeg` on `PATH`, or Playwright's bundled build (used as a fallback)
- A Chromium-family browser, only for the design-mock captures below

## Capturing the app

`run-app.ps1` sets up a throwaway world and drives the real app inside it:

- its own workspace, `BACKSPACE_PREFS` and `BACKSPACE_DATA`, so your real
  `~/.config/backspace` and your projects are untouched;
- a `backspace.toml` with a **deterministic mock model** (`bench/mock.py`) that
  plans six tickets in two waves and runs each worker through a fixed number of
  tool calls — so every capture is reproducible and needs no API key;
- `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`, so the
  WebView2 window can be driven over Chrome DevTools Protocol.

```powershell
./run-app.ps1 -Plan app-plan.json -Out ../../target/app-shots
./run-app.ps1 -Dump -Text                 # what is on screen and what is clickable
./run-app.ps1 -Gated -Plan review-plan.json   # stop at the plan, for the review still
```

`-Gated` sets `auto_approve = false` and `plan_approval = true`. That matters:
the diagram only has something to draw once a review is genuinely pending, so an
auto-approved run produces an empty one. The harness gates the plan on exactly
that pair — `harness.rs` reads
`wf.plan_approval && !cfg.orchestrator.auto_approve`.

Two things about the launch are deliberate:

- It does **not** set `BACKSPACE_READY_FILE` or `BACKSPACE_LAYOUT`. Those put
  the shell into its bench mode, which unshifts a tab literally named "Bench" —
  fine for timing, wrong for a screenshot.
- The 2×2 starting layout goes in `tabs_by_project`, keyed by `Prefs::tab_key()`.
  `Prefs::tabs_for()` answers a project with no entry of its own using
  `TabSpec::defaults()`, *not* the global `tabs`, so a global layout is ignored.

### Driving it

`cdp-shoot.mjs` connects to the WebView2 debug port and runs a JSON plan:

```json
[
  { "waitFor": "document.body.innerText.includes('Working')", "timeout": 45000 },
  { "shot": "01-canvases" },
  { "clickSel": ".vbtn", "settle": 900 },
  { "clickSel": "[data-wb='wall']", "settle": 2800 },
  { "shot": "02-wall" }
]
```

| Step | Effect |
|---|---|
| `{"shot": "name"}` | capture to `<out>/<name>.png` |
| `{"click": "Wall"}` | click the smallest visible element whose text is exactly this |
| `{"clickContains": "Diagram review"}` | same, substring match |
| `{"clickSel": ".chooser [data-k='agent']"}` | click a CSS selector |
| `{"eval": "expr"}` | evaluate and print |
| `{"waitFor": "expr", "timeout": ms}` | poll until truthy |
| `{"wait": ms}` | sleep |
| `{"list": true}` | print every visible clickable element and its box |

Prefer `clickSel` where you can. Text matching picks the *smallest* element
containing the string, which is right for a menu item and wrong for a chip inside
a pane that also has a header with the same words.

### Making the GIF

```powershell
./app-tour.ps1 -Frames "../../target/app-shots/01-canvases.png,../../target/app-shots/02-wall.png,..."
```

Each still is held, then crossfaded into the next: beats at 2 s with a 0.35 s
fade, 12 fps, encoded with `palettegen/stats_mode=diff` and
`paletteuse/diff_mode=rectangle` — that last option is what keeps four
1440×900 beats under 2 MB.

## The hero SVG

`docs/assets/how-it-works.svg` is written by hand — it has to be, because it is
the argument the README opens with. If you edit it, keep the rules it was built
with:

- Only `transform` and `opacity` are animated, so it stays cheap.
- The entrance is staggered per node and uses one strong ease-out.
- Motion is presentational only: `prefers-reduced-motion` resolves to the
  finished diagram, which carries the same information.
- One shared 7 s cycle, with the fade-out at the same percentage in every
  keyframe, so the loop never drifts out of phase.

## The design mock

`design/workbench.html` is still the clickable design both shells reproduce, and
`shoot.ps1` + `tour-driver.js` can photograph any state of it without a browser
open — useful for checking the design against the app:

```powershell
./shoot.ps1 -Html ../../design/workbench.html -Out out/plan.png `
  -Inject tour-driver.js -Query "env=diagram&review=plan&theme=dark&nospin=1" -Scale 2
```

The mock is a fragment with no `</body>`, so `shoot.ps1` appends the driver
rather than inserting it. Query keys: `env`, `side`, `review`, `btab`, `theme`,
`cursor=x,y`, `ring=0..1`, `nospin=1`. No README image uses this path any more —
the images are the real app — but the mock is the fastest way to see a screen the
app cannot easily be put into.
