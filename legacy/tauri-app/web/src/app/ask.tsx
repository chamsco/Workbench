// The two controls a bot can put in a reply: a question with choices, and a
// routine it proposes. Both are fenced JSON (see `Agents::brief_in` in
// crates/core/src/agents.rs), so a CLI, the phone or any other reader sees a
// plain code block; this face draws them as controls instead.
//
// Fenced rather than a tool call because a bot's reply can already be a
// transcript another model reads back — one more fence costs a line, a tool
// round-trip costs a turn and can fail.

import { createContext, useContext, useState } from "react";
import { IconCheck, IconClock, IconX } from "@tabler/icons-react";

import {
  agentById,
  getHost,
  routineWhen,
  saveRoutine,
  sendTo,
  useChat,
  type Routine,
  type Thread,
} from "../bridge";

const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";

/** The thread a message is in: an ask card answers into it. Provided by
 *  ThreadView so RichText (which has no thread) can still send. */
export const MessageActions = createContext<{ thread: Thread } | null>(null);
export const useMessageActions = () => useContext(MessageActions);

// ---------------------------------------------------------------- fences

export type Block =
  | { kind: "md"; text: string }
  | { kind: "fence"; tag: "openui" | "ask" | "routine"; body: string; open: boolean };

const FENCE = /```(openui|ask|routine)[^\n]*\n([\s\S]*?)(```|$)/g;

/** Split a reply into Markdown and the fences this face draws. `open` is a
 *  fence still being streamed: the card waits for its closing backticks. */
export function splitFences(text: string): Block[] {
  const out: Block[] = [];
  let last = 0;
  const md = (t: string) => {
    if (t) out.push(...liftRoutine(t));
  };
  for (const m of text.matchAll(FENCE)) {
    md(text.slice(last, m.index));
    out.push({ kind: "fence", tag: m[1] as "openui" | "ask" | "routine", body: m[2], open: m[3] !== "```" });
    last = m.index! + m[0].length;
  }
  md(text.slice(last));
  return out;
}

/** A model that skips the ```routine fence still writes the JSON: lift a bare
 *  (or ```json) object with a name and instructions out of the prose, so it is
 *  a card to add and not a blob to read. */
