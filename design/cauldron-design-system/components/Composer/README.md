# Composer

The one place a person writes to agents: a context row above, the box, and a usage and stage line below.

- Shell: `radius-shell` (22), `inset` fill, 6px padding, `shadow-box`. Body: `radius-panel` (16), `panel` fill, 1px `line-strong` inset ring; on focus the ring turns `accent`. No glow.
- Text is `body`; placeholder `fg-3` ("Ask the coordinator, @ an agent or a file, / for commands").
- Foot, left to right: attach, trust picker (Ask first), spacer, agent picker, effort picker, voice, send. Pickers are 28 high, `label` text in `fg-2`, chevron after.
- Context row: project, machine and worktree, branch; `meta` text with 14px icons. Below: usage left, stages right as plain text, the current stage in `accent`.
- Empty thread: the composer sits in the upper third. With messages: it docks to the bottom of the thread column.
