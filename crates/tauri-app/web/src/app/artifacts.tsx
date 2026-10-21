// Artifacts: the web view of a thing you already have.
//
// A project has an artifact — one page with its bots, its sessions and its
// cards, the way a folder has a face. A card has an artifact — the same
// shapeshift card, drawn as a page you can actually use: check the items off,
// see the split, watch the countdown. The sidebar's Artifacts section lists
// them; this file draws them.
//
// The rule they follow is the rule the whole face follows: an artifact is a
// view of a thing that exists, never a copy of it. Ticking a checklist item
// ticks the card; the project page is still the project page.

import { useMemo } from "react";
import { IconArrowLeft, IconCalendar, IconCheck, IconClock, IconFolder, IconUsersGroup } from "@tabler/icons-react";

import { RowPill } from "@/components/row-pill";
import { Avatar } from "./avatar";
import { CardGlyph } from "./shapeshift";
import { agentById, agentStatus, closeArtifact, openArtifact, openProject, openThread, useChat, type Project, type ThreadInfo } from "../bridge";
import { KIND_LABEL } from "../shapeshift";
import { setDone, toggleItem, useCards } from "../shapeshift";
import { useNow } from "./sessions";

const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";

// ---------------------------------------------------------------- sidebar

/** Every artifact there is: one per project, one per open card. */
export function ArtifactsSide() {
  const { projects, artifact } = useChat();
  const cards = useCards().filter((c) => !c.done);
  const row = "group/row relative flex h-11 w-full cursor-pointer items-center gap-2.5 rounded-lg px-2 text-left";
  const on = (kind: "project" | "card", id: string) => artifact?.kind === kind && artifact.id === id;
  return (
    <section>
      <div className="flex h-5 items-center px-2.5 text-[10.5px]/4 font-medium text-muted-foreground/55">Artifacts</div>
      {projects.length === 0 && cards.length === 0 && <div className="px-2.5 py-1.5 text-[12.5px]/5 text-muted-foreground">No artifacts yet. A project or a card becomes one.</div>}
      {projects.map((p) => (
        <button key={p.id} type="button" className={row} onClick={() => openArtifact({ kind: "project", id: p.id })} title={`${p.name} — its web view`}>
          <RowPill className={on("project", p.id) ? "bg-accent" : ""} />
          <span className="relative grid size-7 shrink-0 place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
            <IconFolder size={14} />
          </span>
          <span className="relative flex min-w-0 flex-1 flex-col">
            <span className="truncate text-[13.5px]/4 font-medium text-foreground">{p.name}</span>
            <span className="truncate text-[11.5px]/4 text-muted-foreground">
              project · {p.agents.length} bot{p.agents.length === 1 ? "" : "s"}
            </span>
          </span>
        </button>
      ))}
      {cards.slice(0, 8).map((c) => (
        <button key={c.id} type="button" className={row} onClick={() => openArtifact({ kind: "card", id: c.id })} title={`${c.title} — its web view`}>
          <RowPill className={on("card", c.id) ? "bg-accent" : ""} />
          <span className="relative grid size-7 shrink-0 place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
            <CardGlyph kind={c.kind} size={14} />
          </span>
          <span className="relative flex min-w-0 flex-1 flex-col">
            <span className="truncate text-[13.5px]/4 font-medium text-foreground">{c.title}</span>
            <span className="truncate text-[11.5px]/4 text-muted-foreground">
              {KIND_LABEL[c.kind]} · {c.detail}
            </span>
          </span>
        </button>
      ))}
    </section>
  );
}

// ---------------------------------------------------------------- the view

/** The artifact itself, in the main pane. */
export function ArtifactView() {
  const { artifact, projects } = useChat();
  const cards = useCards();
  if (!artifact) return null;
  const project = artifact.kind === "project" ? projects.find((p) => p.id === artifact.id) : undefined;
  const card = artifact.kind === "card" ? cards.find((c) => c.id === artifact.id) : undefined;
  return (
    <div className="h-full overflow-y-auto px-6 pt-8 pb-16">
      <div className="mx-auto flex max-w-2xl flex-col gap-5">
        <button type="button" onClick={closeArtifact} className="flex w-fit cursor-pointer items-center gap-1.5 text-[12.5px] font-medium text-muted-foreground hover:text-foreground">
          <IconArrowLeft size={14} />
          All bots
        </button>
        {project && <ProjectArtifact project={project} />}
        {card && <CardArtifact card={card} />}
        {!project && !card && <p className="text-[13px]/5 text-muted-foreground">This artifact is gone — the card was deleted, or the project was removed.</p>}
      </div>
    </div>
  );
}

