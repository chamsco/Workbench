// Shapeshift's face: the composer strip that becomes a card as you type, the
// sidebar section and home row that list saved cards, and the sheet that
// turns one into a bot's routine or a message in a thread.
//
// The fusion rule, in three lines: the input is the composer you already
// have (a mode, not a new box), the list is one more sidebar section beside
// Bots and Chats (not a new zone), and a card's whole purpose is to become
// something that already exists — a routine, a message, a memory note. No
// card ever lives only as a card.

import { useEffect, useMemo, useState } from "react";
import {
  IconArrowsExchange,
  IconAt,
  IconBell,
  IconCalculator,
  IconCalendarEvent,
  IconChartBar,
  IconCheck,
  IconChecklist,
  IconClock,
  IconHourglassHigh,
  IconMapPin,
  IconNotes,
  IconReceipt2,
  IconSend,
  IconSparkles,
  IconTrash,
  IconUsers,
  IconX,
} from "@tabler/icons-react";

import { RowPill } from "@/components/row-pill";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Avatar } from "./avatar";
import { agentById, chatState, editRoutine, getHost, openArtifact, openCard, saveRoutine, sendTo, useChat, type Agent, type Routine, type ThreadInfo } from "../bridge";
import {
  KIND_LABEL,
  addCard,
  buildCard,
  cardLine,
  cardTime,
  calmStart,
  calmStep,
  classify,
  money,
  newId,
  removeCard,
  setDone,
  toggleItem,
  useCards,
  type Card,
  type CardKind,
} from "../shapeshift";

// Copied, not imported: the well is one class string, and the two bots-ui
// files already copy it from each other for exactly this reason.
const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";

export function CardGlyph({ kind, size = 16 }: { kind: CardKind; size?: number }) {
  const props = { size } as const;
  switch (kind) {
    case "event": return <IconCalendarEvent {...props} />;
    case "reminder": return <IconBell {...props} />;
    case "checklist": return <IconChecklist {...props} />;
    case "timer": return <IconClock {...props} />;
    case "split": return <IconUsers {...props} />;
    case "expense": return <IconReceipt2 {...props} />;
    case "convert": return <IconArrowsExchange {...props} />;
    case "calc": return <IconCalculator {...props} />;
    case "poll": return <IconChartBar {...props} />;
    case "countdown": return <IconHourglassHigh {...props} />;
    case "note": return <IconNotes {...props} />;
  }
}

// ---------------------------------------------------------------- preview

/** One example per kind, for the palette the strip shows when it is empty. */
const PALETTE: [CardKind, string][] = [
  ["event", "Dinner with Priya Friday 8pm"],
  ["checklist", "Buy milk, eggs and bread"],
  ["timer", "25 min focus"],
  ["split", "Split 2400 between 3"],
  ["expense", "Spent 450 on uber"],
  ["convert", "5 miles in km"],
  ["calc", "18% of 3450"],
  ["poll", "Pizza or burgers for Friday?"],
  ["countdown", "Days until Christmas"],
  ["reminder", "Remind me to pay rent tomorrow"],
];

/**
 * The strip above the composer, in everyday mode: it classifies the draft,
 * runs the calm-UI rule so a half-typed line cannot flicker the card, and
 * shows the card Enter would save. Empty text shows the palette instead —
 * every card type, one tap from a real example.
 */
