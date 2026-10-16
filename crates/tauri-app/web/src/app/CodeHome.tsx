// Code → ADE's home: the project, not the agents.
//
// The Agents zone's home is a grid of agents, because that zone is about the
// agents. Code's ADE is about a *project*: one folder, one CLI, sessions you
// have already had in it, what is uncommitted, what is still waiting on you.
// So this is that, and the composer under it is the one you start the next
// session with. (The IDE side of the switch lands on a session canvas instead —
// see docs/ADE-AND-IDE.md.)

import { useEffect, useMemo, useState } from "react";
import {
  IconAlertTriangleFilled,
  IconArrowRight,
  IconBrandGit,
  IconCircleFilled,
  IconFolderOpen,
  IconGitBranch,
  IconLoader2,
  IconPlus,
  IconSparkles,
  IconTicket,
  IconWindow,
} from "@tabler/icons-react";

import { getHost, inScope, openThread, useChat, type CodeHome as Home } from "../bridge";

const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";
const baseName = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;

/** How long ago, in the sidebar's own vocabulary. */
function ago(ms: number) {
  const s = (Date.now() - ms) / 1000;
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  if (s < 7 * 86400) return `${Math.floor(s / 86400)}d`;
  return new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });
}

/** A card of rows, the way Settings and the agents' home draw them.
 *
 *  The window's painting runs at full strength behind this screen, so the body
 *  of every card is a well: rows of file names over a bright sky are not
 *  readable, and a home is read, not looked at. */
function Card({ title, action, children }: { title: string; action?: React.ReactNode; children: React.ReactNode }) {
  return (
    <section className="flex min-w-0 flex-col gap-1.5">
      <div className="flex h-6 items-center gap-2 px-1 text-[12.5px] font-semibold text-foreground">
        {title}
        <span className="flex-1" />
        {action}
      </div>
      <div className={`rounded-2xl bg-well p-1.5 ${WELL}`}>{children}</div>
    </section>
  );
}

function Empty({ children }: { children: React.ReactNode }) {
  return <div className="px-2 py-2 text-[12.5px]/5 text-muted-foreground">{children}</div>;
}

