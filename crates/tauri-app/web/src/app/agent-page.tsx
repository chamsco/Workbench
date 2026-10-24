// A bot's own page: what the sidebar's "Bots" section drills into. The
// screenshot puts the prompt first; this is the other half — one bot, seen
// whole: the sessions you have had with it, the cards written for it, its
// routines on a clock, and the projects it works in.
//
// Clicking a bot anywhere (sidebar, strip, card) lands here. Message still
// opens the thread; this page is the place you look at what a bot *is*, the
// way the project page is the place you look at a folder.

import { useMemo, useRef, useState, type ReactNode } from "react";
import {
  IconArrowLeft,
  IconClock,
  IconFolder,
  IconMessageCircle,
  IconNotes,
  IconPencil,
  IconPlus,
} from "@tabler/icons-react";

import { RowPill } from "@/components/row-pill";
import { Avatar } from "./avatar";
import {
  agentStatus,
  chatState,
  closeBot,
  editAgent,
  editRoutine,
  getHost,
  nextRoutine,
  nextWakeLabel,
  openAgentThread,
  openCard,
  openProject,
  openThread,
  send,
  useChat,
} from "../bridge";
import { CardGlyph, saveEveryday } from "./shapeshift";
import { useCards } from "../shapeshift";
import {
  concludeSession,
  endSession,
  liveState,
  renameSession,
  scrollToSession,
  sessionLabel,
  touchSession,
  useNow,
  useSessions,
  type Session,
} from "./sessions";

const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";

