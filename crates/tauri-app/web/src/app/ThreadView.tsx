// Whirl's components/thread/thread-view.tsx (MIT, src/whirl/LICENSE): the
// same scroller, spacing and row layout, fed from a Backspace thread.

import { memo, useMemo } from "react";
import { motion } from "motion/react";

import { computeIsGenerating, type ChatMessage } from "@/lib/messages";
import { EASE_OUT } from "@/lib/motion";
import { cn } from "@/lib/utils";
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from "@/components/ui/message-scroller";
import { UserMessage } from "@/components/thread/user-message";
import { AssistantMessage } from "./AssistantMessage";
import { branch, edit, getHost, retry, toChatMessage, type Msg, type Thread } from "../bridge";

const PREVIOUS_TURN_PEEK_PX = 72;
const EDGE_THRESHOLD_PX = 128;

export function ThreadView({ thread }: { thread: Thread }) {
  const messages = useMemo(() => thread.messages.map((m) => toChatMessage(thread, m)), [thread]);
  const generating = computeIsGenerating(messages);
  const host = getHost();
  const lastAssistant = [...thread.messages].reverse().find((m) => m.role === "assistant")?.id;

  if (messages.length === 0) {
    return (
      <div className="flex h-full items-center justify-center px-6 pb-40 text-center">
        <div className="text-[15px]/6 text-muted-foreground">
          Say hello to {host.routeName(thread.route)}. Replies stream in here.
        </div>
      </div>
    );
  }

  return (
    <motion.div
      key={thread.id}
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ opacity: { duration: 0.22, ease: EASE_OUT }, y: { type: "spring", stiffness: 520, damping: 38 } }}
      className="h-full min-h-0"
    >
      <MessageScrollerProvider
        autoScroll={generating}
        defaultScrollPosition="end"
        scrollEdgeThreshold={EDGE_THRESHOLD_PX}
        scrollPreviousItemPeek={PREVIOUS_TURN_PEEK_PX}
      >
        <MessageScroller>
          <MessageScrollerViewport className="px-3 md:px-6">
            <MessageScrollerContent
              className={cn(
                "mx-auto w-full max-w-2xl pt-8 pb-[max(11rem,calc(var(--dock-clearance,0px)+0.75rem))]",
              )}
            >
              {thread.messages.map((m, i) => (
                <Row
                  key={m.id}
                  thread={thread}
                  raw={m}
                  message={messages[i]}
                  canRetry={m.id === lastAssistant && !generating}
                  generating={generating}
                />
              ))}
            </MessageScrollerContent>
          </MessageScrollerViewport>
          <MessageScrollerButton className="bottom-32" />
        </MessageScroller>
      </MessageScrollerProvider>
    </motion.div>
  );
}

const Row = memo(function Row({
  thread,
  raw,
  message,
  canRetry,
  generating,
}: {
  thread: Thread;
  raw: Msg;
  message: ChatMessage;
  canRetry: boolean;
  generating: boolean;
}) {
  const host = getHost();
  const toast = (e: unknown) => host.toast(String(e), "err");
  const answeredBy = raw.via ? `${host.providerName(raw.via)} answered` : undefined;
  return (
    <MessageScrollerItem messageId={message.id} scrollAnchor={message.role === "user"}>
      {message.role === "user" ? (
        <UserMessage
          message={message}
          onEdit={generating ? undefined : (content) => void edit(thread.id, raw.id, content).catch(toast)}
        />
      ) : (
        <AssistantMessage
          message={message}
          error={raw.error}
          answeredBy={answeredBy}
          ad={raw.ad}
          onRetry={canRetry ? () => void retry(thread.id).catch(toast) : undefined}
          onBranch={() => void branch(thread.id, raw.id).catch(toast)}
        />
      )}
    </MessageScrollerItem>
  );
});
