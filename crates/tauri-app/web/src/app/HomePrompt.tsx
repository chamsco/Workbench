// The hero: one input where the Agents side starts, modeled on the
// "What should your agents work on?" screen — the prompt first, everything
// else a strip under it. It is the composer you already know with a router
// attached: type a line, the strip says who it goes to and what it becomes,
// Enter does it.
//
//   "@kuvo is the banner copy ready?"   → chat   · Kuvo's session
//   "@kuvo @june ship the banner"       → group  · a new session, both in it
//   "create an agent Watcher that…"     → the new-bot sheet, prefilled
//   "move morning briefing to 9am"      → the routine sheet, time applied
//   "buy milk, eggs and bread"          → a card, as shapeshift always did
//
// Nothing here decides anything twice: the plan is pure (router.ts), the strip
// shows it, Enter executes it. What the strip cannot show it does not do.

import { useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import {
  IconClock,
  IconFolder,
  IconHelp,
  IconMessageCircle,
  IconNotes,
  IconServer,
  IconSparkles,
  IconUserPlus,
  IconUsersGroup,
} from "@tabler/icons-react";

import { EASE_OUT, SHED_BLUR } from "@/lib/motion";
import { Avatar } from "./avatar";
import { mentionAtCaret, mentionOptions } from "./mentions";
import { planLine, routePlan, type Plan } from "./router";
import { saveEveryday } from "./shapeshift";
import {
  chatState,
  createGroup,
  editRoutine,
  getHost,
  openAgentThread,
  openBot,
  projectById,
  send,
  startBot,
  useChat,
} from "../bridge";

const WELL = "shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]";
const GLYPH: Record<Plan["kind"], typeof IconSparkles | null> = {
  chat: IconMessageCircle,
  group: IconUsersGroup,
  "new-agent": IconUserPlus,
  routine: IconClock,
  card: IconNotes,
  pick: IconHelp,
  empty: null,
};
const STRIP_SPRING = { type: "spring", stiffness: 900, damping: 55, mass: 0.5 } as const;

export function HomePrompt() {
  const host = getHost();
  const { agents, projects, threads } = useChat();
  const [text, setText] = useState("");
  const [caret, setCaret] = useState(0);
  const [pick, setPick] = useState(-1); // the autocomplete's highlighted row
  const taRef = useRef<HTMLTextAreaElement>(null);

  const plan = useMemo(() => routePlan(text, agents), [text, agents]);
  const mention = mentionAtCaret(text, caret);
  const options = useMemo(
    () => (mention ? mentionOptions(mention.query, mention.machine, agents) : []),
    [mention, agents],
  );
  const folder = host.project();
  const project = folder ? projects.find((p) => p.folder === folder) ?? null : null;

  useEffect(() => {
    setPick(options.length ? 0 : -1);
  }, [options.length]);

  const insert = (name: string) => {
    if (!mention) return;
    const next = `${text.slice(0, mention.start)}@${name} ${text.slice(caret)}`;
    setText(next);
    requestAnimationFrame(() => {
      const el = taRef.current;
      if (!el) return;
      const at = mention.start + name.length + 2;
      el.focus();
      el.setSelectionRange(at, at);
      setCaret(at);
    });
  };

  const run = async () => {
    if (plan.kind === "empty") return;
    setText("");
    if (plan.kind === "card") {
      saveEveryday(text);
      return;
    }
    if (plan.kind === "new-agent") {
      startBot(project?.id ?? "", { name: plan.name, job: plan.job });
      return;
    }
    if (plan.kind === "routine") {
      editRoutine(plan.agent.id, { ...plan.routine, time: plan.time ?? plan.routine.time });
      return;
    }
    if (plan.kind === "pick") {
      host.toast("Name a bot with @, or say what the line is about", "err");
      return;
    }
    if (plan.kind === "group") {
      try {
        await createGroup(plan.title, plan.goal, plan.agents.map((a) => a.id));
        const t = chatState().thread;
        // Through send(): a group's first message is a message like any other,
        // and it opens (or renews) the session the same way.
        if (t && plan.text) await send(plan.text, [], t.route, false);
      } catch (e) {
        host.toast(String(e), "err");
      }
      return;
    }
    // chat
    const a = plan.agents[0];
    await openAgentThread(a, project ?? null);
    const t = chatState().thread;
    if (t && plan.text) await send(plan.text, [], t.route ?? a.route, false);
  };

  const line = planLine(plan);
  const Glyph = GLYPH[plan.kind];
  const busy = threads.some((t) => t.busy);

  return (
    <div className="flex flex-col items-center gap-4 pb-2">
      <div className="text-center">
        <h1 className="text-[26px]/9 font-medium tracking-tight md:text-[32px]/10">What should your agents work on?</h1>
        <p className="mt-2 text-[13px]/5 text-muted-foreground">
          Describe it, or @mention a bot to route it. Two mentions open a group; "create an agent…" makes one; a schedule change finds the routine.
        </p>
      </div>

      <div className="relative w-full max-w-2xl">
        {/* The plan strip: what Enter will do, said before it does it. Same
            shape as the everyday strip — one mode up, not a second surface. */}
        <AnimatePresence initial={false}>
          {plan.kind !== "empty" && (
            <motion.div
              key="plan"
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ height: STRIP_SPRING, opacity: { duration: 0.15, ease: EASE_OUT } }}
              className="overflow-hidden"
            >
              <div className={`mb-2 flex items-center gap-2.5 rounded-2xl bg-well px-3.5 py-2.5 ${WELL}`}>
                {Glyph && <Glyph size={16} className="shrink-0 text-muted-foreground" />}
                <span className="min-w-0 flex-1 truncate text-[13px]/5 text-foreground">{line.text}</span>
                {plan.kind === "chat" &&
                  plan.agents.map((a) => (
                    <button key={a.id} type="button" onClick={() => openBot(a.id)} title={`Open ${a.name}'s page`} className="shrink-0">
                      <Avatar agent={a} size={22} />
                    </button>
                  ))}
                <span className="hidden shrink-0 items-center text-[11px]/4 text-muted-foreground sm:flex">Enter routes · Esc clears</span>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        <div className={`rounded-[26px] bg-well ${WELL}`}>
          <textarea
            ref={taRef}
            value={text}
            rows={2}
            onChange={(e) => {
              setText(e.target.value);
              setCaret(e.target.selectionStart ?? e.target.value.length);
            }}
            onSelect={(e) => setCaret((e.target as HTMLTextAreaElement).selectionStart ?? 0)}
            onKeyDown={(e) => {
              if (mention && options.length > 0) {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setPick((i) => (i + 1) % options.length);
                  return;
                }
                if (e.key === "ArrowUp") {
                  e.preventDefault();
                  setPick((i) => (i - 1 + options.length) % options.length);
                  return;
                }
                if (e.key === "Enter" || e.key === "Tab") {
                  e.preventDefault();
                  insert(options[Math.max(0, pick)].name);
                  return;
                }
              }
              if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                void run();
              }
              if (e.key === "Escape") setText("");
            }}
            placeholder={agents.length ? "Describe a task, an idea, a bug — or @mention a bot…" : "No bots yet. Describe the bot you want and it gets created…"}
            aria-label="What should your agents work on?"
            className="block w-full resize-none bg-transparent px-4 py-3.5 text-[14px]/6 outline-none placeholder:text-muted-foreground"
          />
          <div className="flex items-center gap-1.5 px-3 pb-3">
            <button
              type="button"
              onClick={() => host.newProject()}
              className="flex cursor-pointer items-center gap-1.5 rounded-full bg-foreground/[0.06] px-2.5 py-1 text-[12px] font-medium text-muted-foreground hover:text-foreground"
              title="The folder new work runs in"
            >
              <IconFolder size={13} />
              {project ? project.name : folder ? folder.split(/[\\/]/).pop() : "No project"}
            </button>
            <span className="flex cursor-default items-center gap-1.5 rounded-full bg-foreground/[0.06] px-2.5 py-1 text-[12px] font-medium text-muted-foreground" title="Where the work happens">
              <IconServer size={13} />
              {host.machineName()}
            </span>
            <span className="flex cursor-default items-center gap-1.5 rounded-full bg-foreground/[0.06] px-2.5 py-1 text-[12px] font-medium text-muted-foreground" title="Routing is automatic — @mention to pin it">
              <IconSparkles size={13} />
              Auto{busy ? " · a bot is working" : ""}
            </span>
            <span className="ml-auto text-[11px]/4 text-muted-foreground/70">
              {agents.length} bot{agents.length === 1 ? "" : "s"} · {projectById(project?.id)?.agents.length ?? 0} assigned
            </span>
          </div>
        </div>

        {/* The @ autocomplete: bots, and the machines they live on. */}
        <AnimatePresence>
          {mention && options.length > 0 && (
            <motion.div
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: 4 }}
              transition={{ duration: 0.12, ease: EASE_OUT }}
              className={`absolute bottom-full left-0 z-30 mb-2 w-72 overflow-hidden rounded-2xl bg-surface p-1 shadow-[0_16px_50px_rgba(0,0,0,.28)] ring-1 ring-border ${WELL}`}
            >
              <div className="px-2.5 py-1 text-[10.5px]/4 font-medium text-muted-foreground/55">
                {mention.machine ? `On ${mention.machine}` : "Route to"}
              </div>
              {options.map((a, i) => (
                <button
                  key={a.id}
                  type="button"
                  onMouseEnter={() => setPick(i)}
                  onClick={() => insert(a.name)}
                  className={`flex w-full cursor-pointer items-center gap-2.5 rounded-xl px-2.5 py-1.5 text-left ${i === pick ? "bg-accent" : ""}`}
                >
                  <Avatar agent={a} size={24} />
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate text-[13px]/4 font-medium">{a.name}</span>
                    <span className="truncate text-[11px]/4 text-muted-foreground">{a.job.split("\n")[0] || "No job yet"}</span>
                  </span>
                  {mention.machine && <span className="shrink-0 font-mono text-[10.5px] text-muted-foreground">@{mention.machine}/{a.name}</span>}
                </button>
              ))}
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
