# GPUI vs Tauri

Two shells over the same in-process harness (`backspace-core`), drawing the
same design (`design/workbench.html`): `crates/app` (GPUI, native) and
`crates/tauri-app` (Tauri 2, the replica's HTML/CSS/JS in the system
webview). Build both in one cargo invocation so they link the identical
harness build:

    cargo build --release -p backspace -p backspace-tauri
    python3 bench/bench.py --runs 3        # writes bench/results.json
    python3 bench/threads.py gpui          # Linux: CPU per thread
    python3 bench/threads.py tauri

`bench.py` launches each app twice per run against a fresh workspace and a
deterministic mock model (`mock.py`, auto-approve on), alternating the
order between runs:

- **idle**: launch → first frame with data (each app writes a timestamp
  when its first frame is on screen), then 10 s at rest.
- **workload**: goal set via `BACKSPACE_GOAL`, 2×2 terminals: the main
  agent plans six tickets in two waves, four workers run in parallel, each
  makes 12 tool calls with shell output, then everything merges.

CPU and memory cover the whole process tree (Tauri's WebKit helpers
included). PSS splits shared pages fairly, so it is the better memory
number on Linux.

## Results: Linux, Xvfb, no GPU (this sandbox)

4 vCPU Xeon 2.8 GHz, Xvfb, Mesa llvmpipe (software Vulkan). Median of 3.

| metric | GPUI | Tauri |
|---|---:|---:|
| binary (MB) | 59.0 | 17.8 (+ system WebKitGTK) |
| startup to first frame (ms) | **391** | 2197 |
| idle CPU (%) | 1.0 | **0.4** |
| idle memory, PSS (MB) | **166** | 226 |
| idle memory, RSS (MB) | **189** | 379 |
| workload CPU (s) | 38.4 | **13.2** |
| workload wall (s) | 15.8 | **11.7** |
| workload peak PSS (MB) | **208** | 300 |

Where the workload CPU went (`threads.py`):

| GPUI | s | Tauri | s |
|---|---:|---|---:|
| `llvmpipe-*` (software GPU driver) | 36.2 (93%) | WebKitWebProcess main thread | 12.3 (94%) |
| app main thread (layout + scene) | 2.2 | app main thread | 0.4 |

## How to read this

- **The workload CPU number here measures Mesa, not GPUI.** 93% of GPUI's
  CPU is llvmpipe emulating a GPU: GPUI redraws the full window every frame
  and expects hardware to rasterise it. WebKitGTK, with its GPU path off,
  uses a CPU raster path with damage tracking, which suits this machine.
  The wall time is longer for GPUI for the same reason: llvmpipe takes the
  cores that the harness and mock need.
- **What carries over to real hardware:** startup (5.6× faster for GPUI),
  memory (about 26% less PSS idle and 31% less at peak), and GPUI's own
  main-thread cost (2.2 s across the run). Whether GPUI also wins on
  workload CPU depends on the GPU taking over llvmpipe's 36 s. That is
  likely, but these numbers do not show it. Run the bench on the Mac
  before deciding.
- **Idle CPU is now even**, but only after one fix. The replica's
  "Agents running" dot pulsed forever, even with nothing running. Every
  animation frame then made WebKit recompute the full-window
  `backdrop-filter` blur, which cost 89% CPU at idle. GPUI drew that dot
  static. Both apps now pulse it only while an agent runs. The lesson for
  the Tauri build: any always-on CSS animation under a large
  `backdrop-filter` is expensive.
- **Not identical in features.** GPUI has no web engine, so its Browser
  shows an HTTP text snapshot with an "Open in browser" link, while Tauri
  embeds the live page. If a live preview inside the app is a requirement,
  this decides it for Tauri, or for GPUI with an external browser window.
- **Input under load:** in this sandbox, synthetic typing (xdotool) into
  Tauri dropped keystrokes while agents streamed; into GPUI it did not.
  This is anecdotal and was not measured.

## On macOS

`bench.py` runs unchanged: memory and CPU come from `ps`, and WebKit's XPC
helpers (`com.apple.WebKit.*`, which are not children of the app) are
counted by name and start time. PSS is not available there, so read RSS.
`threads.py` is Linux only.
