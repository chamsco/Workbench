// The message shapes Whirl's thread components read (from Whirl's
// lib/messages.ts, MIT). Backspace fills them from its own chat threads in
// ../../bridge.ts.

export type MessageStatus = "pending" | "streaming" | "complete" | "stopped" | "error";

export type MessageAttachment = {
  id: string;
  name: string;
  size: number;
  type: string;
  url?: string;
  text?: string;
  skippedReason?: string;
};

export type ChatMessage = {
  id: string;
  role: "user" | "assistant";
  content: string;
  createdAt: number;
  status?: MessageStatus;
  streamId?: string;
  model?: string;
  attachments?: MessageAttachment[];
  outputTokens?: number;
  durationMs?: number;
  usageCost?: number;
};

export const TERMINAL_STATUSES: ReadonlySet<MessageStatus> = new Set(["complete", "stopped", "error"]);

export function isTerminal(status: MessageStatus | undefined): boolean {
  return status === undefined || TERMINAL_STATUSES.has(status);
}

export function computeIsGenerating(messages: ChatMessage[] | undefined): boolean {
  const last = messages?.[messages.length - 1];
  return !!last && last.role === "assistant" && !isTerminal(last.status);
}
