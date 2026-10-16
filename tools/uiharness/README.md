# uiharness

Load the Backspace desktop UI (`crates/tauri-app/ui`) in headless Chromium so an
agent can read **real computed layout** and take **screenshots** — no Tauri, no
window, no desktop session.

Zero dependencies. Node's `http`, `fs` and global `WebSocket` are the whole
toolchain. Nothing in this directory writes to `crates/`.

```
node tools/uiharness/run.mjs [flags]
```

## What it does

1. Serves `crates/tauri-app/ui` on an ephemeral `127.0.0.1` port, with correct
   content types for html/css/js/mjs/json/svg/png/woff2/ttf.
2. Serves one virtual route, `/__harness/index.html`: the real `index.html` with
   `<base href="/">` and `preload.js` inlined as the **first script in `<head>`**.
   The base is what lets a page served from `/__harness/` still resolve the app's
   relative `style.css` / `chat-web/whirl.js` the way it does at the asset root.
   `index.html` on disk is never touched.
3. Launches Chromium headless with `--remote-debugging-port=0`, reads the chosen
   port out of `DevToolsActivePort`, and drives it over CDP with a small client of
   its own (`cdp.mjs`).
4. Waits for `window.__HARNESS_READY === true`, then for the app's own boot class
   to clear (`.win.booting`) and for DOM mutations to stop, so evals and
   screenshots see a settled screen.
5. Runs your `--eval` / `--eval-file` expressions in order and/or screenshots.

Page console output is echoed to **stdout** with `[console]`, `[console.error]`,
`[console.warn]`, `[pageerror]` prefixes; the harness's own trace goes to
**stderr** with `[http]`, `[cdp]`, `[chrome]`, `[harness]`, `[shot]`. Errors in
the page (uncaught exceptions, `console.error`, failed subresources) make the run
exit non-zero and are listed at the end.

## Flags

| Flag | Meaning |
| --- | --- |
| `--shot <path.png>` | Full-page screenshot (`Page.captureScreenshot`, `captureBeyondViewport: true`) written to `path.png`, directories created as needed. |
| `--size <W>x<H>` | Viewport via `Emulation.setDeviceMetricsOverride`. Default `1440x900`. |
| `--eval <js>` | Evaluate after ready; prints `RESULT: <json>`. Repeatable, printed in the order given. |
| `--eval-file <path>` | Same, expression read from a file. Repeatable, mixes freely with `--eval`. |
| `--wait <ms>` | Extra settle time after ready, before evals/screenshots. Default `600`. |
| `--url <path>` | Navigate to an app path instead of `/__harness/index.html`, e.g. `--url /index.html`. |
| `--shot-mode full\|viewport` | `full` (default) uses `captureBeyondViewport`; `viewport` captures only the visible box. For chasing capture oddities. |
| `--chrome <path>` | Chromium binary. Default the Playwright one; `UIHARNESS_CHROME` also works. |
| `--ui-root <path>` | Static UI root. Default `crates/tauri-app/ui`. |
| `--keep` | Leave the browser and temp profile behind. |
| `--quiet` | Drop the `[http]`/`[cdp]` trace. Console/page errors still print. |

Exit code is `0` only when the page booted clean and nothing threw.

## The Tauri stub

`preload.js` runs before the app's own scripts and installs a `window.__TAURI__`
good enough to boot: `core.invoke`, `event.listen`, `window.getCurrentWindow`,
plus `__TAURI_INTERNALS__`. `invoke` records every call in `window.__CALLS` and
answers with sensible defaults — `prefs` (with `tabs`, `active_tab`),
`boot` (`{platform:"win", layout:0, user:"tester"}`), a minimal `snapshot`,
`[]` for list commands, `null` for setters and chat/agent/memory verbs.

From a `--eval` you can take over any command or drive app events:

```js
window.__H.mock("snapshot", () => ({ name: "demo", workspace: "/tmp/ws", agents: [], approvals: [], tickets: [], link: { state: "off" }, index: 0 }));
window.__H.fire("state", {});          // every registered "state" listener
window.__H.calls().map(c => c.cmd);    // what the app has asked for so far
await window.__H.frames(2);            // wait two animation frames
```

## Examples

**1. Did the app boot, and what did it ask for?**

