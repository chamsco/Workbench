// The seam between Whirl's components and Backspace. The vanilla shell
// (ui/chat.js) hands over a `Host` with the Tauri invoke and the route
// helpers it already owns; this module keeps the chat data in a tiny
// external store React subscribes to, and maps Backspace threads onto the
// message shape Whirl's thread view reads.

import { useSyncExternalStore } from "react";

import type { ChatMessage, MessageStatus } from "@/lib/messages";

export type RouteKind = "cli" | "local" | "router" | "cloud";
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
};

export type Thread = {
  id: string;
  title: string;
  created: number;
  updated: number;
  pinned: boolean;
  route: Route;
  messages: Msg[];
  branched_from?: string | null;
  project?: string | null;
  app?: string | null;
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
};

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
};

let state: State = { threads: [], id: null, thread: null, epoch: 0, query: "", scope: null, replyTo: null };
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

/** Pull the list and the open thread again (on every core change). */
export async function refresh() {
  const threads = await host.invoke<ThreadInfo[]>("chat_list").catch(() => state.threads);
  let thread = state.thread;
  if (state.id) {
    thread = await host.invoke<Thread | null>("chat_thread", { id: state.id }).catch(() => null);
  }
  set({ threads, thread, id: thread ? state.id : null, epoch: state.epoch + 1 });
}

export async function openThread(id: string) {
  const thread = await host.invoke<Thread | null>("chat_thread", { id }).catch(() => null);
  set({ id: thread ? id : null, thread, replyTo: null });
  const threads = await host.invoke<ThreadInfo[]>("chat_list").catch(() => state.threads);
  set({ threads });
}

export function goHome() {
  set({ id: null, thread: null, replyTo: null });
}

/** Threads that belong to the current scope. */
export const inScope = (t: { project?: string | null; app?: string | null }, scope: string | null) =>
  !t.app && (t.project ?? null) === scope;

export function setScope(scope: string | null) {
  if (scope === state.scope) return;
  const keep = state.thread && inScope(state.thread, scope);
  set({ scope, ...(keep ? {} : { id: null, thread: null, replyTo: null }) });
}

export const setReplyTo = (replyTo: string | null) => set({ replyTo });

export function setQuery(query: string) {
  set({ query });
}

export type NewFile = { name: string; mime: string; data: string };

/** Send from the composer; creates the thread on the first message. */
export async function send(text: string, files: NewFile[], route: Route | null) {
  let id = state.id;
  if (!id) {
    if (!route) throw new Error("Connect a model first");
    const t = await host.invoke<Thread>("chat_new", { route, scope: { project: state.scope } });
    id = t.id;
    set({ id, thread: t });
  }
  const replyTo = state.replyTo;
  set({ replyTo: null });
  await host.invoke("chat_send", { id, text, files, replyTo });
  await refresh();
}

export const stop = (id: string) => host.invoke("chat_stop", { id });
export const retry = (id: string) => host.invoke("chat_retry", { id });
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
    attachments: m.attachments.map((a) => ({
      id: a.id,
      name: a.name,
      size: a.size,
      type: a.mime,
      url: fileUrl(t.id, a.id, a.mime),
    })),
  };
}
