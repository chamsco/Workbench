// uiharness/preload.js — the stub injected into the page BEFORE the app's own
// scripts run. It stands in for the Tauri runtime so crates/tauri-app/ui boots
// unchanged in headless Chromium.
//
// Nothing here is app code and nothing here edits the app: run.mjs serves the
// real index.html with this file's source inlined as the first <script> in
// <head>, so window.__TAURI__ exists before app.js reads it at parse time
// (`const TAURI = window.__TAURI__`).
//
// Contract, from the harness side:
//   window.__MOCK[cmd]  -> your own handler, wins over the defaults below
//   window.__CALLS      -> every invoke() the app made, in order
//   window.__LISTENERS  -> event name -> [callbacks]; read by __H.fire()
//   window.__H          -> { calls, mock, fire, setReady }
//   window.__HARNESS_READY === true  -> the harness stops waiting
(function () {
  "use strict";

  if (window.__HARNESS_READY === undefined) window.__HARNESS_READY = false;
  window.__CALLS = window.__CALLS || [];
  window.__MOCK = window.__MOCK || {};
  window.__LISTENERS = window.__LISTENERS || {};
  window.__MOCKS_SEEN = window.__MOCKS_SEEN || [];

  // ------------------------------------------------------------------ defaults
  // The smallest snapshot the shell can render: no project, no agents, no
  // tickets, nothing pending. app.js does `snap.approvals.filter(...)` and
  // `.length` on several fields, so the empty ones have to be there.
  function snapshot() {
    return {
      name: "local",
      workspace: null,
      agents: [],
      approvals: [],
      tickets: [],
      artifacts: [],
      options: [],
      canvas_ops: [],
      board: [],
      total_cost_usd: 0,
      router_cost_usd: 0,
      config_source: "harness",
      link: { state: "off" },
      index: 0,
    };
  }

  // Boot payload. platform "win" gives the app its native window chrome
  // (documentElement gets .native and .os-win, shell.js draws the caption
  // buttons); layout 0 / no `bench` means no bench tab is injected.
  function boot() {
    return { platform: "win", layout: 0, user: "tester" };
  }

  // Shape borrowed from a real target/app-demo/prefs.json, trimmed. `tabs` and
  // `active_tab` are not optional: app.js does prefs.tabs.forEach and
  // prefs.tabs[prefs.active_tab] unguarded.
  function prefs() {
    return {
      theme: "dark",
      tabs: [{ name: "Harness", panes: [{ kind: "agent", agent: 0, url: null, option: null }], layout: null }],
      active_tab: 0,
      onboarded: true,
      mode: "chat",
      code_view: "agents",
      chat_view: "chat",
      wb_view: "home",
      motion: "full",
      appearance: {},
      projects: [],
      machines: [],
      pinned_apps: [],
      uses: ["chat", "code"],
      check_updates: false,
      split: null,
      memory_on: false,
      memory_auto: false,
      memory_dream: false,
      companion: { enabled: false, addr: "127.0.0.1:7421", token: "", allow_lan: false },
      share: { enabled: false, addr: "127.0.0.1:7420", token: "" },
      harnesses: {},
      routers: [],
      cloud: { url: "http://127.0.0.1:7430", token: "" },
      connections: [],
      remote_agents: [],
      tracing: { endpoint: "", headers: {}, content: false },
      local_tasks: [],
      api_keys: {},
    };
  }

  const DEFAULTS = {
    boot: boot,
    prefs: prefs,
    snapshot: snapshot,

    // Lists. [] keeps every `.map`/`.filter`/`.length` in the shell happy.
    machines: [],
    chat_list: [],
    chat_thread: null,
    agents_list: [],
    apps_list: [],
    app_examples: [],
    app_install_dir: "",
    projects: [],
    providers: [],
    models: [],
    plans: [],
    harnesses: [],
    local_models: [],
    memory_list: [],
    memory_dreams: [],
    git_activity: [],
    git_changes: { files: [], branch: "", fresh: false },
    git_log: [],
    worktrees: [],
    tabs_for: null,
    accounts: { policy: "automatic", accounts: [] },
    skills_catalog: { skills: [], workflow: { main_skills: [], worker_skills: [], triage_skills: [] } },
    colony_status: { running: [], done: [] },
    companion_status: { enabled: false },
    share_status: { enabled: false },
    cloud_cached: null,
    diagram: null,
    trace_get: null,
    link_preview: false,
    frame_check: { ok: false },
    qr_svg: "",
    check_update: { newer: false },

    // Reads and clicks the harness never answers for real.
    read_file: "",
    read_image: "",
    list_files: [],
    pick_folder: null,
    pick_app: null,
    pick_option: null,
    open_project: null,
    close_project: null,
    forget_project: null,
    choose: null,
    scan: null,
  };

  // Commands whose name is enough to know the answer is "fine, nothing
  // happened": setters, senders, and the chat/agent/memory verbs.
  function looksLikeACommand(cmd) {
    return /^(set_|save_|forget_|add_|remove_|select_|open_|close_|send|stop|start_|resume|approve|reject|post_|file_|notify|reveal_|ensure_|pick_|chat_|agent_|memory_|terminal_|trace_test|cli_login|cloud_|account_|connect_|disconnect_|rescan|install_|uninstall_|toggle_|clear_|kill_|cancel_|post|put|delete_)/.test(cmd);
  }

  function defaultFor(cmd) {
    if (looksLikeACommand(cmd)) return null;
    // Unknown command: record it and hand back null. A harness test that needs
    // a real answer registers window.__H.mock(cmd, fn) instead.
    return null;
  }

  // ------------------------------------------------------------------ invoke
  async function invoke(cmd, args) {
    const entry = { cmd: cmd, args: args === undefined ? null : args };
    window.__CALLS.push(entry);
    if (Object.prototype.hasOwnProperty.call(window.__MOCK, cmd)) {
      const fn = window.__MOCK[cmd];
      if (typeof fn === "function") return await fn(entry.args, entry);
      return fn; // a mock may be a plain value
    }
    const d = DEFAULTS[cmd];
    if (typeof d === "function") return d(entry.args);
    if (Object.prototype.hasOwnProperty.call(DEFAULTS, cmd)) return d;
    return defaultFor(cmd);
  }

  const CHANNEL = "__harness_channel__";
  const CALLBACK = "__harness_callback__";

  // ------------------------------------------------------------------ window
  const noop = () => Promise.resolve();
  function currentWindow() {
    return {
      label: "main",
      minimize: noop,
      maximize: noop,
      unmaximize: noop,
      toggleMaximize: noop,
      isMaximized: () => Promise.resolve(false),
      isMinimized: () => Promise.resolve(false),
      isVisible: () => Promise.resolve(true),
      isFocused: () => Promise.resolve(true),
      show: noop,
      hide: noop,
      close: noop,
      destroy: noop,
      setTitle: noop,
      setFocus: noop,
      setFullscreen: noop,
      isFullscreen: () => Promise.resolve(false),
      startDragging: noop,
      startResizeDragging: noop,
      setDecorations: noop,
      onResized: () => Promise.resolve(noop),
      onMoved: () => Promise.resolve(noop),
      onCloseRequested: () => Promise.resolve(noop),
      onFocusChanged: () => Promise.resolve(noop),
      scaleFactor: () => Promise.resolve(1),
      innerPosition: () => Promise.resolve({ x: 0, y: 0 }),
      outerPosition: () => Promise.resolve({ x: 0, y: 0 }),
      innerSize: () => Promise.resolve({ width: window.innerWidth, height: window.innerHeight }),
      outerSize: () => Promise.resolve({ width: window.innerWidth, height: window.innerHeight }),
      theme: () => Promise.resolve("dark"),
      asTauriWindow: () => currentWindow(),
    };
  }

  function eventApi() {
    const listeners = window.__LISTENERS;
    return {
      // event.listen(name, cb) -> Promise<unlisten>. The callback is kept in
      // window.__LISTENERS[name] so a test can fire it with __H.fire.
      listen: async function (name, cb, opts) {
        const array = (listeners[name] = listeners[name] || []);
        array.push(cb);
        void opts; // { target } is ignored: there is one window here
        return function unlisten() {
          const i = array.indexOf(cb);
          if (i >= 0) array.splice(i, 1);
        };
      },
      once: async function (name, cb) {
        const array = (listeners[name] = listeners[name] || []);
        const wrapped = function (e) {
          const i = array.indexOf(wrapped);
          if (i >= 0) array.splice(i, 1);
          cb(e);
        };
        array.push(wrapped);
        return function unlisten() {
          const i = array.indexOf(wrapped);
          if (i >= 0) array.splice(i, 1);
        };
      },
      emit: async function (name, payload) {
        (listeners[name] || []).slice().forEach(function (cb) {
          cb({ event: name, id: 0, payload: payload });
        });
      },
      emitTo: async function () {},
      TauriEvent: { WINDOW_RESIZED: "tauri://resize", WINDOW_CLOSE_REQUESTED: "tauri://close-requested" },
    };
  }

  // The pieces of @tauri-apps/api that a desktop UI commonly reaches for. The
  // app under test only uses core.invoke, event.listen and window.getCurrentWindow,
  // but a boot that touches one more module should not be the reason a run fails.
  const core = {
    invoke: invoke,
    convertFileSrc: (path) => "asset://localhost/" + String(path).replace(/^\/+/, ""),
    isTauri: true,
    Channel: class Channel {
      constructor() {
        this.onmessage = null;
        this.id = 0;
      }
    },
    Resource: class Resource {
      constructor(rid) {
        this.rid = rid;
      }
      close() {
        return Promise.resolve();
      }
    },
  };

  window.__TAURI__ = Object.assign(window.__TAURI__ || {}, {
    core: core,
    invoke: invoke,
    event: eventApi(),
    window: {
      getCurrentWindow: currentWindow,
      getCurrent: currentWindow,
      getAllWindows: () => Promise.resolve([currentWindow()]),
      Window: function Window() {
        return currentWindow();
      },
      LogicalSize: class LogicalSize {
        constructor(width, height) {
          this.width = width;
          this.height = height;
        }
      },
      PhysicalSize: class PhysicalSize {
        constructor(width, height) {
          this.width = width;
          this.height = height;
        }
      },
      LogicalPosition: class LogicalPosition {
        constructor(x, y) {
          this.x = x;
          this.y = y;
        }
      },
      PhysicalPosition: class PhysicalPosition {
        constructor(x, y) {
          this.x = x;
          this.y = y;
        }
      },
    },
    webviewWindow: { getCurrentWebviewWindow: currentWindow },
    webview: { getCurrentWebview: currentWindow },
    dpi: { LogicalSize: class LogicalSize {}, PhysicalSize: class PhysicalSize {} },
    path: {
      sep: "\\",
      join: async (...parts) => parts.filter(Boolean).join("\\"),
      appDataDir: async () => "C:\\Users\\tester\\AppData\\Roaming\\backspace",
      appConfigDir: async () => "C:\\Users\\tester\\AppData\\Roaming\\backspace",
      homeDir: async () => "C:\\Users\\tester",
      tempDir: async () => "C:\\Users\\tester\\AppData\\Local\\Temp",
      resolve: async (...parts) => parts.filter(Boolean).join("\\"),
    },
    app: {
      getName: async () => "Backspace",
      getVersion: async () => "0.0.0-harness",
      getTauriVersion: async () => "2.0.0",
      show: noop,
      hide: noop,
      exit: async () => {},
    },
    // Not real APIs: the harness's own handles, kept on __TAURI__ so a page
    // script that only has __TAURI__ can still reach them.
    mock: (cmd, fn) => window.__H.mock(cmd, fn),
    calls: () => window.__CALLS,
  });

  // Tauri v2 also exposes the raw transport under __TAURI_INTERNALS__. Some
  // bundles check for it. Same behaviour, no channels.
  window.__TAURI_INTERNALS__ = Object.assign(window.__TAURI_INTERNALS__ || {}, {
    invoke: invoke,
    transformCallback: (cb) => {
      const id = CALLBACK + (window.__CALLBACK_ID = (window.__CALLBACK_ID || 0) + 1);
      window[id] = cb;
      return id;
    },
    unregisterCallback: (id) => {
      delete window[id];
    },
    convertFileSrc: core.convertFileSrc,
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
    plugins: { path: {} },
    callbacks: new Map(),
  });

  // ------------------------------------------------------------------ __H
  window.__H = {
    calls: () => window.__CALLS,
    /** Mock a command. fn(args) may return a value or a promise; a non-function
     *  is used as the value. Wins over the built-in defaults. */
    mock(cmd, fn) {
      window.__MOCK[cmd] = fn;
      window.__MOCKS_SEEN.push(cmd);
      return fn;
    },
    unmock(cmd) {
      delete window.__MOCK[cmd];
    },
    /** Fire a Tauri event at every listener registered for it:
     *  __H.fire("state", { ... }) */
    fire(name, payload) {
      const array = window.__LISTENERS[name] || [];
      array.slice().forEach((cb) => {
        try {
          cb({ event: name, id: 0, payload: payload });
        } catch (err) {
          console.warn("[harness] listener for " + name + " threw:", err);
        }
      });
      return array.length;
    },
    listeners: (name) => (window.__LISTENERS[name] || []).slice(),
    /** Resolve on the next animation frames, with a timer fallback so this
     *  cannot hang in a background or throttled tab. */
    frames: (n = 2) =>
      new Promise((resolve) => {
        let left = n;
        let done = false;
        const tick = () => {
          if (done) return;
          if (--left <= 0) {
            done = true;
            resolve();
          } else {
            requestAnimationFrame(tick);
          }
        };
        requestAnimationFrame(tick);
        setTimeout(() => {
          if (!done) {
            done = true;
            resolve();
          }
        }, 16 * n + 250);
      }),
    /** Force the ready flag early (normally set on load, see below). */
    setReady() {
      window.__HARNESS_READY = true;
    },
    /** How many times the DOM has changed shape. run.mjs watches this to know
     *  the app has stopped rendering before it measures or screenshots: a
     *  screenshot taken mid-render is a screenshot of a state that never
     *  existed. Counts class/style/subtree changes anywhere. */
    revisions: 0,
  };

  // ------------------------------------------------------------------ ready
  // The app boots on DOMContentLoaded and finishes its first paint a couple of
  // frames later, so READY is set two frames after `load` — after the app has
  // rendered, not merely after its scripts parsed. The timers are a backstop:
  // a run must never hang on a flag.
  let announced = false;
  function announceReady() {
    if (announced) return;
    announced = true;
    window.__HARNESS_READY = true;
  }
  function afterFrames(n) {
    if (n <= 0) return announceReady();
    requestAnimationFrame(() => setTimeout(() => afterFrames(n - 1), 32));
  }
  if (document.readyState === "complete") {
    setTimeout(() => afterFrames(2), 0);
  } else {
    addEventListener("load", () => setTimeout(() => afterFrames(2), 0), { once: true });
  }
  setTimeout(announceReady, 15000);

  // ------------------------------------------------------------------ settle
  // Count DOM changes. The observer is installed as soon as there is a document
  // element to observe; the first revision lands with the app's own boot.
  try {
    const start = () => {
      new MutationObserver(() => {
        window.__H.revisions++;
      }).observe(document.documentElement, {
        subtree: true,
        childList: true,
        attributes: true,
        characterData: true,
      });
      window.__H.observing = true;
    };
    if (document.documentElement) start();
    else addEventListener("DOMContentLoaded", start, { once: true });
  } catch (err) {
    console.warn("[harness] could not observe the DOM for settle detection:", err);
  }
})();
