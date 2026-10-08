// Rich answers: an assistant reply is Markdown, except ```openui blocks,
// which are OpenUI Lang (thesysdev/openui, MIT) rendered as real UI: cards,
// tables, charts, steps, follow-ups. The model learns the language from a
// prompt built from a small subset of OpenUI's chat library (setRich), so the
// cost per turn stays a couple of thousand tokens.

import { useEffect, useState } from "react";
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

export function RichText({ text, streaming }: { text: string; streaming: boolean }) {
  const dark = useDark();
  if (!text.includes("```openui")) return <Markdown streaming={streaming}>{text}</Markdown>;
  const parts: { md?: string; ui?: string; open?: boolean }[] = [];
  let last = 0;
  for (const m of text.matchAll(FENCE)) {
    if (m.index! > last) parts.push({ md: text.slice(last, m.index) });
    parts.push({ ui: m[1], open: !m[0].trimEnd().endsWith("```") });
    last = m.index! + m[0].length;
  }
  if (last < text.length) parts.push({ md: text.slice(last) });
  return (
    <>
      {parts.map((p, i) =>
        p.ui != null ? (
          <div key={i} className="openui-block my-3">
            <ThemeProvider mode={dark ? "dark" : "light"}>
              <Renderer response={p.ui} library={openuiChatLibrary} isStreaming={streaming && !!p.open} />
            </ThemeProvider>
          </div>
        ) : p.md!.trim() ? (
          <Markdown key={i} streaming={streaming && i === parts.length - 1}>{p.md!}</Markdown>
        ) : null,
      )}
    </>
  );
}
