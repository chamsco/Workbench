// The iMessage touches Whirl's thread view doesn't have, in its idiom:
// tapbacks (one per message, the six iMessage ones), replying to a specific
// message (a quote above the reply and a chip over the composer), link
// previews (the first link in a message, as a card), and Remember (save a
// message to Memory).

import { useState, type ReactNode } from "react";
import { IconArrowBackUp, IconBookmark, IconCheck, IconMoodSmile, IconX } from "@tabler/icons-react";

import { MessageActionButton } from "@/components/thread/message-action-button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { getHost, linkPreview, react, setReplyTo, type Msg } from "../bridge";

export const TAPBACKS = ["❤️", "👍", "👎", "😂", "‼️", "❓"];

export function TapbackButton({ thread, msg }: { thread: string; msg: Msg }) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        aria-label="React"
        className="flex size-7 cursor-pointer items-center justify-center rounded-lg text-muted-foreground transition-colors duration-150 hover:bg-black/[0.05] hover:text-foreground data-popup-open:text-foreground dark:hover:bg-white/[0.06]"
      >
        <IconMoodSmile size={15} />
      </PopoverTrigger>
      <PopoverContent side="top" align="center" sideOffset={6} className="w-auto min-w-0 rounded-full p-1">
        <div className="flex items-center gap-0.5">
          {TAPBACKS.map((e) => {
            const on = msg.reactions.includes(e);
            return (
              <button
                key={e}
                type="button"
                aria-label={`React ${e}`}
                aria-pressed={on}
                onClick={() => {
                  setOpen(false);
                  void react(thread, msg.id, e).catch((x) => getHost().toast(String(x), "err"));
                }}
                className={`flex size-8 cursor-pointer items-center justify-center rounded-full text-[17px] transition-[background-color,scale] duration-150 hover:scale-110 active:scale-95 ${
                  on ? "bg-foreground/10" : "hover:bg-black/[0.05] dark:hover:bg-white/[0.06]"
                }`}
              >
                {e}
              </button>
            );
          })}
        </div>
      </PopoverContent>
    </Popover>
  );
}

/** The tapback badge riding the bubble's corner. Click to take it back. */
export function Tapbacks({ thread, msg, className = "" }: { thread: string; msg: Msg; className?: string }) {
  if (!msg.reactions.length) return null;
  return (
    <div className={`flex gap-0.5 ${className}`}>
      {msg.reactions.map((e) => (
        <button
          key={e}
          type="button"
          title="Remove reaction"
          onClick={() => void react(thread, msg.id, e)}
          className="flex h-6 min-w-6 cursor-pointer items-center justify-center rounded-full bg-surface px-1 text-[13px] shadow-[0_0_0_1px_var(--well-outline),0_1px_3px_rgba(0,0,0,0.12)] transition-[scale] duration-150 hover:scale-110"
        >
          {e}
        </button>
      ))}
    </div>
  );
}

export function ReplyButton({ msg }: { msg: Msg }) {
  return (
    <MessageActionButton label="Reply to this message" onClick={() => setReplyTo(msg.id)}>
      <IconArrowBackUp size={15} />
    </MessageActionButton>
  );
}

export function RememberButton({ msg }: { msg: Msg }) {
  const [done, setDone] = useState(false);
  if (!msg.text.trim()) return null;
  return (
    <MessageActionButton
      label="Remember this"
      tooltip={done ? "Saved to Memory" : "Remember (save to Memory)"}
      onClick={() => {
        const text = msg.text.length > 600 ? msg.text.slice(0, 600) + "…" : msg.text;
        void getHost()
          .remember(text)
          .then(() => {
            setDone(true);
            setTimeout(() => setDone(false), 1500);
          })
          .catch((e) => getHost().toast(String(e), "err"));
      }}
    >
      {done ? <IconCheck size={15} /> : <IconBookmark size={15} />}
    </MessageActionButton>
  );
}

const snippet = (t: string, n = 90) => {
  const s = t.replace(/\s+/g, " ").trim();
  return s.length > n ? s.slice(0, n) + "…" : s;
};

/** Above a message that answers another: who and what, click to jump. */
export function Quote({ of, align }: { of: Msg | undefined; align: "start" | "end" }) {
  if (!of) return null;
  return (
    <button
      type="button"
      onClick={() => document.querySelector(`[data-message-id="${of.id}"]`)?.scrollIntoView({ behavior: "smooth", block: "center" })}
      className={`flex max-w-[85%] cursor-pointer items-center gap-1.5 text-[12px]/4 text-muted-foreground hover:text-foreground ${align === "end" ? "self-end" : "self-start"}`}
    >
      <IconArrowBackUp size={13} className="shrink-0" />
      <span className="shrink-0 font-medium">{of.role === "user" ? "You" : "Reply"}:</span>
      <span className="truncate">{snippet(of.text)}</span>
    </button>
  );
}

/** Over the composer while replying. */
export function ReplyChip({ to, onCancel }: { to: Msg; onCancel: () => void }) {
  return (
    <div className="mb-1.5 flex items-center gap-2 rounded-2xl bg-well px-3 py-1.5 text-[12.5px]/5 shadow-[inset_0_0_0_1px_var(--well-outline)]">
      <IconArrowBackUp size={14} className="shrink-0 text-muted-foreground" />
      <span className="shrink-0 font-medium">Replying to {to.role === "user" ? "you" : "this reply"}</span>
      <span className="min-w-0 flex-1 truncate text-muted-foreground">{snippet(to.text, 120)}</span>
      <button type="button" aria-label="Cancel reply" onClick={onCancel} className="flex size-6 cursor-pointer items-center justify-center rounded-full text-muted-foreground hover:bg-black/[0.05] hover:text-foreground dark:hover:bg-white/[0.06]">
        <IconX size={13} />
      </button>
    </div>
  );
}

const URL_RE = /https?:\/\/[^\s<>)\]"'`]+[^\s<>)\]"'`.,;:!?]/;
export const firstUrl = (t: string) => {
  const m = t.replace(/```[\s\S]*?```/g, "").match(URL_RE);
  return m ? m[0] : null;
};

/** The first link in a message, as a card (fetched once by the core). */
export function LinkCard({ text, align = "start" }: { text: string; align?: "start" | "end" }): ReactNode {
  const url = firstUrl(text);
  if (!url) return null;
  const meta = linkPreview(url);
  if (!meta) return null;
  return (
    <button
      type="button"
      onClick={() => getHost().openUrl(url)}
      className={`mt-2 flex w-full max-w-sm cursor-pointer flex-col overflow-hidden rounded-2xl bg-well text-left shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)] transition-[scale] duration-150 active:scale-[0.99] ${align === "end" ? "self-end" : "self-start"}`}
    >
      {meta.image && <img src={meta.image} alt="" className="aspect-[1.91/1] w-full object-cover" loading="lazy" />}
      <div className="flex flex-col gap-0.5 px-3.5 py-2.5">
        <span className="text-[11.5px]/4 text-muted-foreground">{meta.site}</span>
        {meta.title && <span className="line-clamp-2 text-[13.5px]/5 font-medium">{meta.title}</span>}
        {meta.description && <span className="line-clamp-2 text-[12.5px]/5 text-muted-foreground">{meta.description}</span>}
      </div>
    </button>
  );
}
