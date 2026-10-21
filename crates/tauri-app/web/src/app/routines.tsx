// A bot's routine is its job on a clock (docs/AGENTS-PROJECTS-ROUTINES-2026-10-17.md).
// The dialog is three short steps — what it does, when, where — because a schedule
// only has three real questions; the cards reuse the Agents side's wells and type so
// a routine reads as part of the bot, not as a separate scheduling app.

import { useEffect, useRef, useState, type ReactElement, type ReactNode } from "react";
import {
  IconChevronLeft,
  IconChevronRight,
  IconClock,
  IconPencil,
  IconPlayerPlay,
  IconPlus,
  IconTrash,
} from "@tabler/icons-react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Avatar } from "./avatar";
import {
  cancelRoutine,
  editRoutine,
  getHost,
  nextWakeLabel,
  openAgentThread,
  removeRoutine,
  routineWhen,
  runRoutine,
  saveRoutine,
  useChat,
  type Agent,
  type Routine,
} from "../bridge";

// Copied from Agents.tsx rather than exported from there: these files are the
// only two bots-ui owns, and the well is one class string, not a dependency.
const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";
const label = "text-[12px] font-medium text-muted-foreground";
const field = `h-9 w-full rounded-xl bg-well px-3 text-[13.5px] outline-none ${WELL}`;
const WEEKDAYS = ["", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const STEPS = ["What it does", "When", "Where"];
const KINDS: [Routine["kind"], string][] = [
  ["daily", "Every day"],
  ["weekdays", "Weekdays"],
  ["weekly", "Weekly"],
  ["every", "Every N"],
];

/** The window's local guess at the next fire, for the "Next wake" line only —
 *  the core recomputes and stores the authoritative `next_run` on save. */
function nextFire(r: Pick<Routine, "kind" | "time" | "days" | "every_minutes">, now: number): number {
  if (r.kind === "every") return now + Math.max(5, r.every_minutes) * 60_000;
  const [hh, mm] = (r.time || "08:00").split(":").map(Number);
  const hours = Number.isFinite(hh) ? hh : 8;
  const mins = Number.isFinite(mm) ? mm : 0;
  const at = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate(), hours, mins, 0, 0).getTime();
  for (let i = 0; i < 8; i++) {
    const d = new Date(now + i * 86_400_000);
    const iso = d.getDay() === 0 ? 7 : d.getDay();
    if (r.kind === "weekdays" && iso > 5) continue;
    if (r.kind === "weekly" && !(r.days.length ? r.days : [1]).includes(iso)) continue;
    const x = at(d);
    if (x > now) return x;
  }
  return 0;
}

function wakePreview(r: Pick<Routine, "enabled" | "kind" | "time" | "days" | "every_minutes">): string {
  if (!r.enabled) return "not scheduled (paused)";
  if (r.kind === "every") {
    const m = Math.max(5, r.every_minutes);
    return m >= 60 && m % 60 === 0 ? `in ${m / 60} hour${m === 60 ? "" : "s"}` : `in ${m} min`;
  }
  return nextWakeLabel(nextFire(r, Date.now()));
}

// ---------------------------------------------------------------- dialog

export function RoutineDialog(): ReactElement | null {
  const { routineEdit } = useChat();
  if (!routineEdit) return null;
  // Keyed so opening another routine (or the same one again) starts clean: a
  // half-typed schedule must never leak into the next sheet.
  const key = `${routineEdit.agent}:${routineEdit.routine === "new" ? "new" : routineEdit.routine.id}`;
  return <RoutineSheet key={key} agentId={routineEdit.agent} initial={routineEdit.routine} />;
}