function ProjectArtifact({ project }: { project: Project }) {
  const { threads } = useChat();
  const cards = useCards().filter((c) => !c.done);
  const mine = useMemo(
    () => threads.filter((t) => !t.members?.length && !t.app && project.agents.includes(t.agent ?? "")).sort((x, y) => y.updated - x.updated),
    [threads, project.agents],
  );
  return (
    <>
      <div className="flex items-center gap-3">
        <span className={`grid size-11 shrink-0 place-items-center rounded-2xl bg-well text-muted-foreground ${WELL}`}>
          <IconFolder size={20} />
        </span>
        <div className="min-w-0">
          <h1 className="text-[22px]/8 font-medium tracking-tight">{project.name}</h1>
          <p className="truncate text-[12px]/4 text-muted-foreground" title={project.folder}>
            {project.folder}
          </p>
        </div>
      </div>

      <section className="flex flex-col gap-2">
        <h2 className="text-[13px] font-semibold text-muted-foreground">Bots on this project</h2>
        <div className="flex flex-wrap gap-2">
          {project.agents.map((id) => {
            const a = agentById(id);
            if (!a) return null;
            const st = agentStatus(a, mine.find((t) => t.agent === id) ?? null);
            return (
              <span key={id} className={`flex items-center gap-2.5 rounded-2xl bg-well px-3.5 py-2.5 ${WELL}`}>
                <Avatar agent={a} size={28} state={st.kind === "wake" ? "wake" : st.kind} />
                <span className="flex flex-col">
                  <span className="text-[13px]/4 font-medium">{a.name}</span>
                  <span className="text-[11.5px]/4 text-muted-foreground">{st.label}</span>
                </span>
              </span>
            );
          })}
          {project.agents.length === 0 && <p className="px-1 text-[12.5px]/5 text-muted-foreground">No bots assigned yet.</p>}
        </div>
      </section>

      <section className="flex flex-col gap-2">
        <h2 className="text-[13px] font-semibold text-muted-foreground">Sessions here</h2>
        {mine.length === 0 && <p className="px-1 text-[12.5px]/5 text-muted-foreground">No sessions in this project yet.</p>}
        <div className="flex flex-col gap-2">
          {mine.map((t) => (
            <button key={t.id} type="button" onClick={() => void openThread(t.id)} className={`flex items-center gap-3 rounded-2xl bg-well px-4 py-3 text-left ${WELL}`}>
              <span className="grid size-8 shrink-0 place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
                <IconUsersGroup size={15} />
              </span>
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate text-[13px]/4 font-medium">{t.title}</span>
                <span className="truncate text-[11.5px]/4 text-muted-foreground">{t.preview || "No messages yet"}</span>
              </span>
              {t.busy && <IconClock size={14} className="shrink-0 animate-spin text-muted-foreground" />}
            </button>
          ))}
        </div>
      </section>

      <section className="flex flex-col gap-2">
        <h2 className="text-[13px] font-semibold text-muted-foreground">Cards</h2>
        {cards.length === 0 && <p className="px-1 text-[12.5px]/5 text-muted-foreground">No open cards.</p>}
        <div className="flex flex-wrap gap-2">
          {cards.map((c) => (
            <span key={c.id} className={`flex items-center gap-2 rounded-full bg-well px-3 py-1.5 text-[12.5px] ${WELL}`}>
              <CardGlyph kind={c.kind} size={13} />
              {c.title}
            </span>
          ))}
        </div>
      </section>

      <button type="button" onClick={() => openProject(project)} className="flex w-fit cursor-pointer items-center gap-1.5 rounded-full bg-primary px-3.5 py-2 text-[13px] font-medium text-primary-foreground hover:bg-(--primary-hover)">
        <IconFolder size={15} />
        Open the project
      </button>
    </>
  );
}

