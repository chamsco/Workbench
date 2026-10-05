// Who answers: Whirl's components/model-select.tsx (MIT, src/whirl/LICENSE)
// look — the chip in the composer, the frosted popover above it, its rows
// and separators — over Backspace's routes: coding CLIs on this machine,
// Ollama models, routers, and Backspace Cloud's plan-gated models.

import { useMemo } from "react";
import {
  IconBrandOpenai,
  IconCheck,
  IconChevronRight,
  IconCloud,
  IconCpu,
  IconLockFilled,
  IconPointer,
  IconRoute,
  IconSettings,
  IconSparkles,
  IconTerminal2,
  IconLetterX,
  type Icon,
} from "@tabler/icons-react";

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { getHost, useChat, type Route, type RouteItem } from "../bridge";

const ROW =
  "flex w-full cursor-pointer items-center gap-2 rounded-lg px-2 py-1 text-left text-[13.5px]/5 transition-colors duration-100 hover:bg-accent hover:text-accent-foreground";

export function glyphFor(id: string | undefined): Icon {
  switch (id) {
    case "claude":
      return IconSparkles;
    case "codex":
      return IconBrandOpenai;
    case "cursor":
      return IconPointer;
    case "grok":
      return IconLetterX;
    case "opencode":
      return IconTerminal2;
    case "ollama":
      return IconCpu;
    case "cloud":
      return IconCloud;
    default:
      return id?.startsWith("router:") ? IconRoute : IconTerminal2;
  }
}

export const routeGlyphId = (r: Route | null) =>
  !r ? "cloud" : r.kind === "cli" ? r.provider : r.kind === "local" ? "ollama" : r.kind === "cloud" ? "cloud" : `router:${r.provider}`;

const same = (a: Route | null | undefined, b: Route | null | undefined) =>
  !!a && !!b && a.kind === b.kind && a.provider === b.provider && (a.model ?? null) === (b.model ?? null);

export function RoutePicker({ value, onValueChange }: { value: Route | null; onValueChange: (r: Route) => void }) {
  const host = getHost();
  const { epoch } = useChat();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const groups = useMemo(() => host.routes(), [epoch]);
  const Glyph = glyphFor(routeGlyphId(value));
  const name = value ? shortName(host.routeName(value)) : "Pick a model";

  const pick = (it: RouteItem) => {
    if (it.setup) {
      host.openSettings("plan");
      return;
    }
    if (it.route && !it.disabled) onValueChange(it.route);
  };

  return (
    <Popover>
      <PopoverTrigger
        aria-label="Choose who answers"
        className="relative flex h-9 max-w-56 shrink-0 cursor-pointer items-center rounded-full px-3 text-[13.5px]/4 font-medium text-muted-foreground transition-colors duration-150 hover:bg-black/[0.05] hover:text-foreground data-popup-open:bg-black/[0.05] data-popup-open:text-foreground dark:hover:bg-white/[0.06] dark:data-popup-open:bg-white/[0.06]"
      >
        <span className="flex min-w-0 items-center gap-1.5 whitespace-nowrap">
          <Glyph size={15} className="shrink-0" />
          <span className="truncate">{name}</span>
        </span>
      </PopoverTrigger>
      <PopoverContent side="top" align="end" sideOffset={8} className="w-[300px] min-w-0 p-0">
        <div className="max-h-[min(60vh,420px)] overflow-y-auto p-1">
          {groups.map((g, gi) => (
            <div key={g.name}>
              {gi > 0 && <div className="my-1 h-px bg-border" />}
              <div className="flex items-baseline gap-1.5 px-2 pt-1 pb-0.5 text-[11.5px]/4 font-medium text-muted-foreground">
                {g.name}
                {g.sub && <span className="truncate font-normal opacity-70">{g.sub}</span>}
              </div>
              {g.items.map((it, i) => {
                const G = glyphFor(it.id);
                const on = same(it.route, value);
                return (
                  <button
                    key={`${g.name}-${i}`}
                    type="button"
                    disabled={it.disabled}
                    onClick={() => pick(it)}
                    className={`${ROW} disabled:cursor-default disabled:opacity-50 disabled:hover:bg-transparent`}
                  >
                    <G size={15} className="shrink-0 text-muted-foreground" />
                    <span className="flex min-w-0 flex-col">
                      <span className="truncate">{it.label}</span>
                      {it.sub && <span className="truncate text-[11.5px]/4 text-muted-foreground">{it.sub}</span>}
                    </span>
                    {it.badge && (
                      <span className="ml-auto flex shrink-0 items-center gap-1 rounded-full bg-black/[0.05] px-2 py-0.5 text-[11px]/4 font-medium text-muted-foreground dark:bg-white/[0.08]">
                        {it.badge === "Upgrade" && <IconLockFilled size={10} />}
                        {it.badge}
                      </span>
                    )}
                    {it.setup && <IconChevronRight size={13} className="ml-auto shrink-0 text-muted-foreground" />}
                    {on && <IconCheck size={14} className="ml-auto shrink-0 text-muted-foreground" />}
                  </button>
                );
              })}
            </div>
          ))}
          <div className="my-1 h-px bg-border" />
          <button type="button" className={ROW} onClick={() => host.openSettings("providers")}>
            <IconSettings size={15} className="shrink-0 text-muted-foreground" />
            Manage CLIs and models
          </button>
        </div>
      </PopoverContent>
    </Popover>
  );
}

/* "Ollama · qwen2.5-coder:7b" reads long in a chip; keep the model. */
function shortName(n: string) {
  const parts = n.split(" · ");
  return parts.length > 1 && parts[0] !== "Cloud" ? parts.slice(1).join(" · ") : parts[parts.length - 1];
}
