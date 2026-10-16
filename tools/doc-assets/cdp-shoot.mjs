#!/usr/bin/env node
/*
  cdp-shoot.mjs — drive and photograph the running Tauri app.

  The Tauri shell is a WebView2 window, so if it is launched with
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 we can talk
  Chrome DevTools Protocol to it: click by visible text, wait for a condition,
  and capture the page at its real size. No synthetic OS mouse, no window
  positioning, no screenshot of the desktop.

  Usage:
    node cdp-shoot.mjs --port=9222 --out=target/app-shots --plan=tools/doc-assets/app-plan.json
    node cdp-shoot.mjs --port=9222 --dump          # list what is clickable right now

  Plan steps (JSON array):
    {"shot": "name"}                      capture to <out>/<name>.png
    {"click": "Wall"}                     click the smallest visible element whose text is exactly this
    {"clickContains": "Diagram"}          same, but substring match
    {"eval": "expression"}                evaluate and print the value
    {"waitFor": "expression", "timeout": 30000}   poll until truthy
    {"wait": 1200}                        sleep
*/

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { argv, exit } from "node:process";

const argvMap = {};
for (const raw of argv.slice(2)) {
  const m = /^--([^=]+)(?:=(.*))?$/.exec(raw);
  if (m) argvMap[m[1]] = m[2] === undefined ? true : m[2];
}
const port = Number(argvMap.port || 9222);
const outDir = String(argvMap.out || "target/app-shots");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function findTarget() {
  const deadline = Date.now() + 60000;
  let lastErr = "no targets";
  while (Date.now() < deadline) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
      const page = list.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
      if (page) return page;
      lastErr = `targets: ${list.map((t) => t.type).join(",") || "none"}`;
    } catch (e) {
      lastErr = e.message;
    }
    await sleep(400);
  }
  throw new Error(`no CDP page target on ${port} (${lastErr})`);
}

const target = await findTarget();
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((res, rej) => {
  ws.addEventListener("open", res, { once: true });
  ws.addEventListener("error", () => rej(new Error("CDP socket failed")), { once: true });
});

let seq = 0;
const pending = new Map();
ws.addEventListener("message", (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    const { res, rej } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? rej(new Error(JSON.stringify(msg.error))) : res(msg.result);
  }
});
const send = (method, params = {}) =>
  new Promise((res, rej) => {
    const id = ++seq;
    pending.set(id, { res, rej });
    ws.send(JSON.stringify({ id, method, params }));
  });

async function evaluate(expression) {
  const r = await send("Runtime.evaluate", {
    expression, returnByValue: true, awaitPromise: true, userGesture: true,
  });
  if (r.exceptionDetails) throw new Error(`eval threw: ${r.exceptionDetails.text}`);
  return r.result.value;
}

// The smallest visible element whose text matches: outer containers also contain
// the text, and clicking those would hit the wrong thing.
const picker = (mode, text) => `(() => {
  const want = ${JSON.stringify(text)};
  const norm = (s) => (s || "").replace(/\\s+/g, " ").trim();
  const hit = [...document.querySelectorAll("button,[role=tab],[role=button],a,li,span,div")]
    .map((el) => ({ el, r: el.getBoundingClientRect(), t: norm(el.textContent) }))
    .filter((c) => c.r.width > 2 && c.r.height > 2 && c.r.bottom > 0 && c.r.right > 0 &&
                   (${mode === "exact" ? "c.t === want" : "c.t.includes(want)"}))
    .sort((a, b) => a.r.width * a.r.height - b.r.width * b.r.height)[0];
  if (!hit) return null;
  return { x: Math.round(hit.r.left + hit.r.width / 2), y: Math.round(hit.r.top + hit.r.height / 2),
           tag: hit.el.tagName, cls: String(hit.el.className).slice(0, 60), text: hit.t.slice(0, 60) };
})()`;

async function click(mode, text) {
  const box = await evaluate(picker(mode, text));
  if (!box) throw new Error(`nothing clickable matching "${text}"`);
  const base = { x: box.x, y: box.y, button: "left", clickCount: 1 };
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", x: box.x, y: box.y, button: "none" });
  await sleep(80);
  await send("Input.dispatchMouseEvent", { type: "mousePressed", ...base });
  await sleep(40);
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", ...base });
  console.log(`clicked <${box.tag} class="${box.cls}"> "${box.text}" at ${box.x},${box.y}`);
}

