// The agent's side panel: on the right of a bot's thread and of its page.
// The bot itself sits on top, its name over its brain, and under it four tabs:
// Settings (model, instructions, personality and the switches), Context (what
// its next reply reads, as a map the way a disk space app draws a drive),
// Traces (how each reply was made), Routines (its jobs on a clock) and Desktop and Browser
// (its browser or your desktop as it last saw it, with Take control).

import { useEffect, useMemo, useRef, useState } from "react";
import { IconAlertTriangleFilled, IconBrowser, IconChevronRight, IconDeviceDesktop, IconHandStop, IconLayoutSidebarRightCollapse, IconLayoutSidebarRightExpand, IconSettings } from "@tabler/icons-react";

import { cn } from "@/lib/utils";
import { editAgent, getHost, openBot, openTrace, saveAgent, useChat, type Agent, type Thread, type Trace } from "../bridge";
import { RULES } from "./Agents";
import { PERSONALITIES } from "./bot-create";
import { Orb } from "./orb";
import { Logo, routeGlyphId, RoutePicker } from "./RoutePicker";
import { RoutineCards } from "./routines";

type Part = { group: string; label: string; tokens: number };
type ContextMap = { window: number; model: string; parts: Part[]; last_input: number | null };

/** Each kind of content: its colour and the word under a tile's name. Grey for
 *  what you wrote once, violet for memory, rose for the chat, blue for files,
 *  after the context map the owner picked. */
const KIND: Record<string, { hue: string; word: string }> = {
  Instructions: { hue: "#7c8594", word: "Instructions" },
  Memory: { hue: "#7a5cf0", word: "Memory" },
  Conversation: { hue: "#d4568c", word: "Chat" },
  Files: { hue: "#3a72e8", word: "File" },
};
const ORDER = ["Instructions", "Memory", "Conversation", "Files"];
/** Below this share of the window, the used tiles would be slivers next to
 *  Free: the map opens on what is used and Free waits behind the toggle. */
const WHOLE_FROM = 0.2;

