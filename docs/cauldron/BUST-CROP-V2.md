# Chat bust, crop v2 (Charles, 2026-11-12 13:42Z)

Charles liked the bust in the Highlights list ("this is exactly what I wanted") and asked for the head a little smaller so the shoulders show.
Sheet: `design/cauldron-avatars-and-thread.html`, section "Chat agent: a bust".

In `crates/chrome/src/avatar.rs` (sa3a1p) only these change:

| What | Was | Now |
|---|---|---|
| `BUST` (slot crop x, y, w, h) | `22, 9, 56, 56` | `18, 7, 64, 64` |
| `BUST_DRAW` (slot plus room up and right for marks) | `22, -18, 64, 83` | `18, -18, 72, 89` |
| bust `?` / `!` mark scales | `.2` / `.25` | `.228` / `.285` |
| bust z sizes and stroke | `9, 7.5, 6`, `3.2` | `10.3, 8.6, 6.8`, `3.65` |

The body path stays as it is (arc through y=100): the image's bottom edge at y=71 is what cuts it, so the shoulders end at the slot edge.
Outline weight follows `1.4 * BUST.3 / px` on its own. Marks stay the same size on screen (the crop grew 14%).
Check at 16px next to a step line: label x must still equal the step text x, and the slot stays a 16px square.

The thread's status line itself (not only the Highlights list) still has to draw this bust: Charles noticed it is missing there.
