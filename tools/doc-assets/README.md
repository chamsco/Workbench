# How the README images are made

Everything under [`docs/assets/`](../../docs/assets) is generated from files in
this repo. Nothing is a hand-taken screenshot that will silently rot.

| Image | Where it comes from |
|---|---|
| `how-it-works.svg` | hand-authored, animated SVG — one sentence in, many agents, one diff out |
| `the-gate.svg` | hand-authored, animated SVG — what happens to a single ticket |
| `decision-ladder.svg` | hand-authored, animated SVG — where a ticket starts, and when it escalates |
| `architecture.svg` | hand-authored, animated SVG — three front doors, one harness |
| `app-ade.png` | the real UI — the ADE workspace, with the prompt composer and the first agent card |
| `tour.gif` | the **real app**: three beats captured during one scripted run |
| `app-review-plan.png` | the real app — a plan awaiting approval, diagram first |
| `app-canvases.png` | the real app — four agents at once (the tour GIF's first beat, kept as a still) |
| `app-wall.png` | the real app — the Wall, every agent tiled |
| `app-picker.png` | the real app — what a canvas can show |

The stills are 1440×900 at 1× because that is the app's own window. The GIF is
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

### When the window won't open

The CDP route above needs a real WebView2 window, and WebView2 starts by shelling
out to `reg.exe` for its browser policy. In a locked-down sandbox that call is
blocked, the shell exits immediately, and no amount of retrying helps — the error
is `PROGRAM BLOCKED BY SECURITY POLICY`.

For those cases there is [`tools/uiharness`](../uiharness): it loads the *real*
`crates/tauri-app/ui` into headless Chromium with a `preload.js` that stands in for
the Tauri IPC. Same components, same CSS, same bridge — only the Rust side is
mocked. Chat-box stills, when we need them, come from this path, because the live
window could not be opened in the sandbox we used.

```sh
node tools/uiharness/run.mjs --eval-file seed.js --shot out.png --size 1440x1080
```

Three things bite when you seed a state this way:

- **Mock before the first frame.** Some commands are read once at mount —
  `chat_list` is called from `setHost`. An `--eval-file` runs after that, so mock
  the command *and* then call `window.__H.fire("state", {})`, which makes the app
  ask again.
- **A programmatic `.click()` is not a user click.** It skips the app's own
  home → thread transition, which is what dims the painted backdrop. Call
  `window.Art.setVivid(false)` yourself or the art stays loud behind the text.
- **The transcript scrolls itself to the bottom.** Size the viewport so the whole
  exchange fits; setting `scrollTop = 0` just clips the last message instead.

### Making the GIF

```powershell
./app-tour.ps1 -Frames "../../target/app-shots/01-canvases.png,../../target/app-shots/02-wall.png,..."
```

Each still is held, then crossfaded into the next: beats at 2 s with a 0.35 s
fade, 12 fps, encoded with `palettegen/stats_mode=diff` and
`paletteuse/diff_mode=rectangle` — that last option is what keeps four
1440×900 beats under 2 MB.

## The animated SVGs

`how-it-works.svg`, `the-gate.svg`, `decision-ladder.svg` and `architecture.svg`
are written by hand — they have to be, because each one is the argument of the
section it sits in. If you edit one, keep the rules they were built with:

- **They play once and settle.** `animation-iteration-count: 1` with
  `animation-fill-mode: forwards`, so the last keyframe is the finished diagram.
  A loop would leave the picture half-built most of the time, and blank at the
  moment someone scrolls past.
- Only `transform`, `opacity` and `stroke-dashoffset` are animated.
- One strong ease-out, staggered per node; nothing moves after it lands.
- Drawable paths carry `pathLength="1"`, so a dash-offset of 1 hides them exactly.
- Motion is presentational only: `prefers-reduced-motion` resolves to the
  finished diagram, which carries the same information.
- No arrowheads on curved fan lines — a marker paints even while its path is
  dash-hidden, and four of them converge into a blob.

To look at a frame without a browser open, `target/svgcheck/render-svg.mjs` (a
scratch script, not part of the build) renders any of them at a chosen time with
Playwright.

**Don't verify them with a full-page screenshot.** A `fullPage: true` capture
re-creates the image document for every SVG on the page, which resets its CSS
animation clock to zero — you photograph the blank first frame and conclude the
diagram is broken. Scroll to the section and shoot the viewport instead.

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
