# AgentTabs

Tabs for one thread's agents: the coordinator pinned first, then one tab per 1:1 conversation. Never thread-to-thread.

- Tab: `size-control-lg` (32) high, top corners `radius-card`, sits on `frame` and joins the `card` below when selected.
- Selected: `card` fill, `fg`, `body-strong`. Others: `fg-2`, `hover` on hover.
- Leading 12px circle in the agent's `agent-N` colour (the flat avatar at small size). Close shows on hover and on the selected tab.
- Width: equal shrink from 210 to 72; past that, two 24px arrows appear beside the plus and the row scrolls.
