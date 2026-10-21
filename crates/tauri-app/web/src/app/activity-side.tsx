// The activity feed, and the sidebar toggle that shows it: what the bots
// recently did (ran a routine, opened or ended a session, took a card) and
// what they are waiting on you for.
//
// The feed is derived (activity.ts), so this file only decides how it looks
// and how an answer travels: an answer is a message in the bot's thread,
// sent the same way the hero prompt sends one.

import { Fragment, useState } from "react";
import { IconActivity, IconCheck, IconClock, IconList, IconMessages, IconSend, IconX } from "@tabler/icons-react";

import { RowPill } from "@/components/row-pill";
import { Avatar, Stack } from "./avatar";
import { CardGlyph } from "./shapeshift";
import { useCards } from "../shapeshift";
import { agentById, chatState, openAgentThread, openBot, openCard, openThread, send as sendLine, useChat } from "../bridge";
import { answerAsk, dayOf, dismissAsk, openAsks, sessionDetail, stamp, useActivity, useAsks, type ActivityRow, type Ask } from "./activity";
import { useSessions, useNow } from "./sessions";

const oneLine = (job: string) => job.split("\n").find((l) => l.trim())?.trim() ?? "";

/** The sidebar's two faces: the lists, or what the bots have been up to. */
export function ActivityToggle({ view, onView }: { view: "bots" | "activity"; onView: (v: "bots" | "activity") => void }) {
  const asks = useAsks();
  const open = openAsks(asks).length;
  const tab = (v: "bots" | "activity") =>
    `flex h-7 flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-[9px] text-[12.5px] font-medium transition-colors ${
      view === v ? "bg-foreground text-(--surface)" : "text-muted-foreground hover:text-foreground"
    }`;
  return (
    <div role="tablist" aria-label="Sidebar" className="flex items-center gap-1 rounded-xl bg-well p-1 shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]">
      <button type="button" role="tab" aria-selected={view === "bots"} className={tab("bots")} onClick={() => onView("bots")} title="Bots, group chats and artifacts">
        <IconList size={14} />
        Bots
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={view === "activity"}
        className={tab("activity")}
        onClick={() => onView("activity")}
        title={open ? `${open} question${open === 1 ? "" : "s"} waiting on you` : "What your bots did, and what they need"}
      >
        <IconActivity size={14} />
        Activity
        {open > 0 && (
          <span className="grid min-w-4 place-items-center rounded-full bg-sky-500 px-1 text-[10px]/4 font-semibold text-white">
            {open}
          </span>
        )}
      </button>
    </div>
  );
}

type Filter = "all" | "did" | "asks";

/** The feed itself: one row per thing that happened, newest first, under the
 *  day it happened on. A row opens what it is about; an ask can be answered
 *  where it sits. */
export function ActivitySide() {
  const { agents } = useChat();
  const sessions = useSessions();
  const cards = useCards();
  const asks = useAsks();
  const rows = useActivity({ agents, sessions, cards });
  const now = useNow(30_000);
  const [filter, setFilter] = useState<Filter>("all");

  const shown = rows.filter((r) => (filter === "all" ? true : filter === "asks" ? r.kind === "ask" : r.kind !== "ask"));
  const open = openAsks(asks).length;
  let day = "";

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-1 px-2.5">
        {(["all", "did", "asks"] as Filter[]).map((f) => (
          <button
            key={f}
            type="button"
            onClick={() => setFilter(f)}
            aria-pressed={filter === f}
            className={`cursor-pointer rounded-full px-2.5 py-1 text-[12px] font-medium transition-colors ${
              filter === f ? "bg-foreground text-(--surface)" : "text-muted-foreground hover:bg-accent hover:text-foreground"
            }`}
          >
            {f === "all" ? "All" : f === "did" ? "Did" : "Asks"}
            {f === "asks" && open > 0 && <span className="ml-1 text-sky-500">{open}</span>}
          </button>
        ))}
        <span className="ml-auto text-[11px]/4 text-muted-foreground/70">
          {rows.length} event{rows.length === 1 ? "" : "s"}
        </span>
      </div>

      {shown.length === 0 && (
        <div className="px-2.5 py-1.5 text-[12.5px]/5 text-muted-foreground">
          {filter === "asks"
            ? "No open questions. When a bot needs you, it asks here — and the answer goes straight into its thread."
            : filter === "did"
              ? "Nothing yet. A routine that runs, a session that opens or a card a bot takes shows up here."
              : "Nothing yet. Bots post here when they wake up, run a routine, or need something from you."}
        </div>
      )}

      {shown.map((r) => {
        // The feed is sorted by time, so days run today → yesterday → earlier
        // and a row that changes the day also carries its header.
        const when = dayOf(r.at, now);
        const fresh = when !== day;
        if (fresh) day = when;
        const row = r.kind === "ask" ? <AskCard ask={r.ask} now={now} /> : <DidRow row={r} now={now} />;
        return (
          <Fragment key={`${r.kind}-${r.kind === "ask" ? r.ask.id : r.kind === "session" ? r.session.threadId : r.kind === "card" ? r.card.id : `${r.agent.id}/${r.routine.id}`}`}>
            {fresh && <div className="flex h-5 items-center px-2.5 text-[10.5px]/4 font-medium text-muted-foreground/55">{when}</div>}
            {row}
          </Fragment>
        );
      })}
    </div>
  );
}

