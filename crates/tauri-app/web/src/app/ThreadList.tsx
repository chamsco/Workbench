// The sidebar's chat list: Whirl's thread-list / thread-row / sidebar-row
// (MIT, src/whirl/LICENSE) — the whisper-quiet date markers, 32px rows on
// a hover pill, the spinner while a reply runs, the ⋯ menu — over
// Backspace's threads. Rendered into the vanilla sidebar through a portal.

import { useMemo, useState } from "react";
import {
  IconCircleFilled,
  IconDots,
  IconEdit,
  IconGitBranch,
  IconLoader2,
  IconPencil,
  IconPin,
  IconPinFilled,
  IconSearch,
  IconTrash,
} from "@tabler/icons-react";
import { AnimatePresence, motion } from "motion/react";

import { pinRasterPath } from "@/lib/motion";
import { RowPill } from "@/components/row-pill";
import { SidebarRow } from "@/components/sidebar-row";
import { ConfirmDialog } from "@/components/confirm-dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
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
import { getHost, goHome, openThread, pin, remove, rename, setQuery, useChat, type ThreadInfo } from "../bridge";

const DAY = 864e5;
const sameDay = (a: number, b: number) => new Date(a).toDateString() === new Date(b).toDateString();

function bucket(t: ThreadInfo, now: number) {
  if (t.pinned) return "Pinned";
  if (sameDay(t.updated, now)) return "Today";
  if (sameDay(t.updated, now - DAY)) return "Yesterday";
  if (now - t.updated < 7 * DAY) return "Previous 7 days";
  if (now - t.updated < 30 * DAY) return "Previous 30 days";
  return "Older";
}

export function ThreadList({ onNewChat }: { onNewChat: () => void }) {
  const { threads, id, query } = useChat();
  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const now = Date.now();
    const out: { label: string; items: ThreadInfo[] }[] = [];
    for (const t of threads) {
      if (q && !t.title.toLowerCase().includes(q) && !t.preview.toLowerCase().includes(q)) continue;
      const label = bucket(t, now);
      let g = out.find((x) => x.label === label);
      if (!g) out.push((g = { label, items: [] }));
      g.items.push(t);
    }
    // Pinned first, whatever the order threads arrive in.
    return out.sort((a, b) => Number(b.label === "Pinned") - Number(a.label === "Pinned"));
  }, [threads, query]);

  return (
    <div className="whirl-side flex flex-col gap-3">
      <div className="flex flex-col gap-0.5">
        <SidebarRow icon={IconEdit} label="New chat" onClick={onNewChat} />
        <label className="group/row relative flex h-8 w-full items-center gap-2.5 rounded-lg px-2.5 text-[13.5px]/4 font-medium text-foreground-soft">
          <RowPill className="group-focus-within/row:bg-accent" />
          <IconSearch size={16} className="relative shrink-0" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setQuery("");
                (e.target as HTMLInputElement).blur();
              }
            }}
            placeholder="Search"
            aria-label="Search chats"
            className="relative h-full min-w-0 flex-1 bg-transparent outline-none placeholder:text-foreground-soft"
          />
        </label>
      </div>
      {threads.length === 0 ? (
        <div className="px-2.5 py-2 text-[13px]/5 text-muted-foreground">
          No chats yet. Start one with any model you've connected.
        </div>
      ) : groups.length === 0 ? (
        <div className="px-2.5 py-2 text-[13px]/5 text-muted-foreground">No chats match “{query}”.</div>
      ) : (
        <div className="flex flex-col gap-2">
          {groups.map((g) => (
            <section key={g.label}>
              <div className="flex h-5 items-center px-2.5 text-[10.5px]/4 font-medium text-muted-foreground/55">
                {g.label}
              </div>
              <div className="flex flex-col gap-0.5">
                {g.items.map((t) => (
                  <ThreadRow key={t.id} thread={t} active={t.id === id} />
                ))}
              </div>
            </section>
          ))}
        </div>
      )}
    </div>
  );
}

function ThreadRow({ thread, active }: { thread: ThreadInfo; active: boolean }) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [title, setTitle] = useState(thread.title);
  const host = getHost();
  const fail = (e: unknown) => host.toast(String(e), "err");
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
        onClick={() => void openThread(thread.id)}
        title={thread.preview || thread.title}
        className="relative flex h-8 w-full cursor-pointer items-center rounded-lg pr-8 pl-2.5 text-[13.5px]/4 font-medium text-foreground-soft"
      >
        <RowPill className={`group-has-data-popup-open/thread:bg-accent ${active ? "bg-accent" : ""}`} />
        <span className="relative flex min-w-0 flex-1 items-center gap-1.5 text-left">
          {thread.pinned ? (
            <IconPinFilled size={12} className="shrink-0" aria-label="Pinned" />
          ) : thread.title.endsWith("(branch)") ? (
            <IconGitBranch size={13} className="shrink-0" aria-label="Branched thread" />
          ) : null}
          <span className="block min-w-0 flex-1 truncate">{thread.title}</span>
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
