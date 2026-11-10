// The real open-bot-avatar face, as a React atom.
//
// The package is loaded by the shell as a classic script (`ui/vendor/
// open-bot-avatar.js`), so it is on `window` before the chat face mounts. We
// paint its standalone SVG first — that is the whole avatar when the animation
// clock is paused — then let its own mount take over and add volume, blinking
// and gaze. If the script did not load, the caller falls back to the orb.

import { useEffect, useRef } from "react";

type Mood = "idle" | "thinking" | "happy";
type FaceState = "idle" | "working" | "needs" | "wake";
type OBA = {
  brain?: unknown;
  renderAvatarSvg: (definition: unknown, options: Record<string, unknown>) => string;
  mountAvatar: (el: HTMLElement, options: Record<string, unknown>) => { destroy: () => void };
};

declare global {
  interface Window {
    OpenBotAvatar?: OBA;
  }
}

export const hasPackageFace = () => typeof window !== "undefined" && !!window.OpenBotAvatar?.brain;

const moodOf = (state: FaceState): Mood => (state === "working" ? "thinking" : state === "needs" ? "happy" : "idle");

export function PackageFace({
  color = "#64748b",
  size = 32,
  state = "idle",
  className = "",
}: {
  color?: string;
  size?: number;
  state?: FaceState;
  className?: string;
}) {
  const host = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const el = host.current;
    const oba = window.OpenBotAvatar;
    if (!el || !oba?.brain) return;
    const mood = moodOf(state);
    let mounted: { destroy: () => void } | null = null;
    try {
      el.innerHTML = oba.renderAvatarSvg(oba.brain, {
        color,
        size,
        depth: 6,
        motion: mood,
        reducedMotion: true,
      });
      mounted = oba.mountAvatar(el, {
        definition: oba.brain,
        color,
        depth: 6,
        mood,
        followPointer: false,
        label: "",
        reducedMotion: "system",
      });
    } catch {
      // The static SVG is already in place; a failed mount is still a face.
    }
    return () => mounted?.destroy();
  }, [color, size, state]);

  return (
    <span
      ref={host}
      className={`bs-bot-face ${className}`}
      style={{ width: size, height: size, display: "inline-block", position: "relative", lineHeight: 0, flex: "none" }}
      aria-hidden="true"
    />
  );
}
