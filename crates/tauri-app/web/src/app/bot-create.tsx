// The new-bot sheet and the page a fresh bot opens onto
// (docs/AGENTS-PROJECTS-ROUTINES-2026-10-17.md).
//
// One scrolling sheet, not a wizard: Kuvo's New Bot shows everything at once and
// collapses Start From to a one-line summary once you have picked, so you can see
// the name, face, personality, job and folder you are about to create. The page
// under a just-made bot is the same idea: the bot introduces itself, offers to
// start its job, and shows the routine cards it will live by.

import { useEffect, useRef, useState, type ReactElement, type ReactNode } from "react";
import { IconDeviceDesktop, IconFolder, IconLoader2, IconPlayerPlay } from "@tabler/icons-react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { RoutePicker } from "./RoutePicker";
import { Avatar, COLORS, FACES } from "./avatar";
import { RoutineCards } from "./routines";
import {
  agentStatus,
  assignProject,
  cancelBot,
  getHost,
  openAgentThread,
  saveAgent,
  saveRoutine,
  startJob,
  useChat,
  type Agent,
  type Computer,
  type Routine,
  type Route,
  type Thread,
} from "../bridge";

// Copied from Agents.tsx: both files are new and bots-ui owns only these two, and
// the well is one class string, not something to re-export for.
const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";
const label = "text-[12px] font-medium text-muted-foreground";
const field = `h-9 w-full rounded-xl bg-well px-3 text-[13.5px] outline-none ${WELL}`;
const FALLBACK_ROUTE: Route = { kind: "cli", provider: "claude", model: null };

const blankComputer = (): Computer => ({ kind: "folder", image: "", desktop: false, host: "", user: "", port: 0, key: "", desktop_url: "" });

// ---------------------------------------------------------------- templates

export type BotTemplate = {
  id: string;
  /** The tile's name: what kind of bot this is. */
  name: string;
  /** The name the bot is born with ("Herald" for Morning digest); "" for Blank. */
  botName: string;
  emoji: string;
  color: string;
  /** The one-line under the tile. */
  blurb: string;
  /** The one-line job: what the sidebar and the bot page show. */
  job: string;
  /** The long instructions: the rest of its backstory. */
  instructions: string;
  /** A sentence from PERSONALITIES, appended to its system prompt. */
  personality: string;
  /** The first routine the sheet offers to add (Herald, Keeper, Courier, Tally). */
  routine?: {
    name: string;
    instructions: string;
    kind: Routine["kind"];
    /** "HH:MM" for the time kinds; a N-hourly routine has no clock time. */
    time?: string;
    days?: number[];
    every_minutes?: number;
  };
};