const k = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(n >= 10_000 ? 0 : 1)}k` : String(n));

const TABS = [
  ["settings", "Settings"],
  ["context", "Context"],
  ["traces", "Traces"],
  ["routines", "Routines"],
  ["desktop", "Desktop"],
  ["browser", "Browser"],
] as const;
type Tab = (typeof TABS)[number][0];

export function AgentSide({ agent, thread: open, threadId }: { agent: Agent; thread: Thread | null; threadId: string | null }) {
  // On the bot's page no thread is open: read its latest one.
  const { epoch } = useChat();
  const [read, setRead] = useState<Thread | null>(null);
  useEffect(() => {
    if (open || !threadId) return setRead(null);
    let live = true;
    getHost()
      .invoke<Thread | null>("chat_thread", { id: threadId })
      .then((t) => live && setRead(t))
      .catch(() => live && setRead(null));
    return () => {
      live = false;
    };
  }, [open, threadId, epoch]);
  const thread = open ?? read;
  const [tab, setTab] = useState<Tab>(() => {
    try {
      const t = localStorage.getItem("bs.agentSide") as Tab;
      return TABS.some(([id]) => id === t) ? t : "settings";
    } catch {
      return "settings";
    }
  });
  const pick = (t: Tab) => {
    setTab(t);
    try {
      localStorage.setItem("bs.agentSide", t);
    } catch {
      /* private window */
    }
  };
  // The panel's width is the user's: drag its left edge, it stays put.
  const [w, setW] = useState(() => {
    try {
      const n = Number(localStorage.getItem("bs.agentSideW"));
      return n >= 260 && n <= 640 ? n : 360;
    } catch {
      return 360;
    }
  });
  const drag = (e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    const x0 = e.clientX, w0 = w;
    let last = w0;
    const move = (m: PointerEvent) => setW((last = Math.min(640, Math.max(260, w0 + x0 - m.clientX))));
    const up = () => {
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", up);
      try {
        localStorage.setItem("bs.agentSideW", String(Math.round(last)));
      } catch {
        /* private window */
      }
    };
    addEventListener("pointermove", move);
    addEventListener("pointerup", up);
  };
  // And it folds away: a slim strip with the one button that brings it back.
  const [shut, setShut] = useState(() => {
    try {
      return localStorage.getItem("bs.agentSideShut") === "1";
    } catch {
      return false;
    }
  });
  const fold = (v: boolean) => {
    setShut(v);
    try {
      localStorage.setItem("bs.agentSideShut", v ? "1" : "0");
    } catch {
      /* private window */
    }
  };
  const busy = thread?.messages.some((m) => m.role === "assistant" && m.status === "streaming");
  if (shut) {
    return (
      <aside className="agent-side flex shrink-0 flex-col items-center border-l border-border pt-3" style={{ width: 44 }} aria-label={`${agent.name}'s panel, folded`}>
        <button type="button" onClick={() => fold(false)} aria-label="Open the panel" title="Open the panel" className="grid size-8 cursor-pointer place-items-center rounded-lg text-muted-foreground transition-colors hover:bg-well hover:text-foreground">
          <IconLayoutSidebarRightExpand size={17} />
        </button>
      </aside>
    );
  }
  return (
    <aside className="agent-side relative flex min-h-0 shrink-0 flex-col overflow-hidden border-l border-border" style={{ width: w }} aria-label={`${agent.name}'s panel`}>
      <div role="separator" aria-orientation="vertical" aria-label="Resize panel" onPointerDown={drag} className="absolute inset-y-0 -left-1 z-10 w-2 cursor-col-resize hover:bg-foreground/10" />
      <button type="button" onClick={() => fold(true)} aria-label="Fold the panel away" title="Fold the panel away" className="absolute top-2 right-2 z-10 grid size-7 cursor-pointer place-items-center rounded-lg text-muted-foreground transition-colors hover:bg-well hover:text-foreground">
        <IconLayoutSidebarRightCollapse size={16} />
      </button>
      <div className="flex shrink-0 flex-col items-center gap-1 px-3 pt-4 pb-2">
        <div className="max-w-full truncate text-[15px] font-semibold text-foreground">{agent.name}</div>
        <Orb agent={agent} size={104} state={busy ? "working" : "idle"} />
        <button type="button" onClick={() => openBot(agent.id)} className="text-[11.5px] text-muted-foreground underline-offset-2 hover:text-foreground hover:underline">
          Sessions, cards and projects
        </button>
      </div>
      <div className="flex shrink-0 flex-wrap justify-center gap-x-2 border-b border-border px-2" role="tablist">
        {TABS.map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            onClick={() => pick(id)}
            className={cn(
              "-mb-px h-8 shrink-0 border-b-2 border-transparent text-[12px] font-medium text-muted-foreground transition-colors hover:text-foreground",
              tab === id && "border-foreground text-foreground",
            )}
          >
            {label}
          </button>
        ))}
      </div>
      <div className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-3 pt-3 pb-4">
        {tab === "settings" && <SettingsTab agent={agent} />}
        {tab === "context" && <ContextTab thread={thread} agent={agent} />}
        {tab === "traces" && <TracesTab thread={thread} />}
        {tab === "routines" && <RoutineCards agent={agent} />}
        {(tab === "desktop" || tab === "browser") && <ScreenTab agent={agent} focus={tab} />}
      </div>
    </aside>
  );
}

// ------------------------------------------------------------------ context

