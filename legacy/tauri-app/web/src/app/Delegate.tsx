// The Delegate sheet: the tasks waiting on the fleet, and which bot each one
// goes to (the mockup's drag-a-task-onto-an-agent board).
//
// The board and its prices are computed by the core (`delegate.rs`), so this
// file draws and edits: a card dragged onto a bot is a manual attribution, a
// card dragged back is Unassigned, and "Auto" lets the shortest-queue rule
// fill what waits. Every card carries the line that says why it landed where
// it did, so a decision is visible without opening a trace.

import { useEffect, useMemo, useState } from "react";
import {
  IconCircleDashed,
  IconGripVertical,
  IconInbox,
  IconLoader2,
  IconPlus,
  IconReload,
  IconSparkles,
  IconTargetArrow,
  IconX,
} from "@tabler/icons-react";

import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import {
  closeDelegate,
  delegateAdd,
  delegateAssign,
  delegateAuto,
  delegateDelete,
  delegateImport,
  delegateState,
  openDelegate,
  useChat,
  type DelegateAgent,
  type DelegateSnapshot,
  type DelegateTask,
} from "../bridge";

const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";
const label = "text-[12px] font-medium text-muted-foreground";

/** "claude-sonnet-5-5" -> "Sonnet 5.5", "gpt-6-sol" -> "GPT 6 Sol". */
export function prettyModel(id: string): string {
  const s = id
    .replace(/^claude-/, "")
    .replace(/-(\d+)-(\d+)$/, " $1.$2")
    .replace(/-/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
  return s.replace(/^Gpt\b/, "GPT");
}

/** "13:31", the clock time a queue would finish. */
function clock(ms: number): string {
  return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
}

function money(n: number): string {
  return `$${n.toFixed(2)}`;
}

/** A bot's face: the mockup's squircle, tinted from the bot's own colour. */
function Face({ agent, size = 40 }: { agent: DelegateAgent; size?: number }) {
  const color = agent.color || "#64748b";
  return (
    <span
      className="grid shrink-0 place-items-center rounded-2xl ring-1 ring-black/[0.04] dark:ring-white/[0.06]"
      style={{ width: size, height: size, background: `${color}1f`, color, fontSize: size * 0.46 }}
      aria-hidden
    >
      {agent.emoji || agent.name.slice(0, 1).toUpperCase()}
    </span>
  );
}

export function DelegateDialog() {
  const { delegateOpen, agents } = useChat();
  const [snap, setSnap] = useState<DelegateSnapshot | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);
  const [draft, setDraft] = useState("");
  const [est, setEst] = useState(10);
  const [drag, setDrag] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);

  // Open the sheet: fetch the board. Closing keeps the last one so reopening
  // paints immediately while the fresh answer is in flight.
  useEffect(() => {
    if (!delegateOpen) return;
    let live = true;
    setErr("");
    delegateState()
      .then((s) => live && setSnap(s))
      .catch((e) => live && setErr(String(e)));
    return () => {
      live = false;
    };
  }, [delegateOpen]);

  // A bot deleted or made on the other side of the app should not leave a
  // stale row: agents are part of the store, so re-ask when they change.
  useEffect(() => {
    if (!delegateOpen) return;
    delegateState()
      .then(setSnap)
      .catch(() => {});
  }, [delegateOpen, agents.length]);

  const run = async (p: Promise<DelegateSnapshot>) => {
    setBusy(true);
    try {
      setSnap(await p);
      setErr("");
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const byId = useMemo(() => new Map((snap?.agents ?? []).map((a) => [a.id, a])), [snap]);

  const addTask = async (e: React.FormEvent) => {
    e.preventDefault();
    const title = draft.trim();
    if (!title) return;
    const min = Number.isFinite(est) ? Math.max(1, Math.min(480, Math.round(est))) : 10;
    setDraft("");
    await run(delegateAdd(title, min));
  };

  const dropOn = (agent: string | null) => (e: React.DragEvent) => {
    e.preventDefault();
    const id = e.dataTransfer.getData("text/plain") || drag;
    setDrag(null);
    setOver(null);
    if (id) void run(delegateAssign(id, agent));
  };

  const allowDrop = (key: string) => (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    setOver(key);
  };

  const tasks = snap?.unassigned ?? [];

  return (
    <Dialog
      open={delegateOpen}
      onOpenChange={(o) => {
        if (o) openDelegate();
        else closeDelegate();
      }}
    >
      <DialogContent
        className="top-[6vh] max-h-[88vh] w-[calc(100vw-2rem)] overflow-hidden p-0 sm:max-w-[920px]"
        backdropClassName="bg-black/55 backdrop-blur-[2px]"
      >
        <div className="flex max-h-[88vh] min-h-[520px] flex-col">
          {/* Header: the sheet's name, the one instruction, and the switch. */}
          <DialogHeader className="flex flex-row items-center gap-2.5 border-b border-border/60 px-5 py-4">
            <IconSparkles size={17} className="shrink-0 text-sky-500" aria-hidden />
            <div className="flex min-w-0 flex-1 flex-col">
              <DialogTitle className="text-[15px] font-semibold leading-tight">Delegate</DialogTitle>
              <DialogDescription className="mt-0.5 text-[12.5px] leading-tight text-muted-foreground">
                Drag a task onto an agent
              </DialogDescription>
            </div>
            {busy && <IconLoader2 size={14} className="shrink-0 animate-spin text-muted-foreground" aria-hidden />}
            <button
              type="button"
              onClick={() => void run(delegateImport())}
              title="Import the open project's tickets"
              aria-label="Import the open project's tickets"
              className="grid size-7 shrink-0 cursor-pointer place-items-center rounded-full text-muted-foreground hover:bg-accent hover:text-foreground"
            >
              <IconReload size={14} />
            </button>
            <span className={`${label} flex select-none items-center gap-2 pl-1`}>
              Auto
              <button
                type="button"
                role="switch"
                data-delegate-auto
                aria-checked={!!snap?.auto}
                aria-label="Assign new tasks automatically"
                disabled={busy || !snap}
                onClick={() => void run(delegateAuto(!snap?.auto))}
                className={`relative h-[22px] w-[38px] cursor-pointer rounded-full transition-colors disabled:cursor-default disabled:opacity-50 ${
                  snap?.auto ? "bg-sky-500" : "bg-foreground/15"
                }`}
              >
                <span
                  className={`absolute top-[3px] size-4 rounded-full bg-white shadow-sm transition-[left] ${
                    snap?.auto ? "left-[19px]" : "left-[3px]"
                  }`}
                />
              </button>
            </span>
          </DialogHeader>

          {err && (
            <div className="border-b border-border/60 bg-red-500/10 px-5 py-2 text-[12px] text-red-600 dark:text-red-300">{err}</div>
          )}

          <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,36%)_minmax(0,1fr)]">
            {/* Unassigned: the drop target for taking work back. */}
            <aside className="flex min-h-0 flex-col overflow-y-auto p-3.5">
              <div
                data-delegate-drop="unassigned"
                onDragOver={allowDrop("unassigned")}
                onDragLeave={() => setOver(null)}
                onDrop={dropOn(null)}
                className={`flex flex-1 flex-col gap-2 rounded-2xl bg-well p-2.5 ${WELL} ${
                  over === "unassigned" && drag ? "ring-2 ring-sky-500/40" : ""
                }`}
              >
                <div className="flex items-center gap-1.5 px-1.5 pb-0.5 pt-1">
                  <span className={`${label} text-[12.5px]`}>Unassigned</span>
                  <span className="grid min-w-[18px] place-items-center rounded-md bg-background px-1 py-px text-[11px] font-semibold text-muted-foreground shadow-sm ring-1 ring-black/[0.04] dark:ring-white/[0.06]">
                    {tasks.length}
                  </span>
                  <button
                    type="button"
                    onClick={() => void run(delegateImport())}
                    title="Import the open project's tickets"
                    className="ml-auto cursor-pointer rounded-md px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground/70 hover:bg-accent hover:text-foreground"
                  >
                    From plan
                  </button>
                </div>

                {tasks.length === 0 && (
                  <div className="flex flex-1 flex-col items-center justify-center gap-1.5 px-4 py-10 text-center">
                    <IconInbox size={22} className="text-muted-foreground/40" aria-hidden />
                    <p className="text-[12.5px] text-muted-foreground">Everything is with a bot.</p>
                  </div>
                )}

                {tasks.map((t) => (
                  <TaskCard
                    key={t.id}
                    task={t}
                    agents={snap?.agents ?? []}
                    dragging={drag === t.id}
                    onDragStart={(e) => {
                      e.dataTransfer.setData("text/plain", t.id);
                      e.dataTransfer.effectAllowed = "move";
                      setDrag(t.id);
                    }}
                    onDragEnd={() => {
                      setDrag(null);
                      setOver(null);
                    }}
                    onAssign={(agent) => void run(delegateAssign(t.id, agent))}
                    onDelete={() => void run(delegateDelete(t.id))}
                  />
                ))}

                {/* New task: title, then minutes, then Enter. */}
                <form onSubmit={addTask} className="mt-auto flex items-center gap-1.5 pt-1">
                  <input
                    value={draft}
                    onChange={(e) => setDraft(e.target.value)}
                    placeholder="Add a task…"
                    aria-label="New task"
                    className="h-8 min-w-0 flex-1 rounded-lg bg-background px-2.5 text-[12.5px] outline-none ring-1 ring-black/[0.04] placeholder:text-muted-foreground/60 dark:ring-white/[0.06]"
                  />
                  <label className="flex h-8 items-center gap-1 rounded-lg bg-background px-1.5 text-[11.5px] text-muted-foreground ring-1 ring-black/[0.04] dark:ring-white/[0.06]">
                    <input
                      type="number"
                      min={1}
                      max={480}
                      value={est}
                      onChange={(e) => setEst(Number(e.target.value))}
                      aria-label="Minutes"
                      className="w-8 bg-transparent text-right outline-none"
                    />
                    m
                  </label>
                  <button
                    type="submit"
                    disabled={!draft.trim()}
                    aria-label="Add task"
                    className="grid size-8 shrink-0 cursor-pointer place-items-center rounded-lg bg-foreground text-background disabled:cursor-default disabled:opacity-30"
                  >
                    <IconPlus size={15} />
                  </button>
                </form>
              </div>
            </aside>

            {/* The fleet, each bot with its model, minute price and queue. */}
            <section className="min-h-0 overflow-y-auto border-l border-border/50 p-3.5">
              <div className="flex items-baseline justify-between px-1.5 pb-1.5 pt-1">
                <span className={`${label} text-[12.5px]`}>Agents</span>
                <span className="text-[11.5px] text-muted-foreground/80">
                  {snap && snap.queues_usd > 0 ? `${money(snap.queues_usd)} in queues` : "nothing queued"}
                </span>
              </div>

              {snap && snap.agents.length === 0 && (
                <div className="flex flex-col items-center gap-2 rounded-2xl bg-well px-4 py-12 text-center">
                  <IconSparkles size={20} className="text-muted-foreground/40" aria-hidden />
                  <p className="text-[12.5px] text-muted-foreground">
                    No bots yet. Make one in Chat → Agents, then give it work here.
                  </p>
                </div>
              )}

              <div className="flex flex-col">
                {snap?.agents.map((a) => (
                  <AgentRow
                    key={a.id}
                    agent={a}
                    over={over === a.id && !!drag}
                    onDragOver={allowDrop(a.id)}
                    onDragLeave={() => setOver(null)}
                    onDrop={dropOn(a.id)}
                    dragging={drag}
                    onUnassign={(id) => void run(delegateAssign(id, null))}
                    onDragTask={(e, id) => {
                      e.dataTransfer.setData("text/plain", id);
                      e.dataTransfer.effectAllowed = "move";
                      setDrag(id);
                    }}
                    onDragTaskEnd={() => {
                      setDrag(null);
                      setOver(null);
                    }}
                  />
                ))}
              </div>
            </section>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function TaskCard({
  task,
  agents,
  dragging,
  onDragStart,
  onDragEnd,
  onAssign,
  onDelete,
}: {
  task: DelegateTask;
  agents: DelegateAgent[];
  dragging: boolean;
  onDragStart: (e: React.DragEvent) => void;
  onDragEnd: () => void;
  onAssign: (agent: string) => void;
  onDelete: () => void;
}) {
  return (
    <article
      draggable
      data-delegate-task={task.id}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      className={`group/card relative flex cursor-grab flex-col gap-1 rounded-xl bg-background px-3 py-2.5 text-left shadow-sm ring-1 ring-black/[0.05] active:cursor-grabbing dark:ring-white/[0.07] ${
        dragging ? "opacity-50" : ""
      }`}
    >
      <div className="flex items-center gap-1.5">
        <IconGripVertical size={13} className="-ml-1 shrink-0 text-muted-foreground/35" aria-hidden />
        <span className="min-w-0 flex-1 truncate text-[13px] font-semibold leading-snug">{task.title}</span>
        <button
          type="button"
          onClick={onDelete}
          aria-label={`Delete ${task.title}`}
          className="grid size-5 shrink-0 cursor-pointer place-items-center rounded-md text-muted-foreground/50 opacity-0 hover:bg-accent hover:text-foreground group-hover/card:opacity-100 focus:opacity-100"
        >
          <IconX size={12} />
        </button>
      </div>
      <div className="flex items-center gap-1.5 pl-[18px] text-[11.5px] text-muted-foreground">
        <IconTargetArrow size={12} className="shrink-0 text-orange-500/80" aria-hidden />
        <span>~{task.est_min} min</span>
        {task.ticket && <span className="rounded bg-well px-1 text-[10.5px] text-muted-foreground/80">{task.ticket}</span>}
        {/* Keyboard and touch path: drag is the fast one, not the only one. */}
        <select
          value={task.agent ?? ""}
          onChange={(e) => e.target.value && onAssign(e.target.value)}
          aria-label={`Assign ${task.title}`}
          className="ml-auto h-5 max-w-[110px] cursor-pointer rounded-md bg-transparent text-[11px] text-muted-foreground/70 opacity-0 outline-none transition-opacity hover:text-foreground focus:opacity-100 group-hover/card:opacity-100"
        >
          <option value="">Assign to…</option>
          {agents.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
            </option>
          ))}
        </select>
      </div>
    </article>
  );
}

