// The agent's side panel: on the right of a bot's thread and of its page.
// Three tabs: Context (what its next reply reads, as a treemap the way a disk
// space map draws a drive), Traces (how each reply was made) and Settings (the
// switches you reach for mid-conversation; the rest is a click away).

import { useEffect, useMemo, useState } from "react";
import { IconAlertTriangleFilled, IconChevronRight, IconSettings } from "@tabler/icons-react";

import { cn } from "@/lib/utils";
import { editAgent, getHost, openTrace, saveAgent, useChat, type Agent, type Thread, type Trace } from "../bridge";
import { RULES } from "./Agents";

type Part = { group: string; label: string; tokens: number };
type ContextMap = { window: number; model: string; parts: Part[]; last_input: number | null };

// The dataviz palette, validated in both themes; one hue per kind of content.
const HUE: Record<string, string> = {
  Instructions: "#3b82f6",
  Memory: "#2a9a72",
  Conversation: "#c96a4a",
  Files: "#8b6ff0",
};
const ORDER = ["Instructions", "Memory", "Conversation", "Files"];

const k = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(n >= 10_000 ? 0 : 1)}k` : String(n));

const TABS = [
  ["context", "Context"],
  ["traces", "Traces"],
  ["settings", "Settings"],
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
      return (localStorage.getItem("bs.agentSide") as Tab) || "context";
    } catch {
      return "context";
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
  return (
    <aside className="agent-side flex w-[300px] shrink-0 flex-col border-l border-border" aria-label={`${agent.name}'s panel`}>
      <div className="flex shrink-0 gap-1 px-3 pt-3 pb-2" role="tablist">
        {TABS.map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            onClick={() => pick(id)}
            className={cn(
              "h-7 rounded-md px-2.5 text-[12.5px] font-medium text-muted-foreground transition-colors hover:text-foreground",
              tab === id && "bg-well text-foreground",
            )}
          >
            {label}
          </button>
        ))}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-3 pb-4">
        {tab === "context" && <ContextTab thread={thread} />}
        {tab === "traces" && <TracesTab thread={thread} />}
        {tab === "settings" && <SettingsTab agent={agent} />}
      </div>
    </aside>
  );
}

// ------------------------------------------------------------------ context

