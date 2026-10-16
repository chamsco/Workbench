// uiharness/cdp.mjs — a minimal Chrome DevTools Protocol client.
//
// One WebSocket to the browser endpoint, flat sessions to reach a page.
// Node 26's global WebSocket is the only transport; there is no dependency to
// install and none is wanted.

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export class CdpError extends Error {
  constructor(method, error) {
    super(`${method}: ${error?.message || JSON.stringify(error)}${error?.data ? " — " + JSON.stringify(error.data) : ""}`);
    this.name = "CdpError";
    this.method = method;
    this.code = error?.code;
    this.data = error?.data;
  }
}

export class Cdp {
  #ws;
  #next = 0;
  #pending = new Map();
  #listeners = new Map(); // `${sessionId||""}|${method}` -> [fn]
  #closed = false;

  constructor(url) {
    this.url = url;
  }

  static connect(url, { timeoutMs = 15000 } = {}) {
    return new Promise((resolvePromise, reject) => {
      let ws;
      try {
        ws = new WebSocket(url);
      } catch (err) {
        return reject(err);
      }
      const cdp = new Cdp(url);
      cdp.#ws = ws;
      const t = setTimeout(() => {
        try {
          ws.close();
        } catch {}
        reject(new Error(`timed out connecting to ${url}`));
      }, timeoutMs);
      ws.addEventListener("open", () => {
        clearTimeout(t);
        resolvePromise(cdp);
      });
      ws.addEventListener("error", (e) => {
        clearTimeout(t);
        reject(new Error(`websocket error on ${url}: ${e?.message || "unknown"}`));
      });
      ws.addEventListener("message", (ev) => cdp.#receive(ev.data));
      ws.addEventListener("close", () => {
        cdp.#closed = true;
        for (const [, p] of cdp.#pending) p.reject(new Error("CDP socket closed"));
        cdp.#pending.clear();
      });
    });
  }

  get closed() {
    return this.#closed;
  }

  #receive(data) {
    let msg;
    try {
      msg = JSON.parse(typeof data === "string" ? data : String(data));
    } catch {
      return;
    }
    if (msg.id != null && this.#pending.has(msg.id)) {
      const p = this.#pending.get(msg.id);
      this.#pending.delete(msg.id);
      if (msg.error) p.reject(new CdpError(p.method, msg.error));
      else p.resolve(msg.result ?? {});
      return;
    }
    if (msg.method) {
      const key = `${msg.sessionId || ""}|${msg.method}`;
      for (const fn of this.#listeners.get(key) || []) {
        try {
          fn(msg.params || {}, msg);
        } catch (err) {
          process.stderr.write(`[cdp] listener for ${msg.method} threw: ${err?.stack || err}\n`);
        }
      }
      for (const fn of this.#listeners.get(`*|${msg.method}`) || []) {
        try {
          fn(msg.params || {}, msg);
        } catch {}
      }
    }
  }

  send(method, params = {}, sessionId) {
    if (this.#closed) return Promise.reject(new Error(`CDP socket is closed (${method})`));
    const id = ++this.#next;
    const payload = { id, method, params };
    if (sessionId) payload.sessionId = sessionId;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject, method });
      try {
        this.#ws.send(JSON.stringify(payload));
      } catch (err) {
        this.#pending.delete(id);
        reject(err);
      }
    });
  }

  on(method, fn, sessionId) {
    const key = `${sessionId || ""}|${method}`;
    const list = this.#listeners.get(key) || [];
    list.push(fn);
    this.#listeners.set(key, list);
    return () => {
      const cur = this.#listeners.get(key) || [];
      const i = cur.indexOf(fn);
      if (i >= 0) cur.splice(i, 1);
    };
  }

  /** Wait for one event, or resolve null after timeoutMs. */
  once(method, sessionId, timeoutMs = 15000) {
    return new Promise((resolve) => {
      const off = this.on(
        method,
        (params) => {
          off();
          clearTimeout(t);
          resolve(params);
        },
        sessionId,
      );
      const t = setTimeout(() => {
        off();
        resolve(null);
      }, timeoutMs);
    });
  }

  close() {
    this.#closed = true;
    try {
      this.#ws.close();
    } catch {}
  }
}

export async function waitFor(check, { timeoutMs = 20000, everyMs = 100, what = "condition" } = {}) {
  const deadline = Date.now() + timeoutMs;
  let last;
  while (Date.now() < deadline) {
    try {
      last = await check();
    } catch (err) {
      last = undefined;
    }
    if (last) return last;
    await sleep(everyMs);
  }
  throw new Error(`timed out after ${timeoutMs}ms waiting for ${what}`);
}

export { sleep };
