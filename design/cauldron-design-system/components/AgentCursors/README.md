# AgentCursors

Agents editing a shared document the way teammates do: a pointer with a name flag, edits as suggestions, comments as pins.

- Shell > Panel. Header row: document path in `fg-3`, an agent faces strip (24px figures, the followed agent ringed), "3 suggestions", total cost in mono, and a primary **Accept all** (only when more than one suggestion is pending).
- Page: real headings, paragraphs and lists, 14/24. Agent edits are suggestions: removed text struck through in `fg-3`, inserted text underlined with a 22% tint of that agent's colour.
- Pointer: a small arrow in the agent's colour with a flag below it on `raised`: name in the agent's colour, running cost in mono, and what it is doing in `fg-2` ("Checking the rest"). Flags clip at the right edge, they never move.
- Suggestion window (260px, `raised`, `shadow-menu`): agent dot, name, time; the agent's note; Reject (secondary) and Accept (primary). Opens on click or focus; Esc closes.
- Following: selecting a face follows that agent; other pointers fade to 40%.
- Comment pins: a dark 26px pin with the agent's mark that expands to a window with a resolve check and a reply field.
- Agent colours are the people-colours (`agent-N`); they are the only per-agent colour in the UI.
- The affectUI Agent Cursors is a paid Pro piece; this follows its public description only.
