#!/usr/bin/env node
// uiharness/run.mjs — load the Backspace desktop UI in headless Chromium so an
// agent can measure real computed layout and take screenshots.
//
//   node tools/uiharness/run.mjs [flags]
//
// No dependencies: a node:http file server over crates/tauri-app/ui, a stub
// Tauri runtime injected as the page's first script, and a hand-rolled CDP
// client over Node's global WebSocket.
//
// Flags: --shot --size --eval --eval-file --wait --url
// See tools/uiharness/README.md.
import { spawn } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync, existsSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { startServer } from "./server.mjs";
import { Cdp, sleep, waitFor } from "./cdp.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "..", "..");
const UI_ROOT = join(REPO, "crates", "tauri-app", "ui");
const DEFAULT_CHROME = "C:\\Users\\chamy\\AppData\\Local\\ms-playwright\\chromium-1248\\chrome-win64\\chrome.exe";
const DEFAULT_PATH = "/__harness/index.html";

const HELP = `uiharness — headless Chromium for the Backspace desktop UI

  node tools/uiharness/run.mjs [flags]

  --shot <path.png>     full-page screenshot written to <path.png>
  --size <W>x<H>        viewport size (default 1440x900)
  --eval <js>           evaluate an expression after ready, print RESULT: <json>
                        (repeatable; results print in the order given)
  --eval-file <path>    same, with the expression read from a file (repeatable)
  --wait <ms>           extra settle time after ready, before evals (default 600)
  --url <path>          navigate to this app path instead of ${DEFAULT_PATH}
  --shot-mode full|viewport
                        full = captureBeyondViewport (default), viewport = the
                        visible box only; for tracking down capture oddities
  --chrome <path>       Chromium binary (default ${DEFAULT_CHROME})
  --ui-root <path>      static UI root (default crates/tauri-app/ui)
  --keep                leave the browser and the temp profile behind
  --quiet               do not print the [http]/[cdp] trace
  -h, --help            this text
`;

// ------------------------------------------------------------------ cli
function parseArgs(argv) {
  const opts = {
    steps: [],
    wait: 600,
    url: DEFAULT_PATH,
    width: 1440,
    height: 900,
    shotMode: "full",
    chrome: process.env.UIHARNESS_CHROME || DEFAULT_CHROME,
    uiRoot: UI_ROOT,
    keep: false,
    quiet: false,
  };
  const need = (flag, i) => {
    if (i + 1 >= argv.length) throw new Error(`${flag} needs a value`);
    return argv[i + 1];
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const val = () => {
      const v = need(a, i);
      i++;
      return v;
    };
    switch (a) {
      case "--shot": {
        const p = val();
        opts.steps.push({ kind: "shot", path: p });
        break;
      }
      case "--shot-mode": {
        const m = val();
        if (m !== "full" && m !== "viewport") throw new Error("--shot-mode wants full or viewport");
        opts.shotMode = m;
        break;
      }
      case "--eval":
        opts.steps.push({ kind: "eval", label: "<--eval>", code: val() });
        break;
      case "--eval-file": {
        const p = val();
        const file = isAbsolute(p) ? p : resolve(process.cwd(), p);
        if (!existsSync(file)) throw new Error(`--eval-file: no such file: ${file}`);
        opts.steps.push({ kind: "eval", label: file, code: readFileSync(file, "utf8") });
        break;
      }
      case "--size": {
        const m = /^(\d+)\s*[x×]\s*(\d+)$/i.exec(val().trim());
        if (!m) throw new Error("--size wants <W>x<H>, e.g. --size 1280x800");
        opts.width = +m[1];
        opts.height = +m[2];
        break;
      }
      case "--wait":
        opts.wait = Math.max(0, +val());
        break;
      case "--url":
        opts.url = val();
        break;
      case "--chrome":
        opts.chrome = val();
        break;
      case "--ui-root":
        opts.uiRoot = resolve(val());
        break;
      case "--keep":
        opts.keep = true;
        break;
      case "--quiet":
        opts.quiet = true;
        break;
      case "-h":
      case "--help":
        opts.help = true;
        break;
      default:
        throw new Error(`unknown flag: ${a}`);
    }
  }
  return opts;
}

