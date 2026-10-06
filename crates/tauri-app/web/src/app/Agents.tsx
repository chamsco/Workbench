// The Agents side of Chat (after Noodle): agents with a name, a job and
// their own folder and computer; one-to-one threads and groups with a goal.
// Built from the same Whirl pieces as the rest of the chat face: wells,
// row pills, the frosted dialog.

import { useEffect, useMemo, useRef, useState } from "react";
import {
  IconCloudComputing,
  IconDeviceDesktop,
  IconFolder,
  IconLoader2,
  IconPlayerPlay,
  IconPlayerStop,
  IconPlus,
  IconServer,
  IconUsersGroup,
  IconUserPlus,
} from "@tabler/icons-react";

import { RowPill } from "@/components/row-pill";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { RoutePicker } from "./RoutePicker";
import {
  agentById,
  computer,
  createGroup,
  deleteAgent,
  editAgent,
  editGroup,
  getHost,
  openAgent,
  openThread,
  saveAgent,
  useChat,
  type Agent,
  type Computer,
  type ComputerStatus,
  type Route,
  type Thread,
  type ThreadInfo,
} from "../bridge";

const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";
const COLORS = ["#f97316", "#e11d48", "#8b5cf6", "#2563eb", "#0891b2", "#16a34a", "#ca8a04", "#64748b"];
const EMOJI = ["🧭", "✍️", "🔬", "🎬", "🧪", "📈", "🛠️", "🎨", "📚", "🤖", "🦊", "🐙"];

const blankComputer = (): Computer => ({ kind: "folder", image: "", desktop: false, host: "", user: "", port: 0, key: "", desktop_url: "" });

export function Avatar({ agent, size = 32, ring = false }: { agent?: Agent; size?: number; ring?: boolean }) {
  const color = agent?.avatar.color || "#64748b";
  const emoji = agent?.avatar.emoji;
  return (
    <span
      className={`inline-grid shrink-0 place-items-center rounded-full font-semibold text-white ${ring ? "ring-2 ring-(--surface)" : ""}`}
      style={{ width: size, height: size, background: color, fontSize: emoji ? size * 0.5 : size * 0.42 }}
      aria-hidden
    >
      {emoji || (agent?.name || "?").slice(0, 1).toUpperCase()}
    </span>
  );
}

function Stack({ ids, size = 22 }: { ids: string[]; size?: number }) {
  return (
    <span className="flex -space-x-2">
      {ids.slice(0, 3).map((id) => (
        <Avatar key={id} agent={agentById(id)} size={size} ring />
      ))}
    </span>
  );
}

const computerLabel = (c: Computer, remote = false) =>
  remote ? "Lives elsewhere (A2A)" : c.kind === "docker" ? (c.desktop ? "Own desktop (Docker)" : "Own Linux computer (Docker)") : c.kind === "ssh" ? `Server ${c.host}` : "Its folder on this machine";
const ComputerIcon = ({ c, size = 14 }: { c: Computer; size?: number }) =>
  c.kind === "docker" ? <IconDeviceDesktop size={size} /> : c.kind === "ssh" ? <IconServer size={size} /> : <IconFolder size={size} />;

// ---------------------------------------------------------------- sidebar