export function ShapeShiftPreview({
  text,
  onExit,
  onSample,
}: {
  text: string;
  onExit: () => void;
  onSample: (t: string) => void;
}) {
  const [calm, setCalm] = useState(calmStart);
  const ranked = useMemo(() => classify(text), [text]);
  useEffect(() => setCalm((c) => calmStep(c, ranked)), [ranked]);
  const card = useMemo(() => (text.trim() ? buildCard(text, calm.card) : null), [text, calm.card]);
  const hint = (
    <span className="hidden shrink-0 items-center text-[11px]/4 text-muted-foreground sm:flex">
      Enter saves · Esc exits
    </span>
  );
  const exit = (
    <button
      type="button"
      onClick={onExit}
      aria-label="Leave everyday mode"
      className="grid size-7 shrink-0 cursor-pointer place-items-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
    >
      <IconX size={14} />
    </button>
  );
  return (
    <div className={`mb-2 flex flex-col gap-2 rounded-2xl bg-well px-4 py-3 ${WELL}`}>
      {card ? (
        <div className="flex items-center gap-3">
          <span className="grid size-8 shrink-0 place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
            <CardGlyph kind={card.kind} />
          </span>
          <span className="flex min-w-0 flex-1 flex-col">
            <span className="flex min-w-0 items-center gap-2">
              <span className="truncate text-[13.5px]/5 font-medium text-foreground">{card.title}</span>
              <span className="shrink-0 rounded-full bg-foreground/[0.06] px-2 py-0.5 text-[10.5px] font-medium text-muted-foreground">
                {KIND_LABEL[card.kind]}
              </span>
            </span>
            <span className="truncate text-[11.5px]/4 text-muted-foreground">{card.detail}</span>
          </span>
          {hint}
          {exit}
        </div>
      ) : (
        <div className="flex flex-col gap-2.5">
          <div className="flex items-center gap-2 text-[12px]/5 text-muted-foreground">
            <IconSparkles size={14} className="shrink-0" />
            <span className="min-w-0 flex-1">
              Type an everyday thing and it becomes a card — give one to a bot and it wakes itself up for it.
            </span>
            {hint}
            {exit}
          </div>
          <div className="flex flex-wrap gap-1.5">
            {PALETTE.map(([kind, sample]) => (
              <button
                key={kind}
                type="button"
                onClick={() => onSample(sample)}
                className="flex h-7 cursor-pointer items-center gap-1.5 rounded-full bg-foreground/[0.05] px-2.5 text-[11.5px] font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                <CardGlyph kind={kind} size={12} />
                {KIND_LABEL[kind]}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

/** Save what the composer holds as a card. Called by the composer's Enter in
 *  everyday mode and by its save pill; the mode stays on for the next one. */
export function saveEveryday(text: string, forAgent?: string) {
  const t = text.trim();
  if (!t) return;
  const top = classify(t)[0];
  const card = buildCard(t, top?.kind ?? "note");
  const id = newId();
  addCard({ ...card, id, ...(forAgent ? { for: forAgent } : null) });
  getHost().toast(`Saved — ${card.title}`, "ok");
  // A card saved from the home is a card that exists: open its artifact, the
  // web view it becomes. Inside a thread it only saves — the conversation you
  // are in stays where it is.
  if (!chatState().id) openArtifact({ kind: "card", id });
}

// ---------------------------------------------------------------- home

/** The home's cards, as an agenda: what is coming and when, one line each —
 *  the shape a reminders list has. Timed cards lead, soonest first; the ones
 *  without a clock sit under them as "anytime". A row opens the card's sheet,
 *  where it can be ticked, marked done or given to a bot. */
export function CardsAgenda() {
  const cards = useCards();
  const open = cards.filter((c) => !c.done);
  // A card's clock, when it has one: an event or a reminder fires at `when`,
  // a countdown runs to `until`; everything else has no clock.
  const at = (c: Card) => (c.kind === "event" || c.kind === "reminder" ? c.data.when : c.kind === "countdown" ? c.data.until : 0);
  const rows = [
    ...open.filter((c) => at(c) > 0).sort((a, b) => at(a) - at(b)),
    ...open.filter((c) => at(c) === 0).sort((a, b) => b.at - a.at),
  ].slice(0, 8);
  return (
    <section className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between">
        <h2 className="text-[14px] font-semibold">Agenda</h2>
        <span className="text-[11.5px] text-muted-foreground">＋ → Everyday task in the composer</span>
      </div>
      {rows.length === 0 ? (
        <div className="px-1 text-[12.5px]/5 text-muted-foreground">
          Nothing on it yet. Type an everyday thing — “buy milk, eggs and bread”, “dinner with Priya Friday 8pm”, “split 2400 between 3” — and it lands here.
        </div>
      ) : (
        <div className="flex flex-col">
          {rows.map((c) => (
            <button
              key={c.id}
              type="button"
              onClick={() => openCard(c.id)}
              className="group/row relative flex w-full cursor-pointer items-center gap-3 rounded-xl px-2 py-1.5 text-left"
              title={c.text}
            >
              <RowPill />
              <span className="relative w-[104px] shrink-0 truncate text-[12px] font-medium tabular-nums text-muted-foreground">{agendaWhen(c)}</span>
              <span className="relative grid size-7 shrink-0 place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
                <CardGlyph kind={c.kind} size={14} />
              </span>
              <span className="relative flex min-w-0 flex-1 flex-col">
                <span className="truncate text-[13.5px] font-medium">{c.title}</span>
                <span className="truncate text-[11.5px] text-muted-foreground">{agendaSub(c)}</span>
              </span>
            </button>
          ))}
        </div>
      )}
    </section>
  );
}

/** The agenda's left column: "Today 20:00", "Tomorrow 09:00", "Wed 20:00" —
 *  or "Anytime" for a card with no clock. */
function agendaWhen(c: Card): string {
  const at = c.kind === "event" || c.kind === "reminder" ? c.data.when : c.kind === "countdown" ? c.data.until : 0;
  if (!at) return "Anytime";
  const d = new Date(at);
  const midnight = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((midnight(d) - midnight(new Date())) / 86400e3);
  const time = d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  if (days === 0) return `Today ${time}`;
  if (days === 1) return `Tomorrow ${time}`;
  if (days === -1) return `Yesterday ${time}`;
  if (days > 1 && days < 7) return `${d.toLocaleDateString([], { weekday: "short" })} ${time}`;
  return `${d.toLocaleDateString([], { month: "short", day: "numeric" })} ${time}`;
}

/** The line under the title: the useful half of the card. A timed card's
 *  detail is its own clock, and the left column already says that — so for an
 *  event this line is who and where, for a reminder just the bot it is for. */
function agendaSub(c: Card): string {
  const who = c.for ? agentById(c.for)?.name : undefined;
  if (c.kind === "event") return [c.data.who && `with ${c.data.who}`, c.data.where && `on ${c.data.where}`, who].filter(Boolean).join(" · ");
  if (c.kind === "reminder" || c.kind === "countdown") return who ?? "";
  return [who, c.detail].filter(Boolean).join(" · ");
}

// ---------------------------------------------------------------- sheet

/** Store-driven, like RoutineDialog: it renders while its field is set. */
export function CardSheet() {
  const { cardOpen } = useChat();
  const card = useCards().find((c) => c.id === cardOpen) ?? null;
  if (!cardOpen || !card) return null;
  return <CardSheetBody key={card.id} card={card} />;
}

function CardSheetBody({ card }: { card: Card }) {
  const host = getHost();
  const { agents, threads } = useChat();
  const [giving, setGiving] = useState(false);
  const [sending, setSending] = useState(false);
  const [confirmDel, setConfirmDel] = useState(false);
  const [busy, setBusy] = useState(false);

  const give = async (agent: Agent) => {
    setBusy(true);
    try {
      const routine: Routine = {
        id: "",
        name: card.title.slice(0, 60),
        instructions: cardLine(card),
        kind: "daily",
        time: cardTime(card),
        days: [],
        every_minutes: 0,
        project: "",
        enabled: true,
        last_run: 0,
        next_run: 0,
        runs: 0,
        utc_offset_minutes: -new Date().getTimezoneOffset(),
        created: 0,
        updated: 0,
      };
      // Straight into the routine's own sheet afterwards: the card chose a
      // sensible schedule, the three steps let you correct it.
      const saved = await saveRoutine(agent.id, routine);
      const r = saved.routines.find((x) => x.name === routine.name);
      if (r) editRoutine(agent.id, r);
      setGiving(false);
      openCard(null);
    } catch (e) {
      host.toast(String(e), "err");
    } finally {
      setBusy(false);
    }
  };

  const sendToThread = async (thread: ThreadInfo) => {
    setBusy(true);
    try {
      await sendTo(thread, cardLine(card));
      setSending(false);
      openCard(null);
      host.toast("Sent", "ok");
    } catch (e) {
      host.toast(String(e), "err");
    } finally {
      setBusy(false);
    }
  };

  const remember = async () => {
    try {
      await host.remember(card.text);
      host.toast("Will remember that", "ok");
    } catch (e) {
      host.toast(String(e), "err");
    }
  };

  const botThreads = threads.filter((t) => !!t.agent || (t.members?.length ?? 0) > 0);
  // A card written for a bot says so at the top — its page lists it, and the
  // sheet shows where it was headed.
  const forBot = card.for ? agents.find((x) => x.id === card.for) ?? null : null;
  const pill =
    "flex h-8 cursor-pointer items-center gap-1.5 rounded-full px-3 text-[12.5px] font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:cursor-default disabled:opacity-50";

  return (
    <Dialog open onOpenChange={(o) => !o && !busy && openCard(null)}>
      <DialogContent className="top-[12vh] max-h-[80vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <div className="flex items-baseline justify-between gap-2">
            <DialogTitle>{card.title}</DialogTitle>
            <span className="shrink-0 text-[11.5px] font-medium text-muted-foreground">{KIND_LABEL[card.kind]}</span>
          </div>
          <DialogDescription>{card.text}</DialogDescription>
          {forBot && (
            <div className="mt-1 flex items-center gap-1.5 text-[12px] text-muted-foreground">
              <Avatar agent={forBot} size={18} />
              for {forBot.name}
            </div>
          )}
        </DialogHeader>

        <div className="mt-3 flex flex-col gap-3">
          <div className={`flex items-start gap-3 rounded-2xl bg-well px-4 py-3.5 ${WELL}`}>
            <span className="grid size-9 shrink-0 place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
              <CardGlyph kind={card.kind} size={18} />
            </span>
            <CardBody card={card} />
          </div>

          {giving && (
            <div className="flex max-h-56 flex-col gap-0.5 overflow-y-auto">
              {agents.length === 0 && <div className="px-1 text-[12.5px] text-muted-foreground">No bots yet — make one first.</div>}
              {agents.map((a) => (
                <button
                  key={a.id}
                  type="button"
                  disabled={busy}
                  onClick={() => void give(a)}
                  className="flex cursor-pointer items-center gap-2.5 rounded-xl px-2.5 py-2 text-left hover:bg-accent disabled:opacity-50"
                >
                  <Avatar agent={a} size={26} />
                  <span className="flex min-w-0 flex-col">
                    <span className="truncate text-[13.5px] font-medium">{a.name}</span>
                    <span className="truncate text-[11.5px] text-muted-foreground">
                      Wakes itself up for it at {cardTime(card)} — you can correct the schedule after
                    </span>
                  </span>
                </button>
              ))}
            </div>
          )}

          {sending && (
            <div className="flex max-h-56 flex-col gap-0.5 overflow-y-auto">
              {botThreads.length === 0 && <div className="px-1 text-[12.5px] text-muted-foreground">No bot chats yet.</div>}
              {botThreads.map((t) => {
                const a = agents.find((x) => x.id === t.agent);
                return (
                  <button
                    key={t.id}
                    type="button"
                    disabled={busy}
                    onClick={() => void sendToThread(t)}
                    className="flex cursor-pointer items-center gap-2.5 rounded-xl px-2.5 py-2 text-left hover:bg-accent disabled:opacity-50"
                  >
                    {a ? <Avatar agent={a} size={26} /> : <span className="grid size-6 place-items-center rounded-full bg-foreground/[0.06]"><IconUsers size={13} /></span>}
                    <span className="flex min-w-0 flex-col">
                      <span className="truncate text-[13.5px] font-medium">{a?.name ?? t.title}</span>
                      <span className="truncate text-[11.5px] text-muted-foreground">{t.preview || "No messages yet"}</span>
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        <DialogFooter className="justify-between">
          <div className="flex gap-1">
            <Button variant="ghost" className="text-red-500" disabled={busy}
              onClick={() => (confirmDel ? (removeCard(card.id), openCard(null)) : setConfirmDel(true))}>
              <IconTrash size={14} /> {confirmDel ? "Delete?" : "Delete"}
            </Button>
          </div>
          <div className="flex flex-wrap justify-end gap-1.5">
            <button type="button" className={pill} disabled={busy} onClick={() => void remember()}>
              <IconNotes size={14} /> Remember
            </button>
            <button type="button" className={pill} disabled={busy || botThreads.length === 0} onClick={() => { setSending((s) => !s); setGiving(false); }}>
              <IconSend size={14} /> Send to a chat
            </button>
            <button type="button" className={pill} disabled={busy || agents.length === 0} onClick={() => { setGiving((g) => !g); setSending(false); }}>
              <IconUsers size={14} /> Give to a bot
            </button>
            <Button disabled={busy} onClick={() => { setDone(card.id, !card.done); openCard(null); }}>
              {card.done ? "Reopen" : "Mark done"}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** The card itself, big: the computed answer is the point. */
function CardBody({ card }: { card: Card }) {
  const d = (
    <div className="flex min-w-0 flex-1 flex-col gap-1">
      <span className="text-[13.5px]/5 font-medium text-foreground">{card.title}</span>
      <span className="text-[12px]/5 text-muted-foreground">{card.detail}</span>
    </div>
  );
  switch (card.kind) {
    case "event": {
      const at = new Date(card.data.when).toLocaleString([], { weekday: "long", hour: "numeric", minute: "2-digit" });
      return (
        <>
          {d}
          <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[12px] text-muted-foreground">
            <span>{at}</span>
            {card.data.who && <span className="flex items-center gap-1"><IconAt size={12} /> {card.data.who}</span>}
            {card.data.where && <span className="flex items-center gap-1"><IconMapPin size={12} /> {card.data.where}</span>}
          </div>
        </>
      );
    }
    case "checklist":
      return (
        <>
          {d}
          <div className="flex flex-col gap-0.5">
            {card.data.items.map((item, i) => {
              const on = card.data.done.includes(i);
              return (
                <button
                  key={`${item}-${i}`}
                  type="button"
                  onClick={() => toggleItem(card.id, i)}
                  className="flex cursor-pointer items-center gap-2 rounded-lg px-1 py-1 text-left text-[12.5px] hover:bg-accent"
                >
                  <span className={`grid size-4 shrink-0 place-items-center rounded-[5px] border ${on ? "border-foreground bg-foreground text-(--surface)" : "border-foreground/30"}`}>
                    {on && <IconCheck size={11} strokeWidth={3} />}
                  </span>
                  <span className={on ? "text-muted-foreground line-through" : ""}>{item}</span>
                </button>
              );
            })}
          </div>
        </>
      );
    case "split":
      return (
        <>
          {d}
          <div className="text-[12px] text-muted-foreground">
            {money(card.data.amount / card.data.people)} each — {money(card.data.amount)} over {card.data.people}
          </div>
        </>
      );
    case "expense":
      return (
        <>
          {d}
          <div className="text-[12px] text-muted-foreground">{money(card.data.amount)}</div>
        </>
      );
    case "convert":
      return (
        <>
          {d}
          <div className="text-[12px] text-muted-foreground">
            {card.data.value} {card.data.from} = {card.data.result} {card.data.to}
          </div>
        </>
      );
    case "calc":
      return (
        <>
          {d}
          <div className="text-[12px] text-muted-foreground">
            {card.data.expr} = {card.data.result}
          </div>
        </>
      );
    case "poll":
      return (
        <>
          {d}
          <div className="flex flex-wrap gap-1.5">
            {card.data.options.map((o) => (
              <span key={o} className="rounded-full bg-foreground/[0.06] px-2.5 py-1 text-[12px] font-medium text-muted-foreground">
                {o}
              </span>
            ))}
          </div>
        </>
      );
    default:
      return d;
  }
}