// ------------------------------------------------------------------ chromium
function launchChromium({ chrome, userDataDir, log }) {
  if (!existsSync(chrome)) throw new Error(`Chromium not found: ${chrome}\n(pass --chrome <path>)`);
  const args = [
    "--headless=new",
    "--remote-debugging-port=0",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-gpu",
    `--user-data-dir=${userDataDir}`,
    "--hide-scrollbars",
    "--disable-features=Translate,MediaRouter,OptimizationHints",
    "--no-service-autorun",
    "--disable-background-networking",
    "--disable-sync",
    "--metrics-recording-only",
    "--mute-audio",
    "about:blank",
  ];
  log(`[chrome] ${chrome} ${args.filter((a) => a !== "about:blank").join(" ")}`);
  const child = spawn(chrome, args, { stdio: ["ignore", "pipe", "pipe"], windowsHide: true });
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  const tail = [];
  const keep = (s) => {
    for (const line of String(s).split(/\r?\n/)) {
      if (!line.trim()) continue;
      tail.push(line);
      if (tail.length > 40) tail.shift();
      if (/ERROR|FATAL|Failed|error:/.test(line)) log(`[chrome] ${line}`);
    }
  };
  child.stdout.on("data", keep);
  child.stderr.on("data", keep);
  child.on("error", (err) => log(`[chrome] spawn error: ${err.message}`));
  child.on("exit", (code) => log(`[chrome] exited with ${code}`));
  return { child, tail };
}

async function connectBrowser({ userDataDir, chrome, log, tail }) {
  const portFile = join(userDataDir, "DevToolsActivePort");
  let text;
  try {
    text = await waitFor(
      () => (existsSync(portFile) ? readFileSync(portFile, "utf8") : null),
      { timeoutMs: 30000, everyMs: 50, what: "Chromium's DevToolsActivePort file" },
    );
  } catch (err) {
    throw new Error(`${err.message}\nChromium said:\n${tail.slice(-10).join("\n") || "(nothing)"}`);
  }
  const lines = String(text).split(/\r?\n/);
  const port = Number(lines[0]);
  const wsPath = (lines[1] || "").trim();
  if (!port) throw new Error(`bad DevToolsActivePort: ${JSON.stringify(text)}`);
  const wsUrl = `ws://127.0.0.1:${port}${wsPath || "/devtools/browser"}`;
  log(`[cdp] DevToolsActivePort ${port} → ${wsUrl}`);
  const cdp = await Cdp.connect(wsUrl);
  return { cdp, port, wsUrl };
}

// ------------------------------------------------------------------ page glue
function wrapExpression(code) {
  return `(async () => {\n  return await (\n${code}\n  );\n})()`;
}

function formatResult(value) {
  if (value === undefined) return "undefined";
  try {
    return JSON.stringify(value, null, 2) ?? String(value);
  } catch {
    return String(value);
  }
}