export function AgentsSide() {
  const { agents, threads, id } = useChat();
  const groups = threads.filter((t) => (t.members?.length ?? 0) > 0 && !t.app);
  const lastWith = (a: Agent) => threads.filter((t) => t.agent === a.id).sort((x, y) => y.updated - x.updated)[0];
  const row = "group/row relative flex h-11 w-full cursor-pointer items-center gap-2.5 rounded-lg px-2 text-left";
  return (
    <div className="whirl-side flex flex-col gap-3">
      <div className="flex flex-col gap-0.5">
        <button type="button" onClick={() => editAgent("new")} className="group/row relative flex h-8 w-full cursor-pointer items-center gap-2.5 rounded-lg px-2.5 text-[13.5px]/4 font-medium text-foreground-soft">
          <RowPill />
          <IconUserPlus size={16} className="relative shrink-0" />
          <span className="relative">New agent</span>
        </button>
        <button type="button" onClick={() => editGroup(true)} disabled={agents.length === 0} className="group/row relative flex h-8 w-full cursor-pointer items-center gap-2.5 rounded-lg px-2.5 text-[13.5px]/4 font-medium text-foreground-soft disabled:cursor-default disabled:opacity-50">
          <RowPill />
          <IconUsersGroup size={16} className="relative shrink-0" />
          <span className="relative">New group</span>
        </button>
      </div>
      <section>
        <div className="flex h-5 items-center px-2.5 text-[10.5px]/4 font-medium text-muted-foreground/55">Agents</div>
        {agents.length === 0 && <div className="px-2.5 py-1.5 text-[12.5px]/5 text-muted-foreground">No agents yet. Give one a name and a job.</div>}
        {agents.map((a) => {
          const t = lastWith(a);
          const on = !!t && t.id === id;
          return (
            <button key={a.id} type="button" className={row} onClick={() => void openAgent(a)} title={a.job}>
              <RowPill className={on ? "bg-accent" : ""} />
              <span className="relative">
                <Avatar agent={a} size={28} />
                {t?.busy && <span className="absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full bg-emerald-500 ring-2 ring-(--surface)" />}
              </span>
              <span className="relative flex min-w-0 flex-1 flex-col">
                <span className="truncate text-[13.5px]/4 font-medium text-foreground">{a.name}</span>
                <span className="truncate text-[11.5px]/4 text-muted-foreground">{t?.preview || a.job || "No messages yet"}</span>
              </span>
              {t?.unread && !on && <span className="relative size-2 shrink-0 rounded-full bg-sky-500" />}
            </button>
          );
        })}
      </section>
      {groups.length > 0 && (
        <section>
          <div className="flex h-5 items-center px-2.5 text-[10.5px]/4 font-medium text-muted-foreground/55">Groups</div>
          {groups.map((g) => (
            <button key={g.id} type="button" className={row} onClick={() => void openThread(g.id)}>
              <RowPill className={g.id === id ? "bg-accent" : ""} />
              <span className="relative">
                <Stack ids={g.members ?? []} size={20} />
              </span>
              <span className="relative flex min-w-0 flex-1 flex-col">
                <span className="truncate text-[13.5px]/4 font-medium text-foreground">{g.title}</span>
                <span className="truncate text-[11.5px]/4 text-muted-foreground">{g.preview || `${g.members?.length ?? 0} agents`}</span>
              </span>
              {g.busy && <IconLoader2 size={14} className="relative animate-spin text-muted-foreground" />}
            </button>
          ))}
        </section>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- home

export function AgentsHome() {
  const { agents, threads } = useChat();
  const groups = threads.filter((t) => (t.members?.length ?? 0) > 0 && !t.app);
  return (
    <div className="h-full overflow-y-auto px-6 pt-10 pb-16">
      <div className="mx-auto flex max-w-3xl flex-col gap-6">
        <div>
          <h1 className="text-[24px]/8 font-medium tracking-tight">Your agents</h1>
          <p className="mt-1 max-w-[60ch] text-[13.5px]/6 text-muted-foreground">
            Give each one a name and a job. Talk to one, or put several in a group with a goal: they read each other's messages and split the work. Each works in its own folder, or on its own computer.
          </p>
        </div>
        <div className="grid grid-cols-1 gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
          {agents.map((a) => (
            <AgentCard key={a.id} agent={a} />
          ))}
          <button type="button" onClick={() => editAgent("new")} className="flex min-h-36 cursor-pointer flex-col items-center justify-center gap-2 rounded-2xl border-[1.5px] border-dashed border-border text-[13.5px] font-medium text-muted-foreground transition-colors hover:border-foreground/30 hover:text-foreground">
            <IconPlus size={20} />
            New agent
          </button>
        </div>
        {agents.length > 1 && (
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <h2 className="text-[14px] font-semibold">Groups</h2>
              <button type="button" onClick={() => editGroup(true)} className="flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-1.5 text-[13px] font-medium hover:bg-accent">
                <IconUsersGroup size={15} /> New group
              </button>
            </div>
            {groups.length === 0 ? (
              <div className={`rounded-2xl bg-well px-4 py-3 text-[13px]/5 text-muted-foreground ${WELL}`}>
                A group is one conversation with a whole team: a planner, a writer and a researcher in the same thread, and you step in whenever you like.
              </div>
            ) : (
              groups.map((g) => (
                <button key={g.id} type="button" onClick={() => void openThread(g.id)} className={`flex cursor-pointer items-center gap-3 rounded-2xl bg-well px-4 py-3 text-left ${WELL}`}>
                  <Stack ids={g.members ?? []} size={26} />
                  <span className="flex min-w-0 flex-col">
                    <span className="truncate text-[14px] font-medium">{g.title}</span>
                    <span className="truncate text-[12.5px] text-muted-foreground">{g.preview || (g.members ?? []).map((m) => agentById(m)?.name).filter(Boolean).join(", ")}</span>
                  </span>
                </button>
              ))
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function AgentCard({ agent }: { agent: Agent }) {
  const host = getHost();
  return (
    <div className={`flex min-h-36 flex-col gap-2 rounded-2xl bg-well p-4 ${WELL}`}>
      <div className="flex items-center gap-3">
        <Avatar agent={agent} size={36} />
        <div className="flex min-w-0 flex-col">
          <span className="truncate text-[14.5px] font-semibold">{agent.name}</span>
          <span className="truncate text-[11.5px] text-muted-foreground">{host.routeName(agent.route)}</span>
        </div>
      </div>
      <p className="line-clamp-2 flex-1 text-[12.5px]/5 text-muted-foreground">{agent.job || "No job yet."}</p>
      <div className="flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
        <ComputerIcon c={agent.computer} size={13} />
        <span className="truncate">{computerLabel(agent.computer, agent.route.kind === "a2a")}</span>
      </div>
      <div className="flex items-center gap-1.5">
        <button type="button" onClick={() => void openAgent(agent)} className="cursor-pointer rounded-full bg-primary px-3 py-1.5 text-[12.5px] font-medium text-primary-foreground hover:bg-(--primary-hover)">
          Message
        </button>
        <button type="button" onClick={() => editAgent(agent)} className="cursor-pointer rounded-full px-3 py-1.5 text-[12.5px] font-medium hover:bg-accent">
          Edit
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- thread header

export function AgentThreadHeader({ thread }: { thread: Thread }) {
  const a = agentById(thread.agent);
  const members = thread.members ?? [];
  if (!a && members.length === 0) return null;
  return (
    <div className="flex flex-col items-center gap-1 pt-8 pb-4 text-center">
      {a ? <Avatar agent={a} size={56} /> : <Stack ids={members} size={40} />}
      <div className="mt-1.5 text-[19px]/7 font-semibold tracking-tight">{a ? a.name : thread.title}</div>
      <div className="max-w-md text-[13px]/5 text-muted-foreground">{a ? a.job : thread.goal}</div>
      {!a && <div className="text-[12.5px]/5 text-muted-foreground">{members.map((m) => agentById(m)?.name ?? "removed").join(", ")}</div>}
      {a && <ComputerBar agent={a} />}
    </div>
  );
}

function ComputerBar({ agent }: { agent: Agent }) {
  const { epoch } = useChat();
  const [st, setSt] = useState<ComputerStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const host = getHost();
  const last = useRef(0);
  useEffect(() => {
    let live = true;
    // Again as the thread moves (a reply may have started the computer),
    // but at most every few seconds: each check runs `docker inspect`.
    const now = Date.now();
    if (agent.computer.kind !== "folder" && now - last.current > 3000) {
      last.current = now;
      void computer(agent.id, "status").then((s) => live && setSt(s)).catch(() => {});
    }
    return () => {
      live = false;
    };
  }, [agent.id, agent.computer.kind, epoch]);
  if (agent.computer.kind === "folder") return null;
  const act = async (a: "start" | "stop") => {
    setBusy(true);
    try {
      setSt(await computer(agent.id, a));
    } catch (e) {
      host.toast(String(e), "err");
    } finally {
      setBusy(false);
    }
  };
  const chip = "flex cursor-pointer items-center gap-1 rounded-full px-2.5 py-1 text-[12px] font-medium hover:bg-accent disabled:opacity-50";
  return (
    <div className="mt-2 flex items-center gap-1 text-[12px] text-muted-foreground">
      <ComputerIcon c={agent.computer} />
      <span>{computerLabel(agent.computer, agent.route.kind === "a2a")}</span>
      {st && <span className="rounded-full bg-black/[0.05] px-2 py-0.5 dark:bg-white/[0.08]">{st.state}</span>}
      {agent.computer.kind === "docker" && st?.state !== "running" && (
        <button type="button" disabled={busy} className={chip} onClick={() => void act("start")}>
          {busy ? <IconLoader2 size={13} className="animate-spin" /> : <IconPlayerPlay size={13} />} Start
        </button>
      )}
      {agent.computer.kind === "docker" && st?.state === "running" && (
        <button type="button" disabled={busy} className={chip} onClick={() => void act("stop")}>
          <IconPlayerStop size={13} /> Stop
        </button>
      )}
      {st?.desktop && (
        <button type="button" className={chip} onClick={() => host.openComputer(st.desktop!)}>
          <IconCloudComputing size={13} /> Open desktop
        </button>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- editors

export function AgentEditors() {
  const { editing, grouping } = useChat();
  return (
    <>
      {editing && <AgentEditor key={editing === "new" ? "new" : editing.id} agent={editing === "new" ? null : editing} />}
      {grouping && <GroupEditor />}
    </>
  );
}

const field = `h-9 w-full rounded-xl bg-well px-3 text-[13.5px] outline-none ${WELL}`;
const label = "text-[12px] font-medium text-muted-foreground";

function AgentEditor({ agent }: { agent: Agent | null }) {
  const host = getHost();
  const [a, setA] = useState<Agent>(
    () =>
      agent ?? {
        id: "",
        name: "",
        job: "",
        avatar: { emoji: EMOJI[Math.floor(Math.random() * EMOJI.length)], color: COLORS[Math.floor(Math.random() * COLORS.length)] },
        route: (host.defaultRoute() ?? { kind: "cli", provider: "claude", model: null }) as Route,
        shared: [],
        computer: blankComputer(),
        memory: true,
        created: 0,
        updated: 0,
      },
  );
  const [err, setErr] = useState("");
  const [confirmDel, setConfirmDel] = useState(false);
  const set = (p: Partial<Agent>) => setA((x) => ({ ...x, ...p }));
  const setC = (p: Partial<Computer>) => setA((x) => ({ ...x, computer: { ...x.computer, ...p } }));
  const save = async () => {
    try {
      const saved = await saveAgent(a);
      editAgent(null);
      if (!agent) await openAgent(saved);
    } catch (e) {
      setErr(String(e));
    }
  };
  const kinds: [Computer["kind"], string, string][] = [
    ["folder", "This machine", "Works in its own folder here"],
    ["docker", "Its own computer", "A Docker container here, kept between uses"],
    ["ssh", "A VPS", "Any server you can SSH to"],
  ];
  return (
    <Dialog open onOpenChange={(o) => !o && editAgent(null)}>
      <DialogContent className="max-h-[86vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{agent ? `Edit ${agent.name}` : "New agent"}</DialogTitle>
          <DialogDescription>A name, a job, and where it works. It remembers what you do together.</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3.5">
          <div className="flex items-end gap-3">
            <Avatar agent={a} size={48} />
            <div className="flex flex-1 flex-col gap-1">
              <span className={label}>Name</span>
              <input autoFocus className={field} value={a.name} maxLength={40} placeholder="Kira" onChange={(e) => set({ name: e.target.value })} />
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-1">
            {EMOJI.map((e) => (
              <button key={e} type="button" onClick={() => set({ avatar: { ...a.avatar, emoji: e } })} className={`grid size-8 cursor-pointer place-items-center rounded-full text-[16px] ${a.avatar.emoji === e ? "bg-foreground/10" : "hover:bg-accent"}`}>
                {e}
              </button>
            ))}
            <span className="mx-1 h-5 w-px bg-border" />
            {COLORS.map((c) => (
              <button key={c} type="button" aria-label={`Colour ${c}`} onClick={() => set({ avatar: { ...a.avatar, color: c } })} className={`size-6 cursor-pointer rounded-full ${a.avatar.color === c ? "ring-2 ring-foreground ring-offset-2 ring-offset-(--surface)" : ""}`} style={{ background: c }} />
            ))}
          </div>
          <div className="flex flex-col gap-1">
            <span className={label}>Job and backstory</span>
            <textarea className={`${field} h-auto min-h-24 py-2 leading-5`} value={a.job} placeholder="Plans launches. Breaks a goal into a week of steps, keeps everyone on the date, and asks before changing scope." onChange={(e) => set({ job: e.target.value })} />
          </div>
          <div className="flex items-center justify-between gap-2">
            <span className={label}>Runs on</span>
            <RoutePicker value={a.route} onValueChange={(r) => set({ route: r })} />
          </div>
          <div className="flex flex-col gap-1.5">
            <span className={label}>Its computer</span>
            <div className="grid grid-cols-3 gap-1.5">
              {kinds.map(([k, t, d]) => (
                <button key={k} type="button" onClick={() => setC({ kind: k })} className={`flex cursor-pointer flex-col items-start gap-0.5 rounded-xl p-2.5 text-left ${a.computer.kind === k ? "bg-foreground text-(--surface)" : `bg-well ${WELL}`}`}>
                  <span className="text-[12.5px] font-semibold">{t}</span>
                  <span className={`text-[11px]/4 ${a.computer.kind === k ? "opacity-80" : "text-muted-foreground"}`}>{d}</span>
                </button>
              ))}
            </div>
            {a.computer.kind === "docker" && (
              <div className="flex flex-col gap-1.5">
                <input className={field} value={a.computer.image} placeholder={a.computer.desktop ? "lscr.io/linuxserver/webtop:ubuntu-xfce" : "ubuntu:24.04"} onChange={(e) => setC({ image: e.target.value })} />
                <label className="flex items-center gap-2 text-[12.5px]">
                  <input type="checkbox" checked={a.computer.desktop} onChange={(e) => setC({ desktop: e.target.checked })} />
                  A full desktop you can open (web desktop image, about 1.5 GB)
                </label>
              </div>
            )}
            {a.computer.kind === "ssh" && (
              <div className="grid grid-cols-[1fr_7rem_5rem] gap-1.5">
                <input className={field} value={a.computer.host} placeholder="vps.example.com" onChange={(e) => setC({ host: e.target.value })} />
                <input className={field} value={a.computer.user} placeholder="root" onChange={(e) => setC({ user: e.target.value })} />
                <input className={field} value={a.computer.port || ""} placeholder="22" onChange={(e) => setC({ port: Number(e.target.value) || 0 })} />
                <input className={`${field} col-span-3`} value={a.computer.key} placeholder="~/.ssh/id_ed25519 (optional)" onChange={(e) => setC({ key: e.target.value })} />
                <input className={`${field} col-span-3`} value={a.computer.desktop_url} placeholder="Web desktop URL on the VPS (optional, e.g. noVNC)" onChange={(e) => setC({ desktop_url: e.target.value })} />
              </div>
            )}
          </div>
          <div className="flex flex-col gap-1">
            <span className={label}>Folders it may also use (one per line)</span>
            <textarea className={`${field} h-auto min-h-14 py-2 font-mono text-[12px] leading-5`} value={a.shared.join("\n")} placeholder="~/Projects/almanac" onChange={(e) => set({ shared: e.target.value.split("\n").map((s) => s.trim()).filter(Boolean) })} />
          </div>
          <label className="flex items-center gap-2 text-[12.5px]">
            <input type="checkbox" checked={a.memory} onChange={(e) => set({ memory: e.target.checked })} />
            Give it your memory notes (global and its own){a.route.kind === "a2a" ? ". They'd leave this machine." : ""}
          </label>
          {err && <div className="text-[12.5px] text-red-500">{err}</div>}
        </div>
        <DialogFooter>
          {agent && (
            <Button variant="ghost" className="mr-auto text-red-500" onClick={() => (confirmDel ? void deleteAgent(agent.id).then(() => editAgent(null)) : setConfirmDel(true))}>
              {confirmDel ? `Delete ${agent.name} and its folder?` : "Delete"}
            </Button>
          )}
          <Button variant="ghost" onClick={() => editAgent(null)}>
            Cancel
          </Button>
          <Button onClick={() => void save()}>{agent ? "Save" : "Create"}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function GroupEditor() {
  const { agents } = useChat();
  const [title, setTitle] = useState("");
  const [goal, setGoal] = useState("");
  const [members, setMembers] = useState<string[]>(() => agents.slice(0, 3).map((a) => a.id));
  const [err, setErr] = useState("");
  const toggle = (id: string) => setMembers((m) => (m.includes(id) ? m.filter((x) => x !== id) : [...m, id]));
  const ordered = useMemo(() => members.filter((m) => agents.some((a) => a.id === m)), [members, agents]);
  return (
    <Dialog open onOpenChange={(o) => !o && editGroup(false)}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>New group</DialogTitle>
          <DialogDescription>One conversation, a whole team. They take turns in the order you pick them.</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <input autoFocus className={field} value={title} placeholder="Launch week" onChange={(e) => setTitle(e.target.value)} />
          <textarea className={`${field} h-auto min-h-16 py-2 leading-5`} value={goal} placeholder="Goal: Almanac 2.0 ships Thursday, one message, one page, one number." onChange={(e) => setGoal(e.target.value)} />
          <div className="flex flex-col gap-1">
            {agents.map((a) => {
              const i = ordered.indexOf(a.id);
              return (
                <button key={a.id} type="button" onClick={() => toggle(a.id)} className={`flex cursor-pointer items-center gap-2.5 rounded-xl px-2.5 py-2 text-left ${i >= 0 ? "bg-foreground/[0.06]" : "hover:bg-accent"}`}>
                  <Avatar agent={a} size={26} />
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="text-[13.5px] font-medium">{a.name}</span>
                    <span className="truncate text-[11.5px] text-muted-foreground">{a.job}</span>
                  </span>
                  {i >= 0 && <span className="grid size-5 place-items-center rounded-full bg-foreground text-[11px] font-semibold text-(--surface)">{i + 1}</span>}
                </button>
              );
            })}
          </div>
          {err && <div className="text-[12.5px] text-red-500">{err}</div>}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => editGroup(false)}>
            Cancel
          </Button>
          <Button
            disabled={ordered.length === 0}
            onClick={() => void createGroup(title.trim() || "New group", goal.trim(), ordered).catch((e) => setErr(String(e)))}
          >
            Start group
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export type { ThreadInfo };
