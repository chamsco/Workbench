// Direction 04 — the quiet shell.
//
// The app's engine (projects, agents, canvases, chat, tickets) is unchanged.
// This file draws the new frame around it: the project rail, a page-specific
// sidebar, an almost-empty top bar, and the Home / Work / Canvas / Limits pages.
// Code, Crew, Plugins and Skills hand their body back to the existing surfaces
// so nothing that worked before is lost.

var Quiet = (() => {
  const $q = (s, el = document) => el.querySelector(s);
  const $$q = (s, el = document) => [...el.querySelectorAll(s)];
  const QKEY = "bs.quiet_page";
  const SKEY = "bs.quiet_skin";
  const CKEY = "bs.q_code_view";

  const PAGES = {
    home: "Home", work: "Work", code: "Code", canvas: "Canvas", crew: "Crew",
    today: "Today", machines: "Machines", team: "Team",
    needs: "Needs", board: "Board", review: "Review", automate: "Automate",
    inbox: "Inbox", calendar: "Calendar", tasks: "Tasks", limits: "Limits",
    plugins: "Plugins", skills: "Skills", settings: "Settings",
    apps: "Apps", memory: "Memory",
  };
  // Pages drawn by the old zones. Everything else is ours.
  const LEGACY = new Set(["code", "crew", "plugins", "skills", "apps", "memory"]);
  // Pages whose sidebar is the chat face's own (agents/tree), not ours.
  const CHAT_SIDE = new Set(["crew"]);

  let ready = false;
  let page = "home";
  let internal = false;
  let codeSurface = "session";
  let sessionFilter = "all";
  let data = { agents: [], chats: [], routines: [], harnesses: [], git: null, version: 0, at: 0, loading: false };
  let viewKey = "", sideKey = "", lastPending = 0;
  let todayDate = "";
  let todayGroup = "status";
  let agentEdit = null;
  let fallbackDraft = [];
  let doctor = { at: 0, healed: 0, scanned: 0 };
  let doctorBusy = false;
  let doctorAutoOnce = -1;
  const healedIds = new Set();
  const XKEY = "bs.q_agent_extra";
  const DKEY = "bs.q_doctor_auto";

  // ------------------------------------------------------------------ icons
  const ICON = {
    logo: '<path d="M3.2 6.4 7 18.2l5-9 5 9 3.8-11.8"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    search: '<circle cx="10.5" cy="10.5" r="6"/><path d="m15.4 15.4 4.1 4.1"/>',
    home: '<path d="M4 10.7 12 4l8 6.7V19a1 1 0 0 1-1 1h-4.5v-6h-5v6H5a1 1 0 0 1-1-1z"/>',
    inbox: '<path d="M4 5h16v11H4z"/><path d="M4 12h4l1.5 2h5L16 12h4"/>',
    calendar: '<rect x="4" y="5.5" width="16" height="14" rx="2"/><path d="M8 3.5v4M16 3.5v4M4 10h16"/>',
    tasks: '<path d="m4.5 7 1.5 1.5L8.5 6M4.5 13l1.5 1.5L8.5 12M12 7.5h8M12 13.5h8"/>',
    hand: '<path d="M7.5 12V6.5a1.4 1.4 0 0 1 2.8 0V11M10.3 10.5V4.8a1.4 1.4 0 0 1 2.8 0v6M13.1 10.8V6.3a1.4 1.4 0 0 1 2.8 0v6.2c0 3.1-2.2 5.6-5.3 5.6-1.5 0-2.6-.8-3.4-2l-2-3.2a1.4 1.4 0 0 1 2.3-1.6l1.4 1.5"/>',
    grid: '<rect x="3.8" y="3.8" width="6.4" height="6.4" rx="1.4"/><rect x="13.8" y="3.8" width="6.4" height="6.4" rx="1.4"/><rect x="3.8" y="13.8" width="6.4" height="6.4" rx="1.4"/><rect x="13.8" y="13.8" width="6.4" height="6.4" rx="1.4"/>',
    board: '<rect x="3.5" y="4" width="5" height="9" rx="1.4"/><rect x="9.5" y="4" width="5" height="14" rx="1.4"/><rect x="15.5" y="4" width="5" height="6" rx="1.4"/>',
    branch: '<circle cx="7" cy="6" r="2.2"/><circle cx="7" cy="18" r="2.2"/><circle cx="17" cy="8" r="2.2"/><path d="M7 8.2v7.6M17 10.2c0 3.3-3 4-6 4.4"/>',
    zap: '<path d="M13 2.5 5.5 13.5H11l-1 8 7.5-11H12z"/>',
    chart: '<path d="M4.5 19.5V14M9.5 19.5V9M14.5 19.5V4.5M19.5 19.5V11.5"/>',
    moon: '<path d="M19 14.5A8 8 0 0 1 9.5 5a8 8 0 1 0 9.5 9.5z"/>',
    sun: '<circle cx="12" cy="12" r="3.5"/><path d="M12 2.5v2.2M12 19.3v2.2M2.5 12h2.2M19.3 12h2.2M5.2 5.2l1.6 1.6M17.2 17.2l1.6 1.6M18.8 5.2l-1.6 1.6M6.8 17.2l-1.6 1.6"/>',
    drop: '<path d="M12 3.2c2.8 3.3 6 7 6 10.3a6 6 0 0 1-12 0c0-3.3 3.2-7 6-10.3z"/>',
    gear: '<circle cx="12" cy="12" r="3"/><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M18.4 5.6l-2.1 2.1M7.7 16.3l-2.1 2.1"/>',
    code: '<path d="m9 7.5-5 4.5 5 4.5M15 7.5l5 4.5-5 4.5"/>',
    work: '<rect x="3.5" y="7" width="17" height="12" rx="2.5"/><path d="M8.5 7V5.5A1.5 1.5 0 0 1 10 4h4a1.5 1.5 0 0 1 1.5 1.5V7M3.5 12.5h17"/>',
    canvas: '<rect x="3.5" y="4" width="17" height="16" rx="2"/><path d="M3.5 9h17M9 9v11M15 9v11"/>',
    crew: '<circle cx="9" cy="8.5" r="2.6"/><circle cx="16.5" cy="9.5" r="2"/><path d="M4 18.5a5.2 5.2 0 0 1 10 0M14.5 18.5a4.2 4.2 0 0 1 6 0"/>',
    plugins: '<path d="M8 3.5v3M16 3.5v3M6 6.5h12v4a6 6 0 0 1-12 0zM12 16.5v4"/>',
    skills: '<path d="M5 4.5h9l5 5v10H5z"/><path d="M14 4.5v5h5"/>',
    limits: '<path d="M12 3.5a8.5 8.5 0 1 0 8.5 8.5"/><path d="M12 7v5l3 2"/><path d="M12 3.5V7"/>',
    chev: '<path d="m9.5 6 6 6-6 6"/>',
    up: '<path d="M12 19V5M6 11l6-6 6 6"/>',
    check: '<path d="m5 12.5 4.5 4.5L19 7"/>',
    warn: '<path d="M12 3 2.8 20h18.4z"/><path d="M12 9v5M12 17.5v.2"/>',
    clock: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7v5l3 2"/>',
    folder: '<path d="M3.5 7.5A1.5 1.5 0 0 1 5 6h4l2 2.5h8A1.5 1.5 0 0 1 20.5 10v7A1.5 1.5 0 0 1 19 18.5H5A1.5 1.5 0 0 1 3.5 17z"/>',
    file: '<path d="M6 3.5h7l5 5v12a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-16a1 1 0 0 1 1-1z"/><path d="M13 3.5v5h5"/>',
    more: '<circle cx="5.5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="18.5" cy="12" r="1"/>',
    panel: '<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><path d="M14.5 4.5v15"/>',
    refresh: '<path d="M20 11a8 8 0 1 0-1.7 5.4M20 4v7h-7"/>',
    download: '<path d="M12 3.5v11M7.5 10l4.5 4.5 4.5-4.5M4.5 19.5h15"/>',
    sort: '<path d="M8 5v14M5 8l3-3 3 3M16 19V5M13 16l3 3 3-3"/>',
    play: '<path d="M8 5.5 19 12 8 18.5z"/>',
    laptop: '<rect x="4.5" y="4.5" width="15" height="10.5" rx="2"/><path d="M2.5 19h19M8 19l.6 1.6h6.8L16 19"/>',
    server: '<rect x="3.5" y="4" width="17" height="6.4" rx="1.6"/><rect x="3.5" y="13.6" width="17" height="6.4" rx="1.6"/><path d="M7 7.2h.01M7 16.8h.01"/>',
    terminal: '<rect x="3.5" y="4.5" width="17" height="15" rx="2.4"/><path d="m7 9 3 3-3 3M12.5 15h4"/>',
    heart: '<path d="M12 20s-7.4-4.6-7.4-9.6A4.4 4.4 0 0 1 12 7.1a4.4 4.4 0 0 1 7.4 3.3C19.4 15.4 12 20 12 20z"/><path d="M8.7 12.3h1.8l1-2 1.6 3.8 1-1.8h1.6"/>',
    wifi: '<path d="M3.5 9.4a13 13 0 0 1 17 0M6.6 12.7a9 9 0 0 1 10.8 0M9.6 15.9a4.6 4.6 0 0 1 4.8 0"/><circle cx="12" cy="19" r=".7" fill="currentColor" stroke="none"/>',
    cloud: '<path d="M7.2 18.5a4.1 4.1 0 0 1-.5-8.2 5.6 5.6 0 0 1 10.8-1.2 4.2 4.2 0 0 1 .6 8.4z"/>',
    monitor: '<rect x="3" y="4.5" width="18" height="12" rx="2"/><path d="M9 20h6M12 16.5V20"/>',
    x: '<path d="M6 6l12 12M18 6 6 18"/>',
    help: '<circle cx="12" cy="12" r="8.5"/><path d="M9.7 9.4a2.4 2.4 0 1 1 3.2 2.2c-.6.3-.9.8-.9 1.5v.4M12 16.8v.2"/>',
    dot: '<circle cx="12" cy="12" r="3" fill="currentColor" stroke="none"/>',
  };
  const ic = (n, cls = "") =>
    `<svg class="qi ${cls}" viewBox="0 0 24 24" aria-hidden="true">${ICON[n] || ICON.dot}</svg>`;

  // The vendored open-bot-avatar renderer (same package and gallery as the
  // prototype) for the shell's own surfaces. React's bot face stays in the chat
  // face; this is the plain-JS shell's exact copy of the same art.
  const OBA = window.OpenBotAvatar || null;
  const WALL_SHAPES = ["brain", "cat", "star"];
  function botFace(color, size, label, shape) {
    const def = OBA && OBA[shape || "brain"];
    if (def) {
      try {
        return `<span class="qw-av qw-bot" style="--c:${color};width:${size}px;height:${size}px" aria-hidden="true">${OBA.renderAvatarSvg(def, {
          color, size, depth: 6, motion: "idle", time: 0.4 + (String(label).length % 7) * 0.37, reducedMotion: true,
        })}</span>`;
      } catch {}
    }
    return `<span class="qw-av" style="--c:${color}">${esc(initials(label))}</span>`;
  }

  // Sessions use the model provider's mark, not the bot face: Claude Code,
  // Codex/OpenAI, Cursor, Grok and OpenCode each keep their own logo.
  const PROVIDER_BG = { claude: "#d97757", codex: "#10a37f", openai: "#10a37f", cursor: "#f59e0b", grok: "#111111", opencode: "#211e1e" };
  function providerOf(a) {
    const title = ((a && (a.title || a.name)) || "").toLowerCase();
    if (!a || a.id === 0 || title.includes("claude") || title.includes("main")) return "claude";
    if (title.includes("cursor")) return "cursor";
    if (title.includes("codex")) return "codex";
    if (title.includes("grok")) return "grok";
    if (title.includes("opencode")) return "opencode";
    const rp = (a && a.route && a.route.provider) || "";
    if (PROVIDER_BG[rp]) return rp;
    const s = ((a && a.decision && a.decision.model) || "").toLowerCase();
    if (s.includes("cursor")) return "cursor";
    if (s.includes("codex") || s.includes("gpt") || s.includes("openai")) return "codex";
    if (s.includes("grok")) return "grok";
    if (s.includes("opencode")) return "opencode";
    return "openai";
  }
  function providerLogo(a, size = 24) {
    const id = providerOf(a);
    if (window.Logos && Logos.mark(id)) {
      return `<span class="qs-prov" style="--c:${PROVIDER_BG[id] || "#211e1e"};width:${size}px;height:${size}px" aria-hidden="true">${Logos.mark(id)}</span>`;
    }
    return botFace("#64748b", size, (a && a.title) || "Agent", "brain");
  }

  // ------------------------------------------------------------------ data
  const projectPath = () => (snap && snap.workspace) || "";
  const projectName = () => (projectPath() ? base(projectPath()) : "Workbench");
  const user = () => (window.__BOOT && window.__BOOT.user) || "there";
  const initials = (s) => {
    const words = String(s || "W").replace(/[\\/]+$/, "").split(/[\\/\s_-]+/).filter(Boolean);
    return (words.length > 1 ? words[0][0] + words[1][0] : words[0]?.slice(0, 2) || "W").toUpperCase();
  };
  const initialsPath = (p) => initials(base(p));
  const money = (n) => "$" + (Number(n) || 0).toFixed(2);
  const compact = (n) => {
    n = Number(n) || 0;
    if (n >= 1e6) return (n / 1e6).toFixed(1).replace(/\.0$/, "") + "m";
    if (n >= 1e3) return (n / 1e3).toFixed(1).replace(/\.0$/, "") + "k";
    return String(n);
  };
  const ago = (ts) => {
    if (!ts) return "";
    const d = Math.max(0, Date.now() - ts);
    const m = Math.round(d / 60000);
    if (m < 1) return "now";
    if (m < 60) return m + "m";
    const h = Math.round(m / 60);
    if (h < 24) return h + "h";
    const days = Math.round(h / 24);
    return days < 7 ? days + "d" : new Date(ts).toLocaleDateString([], { month: "short", day: "numeric" });
  };
  const clock = (ts) => new Date(ts).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  const today = () => new Date().toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" });
  const wake = (ms) => {
    if (!ms) return "not scheduled";
    const d = new Date(ms), now = new Date();
    const day = (x) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
    const days = Math.round((day(d) - day(now)) / 864e5);
    if (days <= 0) return "today at " + clock(d);
    if (days === 1) return "tomorrow at " + clock(d);
    if (days < 7) return d.toLocaleDateString([], { weekday: "short" }) + " at " + clock(d);
    return "in " + days + " days";
  };

  // ------------------------------------------------------------------ days
  const dayKey = (ts = Date.now()) => {
    const d = new Date(ts);
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  };
  const dayDate = (key) => {
    const [y, m, d] = String(key || dayKey()).split("-").map(Number);
    return new Date(y || new Date().getFullYear(), (m || 1) - 1, d || 1);
  };
  const shiftDay = (key, n) => { const d = dayDate(key); d.setDate(d.getDate() + n); return dayKey(d.getTime()); };
  const dayTitle = (key) => dayDate(key).toLocaleDateString("en-US", { weekday: "short", month: "short", day: "numeric" });
  const dayLong = (key) => dayDate(key).toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" });
  const isToday = (key) => key === dayKey();
  const dayChats = (key = todayDate) => chats().filter((t) => {
    if (dayKey(t.updated) !== key) return false;
    if (!t.project) return true;
    return !projectPath() || t.project === projectPath();
  }).sort((a, b) => b.updated - a.updated);
  const dayCount = (key) => dayChats(key).length;
  const dayRange = (list) => {
    if (!list.length) return { first: 0, last: 0, hours: 0 };
    const times = list.map((t) => t.updated).filter(Boolean).sort((a, b) => a - b);
    const first = times[0], last = times[times.length - 1];
    return { first, last, hours: Math.max(0, (last - first) / 36e5) };
  };

  // Per-agent extras the core model does not own yet: the fallback chain and
  // the switches from the agent sheet. Kept beside the app's own prefs.
  const allExtras = () => { try { return JSON.parse(localStorage.getItem(XKEY) || "{}") || {}; } catch { return {}; } };
  const extraOf = (id) => ({ fallbacks: [], effort: "high", fast: false, rotation: true, kind: "agent", ...(allExtras()[id] || {}) });
  const kindOf = (a, x) => {
    if (x && x.kind === "doctor") return "doctor";
    const text = `${(a && a.name) || ""} ${(a && a.job) || ""}`.toLowerCase();
    return /\b(doctor|medic)\b/.test(text) ? "doctor" : "agent";
  };
  const saveExtra = (id, patch) => {
    const all = allExtras();
    all[id] = { ...(all[id] || {}), ...patch };
    try { localStorage.setItem(XKEY, JSON.stringify(all)); } catch {}
  };
  const doctorAuto = () => { try { return localStorage.getItem(DKEY) !== "0"; } catch { return true; } };
  const setDoctorAuto = (on) => { try { localStorage.setItem(DKEY, on ? "1" : "0"); } catch {} };

  // ------------------------------------------------------------------ models
  const PROVIDERS = [
    ["claude", "Claude Code", "claude", "Claude"],
    ["codex", "Codex", "codex", "OpenAI"],
    ["cursor", "Cursor", "cursor", "Cursor"],
    ["grok", "Grok", "grok", "xAI"],
    ["opencode", "OpenCode", "opencode", "OpenCode"],
    ["ollama", "Ollama", "ollama", "Local"],
  ];
  const MODELS = {
    claude: [["claude-opus-4-6", "Opus 4.6"], ["claude-sonnet-4-6", "Sonnet 4.6"], ["claude-haiku-4-5", "Haiku 4.5"]],
    codex: [["gpt-5.1-codex", "GPT-5.1 Codex"], ["gpt-5.1", "GPT-5.1"], ["gpt-5-mini", "GPT-5 mini"]],
    cursor: [["cursor", "Composer"], ["gpt-5.1-codex", "GPT-5.1 Codex"]],
    grok: [["grok-4", "Grok 4"], ["grok-4-fast", "Grok 4 Fast"]],
    opencode: [["default", "Default"]],
    ollama: [["qwen3-coder", "Qwen3 Coder"], ["llama3.3", "Llama 3.3"]],
    openai: [["gpt-5.1", "GPT-5.1"], ["gpt-5.1-codex", "GPT-5.1 Codex"]],
  };
  const modelLabel = (route, provider) => {
    const id = (route && route.model) || "";
    const hit = Object.values(MODELS).flat().find(([v]) => v === id);
    if (hit) return hit[1];
    if (id) return id;
    const short = { claude: "Claude", codex: "OpenAI", cursor: "Cursor", grok: "Grok", opencode: "OpenCode", ollama: "Ollama" }[provider] || "Provider";
    return short + " default";
  };

  // ------------------------------------------------------------------ fleet
  function machineList() {
    const raw = (typeof machines !== "undefined" && Array.isArray(machines)) ? machines : [];
    return raw.map((m, i) => {
      const local = m.local === true || i === 0;
      const ssh = String(m.url || "").startsWith("ssh");
      const os = m.os || (local
        ? (/Win/i.test(navigator.userAgent) ? "windows" : /Mac/i.test(navigator.userAgent) ? "macos" : "linux")
        : ssh ? "linux" : "remote");
      return {
        ...m,
        local,
        os,
        kind: m.kind || (local ? "local" : ssh ? "ssh" : "remote"),
        state: (m.link && m.link.state) || "online",
        paths: Array.isArray(m.paths) && m.paths.length ? m.paths : (m.project ? [m.project] : []),
        detail: m.detail || (local ? "This machine" : m.url || "Remote harness"),
        tailscale: m.tailscale !== false,
        running: m.running || 0,
        pending: m.pending || 0,
      };
    });
  }
  const iconForMachine = (m) => m.local ? "laptop" : m.kind === "ssh" ? "server" : m.kind === "cloud" ? "cloud" : "monitor";
  const harnesses = () => (data.harnesses || []).filter((h) => h.kind === "cli" || (!h.kind && h.id));

  function doctorRun(auto) {
    if (doctorBusy) return;
    doctorBusy = true;
    const failed = chats().filter((t) => t.failed && !healedIds.has(t.id));
    const scanned = failed.length;
    let healed = 0;
    Promise.all(failed.slice(0, 6).map((t) =>
      invoke("chat_retry", { id: t.id }).then(() => { healed++; healedIds.add(t.id); }).catch(() => {})
    )).then(() => invoke("chat_purge_ghosts").catch(() => []))
      .then(() => {
        doctor = { at: Date.now(), healed, scanned };
        doctorBusy = false;
        if (healed) toast(`Doctor repaired ${healed} chat${healed === 1 ? "" : "s"}`, "ok");
        else if (!auto) toast(scanned ? "Doctor could not wake those chats" : "Doctor: nothing to repair", "ok");
        load(true);
      })
      .catch(() => { doctorBusy = false; });
  }
  function maybeDoctor() {
    if (page !== "team" || !doctorAuto() || doctorBusy) return;
    if (Date.now() - (doctor.at || 0) < 10 * 60e3) return;
    if (doctorAutoOnce === data.version) return;
    doctorAutoOnce = data.version;
    setTimeout(() => doctorRun(true), 900);
  }

  const workAgents = () => ((snap && snap.agents) || []).filter(a => a.kind !== "triage");
  const pending = () => ((snap && snap.approvals) || []).filter(a => a.state && a.state.state === "pending");
  const running = () => workAgents().filter(a => a.status === "running");
  const awaiting = () => workAgents().filter(a => a.status === "awaiting_approval");
  const bots = () => data.agents || [];
  const chats = () => data.chats || [];
  const routines = () => {
    const all = [];
    for (const a of data.routines.length ? data.routines : data.agents) {
      for (const r of a.routines || []) if (r.enabled && r.next_run > 0) all.push({ ...r, agent: a });
    }
    return all.sort((x, y) => x.next_run - y.next_run);
  };
  const nextRoutine = (a) => (a.routines || []).filter(r => r.enabled && r.next_run > 0).sort((x, y) => x.next_run - y.next_run)[0];
  const botThread = (a) => chats().filter(t => t.agent === a.id).sort((x, y) => y.updated - x.updated)[0];
  const botStatus = (a) => {
    const t = botThread(a);
    if (t && t.busy) return { label: "Working", tone: "work" };
    if (t && t.unread) return { label: "Needs you", tone: "need" };
    const r = nextRoutine(a);
    if (r) return { label: "Wakes " + wake(r.next_run), tone: "wake" };
    return { label: "Watching", tone: "idle" };
  };
  const unreadChats = () => chats().filter(t => t.unread).length;
  const needsCount = () => pending().length + awaiting().length + unreadChats();
  const tickets = () => ((snap && snap.tickets) || []);
  const byState = (...states) => tickets().filter(t => states.includes(t.state));

  async function load(force) {
    if (data.loading) return;
    if (!force && data.at && Date.now() - data.at < 4000) return;
    data.loading = true;
    try {
      const [agents, list, routines, git, harnesses] = await Promise.all([
        invoke("agents_list").catch(() => []),
        invoke("chat_list").catch(() => []),
        invoke("routines_sync", { offset: -new Date().getTimezoneOffset() }).catch(() => null),
        invoke("git_changes").catch(() => null),
        invoke("harnesses").catch(() => []),
      ]);
      data.agents = Array.isArray(agents) ? agents : [];
      data.chats = Array.isArray(list) ? list : [];
      data.routines = Array.isArray(routines) ? routines : data.agents;
      data.harnesses = Array.isArray(harnesses) ? harnesses : [];
      data.git = git && typeof git === "object" ? git : null;
      data.at = Date.now();
      data.version++;
      data.loading = false;
      render();
    } catch {
      data.loading = false;
    }
  }

  // ------------------------------------------------------------------ routing
  function current() { return page; }
  function go(next) {
    if (!PAGES[next]) next = "home";
    page = next;
    if (next === "today") todayDate = dayKey();
    try { localStorage.setItem(QKEY, page); } catch {}
    internal = true;
    if (next === "code") {
      closeSettingsIfOpen();
      S.mode = "code"; S.codeView = "agents";
      if (typeof setWb === "function") setWb("canvases");
      setMode("code");
      if (window.Shell) Shell.layout();
      if (typeof narrow !== "function" || !narrow()) setSide(true);
    } else if (next === "crew") {
      closeSettingsIfOpen();
      S.mode = "crew";
      setMode("crew");
      if (window.Shell) Shell.layout();
      if (typeof narrow !== "function" || !narrow()) setSide(true);
      // The face's own effects settle after the first paint; keep the agents
      // list in view rather than letting the old home collapse it.
      setTimeout(() => { try { if (typeof narrow !== "function" || !narrow()) setSide(true); } catch {} }, 0);
    } else if (next === "plugins" || next === "skills" || next === "apps" || next === "memory") {
      closeSettingsIfOpen();
      S.mode = next;
      setMode(next);
      if (window.Shell) Shell.layout();
    }
    render();
    if (next === "home" || next === "work") load(true);
    internal = false;
    try { if (typeof narrow !== "function" || !narrow()) setSide(true); } catch {}
  }

  // The shell's own callers (search, project open, a thread, a shortcut) still
  // speak `setMode`; keep the page in step without letting our own `go` loop.
  function fromMode(m) {
    if (!ready || internal) return;
    let next = null;
    if (m === "code") {
      next = "code";
      if (S.codeView === "pair" && codeSurface !== "panes") {
        codeSurface = "panes";
        try { localStorage.setItem(CKEY, codeSurface); } catch {}
      }
    }
    else if (m === "crew") next = "crew";
    else if (m === "plugins" || m === "skills" || m === "apps" || m === "memory") next = m;
    if (!next || next === page) return;
    page = next;
    try { localStorage.setItem(QKEY, page); } catch {}
    render();
  }
  const closeSettingsIfOpen = () => { if (S.settings && window.closeSettings) closeSettings(); };

  function newSession() {
    closeSettingsIfOpen();
    codeSurface = "panes"; try { localStorage.setItem(CKEY, codeSurface); } catch {}
    S.mode = "code"; S.codeView = "pair";
    setCodeView("pair");
    if (window.Chat && Chat.newChat) Chat.newChat();
    page = "code"; try { localStorage.setItem(QKEY, page); } catch {}
    if (window.Shell) Shell.layout();
    render();
  }

  // ------------------------------------------------------------------ rail
  function renderRail() {
    const el = $("#qRail"); if (!el) return;
    const projects = (prefs.projects || []).slice(0, 5);
    el.innerHTML = `
      <button class="qr-logo${page === "home" ? " on" : ""}" data-q-go="home" aria-label="Workbench home" title="Workbench home">${ic("logo")}</button>
      <div class="qr-sep"></div>
      ${projects.map(p => `<button class="qr-proj${p === projectPath() ? " on" : ""}" data-q-project="${esc(p)}" title="${esc(base(p))}" aria-label="${esc(base(p))}" data-q-tip="${esc(base(p))}">${esc(initialsPath(p))}</button>`).join("")}
      <button class="qr-add" data-q-add-project aria-label="Add project" title="Add project">${ic("plus")}</button>`;
  }

  // ------------------------------------------------------------------ top bar
  function renderTop() {
    const el = $("#qTop"); if (!el) return;
    const crumb = page === "home" ? `<b>${esc(PAGES[page])}</b>`
      : `<span>${esc(projectName())}</span><span class="q-sep">/</span><b>${esc(PAGES[page] || "Home")}</b>`;
    el.innerHTML = `
      <button class="qicon" id="qCollapse" aria-label="Collapse sidebar" title="Collapse sidebar">${ic("panel")}</button>
      <div class="q-crumb">${crumb}</div>
      <span class="q-grow"></span>
      <div class="qnav" id="qNav" role="toolbar" aria-label="Workspace"></div>
      ${page === "code" ? `<div class="q-viewseg"><button data-q-sview="session" class="${sessionOn() ? "on" : ""}">Session</button><button data-q-sview="panes" class="${sessionOn() ? "" : "on"}">Panes</button></div>` : ""}
      ${page === "code" && !sessionOn() ? `<button class="qicon" data-q-panel aria-label="Side panel" title="History, files and sessions">${ic("panel")}</button>` : ""}
      <button class="qicon" data-q-search aria-label="Search" title="Search">${ic("search")}</button>
      <button class="qicon" data-q-more aria-label="More" title="More">${ic("more")}</button>`;
    const btn = $("#qCollapse");
    if (btn) btn.onclick = () => {
      const w = $("#win");
      const closed = typeof narrow === "function" && narrow() ? !w.classList.contains("side-open") : w.classList.contains("side-closed");
      setSide(closed);
    };
  }

  // ------------------------------------------------------------------ global sidebar nav
  // The mode switcher and the five work links live above every page, not just
  // Home, so the same routes are reachable from Code, Crew, Canvas and Settings.
  function renderNav() {
    const el = $("#qNav"); if (!el) return;
    const modes = [["code", "Code", "code"], ["work", "Work", "work"], ["canvas", "Canvas", "canvas"], ["crew", "Crew", "crew"]];
    el.innerHTML = `
      <div class="qnav-modes" role="group" aria-label="Modes">
        ${modes.map(([k, l, icon]) => `<button data-q-go="${k}" class="qnav-ic${page === k ? " on" : ""}" aria-label="${l}" aria-current="${page === k ? "page" : "false"}" data-q-tip="${l}">${ic(icon)}</button>`).join("")}
      </div>`;
  }

  function renderWork() {
    const el = $("#qWork"); if (!el) return;
    const links = [
      ["needs", "Needs you", "hand", needsCount()],
      ["board", "Agent board", "board", tickets().length],
      ["review", "Code review", "check", byState("ready_for_human").length],
      ["automate", "Automations", "zap", routines().length],
      ["limits", "Limits", "limits", ""],
    ];
    el.innerHTML = links.map(([k, l, icon, n]) => `<button data-q-go="${k}" class="qwork-ic${page === k ? " on" : ""}" aria-label="${n ? `${l}, ${n} waiting` : l}" aria-current="${page === k ? "page" : "false"}" data-q-tip="${n ? `${l} (${n})` : l}">${ic(icon)}${n ? `<i class="qwork-dot"></i>` : ""}</button>`).join("");
  }

  // ------------------------------------------------------------------ bottom rail
  function renderDock() {
    const el = $("#qDock"); if (!el) return;
    const skin = skinName();
    const dark = skin === "dark" || skin === "glass";
    const m = typeof machine === "function" ? machine() : { name: "This Mac", link: { state: "online" } };
    const online = m && m.link && m.link.state === "online";
    el.innerHTML = `
      <span class="qd-left"><span class="qd-mach" title="${esc((m && m.name) || "This Mac")}"><i class="${online ? "on" : ""}"></i>${esc((m && m.name) || "This Mac")}</span></span>
      <span class="qd-right">
        <button class="qicon" data-q-go="settings" aria-label="Settings" data-q-tip="Settings">${ic("gear")}</button>
        <button class="qicon${dark ? " on" : ""}" data-q-theme="${dark ? "light" : "dark"}" aria-label="${dark ? "Light mode" : "Night mode"}" data-q-tip="${dark ? "Light mode" : "Night mode"}">${dark ? ic("sun") : ic("moon")}</button>
        <button class="qicon${skin === "glass" ? " on" : ""}" data-q-skin="glass" aria-label="Glass" data-q-tip="Glass">${ic("drop")}</button>
        <button class="qicon" data-q-help aria-label="Help" data-q-tip="Help">${ic("help")}</button>
        <button class="qd-user" data-q-settings aria-label="Account" title="${esc(user())}">${esc(initials(user()))}</button>
        <span class="qd-rule" aria-hidden="true"></span>
        <span class="qd-places" role="group" aria-label="Places">
          <button class="qicon${page === "today" ? " on" : ""}" data-q-go="today" aria-label="Today" data-q-tip="Today">${ic("calendar")}</button>
          <button class="qicon${page === "machines" ? " on" : ""}" data-q-go="machines" aria-label="Machines" data-q-tip="Machines">${ic("server")}</button>
          <button class="qicon${page === "team" ? " on" : ""}" data-q-go="team" aria-label="Team" data-q-tip="Team">${ic("crew")}</button>
        </span>
      </span>`;
  }

  // ------------------------------------------------------------------ vertical session tabs
  function renderTabs() {
    const el = $("#qTabs"); if (!el) return;
    const list = chats().slice().sort((a, b) => b.updated - a.updated).slice(0, 7);
    // Retired 2026-11-05: the strip was never styled and repeats the sidebar's
    // Sessions list; inside the one sheet it pushed the page below the sidebar.
    const show = false && list.length > 0 && page !== "settings";
    el.hidden = !show;
    const w = $("#win");
    if (w) w.classList.toggle("q-tabs-on", show);
    if (!show) { el.innerHTML = ""; return; }
    const active = page === "code" ? sessionTitle() : "";
    el.innerHTML = `<span class="qtabs-h" title="${list.length} current sessions">${ic("panel")}<em>${list.length}</em></span>` +
      list.map((t) => {
        const on = active && t.title === active;
        return `<button class="qtab${on ? " on" : ""}" data-q-thread="${esc(t.id)}" title="${esc(t.title || "New chat")}" aria-label="${esc(t.title || "New chat")}">${esc(initials(t.title || "Chat"))}<i class="qdot ${toneOf(t)}"></i></button>`;
      }).join("");
  }

  // ------------------------------------------------------------------ sidebar
  function navRow(label, target, icon, count, active) {
    return `<button class="qn-row${active ? " on" : ""}" data-q-go="${target}">${ic(icon)}<span>${label}</span>${count ? `<em>${count}</em>` : ""}</button>`;
  }

  function renderSide() {
    const el = $("#qSide"); if (!el) return;
    if (page === "crew") { el.innerHTML = ""; return; }
    if (page === "code" && !sessionOn()) { el.innerHTML = ""; return; }
    const p = (prefs.projects || []);
    if (page === "code") {
      const ags = workAgents();
      const lead = ags[0];
      const chats = data.chats.filter((t) => !(t.members && t.members.length)).sort((a, b) => b.updated - a.updated);
      const cloud = chats.filter((t) => t.route && t.route.kind === "cloud");
      const local = chats.filter((t) => !(t.route && t.route.kind === "cloud"));
      const row = (t, isCurrent = false) => `<button class="qs-session${isCurrent ? " on" : ""}" ${isCurrent ? 'data-q-focus-session' : `data-q-thread="${esc(t.id)}"`}>
        <span class="qs-session-t">${esc(isCurrent ? sessionTitle() : t.title || "New chat")}</span>
        <span class="qs-session-m">${ic(t.route && t.route.kind === "cloud" ? "cloud" : "laptop")}<i class="qdot ${isCurrent ? (lead && lead.status === "running" ? "work" : "idle") : t.busy ? "work" : t.unread ? "need" : "idle"}"></i>${esc(isCurrent ? (lead ? agentName(lead) + " · " + (lead.status || "idle") : "No session") : t.busy ? "Working" : t.unread ? "Needs you" : "Idle")}<span class="qs-time">${esc(isCurrent ? "now" : ago(t.updated))}</span></span>
      </button>`;
      el.innerHTML = `
        <button class="qs-new" data-q-new-chat>${ic("plus")}New session<kbd>⌘N</kbd></button>
        <div class="qs-head"><b>Sessions</b><span class="grow"></span><button class="qs-icon" data-q-toast="Sessions are ordered by recent activity" aria-label="Sort sessions">${ic("sort")}</button><button class="qs-icon" data-q-search aria-label="Search sessions">${ic("search")}</button></div>
        <div class="qs-loc">${ic("laptop")}This Mac<span>${local.length + 1}</span></div>
        <div class="qs-list">
          ${row({ id: "", title: sessionTitle(), updated: Date.now(), route: null }, true)}
          ${local.map((t) => row(t)).join("")}
        </div>
        ${cloud.length ? `<div class="qs-loc">${ic("cloud")}Cloud<span>${cloud.length}</span></div><div class="qs-list">${cloud.map((t) => row(t)).join("")}</div>` : ""}
        ${(() => {
          const remotes = (typeof machines !== "undefined" ? machines : []).filter((m) => !m.local);
          return remotes.map((m) => `<div class="qs-loc">${ic(m.kind === "ssh" ? "server" : "monitor")}${esc(m.name)}<span>${(m.running || 0) + (m.pending || 0)}</span></div>${m.project ? `<div class="qs-list"><button class="qs-session" data-q-settings-section="machines"><span class="qs-session-t">${esc(base(m.project))}</span><span class="qs-session-m">${ic("monitor")}${m.running || 0} running<span class="qs-time">remote</span></span></button></div>` : ""}`).join("");
        })()}
        <button class="qs-add" data-q-settings-section="machines">${ic("plus")}Connect machine<span class="qs-kbd">CLOUD / SSH</span></button>
        <div class="qs-sidefoot">
          <div class="qs-sidetools">
            <button class="qs-icon" data-q-theme="light" aria-label="Light theme" title="Light">${ic("sun")}</button>
            <button class="qs-icon" data-q-update aria-label="Check for updates" title="Check for updates">${ic("download")}</button>
          </div>
          <button class="qs-get" data-q-onboard>
            <span class="qs-get-ic">${ic("check")}</span>
            <span><b>Getting started</b><small>${prefs.onboarded ? "5 of 5" : "Continue setup"}</small></span>
            <span class="qs-get-bar"><i style="width:${prefs.onboarded ? 100 : 20}%"></i></span>
          </button>
        </div>`;
      return;
    }
    if (page === "canvas") {
      el.innerHTML = `
        <div class="qs-head"><b>Canvas</b><button class="qs-icon" data-q-add-canvas title="New canvas">${ic("plus")}</button></div>
        <div class="qs-list">${(prefs.tabs || []).map((t, i) => `
          <button class="qs-session${i === prefs.active_tab ? " on" : ""}" data-q-tab="${i}">
            <span class="qs-session-t">${esc(t.name || "Canvas " + (i + 1))}</span>
            <span class="qs-session-m">${t.panes.length} pane${t.panes.length === 1 ? "" : "s"} · ${esc(t.panes.map(x => x.kind).join(", "))}</span>
          </button>`).join("") || `<p class="qs-empty">No canvases yet.</p>`}</div>
        <button class="qs-add" data-q-add-canvas>${ic("plus")}New canvas</button>`;
      return;
    }
    if (page === "today") {
      const today = dayKey();
      const days = Array.from({ length: 7 }, (_, i) => shiftDay(today, -i));
      if (!days.includes(todayDate)) days.push(todayDate);
      days.sort((a, b) => (a < b ? 1 : -1));
      el.innerHTML = `
        <div class="qs-head"><b>Days</b><button class="qs-icon" data-q-go="today" title="Today">${ic("calendar")}</button></div>
        <div class="qs-list">
          ${days.map((k) => {
            const n = dayCount(k);
            return `<button class="qs-session${k === todayDate ? " on" : ""}" data-q-day="${k}">
              <span class="qs-session-t">${k === today ? "Today" : dayTitle(k)}</span>
              <span class="qs-session-m">${n ? `${n} chat${n === 1 ? "" : "s"}` : "Quiet"}${k === today ? " · " + dayDate(k).toLocaleDateString("en-US", { weekday: "long" }) : ""}</span>
            </button>`;
          }).join("")}
        </div>
        <button class="qs-add" data-q-go="home">${ic("home")}Back to HQ</button>`;
      return;
    }
    if (page === "machines") {
      const list = machineList();
      el.innerHTML = `
        <div class="qs-head"><b>Fleet</b><button class="qs-icon" data-q-settings-section="machines" title="Add a machine">${ic("plus")}</button></div>
        <div class="qs-list">
          ${list.map((m) => `<button class="qs-session" data-q-go="machines">
            <span class="qs-session-t">${esc(m.name)}</span>
            <span class="qs-session-m"><i class="qdot ${m.state === "online" ? "ok" : "idle"}"></i>${esc(m.os)} · ${esc(m.state)}${m.running ? ` · ${m.running} running` : ""}</span>
          </button>`).join("") || `<p class="qs-empty">No machines paired yet.</p>`}
        </div>
        <div class="qs-group"><div class="qs-label">Pinned</div>
          ${navRow("Harnesses", "machines", "terminal", harnesses().length || "", true)}
        </div>
        <button class="qs-add" data-q-go="home">${ic("home")}Back to HQ</button>`;
      return;
    }
    if (page === "team") {
      el.innerHTML = `
        <div class="qs-head"><b>Team</b><button class="qs-icon" data-q-go="crew" title="Open Crew">${ic("crew")}</button></div>
        <div class="qs-list">
          ${bots().map((a) => {
            const x = extraOf(a.id), st = botStatus(a);
            return `<button class="qs-session" data-q-agent-edit="${esc(a.id)}">
              <span class="qs-session-t">${esc(a.name)}</span>
              <span class="qs-session-m"><i class="qdot ${st.tone === "work" ? "work" : st.tone === "need" ? "need" : "idle"}"></i>${esc(kindOf(a, x) === "doctor" ? "Doctor" : st.label)} · ${esc(modelLabel(a.route, providerOf(a)))}</span>
            </button>`;
          }).join("") || `<p class="qs-empty">No bots yet.</p>`}
        </div>
        <button class="qs-add" data-q-go="home">${ic("home")}Back to HQ</button>`;
      return;
    }
    if (page === "work") {
      const rows = chats().slice().sort((a, b) => b.updated - a.updated).slice(0, 7);
      el.innerHTML = `
        <div class="qs-head"><b>Recent</b><button class="qs-icon" data-q-search title="Search">${ic("search")}</button></div>
        <div class="qs-list">${rows.length ? rows.map(t => `
          <button class="qs-session" data-q-thread="${esc(t.id)}">
            <span class="qs-session-t">${esc(t.title || "New chat")}</span>
            <span class="qs-session-m"><i class="qdot ${t.busy ? "work" : t.unread ? "need" : "idle"}"></i>${esc(t.busy ? "Working" : t.unread ? "Needs you" : "Idle")} · ${esc(ago(t.updated))}</span>
          </button>`).join("") : `<p class="qs-empty">No sessions yet.</p>`}</div>
        <button class="qs-link" data-q-go="code">${ic("chev")}View all sessions</button>`;
      return;
    }
    if (page === "needs" || page === "board" || page === "review" || page === "automate" || page === "inbox" || page === "calendar" || page === "tasks" || page === "limits" || page === "plugins" || page === "skills" || page === "settings") {
      el.innerHTML = `
        <div class="qs-head"><b>Work</b><button class="qs-icon" data-q-go="home" title="Home">${ic("home")}</button></div>
        <div class="qs-group">
          ${navRow("Needs you", "needs", "hand", needsCount() || "", page === "needs")}
          ${navRow("Board", "board", "grid", tickets().length || "", page === "board")}
          ${navRow("Review", "review", "check", byState("ready_for_human").length || "", page === "review")}
          ${navRow("Automate", "automate", "zap", routines().length || "", page === "automate")}
        </div>
        <div class="qs-group">
          ${navRow("Inbox", "inbox", "inbox", unreadChats() || "", page === "inbox")}
          ${navRow("Calendar", "calendar", "calendar", routines().length || "", page === "calendar")}
          ${navRow("Tasks", "tasks", "tasks", tickets().length || "", page === "tasks")}
        </div>
        <div class="qs-group">
          ${navRow("Plugins", "plugins", "plugins", "", page === "plugins")}
          ${navRow("Skills", "skills", "skills", "", page === "skills")}
          ${navRow("Settings", "settings", "gear", "", page === "settings")}
        </div>
        <button class="qs-add" data-q-go="home">${ic("home")}Back to HQ</button>`;
      return;
    }

    // Home / the default page sidebar: the full navigation.
    const skin = skinName();
    const done = prefs.onboarded ? 5 : 1;
    el.innerHTML = `
      <div class="qs-profile">
        <span class="qs-avatar">${esc(initials(user()))}</span>
        <span class="qs-who"><b>${esc(user())}</b><small>Pro · ${esc(typeof machine === "function" && machine().local === false ? "Cloud" : "This Mac")}</small></span>
      </div>
      <label class="qs-search">${ic("search")}<input id="qSearch" placeholder="Search Workbench" aria-label="Search Workbench"><kbd>⌘K</kbd></label>
      <div class="qs-group">
        ${navRow("HQ", "home", "home", "", page === "home")}
        ${navRow("Today", "today", "calendar", "", page === "today")}
        ${navRow("Inbox", "inbox", "inbox", unreadChats() || "", page === "inbox")}
        ${navRow("Calendar", "calendar", "calendar", routines().length || "", page === "calendar")}
        ${navRow("Tasks", "tasks", "tasks", tickets().length || "", page === "tasks")}
      </div>
      <div class="qs-group">
        <div class="qs-label">Projects<button data-q-add-project aria-label="Add project">${ic("plus")}</button></div>
        ${p.slice(0, 5).map(x => `<button class="qn-row${x === projectPath() ? " on" : ""}" data-q-project="${esc(x)}">${ic("folder")}<span>${esc(base(x))}</span></button>`).join("") || `<p class="qs-empty">No projects yet.</p>`}
      </div>
      <div class="qs-group">
        <div class="qs-label">Add-ons</div>
        ${navRow("Plugins", "plugins", "plugins", "", page === "plugins")}
        ${navRow("Skills", "skills", "skills", "", page === "skills")}
        ${navRow("Team", "team", "crew", bots().length || "", page === "team")}
        ${navRow("Machines", "machines", "server", machineList().length || "", page === "machines")}
        ${navRow("Settings", "settings", "gear", "", page === "settings")}
      </div>
      <div class="qs-foot">
        <button class="qs-get" data-q-onboard>
          <span class="qs-get-ic">${ic("check")}</span>
          <span><b>Getting started</b><small>${done} of 5</small></span>
          <span class="qs-get-bar"><i style="width:${done * 20}%"></i></span>
        </button>
      </div>`;
    const search = $("#qSearch");
    if (search) {
      search.onclick = (e) => { e.preventDefault(); if (window.Shell) Shell.search(); };
      search.onkeydown = (e) => { if (e.key === "Enter" || e.key === "k" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); if (window.Shell) Shell.search(); } };
    }
  }

  // ------------------------------------------------------------------ pages
  function head(title, sub, actions = "", eyebrow = "") {
    return `<header class="q-head">
      <div class="q-eyebrow"><span class="q-pip"></span>${eyebrow || esc(projectName())}</div>
      <h1>${title}</h1>
      <p>${sub}</p>
      ${actions ? `<div class="q-actions">${actions}</div>` : ""}
    </header>`;
  }

  function homePage() {
    const runs = running(), need = pending(), botsBusy = chats().filter(t => t.busy);
    const runningText = runs.length
      ? `${runs.slice(0, 3).map(a => a.id === 0 ? "Claude Code" : a.title).join(", ")} ${runs.length === 1 ? "is" : "are"} working on ${esc(projectName())}.`
      : botsBusy.length ? `${botsBusy.slice(0, 2).map(t => t.title).join(" and ")} ${botsBusy.length === 1 ? "is" : "are"} working.` : "No sessions are running right now.";
    const needText = need.length ? `${need.length} plan${need.length === 1 ? "" : "s"} or deliverable${need.length === 1 ? "" : "s"} waiting for your review.`
      : unreadChats() ? `${unreadChats()} agent repl${unreadChats() === 1 ? "y" : "ies"} waiting for you.` : "Nothing is waiting on you.";
    const tokens = workAgents().reduce((s, a) => s + (a.input_tokens || 0) + (a.output_tokens || 0), 0);
    const rows = chats().slice().sort((a, b) => b.updated - a.updated).slice(0, 4);
    const routinesNow = routines();
    const next = routinesNow[0];
    return `<div class="q-page q-home">
      ${head(projectName() === "Workbench" ? "Workbench" : projectName(),
        `${runningText} ${needText}`,
        `<button class="q-btn dark" data-q-new>New session ${ic("plus")}</button>
         <button class="q-btn" data-q-go="code">Open Code</button>
         <button class="q-btn" data-q-go="crew">Ask the crew</button>`,
        esc(today()))}
      <div class="q-cards">
        <button class="q-card" data-q-go="code">
          <span class="q-card-h"><i class="qdot work"></i>Running now</span>
          <b class="q-stat">${runs.length + botsBusy.length} session${runs.length + botsBusy.length === 1 ? "" : "s"}</b>
          <span class="q-card-b">${runningText}</span>
          <span class="q-card-f">${bots().length || runs.length} agent${(bots().length || runs.length) === 1 ? "" : "s"} · ${esc(projectName())} ${ic("chev")}</span>
        </button>
        <button class="q-card" data-q-go="needs">
          <span class="q-card-h"><i class="qdot need"></i>Needs you</span>
          <b class="q-stat">${needsCount()}</b>
          <span class="q-card-b">${needText}</span>
          <span class="q-card-f">Open Needs you ${ic("chev")}</span>
        </button>
        <button class="q-card" data-q-go="limits">
          <span class="q-card-h">${ic("chart")}This session</span>
          <b class="q-stat">${money(snap.total_cost_usd)}</b>
          <span class="q-card-b">${compact(tokens)} tokens across ${workAgents().length || 0} agent${workAgents().length === 1 ? "" : "s"}. Router ${money(snap.router_cost_usd)}.</span>
          <span class="q-card-f">Open Limits ${ic("chev")}</span>
        </button>
      </div>
      <div class="q-sec-head"><b>Recent sessions</b><button data-q-go="work">View all ${ic("chev")}</button></div>
      <div class="q-list">
        ${rows.length ? rows.map((t, i) => `<button class="q-row" data-q-thread="${esc(t.id)}">
          ${botFace(["#8b5cf6", "#14b8a6", "#ef4444", "#f59e0b"][i % 4], 28, t.title || "Chat", WALL_SHAPES[i % WALL_SHAPES.length])}
          <span class="q-row-main"><b>${esc(t.title || "New chat")}</b><small><i class="qdot ${t.busy ? "work" : (t.unread || t.failed) ? "need" : "idle"}"></i>${esc(t.agent ? "Agent thread" : t.members && t.members.length ? "Group chat" : "Session")} · ${esc(t.busy ? "working" : (t.unread || t.failed) ? "needs you" : "idle")}</small></span>
          <span class="q-row-end">${esc(ago(t.updated))}</span>
        </button>`).join("") : `<div class="q-empty">No sessions yet. Start one with “New session”.</div>`}
      </div>
      <button class="q-brief" data-q-go="work">
        <span class="q-brief-ic">${ic("zap")}</span>
        <span class="q-brief-copy"><b>Morning brief</b><small>${next ? `${routinesNow.length} routine${routinesNow.length === 1 ? "" : "s"} scheduled · next ${esc(wake(next.next_run))}` : "No routines scheduled yet."}</small></span>
        <span class="q-btn sm">Open Work</span>
      </button>
    </div>`;
  }

  function workPage() {
    const need = pending(), awaitingAgents = awaiting();
    const inProgress = byState("in_progress", "queued", "assigned");
    const review = byState("ready_for_human");
    const rs = routines();
    return `<div class="q-page">
      ${head("Work", "What needs you, what’s in flight, and what got done.",
        `<button class="q-btn dark" data-q-new>New session ${ic("plus")}</button>
         <button class="q-btn" data-q-go="board">Open board</button>`)}
      <div class="q-two">
        <section class="q-panel">
          <div class="q-sec-head"><b>Needs you</b><span class="q-count">${need.length + awaitingAgents.length}</span></div>
          ${need.length ? need.slice(0, 4).map(a => `<button class="q-row" data-q-ap="${a.id}">
            <span class="q-row-ic need">${ic("hand")}</span>
            <span class="q-row-main"><b>${a.kind === "plan" ? "Approve the plan" : "Review the deliverable"}</b><small>${a.tickets.length} ticket${a.tickets.length === 1 ? "" : "s"} · agent ${a.agent}</small></span>
            <span class="q-row-end">Review</span>
          </button>`).join("") : ""}
          ${awaitingAgents.slice(0, 3).map(a => `<button class="q-row" data-q-agent="${a.id}">
            <span class="q-row-ic need">${ic("zap")}</span>
            <span class="q-row-main"><b>${esc(a.id === 0 ? "Main agent" : a.title)}</b><small>Waiting for your review</small></span>
            <span class="q-row-end">Open</span>
          </button>`).join("")}
          ${!need.length && !awaitingAgents.length ? `<div class="q-empty">Nothing waiting. Go make something.</div>` : ""}
        </section>
        <section class="q-panel">
          <div class="q-sec-head"><b>In progress</b><span class="q-count">${inProgress.length}</span></div>
          ${inProgress.length ? inProgress.slice(0, 5).map(t => `<button class="q-row" data-q-ticket="${esc(t.key)}"><span class="q-row-ic">${ic("work")}</span><span class="q-row-main"><b>${esc(t.title)}</b><small>${esc(t.key)} · ${esc(t.state.replace(/_/g, " "))}</small></span><span class="q-row-end">Open</span></button>`).join("") : `<div class="q-empty">No tickets in progress.</div>`}
        </section>
      </div>
      <div class="q-two">
        <section class="q-panel">
          <div class="q-sec-head"><b>In review</b><span class="q-count">${review.length}</span></div>
          ${review.length ? review.slice(0, 5).map(t => `<button class="q-row" data-q-ticket="${esc(t.key)}"><span class="q-row-ic ok">${ic("check")}</span><span class="q-row-main"><b>${esc(t.title)}</b><small>${esc(t.key)} · ready for a human</small></span><span class="q-row-end">Open</span></button>`).join("") : `<div class="q-empty">Nothing in review.</div>`}
        </section>
        <section class="q-panel">
          <div class="q-sec-head"><b>Automations</b><span class="q-count">${rs.length}</span></div>
          ${rs.length ? rs.slice(0, 4).map(r => `<div class="q-row">
            <span class="q-row-ic">${ic("clock")}</span>
            <span class="q-row-main"><b>${esc(r.name)}</b><small>${esc(r.agent.name)} · wakes ${esc(wake(r.next_run))}</small></span>
            <button class="q-btn sm" data-q-run="${esc(r.agent.id)}|${esc(r.id)}">Run now</button>
          </div>`).join("") : `<div class="q-empty">No automations yet. Add a routine to a bot.</div>`}
          <button class="q-add-row" data-q-go="automate">${ic("plus")}Manage automations</button>
        </section>
      </div>
    </div>`;
  }

  // ------------------------------------------------------------------ today
  function chatBot(t) {
    const list = bots();
    if (t.agent) return list.find((a) => a.id === t.agent) || null;
    if (t.members && t.members.length) return list.find((a) => a.id === t.members[0]) || null;
    return null;
  }
  const botColor = (a) => (a && a.avatar && a.avatar.color) || "#8b5cf6";
  function chatFace(t, size = 34) {
    const a = chatBot(t);
    return botFace(botColor(a), size, (a && a.name) || t.title || "Chat");
  }
  const toneOf = (t) => t.busy ? "work" : (t.unread || t.failed) ? "need" : "idle";
  const duration = (h) => {
    if (!h) return "—";
    const hh = Math.floor(h), mm = Math.round((h - hh) * 60);
    return `${hh}h ${String(mm).padStart(2, "0")}m`;
  };
  function todayGroups(list) {
    if (todayGroup === "project") {
      const map = new Map();
      for (const t of list) {
        const k = t.project || projectPath() || "Elsewhere";
        if (!map.has(k)) map.set(k, []);
        map.get(k).push(t);
      }
      return [...map.entries()].map(([k, rows]) => [base(k) || k, rows]);
    }
    return [
      ["Working", list.filter((t) => t.busy)],
      ["Needs you", list.filter((t) => !t.busy && (t.unread || t.failed))],
      ["Later", list.filter((t) => !t.busy && !t.unread && !t.failed)],
    ];
  }

  function todayPage() {
    const list = dayChats();
    const span = dayRange(list);
    const groups = todayGroups(list);
    const pendingCount = pending().length;
    const reviewCount = byState("ready_for_human").length;
    const doneCount = byState("done", "approved").length;
    const approvals = (snap && snap.approvals) || [];
    const approved = approvals.filter((a) => a.state && a.state.state === "approved").length;
    const decided = approvals.length;
    const decisionPct = decided ? Math.round((approved / decided) * 100) : 0;
    const hours = new Set(list.map((t) => new Date(t.updated).getHours()));
    const nowHour = isToday(todayDate) ? new Date().getHours() : -1;
    const leads = bots().slice(0, 4);
    const fleet = machineList();
    return `<div class="q-page q-wide q-today">
      <header class="qt-head">
        <div class="qt-head-copy">
          <h1 class="qt-date">${esc(dayDate(todayDate).toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" }))}</h1>
          <p class="qt-sub">${esc(projectName())}<span>·</span>${list.length} chat${list.length === 1 ? "" : "s"} touched<span>·</span>Updated ${new Date().toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</p>
        </div>
        <div class="qt-daynav">
          <button data-q-day="-1" aria-label="Previous day" title="Previous day">${ic("chev", "flip")}</button>
          <input id="qDayPick" type="date" value="${todayDate}" aria-label="Pick a day">
          <button data-q-day="1" aria-label="Next day" title="Next day">${ic("chev")}</button>
          ${isToday(todayDate) ? "" : `<button class="qt-todaybtn" data-q-day="${dayKey()}">Today</button>`}
        </div>
      </header>
      <div class="qt-layout">
        <section class="qt-main">
          <div class="qt-toolbar">
            <b>Chats</b><span class="qt-count">${list.length}</span>
            <span class="qt-filter">Group
              <select id="qDayGroup" aria-label="Group chats">
                <option value="status"${todayGroup === "status" ? " selected" : ""}>Status</option>
                <option value="project"${todayGroup === "project" ? " selected" : ""}>Project</option>
              </select>
            </span>
          </div>
          ${list.length ? groups.map(([name, rows]) => rows.length ? `<div class="qt-group">
            <div class="qt-group-h"><span class="chev">${ic("chev")}</span><i class="qdot ${name === "Working" ? "work" : name === "Needs you" ? "need" : "idle"}"></i><b>${esc(name)}</b><em>${rows.length}</em></div>
            <div class="qt-chats">
              ${rows.map((t) => `<button class="qt-row" data-q-thread="${esc(t.id)}">
                <span class="qt-row-av">${chatFace(t)}</span>
                <span class="qt-row-main">
                  <b>${esc(t.title || "New chat")}<span>${esc(t.project ? base(t.project) : projectName())}</span></b>
                  <small>${esc(t.preview || (t.members && t.members.length ? `${t.members.length} bots in this room` : "No messages yet"))}</small>
                </span>
                <span class="qt-row-end">${clock(t.updated)}<i class="qdot ${toneOf(t)}"></i></span>
              </button>`).join("")}
            </div>
          </div>` : "").join("") : `<div class="qt-empty">A quiet day in ${esc(projectName())}.<br>Start a session or pick another day above.</div>`}
          <div class="qt-dock">
            <span>${ic("server")}<b>${fleet.filter((m) => m.state === "online").length}</b>/${fleet.length} machines online</span>
            <span>${ic("terminal")}<b>${harnesses().length}</b> harnesses ready</span>
            <span class="sp"></span>
            <span class="qt-dock-faces">${leads.map((a) => botFace(botColor(a), 20, a.name)).join("")}</span>
            <button data-q-go="team">${leads.length ? leads.map((a) => a.name.split(" ")[0]).slice(0, 2).join(" & ") : "Meet your team"} ${ic("chev")}</button>
          </div>
        </section>
        <aside class="qt-side">
          <section class="qt-card">
            <span class="qt-card-h">${ic("clock")}Worked so far</span>
            <b class="qt-big">${duration(span.hours)}</b>
            <div class="qt-fine">${span.first ? `${clock(span.first)} – ${clock(span.last)}` : "No activity logged"}</div>
            <div class="qt-hours">${Array.from({ length: 24 }, (_, i) => `<i class="${hours.has(i) ? "on" : ""}${i === nowHour ? " now" : ""}"></i>`).join("")}</div>
            <div class="qt-axis"><span>0</span><span>6</span><span>12</span><span>18</span><span>24</span></div>
          </section>
          <section class="qt-card">
            <span class="qt-card-h">${ic("inbox")}Messages</span>
            <div class="qt-pair">
              <div><b>${list.length}</b><small>sent</small></div>
              <div><b>${list.filter((t) => t.preview).length}</b><small>replies</small></div>
            </div>
            <div class="qt-line"><i style="width:${list.length ? Math.max(6, Math.round((list.filter((t) => t.preview).length / list.length) * 100)) : 0}%"></i></div>
          </section>
          <section class="qt-card">
            <span class="qt-card-h">${ic("check")}To-dos</span>
            <div class="qt-pair">
              <div><b>${doneCount}</b><small>finished by agents</small></div>
              <div><b>${reviewCount}</b><small>waiting for review</small></div>
            </div>
            <div class="qt-todo">
              <div class="qt-todo-row"><span>Accepted by you</span><b>${approved}</b></div>
              <div class="qt-todo-row"><span>Needs your call</span><b>${pendingCount}</b></div>
              <div class="qt-todo-row"><span>Added</span><b>${tickets().length}</b></div>
            </div>
          </section>
          <section class="qt-card">
            <span class="qt-card-h">Decisions</span>
            <div class="qt-donut-row">
              <svg class="qt-ring" viewBox="0 0 56 56" role="img" aria-label="${decisionPct}% approved">
                <circle class="qt-ring-track" cx="28" cy="28" r="23" pathLength="100"></circle>
                <circle class="qt-ring-fill" cx="28" cy="28" r="23" pathLength="100" stroke-dasharray="${decisionPct} 100" transform="rotate(-90 28 28)"></circle>
                <text x="28" y="31.5" text-anchor="middle">${decisionPct}%</text>
              </svg>
              <span><b>${decided}</b><small>${approved} approved</small></span>
            </div>
          </section>
          <section class="qt-card">
            <span class="qt-card-h">${ic("crew")}Chats</span>
            <div class="qt-pair">
              <div><b>${list.filter((t) => t.busy).length}</b><small>active</small></div>
              <div><b>${new Set(list.map((t) => t.project || projectPath() || "here")).size}</b><small>places</small></div>
            </div>
          </section>
          <section class="qt-card">
            <span class="qt-card-h">${ic("crew")}Your team</span>
            <div class="qt-mini-team">
              ${bots().slice(0, 4).map((a) => {
                const st = botStatus(a);
                return `<button data-q-agent-edit="${esc(a.id)}">${botFace(botColor(a), 22, a.name)}<span><b>${esc(a.name)}</b><small>${esc(st.label)}</small></span></button>`;
              }).join("") || `<div class="qt-fine">No bots yet.</div>`}
            </div>
          </section>
        </aside>
      </div>
    </div>`;
  }

  // ------------------------------------------------------------------ machines
  function machineUpdates(m, hs) {
    return hs.filter((h) => {
      const v = m.local ? h.version : (h.hosts && h.hosts[m.name]);
      return v && h.latest && v !== h.latest;
    }).length;
  }
  function machineCard(m, hs = []) {
    const tone = m.os === "macos" ? "var(--q-green)" : m.os === "windows" ? "var(--q-blue)" : m.os === "linux" ? "var(--q-lav)" : "var(--q-peach)";
    const local = m.local ? `<span class="qt-tag">this machine</span>` : "";
    const tst = m.tailscale ? `<span class="qt-tag">${ic("wifi")}tailscale</span>` : "";
    const updates = machineUpdates(m, hs);
    return `<article class="qt-machine${m.state === "offline" ? " off" : ""}" style="--c:${tone}">
      <span class="qt-m-ic">${ic(iconForMachine(m))}</span>
      <span class="qt-m-name">${esc(m.name)}<i class="qdot ${m.state === "online" ? "ok" : "idle"}"></i></span>
      <span class="qt-m-sub">${esc(m.os)} · ${esc(m.state)}${m.running ? ` · ${m.running} running` : ""}${m.pending ? ` · ${m.pending} waiting` : ""}</span>
      <span class="qt-m-detail">${esc(m.detail || "")}</span>
      ${m.paths.slice(0, 2).map((p) => `<span class="qt-m-path">${esc(p)}</span>`).join("")}
      <span class="qt-m-tags">${local}${tst}${updates ? `<span class="qt-tag warn">${updates} update${updates === 1 ? "" : "s"}</span>` : ""}${m.version ? `<span class="qt-tag">v${esc(m.version)}</span>` : ""}</span>
    </article>`;
  }
  function machinesPage() {
    const list = machineList();
    const hs = harnesses();
    const remote = list.filter((m) => !m.local).slice(0, 3);
    const host = (m) => m.name || (m.local ? "This Mac" : "Remote");
    const ver = (h, m) => m.local ? h.version : (h.hosts && h.hosts[m.name]) || "";
    const rest = list.length % 3;
    const addSpan = rest ? 3 - rest : 0;
    return `<div class="q-page q-wide">
      ${head("Machines", "Every host, what runs on it, and which harness version it carries. Machines are first-class, not a settings row.",
        `<button class="q-btn" data-q-settings-section="machines">${ic("plus")}Pair</button>
         <button class="q-btn dark" data-q-settings-section="machines">Add machine</button>`, "Fleet")}
      <div class="qt-machines">${list.map((m) => machineCard(m, hs)).join("") || `<div class="qt-empty">No machines paired. This one is running everything.</div>`}
        ${addSpan ? `<button class="qt-machine qt-machine-add" style="grid-column:span ${addSpan}" data-q-settings-section="machines"><span class="qt-m-ic">${ic("plus")}</span><b>Pair another machine</b><small>SSH, cloud or this Mac</small></button>` : ""}
      </div>
      <section class="qt-harness">
        <div class="qt-hh">${ic("terminal")}<b>Harnesses</b><span>${hs.length} installed · versions update per host</span></div>
        <table class="qt-htable">
          <thead><tr><th>Harness</th><th>${esc(host(list[0] || { name: "This Mac", local: true }))}</th>${remote.map((m) => `<th>${esc(host(m))}</th>`).join("")}</tr></thead>
          <tbody>
            ${hs.map((h) => `<tr>
              <td><span class="qt-hname"><span class="qs-prov" style="--c:${PROVIDER_BG[h.id] || "#64748b"}">${(window.Logos && Logos.mark(h.id)) || esc((h.name || "?")[0])}</span><span>${esc(h.name)}<small class="qt-fine" style="display:block">latest ${esc(h.latest || h.version || "—")}</small></span></span></td>
              ${[list[0] || { name: "This Mac", local: true }, ...remote].map((m) => {
                const v = ver(h, m);
                const old = v && h.latest && v !== h.latest;
                return `<td><span class="qt-hver${old ? " old" : ""}">${v ? `<i class="qdot"></i>${esc(v)}` : "—"}</span>${old ? `<button class="qt-update" data-q-harness-update="${esc(h.id)}|${esc(m.name)}">Update</button>` : ""}</td>`;
              }).join("")}
            </tr>`).join("") || `<tr><td colspan="${2 + remote.length}" class="qt-fine">No harnesses scanned yet.</td></tr>`}
          </tbody>
        </table>
      </section>
    </div>`;
  }

  // ------------------------------------------------------------------ team
  function doctorCard() {
    const failed = chats().filter((t) => t.failed && !healedIds.has(t.id)).length;
    const auto = doctorAuto();
    const last = doctor.at ? ago(doctor.at) : "not yet";
    return `<section class="qt-doctor">
      <span class="qt-doctor-ic">${ic("heart")}</span>
      <span><b>Doctor</b><small>${failed ? `${failed} chat${failed === 1 ? "" : "s"} need repair` : "All chats healthy"} · ${doctor.healed ? `${doctor.healed} repaired` : "auto-repair " + (auto ? "on" : "off")} · checked ${esc(last)}</small></span>
      <span class="sp"></span>
      <button class="qt-switch${auto ? " on" : ""}" data-q-doctor-auto role="switch" aria-checked="${auto}" aria-label="Auto-repair chats" title="Auto-repair chats"></button>
      <button class="q-btn sm" data-q-doctor>${ic("refresh")}Run check</button>
    </section>`;
  }
  function teamCard(a) {
    const x = extraOf(a.id);
    const st = botStatus(a);
    const p = providerOf(a);
    const effort = { low: "Low effort", medium: "Medium effort", high: "High effort", xhigh: "X-High effort", max: "Max effort" }[x.effort] || "High effort";
    const fbs = (a.fallbacks && a.fallbacks.length) ? a.fallbacks : (x.fallbacks || []);
    return `<button class="qt-botcard" data-q-agent-edit="${esc(a.id)}">
      <span class="qt-botcard-h">${botFace(botColor(a), 38, a.name)}
        <span><b>${esc(a.name)}</b><small>${esc(p === "codex" ? "OpenAI" : p === "claude" ? "Anthropic" : p)} · ${esc(modelLabel(a.route, p))}</small></span>
      </span>
      <span class="qt-bot-role">${esc(a.job || "No job written yet.")}</span>
      <span class="qt-bot-meta">${esc(effort)}<span>·</span>${fbs.length ? `${fbs.length} fallback${fbs.length === 1 ? "" : "s"}` : "No fallback"}</span>
      <span class="qt-bot-foot"><i class="qdot ${st.tone === "work" ? "work" : st.tone === "need" ? "need" : "idle"}"></i><span>${esc(st.label)}</span><em class="qt-kind">${kindOf(a, x) === "doctor" ? "Doctor" : "Agent"}</em></span>
    </button>`;
  }
  function agentDrawer() {
    const a = bots().find((b) => b.id === agentEdit);
    if (!a) return "";
    const x = extraOf(a.id);
    const route = a.route || {};
    const prov = route.provider || providerOf(a);
    const comp = (a.computer && a.computer.kind) || "folder";
    const kind = kindOf(a, x);
    const fallbacks = fallbackDraft;
    const models = [...new Set(Object.values(MODELS).flat().map(([v]) => v))];
    return `<div class="qt-scrim" data-q-agent-close></div>
      <aside class="qt-drawer" role="dialog" aria-modal="true" aria-label="${esc(a.name)} settings">
        <header class="qt-drawer-head">${botFace(botColor(a), 36, a.name)}
          <span><b>${esc(a.name)}</b><small>${esc(kind === "doctor" ? "Doctor" : "Agent")} · ${esc(modelLabel(a.route, prov))}</small></span>
          <button class="qicon" data-q-agent-close aria-label="Close">${ic("x")}</button>
        </header>
        <div class="qt-drawer-body">
          <label class="qt-field">Name<input id="qaName" value="${esc(a.name)}" maxlength="40"></label>
          <label class="qt-field">Role<input id="qaJob" value="${esc(a.job || "")}" placeholder="What it is for, in one line"></label>
          <label class="qt-field">Instructions<textarea id="qaInstr" placeholder="How it works, what it checks, what to say when nothing happened.">${esc(a.personality || "")}</textarea></label>
          <div class="qt-fieldrow">
            <label class="qt-field">Runs on<select id="qaComputer">
              ${machineList().map((m) => `<option value="${m.local ? "folder" : m.kind}"${(m.local ? comp === "folder" : comp === m.kind) ? " selected" : ""}>${esc(m.name)}</option>`).join("") || `<option value="folder">This Mac</option>`}
            </select></label>
            <label class="qt-field">Provider<select id="qaProv">
              ${PROVIDERS.map(([id, name]) => `<option value="${id}"${id === prov ? " selected" : ""}>${name}</option>`).join("")}
            </select></label>
          </div>
          <label class="qt-field">Model<input id="qaModel" value="${esc(route.model || "")}" list="qaModels" placeholder="Its own default"><datalist id="qaModels">${models.map((m) => `<option value="${esc(m)}"></option>`).join("")}</datalist></label>
          <div class="qt-fb">
            <div class="qt-fb-h">Fallbacks<small>Tried in order when the model above fails</small></div>
            ${fallbacks.map((f, i) => `<div class="qt-fb-row">
              <select data-fb-prov="${i}" aria-label="Fallback provider">${PROVIDERS.map(([id, name]) => `<option value="${id}"${id === f.provider ? " selected" : ""}>${name}</option>`).join("")}</select>
              <input data-fb-model="${i}" value="${esc(f.model || "")}" placeholder="model" aria-label="Fallback model">
              <button class="qt-fb-del" data-q-fb-del="${i}" aria-label="Remove fallback">${ic("x")}</button>
            </div>`).join("")}
            <button class="qt-fb-add" data-q-fb-add>${ic("plus")}Add fallback</button>
          </div>
          <div>
            <span class="qt-seg-h">Intelligence</span>
            <div class="qt-effort">${[["low", "Low"], ["medium", "Medium"], ["high", "High"], ["xhigh", "X-High"], ["max", "Max"]].map(([k, l]) => `<label><input type="radio" name="qaEffort" value="${k}"${(x.effort || "high") === k ? " checked" : ""}><span>${l}</span></label>`).join("")}</div>
          </div>
          <div class="qt-toggles">
            <label class="qt-toggle">Fast mode<small>Skips the long think</small><input type="checkbox" id="qaFast"${x.fast ? " checked" : ""}></label>
            <label class="qt-toggle">Quick rotation<small>⌘B cycles these</small><input type="checkbox" id="qaRotate"${x.rotation ? " checked" : ""}></label>
          </div>
          <div>
            <span class="qt-seg-h">Kind</span>
            <div class="qt-kindrow">
              <label><input type="radio" name="qaKind" value="agent"${kind !== "doctor" ? " checked" : ""}><span>Agent<small>Takes the work</small></span></label>
              <label><input type="radio" name="qaKind" value="doctor"${kind === "doctor" ? " checked" : ""}><span>Doctor<small>Fixes the chats</small></span></label>
            </div>
          </div>
        </div>
        <footer class="qt-drawer-foot"><button class="q-btn" data-q-agent-close>Cancel</button><button class="q-btn dark" data-q-agent-save="${esc(a.id)}">Save bot</button></footer>
      </aside>`;
  }
  function teamPage() {
    const list = bots();
    const rest = list.length % 3;
    const addSpan = rest ? 3 - rest : 0;
    return `<div class="q-page q-wide">
      ${head("Meet your team", "Named bots with their own instructions, model, fallbacks and a face — not a model string with a chat box.",
        `<button class="q-btn" data-q-go="crew">Open Crew ${ic("chev")}</button>`, "Team")}
      ${doctorCard()}
      <div class="qt-teamgrid">${list.map(teamCard).join("") || `<div class="qt-empty">No bots yet. Create one in Crew and it will appear here with its full settings.</div>`}
        ${addSpan ? `<button class="qt-botcard qt-bot-add" style="grid-column:span ${addSpan}" data-q-go="crew"><span class="qt-add-ic">${ic("plus")}</span><b>New bot</b><small>Give it a name, a job and a voice</small></button>` : ""}
      </div>
      ${agentEdit ? agentDrawer() : ""}
    </div>`;
  }

  function needsPage() {
    const list = pending();
    const agents = awaiting();
    return `<div class="q-page">
      ${head("Needs you", "Plans and deliverables that stop the loop until you decide.",
        `<button class="q-btn" data-q-go="review">See all review</button>`)}
      <div class="q-list">
        ${list.length ? list.map(a => `<button class="q-row" data-q-ap="${a.id}">
          <span class="q-row-ic need">${ic(a.kind === "plan" ? "hand" : "check")}</span>
          <span class="q-row-main"><b>${a.kind === "plan" ? "Approve the plan" : "Review the deliverable"}</b><small>${a.tickets.length} ticket${a.tickets.length === 1 ? "" : "s"} · agent ${a.agent} · ${esc(projectName())}</small></span>
          <span class="q-row-end">Review ${ic("chev")}</span>
        </button>`).join("") : ""}
        ${agents.map(a => `<button class="q-row" data-q-agent="${a.id}">
          <span class="q-row-ic need">${ic("zap")}</span>
          <span class="q-row-main"><b>${esc(a.id === 0 ? "Main agent" : a.title)}</b><small>Waiting for your review · ${esc(projectName())}</small></span>
          <span class="q-row-end">Open ${ic("chev")}</span>
        </button>`).join("")}
        ${!list.length && !agents.length ? `<div class="q-empty">Nothing waiting on you.</div>` : ""}
      </div>
    </div>`;
  }

  function boardPage() {
    const cols = [
      ["Queued", byState("queued", "assigned")],
      ["In progress", byState("in_progress")],
      ["In review", byState("ready_for_human")],
      ["Done", byState("done", "approved", "wontfix")],
    ];
    return `<div class="q-page q-wide">
      ${head("Board", "Every ticket in this project, by state.",
        `<button class="q-btn dark" data-q-new>New session ${ic("plus")}</button>`)}
      <div class="q-board">
        ${cols.map(([name, list]) => `<section class="q-col">
          <div class="q-col-h"><b>${name}</b><span>${list.length}</span></div>
          ${list.map(t => `<button class="q-ticket" data-q-ticket="${esc(t.key)}">
            <span class="q-ticket-k">${esc(t.key)}</span>
            <b>${esc(t.title)}</b>
            <small>${esc(t.check ? "check: " + t.check : t.state.replace(/_/g, " "))}</small>
          </button>`).join("")}
          ${list.length ? "" : `<div class="q-col-empty">Empty</div>`}
        </section>`).join("")}
      </div>
    </div>`;
  }

  function reviewPage() {
    const list = (snap.approvals || []).slice().reverse();
    return `<div class="q-page">
      ${head("Review", "Plans and deliverables, newest first.", "")}
      <div class="q-list">
        ${list.length ? list.map(a => `<button class="q-row" data-q-ap="${a.id}">
          <span class="q-row-ic ${a.state.state === "pending" ? "need" : a.state.state === "approved" ? "ok" : "idle"}">${ic(a.state.state === "approved" ? "check" : a.kind === "plan" ? "hand" : "work")}</span>
          <span class="q-row-main"><b>${a.kind === "plan" ? "Plan" : "Deliverable"} · ${a.tickets.length} ticket${a.tickets.length === 1 ? "" : "s"}</b><small>${esc(a.state.state)} · agent ${a.agent}</small></span>
          <span class="q-row-end">Open ${ic("chev")}</span>
        </button>`).join("") : `<div class="q-empty">No reviews yet.</div>`}
      </div>
    </div>`;
  }

  function automatePage() {
    const rs = routines();
    return `<div class="q-page">
      ${head("Automate", "Routines that wake a bot on a clock, and what they will do.",
        `<button class="q-btn" data-q-go="crew">Open Crew</button>`)}
      <div class="q-list">
        ${rs.length ? rs.map(r => `<div class="q-row">
          <span class="q-row-ic">${ic("clock")}</span>
          <span class="q-row-main"><b>${esc(r.name)}</b><small>${esc(r.agent.name)} · ${esc(r.kind)}${r.time ? " at " + esc(r.time) : ""} · next ${esc(wake(r.next_run))}</small></span>
          <button class="q-btn sm" data-q-run="${esc(r.agent.id)}|${esc(r.id)}">Run now</button>
        </div>`).join("") : `<div class="q-empty">No automations yet. Add a routine to a bot in Crew.</div>`}
      </div>
    </div>`;
  }

  function tasksPage() {
    const list = tickets();
    return `<div class="q-page">
      ${head("Tasks", "Everything the team has planned for this project.",
        `<button class="q-btn" data-q-go="board">Board ${ic("chev")}</button>`)}
      <div class="q-list">
        ${list.length ? list.map(t => `<button class="q-row" data-q-ticket="${esc(t.key)}">
          <span class="q-row-ic ${t.state === "ready_for_human" ? "ok" : t.state === "failed" ? "need" : ""}">${ic(t.state === "done" ? "check" : "work")}</span>
          <span class="q-row-main"><b>${esc(t.key)} · ${esc(t.title)}</b><small>${esc(t.state.replace(/_/g, " "))}${t.assignee != null ? " · agent " + t.assignee : ""}</small></span>
          <span class="q-row-end">${esc(t.check || "")}</span>
        </button>`).join("") : `<div class="q-empty">No tasks yet. Describe a goal in Code and the team will plan it.</div>`}
      </div>
    </div>`;
  }

  function inboxPage() {
    const list = chats().slice().sort((a, b) => b.updated - a.updated);
    return `<div class="q-page">
      ${head("Inbox", "Every thread with something new, and every bot waiting on you.", "")}
      <div class="q-list">
        ${list.length ? list.map((t, i) => `<button class="q-row" data-q-thread="${esc(t.id)}">
          ${botFace(["#8b5cf6", "#14b8a6", "#ef4444", "#f59e0b"][i % 4], 28, t.title || "Chat", WALL_SHAPES[i % WALL_SHAPES.length])}
          <span class="q-row-main"><b>${esc(t.title || "New chat")}</b><small>${esc(t.busy ? "working" : t.unread ? "needs you" : "idle")} · ${esc(t.members && t.members.length ? "group chat" : t.agent ? "agent thread" : "session")}</small></span>
          <span class="q-row-end">${esc(ago(t.updated))}</span>
        </button>`).join("") : `<div class="q-empty">Inbox zero.</div>`}
      </div>
    </div>`;
  }

  function calendarPage() {
    const rs = routines();
    return `<div class="q-page">
      ${head("Calendar", "What wakes next.", "")}
      <div class="q-list">
        ${rs.length ? rs.map(r => `<div class="q-row">
          <span class="q-row-ic">${ic("calendar")}</span>
          <span class="q-row-main"><b>${esc(r.name)}</b><small>${esc(r.agent.name)} · ${esc(wake(r.next_run))}</small></span>
          <button class="q-btn sm" data-q-run="${esc(r.agent.id)}|${esc(r.id)}">Run now</button>
        </div>`).join("") : `<div class="q-empty">Nothing on the calendar.</div>`}
      </div>
    </div>`;
  }

  function canvasPage() {
    const tabs = prefs.tabs || [];
    return `<div class="q-page">
      ${head("Canvas", "One canvas can hold an agent, a terminal, a browser, files or the review.",
        `<button class="q-btn dark" data-q-add-canvas>${ic("plus")}New canvas</button>`)}
      <div class="q-canvases">
        ${tabs.map((t, i) => `<button class="q-canvas${i === prefs.active_tab ? " on" : ""}" data-q-tab="${i}">
          <span class="q-canvas-h"><b>${esc(t.name || "Canvas " + (i + 1))}</b><span>${t.panes.length}</span></span>
          <span class="q-canvas-body">${t.panes.map(p => `<span class="q-canvas-p">${ic(p.kind === "agent" ? "crew" : p.kind === "terminal" ? "code" : p.kind === "browser" ? "monitor" : p.kind === "files" ? "folder" : "canvas")}${esc(p.kind)}</span>`).join("")}</span>
          <span class="q-canvas-f">${esc(t.panes.map(p => p.kind).join(" · "))}</span>
        </button>`).join("") || `<div class="q-empty">No canvases yet. Start one from Code.</div>`}
      </div>
    </div>`;
  }

  function limitsPage() {
    const list = workAgents();
    const total = Math.max(0.0001, snap.total_cost_usd || 0);
    return `<div class="q-page">
      ${head("Limits", "What this session has spent, by agent.", "")}
      <div class="q-cards">
        <div class="q-card static"><span class="q-card-h">Session spend</span><b class="q-stat">${money(total)}</b><span class="q-card-b">${compact(list.reduce((s, a) => s + (a.input_tokens || 0), 0))} in · ${compact(list.reduce((s, a) => s + (a.output_tokens || 0), 0))} out</span></div>
        <div class="q-card static"><span class="q-card-h">Router</span><b class="q-stat">${money(snap.router_cost_usd)}</b><span class="q-card-b">Decision cost across ${list.length} agent${list.length === 1 ? "" : "s"}</span></div>
        <div class="q-card static"><span class="q-card-h">Agents</span><b class="q-stat">${list.length}</b><span class="q-card-b">${running().length} running · ${awaiting().length} waiting</span></div>
      </div>
      <div class="q-panel">
        <div class="q-sec-head"><b>By agent</b><span class="q-count">${money(total)}</span></div>
        ${list.map(a => {
          const pct = Math.max(2, Math.round(((a.cost_usd || 0) / total) * 100));
          return `<div class="q-meter">
            <span class="q-meter-h"><b>${esc(a.id === 0 ? "Main agent" : a.title)}</b><span>${money(a.cost_usd)} · ${compact((a.input_tokens || 0) + (a.output_tokens || 0))} tok</span></span>
            <span class="q-meter-bar"><i style="width:${pct}%"></i></span>
            <span class="q-meter-f">${esc(a.decision ? a.decision.model + " @ " + a.decision.effort : a.status)}</span>
          </div>`;
        }).join("") || `<div class="q-empty">No agents yet.</div>`}
      </div>
    </div>`;
  }

  function settingsPage() {
    const sections = [
      ["general", "General", "What Backspace is for, and the open project", "gear"],
      ["appearance", "Appearance", "Theme, colours, fonts", "sun"],
      ["chat", "Chat", "Models and rich answers", "crew"],
      ["machines", "Machines", "Local, cloud and SSH", "laptop"],
      ["projects", "Projects", "Folders Workbench can open", "folder"],
      ["skills", "Skills", "Playbooks agents load", "skills"],
      ["mcp", "MCP and connections", "Apps, tools and outside servers", "plugins"],
      ["updates", "Updates", "Version and checks", "refresh"],
      ["home", "Archived threads", "Threads you put away", "canvas"],
    ];
    return `<div class="q-page">
      ${head("Settings", "The knobs behind the quiet.", "")}
      <div class="q-settings">
        ${sections.map(([id, t, s, icon]) => `<button class="q-set" data-q-settings-section="${id}"><span class="q-set-ic">${ic(icon)}</span><span><b>${t}</b><small>${s}</small></span>${ic("chev")}</button>`).join("")}
        <button class="q-set" data-q-go="apps"><span class="q-set-ic">${ic("apps")}</span><span><b>Apps</b><small>Views installed on this machine</small></span>${ic("chev")}</button>
        <button class="q-set" data-q-go="memory"><span class="q-set-ic">${ic("canvas")}</span><span><b>Memory</b><small>What agents remember</small></span>${ic("chev")}</button>
      </div>
    </div>`;
  }

  // The Code page's right column: the same agents, their board checklist, and
  // the one action that keeps the crew together. It is not a second dashboard;
  // it is the team wall the session is already running.
  function renderWall() {
    const el = $("#qWall");
    if (!el) return;
    const ags = workAgents();
    const on = page === "code" && S.mode === "code" && S.codeView === "agents" && !!snap.workspace && ags.length > 0;
    el.hidden = !on;
    const w = $("#win");
    if (w) w.classList.toggle("q-wall-on", on);
    if (!on) { el.innerHTML = ""; return; }
    const plan = (snap.approvals || []).some(a => a.kind === "plan" && a.state && a.state.state === "approved");
    const assigned = ags.some(a => a.ticket) || (snap.tickets || []).some(t => t.assignee != null);
    const shared = (snap.board || []).length > 0 || ags.length > 1;
    const reviewed = (snap.tickets || []).some(t => t.state === "ready_for_human") || (snap.approvals || []).some(a => a.kind === "deliverable" && a.state && a.state.state === "approved");
    const done = (snap.tickets || []).length > 0 && (snap.tickets || []).every(t => ["done", "wontfix"].includes(t.state));
    const items = [
      [plan || assigned, "The lead splits the job on a shared board"],
      [assigned, "Each agent takes a task"],
      [shared, "One agent tells another what changed"],
      [reviewed, "The lead runs the checks, then another agent reviews"],
      [done, "Every task done and checked"],
    ];
    const doneCount = items.filter(([v]) => v).length;
    const names = ags.map(a => (a.id === 0 ? "Claude Code" : a.title));
    const lead = names[0] || "The lead";
    const rest = names.slice(1);
    const colors = ["#8b5cf6", "#14b8a6", "#ef4444", "#f59e0b", "#3b82f6"];
    const tone = (a) => a.status === "running" ? "work" : a.status === "awaiting_approval" || a.status === "failed" ? "need" : "idle";
    el.innerHTML = `
      <div class="qw-head"><b>Your agents, as a team</b><button class="qicon" data-q-search aria-label="Search">${ic("search")}</button></div>
      <p class="qw-sub">${esc(lead)} leads${rest.length ? ", with " + esc(rest.slice(0, 2).join(" and ")) + (rest.length > 2 ? " and the rest" : "") : ""}. Each moment is pointed out as it happens.</p>
      <div class="qw-progress"><span>${doneCount} / 5</span><span class="qw-bar"><i style="width:${doneCount * 20}%"></i></span></div>
      <div class="qw-checks">
        ${items.map(([v, label]) => `<div class="qw-check${v ? " done" : ""}"><span class="qw-circle">${v ? ic("check") : ""}</span>${esc(label)}</div>`).join("")}
      </div>
      <div class="qw-agents">
        ${ags.map((a, i) => `<button class="qw-agent" data-q-agent="${a.id}">
          ${botFace(a.avatar && a.avatar.color || colors[i % colors.length], 26, a.id === 0 ? "Claude Code" : a.title, WALL_SHAPES[i % WALL_SHAPES.length])}
          <span class="qw-main"><b>${esc(a.id === 0 ? "Claude Code" : a.title)}</b><small>${esc(a.id === 0 ? "Lead · planning" : a.ticket ? "Worker · " + a.ticket : "Worker")}</small></span>
          <i class="qdot ${tone(a)}"></i>
        </button>`).join("")}
      </div>
      <button class="qw-keep" data-q-keep>${ic("crew")}Keep a crew</button>`;
  }

  // ------------------------------------------------------------------ Code session
  const sessionOn = () => page === "code" && codeSurface === "session" && S.codeView !== "pair";
  const sessionLead = () => workAgents()[0];
  const agentName = (a) => (a.id === 0 ? "Claude Code" : a.title || "Worker");
  const agentRole = (a) => (a.id === 0 ? "Lead" : "Worker");
  const agentColor = (a, i) => (a.avatar && a.avatar.color) || ["#8b5cf6", "#14b8a6", "#ef4444", "#f59e0b"][i % 4];
  const sessionBranch = () => {
    const a = sessionLead();
    return (a && a.branch) || (data.git && data.git.branch) || "main";
  };
  function sessionTitle() {
    const t = prefs.tabs && prefs.tabs[prefs.active_tab];
    const name = t && t.name;
    if (name && !/^(canvas|harness|untitled)$/i.test(name)) return name;
    return "See your agents work as a team";
  }
  function setCodeSurface(which) {
    codeSurface = which === "panes" ? "panes" : "session";
    try { localStorage.setItem(CKEY, codeSurface); } catch {}
    if (codeSurface === "session" && S.codeView === "pair" && typeof setCodeView === "function") setCodeView("agents");
    render();
  }
  function setSessionFilter(v) {
    sessionFilter = v === "all" ? "all" : v;
    renderSession();
    renderSessionFooter();
  }
  function sessionEntry(e, i) {
    const a = e.agent;
    const name = agentName(a);
    const face = providerLogo(a, 26);
    if (e.kind === "user") return `<div class="qs-msg me"><div class="qs-bubble me">${esc(e.text)}</div></div>`;
    if (e.kind === "tool_call") return `<div class="qs-tool">${ic("code")}<span>${esc(e.text)}</span></div>`;
    if (e.kind === "tool_result") return `<pre class="qs-pre">${esc(String(e.text).slice(0, 4000))}</pre>`;
    if (e.kind === "system" || e.kind === "error") return `<div class="qs-sys${e.kind === "error" ? " err" : ""}">${esc(e.text)}</div>`;
    return `<div class="qs-msg">${face}<div class="qs-mbody"><div class="qs-mname">${esc(name)}<span>${esc(agentRole(a))}</span></div><div class="qs-bubble">${esc(e.text)}</div></div></div>`;
  }
  function sessionMessages(ags) {
    const agsById = new Map(ags.map((a, i) => [a.id, { a, i }]));
    let entries = [];
    if (sessionFilter !== "all") {
      const hit = agsById.get(Number(sessionFilter)) || agsById.get(sessionFilter);
      if (hit) entries = (hit.a.log || []).slice(-40).map((e) => ({ ...e, agent: hit.a, index: hit.i }));
    } else {
      ags.forEach((a, i) => (a.log || []).slice(-16).forEach((e) => entries.push({ ...e, agent: a, index: i })));
    }
    if (!entries.length) return `<div class="qs-empty">No transcript yet. The lead is waiting for a goal.</div>`;
    let out = "";
    for (const e of entries) out += sessionEntry(e, e.index);
    return out;
  }
  function renderSession() {
    const el = $("#qSession");
    if (!el) return;
    if (!sessionOn()) { el.hidden = true; return; }
    el.hidden = false;
    const ags = workAgents();
    if (!ags.length) {
      el.innerHTML = `<div class="qs-shell"><div class="qs-empty-state"><h1>No session yet</h1><p>Open a project and describe a goal; the lead will plan it into tickets and the team will start.</p><button class="q-btn dark" data-q-new>New session</button></div></div>`;
      return;
    }
    const lead = ags[0];
    const leadName = agentName(lead);
    const scrollBefore = (($("#qsScroll") || {}).scrollTop) || 0;
    const rest = ags.slice(1).map(agentName);
    const running = ags.filter((a) => a.status === "running").length;
    const steps = ags.reduce((n, a) => n + ((a.log && a.log.length) || 0), 0);
    const tokens = ags.reduce((n, a) => n + (a.input_tokens || 0) + (a.output_tokens || 0), 0);
    const branch = sessionBranch();
    const project = base(snap.workspace) || "Workbench";
    const git = data.git || {};
    const files = Array.isArray(git.files) ? git.files : [];
    const added = files.reduce((n, f) => n + (f.added || 0), 0);
    const removed = files.reduce((n, f) => n + (f.removed || 0), 0);
    const filterChip = (id, label, on) => `<button class="qs-filter-chip${on ? " on" : ""}" data-q-filter="${id}">${id === "all" ? "" : providerLogo(ags.find((a) => String(a.id) === String(id)) || lead, 16)}<span>${esc(label)}</span></button>`;
    el.innerHTML = `<div class="qs-shell">
      <div class="qs-scroll" id="qsScroll">
        <header class="qs-head">
          <div class="qs-eyebrow">${esc(today())}</div>
          <h1>${esc(sessionTitle())}</h1>
          <p>${esc(leadName)} leads${rest.length ? `, with ${esc(rest.slice(0, 2).join(" and "))}${rest.length > 2 ? " and the rest" : ""}` : ""}. ${running ? `${running} agent${running === 1 ? " is" : "s are"} working in this session.` : "The team is between steps."}</p>
          <div class="qs-chips">
            <span class="qs-chip"><i class="qdot ${running ? "work" : "idle"}"></i>${running ? "Working" : "Idle"}</span>
            <span class="qs-chip">${steps} step${steps === 1 ? "" : "s"}</span>
            <span class="qs-chip">${esc(branch)}</span>
            <span class="qs-chip">${ags.length} agent${ags.length === 1 ? "" : "s"}</span>
          </div>
        </header>
        <div class="qs-team">
          ${ags.map((a, i) => `<div class="qs-teammate">
            ${providerLogo(a, 32)}
            <div><b>${esc(agentName(a))} <em>${esc(agentRole(a))}</em></b><small>${a.id === 0 ? "Was made the lead — it hands the other agents their part of the work and puts it together." : `connected to ${esc(project)} on ${esc(branch)} · auto mode`}</small></div>
          </div>`).join("")}
        </div>
        <div class="qs-thread">${sessionMessages(ags)}</div>
      </div>
      <div class="qs-bottom">
        <div class="qs-filter">
          <span class="qs-filter-label">Filter</span>
          ${filterChip("all", "All agents", sessionFilter === "all")}
          ${ags.map((a) => filterChip(String(a.id), agentName(a), String(sessionFilter) === String(a.id))).join("")}
          <button class="qs-filter-add" data-q-go="crew" aria-label="Add an agent">${ic("plus")}</button>
        </div>
        <div class="qs-composer">
          <div class="qs-context">
            <span><i class="qdot ${pending().length ? "need" : "idle"}"></i>${pending().length + awaiting().length} need${pending().length + awaiting().length === 1 ? "" : "s"}</span>
            <span>${ic("chart")}${compact(tokens)} tok</span>
            <span>${ic("board")}Board ${tickets().length}</span>
            <span>${ic("crew")}Workers ${running}</span>
            <span>${ic("check")}Tasks ${byState("done", "approved").length}/${tickets().length || 0}</span>
          </div>
          <textarea id="qsInput" placeholder="Redirect ${esc(leadName)}. It picks it up at its next step…" aria-label="Message ${esc(leadName)}"></textarea>
          <div class="qs-composer-row">
            <button class="qicon" data-q-toast="Attach a file" aria-label="Attach">${ic("plus")}</button>
            <span class="qs-agent-chip">${providerLogo(lead, 18)}${esc(leadName)}</span>
            <span class="qs-model">${esc((lead.decision && lead.decision.model) || "default")}</span>
            <span class="qs-model">Auto</span>
            <button class="qs-send" data-q-session-send aria-label="Send">${ic("up")}</button>
          </div>
        </div>
        <div class="qs-usage"><span>${money(snap.total_cost_usd)}</span><span>·</span><span>${compact(tokens)} tok</span><span>·</span><span>Router ${money(snap.router_cost_usd)}</span></div>
        </div>
      </div>
    </div>`;
    const inp = $("#qsInput");
    if (inp) inp.addEventListener("keydown", (e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); sessionSend(); } });
    const sc = $("#qsScroll");
    if (sc) sc.scrollTop = scrollBefore;
  }
  function renderSessionFooter() {
    const el = $("#qFooter");
    if (!el) return;
    const w = $("#win");
    const on = sessionOn() && workAgents().length > 0;
    el.hidden = !on;
    if (w) w.classList.toggle("q-footer-on", on);
    if (!on) { el.innerHTML = ""; return; }
    const ags = workAgents();
    const total = ags.reduce((n, a) => n + (a.input_tokens || 0) + (a.output_tokens || 0), 0);
    const tokens = total;
    const project = base(snap.workspace) || "Workbench";
    const branch = sessionBranch();
    const git = data.git || {};
    const files = Array.isArray(git.files) ? git.files : [];
    const added = files.reduce((n, f) => n + (f.added || 0), 0);
    const removed = files.reduce((n, f) => n + (f.removed || 0), 0);
    el.innerHTML = `<div class="qfooter-strip">
      ${ags.map((a) => {
        const tok = (a.input_tokens || 0) + (a.output_tokens || 0);
        const pct = tokens ? Math.round((tok / tokens) * 100) : 0;
        const id = providerOf(a);
        return `<button class="qfooter-agent${String(sessionFilter) === String(a.id) ? " on" : ""}" data-q-filter="${a.id}">
          <span class="qfooter-ring" style="--c:${PROVIDER_BG[id] || "#211e1e"};--p:${pct}%">${providerLogo(a, 26)}</span>
          <span class="qfooter-name"><b>${esc(agentName(a))}</b><small>${pct}% tok · ${money(a.cost_usd)}${a.id === 0 ? " · lead" : ""}</small></span>
        </button>`;
      }).join("")}
      <button class="qfooter-add" data-q-go="crew" aria-label="Add an agent">${ic("plus")}</button>
    </div>
    <div class="qfooter-status">
      <span>${ic("folder")}${esc(project)} · ${esc(branch)}</span>
      <span>${ic("file")}${files.length} file${files.length === 1 ? "" : "s"}${added || removed ? ` · +${added} −${removed}` : ""}</span>
      <span class="sp"></span>
      <button data-q-focus-session>Jump ⌘J</button>
      <button data-q-settings>Environment</button>
    </div>`;
  }
  function sessionSend() {
    const inp = $("#qsInput");
    if (!inp) return;
    const text = inp.value.trim();
    if (!text) return;
    const lead = sessionLead();
    inp.value = "";
    if (lead) (lead.log = lead.log || []).push({ kind: "user", text });
    renderSession();
    const sc = $("#qsScroll");
    if (sc) sc.scrollTop = sc.scrollHeight;
    invoke("send", { text }).then(() => {
      if (window.toast) toast("Sent to " + (lead ? agentName(lead) : "the lead"), "ok");
    }).catch((e) => {
      if (window.toast) toast(String(e), "err");
      load(true);
    });
  }

  function renderView() {    const el = $("#qView"); if (!el) return;
    const snapSig = (snap.agents || []).map(a => `${a.status}:${a.cost_usd || 0}:${a.input_tokens || 0}:${a.output_tokens || 0}`).join(",") + ":" + (snap.total_cost_usd || 0) + ":" + (snap.router_cost_usd || 0);
    const tixSig = (snap.tickets || []).map(t => `${t.key}:${t.state}`).join(",") + ":" + (snap.approvals || []).map(a => `${a.id}:${a.state && a.state.state}`).join(",");
    const key = page + ":" + data.version + ":" + tixSig + ":" + snapSig + ":" + JSON.stringify(prefs.projects || []) + ":" + todayDate + ":" + todayGroup + ":" + (agentEdit || "") + ":" + fallbackDraft.length + ":" + doctor.at;
    if (key === viewKey) return;
    viewKey = key;
    if (page === "home") el.innerHTML = homePage();
    else if (page === "today") el.innerHTML = todayPage();
    else if (page === "machines") el.innerHTML = machinesPage();
    else if (page === "team") el.innerHTML = teamPage();
    else if (page === "work") el.innerHTML = workPage();
    else if (page === "needs") el.innerHTML = needsPage();
    else if (page === "board") el.innerHTML = boardPage();
    else if (page === "review") el.innerHTML = reviewPage();
    else if (page === "automate") el.innerHTML = automatePage();
    else if (page === "tasks") el.innerHTML = tasksPage();
    else if (page === "inbox") el.innerHTML = inboxPage();
    else if (page === "calendar") el.innerHTML = calendarPage();
    else if (page === "canvas") el.innerHTML = canvasPage();
    else if (page === "limits") el.innerHTML = limitsPage();
    else if (page === "settings") el.innerHTML = settingsPage();
    el.scrollTop = 0;
  }

  // ------------------------------------------------------------------ visibility
  function applyVisibility() {
    const w = $("#win"); if (!w) return;
    const legacy = LEGACY.has(page);
    const session = sessionOn();
    const chatSide = page === "crew" || (page === "code" && !session);
    w.classList.add("q-on");
    w.classList.toggle("q-plain", !legacy);
    w.classList.toggle("q-legacy", legacy);
    w.classList.toggle("q-code", page === "code");
    w.classList.toggle("q-crew", page === "crew");
    w.classList.toggle("q-plugins", page === "plugins");
    w.classList.toggle("q-skills", page === "skills");
    w.classList.toggle("q-apps", page === "apps");
    w.classList.toggle("q-memory", page === "memory");
    w.classList.toggle("q-session-on", session);
    w.classList.toggle("q-quiet-side", !chatSide);
    w.classList.toggle("q-chat-side", chatSide);
    const qv = $("#qView"), body = $("#body"), qs = $("#qSession"), qf = $("#qFooter");
    if (qv) qv.hidden = legacy;
    if (qs) qs.hidden = !session;
    if (qf) qf.hidden = !session;
    if (!session && w) w.classList.remove("q-footer-on");
    if (body) body.hidden = !legacy || session;
    // The chat face's own list is the Crew/Code sidebar. Force it visible; the
    // old chrome's pair-only rules would otherwise hide it.
    const cl = $("#chatList");
    if (cl) cl.hidden = false;
    const sh = $(".side-head");
    if (sh) sh.hidden = false;
  }

  function render() {
    if (!ready) return;
    if (page === "settings" && S.settings) {
      // The settings sheet owns the window; keep the rail label in step.
      applyVisibility();
      return;
    }
    renderRail();
    renderTop();
    renderNav();
    renderWork();
    const key = page + ":" + data.version + ":" + needsCount() + ":" + (prefs.projects || []).join("|") + ":" + projectPath() + ":" + (prefs.tabs || []).length + ":" + skinName() + ":" + codeSurface + ":" + todayDate;
    if (key !== sideKey) { sideKey = key; renderSide(); }
    if (!LEGACY.has(page)) renderView();
    if (sessionOn()) renderSession(); else { const qs = $("#qSession"); if (qs) qs.hidden = true; }
    renderSessionFooter();
    renderWall();
    applyVisibility();
    applySkin();
    renderDock();
    renderTabs();
    maybeDoctor();
    if (!data.at || Date.now() - data.at > 5000) load();
  }

  // ------------------------------------------------------------------ skin
  function skinName() {
    if ($("#win") && $("#win").classList.contains("q-glass")) return "glass";
    const t = document.documentElement.dataset.theme;
    if (t === "dark") return "dark";
    if (t === "light") return "light";
    try {
      const v = localStorage.getItem(SKEY);
      if (v === "glass") return "glass";
    } catch {}
    return "light";
  }

  function applySkin() {
    const w = $("#win"); if (!w) return;
    let glass = false;
    try { glass = localStorage.getItem(SKEY) === "glass"; } catch {}
    w.classList.toggle("q-glass", glass);
    if (glass) {
      document.documentElement.dataset.theme = "dark";
      w.dataset.backdrop = "aurora";
    } else if (w.dataset.backdrop === "aurora") {
      delete w.dataset.backdrop;
    }
  }

  function setSkin(next) {
    if (next === "glass") {
      const on = skinName() !== "glass";
      try { localStorage.setItem(SKEY, on ? "glass" : "light"); } catch {}
      prefs.theme = "dark";
      if (on) {
        if (window.applyTheme) applyTheme();
        invoke("set_theme", { theme: "dark" });
      } else {
        prefs.theme = "light";
        if (window.applyTheme) applyTheme();
        invoke("set_theme", { theme: "light" });
      }
    } else {
      try { localStorage.setItem(SKEY, next); } catch {}
      prefs.theme = next;
      if (window.applyTheme) applyTheme();
      invoke("set_theme", { theme: next });
    }
    render();
  }

  // ------------------------------------------------------------------ actions
  function openApproval(id) {
    const list = (snap && snap.approvals) || [];
    const at = list.findIndex((a) => a.id === id);
    S.review = at >= 0 ? at : id;
    closeSettingsIfOpen();
    S.mode = "code"; S.codeView = "agents";
    setMode("code");
    if (window.Shell) Shell.layout();
    page = "code"; try { localStorage.setItem(QKEY, page); } catch {}
    setDrawer(true);
    render();
  }

  function openTicket(key) {
    S.ticket = key;
    closeSettingsIfOpen();
    S.mode = "code"; S.codeView = "agents";
    setMode("code");
    if (window.Shell) Shell.layout();
    page = "code"; try { localStorage.setItem(QKEY, page); } catch {}
    setDrawer(true);
    render();
  }

  function openThread(id) {
    if (window.Chat && Chat.setScope) Chat.setScope(null, true);
    S.mode = "crew"; setMode("crew");
    if (window.Shell) Shell.layout();
    page = "crew"; try { localStorage.setItem(QKEY, page); } catch {}
    if (window.WhirlChat && WhirlChat.open) WhirlChat.open(id);
    else if (window.Chat && Chat.open) Chat.open(id);
    render();
  }

  async function runRoutine(spec) {
    const [agent, routine] = String(spec || "").split("|");
    if (!agent || !routine) return;
    try {
      await invoke("routine_run", { agent, routine });
      if (window.toast) toast("Routine started", "ok");
      await load(true);
    } catch (e) {
      if (window.toast) toast(String(e), "err");
    }
  }

  function addCanvas() {
    closeSettingsIfOpen();
    codeSurface = "panes"; try { localStorage.setItem(CKEY, codeSurface); } catch {}
    S.mode = "code"; S.codeView = "agents"; setMode("code");
    page = "code"; try { localStorage.setItem(QKEY, page); } catch {}
    if (window.Shell) Shell.layout();
    if (typeof window.addCanvas === "function") window.addCanvas({ kind: "empty", agent: 0, url: null });
    render();
  }

  // ------------------------------------------------------------------ helper
  function buildHelp() {
    const win = $("#win");
    if (!win || $("#qHelpLayer")) return;
    const layer = document.createElement("div");
    layer.id = "qHelpLayer";
    layer.className = "qhelp-layer";
    layer.hidden = true;
    layer.innerHTML = `
      <div class="qhelp-scrim" data-q-help-close></div>
      <aside class="qhelp-panel" role="dialog" aria-modal="true" aria-label="Helper Agent">
        <div class="qhelp-head">
          ${botFace("#8b5cf6", 30, "Helper", "brain")}
          <div><b>Helper Agent</b><small>A guide to Workbench</small></div>
          <button class="qicon" data-q-help-close aria-label="Close Helper Agent">${ic("x")}</button>
        </div>
        <p class="qhelp-intro">Ask how anything works. It explains the app and never changes your project.</p>
        <div class="qhelp-sugs">
          <button data-q-help-q="What does Needs you count?">What does Needs you count?</button>
          <button data-q-help-q="What is a canvas?">What is a canvas?</button>
          <button data-q-help-q="What is the team wall?">What is the team wall?</button>
          <button data-q-help-q="Where do approvals open?">Where do approvals open?</button>
        </div>
        <div class="qhelp-thread" id="qHelpThread" role="log" aria-live="polite"></div>
        <div class="qhelp-input">
          <input id="qHelpInput" placeholder="Ask a question" aria-label="Ask the Helper Agent">
          <button data-q-help-send aria-label="Send">${ic("up")}</button>
        </div>
      </aside>`;
    win.appendChild(layer);
    const input = $("#qHelpInput");
    if (input) input.addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); helpSend(); } if (e.key === "Escape") closeHelp(); });
  }

  function openHelp() {
    const layer = $("#qHelpLayer");
    if (!layer) return;
    layer.hidden = false;
    setTimeout(() => $("#qHelpInput")?.focus(), 30);
  }
  function closeHelp() {
    const layer = $("#qHelpLayer");
    if (layer) layer.hidden = true;
  }
  function helpAnswer(q) {
    const t = q.toLowerCase();
    if (t.includes("needs")) return "Needs you counts pending plan or deliverable approvals, agents waiting for a review, and unread agent replies. Opening one takes you to the real drawer.";
    if (t.includes("canvas")) return "A canvas is one pane in Code: an agent session, a terminal, a browser, files, or the review. A tab can hold up to four canvases; this page lists the tabs you already have.";
    if (t.includes("wall") || t.includes("team")) return "The team wall is the right column on Code. It names the lead, lists the five steps from the shared board, and shows the agents in this session. It hides when the side panel opens or the window is narrow.";
    if (t.includes("approval") || t.includes("review")) return "Plans and deliverables land in Needs and Review. Opening one goes to Code and opens the existing drawer, where you can approve or reject the actual work.";
    if (t.includes("crew") || t.includes("group") || t.includes("bot")) return "Crew is the agents face: each bot has a page and one-to-one threads, and group chats bring several bots into one room. Type @name in a room to aim a reply at one bot.";
    if (t.includes("limit") || t.includes("cost") || t.includes("token")) return "Limits shows the snapshot's session spend, router cost, tokens and the per-agent split. It is the same number the status line used to carry.";
    if (t.includes("setting") || t.includes("theme")) return "Settings is the hub on the sidebar. Appearance opens the existing sheet on its Theme section; the sun, moon and drop buttons in the sidebar switch Light, Dark and Glass directly.";
    return "I only explain Workbench. Try Needs you, canvases, the team wall, approvals, Crew or Limits.";
  }
  function helpAsk(q) {
    q = String(q || "").trim();
    if (!q) return;
    const thread = $("#qHelpThread");
    if (!thread) return;
    thread.insertAdjacentHTML("beforeend", `<div class="qhelp-msg me">${esc(q)}</div>`);
    thread.scrollTop = thread.scrollHeight;
    setTimeout(() => {
      thread.insertAdjacentHTML("beforeend", `<div class="qhelp-msg"><b>Helper</b>${esc(helpAnswer(q))}</div>`);
      thread.scrollTop = thread.scrollHeight;
    }, 220);
  }
  function helpSend() {
    const input = $("#qHelpInput");
    if (!input) return;
    const q = input.value;
    input.value = "";
    helpAsk(q);
  }

  function wire() {
    const readFallbacks = () => $$q(".qt-fb-row").map((row) => ({
      provider: (row.querySelector("[data-fb-prov]") || {}).value || "openai",
      model: ((row.querySelector("[data-fb-model]") || {}).value || "").trim(),
    })).filter((f) => f.model);
    const openAgent = (id) => {
      agentEdit = id;
      const bot = bots().find((b) => b.id === id);
      const ex = extraOf(id);
      fallbackDraft = [...(((bot && bot.fallbacks && bot.fallbacks.length) ? bot.fallbacks : ex.fallbacks) || [])];
      data.version++;
      render();
    };
    const saveAgent = (id) => {
      const a = bots().find((b) => b.id === id);
      if (!a) return;
      const val = (s) => { const el = $(s); return el ? String(el.value || "").trim() : ""; };
      const prov = val("#qaProv") || (a.route && a.route.provider) || "claude";
      const route = {
        ...(a.route || { kind: "cli", provider: prov, model: null }),
        provider: prov,
        model: val("#qaModel") || null,
        kind: prov === "ollama" ? "local" : ((a.route && a.route.kind === "router") ? "router" : "cli"),
      };
      const comp = val("#qaComputer") || "folder";
      const fallbacks = readFallbacks();
      const effort = (document.querySelector('input[name="qaEffort"]:checked') || {}).value || "high";
      const fast = !!($("#qaFast") && $("#qaFast").checked);
      const rotation = !!($("#qaRotate") && $("#qaRotate").checked);
      const kind = (document.querySelector('input[name="qaKind"]:checked') || {}).value || "agent";
      const fbRoutes = fallbacks.map((f) => ({ kind: f.provider === "ollama" ? "local" : "cli", provider: f.provider, model: f.model || null }));
      const next = { ...a, name: val("#qaName") || a.name, job: val("#qaJob"), personality: val("#qaInstr"), route, fallbacks: fbRoutes, computer: { ...(a.computer || {}), kind: comp } };
      saveExtra(a.id, { fallbacks, effort, fast, rotation, kind });
      invoke("agent_save", { agent: next }).then((saved) => {
        const i = data.agents.findIndex((x) => x.id === a.id);
        if (i >= 0) data.agents[i] = saved || next; else data.agents.push(saved || next);
        data.version++;
        agentEdit = null;
        fallbackDraft = [];
        if (window.toast) toast("Saved " + next.name, "ok");
        render();
      }).catch((err) => { if (window.toast) toast(String(err), "err"); });
    };
    document.addEventListener("click", (e) => {
      const t = e.target.closest("[data-q-go],[data-q-project],[data-q-thread],[data-q-agent],[data-q-agent-edit],[data-q-agent-close],[data-q-agent-save],[data-q-fb-add],[data-q-fb-del],[data-q-doctor],[data-q-doctor-auto],[data-q-harness-update],[data-q-day],[data-q-tab],[data-q-ticket],[data-q-ap],[data-q-run],[data-q-theme],[data-q-skin],[data-q-settings-section],[data-q-settings],[data-q-search],[data-q-add-project],[data-q-add-canvas],[data-q-onboard],[data-q-new],[data-q-more],[data-q-update],[data-q-panel],[data-q-keep],[data-q-help],[data-q-help-close],[data-q-help-q],[data-q-help-send],[data-q-sview],[data-q-filter],[data-q-session-send],[data-q-focus-session],[data-q-new-chat],[data-q-toast]");
      if (!t) return;
      const d = t.dataset;
      if (d.qGo) { e.preventDefault(); go(d.qGo); return; }
      if (d.qDay) { e.preventDefault(); const v = d.qDay; todayDate = /^\d{4}-\d\d-\d\d$/.test(v) ? v : shiftDay(todayDate, Number(v) || 0); data.version++; render(); return; }
      if (d.qAgentEdit) { e.preventDefault(); openAgent(d.qAgentEdit); return; }
      if (d.qAgentClose !== undefined) { e.preventDefault(); agentEdit = null; fallbackDraft = []; data.version++; render(); return; }
      if (d.qAgentSave) { e.preventDefault(); saveAgent(d.qAgentSave); return; }
      if (d.qFbAdd !== undefined) { e.preventDefault(); const now = readFallbacks(); fallbackDraft = now.length ? now : fallbackDraft; fallbackDraft.push({ provider: "openai", model: "gpt-5.1" }); data.version++; render(); return; }
      if (d.qFbDel !== undefined) { e.preventDefault(); const now = readFallbacks(); fallbackDraft = now.length ? now : fallbackDraft; fallbackDraft.splice(+d.qFbDel, 1); data.version++; render(); return; }
      if (d.qDoctor !== undefined) { e.preventDefault(); doctorRun(false); return; }
      if (d.qDoctorAuto !== undefined) { e.preventDefault(); setDoctorAuto(!doctorAuto()); data.version++; render(); return; }
      if (d.qHarnessUpdate) { e.preventDefault(); const [name, host] = String(d.qHarnessUpdate).split("|"); if (window.toast) toast(`Queued update: ${name} on ${host}`, "ok"); return; }
      if (d.qProject) { e.preventDefault(); const p = d.qProject; const back = page; const before = projectPath(); const opened = openProject(p); Promise.resolve(opened).then(() => { if (projectPath() === p || p === before) { page = "today"; todayDate = dayKey(); } else { page = back; } data.version++; try { localStorage.setItem(QKEY, page); } catch {} render(); }); return; }
      if (d.qThread) { e.preventDefault(); openThread(d.qThread); return; }
      if (d.qAgent) { e.preventDefault(); closeSettingsIfOpen(); codeSurface = "panes"; try { localStorage.setItem(CKEY, codeSurface); } catch {}; S.mode = "code"; S.codeView = "agents"; setMode("code"); page = "code"; try { localStorage.setItem(QKEY, page); } catch {}; if (window.openIn) openIn({ kind: "agent", agent: +d.qAgent }); if (window.Shell) Shell.layout(); render(); return; }
      if (d.qTab) { e.preventDefault(); closeSettingsIfOpen(); codeSurface = "panes"; try { localStorage.setItem(CKEY, codeSurface); } catch {}; if (typeof switchTab === "function") switchTab(+d.qTab); S.mode = "code"; S.codeView = "agents"; setMode("code"); page = "code"; try { localStorage.setItem(QKEY, page); } catch {}; if (window.Shell) Shell.layout(); render(); return; }
      if (d.qTicket) { e.preventDefault(); openTicket(d.qTicket); return; }
      if (d.qAp) { e.preventDefault(); openApproval(+d.qAp); return; }
      if (d.qRun) { e.preventDefault(); runRoutine(d.qRun); return; }
      if (d.qTheme) { e.preventDefault(); setSkin(d.qTheme); return; }
      if (d.qSkin) { e.preventDefault(); setSkin("glass"); return; }
      if (d.qSettingsSection) { e.preventDefault(); S.setSection = d.qSettingsSection; openSettings(d.qSettingsSection); return; }
      if (d.qSettings !== undefined) { e.preventDefault(); openSettings(); return; }
      if (d.qSearch !== undefined) { e.preventDefault(); if (window.Shell) Shell.search(); return; }
      if (d.qAddProject !== undefined) { e.preventDefault(); closeSettingsIfOpen(); if (typeof pickProject === "function") pickProject(); return; }
      if (d.qAddCanvas !== undefined) { e.preventDefault(); addCanvas(); return; }
      if (d.qOnboard !== undefined) { e.preventDefault(); if (window.Onboard) Onboard.open(); return; }
      if (d.qNew !== undefined) { e.preventDefault(); newSession(); return; }
      if (d.qMore !== undefined) { e.preventDefault(); if (window.Shell) Shell.search(); return; }
      if (d.qUpdate !== undefined) { e.preventDefault(); if (typeof checkUpdate === "function") checkUpdate(); return; }
      if (d.qPanel !== undefined) { e.preventDefault(); if (window.RPanel) RPanel.toggle(); return; }
      if (d.qKeep !== undefined) { e.preventDefault(); if (window.toast) toast("Crew kept · " + workAgents().length + " agents", "ok"); return; }
      if (d.qHelp !== undefined) { e.preventDefault(); openHelp(); return; }
      if (d.qHelpClose !== undefined) { e.preventDefault(); closeHelp(); return; }
      if (d.qHelpQ) { e.preventDefault(); helpAsk(d.qHelpQ); return; }
      if (d.qHelpSend !== undefined) { e.preventDefault(); helpSend(); return; }
      if (d.qSview) { e.preventDefault(); setCodeSurface(d.qSview); return; }
      if (d.qFilter !== undefined) { e.preventDefault(); setSessionFilter(d.qFilter); return; }
      if (d.qSessionSend !== undefined) { e.preventDefault(); sessionSend(); return; }
      if (d.qFocusSession !== undefined) { e.preventDefault(); $("#qsInput")?.focus(); return; }
      if (d.qNewChat !== undefined) { e.preventDefault(); newSession(); return; }
      if (d.qToast) { e.preventDefault(); if (window.toast) toast(d.qToast); return; }
    }, true);

    document.addEventListener("change", (e) => {
      const pick = e.target.closest("#qDayPick,#qDayGroup");
      if (!pick) return;
      if (pick.id === "qDayPick" && pick.value) todayDate = pick.value;
      if (pick.id === "qDayGroup") todayGroup = pick.value;
      data.version++;
      render();
    });

    addEventListener("resize", () => { if (ready) render(); });
    addEventListener("keydown", (e) => {
      if (e.key !== "Escape") return;
      if ($(".qt-drawer")) { agentEdit = null; fallbackDraft = []; data.version++; render(); return; }
      if ($("#qHelpLayer") && !$("#qHelpLayer").hidden) closeHelp();
    });
    setInterval(() => { if (ready && !document.hidden) load(); }, 30000);
  }

  function init() {
    if (ready) return;
    try { page = localStorage.getItem(QKEY) || "home"; } catch { page = "home"; }
    try { codeSurface = localStorage.getItem(CKEY) === "panes" ? "panes" : "session"; } catch { codeSurface = "session"; }
    todayDate = dayKey();
    if (!PAGES[page]) page = "home";
    // Direction 04 opens on warm paper. "System" is only a preference the old
    // shell had; the first run of the quiet shell resolves it to light once,
    // and the sidebar's sun/moon/drop still let anyone choose dark or glass.
    let seen = null;
    try { seen = localStorage.getItem("bs.quiet_seen"); } catch {}
    if (!seen) {
      try { localStorage.setItem("bs.quiet_seen", "1"); } catch {}
      prefs.theme = "light";
      if (window.applyTheme) applyTheme();
      invoke("set_theme", { theme: "light" });
    }
    ready = true;
    wire();
    buildHelp();
    render();
    setTimeout(() => { try { if (typeof narrow !== "function" || !narrow()) setSide(true); } catch {} }, 0);
    load(true);
  }

  return { init, render, go, current, load, newSession, skin: skinName, setSkin, fromMode };
})();