function liftRoutine(text: string): Block[] {
  const at = text.search(/\{\s*"(?:name|question)"\s*:/);
  if (at < 0) return [{ kind: "md", text }];
  let depth = 0, inStr = false, esc = false, end = -1;
  for (let i = at; i < text.length; i++) {
    const c = text[i];
    if (inStr) {
      if (esc) esc = false;
      else if (c === "\\") esc = true;
      else if (c === '"') inStr = false;
    } else if (c === '"') inStr = true;
    else if (c === "{") depth++;
    else if (c === "}" && --depth === 0) {
      end = i + 1;
      break;
    }
  }
  if (end < 0) return [{ kind: "md", text }];
  const body = text.slice(at, end);
  const isAsk = /^\{\s*"question"/.test(body);
  const ok = isAsk ? !!parseAsk(body) : !!parseRoutine(body)?.instructions;
  // Not a card after all: keep it as prose, but read on past it.
  if (!ok) {
    const rest = liftRoutine(text.slice(end));
    return rest.length === 1 && rest[0].kind === "md" ? [{ kind: "md", text }] : [{ kind: "md", text: text.slice(0, end) }, ...rest];
  }
  // Take a ```json wrapper along with the object.
  const before = text.slice(0, at).replace(/```(?:json)?\s*$/, "");
  const after = text.slice(end).replace(/^\s*```/, "");
  const out: Block[] = [];
  if (before.trim()) out.push({ kind: "md", text: before });
  out.push({ kind: "fence", tag: isAsk ? "ask" : "routine", body, open: false });
  if (after.trim()) out.push(...liftRoutine(after));
  return out;
}

// ---------------------------------------------------------------- ask

export type AskSpec = { question: string; options: string[]; multiple?: boolean; skip?: boolean };

/** A question the bot asked: invalid JSON is not a control, it is a bad reply. */
export function parseAsk(body: string): AskSpec | null {
  try {
    const v = JSON.parse(body) as Partial<AskSpec>;
    if (typeof v.question !== "string" || !v.question.trim()) return null;
    const options = Array.isArray(v.options) ? v.options.filter((o): o is string => typeof o === "string" && !!o.trim()).slice(0, 12) : [];
    if (options.length < 2) return null;
    return { question: v.question.trim(), options, multiple: v.multiple !== false, skip: v.skip !== false };
  } catch {
    return null;
  }
}

export function AskCard({ spec, answered }: { spec: AskSpec; answered: boolean }) {
  const ctx = useMessageActions();
  const host = getHost();
  const [picked, setPicked] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const toggle = (o: string) =>
    setPicked((cur) => (spec.multiple ? (cur.includes(o) ? cur.filter((x) => x !== o) : [...cur, o]) : [o]));
  const send = async (text: string) => {
    if (!ctx || busy) return;
    setBusy(true);
    try {
      await sendTo(ctx.thread, text);
    } catch (e) {
      host.toast(String(e), "err");
    } finally {
      setBusy(false);
    }
  };
  return (
    <aside
      aria-label="Question from the bot"
      className={`my-3 flex w-full max-w-md flex-col gap-2.5 rounded-2xl bg-well px-4 py-3.5 ${WELL}`}
    >
      <div className="text-[14px]/5 font-medium">{spec.question}</div>
      {answered ? (
        <div className="text-[12.5px] text-muted-foreground">Answered — carry on in the chat if you want to change it.</div>
      ) : (
        <>
          <div role={spec.multiple ? "group" : "radiogroup"} aria-label={spec.question} className="flex flex-col gap-1">
            {spec.options.map((o) => {
              const on = picked.includes(o);
              return (
                <button
                  key={o}
                  type="button"
                  role={spec.multiple ? "checkbox" : "radio"}
                  aria-checked={on}
                  onClick={() => toggle(o)}
                  className={`flex cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[13px]/5 transition-colors ${
                    on ? "bg-foreground/[0.07] text-foreground" : "text-muted-foreground hover:bg-accent hover:text-foreground"
                  }`}
                >
                  <span
                    className={`grid size-4 shrink-0 place-items-center border ${
                      spec.multiple ? "rounded-[5px]" : "rounded-full"
                    } ${on ? "border-foreground bg-foreground text-(--surface)" : "border-foreground/30"}`}
                  >
                    {on && <IconCheck size={11} strokeWidth={3} />}
                  </span>
                  <span className="min-w-0">{o}</span>
                </button>
              );
            })}
          </div>
          <div className="mt-0.5 flex items-center gap-2">
            {spec.skip && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void send("Skip this question.")}
                className="flex h-8 cursor-pointer items-center gap-1.5 rounded-full px-3 text-[12.5px] font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:cursor-default disabled:opacity-50"
              >
                <IconX size={13} /> Skip
              </button>
            )}
            <button
              type="button"
              disabled={busy || picked.length === 0}
              onClick={() => void send(`Answer: ${picked.join(", ")}`)}
              className="ml-auto flex h-8 cursor-pointer items-center gap-1.5 rounded-full bg-primary px-3.5 text-[12.5px] font-medium text-primary-foreground transition-[background-color,scale] duration-150 hover:bg-(--primary-hover) active:scale-[0.97] disabled:cursor-default disabled:opacity-40"
            >
              Answer
            </button>
          </div>
        </>
      )}
    </aside>
  );
}

// ---------------------------------------------------------------- routine

export type RoutineSpec = {
  name: string;
  instructions?: string;
  kind?: string;
  time?: string;
  days?: number[];
  every_minutes?: number;
  project?: string;
};

const KINDS = new Set(["daily", "weekdays", "weekly", "every"]);

export function parseRoutine(body: string): RoutineSpec | null {
  try {
    const v = JSON.parse(body) as Partial<RoutineSpec>;
    if (typeof v.name !== "string" || !v.name.trim()) return null;
    if (typeof v.kind === "string" && !KINDS.has(v.kind)) return null;
    return {
      name: v.name.trim().slice(0, 60),
      instructions: typeof v.instructions === "string" ? v.instructions : "",
      kind: typeof v.kind === "string" ? v.kind : "daily",
      time: typeof v.time === "string" ? v.time : "08:00",
      days: Array.isArray(v.days) ? v.days.filter((d): d is number => Number.isInteger(d) && d >= 1 && d <= 7) : [],
      every_minutes: typeof v.every_minutes === "number" ? v.every_minutes : 0,
      project: typeof v.project === "string" ? v.project : "",
    };
  } catch {
    return null;
  }
}

/** The Routine the card would save, for its own schedule preview. */
const asRoutine = (s: RoutineSpec): Routine => ({
  id: "",
  name: s.name,
  instructions: s.instructions ?? "",
  kind: (s.kind as Routine["kind"]) || "daily",
  time: s.time || "08:00",
  days: s.days ?? [],
  every_minutes: s.every_minutes ?? 0,
  project: s.project ?? "",
  enabled: true,
  last_run: 0,
  next_run: 0,
  runs: 0,
  utc_offset_minutes: -new Date().getTimezoneOffset(),
  created: 0,
  updated: 0,
});