export function CodeHome({ scope }: { scope: string }) {
  const host = getHost();
  const { threads, epoch } = useChat();
  const [home, setHome] = useState<Home | null>(null);

  useEffect(() => {
    let live = true;
    host
      .codeHome(scope)
      .then((h) => live && setHome(h), () => live && setHome(null));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scope, epoch]);

  const sessions = useMemo(
    () => threads.filter((t) => inScope(t, scope)).sort((a, b) => b.updated - a.updated),
    [threads, scope],
  );
  const files = home?.files ?? [];
  const tickets = (home?.tickets ?? []).filter((t) => t.state !== "done");
  const worktrees = (home?.worktrees ?? []).filter((w) => w.branch);
  const folder = baseName(scope);

  return (
    <div className="h-full overflow-y-auto px-6 pt-10 pb-56">
      <div className="mx-auto flex max-w-3xl flex-col gap-7">
        <div className="flex min-w-0 flex-col gap-1">
          <div className="flex min-w-0 items-baseline gap-2.5">
            <h1 className="truncate text-[24px]/8 font-medium tracking-tight">{folder}</h1>
            {home?.branch && (
              <span className="flex shrink-0 items-center gap-1 text-[12.5px] text-muted-foreground">
                <IconGitBranch size={13} />
                {home.branch}
                {home.fresh && <span className="text-muted-foreground/70">new repo</span>}
              </span>
            )}
          </div>
          <p className="truncate text-[12.5px] text-muted-foreground" title={scope}>
            {scope}
          </p>
          {home && (
            <p className="mt-0.5 text-[12.5px] text-muted-foreground">
              {home.agents} agent{home.agents === 1 ? "" : "s"}
              {home.running > 0 && ` · ${home.running} working`}
              {home.needs > 0 && <span className="text-(--amber,#f0a040)"> · {home.needs} waiting on you</span>}
            </p>
          )}
        </div>

        <Card
          title="Sessions"
          action={
            <button
              type="button"
              className="flex cursor-pointer items-center gap-1.5 rounded-full px-2.5 py-1 text-[12.5px] font-medium text-foreground-soft hover:bg-accent"
              onClick={() => host.setCodeView("agents")}
            >
              <IconWindow size={14} /> Open the canvases
            </button>
          }
        >
          {sessions.length === 0 ? (
            <Empty>No sessions in {folder} yet. Describe a change below; the CLI works in this folder and you review what it changed with git.</Empty>
          ) : (
            <div className="flex flex-col gap-0.5">
              {sessions.slice(0, 6).map((t) => (
                <button
                  key={t.id}
                  type="button"
                  onClick={() => void openThread(t.id)}
                  title={t.preview || t.title}
                  className="flex min-w-0 cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-left hover:bg-accent"
                >
                  {t.busy ? (
                    <IconLoader2 size={13} className="shrink-0 animate-spin text-muted-foreground" />
                  ) : t.failed ? (
                    <IconAlertTriangleFilled size={12} className="shrink-0 text-[#e5484d]" />
                  ) : (
                    <IconSparkles size={13} className="shrink-0 text-muted-foreground" />
                  )}
                  <span className="min-w-0 flex-1 truncate text-[13.5px]">{t.title || "Untitled"}</span>
                  {t.unread && <IconCircleFilled size={7} className="shrink-0 text-sky-500" />}
                  <span className="shrink-0 text-[11.5px] text-muted-foreground">{ago(t.updated)}</span>
                </button>
              ))}
            </div>
          )}
        </Card>

        <Card title="Uncommitted">
          {files.length === 0 ? (
            <Empty>{home ? "Working tree clean." : "Reading the folder…"}</Empty>
          ) : (
            <div className="flex flex-col gap-0.5">
              {files.slice(0, 7).map((f) => (
                <div key={f.path} className="flex min-w-0 items-center gap-2.5 px-1.5 py-1 text-[12.5px]">
                  <span className="w-3 shrink-0 text-center font-mono text-[11px] text-(--amber,#f0a040)">
                    {f.status.slice(0, 1).toUpperCase()}
                  </span>
                  <span className="min-w-0 flex-1 truncate font-mono text-[12px]">{f.path}</span>
                  {f.added > 0 && <span className="shrink-0 font-mono text-[11px] text-emerald-500">+{f.added}</span>}
                  {f.removed > 0 && <span className="shrink-0 font-mono text-[11px] text-[#e5484d]">−{f.removed}</span>}
                </div>
              ))}
              {files.length > 7 && (
                <button
                  type="button"
                  className="cursor-pointer px-1.5 py-1 text-left text-[12px] text-muted-foreground hover:text-foreground"
                  onClick={() => host.openPanel("changes")}
                >
                  {files.length - 7} more changed file{files.length - 7 === 1 ? "" : "s"} · open Changes
                </button>
              )}
            </div>
          )}
        </Card>

        <div className="grid grid-cols-1 gap-7 sm:grid-cols-2">
          <Card
            title="Tickets"
            action={
              tickets.length > 0 ? (
                <button type="button" className="cursor-pointer text-[12.5px] text-muted-foreground hover:text-foreground" onClick={() => host.openSettings("coding")}>
                  <IconArrowRight size={14} />
                </button>
              ) : undefined
            }
          >
            {tickets.length === 0 ? (
              <Empty>Nothing planned yet. Give the main agent a goal and it proposes tickets for you to approve.</Empty>
            ) : (
              <div className="flex flex-col gap-0.5">
                {tickets.slice(0, 5).map((t) => (
                  <div key={t.key} className="flex min-w-0 items-center gap-2.5 px-1 py-1 text-[12.5px]">
                    <IconTicket size={13} className="shrink-0 text-muted-foreground" />
                    <span className="min-w-0 flex-1 truncate">{t.title || t.key}</span>
                    <span className="shrink-0 text-[11px] text-muted-foreground">{t.state.replace(/_/g, " ")}</span>
                  </div>
                ))}
              </div>
            )}
          </Card>

          <Card title="Worktrees">
            {worktrees.length === 0 ? (
              <Empty>Each ticket&apos;s agent gets its own branch and worktree while it works. None yet.</Empty>
            ) : (
              <div className="flex flex-col gap-0.5">
                {worktrees.slice(0, 5).map((w) => (
                  <div key={w.path || w.branch} className="flex min-w-0 items-center gap-2.5 px-1 py-1 text-[12.5px]">
                    <IconGitBranch size={13} className="shrink-0 text-muted-foreground" />
                    <span className="min-w-0 flex-1 truncate font-mono text-[12px]">{w.branch}</span>
                    <span className="shrink-0 text-[11px] text-muted-foreground">{w.title || w.agent}</span>
                  </div>
                ))}
              </div>
            )}
          </Card>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => host.openPanel("files")}
            className={`flex cursor-pointer items-center gap-1.5 rounded-full bg-well px-3 py-1.5 text-[12.5px] font-medium hover:bg-accent ${WELL}`}
          >
            <IconFolderOpen size={14} /> Files
          </button>
          <button
            type="button"
            onClick={() => host.openPanel("sessions")}
            className={`flex cursor-pointer items-center gap-1.5 rounded-full bg-well px-3 py-1.5 text-[12.5px] font-medium hover:bg-accent ${WELL}`}
          >
            <IconBrandGit size={14} /> History
          </button>
          <button
            type="button"
            onClick={() => host.newProject()}
            className={`flex cursor-pointer items-center gap-1.5 rounded-full bg-well px-3 py-1.5 text-[12.5px] font-medium hover:bg-accent ${WELL}`}
          >
            <IconPlus size={14} /> Another project
          </button>
        </div>
      </div>
    </div>
  );
}