function ContextTab({ thread, agent }: { thread: Thread | null; agent: Agent }) {
  const { epoch } = useChat();
  const [map, setMap] = useState<ContextMap | null | undefined>(undefined);
  const [whole, setWhole] = useState<boolean | null>(null);
  const count = thread?.messages.length ?? 0;
  useEffect(() => {
    if (!thread) return setMap(null);
    let live = true;
    getHost()
      .invoke<ContextMap | null>("chat_context", { id: thread.id })
      .then((m) => live && setMap(m))
      .catch(() => live && setMap(null));
    return () => {
      live = false;
    };
  }, [thread?.id, count, epoch, agent.route]);

  if (!thread) return <p className="py-2 text-[12.5px] text-muted-foreground">Start a conversation and its context window shows up here.</p>;
  if (map === undefined) return <p className="py-2 text-[12.5px] text-muted-foreground">Reading the context…</p>;
  if (!map) return <p className="py-2 text-[12.5px] text-muted-foreground">This conversation's context couldn't be read.</p>;

  const used = map.parts.reduce((s, p) => s + p.tokens, 0);
  const share = used / map.window;
  const pct = Math.min(100, share * 100);
  const showFree = whole ?? share >= WHOLE_FROM;
  const groups = ORDER.map((g) => ({ g, parts: map.parts.filter((p) => p.group === g && p.tokens > 0) })).filter((x) => x.parts.length);
  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex items-center gap-2">
        <span className="inline-flex min-w-0 items-center gap-1.5 rounded-lg px-2 py-1 text-[12px] font-medium text-foreground shadow-[inset_0_0_0_1px_var(--border)]">
          <Logo id={routeGlyphId(agent.route)} size={13} />
          <span className="truncate">{map.model}</span>
          <span className="rounded bg-well px-1 text-[11px] text-muted-foreground tabular-nums">{k(map.window)}</span>
        </span>
        <span className="ml-auto shrink-0 text-[12px] text-muted-foreground tabular-nums">
          <b className="font-semibold text-foreground">{k(used)}</b> / {k(map.window)}
        </span>
        <span className="shrink-0 rounded-md bg-well px-1.5 py-0.5 text-[11.5px] font-semibold tabular-nums">{pct < 1 ? pct.toFixed(1) : Math.round(pct)}%</span>
      </div>
      <ContextMapView groups={groups} window={map.window} free={showFree ? Math.max(0, map.window - used) : 0} />
      <div className="flex items-start justify-between gap-2 text-[11.5px] text-muted-foreground">
        <p>
          Estimated at four characters a token.
          {map.last_input != null && ` The last reply read ${k(map.last_input)}, by its trace.`}
        </p>
        <button
          type="button"
          onClick={() => setWhole(!showFree)}
          className="shrink-0 underline-offset-2 hover:text-foreground hover:underline"
          title={showFree ? "Show only what is used" : "Show the whole window, free space included"}
        >
          {showFree ? "Used only" : "Whole window"}
        </button>
      </div>
    </div>
  );
}

type Rect = { x: number; y: number; w: number; h: number };

/** Squarified treemap (Bruls, Huizing, van Wijk): lay `values` into `r` with
 *  each row's worst aspect ratio as close to 1 as it gets. */
function squarify(values: number[], r: Rect): Rect[] {
  const out: Rect[] = new Array(values.length);
  const total = values.reduce((s, v) => s + v, 0);
  if (!total || r.w <= 0 || r.h <= 0) return values.map(() => ({ x: r.x, y: r.y, w: 0, h: 0 }));
  const scale = (r.w * r.h) / total;
  const idx = values.map((v, i) => ({ a: v * scale, i })).sort((a, b) => b.a - a.a);
  let { x, y, w, h } = r;
  let row: { a: number; i: number }[] = [];
  const worst = (row: { a: number }[], side: number) => {
    const s = row.reduce((t, c) => t + c.a, 0);
    const mx = Math.max(...row.map((c) => c.a));
    const mn = Math.min(...row.map((c) => c.a));
    return Math.max((side * side * mx) / (s * s), (s * s) / (side * side * mn));
  };
  const lay = () => {
    const s = row.reduce((t, c) => t + c.a, 0);
    if (w >= h) {
      const cw = s / h;
      let cy = y;
      for (const c of row) {
        const ch = c.a / cw;
        out[c.i] = { x, y: cy, w: cw, h: ch };
        cy += ch;
      }
      x += cw;
      w -= cw;
    } else {
      const ch = s / w;
      let cx = x;
      for (const c of row) {
        const cw = c.a / ch;
        out[c.i] = { x: cx, y, w: cw, h: ch };
        cx += cw;
      }
      y += ch;
      h -= ch;
    }
    row = [];
  };
  for (const c of idx) {
    const side = Math.min(w, h);
    if (!row.length || worst([...row, c], side) <= worst(row, side)) row.push(c);
    else {
      lay();
      row.push(c);
    }
  }
  if (row.length) lay();
  return out;
}

