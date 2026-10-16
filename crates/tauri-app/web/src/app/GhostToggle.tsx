// The ghost toggle: a chat that is never written to disk.
//
// This is not a UI mood. `Life::Ghost` in the core keeps the thread out of the
// list and off the disk, and turning a saved chat into a ghost deletes the file
// it already had — otherwise it would come back on the next launch and the
// promise would be a lie. Turning the toggle off saves it again from the next
// message on. The limit is honest and worth saying out loud in the tooltip: a
// reply that went to a provider has already left this machine.

import { useState } from "react";
import { Ghost } from "lucide-react";

import { getHost, setLife, type Thread } from "../bridge";

export function GhostToggle({ thread }: { thread: Thread }) {
  const ghost = thread.life === "ghost";
  const [busy, setBusy] = useState(false);

  const flip = async () => {
    if (busy) return;
    setBusy(true);
    try {
      await setLife(thread.id, ghost ? "keep" : "ghost");
      getHost().toast(ghost ? "Kept: this chat is saved again" : "Ghost: this chat is not saved");
    } catch {
      getHost().toast("Could not change how long this chat lives", "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <button
      type="button"
      className="hc-plan hc-ghost"
      aria-pressed={ghost}
      disabled={busy}
      title={
        ghost
          ? "Ghost chat: never written to disk, gone when you close it. Click to keep it. (A reply that already left this machine has left it.)"
          : "Make this a ghost chat: never written to disk, gone when you close it"
      }
      onClick={() => void flip()}
    >
      <Ghost size={14} />
      Ghost
    </button>
  );
}