/** A thing that happened: a row, opened by clicking it. */
function DidRow({ row, now }: { row: Exclude<ActivityRow, { kind: "ask" }>; now: number }) {
  const open = () => {
    if (row.kind === "session") return void openThread(row.session.threadId);
    if (row.kind === "card") return openCard(row.card.id);
    return openBot(row.agent.id);
  };
  const who = row.agentIds.map((id) => agentById(id)?.name).filter(Boolean);
  const [title, sub] =
    row.kind === "routine"
      ? [`${row.agent.name} ran ${row.routine.name || "a routine"}`, oneLine(row.routine.instructions) || `${row.routine.runs} run${row.routine.runs === 1 ? "" : "s"}`]
      : row.kind === "session"
        ? [row.session.title || "A session", `with ${who.join(", ") || "a bot"} · ${sessionDetail(row.session, now)}`]
        : [row.card.title, `${who[0] ?? "a bot"} · ${row.card.done ? "done" : row.card.detail}`];
  return (
    <button type="button" onClick={open} className="group/row relative flex w-full cursor-pointer items-center gap-2.5 rounded-lg px-2 py-1.5 text-left" title={`${title} — ${sub}`}>
      <RowPill />
      <span className="relative grid size-7 shrink-0 place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
        {row.kind === "routine" ? <IconClock size={14} /> : row.kind === "session" ? <IconMessages size={14} /> : <CardGlyph kind={row.card.kind} size={14} />}
      </span>
      <span className="relative flex min-w-0 flex-1 flex-col">
        <span className="truncate text-[13px]/4 font-medium text-foreground">{title}</span>
        <span className="truncate text-[11.5px]/4 text-muted-foreground">{sub}</span>
      </span>
      <span className="relative shrink-0 text-[11px]/4 text-muted-foreground/70">{stamp(row.at, now)}</span>
    </button>
  );
}

/** A question the bot is waiting on: tinted, so it reads as owed, and
 *  answerable without leaving the feed. The home's "Agents need this from
 *  you" section renders the same card, one level up. */
export function AskCard({ ask, now }: { ask: Ask; now: number }) {
  const [draft, setDraft] = useState("");
  const [writing, setWriting] = useState(false);
  const first = agentById(ask.agentIds[0]);
  const many = ask.agentIds.length > 1;
  const answered = ask.state !== "open";

  const send = async (text: string) => {
    const line = text.trim();
    if (!line || answered) return;
    answerAsk(ask.id, line);
    setDraft("");
    setWriting(false);
    try {
      if (ask.threadId) await openThread(ask.threadId);
      else if (first) await openAgentThread(first, null);
      const t = chatState().thread;
      // Through send(): the answer is a message, and a message moves the
      // session it lands in.
      if (t) await sendLine(line, [], t.route, false);
    } catch {
      /* the ask is answered either way; the thread can be opened by hand */
    }
  };

  return (
    <div className="mx-0.5 flex flex-col gap-1.5 rounded-xl bg-sky-500/[0.07] px-2 py-2 ring-1 ring-sky-500/25">
      <div className="flex items-center gap-2">
        {many ? <Stack ids={ask.agentIds} size={22} /> : first ? <Avatar agent={first} size={22} /> : null}
        <button type="button" onClick={() => first && openBot(first.id)} className="min-w-0 flex-1 cursor-pointer truncate text-left text-[11.5px]/4 font-medium text-muted-foreground hover:text-foreground">
          {many ? `${ask.agentIds.length} bots need you` : `${first?.name ?? "A bot"} needs you`}
        </button>
        <span className="shrink-0 text-[11px]/4 text-muted-foreground/70">{stamp(ask.at, now)}</span>
      </div>
      <p className={`text-[12.5px]/5 ${answered ? "text-muted-foreground" : "text-foreground"}`}>{ask.text}</p>
      {ask.detail && !answered && <p className="text-[11.5px]/4 text-muted-foreground">{ask.detail}</p>}
      {answered ? (
        <p className="flex items-center gap-1.5 text-[11.5px]/4 text-muted-foreground">
          <IconCheck size={12} className="shrink-0 text-sky-500" />
          You: {ask.answer}
        </p>
      ) : (
        <>
          {ask.choices && ask.choices.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {ask.choices.map((c) => (
                <button key={c} type="button" onClick={() => void send(c)} className="cursor-pointer rounded-full bg-foreground/[0.07] px-2.5 py-1 text-[11.5px] font-medium hover:bg-foreground/[0.12]">
                  {c}
                </button>
              ))}
            </div>
          )}
          {writing ? (
            <div className="flex items-end gap-1.5">
              <textarea
                autoFocus
                rows={1}
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    void send(draft);
                  }
                  if (e.key === "Escape") setWriting(false);
                }}
                placeholder={`Answer ${first?.name ?? "them"}…`}
                aria-label="Answer"
                className="min-h-8 flex-1 resize-none rounded-lg bg-well px-2.5 py-1.5 text-[12.5px]/5 outline-none shadow-[inset_0_0_0_1px_var(--well-outline)]"
              />
              <button type="button" onClick={() => void send(draft)} disabled={!draft.trim()} aria-label="Send the answer" className="grid size-8 shrink-0 cursor-pointer place-items-center rounded-full bg-foreground text-(--surface) disabled:cursor-default disabled:opacity-40">
                <IconSend size={14} />
              </button>
            </div>
          ) : (
            <div className="flex items-center gap-1">
              <button type="button" onClick={() => setWriting(true)} className="cursor-pointer rounded-full bg-foreground/[0.07] px-2.5 py-1 text-[11.5px] font-medium hover:bg-foreground/[0.12]">
                Answer
              </button>
              <button
                type="button"
                onClick={() => dismissAsk(ask.id)}
                title="Hide it — it stays in the bot's thread"
                aria-label={`Dismiss ${first?.name ?? "the"} question`}
                className="grid size-6 cursor-pointer place-items-center rounded-full text-muted-foreground hover:bg-foreground/[0.07] hover:text-foreground"
              >
                <IconX size={13} />
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
