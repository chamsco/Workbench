// Whirl's components/composer.tsx (MIT, src/whirl/LICENSE), reduced to
// what Backspace's chat does: the same capsule, the same collapsed/expanded
// textbox measurement (text between the plus and the right-hand controls
// on one line, full width with the controls underneath once it outgrows
// it), the attachment tray, the attach menu, the model chip (here: who
// answers, see RoutePicker) and the morphing send button. Queueing, voice,
// mentions of integrations and the question form are Whirl-backend
// features; the attach menu offers "Ask another CLI" instead.

import { useEffect, useLayoutEffect, useMemo, useRef, useState, type RefObject } from "react";
import { IconCheck, IconCloudUpload, IconPaperclip, IconPlus, IconTerminal2, IconWand } from "@tabler/icons-react";
import { AnimatePresence, motion } from "motion/react";

import { cn } from "@/lib/utils";
import { useAttachments } from "@/lib/use-attachments";
import { ComposerAttachments } from "@/components/composer-attachments";
import { SendButton } from "@/components/send-button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { RoutePicker } from "./RoutePicker";
import { Avatar } from "./avatar";
import { mentionAtCaret } from "./mentions";
import type { Agent, NewFile, Route } from "../bridge";

const MAX_TEXTAREA_HEIGHT_PX = 160;
const CONTROL_GAP_PX = 6;
const CONTROLS_ROW_PX = 36 + CONTROL_GAP_PX;
const EXPAND_BUFFER_PX = 16;
const COLLAPSE_BUFFER_PX = 40;
const MORPH_SPRING = { type: "spring", stiffness: 900, damping: 55, mass: 0.5 } as const;
const ITEM = "gap-2 px-2 py-1.5";

