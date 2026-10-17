// Projects in Chat's Agents side: a project is a folder plus the bots assigned
// to it (see docs/AGENTS-PROJECTS-ROUTINES-2026-10-17.md). Three surfaces ride
// this file — the sidebar section, the project page that collects that
// folder's bots, chats and routines, and the small sheet that creates or
// renames one. Built from the same Whirl pieces as the rest of the chat face:
// wells, row pills, the frosted dialog.

import { useState, type ReactElement, type ReactNode } from "react";
import {
  IconArrowLeft,
  IconClock,
  IconDots,
  IconFolder,
  IconMessagePlus,
  IconPencil,
  IconPlayerPlay,
  IconPlus,
  IconTrash,
  IconUserPlus,
} from "@tabler/icons-react";

import { RowPill } from "@/components/row-pill";
import { ConfirmDialog } from "@/components/confirm-dialog";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Avatar } from "./avatar";
import {
  agentStatus,
  assignProject,
  cancelProject,
  deleteAgentProject,
  editProject,
  editRoutine,
  getHost,
  nextWakeLabel,
  openAgentThread,
  openProject,
  openThread,
  routineWhen,
  runRoutine,
  saveAgentProject,
  startBot,
  useChat,
  type Agent,
  type Project,
  type ThreadInfo,
} from "../bridge";

// Copied from Agents.tsx: the same well every card on the Agents side sits in.
const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";
const field = `h-9 w-full rounded-xl bg-well px-3 text-[13.5px] outline-none ${WELL}`;
const label = "text-[12px] font-medium text-muted-foreground";
const sel = "h-8 cursor-pointer rounded-lg border border-border bg-background px-2 text-[12.5px] outline-none";
const iconBtn = "grid size-8 flex-none cursor-pointer place-items-center rounded-full text-muted-foreground hover:bg-accent hover:text-foreground";
const actionPill = "flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-1.5 text-[13px] font-medium hover:bg-accent disabled:cursor-default disabled:opacity-50";

const lastSegment = (path: string) => path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || path;

/** The folder's own name — what the sidebar says above the path, and what a
 *  project is called until the user gives it another name. */
export const projectFolderName = (p: Project): string => lastSegment(p.folder) || p.name;

/** "" is this Mac; a remote machine's name later. */
const machineName = (p: Project) => p.machine || "This Mac";

const STATUS_DOT: Record<ReturnType<typeof agentStatus>["kind"], string> = {
  working: "bg-emerald-500",
  needs: "bg-sky-500",
  wake: "bg-amber-500",
  idle: "bg-foreground/20",
};

/** A bot's job holds the one-liner and the long backstory ("one\n\nlong");
 *  cards that have room for one line show only the first. */
const oneLine = (job: string) => job.split("\n").find((l) => l.trim())?.trim() ?? "";

/** How long ago, in the quiet vocabulary the sidebar uses. */
function ago(ms: number) {
  const s = (Date.now() - ms) / 1000;
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  if (s < 7 * 86400) return `${Math.floor(s / 86400)}d`;
  return new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });
}

/** The bots assigned to a project, resolved against the live list — a deleted
 *  bot leaves an id behind in the project record, and gets skipped here. */
const botsOn = (p: Project, agents: Agent[]) =>
  p.agents.map((id) => agents.find((a) => a.id === id)).filter((a): a is Agent => !!a);

/** A bot's most recent thread with this project, which is the conversation the
 *  project page's Message opens, so its status is the one worth showing. */
const projectThread = (threads: ThreadInfo[], agent: string, folder: string) =>
  threads
    .filter((t) => t.agent === agent && (t.project ?? null) === folder)
    .sort((x, y) => y.updated - x.updated)[0];

// ---------------------------------------------------------------- sidebar

/** The Projects section of the Agents sidebar: a label with its "+", then one
 *  row per project with its machine and bot count, in the folder-row shape the
 *  owner's Kuvo reference uses. */