function AgentRow({
  agent,
  over,
  dragging,
  onDragOver,
  onDragLeave,
  onDrop,
  onUnassign,
  onDragTask,
  onDragTaskEnd,
}: {
  agent: DelegateAgent;
  over: boolean;
  dragging: string | null;
  onDragOver: (e: React.DragEvent) => void;
  onDragLeave: () => void;
  onDrop: (e: React.DragEvent) => void;
  onUnassign: (id: string) => void;
  onDragTask: (e: React.DragEvent, id: string) => void;
  onDragTaskEnd: () => void;
}) {
  const busy = agent.busy_until > 0;
  return (
    <div
      data-delegate-agent={agent.id}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      className={`flex flex-col gap-1.5 rounded-xl px-1.5 py-2 transition-colors ${
        over ? "bg-sky-500/10 ring-2 ring-sky-500/40" : "hover:bg-accent/40"
      }`}
    >
      <div className="flex items-start gap-2.5">
        <Face agent={agent} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="truncate text-[14px] font-semibold leading-tight">{agent.name}</span>
          </div>
          <div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[11.5px] text-muted-foreground">
            <span className="truncate">{prettyModel(agent.model)}</span>
            <span className="text-muted-foreground/40">·</span>
            <span className="shrink-0">{agent.free ? "free" : `~${money(agent.usd_per_min)}/min`}</span>
          </div>
        </div>
        {busy ? (
          <span
            className="shrink-0 rounded-md bg-well px-2 py-1 text-[11px] font-medium text-muted-foreground"
            title={`Its queue is about ${agent.queued_min} min of work`}
          >
            Busy until {clock(agent.busy_until)}
          </span>
        ) : (
          <span className="shrink-0 rounded-md bg-emerald-500/12 px-2 py-1 text-[11px] font-medium text-emerald-700 dark:text-emerald-300">
            Free
          </span>
        )}
      </div>

      {agent.tasks.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5 pl-[50px]">
          {agent.tasks.map((t, i) => (
            <span
              key={t.id}
              draggable
              data-delegate-chip={t.id}
              onDragStart={(e) => onDragTask(e, t.id)}
              onDragEnd={onDragTaskEnd}
              title={t.note || t.title}
              className={`group/chip flex max-w-full cursor-grab items-center gap-1 rounded-lg px-2 py-1 text-[11.5px] font-medium ring-1 active:cursor-grabbing ${
                dragging === t.id
                  ? "opacity-40"
                  : i === 0
                    ? "bg-sky-500/10 text-sky-700 ring-sky-500/20 dark:text-sky-300"
                    : "bg-background text-foreground-soft ring-black/[0.05] dark:ring-white/[0.07]"
              }`}
            >
              <IconCircleDashed size={11} className="shrink-0 opacity-60" aria-hidden />
              <span className="truncate">{t.title}</span>
              <button
                type="button"
                onClick={() => onUnassign(t.id)}
                aria-label={`Take ${t.title} back`}
                className="grid size-4 shrink-0 cursor-pointer place-items-center rounded text-muted-foreground/40 opacity-0 hover:text-foreground group-hover/chip:opacity-100"
              >
                <IconX size={10} />
              </button>
            </span>
          ))}
          <span className="text-[11px] text-muted-foreground/60" title={`~${agent.queued_min} min queued at ${money(agent.usd_per_min)}/min`}>
            {money(agent.cost_usd)}
          </span>
        </div>
      )}
    </div>
  );
}
