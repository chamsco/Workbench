// Rich answers: an assistant reply is Markdown, except ```openui blocks,
// which are OpenUI Lang (thesysdev/openui, MIT) rendered as real UI: cards,
// tables, charts, steps, follow-ups. The model learns the language from a
// prompt built from a small subset of OpenUI's chat library (setRich), so the
// cost per turn stays a couple of thousand tokens.

import { Component, type ReactNode, useEffect, useRef, useState } from "react";
import { Renderer, createLibrary } from "@openuidev/react-lang";
import { ThemeProvider, openuiChatLibrary } from "@openuidev/react-ui";
import "@openuidev/react-ui/layered/styles/index.css";

import { Markdown } from "@/components/thread/markdown";

// What the model is taught; the renderer takes the whole chat library, so
// anything it writes still shows.
const TAUGHT = ["Card", "CardHeader", "TextContent", "Callout", "Table", "Col", "BarChart", "LineChart", "PieChart", "Series", "Slice", "Steps", "StepsItem", "FollowUpBlock", "FollowUpItem", "Tag", "TagBlock", "CodeBlock", "Separator", "ListBlock", "ListItem"];

export function richPrompt(): string {
  const comps = TAUGHT.map((k) => (openuiChatLibrary.components as Record<string, any>)[k]).filter(Boolean);
  return createLibrary({ components: comps, root: "Card" }).prompt({ preamble: "", additionalRules: [] });
}

const FENCE = /```openui[^\n]*\n([\s\S]*?)(?:```|$)/g;

/**
 * A ````openui` block the renderer cannot turn into UI.
 *
 * This used to be impossible to see: when the model wrote the language wrong,
 * or wrote something that is not the language at all (a Python-looking
 * `root = Card(children: [...])`, which small local models like to produce),
 * the renderer either threw where React could not catch it or quietly rendered
 * nothing, and the raw text showed up in the middle of the reply as prose.
 * Now the block is kept — the model did say something — but labelled as
 * something that could not be drawn, with the source one click away so it can
 * be copied back to the model.
 */
function FailedBlock({ source, why }: { source: string; why: string }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="openui-failed my-3 rounded-lg border border-amber-500/30 bg-amber-500/[0.06] p-3 text-sm">
      <div className="flex items-center gap-2 text-amber-700 dark:text-amber-400">
        <span aria-hidden>⚠</span>
        <span className="font-medium">This block could not be drawn as UI: {why}</span>
        <button
          type="button"
          className="ml-auto rounded px-2 py-0.5 text-xs underline decoration-dotted hover:bg-amber-500/10"
          onClick={() => setOpen((v) => !v)}
        >
          {open ? "Hide source" : "Show source"}
        </button>
      </div>
      {open && (
        <pre className="mt-2 max-h-64 overflow-auto rounded bg-black/[0.04] p-2 text-xs dark:bg-white/[0.06]">
          <code>{source}</code>
        </pre>
      )}
    </div>
  );
}

/** Catches a renderer that throws, so one bad block cannot take the reply down. */
class CatchRender extends Component<{ fallback: ReactNode; children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}

function useDark() {
  const get = () => document.documentElement.classList.contains("dark");
  const [dark, setDark] = useState(get);
  useEffect(() => {
    const o = new MutationObserver(() => setDark(get()));
    o.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => o.disconnect();
  }, []);
  return dark;
}

/**
 * One drawn block, with the two ways it can fail handled: it throws (caught
 * above) or it draws nothing at all (measured after paint). Neither is left to
 * leak the raw language into the answer.
 */
function OpenUIBlock({ source, dark, streaming }: { source: string; dark: boolean; streaming: boolean }) {
  const host = useRef<HTMLDivElement>(null);
  const [empty, setEmpty] = useState(false);
  useEffect(() => {
    if (streaming) return;
    const el = host.current;
    if (!el) return;
    // One frame for the renderer to mount, then ask whether anything is there.
    const id = requestAnimationFrame(() => {
      if (el.childElementCount === 0) setEmpty(true);
    });
    return () => cancelAnimationFrame(id);
  }, [source, streaming]);

  if (empty) return <FailedBlock source={source} why="the renderer produced nothing" />;
  return (
    <div ref={host} className="openui-block my-3">
      <ThemeProvider mode={dark ? "dark" : "light"}>
        <Renderer response={source} library={openuiChatLibrary} isStreaming={streaming} />
      </ThemeProvider>
    </div>
  );
}

/**
 * OpenUI Lang written without the fence.
 *
 * The fenced case is handled below, but a small local model will often emit the
 * language bare — `root = Card(children: [...])` — and then it was not source at
 * all to the Markdown renderer, it was prose. The reader got a paragraph of
 * Python-looking noise in the middle of the reply. It is source either way, so
 * it is cut here. Balanced-paren walk, so a multi-line call goes as a unit; if
 * the expression never closes, only its first line goes, and the rest of the
 * reply is left alone.
 */
export function stripBareOpenUI(text: string): string {
  if (!/^\s*root\s*=\s*[A-Za-z_]\w*\s*\(/m.test(text)) return text;
  const lines = text.split("\n");
  const keep: string[] = [];
  for (let i = 0; i < lines.length; i++) {
    if (!/^\s*root\s*=\s*[A-Za-z_]\w*\s*\(/.test(lines[i])) {
      keep.push(lines[i]);
      continue;
    }
    let depth = 0;
    let j = i;
    let closed = false;
    for (; j < lines.length; j++) {
      for (const ch of lines[j]) {
        if (ch === "(") depth++;
        else if (ch === ")") depth--;
      }
      if (depth <= 0) {
        closed = true;
        break;
      }
    }
    i = closed ? j : i;
  }
  return keep.join("\n").replace(/\n{3,}/g, "\n\n").trim();
}

export function RichText({ text, streaming }: { text: string; streaming: boolean }) {
  const dark = useDark();
  const clean = stripBareOpenUI(text);
  if (!clean.includes("```openui")) return <Markdown streaming={streaming}>{clean}</Markdown>;
  const parts: { md?: string; ui?: string; open?: boolean }[] = [];
  let last = 0;
  for (const m of clean.matchAll(FENCE)) {
    if (m.index! > last) parts.push({ md: clean.slice(last, m.index) });
    parts.push({ ui: m[1], open: !m[0].trimEnd().endsWith("```") });
    last = m.index! + m[0].length;
  }
  if (last < clean.length) parts.push({ md: clean.slice(last) });
  return (
    <>
      {parts.map((p, i) =>
        p.ui != null ? (
          <CatchRender key={i} fallback={<FailedBlock source={p.ui} why="the renderer rejected it" />}>
            <OpenUIBlock source={p.ui} dark={dark} streaming={streaming && !!p.open} />
          </CatchRender>
        ) : p.md!.trim() ? (
          <Markdown key={i} streaming={streaming && i === parts.length - 1}>{p.md!}</Markdown>
        ) : null,
      )}
    </>
  );
}