export function ProjectsSide(): ReactElement {
  const { projects, project } = useChat();
  return (
    <section>
      <div className="flex h-5 items-center px-2.5">
        <span className="text-[10.5px]/4 font-medium text-muted-foreground/55">Projects</span>
        <button
          type="button"
          onClick={() => editProject("new")}
          aria-label="New project"
          title="New project"
          className="ml-auto grid size-5 cursor-pointer place-items-center rounded-md text-muted-foreground/60 hover:bg-accent hover:text-foreground"
        >
          <IconPlus size={13} />
        </button>
      </div>
      {projects.length === 0 && (
        <div className="px-2.5 py-1.5 text-[12.5px]/5 text-muted-foreground">No projects yet. A project is a folder you give the bots to work in.</div>
      )}
      {projects.map((p) => (
        <ProjectRow key={p.id} project={p} selected={project?.id === p.id} />
      ))}
    </section>
  );
}

function ProjectRow({ project, selected }: { project: Project; selected: boolean }) {
  const host = getHost();
  const [menuOpen, setMenuOpen] = useState(false);
  const [removing, setRemoving] = useState(false);
  const bots = project.agents.length;
  return (
    <div
      className="group/row group/proj relative"
      onContextMenu={(e) => {
        e.preventDefault();
        setMenuOpen(true);
      }}
    >
      <button
        type="button"
        onClick={() => openProject(project)}
        aria-current={selected ? "page" : undefined}
        title={project.folder}
        className="relative flex h-11 w-full cursor-pointer items-center gap-2.5 rounded-lg py-0 pr-8 pl-2 text-left"
      >
        <RowPill className={`group-has-data-popup-open/proj:bg-accent ${selected ? "bg-accent" : ""}`} />
        <IconFolder size={16} className="relative shrink-0 text-muted-foreground" />
        <span className="relative flex min-w-0 flex-1 flex-col">
          <span className="truncate text-[13.5px]/4 font-medium text-foreground">{project.name}</span>
          <span className="truncate text-[11.5px]/4 text-muted-foreground">
            {machineName(project)} · {bots === 1 ? "1 bot" : `${bots} bots`}
          </span>
        </span>
      </button>
      <DropdownMenu open={menuOpen} onOpenChange={setMenuOpen}>
        <DropdownMenuTrigger
          aria-label={`Options for ${project.name}`}
          className="absolute top-1/2 right-1 flex size-6 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md text-foreground-soft opacity-0 transition-opacity duration-150 group-hover/proj:opacity-100 hover:text-foreground data-popup-open:opacity-100 coarse:opacity-100"
        >
          <IconDots size={16} />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="min-w-44 p-1">
          <DropdownMenuItem className="gap-2 px-2 py-1.5" onClick={() => editProject(project)}>
            <IconPencil size={15} className="text-muted-foreground" />
            Rename
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem variant="destructive" className="gap-2 px-2 py-1.5" onClick={() => setRemoving(true)}>
            <IconTrash size={15} />
            Remove
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <ConfirmDialog
        open={removing}
        onOpenChange={setRemoving}
        title="Remove this project?"
        message={`“${project.name}” goes; its bots, folder and chats stay. Chats keep working in the folder.`}
        confirmLabel="Remove"
        destructive
        onConfirm={() => void deleteAgentProject(project.id).catch((e) => host.toast(String(e), "err"))}
      />
    </div>
  );
}

// ---------------------------------------------------------------- project page

/** The main column with a project open: what the bots assigned to this folder
 *  are doing, the chats they are having here, and the routines that land here. */
