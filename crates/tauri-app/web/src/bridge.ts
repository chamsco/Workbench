// The seam between Whirl's components and Backspace. The vanilla shell
// (ui/chat.js) hands over a `Host` with the Tauri invoke and the route
// helpers it already owns; this module keeps the chat data in a tiny
// external store React subscribes to, and maps Backspace threads onto the
// message shape Whirl's thread view reads.

import { useSyncExternalStore } from "react";

import type { ChatMessage, MessageStatus } from "@/lib/messages";

export type RouteKind = "cli" | "local" | "router" | "cloud" | "a2a";
export type Route = { kind: RouteKind; provider: string; model: string | null };

export type Ad = { id: string; advertiser: string; title: string; body: string; cta: string; url: string };

export type Msg = {
  id: string;
  role: "user" | "assistant";
  text: string;
  at: number;
  status: "sending" | "streaming" | "done" | "error" | "stopped";
  error?: string | null;
  reply_to?: string | null;
  reactions: string[];
  attachments: { id: string; name: string; mime: string; size: number }[];
  model?: string | null;
  ad?: Ad | null;
  cost_usd?: number | null;
  edited?: boolean;
  via?: Route | null;
  author?: string | null;
  author_name?: string | null;
  trace?: string | null;
  interrupted?: boolean;
};

export type Span = {
  span_id: string;
  parent?: string | null;
  name: string;
  kind: "reply" | "tool";
  start: number;
  end?: number | null;
  attrs: Record<string, unknown>;
  error?: string | null;
  input: string;
  output: string;
};
export type Trace = { trace_id: string; thread: string; message: string; spans: Span[] };

export type Computer = {
  kind: "folder" | "docker" | "ssh";
  image: string;
  desktop: boolean;
  host: string;
  user: string;
  port: number;
  key: string;
  desktop_url: string;
};

/** A bot's job on a clock (see docs/AGENTS-PROJECTS-ROUTINES-2026-10-17.md).
 *  Owned by the bot: it needs the bot's model, rules, memory and folder. */
export type Routine = {
  id: string;
  name: string;
  /** What to do when it fires. */
  instructions: string;
  kind: "daily" | "weekdays" | "weekly" | "every";
  /** "HH:MM" local, for daily/weekdays/weekly. */
  time: string;
  /** ISO weekdays 1..7 (Mon..Sun), for weekly. */
  days: number[];
  /** Minutes between runs, for every. */
  every_minutes: number;
  /** A project id, or "" for the bot's own folder. */
  project: string;
  enabled: boolean;
  last_run: number;
  /** Absolute ms, computed by the core. */
  next_run: number;
  runs: number;
  /** Minutes east of UTC (JS offset negated), so Rust can do local time. */
  utc_offset_minutes: number;
  created: number;
  updated: number;
};

/** A Chat project: a folder plus the bots assigned to it. Not a Workbench
 *  project (no tickets, branches or approvals). */
export type Project = {
  id: string;
  name: string;
  folder: string;
  /** "" is this Mac. Stored and shown; only this Mac is settable today. */
  machine: string;
  agents: string[];
  created: number;
  updated: number;
};

export type Agent = {
  id: string;
  name: string;
  job: string;
  avatar: { emoji: string; color: string };
  /** The style text appended to its system prompt (a personality chip). */
  personality: string;
  route: Route;
  shared: string[];
  computer: Computer;
  memory: boolean;
  /** Rules turned off: "edit", "run", "web", "apps". */
  off: string[];
  /** Apps and connections it may not use, by id. */
  apps_off: string[];
  /** Its webhook's secret; "" while the webhook is off. */
  hook: string;
  routines: Routine[];
  created: number;
  updated: number;
};

/** An outside MCP server bots can be given (prefs.connections). */
export type Connection = { id: string; name: string; target: string };

export type ComputerStatus = { kind: string; state: string; detail: string; desktop: string | null };
export type Note = { id: string; text: string; project?: string | null };

