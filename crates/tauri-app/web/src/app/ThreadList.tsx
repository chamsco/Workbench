// The sidebar: Whirl's thread-row (MIT, src/whirl/LICENSE) — the whisper-quiet
// rows on a hover pill, the spinner while a reply runs, the ⋯ menu — over
// Backspace's threads, arranged as a project tree. A project is the thing you
// navigate by, so the threads you have had in it and the canvases it is showing
// both hang under its folder. Rendered into the vanilla sidebar through a portal.

import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  IconActivity,
  IconAlertTriangleFilled,
  IconBox,
  IconCheck,
  IconChevronDown,
  IconCircleFilled,
  IconCloud,
  IconDeviceDesktop,
  IconDots,
  IconEdit,
  IconFileText,
  IconFolder,
  IconFolderPlus,
  IconGitBranch,
  IconLayoutCards,
  IconLoader2,
  IconMessageReply,
  IconMessages,
  IconPencil,
  IconPin,
  IconPinFilled,
  IconPlus,
  IconSearch,
  IconSitemap,
  IconSparkles,
  IconTerminal2,
  IconTrash,
  IconWindow,
  IconWorld,
  type Icon,
} from "@tabler/icons-react";
import { AnimatePresence, motion } from "motion/react";

import { cn } from "@/lib/utils";
import { pinRasterPath } from "@/lib/motion";
import { RowPill } from "@/components/row-pill";
import { ConfirmDialog } from "@/components/confirm-dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { getHost, goHome, inScope, openThread, pin, remove, rename, setQuery, useChat, type ColonyStatus, type ThreadInfo } from "../bridge";

const baseName = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;

/** How long ago, in the quiet vocabulary the sidebar uses. */
function ago(ms: number) {
  const s = (Date.now() - ms) / 1000;
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  if (s < 7 * 86400) return `${Math.floor(s / 86400)}d`;
  return new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });
}

/** One icon, no label: the sidebar's own actions. A row of words up here pushes
 *  everything below it down and says nothing the icon does not. `popper` marks
 *  the ones that open a menu from the shell — the outside-click handler needs
 *  to know the button is part of the menu. */
function Act({ icon: ActIcon, label, onClick, on, popper }: { icon: Icon; label: string; onClick: (el: HTMLElement) => void; on?: boolean; popper?: boolean }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={on}
      data-popper={popper ? "" : undefined}
      onClick={(e) => onClick(e.currentTarget)}
      className={cn(
        "relative flex size-7 cursor-pointer items-center justify-center rounded-lg text-foreground-soft hover:bg-accent hover:text-foreground",
        on && "bg-accent text-foreground",
      )}
    >
      <ActIcon size={16} />
    </button>
  );
}

/** The tree: a folder per project, and under it everything the project holds —
 *  the threads you have had in it and the canvases it is showing. A project is
 *  the thing you navigate by, so a thread is never floating on its own.
 *
 *  Canvas rows come from `Host.canvases()`, which reads the open tab, so the
 *  sidebar and the tab bar can never disagree about what is open. */
