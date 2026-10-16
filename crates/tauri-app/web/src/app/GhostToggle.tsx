// The ghost toggle: a chat that is never written to disk.
//
// The shape follows Whirl's incognito button (MIT, whirlchat/whirl): a 32px
// circle that wears the toolbar pills' translucent well when off and fills with
// ink when on, so the mode reads at a glance. Icon only — the ghost says it all.
// The two glyphs crossfade in place inside one fixed box; waiting out the exit
// would make the toggle feel laggy.
//
// This is not a UI mood. `Life::Ghost` in the core keeps the thread out of the
// list and off the disk, and turning a saved chat into a ghost deletes the file
// it already had — otherwise it would come back on the next launch and the
// promise would be a lie. Turning it off saves it again from the next message.
// The limit is honest and in the tooltip: a reply that went to a provider has
// already left this machine.

import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { IconGhost2, IconGhost2Filled } from "@tabler/icons-react";

import { pinRasterPath } from "@/lib/motion";
import { getHost, setLife, type Thread } from "../bridge";

const GHOST_SPRING = { type: "spring", stiffness: 520, damping: 28 } as const;

export function GhostToggle({ thread }: { thread: Thread }) {
  const on = thread.life === "ghost";
  const [busy, setBusy] = useState(false);

  const flip = async () => {
    if (busy) return;
    setBusy(true);
    try {
      await setLife(thread.id, on ? "keep" : "ghost");
      getHost().toast(on ? "Kept: this chat is saved again" : "Ghost: this chat is not saved");
    } catch {
      getHost().toast("Could not change how long this chat lives", "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <motion.button
      type="button"
      className="ghost-toggle"
      aria-pressed={on}
      aria-label={on ? "Leave ghost mode" : "Go ghost"}
      title={on ? "Leave ghost — this chat is not saved" : "Ghost chat: never written to disk, gone when you close it"}
      disabled={busy}
      whileTap={{ scale: 0.96 }}
      onClick={() => void flip()}
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