export type Thread = {
  id: string;
  title: string;
  created: number;
  updated: number;
  pinned: boolean;
  route: Route;
  messages: Msg[];
  /** How long this thread lives. A ghost is never written to disk. */
  life?: "keep" | "ghost" | "temporal";
  expires_at?: number | null;
  ttl_minutes?: number | null;
  branched_from?: string | null;
  project?: string | null;
  app?: string | null;
  agent?: string | null;
  members?: string[];
  goal?: string | null;
};

export type ThreadInfo = {
  id: string;
  title: string;
  updated: number;
  pinned: boolean;
  route: Route;
  preview: string;
  busy: boolean;
  unread: boolean;
  project?: string | null;
  app?: string | null;
  agent?: string | null;
  members?: string[];
  /** Why its last reply failed (a usage limit, a crash), if it did. */
  failed?: string | null;
  /** A ghost chat: listed while you are in it, gone when you leave. */
  ghost?: boolean;
  /** A temporal chat: when it forgets itself, for the countdown. */
  expires_at?: number | null;
};

export type RouteItem = {
  route?: Route;
  id: string;
  label: string;
  sub?: string;
  badge?: string;
  disabled?: boolean;
  setup?: boolean;
};
export type RouteGroup = { name: string; sub?: string; items: RouteItem[] };

export type Account = { plan: string; used: number; allowance: number; period: string };

/** One machine the harness can run on: this one, or a paired remote. */
export type Machine = {
  index: number;
  name: string;
  local: boolean;
  url?: string;
  selected: boolean;
  project?: string | null;
  link: { state: string; error?: string | null };
};

/** What Colony knows: the named, disposable machines agents create and drive. */
export type ColonyStatus = {
  ready: boolean;
  visor?: string | null;
  message?: string | null;
  workspaces?: { name: string; provider: string; os: string; state: string; ip?: string | null }[] | null;
};

/** One canvas in the open tab: what the sidebar's tree hangs under the folder. */
export type Canvas = { index: number; kind: string; label: string };

/** What the vanilla shell provides. */
export type Host = {
  invoke: <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
  routes: () => RouteGroup[];
  routeName: (r: Route | null) => string;
  providerName: (r: Route | null) => string;
  defaultRoute: () => Route | null;
  usable: (r: Route | null) => boolean;
  account: () => Account | null;
  openSettings: (section?: string) => void;
  runSetup: () => void;
  toast: (text: string, kind?: string) => void;
  openUrl: (url: string) => void;
  /** The project folder Pair is showing, or null for plain chat. */
  scope: () => string | null;
  /** Save text to Memory (scoped to the project in Pair). */
  remember: (text: string, source?: string) => Promise<void>;
  /** The Agents side of Chat is showing. */
  agentsMode: () => boolean;
  /** A message became a note: show "… will remember that". */
  onCaptured: (note: Note, who: string) => void;
  /** Open an agent computer's web desktop beside the chat. */
  openComputer: (url: string) => void;
  /** Code chat: how its CLI runs (shared with the agents), and the agents' picker. */
  codeRun: () => CodeRun;
  setCodeRun: (permission: string, effort: string | null) => Promise<void>;
  picker: {
    mount: (el: HTMLElement, choice: CodeRun & { model: string }, onChange: (c: CodeRun & { model: string }) => void, onClose: () => void) => Promise<void>;
    close: () => void;
    /** A CLI's id (claude) to the picker's model id (claude-code), and back. */
    model: (cli: string) => string;
    cli: (model: string) => string;
    effortName: (cli: string, effort: string | null) => string;
  };
  machineName: () => string;
  /** Every machine work can run on, this one first. */
  machines: () => Machine[];
  /** Switch which machine the work happens on. */
  pickMachine: (index: number) => Promise<void>;
  /** The Colony workspaces on this machine; null when visor-core is missing. */
  colony: () => Promise<ColonyStatus | null>;
  /** Open another project folder — the folder a thread works in. */
  newProject: () => void;
  /** Which zone the sidebar is being drawn for ("chat", "code", …). */
  zone: () => string;
  /** The project open in the shell, whether or not the chat face is scoped. */
  project: () => string | null;
  /** What the open project is showing: the tab's canvases, in order. */
  canvases: () => Canvas[];
  /** Bring a canvas to the front — switches to the IDE and focuses it. */
  openCanvas: (index: number) => void;
  /** A thread is about to be opened: the ADE is where it is read. */
  showThread: () => void;
  /** Switch Code between its two surfaces: the ADE and the IDE. */
  setCodeView: (v: "pair" | "agents") => void;
  projectMenu: (anchor: HTMLElement) => void;
  /** Open Code's side panel on a tab: changes, history, files, sessions. */
  openPanel: (tab: string) => void;
  /** A ghost chat is open: keep the sidebar out of the way. */
  ghostSide?: (on: boolean) => void;
};
export type CodeRun = { permission: string; effort: string | null };

