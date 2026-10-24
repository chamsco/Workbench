// The chat face: Whirl's components/chat-view.tsx + home-intro.tsx (MIT,
// src/whirl/LICENSE) — transcript behind a dock that sits centred on home
// (greeting, composer, two suggestion capsules) and floats translucent over
// the transcript's bottom edge in a thread — on Backspace's chat threads.

import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { IconBolt, IconCloud, IconPlugConnected } from "@tabler/icons-react";
import { AnimatePresence, motion } from "motion/react";

import { EASE_OUT, SHED_BLUR, pinRasterPath } from "@/lib/motion";
import { pickFallbacks, type SuggestionSlot } from "@/lib/suggestions";
import { cn } from "@/lib/utils";
import { SuggestionCards } from "@/components/suggestion-cards";
import { Composer } from "./Composer";
import { CodeComposer, CodeSide } from "./CodeComposer";
import { GhostToggle } from "./GhostToggle";
import { ReplyChip } from "./Extras";
import { AgentEditors, AgentsHome, AgentsSide, AgentThreadHeader } from "./Agents";
import { AgentSide } from "./agent-side";
import { VoiceMode } from "./voice";
import { AgentPage } from "./agent-page";
import { ProjectHome } from "./projects";
import { ArtifactView } from "./artifacts";
import { CodeStart } from "./CodeStart";
import { TracePanel } from "./Trace";
import { ThreadList } from "./ThreadList";
import { ThreadView } from "./ThreadView";
import { ShapeShiftPreview, saveEveryday } from "./shapeshift";
import { agentById, getHost, goHome, saveAgent, send, setReplyTo, setRoute, stop, syncRoutines, useChat, type Agent, type Route } from "../bridge";

const baseName = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;

const DOCK_SPRING = { type: "spring", stiffness: 380, damping: 38, mass: 0.9 } as const;
// The everyday strip grows out of the dock's top edge: a touch stiffer than
// the dock so it reads as part of the composer, not a second surface.
const STRIP_SPRING = { type: "spring", stiffness: 900, damping: 55, mass: 0.5 } as const;
const TRIM_FADE = {
  initial: { opacity: 0, y: 10, filter: "blur(4px)" },
  animate: { opacity: 1, y: 0, filter: "blur(0px)", transitionEnd: SHED_BLUR },
  exit: { opacity: 0, filter: "blur(4px)" },
  transition: { duration: 0.16, ease: EASE_OUT },
} as const;
const THREAD_SWAP = {
  initial: { opacity: 0 },
  animate: { opacity: 1 },
  exit: { opacity: 0 },
  transition: { duration: 0.2, ease: EASE_OUT },
} as const;

const GREETINGS = [
  "What's up, {name}",
  "Hey there, {name}",
  "Back again, {name}?",
  "Oh hey, {name}",
  "Ready when you are, {name}",
  "Good to see you, {name}",
  "Welcome back, {name}",
  "Let's get into it, {name}",
];
/* No usable login name (root, a CI box): lines that don't need one. */
const NAMELESS = ["What's on your mind?", "Ready when you are", "Let's get into it", "Good to see you", "Ask me anything"];