export function ProjectHome(): ReactElement {
  const { agents, threads, project } = useChat();
  const host = getHost();
  const [adding, setAdding] = useState(false);
  const [chatting, setChatting] = useState(false);
  const [removing, setRemoving] = useState(false);
  if (!project) return <></>;
  const bots = botsOn(project, agents);
  // Bot chats carry the folder; groups have no single folder, so they stay out.
  const chats = threads
    .filter((t) => !!t.agent && !t.app && (t.members?.length ?? 0) === 0 && (t.project ?? null) === project.folder)
    .sort((x, y) => y.updated - x.updated);
  const routines = bots
    .flatMap((a) => a.routines.filter((r) => r.project === project.id).map((routine) => ({ agent: a, routine })))
    // A paused routine has no next wake; keep the live ones first.
    .sort((x, y) => (x.routine.enabled === y.routine.enabled ? 0 : x.routine.enabled ? -1 : 1) || (x.routine.next_run || Number.MAX_SAFE_INTEGER) - (y.routine.next_run || Number.MAX_SAFE_INTEGER));
  const fail = (e: unknown) => host.toast(String(e), "err");

  return (
    <div className="h-full overflow-y-auto px-6 pt-10 pb-16">
      <div className="mx-auto flex max-w-3xl flex-col gap-6">
        <button type="button" onClick={() => openProject(null)} className="-ml-2 flex w-fit cursor-pointer items-center gap-1.5 rounded-full px-2 py-1 text-[12.5px] font-medium text-muted-foreground hover:bg-accent hover:text-foreground">
          <IconArrowLeft size={14} />
          Bots
        </button>

        <div className="flex items-start gap-3">
          <span className={`grid size-11 flex-none place-items-center rounded-2xl bg-well text-muted-foreground ${WELL}`}>
            <IconFolder size={22} />
          </span>
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <div className="flex min-w-0 items-center gap-2">
              <h1 className="truncate text-[24px]/8 font-medium tracking-tight">{project.name}</h1>
              <span className="shrink-0 rounded-full bg-black/[0.05] px-2 py-0.5 text-[11.5px] text-muted-foreground dark:bg-white/[0.08]">{machineName(project)}</span>
            </div>
            <div className="truncate font-mono text-[11.5px]/4 text-muted-foreground" title={project.folder}>
              {project.folder}
            </div>
          </div>
          <div className="flex flex-none items-center gap-1">
            <button type="button" onClick={() => editProject(project)} className={actionPill}>
              <IconPencil size={14} />
              Edit
            </button>
            <button type="button" onClick={() => setRemoving(true)} className={`${actionPill} text-red-500`}>
              <IconTrash size={14} />
              Remove
            </button>
          </div>
        </div>

        <Section
          title="Bots on this project"
          action={
            <button type="button" onClick={() => setAdding(true)} className={actionPill}>
              <IconUserPlus size={14} />
              Add bot
            </button>
          }
        >
          {bots.length === 0 ? (
            <Empty>No bots work on this project yet. Add one you already have, or create a bot that starts here.</Empty>
          ) : (
            <div className="grid grid-cols-1 gap-2.5 sm:grid-cols-2">
              {bots.map((a) => (
                <BotCard key={a.id} agent={a} project={project} thread={projectThread(threads, a.id, project.folder)} />
              ))}
            </div>
          )}
        </Section>

        <Section
          title="Chats"
          action={
            <button type="button" onClick={() => setChatting(true)} disabled={bots.length === 0} title={bots.length === 0 ? "Add a bot to this project first" : undefined} className={actionPill}>
              <IconMessagePlus size={14} />
              New chat
            </button>
          }
        >
          {chats.length === 0 ? (
            <Empty>No chats in this project yet. Pick a bot and give it a task; it works in this folder.</Empty>
          ) : (
            <div className={`flex flex-col divide-y divide-(--well-outline) overflow-hidden rounded-2xl bg-well ${WELL}`}>
              {chats.map((t) => {
                const bot = agents.find((a) => a.id === t.agent);
                return (
                  <button key={t.id} type="button" onClick={() => void openThread(t.id)} className="flex cursor-pointer items-center gap-2.5 px-3.5 py-2.5 text-left hover:bg-accent">
                    <Avatar agent={bot} size={26} />
                    <span className="flex min-w-0 flex-1 flex-col">
                      <span className="truncate text-[13.5px] font-medium">{t.title}</span>
                      <span className="truncate text-[11.5px] text-muted-foreground">
                        {bot ? bot.name : "Removed bot"} · {ago(t.updated)}
                      </span>
                    </span>
                    {t.unread && <span className="size-2 shrink-0 rounded-full bg-sky-500" title="Unread" />}
                  </button>
                );
              })}
            </div>
          )}
        </Section>

        <Section title="Routines on this project">
          {routines.length === 0 ? (
            <Empty>No routines run in this project yet. Add one from a bot's page, then pick this project as where it runs.</Empty>
          ) : (
            <div className={`flex flex-col divide-y divide-(--well-outline) overflow-hidden rounded-2xl bg-well ${WELL}`}>
              {routines.map(({ agent, routine }) => (
                <div key={`${agent.id}/${routine.id}`} className="flex items-center gap-3 px-3.5 py-2.5">
                  <span className="grid size-8 flex-none place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
                    <IconClock size={16} />
                  </span>
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate text-[13.5px] font-medium">{routine.name || "Untitled routine"}</span>
                    <span className="truncate text-[11.5px] text-muted-foreground">
                      {agent.name} · {routineWhen(routine)}
                      {routine.enabled && routine.next_run ? ` · next wake ${nextWakeLabel(routine.next_run)}` : ""}
                    </span>
                  </span>
                  <button type="button" onClick={() => void runRoutine(agent.id, routine.id).catch(fail)} className={`${actionPill} text-[12px] text-muted-foreground`} title="Run now">
                    <IconPlayerPlay size={13} />
                    Run now
                  </button>
                  <button type="button" onClick={() => editRoutine(agent.id, routine)} className={iconBtn} aria-label={`Edit ${routine.name}`} title="Edit">
                    <IconPencil size={15} />
                  </button>
                </div>
              ))}
            </div>
          )}
        </Section>
      </div>

      <BotPicker
        open={adding}
        title="Add a bot to this project"
        description={`It will work in ${projectFolderName(project)} when you talk to it here.`}
        agents={agents.filter((a) => !project.agents.includes(a.id))}
        empty="Every bot is already on this project."
        onPick={(a) => {
          setAdding(false);
          void assignProject(project.id, a.id, true).catch(fail);
        }}
        onClose={() => setAdding(false)}
        extra={{
          label: "Create a bot for this project",
          onClick: () => {
            setAdding(false);
            startBot(project.id);
          },
        }}
      />
      <BotPicker
        open={chatting}
        title="New chat in this project"
        description="Pick the bot to talk to here."
        agents={bots}
        empty="Add a bot to this project first."
        onPick={(a) => {
          setChatting(false);
          void openAgentThread(a, project).catch(fail);
        }}
        onClose={() => setChatting(false)}
      />
      <ConfirmDialog
        open={removing}
        onOpenChange={setRemoving}
        title="Remove this project?"
        message={`“${project.name}” goes; its bots, folder and chats stay. Chats keep working in the folder.`}
        confirmLabel="Remove"
        destructive
        onConfirm={() => void deleteAgentProject(project.id).catch(fail)}
      />
    </div>
  );
}

