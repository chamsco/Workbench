// Whirl's components/thread/assistant-message.tsx (MIT, src/whirl/LICENSE),
// cut to what Backspace's replies carry: streamed prose through the same
// typewriter and Markdown, the same error banner, "Stopped", and the hover
// row (copy, retry, branch). Tool-phase activity, artifacts and image
// generation are Whirl-backend features and are left out. Added: who
// answered (multi-harness threads) and the sponsored card for Cloud's
// ad-supported plans, styled as one of Whirl's wells.

import { useState } from "react";
import {
  IconAlertTriangleFilled,
  IconCheck,
  IconCopy,
  IconLoader2,
  IconPlayerStopFilled,
  IconRefresh,
  IconX,
} from "@tabler/icons-react";

import { isTerminal, type ChatMessage } from "@/lib/messages";
import { useTypewriter } from "@/lib/use-typewriter";
import { CheckpointMenu } from "@/components/thread/checkpoint-menu";
import { Markdown } from "@/components/thread/markdown";
import { MessageActionButton } from "@/components/thread/message-action-button";
import { getHost, type Ad } from "../bridge";

const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";

export function AssistantMessage({
  message,
  error,
  answeredBy,
  ad,
  onRetry,
  onBranch,
}: {
  message: ChatMessage;
  error?: string | null;
  answeredBy?: string;
  ad?: Ad | null;
  onRetry?: () => void;
  onBranch?: () => void;
}) {
  const terminal = isTerminal(message.status);
  const [sawLive] = useState(!terminal);
  const stopped = message.status === "stopped";
  const { text, caughtUp } = useTypewriter(message.content, sawLive, stopped);
  const failed = message.status === "error";
  const streamingNow = !terminal && !failed;
  const showText = !failed && text.length > 0;

  return (
    <div data-quotable="assistant" className="group/msg flex w-full min-w-0 flex-col items-start">
      {answeredBy && (
        <div className="mb-1.5 text-[12px]/4 font-medium text-muted-foreground">{answeredBy}</div>
      )}
      {showText && <Markdown streaming={streamingNow || !caughtUp}>{text}</Markdown>}
      {streamingNow && !showText && (
        <div className="flex h-7 items-center gap-2 text-[14px]/5 text-muted-foreground">
          <IconLoader2 size={15} className="animate-spin" />
          <span className="text-shimmer">Thinking</span>
        </div>
      )}
      {failed && <ErrorBanner detail={error ?? undefined} onRetry={onRetry} />}
      {stopped && (
        <div className="mt-2 flex items-center gap-1.5 text-[13px]/4 font-medium text-muted-foreground">
          <IconPlayerStopFilled size={12} />
          Stopped
        </div>
      )}
      {!terminal && showText && <div aria-hidden className="mt-1.5 h-7" />}
      {terminal && !failed && (
        <MessageActions message={message} onRetry={onRetry} onBranch={onBranch} />
      )}
      {ad && terminal && <AdCard ad={ad} />}
    </div>
  );
}

/* A gate (plan limit) reads as one of Whirl's paywall banners; anything
   else is the plain "didn't make it" banner with a retry. */
function ErrorBanner({ detail, onRetry }: { detail?: string; onRetry?: () => void }) {
  const gate = !!detail && /plan|upgrade|free replies|sign up/i.test(detail);
  const headline = gate ? "That needs a different plan." : "Something went wrong.";
  const host = getHost();
  return (
    <div className={`flex w-full max-w-md flex-col gap-3 rounded-2xl bg-well px-4 py-3.5 ${WELL}`}>
      <div className="flex items-start gap-2.5">
        <IconAlertTriangleFilled size={17} className="mt-0.5 shrink-0 text-muted-foreground" />
        <div className="min-w-0">
          <div className="text-[14px]/5 font-medium">{headline}</div>
          <div className="mt-0.5 text-[13px]/5 break-words text-muted-foreground [overflow-wrap:anywhere]">
            {detail ?? "The reply didn't make it. Try again?"}
          </div>
        </div>
      </div>
      <div className="flex items-center gap-2 pl-[27.5px]">
        {gate ? (
          <button
            type="button"
            onClick={() => host.openSettings("plan")}
            className="cursor-pointer rounded-full bg-primary px-3 py-1.5 text-[13px]/4 font-medium text-primary-foreground transition-[background-color,scale] duration-150 hover:bg-(--primary-hover) active:scale-[0.96]"
          >
            See plans
          </button>
        ) : (
          onRetry && (
            <button
              type="button"
              onClick={onRetry}
              className="cursor-pointer rounded-full bg-primary px-3 py-1.5 text-[13px]/4 font-medium text-primary-foreground transition-[background-color,scale] duration-150 hover:bg-(--primary-hover) active:scale-[0.96]"
            >
              Try again
            </button>
          )
        )}
      </div>
    </div>
  );
}

