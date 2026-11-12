# ToolCall

One tool call an agent made: a 32px row that opens to its input and output.

- Row: `size-control-lg` high, `inset` fill, `radius-control`, 8px padding. Left to right: status glyph (check in `fg`, spinner, or x), tool name in `mono` 500, a one-line summary in `fg-3` that truncates, and the duration or live timer. Nothing wraps.
- Failed: `bad-soft` band and the word "Failed" in `bad`; it opens by default and offers Retry.
- Open: sections below a `line` hairline; code in a `card` well, `mono`, capped at 220px with a scroll, Copy turns into "Copied" for 1.5s.
- Several calls group under a header line "Used 3 tools · 4.1s" (live: "Using tools…" plus timer).
- Pattern adapted from the MIT-licensed affectUI Tool Call Card; see the GPUI section for the notice.