export const BOT_TEMPLATES: BotTemplate[] = [
  {
    id: "blank",
    name: "Blank",
    botName: "",
    emoji: "🙂",
    color: "#64748b",
    blurb: "Start from nothing.",
    job: "",
    instructions: "",
    personality: "",
  },
  {
    id: "digest",
    name: "Morning digest",
    botName: "Herald",
    emoji: "🌅",
    color: "#ca8a04",
    blurb: "A short daily brief of what changed and what's next.",
    job: "Writes me a short morning brief: what changed in my projects overnight, what's due today, and the one thing to do first.",
    instructions:
      "Every morning, write a short brief I can read in two minutes. Cover what changed yesterday in my projects, what is due today, and the one thing I should do first. Keep it to a few bullets, name the files or pages you looked at, and say plainly when there is nothing worth reporting.",
    personality: "Direct and brief. Leads with the answer, skips the pleasantries, and says plainly when something's wrong.",
    routine: {
      name: "Morning digest",
      instructions: "Write today's morning brief: what changed, what's due today, and the one thing to do first. Keep it short.",
      kind: "daily",
      time: "08:00",
    },
  },
  {
    id: "research",
    name: "Research",
    botName: "Sage",
    emoji: "🔬",
    color: "#16a34a",
    blurb: "Reads the sources and comes back with the links.",
    job: "Researches a question properly: reads more than one source, compares them, and comes back with a short answer and the links.",
    instructions:
      "When I give you a question, research it properly. Read more than one source, compare what they say, and come back with a short answer, the strongest counterpoint, and the links you used. Say when the evidence is thin instead of filling the gap.",
    personality: "Questions everything first. Checks claims, looks for what could break, and says when the evidence is thin.",
  },
  {
    id: "watcher",
    name: "Build watcher",
    botName: "Keeper",
    emoji: "🛠️",
    color: "#2563eb",
    blurb: "Watches builds and tests so you don't have to.",
    job: "Watches my builds and test runs, and tells me when one breaks, what broke it, and the smallest fix.",
    instructions:
      "Watch my builds and test runs. When one fails, tell me which step failed, the first real error in the log, and the smallest fix you would try. When everything passes, say so in one line. Do not change code unless I ask.",
    personality: "Focused on what works. Weighs effort against payoff, picks the cheapest path, and moves on.",
    routine: {
      name: "Build watch",
      instructions:
        "Check the latest build and test runs. Report any failure with the first real error and the smallest fix; otherwise say in one line that everything is green.",
      kind: "every",
      every_minutes: 120,
    },
  },
  {
    id: "triage",
    name: "Inbox triage",
    botName: "Courier",
    emoji: "✉️",
    color: "#e11d48",
    blurb: "Sorts the inbox into what needs you and what doesn't.",
    job: "Triages my inbox: what needs a reply today, what can wait, and what to archive.",
    instructions:
      "Triage my inbox each afternoon. Sort what needs a reply today, what can wait, and what to archive. For the few that need me, draft a one-line reply in my voice and say why it matters. Never send anything; just tell me what you would do.",
    personality: "Direct and brief. Leads with the answer, skips the pleasantries, and says plainly when something's wrong.",
    routine: {
      name: "Inbox triage",
      instructions:
        "Triage the inbox: what needs a reply today, what can wait, what to archive. Draft one-line replies in my voice for the ones that need me, and send nothing.",
      kind: "weekdays",
      time: "17:00",
    },
  },
  {
    id: "explainer",
    name: "Explainer",
    botName: "Tutor",
    emoji: "📚",
    color: "#8b5cf6",
    blurb: "Explains a thing until it actually lands.",
    job: "Explains something I'm learning, from first principles, with one worked example and one thing to try.",
    instructions:
      "When I ask about something, explain it from first principles, in plain language, with one worked example and one small thing to try. Ask what I already know before going deep. Stop when it has landed; do not pad.",
    personality: "Patient and thoughtful. Explains the why, asks what you want to learn, and lets you make the call.",
  },
  {
    id: "standup",
    name: "Standup",
    botName: "Tally",
    emoji: "📈",
    color: "#0891b2",
    blurb: "Keeps a running standup: done, next, stuck.",
    job: "Runs my standup: what I finished, what's next, and what is stuck.",
    instructions:
      "Each weekday morning, write my standup from what we have discussed and the work in my projects: what I finished, what is next, and what is stuck. Three short lines, honest about what did not move, and ask me the one question that would unblock it.",
    personality: "Warm and encouraging. Celebrates what's working, keeps momentum up, and makes the next step feel easy.",
    routine: {
      name: "Standup",
      instructions:
        "Write today's standup from what we have discussed and the work in my projects: done, next, stuck. Three short lines, and ask the one question that would unblock anything stuck.",
      kind: "weekdays",
      time: "09:15",
    },
  },
];

export const PERSONALITIES: { name: string; text: string }[] = [
  { name: "None", text: "" },
  {
    name: "Straight Shooter",
    text: "Direct and brief. Leads with the answer, skips the pleasantries, and says plainly when something's wrong.",
  },
  {
    name: "Cheerleader",
    text: "Warm and encouraging. Celebrates what's working, keeps momentum up, and makes the next step feel easy.",
  },
  {
    name: "Mentor",
    text: "Patient and thoughtful. Explains the why, asks what you want to learn, and lets you make the call.",
  },
  {
    name: "Skeptic",
    text: "Questions everything first. Checks claims, looks for what could break, and says when the evidence is thin.",
  },
  {
    name: "Pragmatist",
    text: "Focused on what works. Weighs effort against payoff, picks the cheapest path, and moves on.",
  },
  {
    name: "Perfectionist",
    text: "Sweats the details. Notices the rough edge, fixes it before moving on, and cares how the result reads.",
  },
  {
    name: "Visionary",
    text: "Thinks big first. Connects the small task to where things are going, and suggests the version worth building.",
  },
  { name: "Dry Wit", text: "Understated and wry. Says the useful thing, deadpan, and never oversells it." },
];

// ---------------------------------------------------------------- new bot