/** A card, drawn as a page you can use. */
function CardArtifact({ card }: { card: import("../shapeshift/types").Card }) {
  return (
    <>
      <div className="flex items-center gap-3">
        <span className={`grid size-11 shrink-0 place-items-center rounded-2xl bg-well text-muted-foreground ${WELL}`}>
          <CardGlyph kind={card.kind} size={20} />
        </span>
        <div className="min-w-0">
          <h1 className="text-[22px]/8 font-medium tracking-tight">{card.title}</h1>
          <p className="text-[12px]/4 text-muted-foreground">
            {KIND_LABEL[card.kind]} · {card.detail}
          </p>
        </div>
      </div>

      <div className={`rounded-2xl bg-well px-5 py-4 ${WELL}`}>
        {card.kind === "checklist" && (
          <div className="flex flex-col gap-2">
            {card.data.items.map((item, i) => {
              const on = card.data.done.includes(i);
              return (
                <button key={`${item}-${i}`} type="button" onClick={() => toggleItem(card.id, i)} className="flex cursor-pointer items-center gap-2.5 text-left" aria-pressed={on}>
                  <span className={`grid size-5 shrink-0 place-items-center rounded-md border ${on ? "border-transparent bg-foreground text-(--surface)" : "border-border"}`}>
                    {on && <IconCheck size={12} />}
                  </span>
                  <span className={`text-[14px]/6 ${on ? "text-muted-foreground line-through" : ""}`}>{item}</span>
                </button>
              );
            })}
          </div>
        )}
        {card.kind === "event" && (
          <div className="flex flex-col gap-1.5 text-[14px]/6">
            <p className="flex items-center gap-2">
              <IconCalendar size={15} className="text-muted-foreground" />
              {new Date(card.data.when).toLocaleString(undefined, { weekday: "long", hour: "numeric", minute: "2-digit" })}
            </p>
            {card.data.who && <p className="text-muted-foreground">with {card.data.who}</p>}
            {card.data.where && <p className="text-muted-foreground">on {card.data.where}</p>}
          </div>
        )}
        {card.kind === "reminder" && (
          <p className="flex items-center gap-2 text-[14px]/6">
            <IconClock size={15} className="text-muted-foreground" />
            {new Date(card.data.when).toLocaleString(undefined, { weekday: "long", hour: "numeric", minute: "2-digit" })}
          </p>
        )}
        {card.kind === "split" && (
          <div className="flex flex-col gap-1 text-[14px]/6">
            <p>{(card.data.amount / card.data.people).toLocaleString(undefined, { maximumFractionDigits: 2 })} each</p>
            <p className="text-muted-foreground">
              {card.data.amount.toLocaleString()} between {card.data.people}
            </p>
          </div>
        )}
        {card.kind === "countdown" && <Countdown until={card.data.until} />}
        {card.kind === "timer" && <p className="text-[14px]/6">{card.data.minutes} minutes</p>}
        {card.kind === "poll" && (
          <div className="flex flex-col gap-1.5 text-[14px]/6">
            <p className="font-medium">{card.data.question}</p>
            {card.data.options.map((o) => (
              <span key={o} className="text-muted-foreground">
                · {o}
              </span>
            ))}
          </div>
        )}
        {card.kind === "note" && <p className="text-[14px]/6">{card.text}</p>}
        {(card.kind === "expense" || card.kind === "convert" || card.kind === "calc") && <p className="text-[14px]/6">{card.detail}</p>}
      </div>

      <div className="flex items-center gap-3">
        {!card.done && (
          <button type="button" onClick={() => setDone(card.id, true)} className="flex shrink-0 cursor-pointer items-center gap-1.5 rounded-full bg-primary px-3.5 py-2 text-[13px] font-medium text-primary-foreground hover:bg-(--primary-hover)">
            <IconCheck size={15} />
            Mark done
          </button>
        )}
        <p className="text-[12px]/5 text-muted-foreground">“{card.text}”</p>
      </div>
    </>
  );
}

function Countdown({ until }: { until: number }) {
  const now = useNow(1000);
  const left = Math.max(0, until - now);
  const days = Math.floor(left / 86400e3);
  const hours = Math.floor((left % 86400e3) / 3600e3);
  const mins = Math.floor((left % 3600e3) / 60e3);
  return (
    <p className="text-[14px]/6 tabular-nums">
      {days > 0 ? `${days}d ` : ""}
      {hours}h {mins}m left
    </p>
  );
}