function ContextTab({ thread }: { thread: Thread | null }) {
  const { epoch } = useChat();
  const [map, setMap] = useState<ContextMap | null | undefined>(undefined);
  const [whole, setWhole] = useState(false);
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
  }, [thread?.id, count, epoch]);

  if (!thread) return <p className="py-2 text-[12.5px] text-muted-foreground">Start a conversation and its context window shows up here.</p>;
  if (map === undefined) return <p className="py-2 text-[12.5px] text-muted-foreground">Reading the context…</p>;
  if (!map) return <p className="py-2 text-[12.5px] text-muted-foreground">This conversation's context couldn't be read.</p>;

  const used = map.parts.reduce((s, p) => s + p.tokens, 0);
  const pct = Math.min(100, (used / map.window) * 100);
  const groups = ORDER.map((g) => ({ g, parts: map.parts.filter((p) => p.group === g) })).filter((x) => x.parts.length);
  return (
    <div className="grid gap-3">
      <div>
        <div className="flex items-baseline justify-between text-[12.5px]">
          <span className="font-medium text-foreground">
            {k(used)} <span className="font-normal text-muted-foreground">of {k(map.window)} tokens</span>
          </span>
          <span className="text-muted-foreground tabular-nums">{pct < 1 ? pct.toFixed(1) : Math.round(pct)}%</span>
        </div>
        <div className="mt-1.5 flex h-1.5 overflow-hidden rounded-full bg-well">
          {groups.map(({ g, parts }) => (
            <span key={g} style={{ width: `${(parts.reduce((s, p) => s + p.tokens, 0) / map.window) * 100}%`, background: HUE[g] }} />
          ))}
        </div>
        <p className="mt-1.5 text-[11.5px] text-muted-foreground">
          Estimated at four characters a token for {map.model}.
          {map.last_input != null && ` The last reply read ${k(map.last_input)}, by its trace.`}
        </p>
      </div>
      <div className="flex items-center justify-between">
        <div className="flex flex-wrap gap-x-3 gap-y-1 text-[11.5px] text-muted-foreground">
          {groups.map(({ g }) => (
            <span key={g} className="inline-flex items-center gap-1.5">
              <i className="size-2 rounded-[2px]" style={{ background: HUE[g] }} />
              {g}
            </span>
          ))}
        </div>
        <button
          type="button"
          onClick={() => setWhole((w) => !w)}
          className="shrink-0 text-[11.5px] text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
          title={whole ? "Show only what is used" : "Show the whole window, free space included"}
        >
          {whole ? "Used" : "Whole window"}
        </button>
      </div>
      <Treemap groups={groups} free={whole ? Math.max(0, map.window - used) : 0} />
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

function Treemap({ groups, free }: { groups: { g: string; parts: Part[] }[]; free: number }) {
  const W = 276;
  const H = 300;
  const blocks = useMemo(() => {
    const tops = [...groups.map((x) => x.parts.reduce((s, p) => s + p.tokens, 0)), ...(free ? [free] : [])];
    const rects = squarify(tops, { x: 0, y: 0, w: W, h: H });
    return rects.map((r, i) => {
      const grp = groups[i];
      if (!grp) return { key: "free", r, kids: [] as { p: Part; r: Rect }[] };
      const head = r.h > 40 && r.w > 60 ? 16 : 0;
      const inner = { x: r.x + 2, y: r.y + 2 + head, w: Math.max(0, r.w - 4), h: Math.max(0, r.h - 4 - head) };
      const kr = squarify(grp.parts.map((p) => p.tokens), inner);
      return { key: grp.g, r, head, kids: grp.parts.map((p, j) => ({ p, r: kr[j] })) };
    });
  }, [groups, free]);
  return (
    <div className="relative overflow-hidden rounded-lg" style={{ height: H }} role="img" aria-label="The context window as a map: the bigger a box, the more of the window it takes">
      {blocks.map((b) =>
        b.key === "free" ? (
          <div
            key="free"
            className="absolute grid place-items-center rounded-md text-[11px] text-muted-foreground"
            style={{ left: b.r.x + 1, top: b.r.y + 1, width: b.r.w - 2, height: b.r.h - 2, background: "repeating-linear-gradient(135deg, var(--well, rgba(127,127,127,.08)) 0 6px, transparent 6px 12px)", boxShadow: "inset 0 0 0 1px var(--border)" }}
          >
            Free
          </div>
        ) : (
          <div
            key={b.key}
            className="absolute rounded-md"
            style={{ left: b.r.x + 1, top: b.r.y + 1, width: b.r.w - 2, height: b.r.h - 2, background: `color-mix(in srgb, ${HUE[b.key]} 22%, transparent)` }}
          >
            {"head" in b && b.head ? (
              <div className="truncate px-1.5 pt-0.5 text-[10.5px] font-semibold" style={{ color: HUE[b.key] }}>
                {b.key}
              </div>
            ) : null}
            {b.kids.map(({ p, r }, j) => (
              <div
                key={j}
                title={`${p.label} · ${k(p.tokens)} tokens`}
                className="absolute overflow-hidden rounded-[4px] px-1 py-0.5 text-[10px]/3 text-white"
                style={{
                  left: r.x - b.r.x,
                  top: r.y - b.r.y,
                  width: Math.max(0, r.w - 1),
                  height: Math.max(0, r.h - 1),
                  background: `color-mix(in srgb, ${HUE[b.key]} ${70 + (j % 3) * 10}%, #000)`,
                }}
              >
                {r.w > 44 && r.h > 22 && (
                  <>
                    <div className="truncate">{p.label}</div>
                    <div className="opacity-70 tabular-nums">{k(p.tokens)}</div>
                  </>
                )}
              </div>
            ))}
          </div>
        ),
      )}
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

function SettingsTab({ agent }: { agent: Agent }) {
  const host = getHost();
  const save = (p: Partial<Agent>) => void saveAgent({ ...agent, ...p }).catch((e) => host.toast(String(e), "err"));
  return (
    <div className="flex flex-col gap-3 text-[12.5px]">
      <Row title="Model" sub={host.routeName(agent.route)} />
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
        <IconSettings className="size-3.5" /> All of {agent.name}'s settings
      </button>
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
      className={cn("relative h-5 w-8 shrink-0 rounded-full transition-colors", on ? "bg-foreground" : "bg-well")}
    >
      <span className={cn("absolute top-0.5 size-4 rounded-full bg-background transition-transform", on ? "translate-x-3.5" : "translate-x-0.5")} />
    </button>
  );
}