export function ChatApp({ sideEl, user }: { sideEl: HTMLElement; user: string }) {
  const host = getHost();
  const { id, thread, threads, epoch, scope, replyTo, agentsMode, ghost, project, botPage, artifact, agents } = useChat();
  const [draft, setDraft] = useState("");
  const [homeRoute, setHomeRoute] = useState<Route | null>(null);
  // Everyday mode (shapeshift): the composer's ＋ menu turns it on, Esc turns
  // it off, Enter saves a card instead of sending a message. Session state,
  // like the draft: nothing about it outlives the window.
  const [everyday, setEveryday] = useState(false);
  // Voice mode, in a bot's thread: ends when you leave the thread.
  const [voice, setVoice] = useState(false);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  // Next wakes are absolute times computed in the core with an offset the UI
  // sent at save time; re-send this machine's offset once on mount so a DST
  // change (or a save from an older build) cannot leave them an hour out.
  useEffect(() => {
    void syncRoutines();
  }, []);
  const inThread = !!thread;
  const busy = !!thread && threads.some((t) => t.id === thread.id && t.busy);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  // Pair works in the project, so it starts on a coding CLI when there is one.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const firstCli = useMemo(() => host.routes().flatMap((g) => g.items).find((i) => i.route?.kind === "cli" && !i.disabled)?.route ?? null, [epoch]);
  const route = thread ? thread.route : (homeRoute ?? (scope ? (firstCli ?? host.defaultRoute()) : host.defaultRoute()));
  const replying = replyTo && thread ? thread.messages.find((m) => m.id === replyTo) : undefined;
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const anyRoute = useMemo(() => host.routes().some((g) => g.items.some((i) => i.route && !i.disabled)), [epoch]);
  // The scan runs every coding CLI on the machine to read its version and
  // sign-in state, after the app's own loader is gone. Until it answers the
  // route list is empty — which is not the same as having nothing — so the face
  // stays the normal home and only offers to connect once the scan has said no.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const scanned = useMemo(() => host.scanned(), [epoch]);
  // A ghost chat has no list to show, so the sidebar stays out of the way for as
  // long as it is open — and from the moment the mode is switched on, before the
  // thread exists: Whirl hides the shell as incognito opens, not on the first
  // message. Declared here, on every change, rather than fired once when the
  // thread is created.
  useEffect(() => {
    host.ghostSide?.(ghost || thread?.life === "ghost");
  }, [ghost, thread?.id, thread?.life]);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const clis = useMemo(
    () =>
      host
        .routes()
        .flatMap((g) => g.items)
        .filter((i) => i.route?.kind === "cli" && i.route.provider !== route?.provider)
        .map((i) => ({ id: i.route!.provider, name: i.label })),
    [epoch, route?.provider],
  );

  // A thread's draft survives hopping away and back, like Whirl's.
  const drafts = useRef(new Map<string, string>());
  const prevId = useRef<string | null>(id);
  useEffect(() => {
    drafts.current.set(prevId.current ?? "", draft);
    setDraft(drafts.current.get(id ?? "") ?? "");
    prevId.current = id;
    requestAnimationFrame(() => composerRef.current?.focus());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  const submit = async (text: string, files: { name: string; mime: string; data: string }[], plan = false) => {
    if (!route) throw new Error("no route");
    if (!host.usable(route)) {
      host.toast(`${host.providerName(route)} isn't available. Pick another model.`, "err");
      throw new Error("unusable");
    }
    try {
      await send(text, files, route, plan);
    } catch (e) {
      host.toast(String(e), "err");
      throw e;
    }
  };

  const agent = agentById(thread?.agent);
  // The bot this screen is about, whose panel sits on the right: the thread's,
  // or the page's. Its page reads the context of its latest thread.
  const sideAgent = agentsMode && !scope && !artifact ? (agent ?? (botPage ? agents.find((x) => x.id === botPage) ?? null : null)) : null;
  useEffect(() => setVoice(false), [thread?.id]);
  const sideThread = thread ?? (sideAgent ? threads.find((t) => t.agent === sideAgent.id) ?? null : null);
  const group = (thread?.members?.length ?? 0) > 0;
  // Who a group message can be aimed at with @Name: the roster, resolved. The
  // core already honours the name (named_in queues only the members named),
  // so the composer's job is only to make it easy to type.
  const roster = useMemo(
    () => (thread?.members ?? []).map((id) => agents.find((a) => a.id === id)).filter((a): a is Agent => !!a),
    [thread?.members, agents],
  );
  const changeRoute = async (r: Route) => {
    if (agent) {
      // An agent's thread answers with the agent's model: change the agent.
      await saveAgent({ ...agent, route: r }).catch((e) => host.toast(String(e), "err"));
      host.toast(`${agent.name} now runs on ${host.routeName(r)}`, "ok");
      return;
    }
    if (thread) {
      await setRoute(thread.id, r).catch((e) => host.toast(String(e), "err"));
      host.toast(`Next replies come from ${host.routeName(r)}`, "ok");
    } else setHomeRoute(r);
  };

  const newChat = () => {
    goHome();
    setHomeRoute(null);
  };

  const fill = (prompt: string) => {
    setDraft(prompt);
    requestAnimationFrame(() => {
      const el = composerRef.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(prompt.length, prompt.length);
    });
  };

  return (
    <>
      {createPortal(agentsMode && !scope ? <AgentsSide /> : <ThreadList onNewChat={newChat} />, sideEl)}
      <AgentEditors />
      <TracePanel />
      <main className="whirl-pane raised relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-surface md:rounded-lg md:border md:border-border">
        {/* Whirl's perch. The toggle holds the corner on the face a chat starts
            from AND all through a ghost chat — a ghost never routes, so nothing
            else claims the corner, and it is how you leave one. */}
        {(!inThread || thread?.life === "ghost") && (
          <div className="pointer-events-none absolute right-3 top-3 z-10">
            <div className="pointer-events-auto">
              <GhostToggle thread={thread ?? undefined} />
            </div>
          </div>
        )}
        <div className="relative flex min-h-0 min-w-0 flex-1">
          <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
            <div className="min-h-0 flex-1">
              <AnimatePresence mode="popLayout" initial={false}>
                {inThread && !artifact && (
                  <motion.div key={thread.id} {...THREAD_SWAP} className="relative h-full min-h-0">
                    <ThreadView thread={thread} header={<AgentThreadHeader thread={thread} />} />
                    {scope && <CodeSide scope={scope} />}
                  </motion.div>
                )}
                {!inThread && (agentsMode || scope) && !artifact && (
                  <motion.div key={scope ? "code-start" : botPage ? `bot-${botPage}` : project ? `project-${project.id}` : "agents-home"} {...THREAD_SWAP} className="h-full min-h-0">
                    {scope ? <CodeStart /> : botPage ? <AgentPage /> : project ? <ProjectHome /> : <AgentsHome />}
                  </motion.div>
                )}
                {/* An artifact is a web view you step into: it takes the pane,
                    the thread keeps the dock — you can tick the card or read
                    the project and keep talking, and "All bots" drops you back
                    exactly where the thread was. */}
                {artifact && (
                  <motion.div key={`artifact-${artifact.kind}-${artifact.id}`} {...THREAD_SWAP} className="h-full min-h-0">
                    <ArtifactView />
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
            {inThread && agent && voice && (
              <VoiceMode agent={agent} thread={thread} onSend={(t) => submit(t, [])} onClose={() => setVoice(false)} />
            )}
            {/* The landing is a whole screen: the home behind it draws its own
                composer under the hero, so the face's dock steps aside for it.
                A thread open, or the plain chat, keeps the dock. */}
            {!inThread && (agentsMode || scope) ? null : !anyRoute && scanned && !inThread ? (
              <NoModels />
            ) : (
              <div
                className={cn(
                  "pointer-events-none absolute inset-x-0 bottom-0 z-10 flex flex-col justify-end px-3 md:px-6",
                  !inThread && !scope && "top-0 justify-center pb-16",
                )}
              >
                <div className="mx-auto w-full max-w-2xl">
                  <AnimatePresence mode="popLayout" initial={false}>
                    {!inThread && !scope && (
                      <motion.div key="greeting" {...TRIM_FADE}>
                        <Greeting user={user} project={scope} />
                      </motion.div>
                    )}
                  </AnimatePresence>
                  <motion.div
                    layout="position"
                    transition={DOCK_SPRING}
                    transformTemplate={pinRasterPath}
                    className={cn("pointer-events-auto", (inThread || scope) && "pb-4")}
                  >
                    {/* The shapeshift strip: the same composer, one mode up.
                        It grows out of the dock's top edge and folds back into
                        it, so everyday entry never moves the composer. */}
                    <AnimatePresence initial={false}>
                      {everyday && (
                        <motion.div
                          key="everyday"
                          initial={{ height: 0, opacity: 0 }}
                          animate={{ height: "auto", opacity: 1 }}
                          exit={{ height: 0, opacity: 0 }}
                          transition={{ height: STRIP_SPRING, opacity: { duration: 0.15, ease: EASE_OUT } }}
                          className="overflow-hidden"
                        >
                          <ShapeShiftPreview
                            text={draft}
                            onExit={() => setEveryday(false)}
                            onSample={(t) => {
                              setDraft(t);
                              requestAnimationFrame(() => {
                                const el = composerRef.current;
                                if (!el) return;
                                el.focus();
                                el.setSelectionRange(t.length, t.length);
                              });
                            }}
                          />
                        </motion.div>
                      )}
                    </AnimatePresence>
                    {replying && <ReplyChip to={replying} onCancel={() => setReplyTo(null)} />}
                    {scope ? (
                      <CodeComposer
                        value={draft}
                        onValueChange={setDraft}
                        route={route}
                        onRouteChange={(r) => void changeRoute(r)}
                        onSubmit={submit}
                        onStop={thread ? () => void stop(thread.id) : undefined}
                        generating={busy}
                        inThread={inThread}
                        scope={scope}
                        planned={!!thread?.messages.findLast((m) => m.role === "user")?.plan}
                        threadId={thread?.id ?? null}
                        textareaRef={composerRef}
                      />
                    ) : (
                    <Composer
                      value={draft}
                      onValueChange={setDraft}
                      route={agent ? agent.route : route}
                      onRouteChange={(r) => void changeRoute(r)}
                      hideRoute={group}
                      onSubmit={submit}
                      onStop={thread ? () => void stop(thread.id) : undefined}
                      generating={busy}
                      floating={inThread}
                      everyday={everyday}
                      onEverydayChange={setEveryday}
                      onEverydaySubmit={(t) => {
                        saveEveryday(t);
                        setDraft("");
                      }}
                      placeholder={
                        thread?.life === "ghost"
                          ? "Ask anything — off the record"
                          : agent
                          ? `Message ${agent.name}`
                          : group
                            ? `Message ${thread?.title ?? "the group"} (@Name to ask one agent)`
                            : scope
                          ? `Ask ${host.providerName(route) || "a CLI"} to change ${baseName(scope)}`
                          : thread
                            ? `Message ${host.providerName(thread.route)}`
                            : "Ask anything"
                      }
                      clis={clis}
                      textareaRef={composerRef}
                      onVoice={inThread && agent ? () => setVoice(true) : undefined}
                      roster={group ? roster : undefined}
                    />
                    )}
                  </motion.div>
                  {scope && !inThread && <PairNote route={route} folder={baseName(scope)} />}
                  <AnimatePresence mode="popLayout" initial={false}>
                    {!inThread && !scope && (
                      <motion.div key="suggestions" {...TRIM_FADE} className="pointer-events-auto">
                        <Suggestions onPick={fill} />
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              </div>
            )}
          </div>
          {sideAgent && <AgentSide agent={sideAgent} thread={thread} threadId={sideThread?.id ?? null} />}
        </div>
      </main>
    </>
  );
}

/* Whirl's HomeGreeting: a 32px mark beside one line, rising in. The mark
   is Backspace's own. */
function Greeting({ user, project }: { user: string; project: string | null }) {
  const [pick] = useState(() => {
    const pool = user && user !== "there" ? GREETINGS : NAMELESS;
    return pool[Math.floor(Math.random() * pool.length)];
  });
  const line = project ? `What should we change in ${baseName(project)}?` : pick;
  return (
    <div className="mb-7 h-10">
      <motion.div
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.28, ease: EASE_OUT }}
        className="flex h-full min-w-0 items-center justify-center gap-3"
      >
        <span className="flex size-8 shrink-0 items-center justify-center rounded-[9px] bg-foreground text-surface">
          <svg viewBox="0 0 16 16" width="19" height="19" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
            <path d="M5.2 3.2h7.6a1 1 0 0 1 1 1v7.6a1 1 0 0 1-1 1H5.2L1.8 8z" />
            <path d="m7.6 6 4 4M11.6 6l-4 4" />
          </svg>
        </span>
        <h1 className="min-w-0 truncate text-[22px]/8 font-medium tracking-tight md:text-[28px]/9">
          {line.replaceAll("{name}", user)}
        </h1>
      </motion.div>
    </div>
  );
}

/* Pair: say where the CLI works and what it may do. */
function PairNote({ route, folder }: { route: Route | null; folder: string }) {
  const cli = route?.kind === "cli";
  return (
    <div className="pointer-events-auto mt-2 px-3 text-center text-[12px]/4 text-muted-foreground">
      {cli
        ? `Runs in ${folder} and may edit files and run commands there. Review what changed with git.`
        : "Only a coding CLI can work in the project; this model can only talk about it."}
    </div>
  );
}

/* Whirl's two suggestion capsules, from its offline pool. Dismissing one
   draws another. */
function Suggestions({ onPick }: { onPick: (p: string) => void }) {
  const [slots, setSlots] = useState<SuggestionSlot[]>(() =>
    pickFallbacks(2).map((s, i) => ({ ...s, id: `s${i}-${Date.now()}`, loading: false })),
  );
  return (
    <div className="mt-3">
      <SuggestionCards
        suggestions={slots}
        onPick={onPick}
        onDismiss={(id) =>
          setSlots((cur) => {
            const [next] = pickFallbacks(1, cur.map((s) => s.prompt));
            return cur.map((s) => (s.id === id && next ? { ...next, id: `s-${Date.now()}`, loading: false } : s));
          })
        }
      />
    </div>
  );
}

/* Nothing to talk to yet: say what can be connected, in Whirl's well. */
function NoModels() {
  const host = getHost();
  const btn =
    "flex cursor-pointer items-center gap-2 rounded-full px-4 py-2 text-[14px]/5 font-medium transition-[background-color,scale] duration-150 active:scale-[0.96]";
  return (
    <div className="absolute inset-0 flex items-center justify-center px-6 pb-16">
      <div className="flex max-w-md flex-col items-center gap-4 text-center">
        <span className="flex size-12 items-center justify-center rounded-2xl bg-well text-muted-foreground shadow-[inset_0_0_0_1px_var(--well-outline),inset_0_1px_0_0_var(--well-highlight)]">
          <IconPlugConnected size={24} />
        </span>
        <h1 className="text-[22px]/8 font-medium tracking-tight">Connect a model to start chatting</h1>
        <p className="text-[14px]/6 text-muted-foreground">
          Use a coding CLI you already pay for (Claude, Codex, Cursor, Grok, OpenCode), models on this machine through
          Ollama, any OpenAI-compatible router, or Backspace Cloud with nothing to install.
        </p>
        <div className="flex flex-wrap justify-center gap-2">
          <button type="button" onClick={host.runSetup} className={`${btn} bg-primary text-primary-foreground hover:bg-(--primary-hover)`}>
            <IconBolt size={16} />
            Run setup
          </button>
          <button type="button" onClick={() => host.openSettings("plan")} className={`${btn} bg-well hover:bg-accent`}>
            <IconCloud size={16} />
            Start free with Cloud
          </button>
        </div>
      </div>
    </div>
  );
}