export function ThreadList({ onNewChat }: { onNewChat: () => void }) {
  const { threads: all, id, query, scope, epoch } = useChat();
  const host = getHost();
  const [searching, setSearching] = useState(false);

  // In Code the sidebar belongs to the *project*, not to the chat face's scope:
  // the IDE shows the same tree with no thread open at all, and `scope` is null
  // there. Everywhere else it is the chat list, and the scope is the zone's.
  const zone = host.zone();
  const inCode = zone === "code";
  const project = inCode ? host.project() : scope;

  const folders = useMemo(() => {
    const q = query.trim().toLowerCase();
    const hit = (t: ThreadInfo) => !q || t.title.toLowerCase().includes(q) || t.preview.toLowerCase().includes(q);
    const out: { path: string | null; name: string; open: boolean; items: ThreadInfo[] }[] = [];
    const put = (path: string | null) => {
      const items = all
        .filter((t) => !t.app && (t.project ?? null) === path && hit(t))
        .sort((a, b) => b.updated - a.updated);
      // The open project keeps its folder even when it is empty — it is where
      // you are, and its canvases hang there.
      if (path === null && items.length === 0) return;
      out.push({ path, name: path ? baseName(path) : "Chats", open: project === path, items });
    };
    put(project);
    for (const t of all) {
      const p = t.project ?? null;
      if (t.app || p === project || out.some((f) => f.path === p)) continue;
      put(p);
    }
    return out;
  }, [all, project, query]);

  // Read on every render, not memoised: the shell owns this list and changes it
  // from outside React, so a memo keyed on the store's epoch goes stale the
  // moment a canvas is added or removed. (`saveTabs` nudges the store too.)
  const canvases = inCode ? host.canvases() : [];

  const tree =
    folders.length === 0 ? (
      <div className="px-2.5 py-2 text-[13px]/5 text-muted-foreground">
        {project ? "Nothing in this project yet. Ask a CLI to change something; it works in the folder." : "No chats yet. Start one with any model you've connected."}
      </div>
    ) : (
      folders.map((f) => (
        <section key={f.path ?? "chats"} className="flex flex-col gap-0.5">
          <div className="flex h-6 items-center gap-1.5 px-2 text-[11px]/4 font-semibold text-muted-foreground/80">
            <IconFolder size={13} className="shrink-0" />
            <span className="min-w-0 truncate" title={f.path ?? undefined}>
              {f.name}
            </span>
            {f.open && canvases.length > 0 && <span className="ml-auto shrink-0 font-normal text-muted-foreground/60">{canvases.length}</span>}
          </div>
          {f.open && canvases.map((c) => <CanvasRow key={`c${c.index}`} canvas={c} />)}
          {f.items.map((t) => (
            <ThreadRow key={t.id} thread={t} active={t.id === id} inIde={inCode} />
          ))}
          {f.items.length === 0 && !(f.open && canvases.length > 0) && (
            <div className="px-2.5 py-1 text-[12.5px]/5 text-muted-foreground">
              {query ? "No match" : "No threads yet"}
            </div>
          )}
        </section>
      ))
    );

  return (
    <div className="whirl-side flex flex-col gap-2.5">
      <div className="flex items-center gap-0.5 px-0.5">
        <Act icon={IconEdit} label={scope || inCode ? "New thread" : "New chat"} onClick={onNewChat} />
        <Act icon={IconFolder} label="Projects" popper onClick={(el) => host.projectMenu(el)} />
        <Act icon={IconSearch} label="Search" on={searching} onClick={() => { setSearching((s) => !s); if (searching) setQuery(""); }} />
      </div>
      {searching && (
        <label className="group/row relative flex h-8 w-full items-center gap-2.5 rounded-lg px-2.5 text-[13.5px]/4 font-medium text-foreground-soft">
          <RowPill className="group-focus-within/row:bg-accent" />
          <IconSearch size={15} className="relative shrink-0" />
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setQuery("");
                setSearching(false);
              }
            }}
            placeholder="Search"
            aria-label="Search chats"
            className="relative h-full min-w-0 flex-1 bg-transparent outline-none placeholder:text-foreground-soft"
          />
        </label>
      )}

      {/* The machine is the root of the tree, and the folder it is running right
          now hangs under it: which machine you are on, then what it holds. */}
      <EnvironmentRow forceOpen={searching}>{tree}</EnvironmentRow>
    </div>
  );
}

/** A canvas the open project is showing, as a row in the tree. */
function CanvasRow({ canvas }: { canvas: { index: number; kind: string; label: string } }) {
  const host = getHost();
  const Glyph = CANVAS_ICON[canvas.kind] ?? IconWindow;
  return (
    <button
      type="button"
      onClick={() => host.openCanvas(canvas.index)}
      title={canvas.label}
      className="group/row relative flex h-7 w-full cursor-pointer items-center gap-2 rounded-lg pr-2.5 pl-6 text-left text-[12.5px]/4 text-muted-foreground"
    >
      <RowPill />
      <Glyph size={13} className="relative shrink-0" />
      <span className="relative min-w-0 flex-1 truncate">{canvas.label}</span>
    </button>
  );
}

const CANVAS_ICON: Record<string, Icon> = {
  agent: IconSparkles,
  board: IconMessages,
  conversation: IconMessageReply,
  terminal: IconTerminal2,
  files: IconFolder,
  browser: IconWorld,
  diagram: IconSitemap,
  docs: IconFileText,
  options: IconLayoutCards,
  trace: IconActivity,
  empty: IconWindow,
};

/* The root of the tree: which machine the work runs on, and — expanded — the
 * folder it is running right now, with everything that folder holds.
 *
 * One row, like T3 Code's environment picker: this machine, or a machine the
 * harness is paired with. Switching it moves the agents, the worktrees and the
 * work, which is the ⋯ beside the row. Colony's named workspaces are listed
 * there too when visor-core is there; they are created and driven by agents (see
 * Settings → Workspaces and Colony), so picking one opens the page that manages
 * them rather than pretending a thread can be pinned to it, which the core
 * cannot do today. */