type Tile = { key: string; group: string; label: string; tokens: number; r: Rect };

/** The map, standing up so it fits the panel: what is used on top, laid out
 *  kind by kind, and the free space as one hatched band under it. */
function ContextMapView({ groups, window: win, free }: { groups: { g: string; parts: Part[] }[]; window: number; free: number }) {
  // The map is drawn to the panel as it is now: it follows a dragged width and
  // a narrow window instead of overflowing a fixed 276px.
  const wrap = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(276);
  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setWidth(Math.max(140, Math.floor(el.clientWidth) - 8)));
    ro.observe(el);
    setWidth(Math.max(140, Math.floor(el.clientWidth) - 8));
    return () => ro.disconnect();
  }, []);
  const W = width;
  const H = Math.round(Math.min(520, Math.max(240, W * 1.6)));
  const GAP = 3;
  const used = groups.reduce((s, x) => s + x.parts.reduce((t, p) => t + p.tokens, 0), 0);
  const tiles = useMemo(() => {
    const top = free ? Math.round((H * used) / (used + free)) : H;
    const blocks = squarify(
      groups.map((x) => x.parts.reduce((s, p) => s + p.tokens, 0)),
      { x: 0, y: 0, w: W, h: top },
    );
    const out: Tile[] = [];
    groups.forEach((grp, i) => {
      const b = blocks[i];
      const kr = squarify(grp.parts.map((p) => p.tokens), b);
      grp.parts.forEach((p, j) => out.push({ key: `${grp.g}-${j}`, group: grp.g, label: p.label, tokens: p.tokens, r: kr[j] }));
    });
    if (free) out.push({ key: "free", group: "Free", label: "Free", tokens: free, r: { x: 0, y: top, w: W, h: H - top } });
    return out;
  }, [groups, free, used, W, H]);
  const box = useRef<HTMLDivElement>(null);
  const [tip, setTip] = useState<{ t: Tile; x: number; y: number } | null>(null);
  return (
    <div ref={wrap} className="w-full min-w-0">
    <div
      ref={box}
      className="context-map relative rounded-xl p-1 shadow-[inset_0_0_0_1px_var(--border)]"
      style={{ height: H + 8 }}
      role="img"
      aria-label="The context window as a map: the bigger a box, the more of the window it takes"
      onPointerLeave={() => setTip(null)}
    >
      {tiles.map((t) => {
        const kind = KIND[t.group];
        const w = Math.max(0, t.r.w - GAP);
        const h = Math.max(0, t.r.h - GAP);
        const roomy = w > 46 && h > 34;
        if (!kind)
          return (
            <div
              key={t.key}
              className="absolute rounded-md px-2 py-1.5 text-[12px]/4 text-muted-foreground/70"
              style={{
                left: 4 + t.r.x + GAP / 2,
                top: 4 + t.r.y + GAP / 2,
                width: w,
                height: h,
                background: "repeating-linear-gradient(135deg, color-mix(in srgb, var(--muted-foreground) 9%, transparent) 0 1px, transparent 1px 7px)",
                border: "1px dashed color-mix(in srgb, var(--muted-foreground) 30%, transparent)",
              }}
            >
              <div>Free</div>
              <div className="tabular-nums">{k(t.tokens)}</div>
            </div>
          );
        return (
          <div
            key={t.key}
            className="absolute overflow-hidden rounded-md px-2 py-1.5 text-[12px]/4"
            style={{
              left: 4 + t.r.x + GAP / 2,
              top: 4 + t.r.y + GAP / 2,
              width: w,
              height: h,
              background: `color-mix(in srgb, ${kind.hue} 13%, transparent)`,
              boxShadow: `inset 0 0 0 1px color-mix(in srgb, ${kind.hue} 30%, transparent)`,
              color: `color-mix(in srgb, ${kind.hue} 82%, var(--foreground))`,
            }}
            onPointerMove={(e) => {
              const b = box.current?.getBoundingClientRect();
              if (b) setTip({ t, x: e.clientX - b.left, y: e.clientY - b.top });
            }}
          >
            {roomy && (
              <>
                <div className="truncate font-medium">{t.label}</div>
                <div className="truncate opacity-70 tabular-nums">
                  {k(t.tokens)}
                  {w > 80 ? ` · ${kind.word}` : ""}
                </div>
              </>
            )}
          </div>
        );
      })}
      {tip && KIND[tip.t.group] && (
        <div
          className="pointer-events-none absolute z-10 w-max max-w-[220px] rounded-xl bg-popover px-3 py-2 text-[12px]/5 text-popover-foreground shadow-lg ring-1 ring-border"
          style={{ left: Math.max(0, Math.min(tip.x + 12, W - 150)), top: tip.y + 14 }}
        >
          <div className="flex items-center gap-1.5 font-medium">
            <i className="size-2.5 shrink-0 rounded-[3px]" style={{ background: `color-mix(in srgb, ${KIND[tip.t.group].hue} 30%, transparent)`, boxShadow: `inset 0 0 0 1px ${KIND[tip.t.group].hue}` }} />
            <span className="truncate">{tip.t.label}</span>
          </div>
          <div className="text-muted-foreground tabular-nums">
            {KIND[tip.t.group].word} · {k(tip.t.tokens)} tokens · {((tip.t.tokens / win) * 100).toFixed(1)}%
          </div>
        </div>
      )}
    </div>
    </div>
  );
}