export async function main(argv) {
  const opts = parseArgs(argv);
  if (opts.help) {
    process.stdout.write(HELP);
    return 0;
  }
  const log = opts.quiet ? () => {} : (line) => process.stderr.write(line + "\n");

  const preloadPath = join(HERE, "preload.js");
  const preloadSource = readFileSync(preloadPath, "utf8");

  const server = await startServer({ root: opts.uiRoot, preloadSource });
  log(`[http] serving ${opts.uiRoot} on ${server.origin}`);

  const userDataDir = mkdtempSync(join(tmpdir(), "uiharness-"));
  const failures = [];
  let chrome;
  let cdp;
  let sessionId;
  let exitCode = 0;

  const record = (prefix, text) => {
    process.stdout.write(`${prefix} ${text}\n`);
  };
  // A subresource that never arrived is a real problem, but Chrome reports it
  // twice — once as an anonymous "Failed to load resource" console error and
  // once on the Log domain with the URL and the status — so the URL'd one is
  // what gets attributed and the anonymous duplicate is only counted.
  const netProblems = new Map(); // url -> status/why
  let anonymousNetErrors = 0;
  const describe = (arg) => {
    if (!arg) return "";
    if (arg.type === "string") return arg.value;
    if (arg.unserializableValue) return arg.unserializableValue;
    if (arg.value !== undefined) return formatResult(arg.value);
    if (arg.preview) return `${arg.description || arg.type} ${JSON.stringify(arg.preview)}`;
    return arg.description ?? arg.type;
  };

  try {
    chrome = launchChromium({ chrome: opts.chrome, userDataDir, log });
    const browser = await connectBrowser({ userDataDir, chrome: opts.chrome, log, tail: chrome.tail });
    cdp = browser.cdp;

    // A page target to drive. Attach flat so one socket carries both the
    // browser session and the page session.
    const { targetId } = await cdp.send("Target.createTarget", { url: "about:blank" });
    const attached = await cdp.send("Target.attachToTarget", { targetId, flatten: true });
    sessionId = attached.sessionId;
    log(`[cdp] page target ${targetId} (session ${sessionId})`);

    // Page traffic. Everything the page logs lands in this process's stdout
    // with a stable prefix, so a test can grep for it.
    cdp.on(
      "Runtime.consoleAPICalled",
      (p) => {
        const text = (p.args || []).map(describe).join(" ");
        const map = { error: "[console.error]", warning: "[console.warn]", assert: "[console.assert]" };
        const prefix = map[p.type] || "[console]";
        record(prefix, text);
        if (p.type === "error" || p.type === "assert") {
          if (/Failed to load resource/i.test(text)) anonymousNetErrors++;
          else failures.push(`${prefix} ${text}`);
        }
      },
      sessionId,
    );
    cdp.on(
      "Runtime.exceptionThrown",
      (p) => {
        const d = p.exceptionDetails || {};
        const text = d.exception?.description || d.text || "unknown exception";
        const where = d.url ? ` (${d.url}:${(d.lineNumber ?? 0) + 1})` : "";
        record("[pageerror]", text + where);
        failures.push(`[pageerror] ${text}${where}`);
      },
      sessionId,
    );
    cdp.on(
      "Log.entryAdded",
      (p) => {
        const e = p.entry || {};
        const isNetwork = e.source === "network" || e.source === "security";
        const prefix = e.level === "error" ? "[console.error]" : e.level === "warning" ? "[console.warn]" : "[console]";
        record(prefix, `${e.text || ""}${e.url ? ` (${e.url})` : ""}`);
        if (e.level !== "error") return;
        if (isNetwork) {
          const url = e.url || "(unknown url)";
          if (/favicon/i.test(url)) return;
          // The app asked for something that is not on disk. Attributed by URL
          // rather than counted, so a missing chunk or stylesheet names itself.
          netProblems.set(url, e.text || "failed");
        } else if (!/Failed to load resource/i.test(e.text || "")) {
          failures.push(`${prefix} ${e.text}`);
        }
      },
      sessionId,
    );

    await cdp.send("Page.enable", {}, sessionId);
    await cdp.send("Runtime.enable", {}, sessionId);
    await cdp.send("Log.enable", {}, sessionId);

    await cdp.send(
      "Emulation.setDeviceMetricsOverride",
      { width: opts.width, height: opts.height, deviceScaleFactor: 1, mobile: false },
      sessionId,
    );
    log(`[cdp] viewport ${opts.width}x${opts.height}`);

    // Installed before navigation, so it runs before the document's own scripts.
    await cdp.send(
      "Page.addScriptToEvaluateOnNewDocument",
      { source: readFileSync(join(HERE, "bootstrap.js"), "utf8") },
      sessionId,
    );

    const target = opts.url.startsWith("http") ? opts.url : server.url(opts.url);
    log(`[cdp] navigating to ${target}`);
    const loaded = cdp.once("Page.loadEventFired", sessionId, 45000);
    await cdp.send("Page.navigate", { url: target }, sessionId);
    await loaded;

    // Ready: the page sets window.__HARNESS_READY once the app has painted.
    // Poll at 100ms up to 30s; on failure dump what the page said and bail.
    const evaluate = (expression, { awaitPromise = true } = {}) =>
      cdp.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise, userGesture: true }, sessionId);

    let ready = false;
    const startWait = Date.now();
    const deadline = startWait + 30000;
    while (Date.now() < deadline) {
      const r = await evaluate("window.__HARNESS_READY === true", { awaitPromise: false }).catch(() => null);
      if (r?.result?.value === true) {
        ready = true;
        break;
      }
      await sleep(100);
    }
    if (!ready) {
      const url = await evaluate("location.href", { awaitPromise: false }).catch(() => null);
      record("[harness]", "window.__HARNESS_READY never became true within 30s");
      record("[harness]", `url: ${url?.result?.value ?? "?"}`);
      record("[harness]", `title: ${(await evaluate("document.title", { awaitPromise: false }).catch(() => null))?.result?.value ?? "?"}`);
      record("[harness]", `#win className: ${(await evaluate("document.querySelector('#win')?.className ?? null", { awaitPromise: false }).catch(() => null))?.result?.value ?? "?"}`);
      throw new Error("the app never signalled ready — see the console output above");
    }
    log(`[harness] ready after ${Date.now() - startWait}ms`);

    // The app signals ready on a double animation frame, which can land before
    // its first screen has actually painted. The UI itself says when it is
    // settled: the shell carries `booting` (style.css hides the rail and the
    // sidebar under .win.booting) until the first screen mounts. Wait it out so
    // measurements are of real layout, not of a hidden skeleton.
    const bootingGone = Date.now() + 15000;
    while (Date.now() < bootingGone) {
      const r = await evaluate("!document.querySelector('#win')?.classList.contains('booting')", { awaitPromise: false }).catch(() => null);
      if (r?.result?.value === true) break;
      await sleep(50);
    }
    const stillBooting = await evaluate("document.querySelector('#win')?.classList.contains('booting') === true", { awaitPromise: false })
      .then((r) => r?.result?.value === true)
      .catch(() => false);
    if (stillBooting) log("[harness] still booting after 15s; measuring anyway");

    // Then let the app stop drawing. preload.js counts DOM mutations; wait for
    // that count to stop moving so evals and screenshots see a settled screen
    // rather than a frame in the middle of the app's own boot.
    const quietSince = await (async () => {
      let last = -1;
      let sameFor = 0;
      let quietAt = 0;
      const giveUp = Date.now() + 10000;
      while (Date.now() < giveUp) {
        const r = await evaluate("window.__H?.revisions ?? -1", { awaitPromise: false }).catch(() => null);
        const now = r?.result?.value ?? -1;
        if (now === last) {
          sameFor++;
          if (sameFor >= 4) {
            quietAt = Date.now();
            break;
          }
        } else {
          last = now;
          sameFor = 0;
        }
        await sleep(60);
      }
      return { revisions: last, quietMs: quietAt ? Date.now() - quietAt : 0, settled: sameFor >= 4 };
    })();
    log(`[harness] dom settled at revision ${quietSince.revisions}${quietSince.settled ? "" : " (still moving)"}`);

    if (opts.wait > 0) await sleep(opts.wait);

    for (const step of opts.steps) {
      if (step.kind === "eval") {
        let out;
        try {
          out = await evaluate(wrapExpression(step.code));
        } catch (err) {
          record("[evalerror]", `${step.label}: ${err.message}`);
          failures.push(`[evalerror] ${step.label}: ${err.message}`);
          process.stdout.write(`RESULT: null\n`);
          continue;
        }
        const d = out.exceptionDetails;
        if (d) {
          const text = d.exception?.description || d.text || "evaluation threw";
          record("[evalerror]", `${step.label}: ${text}`);
          failures.push(`[evalerror] ${step.label}: ${text}`);
          process.stdout.write(`RESULT: null\n`);
          continue;
        }
        process.stdout.write(`RESULT: ${formatResult(out.result?.value)}\n`);
      } else if (step.kind === "shot") {
        const file = isAbsolute(step.path) ? step.path : resolve(process.cwd(), step.path);
        mkdirSync(dirname(file), { recursive: true });

        // A screenshot has to be a picture of one state, and this UI animates
        // its own layout: the sidebar collapse is a 0.28s transition on the
        // window's grid tracks. Anything that re-lays-out mid-capture lands on
        // a frame the app was only passing through.
        //
        // captureBeyondViewport does exactly that — asked to capture past the
        // viewport, Chromium re-lays-out the page, which fires the app's resize
        // handler, which animates the sidebar, and a screenshot of a half-open
        // sidebar comes out. So: grow the viewport to the content height so
        // there is nothing beyond it to capture, freeze transitions so the next
        // frame is the settled one, capture, then put everything back.
        const measure = await evaluate(
          "(() => { const d = document.documentElement; return { h: Math.max(d.scrollHeight, d.clientHeight), w: Math.max(d.scrollWidth, d.clientWidth) }; })()",
          { awaitPromise: false },
        );
        const contentH = Math.max(1, Math.ceil(measure.result?.value?.h || opts.height));
        const contentW = Math.max(1, Math.ceil(measure.result?.value?.w || opts.width));

        const grown = opts.shotMode === "full" && contentH > opts.height;
        if (grown) {
          await cdp.send(
            "Emulation.setDeviceMetricsOverride",
            { width: contentW, height: contentH, deviceScaleFactor: 1, mobile: false },
            sessionId,
          );
          await sleep(200);
          log(`[shot] viewport grown to ${contentW}x${contentH} for the full-page capture`);
        }

        const FREEZE = "data-uiharness-freeze";
        await evaluate(
          `(() => {
             let s = document.getElementById(${JSON.stringify(FREEZE)});
             if (!s) {
               s = document.createElement("style");
               s.id = ${JSON.stringify(FREEZE)};
               s.textContent = "*,*::before,*::after{transition:none !important;animation:none !important;transition-duration:0s !important;animation-duration:0s !important}";
               document.head.appendChild(s);
             }
             return true;
           })()`,
          { awaitPromise: false },
        );
        await sleep(120); // let the frozen state paint

        const shot = await cdp.send(
          "Page.captureScreenshot",
          { format: "png", captureBeyondViewport: opts.shotMode === "full", fromSurface: true },
          sessionId,
        );
        const bytes = Buffer.from(shot.data, "base64");
        writeFileSync(file, bytes);

        await evaluate(
          `(() => { document.getElementById(${JSON.stringify(FREEZE)})?.remove(); return true; })()`,
          { awaitPromise: false },
        );
        if (grown) {
          await cdp.send(
            "Emulation.setDeviceMetricsOverride",
            { width: opts.width, height: opts.height, deviceScaleFactor: 1, mobile: false },
            sessionId,
          );
          await sleep(150);
        }

        const stateAfter = await evaluate(
          "({ innerW: innerWidth, innerH: innerHeight, cls: document.querySelector('#win')?.className, sideW: Math.round(document.querySelector('#side')?.getBoundingClientRect().width ?? -1) })",
          { awaitPromise: false },
        ).catch(() => null);
        log(`[shot] page state at capture: ${JSON.stringify(stateAfter?.result?.value)}`);
        process.stdout.write(
          `SHOT: ${file} (${bytes.length} bytes, ${contentW}x${contentH} page, ${opts.shotMode === "full" ? "captureBeyondViewport: true" : "viewport only"})\n`,
        );
        log(`[shot] ${file} ${bytes.length} bytes`);
      } else {
        throw new Error(`unknown step ${step.kind}`);
      }
    }

    if (failures.length) {
      record("[harness]", `${failures.length} page problem${failures.length === 1 ? "" : "s"}:`);
      for (const f of failures.slice(0, 25)) record("[harness]", "  " + f);
      exitCode = 1;
    } else if (netProblems.size || anonymousNetErrors) {
      record("[harness]", `${netProblems.size || anonymousNetErrors} subresource load failure${netProblems.size === 1 ? "" : "s"}:`);
      for (const [url, why] of [...netProblems].slice(0, 15)) record("[harness]", `  ${url} — ${why}`);
      if (!netProblems.size) record("[harness]", `  ${anonymousNetErrors} anonymous "Failed to load resource" error(s)`);
      exitCode = 1;
    } else {
      log(`[harness] no page errors`);
    }
  } catch (err) {
    record("[harness]", err?.stack || String(err));
    exitCode = 1;
  } finally {
    if (cdp) {
      try {
        await cdp.send("Browser.close");
      } catch {}
      try {
        cdp.close();
      } catch {}
    }
    chrome?.child?.kill();
    await server.close().catch(() => {});
    if (!opts.keep) {
      // A killed Chromium still holds its profile for a moment on Windows.
      for (let i = 0; i < 5; i++) {
        try {
          rmSync(userDataDir, { recursive: true, force: true });
          break;
        } catch {
          await sleep(150);
        }
      }
    } else {
      log(`[harness] kept profile ${userDataDir}`);
    }
  }
  return exitCode;
}

const invokedDirectly = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (invokedDirectly) {
  main(process.argv.slice(2))
    .then((code) => process.exit(code))
    .catch((err) => {
      process.stderr.write(`[harness] fatal: ${err?.stack || err}\n`);
      process.exit(1);
    });
}