function EnvironmentRow({ children, forceOpen }: { children: ReactNode; forceOpen?: boolean }) {
  const host = getHost();
  const { epoch } = useChat();
  // Expanded by default: the folder you are running now is the point of the row,
  // and a collapsed tree should be a choice rather than the way it lands. A
  // search opens it, or matches would be hidden behind a collapsed root.
  const [open, setOpen] = useState(true);
  const expanded = open || !!forceOpen;
  const [menuOpen, setMenuOpen] = useState(false);
  const [colony, setColony] = useState<ColonyStatus | null>(null);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const all = useMemo(() => host.machines(), [epoch]);
  const here = all.find((m) => m.selected);
  const name = here ? (here.local ? "This machine" : here.name) : host.machineName();
  const workspaces = colony?.ready ? colony.workspaces ?? [] : [];

  useEffect(() => {
    if (!menuOpen || colony) return;
    let live = true;
    host.colony().then((c) => live && setColony(c), () => live && setColony(null));
    return () => {
      live = false;
    };
  }, [menuOpen, colony, host]);

  return (
    <div className="flex min-h-0 flex-col gap-0.5">
      <div className="group/row group/thread relative">
        <button
          type="button"
          aria-expanded={expanded}
          title={`${name} — the folder it is running right now`}
          onClick={() => setOpen((o) => !o)}
          className="group/row relative flex h-8 w-full cursor-pointer items-center gap-2.5 rounded-lg pr-8 pl-2.5 text-[13.5px]/4 font-medium text-foreground-soft"
        >
          <RowPill />
          <span className="relative shrink-0">{here && !here.local ? <IconCloud size={16} /> : <IconDeviceDesktop size={16} />}</span>
          <span className="relative min-w-0 flex-1 truncate text-left">{name}</span>
          <IconChevronDown size={14} className={cn("relative shrink-0 text-muted-foreground transition-transform duration-150", expanded && "rotate-180")} />
        </button>
        <DropdownMenu open={menuOpen} onOpenChange={setMenuOpen}>
          <DropdownMenuTrigger
            aria-label="Where the next thread runs"
            title="Where the next thread runs"
            className="absolute top-1/2 right-1 flex size-6 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md text-foreground-soft opacity-0 transition-opacity duration-150 group-hover/thread:opacity-100 hover:text-foreground data-popup-open:opacity-100 coarse:opacity-100"
          >
            <IconDots size={16} />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="min-w-56 p-1">
            <DropdownMenuGroup>
              <DropdownMenuLabel className="px-2 py-1 text-[10.5px]">Start on</DropdownMenuLabel>
              {all.map((m) => (
                <DropdownMenuItem key={m.index} className="gap-2 px-2 py-1.5" onClick={() => void host.pickMachine(m.index)}>
                  {m.local ? <IconDeviceDesktop size={15} /> : <IconCloud size={15} />}
                  <span className="flex min-w-0 flex-col">
                    <span className="truncate">{m.local ? "This machine" : m.name}</span>
                    {!m.local && (
                      <span className="truncate text-[11px] text-muted-foreground">
                        {m.url} · {m.link.state}
                      </span>
                    )}
                  </span>
                  {m.selected && <IconCheck size={14} className="ml-auto shrink-0" />}
                </DropdownMenuItem>
              ))}
            </DropdownMenuGroup>
            {workspaces.length > 0 && (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuGroup>
                  <DropdownMenuLabel className="px-2 py-1 text-[10.5px]">Colony workspaces</DropdownMenuLabel>
                  {workspaces.map((w) => (
                    <DropdownMenuItem key={w.name} className="gap-2 px-2 py-1.5" onClick={() => host.openSettings("colony")}>
                      <IconBox size={15} />
                      <span className="flex min-w-0 flex-col">
                        <span className="truncate">{w.name}</span>
                        <span className="truncate text-[11px] text-muted-foreground">
                          {[w.provider, w.os, w.state].filter(Boolean).join(" · ")}
                        </span>
                      </span>
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuGroup>
              </>
            )}
            <DropdownMenuSeparator />
            <DropdownMenuItem className="gap-2 px-2 py-1.5" onClick={() => host.openSettings("machines")}>
              <IconPlus size={15} className="text-muted-foreground" />
              Add a machine…
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      {expanded && <div className="flex min-h-0 flex-col gap-0.5 pl-2">{children}</div>}
    </div>
  );
}

function ThreadRow({ thread, active, inIde }: { thread: ThreadInfo; active: boolean; inIde?: boolean }) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [title, setTitle] = useState(thread.title);
  const host = getHost();
  const fail = (e: unknown) => host.toast(String(e), "err");
  // A thread picked from the tree while the IDE is showing is read in the ADE,
  // so the switch happens on the way there rather than teleporting the reader.
  const open = () => {
    if (inIde) host.showThread();
    void openThread(thread.id);
  };
  return (
    <div
      className="group/row group/thread relative"
      onContextMenu={(e) => {
        e.preventDefault();
        setMenuOpen(true);
      }}
    >
      <button
        type="button"
        onClick={open}
        title={thread.preview || thread.title}
        className="relative flex h-8 w-full cursor-pointer items-center rounded-lg pr-8 pl-6 text-[13.5px]/4 font-medium text-foreground-soft"
      >
        <RowPill className={`group-has-data-popup-open/thread:bg-accent ${active ? "bg-accent" : ""}`} />
        <span className="relative flex min-w-0 flex-1 items-center gap-1.5 text-left">
          {thread.pinned ? (
            <IconPinFilled size={12} className="shrink-0" aria-label="Pinned" />
          ) : thread.title.endsWith("(branch)") ? (
            <IconGitBranch size={13} className="shrink-0" aria-label="Branched thread" />
          ) : null}
          <span className="block min-w-0 flex-1 truncate">{thread.title}</span>
          <span className="relative shrink-0 text-[11px] font-normal text-muted-foreground/60 transition-opacity duration-150 group-hover/thread:opacity-0">
            {ago(thread.updated)}
          </span>
        </span>
      </button>
      <AnimatePresence mode="wait" initial={false}>
        {thread.busy ? (
          <motion.span
            key="running"
            initial={{ opacity: 0, scale: 0.5 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.5 }}
            transition={{ duration: 0.15, ease: "easeOut" }}
            transformTemplate={pinRasterPath}
            className="pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 transition-opacity duration-150 group-hover/thread:opacity-0! group-has-data-popup-open/thread:opacity-0!"
          >
            <IconLoader2 size={14} className="animate-spin text-foreground-soft" />
          </motion.span>
        ) : thread.failed ? (
          <motion.span
            key="failed"
            initial={{ opacity: 0, scale: 0.5 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.5 }}
            transition={{ duration: 0.15, ease: "easeOut" }}
            title={`The last reply failed: ${thread.failed}`}
            className="absolute top-1/2 right-2.5 -translate-y-1/2 text-[#e5484d] transition-opacity duration-150 group-hover/thread:opacity-0!"
          >
            <IconAlertTriangleFilled size={14} aria-label="Failed" />
          </motion.span>
        ) : thread.unread && !active ? (
          <motion.span
            key="unread"
            initial={{ opacity: 0, scale: 0.5 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.5 }}
            transition={{ duration: 0.15, ease: "easeOut" }}
            className="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-foreground-soft transition-opacity duration-150 group-hover/thread:opacity-0!"
          >
            <IconCircleFilled size={7} />
          </motion.span>
        ) : null}
      </AnimatePresence>
      <DropdownMenu open={menuOpen} onOpenChange={setMenuOpen}>
        <DropdownMenuTrigger
          aria-label={`Options for ${thread.title}`}
          className="absolute top-1/2 right-1 flex size-6 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md text-foreground-soft opacity-0 transition-opacity duration-150 group-hover/thread:opacity-100 hover:text-foreground data-popup-open:opacity-100 coarse:opacity-100"
        >
          <IconDots size={16} />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="min-w-44 p-1">
          <DropdownMenuItem className="gap-2 px-2 py-1.5" onClick={() => setRenaming(true)}>
            <IconPencil size={15} className="text-muted-foreground" />
            Rename
          </DropdownMenuItem>
          <DropdownMenuItem className="gap-2 px-2 py-1.5" onClick={() => void pin(thread.id, !thread.pinned).catch(fail)}>
            <IconPin size={15} className="text-muted-foreground" />
            {thread.pinned ? "Unpin" : "Pin"}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem variant="destructive" className="gap-2 px-2 py-1.5" onClick={() => setDeleting(true)}>
            <IconTrash size={15} />
            Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <Dialog open={renaming} onOpenChange={setRenaming}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Rename chat</DialogTitle>
            <DialogDescription>Give it a name you'll find later.</DialogDescription>
          </DialogHeader>
          <input
            autoFocus
            value={title}
            maxLength={80}
            onChange={(e) => setTitle(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                void rename(thread.id, title).then(() => setRenaming(false), fail);
              }
            }}
            className="h-9 w-full rounded-full bg-well px-3.5 text-[14px] outline-none shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]"
          />
          <DialogFooter>
            <Button variant="ghost" onClick={() => setRenaming(false)}>
              Cancel
            </Button>
            <Button onClick={() => void rename(thread.id, title).then(() => setRenaming(false), fail)}>Save</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <ConfirmDialog
        open={deleting}
        onOpenChange={setDeleting}
        title="Delete this chat?"
        message={`“${thread.title}” and its attachments will be gone for good.`}
        confirmLabel="Delete"
        destructive
        onConfirm={() => {
          void remove(thread.id).catch(fail);
          if (active) goHome();
        }}
      />
    </div>
  );
}