export function Composer({
  value,
  onValueChange,
  route,
  onRouteChange,
  onSubmit,
  onStop,
  generating,
  floating,
  placeholder = "Ask anything",
  clis,
  textareaRef,
  hideRoute = false,
  everyday = false,
  onEverydayChange,
  onEverydaySubmit,
  roster,
}: {
  value: string;
  onValueChange: (v: string) => void;
  route: Route | null;
  onRouteChange: (r: Route) => void;
  onSubmit: (text: string, files: NewFile[]) => Promise<void>;
  onStop?: () => void;
  generating: boolean;
  floating: boolean;
  placeholder?: string;
  /** CLIs that can be @mentioned for one reply. */
  clis: { id: string; name: string }[];
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  /** Groups: each agent runs on its own model, so there is no one to pick. */
  hideRoute?: boolean;
  /** Everyday mode (shapeshift): Enter saves a card instead of sending a
   *  message, Esc leaves. The composer stays the composer. */
  everyday?: boolean;
  onEverydayChange?: (on: boolean) => void;
  onEverydaySubmit?: (text: string) => void;
  /** Who a group message can be aimed at with @Name. The core already honours
   *  the name — named_in queues only the members named — so this is the
   *  typing aid, not the routing. */
  roster?: Agent[];
}) {
  const attachments = useAttachments();
  const [sending, setSending] = useState(false);
  const [dragging, setDragging] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);
  const textBoxRef = useRef<HTMLDivElement>(null);
  const mirrorRef = useRef<HTMLSpanElement>(null);
  const plusRef = useRef<HTMLDivElement>(null);
  const rightRef = useRef<HTMLDivElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const [expanded, setExpanded] = useState(false);
  const expandedRef = useRef(false);
  const [textHeight, setTextHeight] = useState(36);
  const [insets, setInsets] = useState({ left: 42, right: 138 });
  const [contentWidth, setContentWidth] = useState(0);
  const hasBreak = value.includes("\n");

  // Who the caret is addressing: the group roster, autocompleted. Name
  // matching only (a job match is not an address), prefix before substring,
  // Esc dismisses until the token itself changes.
  const [caret, setCaret] = useState(0);
  const mention = mentionAtCaret(value, caret);
  const [dismissed, setDismissed] = useState(-1);
  const [pick, setPick] = useState(0);
  const options = useMemo(() => {
    if (!mention || !roster?.length || dismissed === mention.start) return [];
    const q = mention.query.trim().toLowerCase();
    if (!q) return roster.slice(0, 6);
    const prefix = roster.filter((a) => a.name.toLowerCase().startsWith(q));
    return (prefix.length ? prefix : roster.filter((a) => a.name.toLowerCase().includes(q))).slice(0, 6);
  }, [mention, roster, dismissed]);
  useEffect(() => setPick(options.length ? 0 : -1), [options.length]);

  const address = (a: Agent) => {
    if (!mention) return;
    const next = `${value.slice(0, mention.start)}@${a.name} ${value.slice(caret)}`;
    onValueChange(next);
    requestAnimationFrame(() => {
      const el = textareaRef.current;
      if (!el) return;
      const at = mention.start + a.name.length + 2;
      el.focus();
      el.setSelectionRange(at, at);
      setCaret(at);
    });
  };

  useLayoutEffect(() => {
    const content = contentRef.current;
    if (!content) return;
    const observer = new ResizeObserver(([entry]) => setContentWidth(entry.contentRect.width));
    observer.observe(content);
    return () => observer.disconnect();
  }, []);

  // Whirl's measurement pass: where would the text sit collapsed, and how
  // tall does the textarea want to be at the width it's heading for.
  useLayoutEffect(() => {
    const el = textareaRef.current;
    const content = contentRef.current;
    const textBox = textBoxRef.current;
    const mirror = mirrorRef.current;
    const plus = plusRef.current;
    const right = rightRef.current;
    if (!el || !content || !textBox || !mirror || !plus || !right) return;
    const left = plus.offsetWidth + CONTROL_GAP_PX;
    const rightZone = right.offsetWidth + CONTROL_GAP_PX;
    setInsets((c) => (c.left === left && c.right === rightZone ? c : { left, right: rightZone }));
    const fullInner = content.clientWidth - 16 - 12;
    const collapsedInner = fullInner - left - rightZone;
    const textWidth = mirror.offsetWidth;
    const nextExpanded =
      hasBreak ||
      (expandedRef.current
        ? textWidth > collapsedInner - COLLAPSE_BUFFER_PX
        : textWidth > collapsedInner - EXPAND_BUFFER_PX);
    expandedRef.current = nextExpanded;
    setExpanded(nextExpanded);
    const prev = [textBox.style.marginLeft, textBox.style.marginRight, el.style.height];
    textBox.style.marginLeft = nextExpanded ? "0px" : `${left}px`;
    textBox.style.marginRight = nextExpanded ? "0px" : `${rightZone}px`;
    el.style.height = "0px";
    setTextHeight(Math.min(el.scrollHeight, MAX_TEXTAREA_HEIGHT_PX));
    [textBox.style.marginLeft, textBox.style.marginRight, el.style.height] = prev;
  }, [value, hasBreak, route, contentWidth, textareaRef]);

  const ready = attachments.drafts.every((d) => d.status === "ready" || d.status === "error");
  const files = attachments.drafts.filter((d) => d.status === "ready" && d.data);
  const canSend = !sending && ready && (value.trim().length > 0 || files.length > 0);

  const submit = async () => {
    if (!canSend) return;
    setSending(true);
    const text = value;
    const payload: NewFile[] = files.map((d) => ({ name: d.name, mime: d.type, data: d.data! }));
    onValueChange("");
    attachments.clear();
    try {
      await onSubmit(text, payload);
    } catch {
      onValueChange(text);
    } finally {
      setSending(false);
    }
  };

  // The ＋ menu's CLI mention: a one-reply @, prepended.
  const mentionCli = (id: string) => {
    const next = `@${id} ${value.replace(/^@\S+\s*/, "")}`;
    onValueChange(next);
    requestAnimationFrame(() => {
      const el = textareaRef.current;
      if (el) {
        el.focus();
        el.setSelectionRange(next.length, next.length);
      }
    });
  };

  return (
    <div
      className={`relative rounded-[26px] border border-[var(--well-outline)] ${
        floating ? "bg-(--well-translucent) backdrop-blur-xl" : "bg-well"
      }`}
      onDragOver={(e) => {
        if (e.dataTransfer.types.includes("Files")) {
          e.preventDefault();
          setDragging(true);
        }
      }}
      onDragLeave={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node)) setDragging(false);
      }}
      onDrop={(e) => {
        e.preventDefault();
        setDragging(false);
        if (e.dataTransfer.files.length) attachments.addFiles(e.dataTransfer.files);
      }}
    >
      {/* Who the caret is addressing: the group roster, one keystroke away. */}
      <AnimatePresence>
        {options.length > 0 && (
          <motion.div
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 4 }}
            transition={{ duration: 0.12, ease: "linear" }}
            className="absolute bottom-full left-2 z-30 mb-2 w-72 overflow-hidden rounded-2xl bg-surface p-1 shadow-[0_16px_50px_rgba(0,0,0,.28)] ring-1 ring-border"
          >
            <div className="px-2.5 py-1 text-[10.5px]/4 font-medium text-muted-foreground/55">Address in the group</div>
            {options.map((a, i) => (
              <button
                key={a.id}
                type="button"
                onMouseEnter={() => setPick(i)}
                onClick={() => address(a)}
                className={`flex w-full cursor-pointer items-center gap-2.5 rounded-xl px-2.5 py-1.5 text-left ${i === pick ? "bg-accent" : ""}`}
              >
                <Avatar agent={a} size={24} />
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate text-[13px]/4 font-medium">{a.name}</span>
                  <span className="truncate text-[11px]/4 text-muted-foreground">{a.job.split("\n")[0] || "No job yet"}</span>
                </span>
                <span className="shrink-0 font-mono text-[10.5px] text-muted-foreground">@{a.name}</span>
              </button>
            ))}
          </motion.div>
        )}
      </AnimatePresence>
      <div ref={contentRef} className="relative p-2">
        <AnimatePresence initial={false}>
          {attachments.drafts.length > 0 && (
            <motion.div
              key="attachments"
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ height: MORPH_SPRING, opacity: { duration: 0.15, ease: "linear" } }}
              className="overflow-hidden"
            >
              <ComposerAttachments
                drafts={attachments.drafts}
                rejectionFor={(d) => (d.status === "error" ? (d.error ?? "Couldn't read it.") : null)}
                onRemove={attachments.remove}
              />
            </motion.div>
          )}
        </AnimatePresence>
        <motion.div
          ref={textBoxRef}
          initial={false}
          animate={{
            marginLeft: expanded ? 0 : insets.left,
            marginRight: expanded ? 0 : insets.right,
            marginBottom: expanded ? CONTROLS_ROW_PX : 0,
            height: textHeight,
          }}
          transition={MORPH_SPRING}
          className="relative"
        >
          <textarea
            ref={textareaRef}
            value={value}
            rows={1}
            autoFocus
            onChange={(e) => {
              onValueChange(e.target.value);
              setCaret(e.target.selectionStart ?? e.target.value.length);
            }}
            onSelect={(e) => setCaret(e.currentTarget.selectionStart ?? 0)}
            onClick={(e) => setCaret(e.currentTarget.selectionStart ?? 0)}
            onKeyUp={(e) => {
              if (e.key.startsWith("Arrow") || e.key === "Home" || e.key === "End") {
                setCaret(e.currentTarget.selectionStart ?? 0);
              }
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape" && everyday) {
                e.preventDefault();
                onEverydayChange?.(false);
                return;
              }
              if (options.length > 0) {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setPick((i) => (i + 1) % options.length);
                  return;
                }
                if (e.key === "ArrowUp") {
                  e.preventDefault();
                  setPick((i) => (i - 1 + options.length) % options.length);
                  return;
                }
                if (e.key === "Enter" || e.key === "Tab") {
                  e.preventDefault();
                  address(options[Math.max(0, pick)]);
                  return;
                }
                if (e.key === "Escape") {
                  e.preventDefault();
                  if (mention) setDismissed(mention.start);
                  return;
                }
              }
              if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                if (everyday) onEverydaySubmit?.(value);
                else void submit();
              }
            }}
            onPaste={(e) => {
              const pasted = e.clipboardData?.files;
              if (pasted && pasted.length > 0) {
                e.preventDefault();
                attachments.addFiles(pasted);
              }
            }}
            placeholder={everyday ? "An everyday thing — buy milk, eggs and bread" : placeholder}
            aria-label="Message"
            className="relative block h-full w-full resize-none overflow-y-auto bg-transparent px-1.5 py-1.5 field-text caret-foreground outline-none placeholder:text-muted-foreground"
          />
        </motion.div>
        <span
          ref={mirrorRef}
          aria-hidden
          className="invisible absolute top-0 left-0 block w-max px-1.5 field-text whitespace-pre"
        >
          {hasBreak ? "" : value}
        </span>
        <div ref={plusRef} className="absolute bottom-2 left-2">
          <input
            ref={fileRef}
            type="file"
            multiple
            hidden
            onChange={(e) => {
              if (e.target.files) attachments.addFiles(e.target.files);
              e.target.value = "";
            }}
          />
          <DropdownMenu>
            <DropdownMenuTrigger
              aria-label="Attach files, or ask another CLI"
              className="raised group flex size-9 shrink-0 cursor-pointer items-center justify-center rounded-full border border-border bg-surface text-muted-foreground transition-[background-color,color,scale] duration-150 hover:text-foreground data-popup-open:text-foreground active:scale-95"
            >
              <IconPlus size={18} stroke={2.5} className="transition-[rotate] duration-200 group-data-popup-open:rotate-45" />
            </DropdownMenuTrigger>
            <DropdownMenuContent side="top" align="start" sideOffset={8} className="min-w-52 p-1">
              <DropdownMenuItem className={ITEM} onClick={() => onEverydayChange?.(!everyday)}>
                <IconWand size={15} className="text-muted-foreground" />
                Everyday task
                {everyday && <IconCheck size={14} className="ml-auto" />}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem className={ITEM} onClick={() => fileRef.current?.click()}>
                <IconPaperclip size={15} className="text-muted-foreground" />
                Add photos & files
              </DropdownMenuItem>
              {clis.length > 0 && (
                <>
                  <DropdownMenuSeparator />
                  <DropdownMenuLabel className="px-2 py-1 text-[11.5px] font-medium text-muted-foreground">
                    Ask another CLI, for one reply
                  </DropdownMenuLabel>
                  {clis.map((c) => (
                    <DropdownMenuItem key={c.id} className={ITEM} onClick={() => mentionCli(c.id)}>
                      <IconTerminal2 size={15} className="text-muted-foreground" />
                      {c.name}
                      <span className="ml-auto font-mono text-[11.5px] text-muted-foreground">@{c.id}</span>
                    </DropdownMenuItem>
                  ))}
                </>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        <div ref={rightRef} className="absolute right-2 bottom-2 flex items-center gap-1.5">
          {!hideRoute && <RoutePicker value={route} onValueChange={onRouteChange} />}
          {everyday ? (
            <button
              type="button"
              disabled={value.trim().length === 0}
              onClick={() => onEverydaySubmit?.(value)}
              className="flex h-9 cursor-pointer items-center gap-1.5 rounded-full bg-primary px-3.5 text-[12.5px] font-medium text-primary-foreground transition-[background-color,scale] duration-150 hover:bg-(--primary-hover) active:scale-[0.97] disabled:cursor-default disabled:opacity-40"
            >
              <IconWand size={14} /> Save card
            </button>
          ) : (
            <SendButton
              state={generating && onStop ? "stop" : sending ? "sending" : "send"}
              canSend={canSend}
              onSend={() => void submit()}
              onStop={onStop}
              onQueue={() => {}}
            />
          )}
        </div>
      </div>
      <AnimatePresence>
        {dragging && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15, ease: "linear" }}
            className={cn(
              "pointer-events-none absolute -inset-1 z-50 flex items-center justify-center rounded-[28px] border-2 border-dashed border-foreground/25 bg-background/70 backdrop-blur-sm",
            )}
          >
            <div className="flex items-center gap-2 text-muted-foreground">
              <IconCloudUpload size={20} stroke={1.8} />
              <span className="text-sm font-medium">Drop files to attach</span>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
