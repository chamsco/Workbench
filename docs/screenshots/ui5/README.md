# UI5: the New thread screen

Captured on Charles's Windows 11 PC, 11 Nov 2026, 100% scaling, client area 1240x702, dark theme, debug build, against the seeded sample data folder (made-up threads; the prefs there route new threads to a CLI that does not exist and list one made-up machine at an unreachable address). No model was contacted.

| File | State |
|---|---|
| `01-new-thread-sample.png` | The screen with no thread open: brand, Code tab joined to the box, the box, the cards |
| `02-focused-sample.png` | The editor focused: the body's line turns accent (70% over the line) |
| `03-design-note-sample.png` | Design selected (drawer closed): its placeholder, the join moves and curves on both sides, Start waits, the note under the box |
| `06-join-vs-wireframe.png` | The Code tab's right foot at 8x, drawer closed (card 970 wide): this build left, the rendered wireframe right |
| `04-machine-menu-sample.png` | The machine button's menu: this machine and the connected ones |
| `05-machine-unreachable-sample.png` | A connected machine that does not answer: Start waits and says why (the reason is Windows' own text, here in French) |

## What the screen does

- New thread (sidebar) or no thread open shows it; the tab reads New thread.
- Sizes are measured from the rendered wireframe (`.nt-wrap` at 1240x702): column 608 wide (an auto-sized grid track, not the CSS's 680), 72 from the card's top, brand 28 with 20 under it, tabs 34 and 35 when selected, box padding 4 and radius 10 (new token `radius::BOX`, R around the padding), body radius 6 and 3 in the joined corner, foot controls 30 high, cards 600 wide.
- Tab join: the selected tab has the box's strip colour, covers the box's top line and curves into it with an inverse corner of radius R; with Code selected the box's top-left is square. Tested: tab bottom equals box top plus 1, same left edge. GPUI draws a border over its children, so the corners sit beside the tab rather than inside it (shot 06 caught the seam).
- Start (Enter or the arrow) on this machine: `Chats::create_in`, then `attach` for each file, then `send`, then the thread opens. On a connected machine it sends the text to that machine's harness as a task (`/v1/task`, as the Tauri shell did) and says "Sent to {name} as {key}"; no local thread is made.
- + attaches files (native picker; the test feeds a path to the same list). Each file shows as a chip with a remove button.
- The effort label cycles and saves `prefs.cli_effort`, as in the thread composer.
- Cards come only from core's threads: the thread counts, and the projects with their thread counts. Pull requests wait for a GitHub connection in core and are left out. No threads: no cards.
- States, each said under the box: loading ("Opening your threads"), error ("Can't open your threads: ..."), no subscription ("Connect a subscription to start a thread."), connecting, and unreachable machine ("Can't reach {name}: ..."). Start does nothing in each.
- Who answers: prefs' default route, else Claude Code if installed, resolved when core opens. Tests set it, so no test can reach a real model.

## Off-scale values

Taken from the rendered wireframe (`.nt-wrap` at 1240x702), not the 4px scale: gaps and padding of 1, 2, 3, 5, 6, 7, 10 and 15; foot controls 30 high and the arrow 31 wide; box radius 10 (`radius::BOX`, R plus the 4 padding). One-pixel offsets (`mb(-1)`, the corner's `left(1)`) draw the tab join. The editor's `-10`/`-8` margins cancel the input's built-in padding.

## Left out

- The brand mark is Lucide's `flask-conical`, standing in for the wireframe's custom cauldron glyph (Lucide only).

- The "+" beside the machine adds machines in Settings (UI8); here it only names that.
- Design is a planned app (wireframe note): its tab shows a note and does not start a thread. A distinct Design thread view is in UI7.
- The card's dot texture (planned as one baked image, see UI1).

## Not checked

- Starting a real thread with a real model, and a real connected machine (the test runs a fake harness on localhost).
- Light theme, 125% and 150% scaling.
- Re-render when core or a connected machine changes: tests do not listen on core's channel (a wake from core's thread breaks the test scheduler) and refresh by hand.