function RoutineSheet({ agentId, initial }: { agentId: string; initial: Routine | "new" }) {
  const { agents, projects } = useChat();
  const host = getHost();
  const r0 = initial === "new" ? null : initial;
  const [step, setStep] = useState(0);
  const [name, setName] = useState(r0?.name ?? "");
  const [instructions, setInstructions] = useState(r0?.instructions ?? "");
  const [kind, setKind] = useState<Routine["kind"]>(r0?.kind ?? "daily");
  const [time, setTime] = useState(r0?.time || "08:00");
  const [days, setDays] = useState<number[]>(r0?.days?.length ? r0.days : [1]);
  const [every, setEvery] = useState(() => {
    const m = r0?.every_minutes || 360;
    return m % 60 === 0 ? m / 60 : m;
  });
  const [unit, setUnit] = useState<"hours" | "minutes">(() => ((r0?.every_minutes || 360) % 60 === 0 ? "hours" : "minutes"));
  const [project, setProject] = useState(r0?.project ?? "");
  const [enabled, setEnabled] = useState(r0?.enabled ?? true);
  const [saving, setSaving] = useState(false);
  const nameRef = useRef<HTMLInputElement>(null);
  const agent = agents.find((a) => a.id === agentId);
  const everyMinutes = Math.max(5, unit === "hours" ? every * 60 : every);
  // Keep a routine's current project on the list even after the bot was taken off
  // it: editing a schedule should never silently move the routine's folder.
  const projectChoices = projects.filter((p) => p.agents.includes(agentId) || p.id === project);

  useEffect(() => {
    // After the dialog's own focus management has run, and without scrolling the
    // sheet away from the first section: start typing, still see where you are.
    const t = window.setTimeout(() => nameRef.current?.focus({ preventScroll: true }), 0);
    return () => window.clearTimeout(t);
  }, []);

  if (!agent) return null;

  const build = (): Routine => ({
    id: r0?.id ?? "",
    name: name.trim(),
    instructions: instructions.trim(),
    kind,
    time: kind === "every" ? "" : time || "08:00",
    days: kind === "weekly" ? days : [],
    every_minutes: kind === "every" ? everyMinutes : 0,
    project,
    enabled,
    // Keep the scheduler's bookkeeping from the old routine; the core rewrites
    // next_run on save and fills id/created for a new one.
    last_run: r0?.last_run ?? 0,
    next_run: r0?.next_run ?? 0,
    runs: r0?.runs ?? 0,
    utc_offset_minutes: -new Date().getTimezoneOffset(),
    created: r0?.created ?? 0,
    updated: r0?.updated ?? 0,
  });

  const preview = wakePreview({ enabled, kind, time, days, every_minutes: everyMinutes });
  const whenText = routineWhen({ ...build(), enabled: true });
  const whereText = projects.find((p) => p.id === project)?.name ?? "its own folder";
  const nextOk = step === 0 ? !!name.trim() : step === 1 ? kind !== "weekly" || days.length > 0 : true;

  const toggleDay = (d: number) =>
    setDays((list) => (list.includes(d) ? list.filter((x) => x !== d) : [...list, d].sort((a, b) => a - b)));

  const save = async () => {
    if (!name.trim() || saving) return;
    setSaving(true);
    try {
      await saveRoutine(agent.id, build());
      cancelRoutine();
    } catch (e) {
      host.toast(String(e), "err");
      setSaving(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => { if (!o && !saving) cancelRoutine(); }}>
      <DialogContent className="top-[12vh] max-h-[80vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <div className="flex items-baseline justify-between gap-2">
            <DialogTitle>{r0 ? "Edit routine" : "New routine"}</DialogTitle>
            <span className="shrink-0 text-[11.5px] font-medium text-muted-foreground">
              Step {step + 1} of 3 · {STEPS[step]}
            </span>
          </div>
          <DialogDescription>
            {agent.name}'s job on a clock. Its model, rules, memory, folder and computer come with it.
          </DialogDescription>
        </DialogHeader>

        <div className="mt-3 flex flex-col gap-3">
          {step === 0 && (
            <>
              <label className="flex flex-col gap-1">
                <span className={label}>Name</span>
                <input
                  ref={nameRef}
                  className={field}
                  value={name}
                  maxLength={60}
                  placeholder="Morning digest"
                  onChange={(e) => setName(e.target.value)}
                />
              </label>
              <label className="flex flex-col gap-1">
                <span className={label}>What it does</span>
                <textarea
                  className={`${field} h-auto min-h-32 py-2 leading-5`}
                  value={instructions}
                  placeholder="What to do when it fires: the steps, the format, and what to say when nothing happened."
                  onChange={(e) => setInstructions(e.target.value)}
                />
              </label>
              <p className="px-1 text-[11.5px]/4 text-muted-foreground">
                A message starting “Routine · {name.trim() || "its name"}” is this routine falling due. {agent.name} carries it out and answers in one line.
              </p>
            </>
          )}

          {step === 1 && (
            <>
              <div className="flex flex-col gap-1">
                <span className={label}>How often</span>
                <div className={`grid grid-cols-4 gap-1 rounded-xl bg-well p-1 ${WELL}`}>
                  {KINDS.map(([k, t]) => (
                    <button
                      key={k}
                      type="button"
                      aria-pressed={kind === k}
                      onClick={() => setKind(k)}
                      className={`h-8 cursor-pointer rounded-lg text-[12px] font-medium transition-colors ${kind === k ? "bg-foreground text-(--surface)" : "text-muted-foreground hover:bg-accent hover:text-foreground"}`}
                    >
                      {t}
                    </button>
                  ))}
                </div>
              </div>
              {kind !== "every" && (
                <label className="flex max-w-40 flex-col gap-1">
                  <span className={label}>At</span>
                  <input type="time" className={field} value={time} onChange={(e) => setTime(e.target.value)} />
                </label>
              )}
              {kind === "weekly" && (
                <div className="flex flex-col gap-1">
                  <span className={label}>Days</span>
                  <div className="flex flex-wrap gap-1">
                    {[1, 2, 3, 4, 5, 6, 7].map((d) => (
                      <button
                        key={d}
                        type="button"
                        aria-pressed={days.includes(d)}
                        onClick={() => toggleDay(d)}
                        className={`h-8 min-w-10 cursor-pointer rounded-full px-2 text-[12px] font-medium ${days.includes(d) ? "bg-foreground text-(--surface)" : `bg-well text-muted-foreground hover:bg-accent hover:text-foreground ${WELL}`}`}
                      >
                        {WEEKDAYS[d]}
                      </button>
                    ))}
                  </div>
                  {days.length === 0 && <span className="text-[11.5px] text-red-500">Pick at least one day.</span>}
                </div>
              )}
              {kind === "every" && (
                <div className="flex flex-col gap-1">
                  <span className={label}>Every</span>
                  <div className="flex items-center gap-2">
                    <input
                      type="number"
                      min={1}
                      aria-label="How many"
                      className={`${field} w-24`}
                      value={every}
                      onChange={(e) => setEvery(Math.max(1, Number(e.target.value) || 1))}
                    />
                    <select
                      className={`${field} w-28 cursor-pointer`}
                      value={unit}
                      aria-label="Unit"
                      onChange={(e) => setUnit(e.target.value as "hours" | "minutes")}
                    >
                      <option value="hours">hours</option>
                      <option value="minutes">minutes</option>
                    </select>
                  </div>
                </div>
              )}
              <div className={`flex items-center gap-2 rounded-xl bg-well px-3 py-2 text-[12.5px] text-muted-foreground ${WELL}`}>
                <IconClock size={14} />
                <span>
                  Next wake: <span className="font-medium text-foreground">{preview}</span>
                </span>
              </div>
            </>
          )}

          {step === 2 && (
            <>
              <div className="flex flex-col gap-1">
                <span className={label}>Where it works</span>
                <div className="grid grid-cols-2 gap-1.5">
                  <Choice
                    on={!project}
                    title="Its own folder"
                    sub={`${agent.name}'s folder on this Mac`}
                    onClick={() => setProject("")}
                  />
                  {projectChoices.map((p) => (
                    <Choice key={p.id} on={project === p.id} title={p.name} sub={p.folder} onClick={() => setProject(p.id)} />
                  ))}
                </div>
                <p className="px-1 text-[11.5px]/4 text-muted-foreground">
                  In a project it wakes in that folder, and its message lands in {agent.name}'s project chat.
                </p>
              </div>
              <div className={`flex items-center gap-3 rounded-xl bg-well px-3.5 py-2.5 ${WELL}`}>
                <Switch on={enabled} label="Routine enabled" onChange={setEnabled} />
                <span className="flex min-w-0 flex-col">
                  <span className="text-[12.5px] font-medium">{enabled ? "On" : "Paused"}</span>
                  <span className="text-[11.5px] text-muted-foreground">
                    {enabled ? "It wakes itself up on this schedule." : "It keeps the schedule but does not wake."}
                  </span>
                </span>
              </div>
              <p className={`rounded-xl bg-well px-3 py-2 text-[12.5px]/5 text-muted-foreground ${WELL}`}>
                {name.trim() || "Untitled"} · {whenText} · {whereText}
              </p>
            </>
          )}
        </div>

        <DialogFooter className="justify-between">
          <div>
            {step > 0 && (
              <Button variant="ghost" onClick={() => setStep(step - 1)} disabled={saving}>
                <IconChevronLeft size={15} /> Back
              </Button>
            )}
          </div>
          <div className="flex gap-2">
            <Button variant="ghost" onClick={cancelRoutine} disabled={saving}>
              Cancel
            </Button>
            {step < 2 ? (
              <Button disabled={!nextOk} onClick={() => setStep(step + 1)}>
                Next <IconChevronRight size={15} />
              </Button>
            ) : (
              <Button disabled={!name.trim() || saving} onClick={() => void save()}>
                {saving ? "Saving…" : "Save routine"}
              </Button>
            )}
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------- cards

export function RoutineCards({ agent }: { agent: Agent }): ReactElement {
  const host = getHost();
  const [running, setRunning] = useState<string | null>(null);
  // Two clicks to delete: the first arms, the second removes. Local state, so
  // leaving the page disarms it too.
  const [confirm, setConfirm] = useState<string | null>(null);

  const run = async (r: Routine) => {
    setConfirm(null);
    setRunning(r.id);
    try {
      await runRoutine(agent.id, r.id);
      host.toast(`${r.name} sent to ${agent.name}`, "ok");
    } catch (e) {
      host.toast(String(e), "err");
    } finally {
      setRunning(null);
    }
  };

  const toggle = async (r: Routine, on: boolean) => {
    setConfirm(null);
    try {
      // next_run 0 on re-enable: the core recomputes it for the current offset.
      await saveRoutine(agent.id, { ...r, enabled: on, next_run: 0, utc_offset_minutes: -new Date().getTimezoneOffset() });
    } catch (e) {
      host.toast(String(e), "err");
    }
  };

  const remove = async (r: Routine) => {
    if (confirm !== r.id) {
      setConfirm(r.id);
      return;
    }
    setConfirm(null);
    try {
      await removeRoutine(agent.id, r.id);
    } catch (e) {
      host.toast(String(e), "err");
    }
  };

  const pill = "flex h-7 cursor-pointer items-center gap-1 rounded-full px-2.5 text-[12px] font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:cursor-default disabled:opacity-50";

  return (
    <section className="flex w-full flex-col gap-2">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-semibold">Routines</h2>
      </div>
      {agent.routines.length === 0 ? (
        <button
          type="button"
          onClick={() => editRoutine(agent.id)}
          className="flex cursor-pointer flex-col items-center gap-1.5 rounded-2xl border-[1.5px] border-dashed border-border px-4 py-5 text-center text-[12.5px]/5 text-muted-foreground transition-colors hover:border-foreground/30 hover:text-foreground"
        >
          <IconPlus size={16} />
          <span>No routines yet. Give {agent.name} a job on a clock and it will wake itself up.</span>
        </button>
      ) : (
        <>
          {agent.routines.map((r) => (
            <div key={r.id} className={`flex flex-col gap-1 rounded-2xl bg-well px-4 py-3 ${WELL}`}>
              <div className="flex items-start gap-2">
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate text-[14px] font-medium">{r.name}</span>
                  <span className="flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
                    <IconClock size={12} className="shrink-0" />
                    <span className="truncate">
                      {routineWhen(r)}
                      {r.enabled && r.next_run > 0 ? ` · next wake ${nextWakeLabel(r.next_run)}` : ""}
                    </span>
                  </span>
                </span>
                <Switch on={r.enabled} label={`${r.name} enabled`} onChange={(on) => void toggle(r, on)} />
              </div>
              <div className="flex items-center gap-0.5">
                <button type="button" className={pill} disabled={running === r.id} onClick={() => void run(r)}>
                  <IconPlayerPlay size={13} /> {running === r.id ? "Running…" : "Run now"}
                </button>
                <button
                  type="button"
                  className={pill}
                  onClick={() => {
                    setConfirm(null);
                    editRoutine(agent.id, r);
                  }}
                >
                  <IconPencil size={13} /> Edit
                </button>
                <button
                  type="button"
                  aria-label={confirm === r.id ? `Confirm deleting ${r.name}` : `Delete ${r.name}`}
                  className={`${pill} ml-auto ${confirm === r.id ? "text-red-500 hover:text-red-500" : ""}`}
                  onClick={() => void remove(r)}
                >
                  <IconTrash size={13} /> {confirm === r.id ? "Delete?" : ""}
                </button>
              </div>
            </div>
          ))}
          <button
            type="button"
            onClick={() => editRoutine(agent.id)}
            className="flex h-10 cursor-pointer items-center justify-center gap-1.5 rounded-2xl border-[1.5px] border-dashed border-border text-[12.5px] font-medium text-muted-foreground transition-colors hover:border-foreground/30 hover:text-foreground"
          >
            <IconPlus size={14} /> Add routine
          </button>
        </>
      )}
    </section>
  );
}

// ---------------------------------------------------------------- bits

/* One choice tile: own folder or a project. Static markup, no hook. */
function Choice({ on, title, sub, onClick }: { on: boolean; title: string; sub: string; onClick: () => void }): ReactNode {
  return (
    <button
      type="button"
      aria-pressed={on}
      onClick={onClick}
      className={`flex cursor-pointer flex-col items-start gap-0.5 rounded-xl p-2.5 text-left ${on ? "bg-foreground text-(--surface)" : `bg-well hover:bg-accent ${WELL}`}`}
    >
      <span className="w-full truncate text-[12.5px] font-semibold">{title}</span>
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