let host: Host;
export const setHost = (h: Host) => {
  host = h;
};
export const getHost = () => host;

// ---------------------------------------------------------------- store

type State = {
  threads: ThreadInfo[];
  id: string | null;
  thread: Thread | null;
  /** Bumped when routes or the plan change, so pickers re-read the host. */
  epoch: number;
  query: string;
  /** Pair's project folder; null shows plain chats. App threads never show here. */
  scope: string | null;
  /** The message the next send answers (iMessage's reply). */
  replyTo: string | null;
  /** Chat's Agents side: named agents and groups instead of plain chats. */
  agentsMode: boolean;
  /** The next chat starts as a ghost: never written to disk. Decided before the
   *  thread exists, because a chat's life is fixed at creation — the only way
   *  back out of a ghost is to delete it. */
  ghost: boolean;
  agents: Agent[];
  /** Chat projects: folders with bots assigned. */
  projects: Project[];
  /** The project page is open; null is the agents home. */
  project: Project | null;
  /** The new-bot sheet is open, optionally for a project. */
  creating: { project: string } | null;
  /** The routine sheet is open: a bot's routine, or "new" for that bot. */
  routineEdit: { agent: string; routine: Routine | "new" } | null;
  /** The project sheet is open: an existing project to edit, or "new". */
  projectDialog: Project | "new" | null;
  /** On the Agents side with no thread: which agent's card is open in the editor. */
  editing: Agent | "new" | null;
  grouping: boolean;
  /** The reply whose trace is open. */
  traceOpen: string | null;
};

let state: State = {
  threads: [], id: null, thread: null, epoch: 0, query: "", scope: null, replyTo: null,
  agentsMode: false, agents: [], projects: [], project: null, creating: null, routineEdit: null,
  projectDialog: null, editing: null, grouping: false, traceOpen: null, ghost: false,
};
const subs = new Set<() => void>();
const emit = () => subs.forEach((f) => f());
const set = (patch: Partial<State>) => {
  state = { ...state, ...patch };
  emit();
};

export function useChat(): State {
  return useSyncExternalStore(
    (f) => {
      subs.add(f);
      return () => subs.delete(f);
    },
    () => state,
  );
}

export const chatState = () => state;

/** The shell changed something the face reads but does not own — the tab's
 *  canvases — so tell the store to hand it out again. Cheap on purpose: the
 *  sidebar asks for canvases only when it re-renders. */
export const nudge = () => set({ epoch: state.epoch + 1 });