// ------------------------------------------------------------------- traces

function dur(ms: number) {
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  return `${Math.floor(ms / 60_000)} min`;
}

function TracesTab({ thread }: { thread: Thread | null }) {
  const replies = useMemo(() => (thread?.messages ?? []).filter((m) => m.role === "assistant" && m.trace).reverse().slice(0, 20), [thread]);
  const [traces, setTraces] = useState<Record<string, Trace | null>>({});
  useEffect(() => {
    let live = true;
    for (const m of replies) {
      if (!m.trace || m.trace in traces) continue;
      getHost()
        .invoke<Trace | null>("trace_get", { id: m.trace })
        .then((t) => live && setTraces((s) => ({ ...s, [m.trace!]: t })))
        .catch(() => live && setTraces((s) => ({ ...s, [m.trace!]: null })));
    }
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [replies]);
  if (!replies.length)
    return <p className="py-2 text-[12.5px] text-muted-foreground">No traced replies yet. Each reply records its tool calls, time and tokens here.</p>;
  return (
    <div className="flex flex-col gap-1">
      {replies.map((m) => {
        const t = traces[m.trace!];
        const root = t?.spans[0];
        const a = root?.attrs ?? {};
        const tools = t ? t.spans.length - 1 : null;
        const failed = t?.spans.some((s) => !!s.error);
        const tin = typeof a["gen_ai.usage.input_tokens"] === "number" ? (a["gen_ai.usage.input_tokens"] as number) : null;
        return (
          <button
            key={m.id}
            type="button"
            onClick={() => openTrace(m.trace!)}
            className="group flex items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-well"
          >
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-1.5 truncate text-[12.5px] text-foreground">
                {failed && <IconAlertTriangleFilled className="size-3 shrink-0 text-destructive" />}
                <span className="truncate">{m.text.replace(/[*_`#>|[\]]/g, "").split(/\s+/).slice(0, 8).join(" ") || "Reply"}</span>
              </div>
              <div className="truncate text-[11.5px] text-muted-foreground tabular-nums">
                {new Date(m.at).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}
                {root?.end ? ` · ${dur(root.end - root.start)}` : ""}
                {tools != null ? ` · ${tools} tool${tools === 1 ? "" : "s"}` : ""}
                {tin != null ? ` · ${k(tin)} in` : ""}
              </div>
            </div>
            <IconChevronRight className="size-3.5 shrink-0 text-muted-foreground opacity-0 group-hover:opacity-100" />
          </button>
        );
      })}
    </div>
  );
}

// ----------------------------------------------------------------- settings

const field = "w-full rounded-lg bg-well px-2.5 py-1.5 text-[12.5px]/5 text-foreground outline-none shadow-[inset_0_0_0_1px_var(--border)] focus:shadow-[inset_0_0_0_1px_var(--ring)]";

function SettingsTab({ agent }: { agent: Agent }) {
  const host = getHost();
  const save = (p: Partial<Agent>) => void saveAgent({ ...agent, ...p }).catch((e) => host.toast(String(e), "err"));
  // The text fields are drafts until they lose focus, so a keystroke is not a save.
  const [job, setJob] = useState(agent.job);
  const [persona, setPersona] = useState(agent.personality);
  useEffect(() => setJob(agent.job), [agent.id, agent.job]);
  useEffect(() => setPersona(agent.personality), [agent.id, agent.personality]);
  return (
    <div className="flex flex-col gap-3.5 text-[12.5px]">
      <div className="flex items-center justify-between gap-2">
        <span className="font-medium text-foreground">Model</span>
        <RoutePicker value={agent.route} onValueChange={(route) => save({ route })} />
      </div>
      <label className="flex flex-col gap-1">
        <span className="font-medium text-foreground">Instructions</span>
        <textarea
          className={`${field} min-h-28 resize-y`}
          value={job}
          placeholder="What it does, and how. The first line is its job; the rest is its backstory."
          onChange={(e) => setJob(e.target.value)}
          onBlur={() => job !== agent.job && save({ job })}
        />
      </label>
      <div className="flex flex-col gap-1">
        <span className="font-medium text-foreground">Personality</span>
        <div className="flex flex-wrap gap-1">
          {PERSONALITIES.map((p) => (
            <button
              key={p.name}
              type="button"
              onClick={() => {
                setPersona(p.text);
                save({ personality: p.text });
              }}
              className={cn("h-6 rounded-full px-2 text-[11.5px] font-medium", persona === p.text ? "bg-foreground text-(--surface)" : "bg-well text-muted-foreground hover:text-foreground")}
            >
              {p.name}
            </button>
          ))}
        </div>
        <textarea
          className={`${field} min-h-14 resize-y`}
          value={persona}
          placeholder="How it talks, in one or two sentences"
          onChange={(e) => setPersona(e.target.value)}
          onBlur={() => persona !== agent.personality && save({ personality: persona })}
        />
      </div>
      <Row title="Memory" sub="Reads and adds to the notes of the project it works in">
        <Toggle on={agent.memory} label="Memory" onChange={(on) => save({ memory: on })} />
      </Row>
      {RULES.map(([id, title, sub]) => (
        <Row key={id} title={title} sub={sub}>
          <Toggle
            on={!agent.off.includes(id)}
            label={title}
            onChange={(on) => save({ off: on ? agent.off.filter((x) => x !== id) : [...agent.off, id] })}
          />
        </Row>
      ))}
      <Row title="Computer" sub={agent.computer.kind === "docker" ? "Its own container" : agent.computer.kind === "ssh" ? agent.computer.host : "A folder on this machine"} />
      <button type="button" onClick={() => editAgent(agent)} className="inline-flex h-8 items-center justify-center gap-1.5 rounded-md bg-well text-[12.5px] font-medium hover:text-foreground">
        <IconSettings className="size-3.5" /> Name, colour, computer and apps
      </button>
    </div>
  );
}

// ------------------------------------------------------------------ screen

type Step = { kind?: string; action?: string; text?: string | null; at?: number };
type Live = {
  kind: "browser" | "desktop" | null;
  url: string | null;
  title: string | null;
  at: number | null;
  frame: string | null;
  steps: Step[];
  paused: boolean;
  browser_running: boolean;
};
type HandsCfg = { decider: string; model: string; chrome: string; headless: boolean };

const DECIDERS = [
  ["", "Its own model", "It reads each page and picks the next step itself"],
  ["local", "On-device", "A small model on this machine runs whole browser tasks"],
  ["jev", "Jev", "TypeSafe's hosted Jev runs whole browser tasks (TYPESAFE_API_KEY)"],
] as const;

function ago(s: number | null) {
  if (!s) return "";
  const d = Math.max(0, Date.now() / 1000 - s);
  return d < 60 ? "just now" : d < 3600 ? `${Math.round(d / 60)} min ago` : `${Math.round(d / 3600)} h ago`;
}

/** Its browser and desktop use: what it last saw, the steps it took, Take
 *  control, and the switches (after Reason's Desktop tab). */
function ScreenTab({ agent, focus }: { agent: Agent; focus: "desktop" | "browser" }) {
  const host = getHost();
  const [live, setLive] = useState<Live | null>(null);
  const [cfg, setCfg] = useState<HandsCfg | null>(null);
  useEffect(() => {
    let on = true;
    const tick = () =>
      host
        .invoke<Live>("agent_hands", { id: agent.id })
        .then((l) => on && setLive(l))
        .catch(() => {});
    tick();
    const t = setInterval(tick, 1500);
    return () => {
      on = false;
      clearInterval(t);
    };
  }, [agent.id]);
  useEffect(() => {
    host
      .invoke<HandsCfg>("hands_prefs")
      .then(setCfg)
      .catch(() => {});
  }, []);
  const save = (p: Partial<Agent>) => void saveAgent({ ...agent, ...p }).catch((e) => host.toast(String(e), "err"));
  const hands = agent.hands ?? ["browser"];
  const has = (h: string) => hands.includes(h);
  const flip = (h: string, on: boolean) => save({ hands: on ? [...hands.filter((x) => x !== h), h] : hands.filter((x) => x !== h) });
  const take = (on: boolean) =>
    host
      .invoke("agent_hands_take", { id: agent.id, on })
      .then(() => setLive((l) => (l ? { ...l, paused: on } : l)))
      .catch((e) => host.toast(String(e), "err"));
  const setDecider = (p: Partial<HandsCfg>) => {
    if (!cfg) return;
    const next = { ...cfg, ...p };
    setCfg(next);
    void host.invoke("set_hands_prefs", { cfg: next }).catch((e) => host.toast(String(e), "err"));
  };
  const webOff = agent.off.includes("web");
  return (
    <div className="flex flex-col gap-3.5 text-[12.5px]">
      <div className="screen-view relative overflow-hidden rounded-lg border border-border bg-well">
        {live?.frame ? (
          <img src={live.frame} alt={`What ${agent.name} last saw`} className="block aspect-[16/10] w-full object-cover object-top" />
        ) : (
          <div className="flex aspect-[16/10] w-full flex-col items-center justify-center gap-1 px-4 text-center text-[11.5px] text-muted-foreground">
            <IconBrowser className="size-5" />
            Nothing yet. When {agent.name} opens a page or looks at your screen, it shows here.
          </div>
        )}
        {live?.paused && <div className="absolute inset-x-0 top-0 bg-[#d97706] px-2 py-0.5 text-center text-[11px] font-medium text-white">You have control</div>}
      </div>
      {live?.kind && (
        <div className="-mt-2 flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
          {live.kind === "desktop" ? <IconDeviceDesktop className="size-3.5 shrink-0" /> : <IconBrowser className="size-3.5 shrink-0" />}
          <span className="min-w-0 flex-1 truncate">{live.kind === "desktop" ? "Your screen" : live.title || live.url}</span>
          <span className="shrink-0">{ago(live.at)}</span>
        </div>
      )}
      <button
        type="button"
        onClick={() => take(!live?.paused)}
        className={cn("inline-flex h-8 items-center justify-center gap-1.5 rounded-md text-[12.5px] font-medium", live?.paused ? "bg-foreground text-(--surface)" : "bg-well hover:text-foreground")}
      >
        <IconHandStop className="size-3.5" /> {live?.paused ? `Hand back to ${agent.name}` : "Take control"}
      </button>
      {!!live?.steps.length && (
        <ol className="flex max-h-48 flex-col gap-1 overflow-y-auto">
          {live.steps.map((s, i) => (
            <li key={i} className="flex gap-1.5 text-[11.5px]">
              <span className="w-12 shrink-0 text-muted-foreground">{s.kind}</span>
              <span className="min-w-0 flex-1 truncate text-foreground">
                {s.action}
                {s.text ? ` ← “${s.text}”` : ""}
              </span>
            </li>
          ))}
        </ol>
      )}
      {focus === "browser" ? (
        <Row title="Browser use" sub={webOff ? "Off while its Web rule is off" : "Its own tab in a Chrome you can watch and sign in to"}>
          <Toggle on={has("browser") && !webOff} label="Browser use" onChange={(on) => flip("browser", on)} />
        </Row>
      ) : (
        <Row title="Desktop use" sub="Sees your screen and uses your mouse and keyboard">
          <Toggle on={has("desktop")} label="Desktop use" onChange={(on) => flip("desktop", on)} />
        </Row>
      )}
      {focus === "browser" && cfg && (
        <div className="flex flex-col gap-1">
          <span className="font-medium text-foreground">Fast browser model</span>
          <div className="flex gap-1">
            {DECIDERS.map(([id, name]) => (
              <button
                key={id}
                type="button"
                onClick={() => setDecider({ decider: id })}
                className={cn("h-6 rounded-full px-2 text-[11.5px] font-medium", cfg.decider === id ? "bg-foreground text-(--surface)" : "bg-well text-muted-foreground hover:text-foreground")}
              >
                {name}
              </button>
            ))}
          </div>
          <span className="text-[11.5px] text-muted-foreground">{DECIDERS.find(([id]) => id === cfg.decider)?.[2]}. Shared by every agent.</span>
          {cfg.decider === "local" && (
            <input
              className={field}
              defaultValue={cfg.model}
              placeholder="ollama:qwen3:4b (empty: your small local model)"
              onBlur={(e) => e.target.value !== cfg.model && setDecider({ model: e.target.value.trim() })}
            />
          )}
        </div>
      )}
    </div>
  );
}

function Row({ title, sub, children }: { title: string; sub?: string; children?: React.ReactNode }) {
  return (
    <div className="flex items-center gap-3">
      <div className="min-w-0 flex-1">
        <div className="font-medium text-foreground">{title}</div>
        {sub && <div className="truncate text-[11.5px] text-muted-foreground">{sub}</div>}
      </div>
      {children}
    </div>
  );
}

function Toggle({ on, label, onChange }: { on: boolean; label: string; onChange: (on: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={() => onChange(!on)}
      className={cn("relative h-5 w-8 shrink-0 rounded-full transition-colors", on ? "bg-[#2f9e6e]" : "bg-[color-mix(in_srgb,var(--muted-foreground)_35%,transparent)]")}
    >
      <span className={cn("absolute top-0.5 left-0 size-4 rounded-full bg-white shadow-sm transition-transform", on ? "translate-x-3.5" : "translate-x-0.5")} />
    </button>
  );
}