function Section({ title, action, children }: { title: string; action?: ReactNode; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2.5">
      <div className="flex items-center justify-between">
        <h2 className="text-[14px] font-semibold">{title}</h2>
        {action}
      </div>
      {children}
    </section>
  );
}

function Empty({ children }: { children: ReactNode }) {
  return <div className={`rounded-2xl bg-well px-4 py-3 text-[12.5px]/5 text-muted-foreground ${WELL}`}>{children}</div>;
}

function BotCard({ agent, project, thread }: { agent: Agent; project: Project; thread?: ThreadInfo }) {
  const host = getHost();
  const status = agentStatus(agent, thread);
  return (
    <div className={`flex flex-col gap-2.5 rounded-2xl bg-well p-3.5 ${WELL}`}>
      <div className="flex items-center gap-2.5">
        <Avatar agent={agent} size={32} />
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-[14px] font-medium">{agent.name}</span>
          <span className="truncate text-[11.5px] text-muted-foreground">{oneLine(agent.job) || "No job yet."}</span>
        </div>
      </div>
      <div className="flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
        <span className={`size-1.5 shrink-0 rounded-full ${STATUS_DOT[status.kind]}`} aria-hidden />
        <span className="truncate">{status.label}</span>
      </div>
      <div className="flex items-center gap-1.5">
        <button type="button" onClick={() => void openAgentThread(agent, project).catch((e) => host.toast(String(e), "err"))} className="cursor-pointer rounded-full bg-primary px-3 py-1.5 text-[12.5px] font-medium text-primary-foreground hover:bg-(--primary-hover)">
          Message
        </button>
        <button type="button" onClick={() => void assignProject(project.id, agent.id, false).catch((e) => host.toast(String(e), "err"))} className="cursor-pointer rounded-full px-3 py-1.5 text-[12.5px] font-medium text-muted-foreground hover:bg-accent hover:text-foreground">
          Remove
        </button>
      </div>
    </div>
  );
}

/** One small picker for both "Add bot" and "New chat": the rows are bots, the
 *  parent only decides what clicking one does. */