/** Pull the list and the open thread again (on every core change). */
export async function refresh() {
  const [threads, agents, projects] = await Promise.all([
    host.invoke<ThreadInfo[]>("chat_list").catch(() => state.threads),
    host.invoke<Agent[]>("agents_list").catch(() => state.agents),
    host.invoke<Project[]>("agent_projects").catch(() => state.projects),
  ]);
  let thread = state.thread;
  if (state.id) {
    thread = await host.invoke<Thread | null>("chat_thread", { id: state.id }).catch(() => null);
  }
  // A project page follows the store: a rename or an assignment is visible
  // without reopening it, and a removed project closes itself.
  const project = state.project ? projects.find((p) => p.id === state.project!.id) ?? null : null;
  set({ threads, agents, projects, project, thread, id: thread ? state.id : null, epoch: state.epoch + 1 });
}

/** A ghost is a one-off: the moment it stops being the thread on screen it is
 *  destroyed, whichever way you left it. Whirl purges its ephemeral thread on
 *  exit (`purgeIncognito`); this is that, at the one place every exit path goes
 *  through. Leaving is never made to wait for the delete — the move is instant,
 *  the delete is bookkeeping. */
function forget(ghost: Thread | null | undefined) {
  if (ghost?.life !== "ghost") return;
  void host
    .invoke("chat_delete", { id: ghost.id })
    .then(() => refresh())
    .catch(() => {});
}

export async function openThread(id: string) {
  const leaving = state.thread;
  const thread = await host.invoke<Thread | null>("chat_thread", { id }).catch(() => null);
  set({ id: thread ? id : null, thread, replyTo: null });
  const threads = await host.invoke<ThreadInfo[]>("chat_list").catch(() => state.threads);
  set({ threads });
  if (leaving?.id !== id) forget(leaving);
}

export function goHome() {
  const leaving = state.thread;
  set({ id: null, thread: null, replyTo: null });
  forget(leaving);
}

type Scoped = { project?: string | null; app?: string | null; agent?: string | null; members?: string[] };
const isAgentThread = (t: Scoped) => !!t.agent || (t.members?.length ?? 0) > 0;

/** Threads that belong to what is showing: Pair's project (threads without an
 *  agent), or Chat's agents and groups (whatever project they also carry).
 *  Apps' threads never show here. */
export const inScope = (t: Scoped, scope: string | null, agentsMode = state.agentsMode) =>
  !t.app && (agentsMode ? isAgentThread(t) : (t.project ?? null) === scope && !isAgentThread(t));

export function setScope(scope: string | null, agentsMode = false) {
  if (scope === state.scope && agentsMode === state.agentsMode) return;
  const keep = state.thread && inScope(state.thread, scope, agentsMode);
  if (!keep) forget(state.thread);
  set({
    scope,
    agentsMode,
    editing: null,
    grouping: false,
    project: null,
    creating: null,
    routineEdit: null,
    projectDialog: null,
    ...(keep ? {} : { id: null, thread: null, replyTo: null }),
  });
}

export const agentById = (id?: string | null) => state.agents.find((a) => a.id === id);
export const editAgent = (editing: Agent | "new" | null) => set({ editing });
export const openTrace = (traceOpen: string | null) => set({ traceOpen });
export const editGroup = (grouping: boolean) => set({ grouping });

// ---------------------------------------------------------------- bots, projects, routines

export const projectById = (id?: string | null) => state.projects.find((p) => p.id === id);

/** Open the new-bot sheet, optionally for a project. */
export const startBot = (project?: string) => set({ creating: { project: project ?? "" }, editing: null, routineEdit: null, projectDialog: null });
export const cancelBot = () => set({ creating: null });
/** Open a project page (null goes back to the agents home). */
export const openProject = (project: Project | null) => {
  if (!project) {
    set({ project: null });
    return;
  }
  set({ project, id: null, thread: null, replyTo: null, creating: null, editing: null, projectDialog: null });
};
/** Open the routine sheet for a bot: an existing routine, or "new". */
export const editRoutine = (agent: string, routine?: Routine) => set({ routineEdit: { agent, routine: routine ?? "new" }, editing: null, creating: null });
export const cancelRoutine = () => set({ routineEdit: null });
/** Open the project sheet: an existing project to edit, or "new". */
export const editProject = (project: Project | "new" | null) => set({ projectDialog: project, creating: null });
export const cancelProject = () => set({ projectDialog: null });