function MessageActions({
  message,
  onRetry,
  onBranch,
}: {
  message: ChatMessage;
  onRetry?: () => void;
  onBranch?: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const text = message.content;
  return (
    <div className="mt-1.5 flex items-center gap-0.5 opacity-0 transition-opacity duration-150 group-hover/msg:opacity-100 focus-within:opacity-100 has-data-popup-open:opacity-100 coarse:opacity-100">
      {text.length > 0 && (
        <MessageActionButton
          label="Copy message"
          tooltip={copied ? "Copied" : "Copy message"}
          onClick={() => {
            navigator.clipboard
              .writeText(text)
              .then(() => {
                setCopied(true);
                setTimeout(() => setCopied(false), 1500);
              })
              .catch(() => {});
          }}
        >
          {copied ? <IconCheck size={15} /> : <IconCopy size={15} />}
        </MessageActionButton>
      )}
      {onRetry && (
        <MessageActionButton label="Retry message" onClick={onRetry}>
          <IconRefresh size={15} />
        </MessageActionButton>
      )}
      {onBranch && <CheckpointMenu onBranch={onBranch} />}
      {(message.model || message.usageCost) && (
        <div className="ml-1.5 flex items-center gap-2.5 px-0.5 text-[11.5px]/4 font-medium tabular-nums text-muted-foreground">
          {message.model && <span>{message.model}</span>}
          {message.usageCost ? <span>${message.usageCost.toFixed(4)}</span> : null}
        </div>
      )}
    </div>
  );
}

/* Cloud's ad-supported plans: a labelled card under the reply, never part
   of it. Dismissable; "Remove ads" goes to the plans. */
function AdCard({ ad }: { ad: Ad }) {
  const [hidden, setHidden] = useState(false);
  const host = getHost();
  if (hidden) return null;
  const go = () => {
    if (ad.url === "backspace://plans") host.openSettings("plan");
    else if (ad.url.startsWith("backspace://settings/")) host.openSettings(ad.url.split("/").pop());
    else host.openUrl(ad.url);
  };
  return (
    <aside aria-label="Sponsored" className={`mt-3 flex w-full max-w-md flex-col gap-1.5 rounded-2xl bg-well px-4 py-3 ${WELL}`}>
      <div className="flex items-center gap-2 text-[11px]/4 text-muted-foreground">
        <span className="rounded-md border border-border px-1.5 font-semibold tracking-wide uppercase">Sponsored</span>
        <span>{ad.advertiser}</span>
        <button
          type="button"
          aria-label="Hide this ad"
          onClick={() => setHidden(true)}
          className="ml-auto flex size-6 cursor-pointer items-center justify-center rounded-full hover:bg-black/[0.05] hover:text-foreground dark:hover:bg-white/[0.06]"
        >
          <IconX size={13} />
        </button>
      </div>
      <div className="text-[14px]/5 font-medium">{ad.title}</div>
      <div className="text-[13px]/5 text-muted-foreground">{ad.body}</div>
      <div className="mt-1 flex items-center gap-3">
        <button
          type="button"
          onClick={go}
          className="cursor-pointer rounded-full bg-primary px-3 py-1.5 text-[13px]/4 font-medium text-primary-foreground transition-[background-color,scale] duration-150 hover:bg-(--primary-hover) active:scale-[0.96]"
        >
          {ad.cta}
        </button>
        <button
          type="button"
          onClick={() => host.openSettings("plan")}
          title="Free plan. Ads are not chosen from your messages."
          className="cursor-pointer text-[12px]/4 text-muted-foreground hover:text-foreground"
        >
          Remove ads
        </button>
      </div>
    </aside>
  );
}