export function RoutineProposalCard({ spec }: { spec: RoutineSpec }) {
  const ctx = useMessageActions();
  const host = getHost();
  const agent = ctx?.thread.agent ? agentById(ctx.thread.agent) : undefined;
  const [busy, setBusy] = useState(false);
  const [denied, setDenied] = useState(false);
  const already = agent?.routines.some((r) => r.name.trim().toLowerCase() === spec.name.trim().toLowerCase()) ?? false;
  const draft = asRoutine(spec);
  const accept = async () => {
    if (!agent || busy) return;
    setBusy(true);
    try {
      await saveRoutine(agent.id, { ...draft, next_run: 0 });
      host.toast(`Routine added: ${agent.name} wakes itself up for it`);
    } catch (e) {
      host.toast(String(e), "err");
    } finally {
      setBusy(false);
    }
  };
  if (denied) {
    return (
      <div className="my-2 flex items-center gap-1.5 text-[12.5px]/5 text-muted-foreground">
        <IconX size={13} /> Declined &ldquo;{spec.name}&rdquo;
        <button type="button" onClick={() => setDenied(false)} className="ml-1 cursor-pointer underline-offset-2 hover:text-foreground hover:underline">
          Undo
        </button>
      </div>
    );
  }
  const who = agent?.name ?? "The bot";
  const row = "grid grid-cols-[84px_1fr] items-baseline gap-3 text-[13px]/5";
  const k = "text-[11.5px]/5 font-medium tracking-wide text-muted-foreground uppercase";
  return (
    <aside
      aria-label="Routine the bot proposes"
      className={`my-3 flex w-full max-w-lg flex-col overflow-hidden rounded-3xl bg-well ${WELL} shadow-[0_10px_30px_-12px_rgba(0,0,0,0.35)]`}
    >
      <div className="flex items-center gap-2.5 border-b border-border/60 px-5 py-3.5">
        <span className="grid size-8 place-items-center rounded-xl bg-background text-muted-foreground ring-1 ring-black/[0.05] dark:ring-white/[0.07]">
          <IconClock size={16} />
        </span>
        <div className="min-w-0">
          <div className="text-[11.5px]/4 font-medium tracking-wide text-muted-foreground uppercase">{who} proposes a routine</div>
          <div className="truncate text-[15px]/5 font-semibold">{spec.name}</div>
        </div>
      </div>
      <div className="flex flex-col gap-3 px-5 py-4">
        <div className={row}>
          <span className={k}>When</span>
          <span>{routineWhen(draft)}</span>
        </div>
        {spec.instructions?.trim() && (
          <div className={row}>
            <span className={k}>Does</span>
            <span className="text-foreground/90">{spec.instructions.trim()}</span>
          </div>
        )}
        {agent && (
          <div className={row}>
            <span className={k}>Runs as</span>
            <span>
              {agent.name} <span className="text-muted-foreground">on {host.routeName(agent.route)}</span>
            </span>
          </div>
        )}
        <p className="rounded-xl bg-background/60 px-3 py-2 text-[12.5px]/5 text-muted-foreground">
          {agent
            ? `If you accept, ${agent.name} wakes itself at that time, does the job, and the result lands in its chat. You can pause or delete the routine in its Routines tab.`
            : "Only a bot can add a routine: open this in a bot's chat."}
        </p>
      </div>
      {agent && (
        <div className="flex items-center justify-end gap-2 border-t border-border/60 px-5 py-3">
          {already ? (
            <span className="flex h-9 items-center gap-1.5 text-[13px] font-medium text-muted-foreground">
              <IconCheck size={14} /> Accepted
            </span>
          ) : (
            <>
              <button
                type="button"
                onClick={() => setDenied(true)}
                className="flex h-9 cursor-pointer items-center rounded-full px-4 text-[13px] font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                Deny
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() => void accept()}
                className="flex h-9 cursor-pointer items-center gap-1.5 rounded-full bg-primary px-4 text-[13px] font-medium text-primary-foreground transition-[background-color,scale] duration-150 hover:bg-(--primary-hover) active:scale-[0.97] disabled:cursor-default disabled:opacity-50"
              >
                <IconCheck size={14} /> Accept
              </button>
            </>
          )}
        </div>
      )}
    </aside>
  );
}
