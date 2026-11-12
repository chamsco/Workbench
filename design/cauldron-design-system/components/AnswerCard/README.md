# AnswerCard

An answer with its evidence: the question, the sources, the answer with numbered citations, follow-ups and actions. For research and Q&A turns.

- No frame of its own: it sits in the thread's panel. Question in `title`; sources as up to three `inset` cards (favicon tile, name in `fg-3`, title clamped to two lines) and a "+N more" tile.
- Answer: `reading` text, 66ch max. A citation is a 18px chip with a mono number (`radius-tag`, `line-strong` ring). Hover or focus opens the source's snippet in a `Menu`-style popover; Esc closes.
- Streaming: words arrive at about 38 per second behind an `accent` caret that blinks; the card is `aria-busy` until done. Stop replaces the other actions while streaming.
- Done: the caret stops, follow-ups appear as hairline rows with an arrow, actions are Copy (answer plus numbered source list), Regenerate, thumbs up and down. Thumbs down asks for a reason: Not accurate, Wrong source, Too long, Out of date.
- Props for generation: `question`, `sources[{title,name,href?,snippet?}]`, `answer` (markup-light: paragraphs, "- " bullets, bold, `[n]` citations), `streaming`, `followUps[]`.
- The affectUI AI Answer is a paid Pro piece; this follows its public description only.