function BotPicker({
  open,
  title,
  description,
  agents,
  empty,
  onPick,
  onClose,
  extra,
}: {
  open: boolean;
  title: string;
  description: string;
  agents: Agent[];
  empty: string;
  onPick: (a: Agent) => void;
  onClose: () => void;
  extra?: { label: string; onClick: () => void };
}) {
  if (!open) return null;
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <div className="mt-2 flex flex-col gap-0.5">
          {agents.length === 0 && <div className="px-1 py-2 text-[12.5px]/5 text-muted-foreground">{empty}</div>}
          {agents.map((a) => (
            <button key={a.id} type="button" onClick={() => onPick(a)} className="flex cursor-pointer items-center gap-2.5 rounded-xl px-2 py-2 text-left hover:bg-accent">
              <Avatar agent={a} size={26} />
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate text-[13.5px] font-medium">{a.name}</span>
                <span className="truncate text-[11.5px] text-muted-foreground">{oneLine(a.job) || "No job yet."}</span>
              </span>
            </button>
          ))}
          {extra && (
            <button type="button" onClick={extra.onClick} className="mt-1 flex cursor-pointer items-center gap-2.5 rounded-xl px-2 py-2 text-left text-[13.5px] font-medium hover:bg-accent">
              <span className="grid size-[26px] flex-none place-items-center rounded-full bg-foreground/[0.06] text-muted-foreground">
                <IconPlus size={15} />
              </span>
              {extra.label}
            </button>
          )}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------- project sheet

/** Create or rename a project. It renders while the store holds the sheet, so
 *  there is no second open state to fall out of sync with it. */
export function ProjectDialog(): ReactElement | null {
  const { projectDialog } = useChat();
  if (!projectDialog) return null;
  // Keyed by what is being edited: switching straight from one project to
  // another starts from that project's values, not the last one's.
  return <ProjectSheet key={projectDialog === "new" ? "new" : projectDialog.id} initial={projectDialog === "new" ? null : projectDialog} />;
}

function ProjectSheet({ initial }: { initial: Project | null }) {
  const host = getHost();
  const [name, setName] = useState(initial?.name ?? "");
  const [folder, setFolder] = useState(initial?.folder ?? "");
  const [machine, setMachine] = useState(initial?.machine ?? "");
  const [saving, setSaving] = useState(false);

  // The folder names the project unless the user already typed a name.
  const choose = async () => {
    try {
      const path = await host.invoke<string | null>("pick_folder");
      if (!path) return;
      setFolder(path);
      setName((n) => (n.trim() ? n : lastSegment(path)));
    } catch (e) {
      host.toast(String(e), "err");
    }
  };

  const save = async () => {
    setSaving(true);
    try {
      await saveAgentProject({
        id: initial?.id ?? "",
        name: name.trim() || lastSegment(folder),
        folder: folder.trim(),
        machine,
        agents: initial?.agents ?? [],
        created: initial?.created ?? 0,
        updated: initial?.updated ?? 0,
      });
      cancelProject();
    } catch (e) {
      host.toast(String(e), "err");
      setSaving(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && cancelProject()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{initial ? "Edit project" : "New project"}</DialogTitle>
          <DialogDescription>A project is a folder plus the bots that work in it. Chats you start here run in that folder.</DialogDescription>
        </DialogHeader>
        <div className="mt-1 flex flex-col gap-3.5">
          <label className="flex flex-col gap-1">
            <span className={label}>Name</span>
            <input autoFocus className={field} value={name} maxLength={60} placeholder={folder ? lastSegment(folder) : "Almanac"} onChange={(e) => setName(e.target.value)} />
          </label>
          <div className="flex flex-col gap-1">
            <span className={label}>Folder</span>
            <div className="flex items-center gap-2">
              <div className={`flex h-9 min-w-0 flex-1 items-center gap-2 rounded-xl bg-well px-3 ${WELL}`}>
                <IconFolder size={15} className="shrink-0 text-muted-foreground" />
                <span className={`truncate text-[13px] ${folder ? "" : "text-muted-foreground"}`} title={folder || undefined}>
                  {folder || "No folder chosen"}
                </span>
              </div>
              <Button variant="outline" onClick={() => void choose()}>
                Choose folder…
              </Button>
            </div>
          </div>
          <div className="flex items-center justify-between gap-2">
            <span className={label}>Machine</span>
            <select className={sel} aria-label="Machine" value={machine} onChange={(e) => setMachine(e.target.value)}>
              <option value="">This Mac</option>
            </select>
          </div>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={cancelProject}>
            Cancel
          </Button>
          <Button disabled={!folder.trim() || saving} onClick={() => void save()}>
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
