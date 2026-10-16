// Code → ADE's landing: the zone's own home, not a second one.
//
// This is `home.js` — the "What should your agents work on?" screen — drawn into
// a node the chat face owns, with its ask line left out. The ADE's composer sits
// under it and carries the same project / branch / machine chips, so the hero,
// the pills and the inbox are what belong here. One screen, one implementation:
// if the home grows a pill, the ADE has it the same afternoon.

import { useEffect, useRef } from "react";

import { useChat } from "../bridge";

/** The vanilla home, mounted where React asks for it. */
type HomeModule = { render: (el: HTMLElement, opts?: { composer?: boolean }) => void };

export function CodeStart() {
  const host = useRef<HTMLDivElement>(null);
  const { epoch } = useChat();

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    // The home owns a child of ours, not this element: it sets className and
    // innerHTML on whatever it is handed, and React is still tracking this one.
    let inner = el.firstElementChild as HTMLElement | null;
    if (!inner) {
      inner = document.createElement("div");
      inner.className = "h-full min-h-0";
      el.appendChild(inner);
    }
    const home = (window as unknown as { Home?: HomeModule }).Home;
    if (!home) return;
    // eslint-disable-next-line react-hooks/exhaustive-deps
    home.render(inner, { composer: false });
  }, [epoch]);

  return <div ref={host} className="h-full min-h-0" />;
}
