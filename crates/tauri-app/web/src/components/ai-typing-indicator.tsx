// An AI typing indicator after Wensity's component of the same name
// (ui.wensity.com/components/ai-typing-indicator): same props and four
// variants. Its source isn't published (the registry is closed to fetches), so
// this is written from the documented API, not copied.

import { useEffect, useState } from "react";
import { IconBook2, IconBrain, IconCode, IconPencil, IconSearch, type Icon } from "@tabler/icons-react";

export type TypingIcon = "think" | "search" | "read" | "write" | "code";
export type TypingPhrase = string | { text: string; icon?: TypingIcon };

const ICONS: Record<TypingIcon, Icon> = {
  think: IconBrain,
  search: IconSearch,
  read: IconBook2,
  write: IconPencil,
  code: IconCode,
};

const SIZES = { sm: "text-[12px]", md: "text-[13px]", lg: "text-[15px]" } as const;

export function AiTypingIndicator({
  variant = "shimmer",
  label = "Thinking",
  phrases,
  icon = "think",
  interval = 2400,
  showElapsed = false,
  bubble = true,
  size = "md",
  className = "",
}: {
  variant?: "dots" | "shimmer" | "wave" | "matrix";
  /** What a screen reader announces, once. */
  label?: string;
  phrases?: TypingPhrase[];
  icon?: TypingIcon;
  interval?: number;
  /** Elapsed seconds beside the text (shimmer and wave). */
  showElapsed?: boolean;
  /** Dots sit in a reply bubble. */
  bubble?: boolean;
  size?: keyof typeof SIZES;
  className?: string;
}) {
  const [i, setI] = useState(0);
  const [secs, setSecs] = useState(0);
  const many = (phrases?.length ?? 0) > 1;
  useEffect(() => {
    if (!many) return;
    const t = setInterval(() => setI((n) => n + 1), interval);
    return () => clearInterval(t);
  }, [many, interval]);
  useEffect(() => {
    if (!showElapsed) return;
    const t0 = Date.now();
    const t = setInterval(() => setSecs(Math.round((Date.now() - t0) / 1000)), 1000);
    return () => clearInterval(t);
  }, [showElapsed]);

  const cur = phrases?.length ? phrases[i % phrases.length] : label;
  const text = typeof cur === "string" ? cur : cur.text;
  const Glyph = ICONS[(typeof cur === "string" ? undefined : cur.icon) ?? icon];

  let body;
  if (variant === "dots") {
    const dots = (
      <span className="flex items-center gap-1.5">
        <i className="think-dot" />
        <i className="think-dot" />
        <i className="think-dot" />
      </span>
    );
    body = bubble ? (
      <span className="flex h-9 w-[60px] items-center justify-center rounded-full rounded-bl-md bg-well text-muted-foreground shadow-[inset_0_0_0_1px_var(--well-outline)]">
        {dots}
      </span>
    ) : (
      <span className="text-muted-foreground">{dots}</span>
    );
  } else if (variant === "matrix") {
    body = (
      <span className="grid grid-cols-5 gap-[3px] text-muted-foreground">
        {Array.from({ length: 25 }, (_, n) => {
          const d = Math.hypot((n % 5) - 2, Math.floor(n / 5) - 2);
          return <i key={n} className="ati-pt" style={{ animationDelay: `${d * 0.16}s` }} />;
        })}
      </span>
    );
  } else if (variant === "wave") {
    body = (
      <span className={`flex items-center gap-2 text-muted-foreground ${SIZES[size]}`}>
        <span className="flex">
          {[...text].map((c, n) => (
            <span key={`${i}-${n}`} className="ati-wave" style={{ animationDelay: `${n * 0.07}s` }}>
              {c === " " ? " " : c}
            </span>
          ))}
        </span>
        {showElapsed && <span className="tabular-nums opacity-60">{secs}s</span>}
      </span>
    );
  } else {
    body = (
      <span className={`flex items-center gap-2 ${SIZES[size]}`}>
        <Glyph size={size === "lg" ? 17 : 15} className="ati-spin shrink-0 text-muted-foreground" />
        <span key={i} className="ati-shimmer ati-in font-medium">
          {text}
        </span>
        {showElapsed && <span className="tabular-nums text-muted-foreground/70">{secs}s</span>}
      </span>
    );
  }

  return (
    <span role="status" aria-label={label} className={`inline-flex ${className}`}>
      <span aria-hidden>{body}</span>
    </span>
  );
}