export async function saveAgentProject(p: Project): Promise<Project> {
  const saved = await host.invoke<Project>("agent_project_save", { project: p });
  await refresh();
  return saved;
}
export async function deleteAgentProject(id: string) {
  await host.invoke("agent_project_delete", { id });
  if (state.project?.id === id) set({ project: null });
  await refresh();
}
export async function assignProject(project: string, agent: string, on: boolean) {
  await host.invoke<Project>("agent_project_assign", { project, agent, on });
  await refresh();
}
export async function saveRoutine(agent: string, routine: Routine) {
  await host.invoke<Agent>("routine_save", { agent, routine });
  await refresh();
}
export async function removeRoutine(agent: string, routine: string) {
  await host.invoke<Agent>("routine_delete", { agent, routine });
  await refresh();
}
export async function runRoutine(agent: string, routine: string) {
  await host.invoke("routine_run", { agent, routine });
  await refresh();
}
/** Recompute next wakes for the machine's current UTC offset (once on mount,
 *  and whenever the clock's zone changed under us). */
export async function syncRoutines() {
  try {
    const agents = await host.invoke<Agent[]>("routines_sync", { offset: -new Date().getTimezoneOffset() });
    if (Array.isArray(agents)) set({ agents });
  } catch {
    /* an older core without routines: leave the face alone */
  }
}

export async function saveAgent(a: Agent): Promise<Agent> {
  const saved = await host.invoke<Agent>("agent_save", { agent: a });
  await refresh();
  return saved;
}
export async function deleteAgent(id: string) {
  await host.invoke("agent_delete", { id });
  if (state.thread?.agent === id) set({ id: null, thread: null });
  await refresh();
}
export const computer = (id: string, action: "status" | "start" | "stop") =>
  host.invoke<ComputerStatus>("agent_computer", { id, action });

/** An agent's conversation: the latest thread with it, or a new one. With a
 *  project, the thread carries the project folder, so the bot works there. */
export async function openAgentThread(a: Agent, project?: Project | null) {
  const folder = project?.folder ?? null;
  const t = state.threads
    .filter((t) => t.agent === a.id && (t.project ?? null) === folder)
    .sort((x, y) => y.updated - x.updated)[0];
  if (t) return openThread(t.id);
  const scope = folder ? { agent: a.id, project: folder } : { agent: a.id };
  const nt = await host.invoke<Thread>("chat_new", { route: a.route, scope });
  set({ id: nt.id, thread: nt });
  await refresh();
}

/** One more turn in an existing thread (the ask card's answer, a retry, a
 *  routine proposal accepted): the thread already exists, so no creation. */
export async function sendTo(thread: Thread, text: string) {
  await host.invoke("chat_send", { id: thread.id, text, files: [] });
  await refresh();
}

/** Kuvo's "Start its job": the bot begins its own job in its thread. */
export async function startJob(thread: Thread) {
  if (!host.usable(thread.route)) {
    throw new Error(`${host.providerName(thread.route)} isn't available. Pick another model.`);
  }
  await sendTo(
    thread,
    "Start your job now. If you need details before you can start, ask me with an ask block offering the likely choices; otherwise tell me anything you need from me, and keep it short.",
  );
}

/** The bot's conversation with no project — the sidebar row's action. */
export async function openAgent(a: Agent) {
  return openAgentThread(a, null);
}

export async function createGroup(title: string, goal: string, members: string[]) {
  const first = state.agents.find((a) => a.id === members[0]);
  if (!first) throw new Error("Pick at least one agent");
  const t = await host.invoke<Thread>("chat_new", { route: first.route, scope: { members, goal, title } });
  set({ id: t.id, thread: t, grouping: false });
  await refresh();
}

