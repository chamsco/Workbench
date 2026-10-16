// The ghost switch, on the home face only.
//
// A chat's life is decided before it exists, not after: `Life::Ghost` in the core
// never writes the thread to disk and never lists it, so there is nothing to
// switch back to — the only way out of a ghost is to delete it. That is why this
// sits on the face you start a chat from and disappears once one is open.
//
// The shape follows Whirl's incognito button (MIT, whirlchat/whirl): a 32px
// circle that wears the tools' translucent well when off and fills with ink when
// on, icon only. The two glyphs crossfade in place inside one fixed box; waiting
// out the exit would make the toggle feel laggy.
//
// The limit is honest and in the tooltip: a reply that went to a provider has
// already left this machine.

import { AnimatePresence, motion } from "motion/react";
import { IconGhost2, IconGhost2Filled } from "@tabler/icons-react";

import { pinRasterPath } from "@/lib/motion";
import { getHost, goHome, remove, setGhost, useChat, type Thread } from "../bridge";

const GHOST_SPRING = { type: "spring", stiffness: 520, damping: 28 } as const;

/** `thread` is set while a ghost chat is open. Whirl keeps the toggle in that
 *  corner for the whole incognito chat — the thread never routes, so nothing
 *  else claims the perch — and clicking it there is how you leave. A ghost was
 *  never written to disk, so leaving deletes it; that is confirmed first. */
export function GhostToggle({ thread }: { thread?: Thread }) {
  const pending = useChat().ghost;
  const inGhost = thread?.life === "ghost";
  const on = inGhost || pending;

  const click = async () => {
    if (inGhost && thread) {
      const ok = confirm("Leave ghost? This chat was never saved — leaving deletes it for good.");
      if (!ok) return;
      await remove(thread.id);
      goHome();
      getHost().toast("Ghost chat deleted");
      return;
    }
    setGhost(!pending);
    getHost().toast(pending ? "Kept: new chats are saved again" : "Ghost: the next chat will not be saved");
  };

  return (
    <motion.button
      type="button"
      className="ghost-toggle"
      aria-pressed={on}
      aria-label={on ? "Leave ghost mode" : "Start a ghost chat"}
      title={
        inGhost
          ? "Leave ghost — this chat was never saved, so leaving deletes it"
          : on
            ? "Ghost: the next chat is never written to disk"
            : "Start a ghost chat — never written to disk, gone when you close it"
      }
      whileTap={{ scale: 0.96 }}
      onClick={() => void click()}
    >
      <span className="ghost-ic">
        <AnimatePresence initial={false} mode="popLayout">
          <motion.span
            key={on ? "on" : "off"}
            className="ghost-glyph"
            initial={{ opacity: 0, scale: 0.6, rotate: -12 }}
            animate={{ opacity: 1, scale: 1, rotate: 0 }}
            exit={{ opacity: 0, scale: 0.6, rotate: 12 }}
            transition={GHOST_SPRING}
            transformTemplate={pinRasterPath}
          >
            {on ? <IconGhost2Filled size={18} /> : <IconGhost2 size={18} stroke={2} />}
          </motion.span>
        </AnimatePresence>
      </span>
    </motion.button>
  );
}