export function AgentPage() {
  const { agents, threads, projects, botPage } = useChat();
  const host = getHost();
  const agent = agents.find((a) => a.id === botPage) ?? null;
  const sessions = useSessions();
  const now = useNow(1000);
  const status = agent ? agentStatus(agent, threads.find((t) => t.agent === agent.id && t.busy) ?? threads.find((t) => t.agent === agent.id) ?? null) : null;
  // Hooks before the early return: on "All bots" the page renders once more
  // with no bot while it animates out, and must call the same hooks.
  const [line, setLine] = useState("");
  const [card, setCard] = useState("");
  const taRef = useRef<HTMLTextAreaElement>(null);
  if (!agent) return null;
  // Every sitting this bot has had, newest first, across all of its threads.
  const mine = sessions.filter((s) => s.agentIds.includes(agent.id)).sort((a, b) => b.startedAt - a.startedAt);
  const previews = new Map(threads.map((t) => [t.id, t.preview ?? ""]));
  const inProjects = projects.filter((p) => p.agents.includes(agent.id));
  const next = nextRoutine(agent);

  const say = async () => {
    const text = line.trim();
    if (!text) return;
    setLine("");
    await openAgentThread(agent, null);
    const t = chatState().thread;
    // Through send(), not sendTo: this is a message like any other, and the
    // session rules (a goodbye ends it, a new subject starts a new one) apply
    // here exactly as they do in the composer.
    if (t) await send(text, [], t.route, false);
  };

  return (
    <div className="h-full overflow-y-auto px-6 pt-8 pb-16">
      <div className="mx-auto flex max-w-3xl flex-col gap-6">
        <button type="button" onClick={closeBot} className="flex w-fit cursor-pointer items-center gap-1.5 text-[12.5px] font-medium text-muted-foreground hover:text-foreground">
          <IconArrowLeft size={14} />
          All bots
        </button>

        <div className="flex items-start gap-4">
          <Avatar agent={agent} size={56} state={status?.kind === "wake" ? "wake" : status?.kind ?? "idle"} />
          <div className="flex min-w-0 flex-1 flex-col">
            <h1 className="text-[24px]/8 font-medium tracking-tight">{agent.name}</h1>
            <p className="mt-1 max-w-[60ch] text-[13px]/5 text-muted-foreground">{agent.job.split("\n")[0] || "No job yet."}</p>
            <p className="mt-1 text-[12px]/4 text-muted-foreground">
              {status ? `${status.label} · ` : ""}
              {host.routeName(agent.route)}
              {next ? ` · wakes ${nextWakeLabel(next.next_run)} for "${next.name}"` : " · no routines yet"}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            <button type="button" onClick={() => void openAgentThread(agent, null)} className="flex cursor-pointer items-center gap-1.5 rounded-full bg-primary px-3.5 py-2 text-[13px] font-medium text-primary-foreground hover:bg-(--primary-hover)">
              <IconMessageCircle size={15} />
              Message
            </button>
            <button type="button" onClick={() => editAgent(agent)} className="flex cursor-pointer items-center gap-1.5 rounded-full px-3.5 py-2 text-[13px] font-medium hover:bg-accent">
              <IconPencil size={15} />
              Edit
            </button>
          </div>
        </div>

        {/* Say one line: it opens (or resumes) the session, iMessage-style —
            the conversation is one long thread, the session is how we talk
            about it. */}
        <div className={`rounded-[22px] bg-well ${WELL}`}>
          <textarea
            ref={taRef}
            value={line}
            rows={1}
            onChange={(e) => setLine(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                void say();
              }
            }}
            placeholder={`Tell ${agent.name} something — it opens a session…`}
            aria-label={`Message ${agent.name}`}
            className="block w-full resize-none bg-transparent px-4 py-3 text-[13.5px]/5 outline-none placeholder:text-muted-foreground"
          />
        </div>

        <Section title="Sessions" note="One thread, one conversation — a session is a sitting in it. It ends when the talking stops or you say that's all; a new subject starts a new one. Click one to scroll back to where it began.">
          {mine.length === 0 && <Empty>No sessions yet. Say the first line above.</Empty>}
          {mine.map((s) => (
            <SessionRow key={s.id} session={s} preview={previews.get(s.threadId) ?? ""} />
          ))}
        </Section>

        <Section title="Cards" note="Everyday things written for this bot — the same cards the ＋ menu makes, tagged with its name.">
          <CardsFor agentId={agent.id} agentName={agent.name} />
          <div className="flex items-center gap-2 px-3.5 py-2.5">
            <span className="grid size-8 shrink-0 place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
              <IconNotes size={15} />
            </span>
            <input
              value={card}
              onChange={(e) => setCard(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && card.trim()) {
                  saveEveryday(card, agent.id);
                  setCard("");
                }
              }}
              placeholder={`An everyday thing for ${agent.name} — buy oats, remind about rent…`}
              aria-label={`A card for ${agent.name}`}
              className="h-8 min-w-0 flex-1 bg-transparent text-[13px] outline-none placeholder:text-muted-foreground"
            />
          </div>
        </Section>

        <Section title="Routines" note="Its job on a clock. It wakes itself up, does it where it lives, and says in one line what it did.">
          {agent.routines.length === 0 && <Empty>No routines yet. Add one and {agent.name} will wake itself up.</Empty>}
          {agent.routines.map((r) => (
            <button
              key={r.id}
              type="button"
              onClick={() => editRoutine(agent.id, r)}
              className="flex min-h-14 w-full cursor-pointer items-center gap-3 px-3.5 py-2.5 text-left hover:bg-accent"
            >
              <span className="grid size-8 shrink-0 place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
                <IconClock size={15} />
              </span>
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate text-[13.5px] font-medium">{r.name || "Untitled routine"}</span>
                <span className="truncate text-[12px] text-muted-foreground">
                  {r.enabled ? (r.next_run ? `next wake ${nextWakeLabel(r.next_run)}` : "enabled") : "paused"}
                </span>
              </span>
              <IconPencil size={14} className="shrink-0 text-muted-foreground" />
            </button>
          ))}
          <button type="button" onClick={() => editRoutine(agent.id)} className="flex min-h-12 w-full cursor-pointer items-center gap-3 px-3.5 text-left text-[13.5px] font-medium hover:bg-accent">
            <span className="grid size-8 place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
              <IconPlus size={16} />
            </span>
            Add routine
          </button>
        </Section>

        <Section title="Projects" note="Folders it works in. A chat started there runs in that folder.">
          {inProjects.length === 0 && <Empty>Not on any project yet.</Empty>}
          <div className="flex flex-wrap gap-1.5 p-3.5">
            {inProjects.map((p) => (
              <button key={p.id} type="button" onClick={() => openProject(p)} className="flex cursor-pointer items-center gap-1.5 rounded-full bg-foreground/[0.06] px-3 py-1.5 text-[12.5px] font-medium hover:bg-accent">
                <IconFolder size={13} />
                {p.name}
              </button>
            ))}
          </div>
        </Section>
      </div>
    </div>
  );
}

function Section({ title, note, children }: { title: string; note: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-1.5">
      <h2 className="text-[14px] font-semibold">{title}</h2>
      <div className={`flex flex-col divide-y divide-(--well-outline) overflow-hidden rounded-2xl bg-well ${WELL}`}>{children}</div>
      <p className="px-1 text-[11.5px]/4 text-muted-foreground">{note}</p>
    </section>
  );
}