export const setReplyTo = (replyTo: string | null) => set({ replyTo });

/** Choose how the next chat lives, before it exists. Turning the mode on also
 *  scrubs any ghost an earlier one left in the core, like Whirl's
 *  `purgeIncognito` on enter. */
export const setGhost = (ghost: boolean) => {
  set({ ghost });
  if (ghost) void host.invoke("chat_purge_ghosts").catch(() => {});
};

export function setQuery(query: string) {
  set({ query });
}

export type NewFile = { name: string; mime: string; data: string };

/** Send from the composer; creates the thread on the first message. */
export async function send(text: string, files: NewFile[], route: Route | null, plan = false) {
  let id = state.id;
  if (!id) {
    if (!route) throw new Error("Connect a model first");
    // The life is fixed here, at creation, and never changes afterwards.
    const ghost = state.ghost;
    const t = await host.invoke<Thread>("chat_new", {
      route,
      scope: { project: state.scope, life: ghost ? "ghost" : "keep" },
    });
    id = t.id;
    set({ id, thread: t, ghost: false });
  }
  const replyTo = state.replyTo;
  set({ replyTo: null });
  const note = await host.invoke<Note | null>("chat_send", { id, text, files, replyTo, plan });
  if (note) {
    const t = state.thread;
    const who = t?.agent ? agentById(t.agent)?.name : (t?.members?.length ?? 0) > 0 ? "Your team" : "Backspace";
    host.onCaptured(note, who || "Backspace");
  }
  await refresh();
}

export const stop = (id: string) => host.invoke("chat_stop", { id });
export const retry = (id: string) => host.invoke("chat_retry", { id });
export const resume = (id: string) => host.invoke("chat_resume", { id });
export const edit = (id: string, msg: string, text: string) => host.invoke("chat_edit", { id, msg, text });
export const setRoute = (id: string, route: Route) => host.invoke("chat_set_route", { id, route });
export const react = (id: string, msg: string, emoji: string) => host.invoke("chat_react", { id, msg, emoji }).then(() => refresh());

export type LinkMeta = { url: string; site: string; title: string; description: string; image: string | null };
const previews = new Map<string, LinkMeta | null>();
/** Link preview metadata, fetched once per URL by the core. */
export function linkPreview(url: string): LinkMeta | null | undefined {
  if (previews.has(url)) return previews.get(url);
  previews.set(url, undefined as unknown as null);
  host
    .invoke<LinkMeta | null>("link_preview", { url })
    .then((m) => previews.set(url, m && (m.title || m.description) ? m : null))
    .catch(() => previews.set(url, null))
    .finally(() => set({ epoch: state.epoch + 1 }));
  return undefined;
}
export const rename = (id: string, title: string) => host.invoke("chat_rename", { id, title });
export const pin = (id: string, pinned: boolean) => host.invoke("chat_pin", { id, pinned });

/** How long a chat lives. `ghost` is never written to disk, so closing it is
 *  the same as destroying it; `temporal` is saved but forgets itself after
 *  `ttlMinutes` without a message. `keep` is an ordinary chat and undoes both. */
export const setLife = (id: string, life: "keep" | "ghost" | "temporal", ttlMinutes: number | null = null) =>
  host.invoke("chat_life", { id, life, ttlMinutes }).then(() => refresh());
export async function remove(id: string) {
  await host.invoke("chat_delete", { id });
  if (state.id === id) set({ id: null, thread: null });
  await refresh();
}
export async function branch(id: string, msg: string) {
  const t = await host.invoke<Thread>("chat_branch", { id, msg });
  await openThread(t.id);
}

// ---------------------------------------------------------------- mapping

const STATUS: Record<Msg["status"], MessageStatus> = {
  sending: "pending",
  streaming: "streaming",
  done: "complete",
  error: "error",
  stopped: "stopped",
};