export function NewBotDialog(): ReactElement | null {
  const { creating } = useChat();
  if (!creating) return null;
  // Remount per opening (the sheet unmounts on close), so no half-filled form
  // can come back when the sheet is opened again. The key carries the spec
  // too: "create an agent Watcher" twice with different names must remount.
  return <NewBotSheet key={`${creating.project || "mac"}:${creating.prefill?.name ?? ""}:${creating.prefill?.job ?? ""}`} openedFor={creating.project} prefill={creating.prefill} />;
}

function NewBotSheet({ openedFor, prefill }: { openedFor: string; prefill?: { name: string; job: string } }) {
  const { projects } = useChat();
  const host = getHost();
  const [template, setTemplate] = useState<BotTemplate | null>(null);
  // A spec from the hero prompt skips the template gallery: the line already
  // said what to make.
  const [picking, setPicking] = useState(!prefill);
  const [name, setName] = useState(prefill?.name ?? "");
  const [job, setJob] = useState(prefill?.job ?? "");
  const [instructions, setInstructions] = useState("");
  const [emoji, setEmoji] = useState(FACES[0]);
  const [color, setColor] = useState(COLORS[7]);
  const [personality, setPersonality] = useState("");
  const [route, setRoute] = useState<Route | null>(() => host.defaultRoute());
  const [project, setProject] = useState(openedFor);
  const [addRoutine, setAddRoutine] = useState(false);
  const [saving, setSaving] = useState(false);
  const nameRef = useRef<HTMLInputElement>(null);
  const opened = projects.find((p) => p.id === openedFor);
  const picked = projects.find((p) => p.id === project);

  useEffect(() => {
    // Real focus, but without scrolling past Start From: the first thing to do
    // is pick a starting point, not type a name you have not chosen yet.
    const t = window.setTimeout(() => nameRef.current?.focus({ preventScroll: true }), 0);
    return () => window.clearTimeout(t);
  }, []);

  const preview: Agent = {
    id: "",
    name: name || template?.botName || "New bot",
    job: composeJob(job, instructions),
    avatar: { emoji, color },
    personality,
    route: route ?? FALLBACK_ROUTE,
    shared: [],
    computer: blankComputer(),
    memory: true,
    off: [],
    apps_off: [],
    hook: "",
    routines: [],
    created: 0,
    updated: 0,
  };

  const chooseTemplate = (t: BotTemplate) => {
    setTemplate(t);
    setPicking(false);
    setName(t.botName);
    setJob(t.job);
    setInstructions(t.instructions);
    setEmoji(t.emoji);
    setColor(t.color);
    setPersonality(t.personality);
    setAddRoutine(!!t.routine);
  };

  const create = async () => {
    if (!name.trim() || saving) return;
    setSaving(true);
    try {
      const r = route ?? FALLBACK_ROUTE;
      const saved = await saveAgent({
        id: "",
        name: name.trim(),
        // The model takes one backstory and the sidebar shows one line: the
        // one-liner leads, the long job follows it.
        job: composeJob(job, instructions),
        // The emoji draws nothing any more — it seeds the blink phase, so a
        // bot made from scratch phases off its own name rather than sharing
        // one phase with every other bot made from scratch.
        avatar: { emoji: emoji === FACES[0] && !template ? name.trim() : emoji, color },
        personality,
        route: r,
        shared: [],
        computer: blankComputer(),
        memory: true,
        off: [],
        apps_off: [],
        hook: "",
        routines: [],
        created: 0,
        updated: 0,
      });
      if (project) await assignProject(project, saved.id, true);
      const suggest = template?.routine;
      if (addRoutine && suggest) {
        await saveRoutine(saved.id, {
          id: "",
          name: suggest.name,
          instructions: suggest.instructions,
          kind: suggest.kind,
          time: suggest.time ?? "",
          days: suggest.days ?? [],
          every_minutes: suggest.every_minutes ?? 0,
          // A routine of a bot that lives on a project runs there too.
          project,
          enabled: true,
          last_run: 0,
          next_run: 0,
          runs: 0,
          utc_offset_minutes: -new Date().getTimezoneOffset(),
          created: 0,
          updated: 0,
        });
      }
      cancelBot();
      await openAgentThread(saved, picked ?? null);
    } catch (e) {
      host.toast(String(e), "err");
      setSaving(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => { if (!o && !saving) cancelBot(); }}>
      <DialogContent className="top-[8vh] max-h-[84vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>New bot</DialogTitle>
          <DialogDescription>
            A name, a face and a job. It keeps its own conversation, remembers what it learns, and can wake itself up.
          </DialogDescription>
        </DialogHeader>

        <div className="mt-3 flex flex-col gap-4">
          <Section title="Start From">
            {picking || (!template && !prefill) ? (
              <div className="grid grid-cols-2 gap-1.5 sm:grid-cols-3">
                {BOT_TEMPLATES.map((t) => {
                  const on = template?.id === t.id;
                  return (
                    <button
                      key={t.id}
                      type="button"
                      aria-pressed={on}
                      onClick={() => chooseTemplate(t)}
                      className={`flex cursor-pointer items-start gap-2.5 rounded-xl p-2.5 text-left transition-colors ${on ? "bg-foreground/[0.06] ring-1 ring-foreground/25" : `bg-well hover:bg-accent ${WELL}`}`}
                    >
                      <span
                        aria-hidden
                        className="grid size-8 shrink-0 place-items-center rounded-full text-[16px]"
                        style={{ background: t.color }}
                      >
                        {t.emoji}
                      </span>
                      <span className="flex min-w-0 flex-col">
                        <span className="truncate text-[12.5px] font-semibold">{t.name}</span>
                        <span className="line-clamp-2 text-[11.5px]/4 text-muted-foreground">{t.blurb}</span>
                      </span>
                    </button>
                  );
                })}
              </div>
            ) : template ? (
              <div className={`flex items-center gap-2.5 rounded-xl bg-well p-2.5 ${WELL}`}>
                <Avatar agent={preview} size={32} />
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate text-[13px] font-semibold">{template.name}</span>
                  <span className="truncate text-[11.5px] text-muted-foreground">{template.blurb}</span>
                </span>
                <button
                  type="button"
                  onClick={() => setPicking(true)}
                  className="cursor-pointer rounded-full px-2.5 py-1 text-[12px] font-medium text-muted-foreground hover:bg-accent hover:text-foreground"
                >
                  Change
                </button>
              </div>
            ) : (
              /* Opened from the hero prompt ("create an agent named Watcher…"):
                 the line is the starting point, not a template. */
              <div className={`flex items-center gap-2.5 rounded-xl bg-well p-2.5 ${WELL}`}>
                <Avatar agent={preview} size={32} />
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate text-[13px] font-semibold">{prefill!.name || "From your line"}</span>
                  <span className="truncate text-[11.5px] text-muted-foreground">{prefill!.job || "Read out of what you typed"}</span>
                </span>
                <button
                  type="button"
                  onClick={() => setPicking(true)}
                  className="cursor-pointer rounded-full px-2.5 py-1 text-[12px] font-medium text-muted-foreground hover:bg-accent hover:text-foreground"
                >
                  Change
                </button>
              </div>
            )}
          </Section>

          <Section title="Name & job">
            <div className="flex items-start gap-3">
              <Avatar agent={preview} size={52} />
              <div className="flex min-w-0 flex-1 flex-col gap-2">
                <label className="flex flex-col gap-1">
                  <span className={label}>Name</span>
                  <input
                    ref={nameRef}
                    className={field}
                    value={name}
                    maxLength={40}
                    placeholder="Herald"
                    onChange={(e) => setName(e.target.value)}
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span className={label}>One-line job</span>
                  <input
                    className={field}
                    value={job}
                    maxLength={120}
                    placeholder="A short morning brief of what changed and what's next."
                    onChange={(e) => setJob(e.target.value)}
                  />
                </label>
              </div>
            </div>
          </Section>

          <Section title="Look">
            <div className="flex flex-wrap items-center gap-2">
              {COLORS.map((c) => (
                <button
                  key={c}
                  type="button"
                  aria-label={`Colour ${c}`}
                  aria-pressed={color === c}
                  onClick={() => setColor(c)}
                  className={`size-7 cursor-pointer rounded-full ${color === c ? "ring-2 ring-foreground ring-offset-2 ring-offset-(--surface)" : ""}`}
                  style={{ background: c }}
                />
              ))}
            </div>
            <div className="flex flex-wrap items-center gap-1">
              {/* Colour is the only shape axis left: every bot is the same brain
                  now, so the old shape row would have shown sixteen identical
                  faces. The stored emoji still seeds the blink phase. */}
              {COLORS.map((c) => (
                <button
                  key={c}
                  type="button"
                  aria-label={`Colour ${c}`}
                  aria-pressed={color === c}
                  onClick={() => setColor(c)}
                  className={`size-7 cursor-pointer rounded-full ${color === c ? "ring-2 ring-foreground ring-offset-2 ring-offset-(--surface)" : ""}`}
                  style={{ background: c }}
                />
              ))}
            </div>
          </Section>

          <Section
            title="Personality"
            note={personality || "No extra style — it answers like a plain, useful assistant."}
          >
            <div className="flex flex-wrap gap-1.5">
              {PERSONALITIES.map((p) => (
                <button
                  key={p.name}
                  type="button"
                  aria-pressed={personality === p.text}
                  onClick={() => setPersonality(p.text)}
                  className={`cursor-pointer rounded-full px-3 py-1.5 text-[12.5px] font-medium transition-colors ${personality === p.text ? "bg-foreground text-(--surface)" : `bg-well text-muted-foreground hover:bg-accent hover:text-foreground ${WELL}`}`}
                >
                  {p.name}
                </button>
              ))}
            </div>
          </Section>

          <Section title="Job" note="Its long instructions: how it works, what it checks, what to say when nothing happened.">
            <textarea
              className={`${field} h-auto min-h-28 py-2 leading-5`}
              value={instructions}
              placeholder="What it should do, in its own words. This becomes its system prompt."
              onChange={(e) => setInstructions(e.target.value)}
            />
          </Section>

          <Section title="Model">
            <div className="flex items-center justify-between gap-3">
              <span className="text-[12.5px] text-muted-foreground">Who answers when it works.</span>
              <RoutePicker value={route} onValueChange={setRoute} />
            </div>
          </Section>

          <Section
            title="Lives On"
            note={
              picked
                ? `It will work in ${picked.folder}.${opened && opened.id === picked.id ? " (This sheet was opened from that project.)" : ""}`
                : "Its own folder on this Mac. You can add it to a project later."
            }
          >
            <div className="grid grid-cols-2 gap-1.5">
              <Choice
                on={!project}
                title="This Mac"
                sub="Its own folder"
                icon={<IconDeviceDesktop size={14} />}
                onClick={() => setProject("")}
              />
              {projects.map((p) => (
                <Choice
                  key={p.id}
                  on={project === p.id}
                  title={p.name}
                  sub={p.folder}
                  icon={<IconFolder size={14} />}
                  onClick={() => setProject(p.id)}
                />
              ))}
            </div>
          </Section>

          {template?.routine && (
            <div className={`flex items-center gap-3 rounded-2xl bg-well px-3.5 py-2.5 ${WELL}`}>
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="text-[13px] font-medium">Add its first routine</span>
                <span className="truncate text-[11.5px] text-muted-foreground">
                  {template.routine.name} · {suggestWhen(template.routine)}
                </span>
              </span>
              <Switch on={addRoutine} label="Add its first routine" onChange={setAddRoutine} />
            </div>
          )}
        </div>

        <DialogFooter className="sticky bottom-0 -mx-4 -mb-4 border-t border-border bg-popover px-4 pt-3 pb-4">
          <Button variant="ghost" onClick={cancelBot} disabled={saving}>
            Cancel
          </Button>
          <Button disabled={!name.trim() || saving} onClick={() => void create()}>
            {saving ? "Creating…" : "Create bot"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------- bot page

export function BotHome({ agent, thread }: { agent: Agent; thread: Thread }): ReactElement {
  const host = getHost();
  const { threads } = useChat();
  const [starting, setStarting] = useState(false);
  const latest = threads.filter((t) => t.agent === agent.id).sort((x, y) => y.updated - x.updated)[0];
  const status = agentStatus(agent, latest);

  const start = async () => {
    if (starting) return;
    setStarting(true);
    try {
      await startJob(thread);
      // The next thing they will want is a way for it to come back on its own.
      if (agent.routines.length === 0 && thread.messages.length === 0) {
        host.toast(`No routine yet — add one below and ${agent.name} will wake itself up.`);
      }
    } catch (e) {
      host.toast(String(e), "err");
    } finally {
      setStarting(false);
    }
  };

  return (
    <div className="mx-auto flex w-full max-w-lg flex-col items-center gap-4 pt-8 pb-4 text-center">
      <Avatar agent={agent} size={56} />
      <div className="flex flex-col items-center gap-1">
        <div className="text-[19px]/7 font-semibold tracking-tight">{agent.name}</div>
        <div className="max-w-md text-[13px]/5 text-muted-foreground">{oneLine(agent.job)}</div>
        <StatusChip status={status} />
      </div>
      <p className="max-w-md text-[13px]/5 text-muted-foreground">
        Start {agent.name} on its job, or give it a task of your own. It keeps this conversation, remembers what it learns,
        and can wake itself up later to carry on.
      </p>
      <button
        type="button"
        disabled={starting}
        onClick={() => void start()}
        className="mt-1 flex h-9 cursor-pointer items-center gap-1.5 rounded-full bg-primary px-4 text-[13px] font-medium text-primary-foreground transition-colors hover:bg-(--primary-hover) disabled:cursor-default disabled:opacity-50"
      >
        {starting ? <IconLoader2 size={15} className="animate-spin" /> : <IconPlayerPlay size={15} />}
        Start its job
      </button>
      <div className="mt-2 w-full text-left">
        <RoutineCards agent={agent} />
      </div>
    </div>
  );
}

function StatusChip({ status }: { status: ReturnType<typeof agentStatus> }): ReactElement {
  const tone =
    status.kind === "working" ? "bg-emerald-500" : status.kind === "needs" ? "bg-sky-500" : status.kind === "wake" ? "bg-amber-500" : "bg-foreground/30";
  return (
    <span className="mt-1 inline-flex items-center gap-1.5 rounded-full bg-black/[0.05] px-2.5 py-1 text-[11.5px] font-medium text-muted-foreground dark:bg-white/[0.08]">
      <span className={`size-1.5 rounded-full ${tone}`} />
      {status.label}
    </span>
  );
}

// ---------------------------------------------------------------- helpers

/** One backstory from the sheet's two fields: the one-liner, then the long job. */
const composeJob = (one: string, long: string) => [one.trim(), long.trim()].filter(Boolean).join("\n\n");

/** The line the sidebar and the bot page show, without the long job under it. */
const oneLine = (job: string) => job.split("\n").find((l) => l.trim())?.trim() ?? "";

const suggestWhen = (r: NonNullable<BotTemplate["routine"]>) => {
  const at = r.time || "08:00";
  switch (r.kind) {
    case "daily":
      return `Every day at ${at}`;
    case "weekdays":
      return `Weekdays at ${at}`;
    case "weekly":
      return `${(r.days?.length ? r.days : [1]).map((d) => ["", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][d]).join(", ")} at ${at}`;
    case "every": {
      const m = r.every_minutes ?? 60;
      return m >= 60 && m % 60 === 0 ? `Every ${m / 60} hour${m === 60 ? "" : "s"}` : `Every ${m} min`;
    }
    default:
      return "Not scheduled";
  }
};

function Section({ title, note, children }: { title: string; note?: ReactNode; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-[13px] font-semibold">{title}</h3>
      {children}
      {note && <p className="px-1 text-[11.5px]/4 text-muted-foreground">{note}</p>}
    </section>
  );
}

/* One "Lives On" choice: this Mac or a project folder. */
function Choice({ on, title, sub, icon, onClick }: { on: boolean; title: string; sub: string; icon: ReactNode; onClick: () => void }): ReactElement {
  return (
    <button
      type="button"
      aria-pressed={on}
      onClick={onClick}
      className={`flex cursor-pointer flex-col items-start gap-0.5 rounded-xl p-2.5 text-left ${on ? "bg-foreground text-(--surface)" : `bg-well hover:bg-accent ${WELL}`}`}
    >
      <span className="flex w-full items-center gap-1.5 text-[12.5px] font-semibold">
        <span className="shrink-0">{icon}</span>
        <span className="truncate">{title}</span>
      </span>
      <span className={`w-full truncate text-[11px]/4 ${on ? "opacity-80" : "text-muted-foreground"}`} title={sub}>
        {sub}
      </span>
    </button>
  );
}

/* The same switch as the agent editor's (not exported from there): a real
   button with role=switch, so keyboard and screen readers get it for free. */
function Switch({ on, disabled, label, onChange }: { on: boolean; disabled?: boolean; label: string; onChange: (on: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!on)}
      className={`relative h-5 w-8 flex-none cursor-pointer rounded-full transition-colors disabled:cursor-default disabled:opacity-40 ${on ? "bg-foreground" : "bg-foreground/15"}`}
    >
      <span className={`absolute top-0.5 size-4 rounded-full bg-(--surface) shadow transition-[left] ${on ? "left-3.5" : "left-0.5"}`} />
    </button>
  );
}
