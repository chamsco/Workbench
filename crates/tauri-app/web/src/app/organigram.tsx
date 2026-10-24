// Who works for whom, drawn by Pele (Mermaid text to SVG, MIT): you at the
// top, the projects your bots work in, and each bot under the project it is on.
// A bot with no project hangs off you; a group chat is a dotted ring around
// its members. Clicking a bot opens its page.

import { useEffect, useMemo, useRef } from "react";

import { openBot, useChat, type Agent } from "../bridge";

/** Mermaid labels are quoted strings: keep quotes and brackets out of them. */
const label = (s: string) => s.replace(/["<>[\](){}]/g, "").replace(/\s+/g, " ").trim().slice(0, 48);

export function organigram(agents: Agent[], projects: { id: string; name: string; agents: string[] }[], groups: { id: string; title: string; members: string[] }[]): string {
  const lines = ["flowchart TD", '  you(["You"])'];
  const placed = new Set<string>();
  projects.forEach((p, i) => {
    const mine = p.agents.filter((id) => agents.some((a) => a.id === id));
    if (!mine.length) return;
    lines.push(`  p${i}["${label(p.name)}"]`, `  you --> p${i}`);
    for (const id of mine) {
      if (!placed.has(id)) lines.push(`  ${node(agents.find((a) => a.id === id)!)}`);
      lines.push(`  p${i} --> b_${safe(id)}`);
      placed.add(id);
    }
  });
  for (const a of agents.filter((a) => !placed.has(a.id))) {
    lines.push(`  ${node(a)}`, `  you --> b_${safe(a.id)}`);
    placed.add(a.id);
  }
  groups.forEach((g, i) => {
    const members = g.members.filter((id) => placed.has(id));
    if (members.length < 2) return;
    lines.push(`  g${i}(("${label(g.title)}"))`);
    for (const id of members) lines.push(`  b_${safe(id)} -.- g${i}`);
  });
  return lines.join("\n");
}

const safe = (id: string) => id.replace(/[^A-Za-z0-9_]/g, "_");
const node = (a: Agent) => `b_${safe(a.id)}["${label(a.name)}"]`;

export function Organigram() {
  const { agents, projects, threads } = useChat();
  const ref = useRef<HTMLDivElement>(null);
  const text = useMemo(() => {
    const groups = threads.filter((t) => (t.members?.length ?? 0) > 1).map((t) => ({ id: t.id, title: t.title, members: t.members ?? [] }));
    return organigram(agents, projects.map((p) => ({ id: p.id, name: p.name, agents: p.agents })), groups);
  }, [agents, projects, threads]);
  useEffect(() => {
    const el = ref.current;
    if (!el || agents.length < 2) return;
    let live = true;
    // Each bot's node carries its id, so a listener on it opens the bot's page.
    void import("pele/lazy").then(async ({ renderAsync }) => {
      const { svg } = await renderAsync(text);
      if (!live) return;
      el.innerHTML = svg;
      for (const a of agents) {
        const n = el.querySelector(`[id*="b_${safe(a.id)}"]`) as HTMLElement | null;
        if (!n) continue;
        n.style.cursor = "pointer";
        n.addEventListener("click", () => openBot(a.id));
      }
    });
    return () => {
      live = false;
    };
  }, [text, agents]);
  if (agents.length < 2) return null;
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-[13.5px] font-semibold">Who works on what</h3>
      <div ref={ref} className="organigram overflow-x-auto [&_svg]:mx-auto [&_svg]:max-w-full" aria-label="Organigram of your agents" />
    </section>
  );
}