const LIST_JS = `(() => {
  const norm = (s) => (s || "").replace(/\\s+/g, " ").trim();
  return [...document.querySelectorAll("button,[role=tab],[role=button],a,[aria-label],li,svg")]
    .map((el) => ({ el, r: el.getBoundingClientRect(), cs: getComputedStyle(el) }))
    .filter((c) => c.r.width > 2 && c.r.height > 2 &&
                   (c.el.matches("button,[role=tab],[role=button],a") ||
                    c.cs.cursor === "pointer" || c.el.getAttribute("aria-label")))
    .map((c) => ({ tag: c.el.tagName, cls: String(c.el.className).slice(0, 38),
                   aria: (c.el.getAttribute("aria-label") || c.el.getAttribute("title") || "").slice(0, 34),
                   text: norm(c.el.textContent).slice(0, 40),
                   box: [Math.round(c.r.left), Math.round(c.r.top), Math.round(c.r.width), Math.round(c.r.height)] }));
})()`;

async function listClickable() {
  const items = await evaluate(LIST_JS);
  const seen = new Set();
  for (const it of items) {
    const key = it.box.join(",");
    if (seen.has(key)) continue;
    seen.add(key);
    console.log(`${String(it.box).padEnd(24)} ${it.tag.padEnd(7)} ${it.aria.padEnd(34)} ${it.cls.padEnd(38)} ${it.text}`);
  }
  console.log(`${items.length} clickable elements`);
}

async function clickSelector(selector) {
  const box = await evaluate(`(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    if (!el) return null;
    const r = el.getBoundingClientRect();
    if (r.width < 2 || r.height < 2) return null;
    return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2),
             tag: el.tagName, cls: String(el.className).slice(0, 50),
             text: (el.textContent || "").replace(/\\s+/g, " ").trim().slice(0, 40) };
  })()`);
  if (!box) throw new Error(`no visible element for ${selector}`);
  const base = { x: box.x, y: box.y, button: "left", clickCount: 1 };
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", x: box.x, y: box.y, button: "none" });
  await sleep(80);
  await send("Input.dispatchMouseEvent", { type: "mousePressed", ...base });
  await sleep(40);
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", ...base });
  console.log(`clicked ${selector} -> <${box.tag} class="${box.cls}"> "${box.text}" at ${box.x},${box.y}`);
}

if (argvMap.dump) {
  console.log(`viewport ${await evaluate("`${innerWidth}x${innerHeight} dpr=${devicePixelRatio}`")}`);
  if (argvMap.text) {
    console.log("--- visible text ---");
    console.log(await evaluate("document.body.innerText.slice(0, 4000)"));
    console.log("--- end text ---");
  }
  await listClickable();
  ws.close();
  exit(0);
}

const planPath = argvMap.plan;
if (!planPath) {
  console.error("need --plan=<file.json> or --dump");
  exit(2);
}
const plan = JSON.parse(readFileSync(planPath, "utf8"));
mkdirSync(outDir, { recursive: true });

await send("Page.enable");
await send("Runtime.enable");
console.log(`viewport ${await evaluate("`${innerWidth}x${innerHeight} dpr=${devicePixelRatio}`")}`);

for (const step of plan) {
  if (step.eval !== undefined) {
    console.log("eval ->", JSON.stringify(await evaluate(step.eval)));
  } else if (step.waitFor !== undefined) {
    const deadline = Date.now() + (step.timeout || 30000);
    let ok = false;
    while (Date.now() < deadline) {
      if (await evaluate(`Boolean(${step.waitFor})`)) { ok = true; break; }
      await sleep(250);
    }
    console.log(ok ? "waitFor ok" : `waitFor TIMED OUT: ${step.waitFor}`);
    if (!ok) process.exitCode = 3;
  } else if (step.wait !== undefined) {
    await sleep(step.wait);
  } else if (step.click !== undefined) {
    await click("exact", step.click);
    await sleep(step.settle ?? 900);
  } else if (step.clickContains !== undefined) {
    await click("contains", step.clickContains);
    await sleep(step.settle ?? 900);
  } else if (step.clickSel !== undefined) {
    await clickSelector(step.clickSel);
    await sleep(step.settle ?? 900);
  } else if (step.list) {
    await listClickable();
  } else if (step.shot !== undefined) {
    const { data } = await send("Page.captureScreenshot", { format: "png", fromSurface: true });
    const file = `${outDir}/${step.shot}.png`;
    writeFileSync(file, Buffer.from(data, "base64"));
    console.log(`shot ${file}`);
  } else {
    console.warn("unknown step", JSON.stringify(step));
  }
}

ws.close();