```bash
node tools/uiharness/run.mjs --eval "({ title: document.title, ready: window.__HARNESS_READY, win: document.querySelector('#win').className, list: !!document.querySelector('#list'), calls: window.__H.calls().map(c => c.cmd) })"
```

**2. A computed style, measured rather than assumed**

```bash
node tools/uiharness/run.mjs --eval "(() => { const w = document.querySelector('#win'); const cs = getComputedStyle(w); return { cols: cs.gridTemplateColumns, sideW: cs.getPropertyValue('--side-w').trim(), transition: cs.transitionProperty, box: w.getBoundingClientRect().width }; })()"
```

Or a whole report in one go, including which stylesheets loaded and how many
rules each contributes:

```bash
node tools/uiharness/run.mjs --eval-file tools/uiharness/evals/computed-style.js
```

**3. A screenshot**

```bash
node tools/uiharness/run.mjs --shot tools/uiharness/out/boot.png
```

Different window size, extra settle time, and a screenshot after the eval:

```bash
node tools/uiharness/run.mjs --size 1280x800 --wait 1200 --eval "document.documentElement.dataset.theme" --shot tools/uiharness/out/light-1280.png
```

**4. Measure a collapse, before and after, with frames in between**

`evals/ghost-collapse.js` is the worked example: it reads the sidebar width and
the window's grid tracks, adds the `ghost-side` class the app's ghost-chat toggle
sets, waits two animation frames (mid-transition, which is the interesting part),
reads again, and waits for the transition to land.

```bash
node tools/uiharness/run.mjs --eval-file tools/uiharness/evals/ghost-collapse.js
```

Write your own in the same shape — an async IIFE that `await`s frames and returns
a plain object:

```js
(async () => {
  const el = document.querySelector('#side');
  const frames = n => new Promise(r => { const t = () => (--n <= 0 ? r() : requestAnimationFrame(t)); requestAnimationFrame(t); });
  const before = getComputedStyle(el).width;
  document.querySelector('#win').classList.add('ghost-side');
  await frames(2);
  const during = getComputedStyle(el).width;
  await new Promise(r => setTimeout(r, 400));
  return { before, during, settled: getComputedStyle(el).width };
})()
```

## Notes and traps

- **Measure the sidebar, don't assume it.** The shell owns its own open/closed
  state and flips it while the first screen renders, so `#side` can be a `0px`
  column at one moment and a ~206px one shortly after. If a measurement depends
  on it, set the state yourself at the top of the eval
  (`#win.classList.toggle("side-closed", false)`), wait out the 0.28s transition,
  and only then read. `evals/ghost-collapse.js` does exactly this.
- **Transitions are frozen while a screenshot is taken.** This UI animates its
  own layout (the sidebar collapse is a `grid-template-columns` transition), and
  `captureBeyondViewport` re-lays-out the page only to capture it, which can land
  on a frame the app was merely passing through. `run.mjs` grows the emulated
  viewport to the content height and disables transitions/animations for the
  exposure, then puts both back. Use `--shot-mode viewport` to compare.
- **The UI is under active development** by other agents, so a run can fail
  because of a file that changed between two runs. Re-run before believing a
  one-off failure.
- Waiting for ready is bounded: 30s for `__HARNESS_READY`, then 15s for
  `.win.booting` to clear, then up to 10s for DOM mutations to stop. On a ready
  timeout the harness dumps the URL, title, the window's classes and the
  collected console output before exiting non-zero.
- Chromium prints a `sandbox_win.cc` "Sandbox cannot access executable" error and
  a "Network service crashed ... restarting" line on this machine. Both are
  harmless here: the browser comes up and serves the page.

## Files

| File | Role |
| --- | --- |
| `run.mjs` | Entry point: flags, server, browser, CDP wiring, evals, screenshots. |
| `server.mjs` | Static file server plus the `/__harness/index.html` virtual route. |
| `preload.js` | The Tauri stub injected as the page's first script. Read from disk at startup. |
| `bootstrap.js` | Installed with `Page.addScriptToEvaluateOnNewDocument`, i.e. at document start. |
| `cdp.mjs` | Minimal CDP client over Node's global `WebSocket`. |
| `evals/computed-style.js` | Example: computed styles, custom properties, loaded stylesheets. |
| `evals/ghost-collapse.js` | Example: the sidebar collapse, measured before/during/after. |
| `out/` | Where the examples put screenshots. |