const Empty = ({ children }: { children: ReactNode }) => (
  <div className="px-3.5 py-3 text-[12.5px]/5 text-muted-foreground">{children}</div>
);

/** One sitting: its title, how it is doing, and the word that ends or resumes
 *  it. Clicking it opens the thread and scrolls back up to where the sitting
 *  began — the conversation is one long thread, the session is a place in it.
 *  The title is the face's until the bot writes one — the seam is
 *  renameSession, which the core's wrap-up can call. */
function SessionRow({ session: s, preview }: { session: Session; preview: string }) {
  const now = useNow(1000);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(s.title);
  const live = liveState(s, now);
  const st = sessionLabel(s, now);
  const open = async () => {
    await openThread(s.threadId);
    // The thread is on screen now; let it paint, then glide to the marker.
    requestAnimationFrame(() => window.setTimeout(() => scrollToSession(s.id), 60));
  };
  return (
    <div className="group/row relative flex min-h-14 items-center gap-3 rounded-lg px-3.5 py-2.5">
      <RowPill />
      <button type="button" onClick={() => void open()} className="relative flex min-w-0 flex-1 cursor-pointer flex-col text-left" title={`${s.title} — scroll back to where this session began`}>
        <span className="relative truncate text-[13.5px] font-medium">{s.title}</span>
        <span className="relative truncate text-[12px] text-muted-foreground">{preview || st}</span>
      </button>
      <span className="relative shrink-0 text-[11.5px] text-muted-foreground">{live === "concluding" ? "wrapping up" : live === "idle" ? "idle" : st}</span>
      {live === "awake" && (
        <button type="button" onClick={() => concludeSession(s.id)} className="relative shrink-0 cursor-pointer rounded-full px-2 py-1 text-[11.5px] font-medium text-muted-foreground hover:bg-accent hover:text-foreground">
          End
        </button>
      )}
      {live === "concluding" && (
        <button type="button" onClick={() => endSession(s.id)} className="relative shrink-0 cursor-pointer rounded-full px-2 py-1 text-[11.5px] font-medium text-muted-foreground hover:bg-accent hover:text-foreground">
          End now
        </button>
      )}
      {live === "idle" && (
        <button type="button" onClick={() => touchSession(s.id)} className="relative shrink-0 cursor-pointer rounded-full px-2 py-1 text-[11.5px] font-medium text-muted-foreground hover:bg-accent hover:text-foreground">
          Reopen
        </button>
      )}
      {editing ? (
        <input
          autoFocus
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={() => {
            renameSession(s.id, draft);
            setEditing(false);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              renameSession(s.id, draft);
              setEditing(false);
            }
            if (e.key === "Escape") setEditing(false);
          }}
          aria-label="Session title"
          className="relative h-7 w-40 shrink-0 rounded-lg bg-background px-2 text-[12.5px] outline-none"
        />
      ) : (
        <button type="button" onClick={() => setEditing(true)} className="relative shrink-0 cursor-pointer rounded-full p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground" title="Rename the session">
          <IconPencil size={13} />
        </button>
      )}
    </div>
  );
}

/** The cards written for this bot, newest first; a row opens the sheet. */
function CardsFor({ agentId, agentName }: { agentId: string; agentName: string }) {
  const cards = useCards().filter((c) => c.for === agentId && !c.done);
  if (cards.length === 0) return <Empty>No cards for {agentName} yet — type one above.</Empty>;
  return (
    <>
      {cards.map((c) => (
        <button key={c.id} type="button" onClick={() => openCard(c.id)} className="flex min-h-14 w-full cursor-pointer items-center gap-3 px-3.5 py-2.5 text-left hover:bg-accent" title={c.text}>
          <span className="grid size-8 shrink-0 place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
            <CardGlyph kind={c.kind} size={15} />
          </span>
          <span className="flex min-w-0 flex-1 flex-col">
            <span className="truncate text-[13.5px] font-medium">{c.title}</span>
            <span className="truncate text-[12px] text-muted-foreground">{c.detail}</span>
          </span>
          {c.kind === "checklist" && (
            <span className="shrink-0 text-[11.5px] text-muted-foreground">
              {c.data.done.length}/{c.data.items.length}
            </span>
          )}
        </button>
      ))}
    </>
  );
}
