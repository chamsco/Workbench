# AgentQuestion

The agent is blocked on a decision only the person can make. It is the same card family as `ApprovalCard`, so the two are never mistaken for chat text.

- Card: `panel`, `radius-panel`, `line-strong` ring, full width of the thread column with equal side gaps; it animates in over `dur-panel`.
- Top: needs-you `StatusLabel` and the asking agent as a `Tag`. Title: the question in `body-strong`, one sentence.
- Options: 2 to 6 rows (`ThreadRow` style), label in `body-strong`, one line of consequence in `meta` `fg-3`, number keys 1 to 6 as `Kbd`. The first is selected by default only when the agent has a recommendation; say so in its description ("Recommended").
- Foot: "Or type an answer" when free text is allowed, and a primary Answer button (Ctrl ↵). After answering the card collapses to one `meta` line with the chosen option.
- Props for generation: `question`, `options[{id,label,description?}]`, `multiple?`, `allowText?`, `action`.