/** Attachment previews, by thread/file, loaded once as data: URLs. */
const fileUrls = new Map<string, string>();
const loading = new Set<string>();

function fileUrl(thread: string, file: string, mime: string): string | undefined {
  if (!mime.startsWith("image/")) return undefined;
  const key = `${thread}/${file}`;
  const hit = fileUrls.get(key);
  if (hit) return hit;
  if (!loading.has(key)) {
    loading.add(key);
    host
      .invoke<string>("chat_file", { id: thread, file })
      .then((url) => {
        fileUrls.set(key, url);
        set({ epoch: state.epoch + 1 });
      })
      .catch(() => {});
  }
  return undefined;
}

export function toChatMessage(t: Thread, m: Msg): ChatMessage {
  return {
    id: m.id,
    role: m.role,
    content: m.text,
    createdAt: m.at,
    status: m.role === "user" ? "complete" : STATUS[m.status],
    // Retries reuse the id; the status flip is enough to restart the reveal.
    streamId: m.role === "assistant" && m.status !== "done" ? `live-${m.id}` : undefined,
    model: m.model ?? undefined,
    usageCost: m.cost_usd ?? undefined,
    traceId: m.trace ?? undefined,
    attachments: m.attachments.map((a) => ({
      id: a.id,
      name: a.name,
      size: a.size,
      type: a.mime,
      url: fileUrl(t.id, a.id, a.mime),
    })),
  };
}

// ---------------------------------------------------------------- display
// Formatting only: the schedule itself (and every next wake) comes from the
// core, so the face and the scheduler cannot disagree.

const WEEKDAYS = ["", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const hhmm = (d: Date) => `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;

/** "Every day at 08:00", "Mon, Wed at 07:30", "Every 6 hours". */
export function routineWhen(r: Routine): string {
  if (!r.enabled) return "Paused";
  const at = r.time || "08:00";
  switch (r.kind) {
    case "daily":
      return `Every day at ${at}`;
    case "weekdays":
      return `Weekdays at ${at}`;
    case "weekly": {
      const days = [...(r.days.length ? r.days : [1])].sort((a, b) => a - b).map((d) => WEEKDAYS[d]).filter(Boolean).join(", ");
      return `${days} at ${at}`;
    }
    case "every":
      return r.every_minutes % 60 === 0 && r.every_minutes >= 60
        ? `Every ${r.every_minutes / 60} hour${r.every_minutes === 60 ? "" : "s"}`
        : `Every ${r.every_minutes} min`;
    default:
      return "Not scheduled";
  }
}

/** "today at 08:00", "tomorrow at 08:00", "Tue at 08:00", "in 3 days". */
export function nextWakeLabel(ms: number): string {
  if (!ms) return "not scheduled";
  const d = new Date(ms);
  const now = new Date();
  const dayOf = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((dayOf(d) - dayOf(now)) / 86_400_000);
  if (days <= 0) return `today at ${hhmm(d)}`;
  if (days === 1) return `tomorrow at ${hhmm(d)}`;
  if (days < 7) return `${d.toLocaleDateString([], { weekday: "short" })} at ${hhmm(d)}`;
  return `in ${days} days`;
}

export const nextRoutine = (a: Agent): Routine | undefined =>
  a.routines.filter((r) => r.enabled && r.next_run > 0).sort((x, y) => x.next_run - y.next_run)[0];

/** Kuvo's status line: Working…, Needs you, Wakes tomorrow at 08:00, Watching. */
export function agentStatus(a: Agent, thread?: ThreadInfo | null): { label: string; kind: "working" | "needs" | "wake" | "idle" } {
  if (thread?.busy) return { label: "Working…", kind: "working" };
  if (thread?.unread) return { label: "Needs you", kind: "needs" };
  const r = nextRoutine(a);
  if (r) return { label: `Wakes ${nextWakeLabel(r.next_run)}`, kind: "wake" };
  return { label: "Watching", kind: "idle" };
}
