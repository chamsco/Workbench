// The home field: a sparse matrix of glyphs drifting down behind the
// greeting, in the accent the Appearance popover picks. Painted on a canvas
// so a full pane of text costs one composited layer, not a thousand nodes.

import { useEffect, useRef } from "react";

const GLYPHS = "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホ0123456789<>/\\|=+-*:.";

export function AsciiField({ className = "bs-ascii-field" }: { className?: string }) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)");

    let timer = 0;
    let drops: number[] = [];
    let speeds: number[] = [];
    let size = 14;
    let cols = 0;
    let w = 0;
    let h = 0;

    const fit = () => {
      const rect = canvas.getBoundingClientRect();
      if (rect.width < 1 || rect.height < 1) return;
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      w = rect.width;
      h = rect.height;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      size = Math.max(11, Math.round(w / 92));
      cols = Math.ceil(w / size);
      drops = Array.from({ length: cols }, () => Math.random() * h * 1.1 - h * 0.1);
      speeds = Array.from({ length: cols }, () => 0.3 + Math.random() * 0.8);
      ctx.clearRect(0, 0, w, h);
    };

    const draw = () => {
      if (!w || !h) return;
      const color = getComputedStyle(canvas).color || "#00ff99";
      ctx.clearRect(0, 0, w, h);
      ctx.fillStyle = color;
      ctx.font = `${size}px "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace`;
      ctx.textBaseline = "top";
      const trail = 7;
      for (let i = 0; i < cols; i++) {
        const head = drops[i] ?? 0;
        const ch = GLYPHS[(Math.random() * GLYPHS.length) | 0];
        for (let t = 0; t < trail; t++) {
          const y = head - t * size * 1.06;
          if (y < -size || y > h) continue;
          ctx.globalAlpha = t === 0 ? 0.95 : 0.72 * (1 - t / trail);
          ctx.fillText(t === 0 ? ch : GLYPHS[(Math.random() * GLYPHS.length) | 0], i * size, y);
        }
        drops[i] = head + (speeds[i] ?? 1);
        if (head * 1 > h && Math.random() > 0.972) drops[i] = -Math.random() * h * 0.25;
      }
      ctx.globalAlpha = 1;
    };

    const start = () => {
      fit();
      draw();
      if (!reduce.matches && !timer) timer = window.setInterval(draw, 70);
    };
    const stop = () => {
      if (timer) window.clearInterval(timer);
      timer = 0;
    };

    const observer = new ResizeObserver(() => {
      fit();
      if (reduce.matches) draw();
    });
    observer.observe(canvas);
    const onVisibility = () => (document.hidden ? stop() : start());
    document.addEventListener("visibilitychange", onVisibility);
    start();

    return () => {
      stop();
      observer.disconnect();
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, []);

  return <canvas ref={ref} aria-hidden className={className} />;
}
