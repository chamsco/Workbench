# Handoff: Backspace (ohMyHarness → Workbench), v0

**Session, 24 Oct 2026 — Plan, Execute, Ship, Commit, and the note chain.**

- **The stages were planned (docs/PLAN-PHASES-2026-10-17.md) and half built in
  core, with no face.** Now `Phase` has `Commit`, `Stage` (Plan, Execute, Ship,
  Commit) groups the phases for the app, and `phase.json` keeps spend per stage
  (work, and the decisions on top, from `router_cost_usd`). A strip under
  Home's composer (`ui/stages.js`) shows them, each stage's decision model
  (click to pick, or "Rules only", which is free) and what stops its gate.
- **Gates:** Plan is the spec and plan gates that already existed. Execute opens
  by rules once every ticket is finished (`stages::execute_gaps`). Ship is yours
  alone. Commit runs the Librarian. A stage's model is asked first and may only
  refuse (`PhaseState::veto`, recorded as a model gate); an unreachable model
  leaves the rules' answer standing and says so.
- **Notes chain up:** `leave_note` (in-process agents and, through the bridge,
  CLI workers) keeps a line on the agent (`AgentRecord.notes`). When a ticket's
  work is accepted its notes move to its parent, deduped, so the main agent holds
  the compiled list. At Commit the Librarian's model (or rules, with none) keeps
  or drops each one with a reason, keepers go through `memory::propose`, and one
  `drain` makes one memory commit with a receipt. The model can only drop what
  it names; `drain` still refuses secrets and what you said not to remember.
- **Not done:** the strip is on the ADE home only, not the IDE; Plan's and
  Execute's model reviews exist for Execute, Ship and Commit but Plan's model is
  not asked yet (its gates are the rules already there); no bake-off
  (`tools/decide-eval`) yet, so no model is recommended per stage.

**Session, 23 Oct 2026 — Orchestrator, executor, and harnesses that talk.**

- **Who runs a project is two choices now.** The ADE composer's model chip is
  the orchestrator (`prefs.planner`: plans, reviews, talks to you); a new
  executor chip beside it says who builds each ticket: one harness
  (`prefs.worker`) or the router, with the models it may not use
  (`prefs.worker_exclude` → `router.exclude_sub`, honoured by routing and by
  escalation). `Runs` in `ui/picker.js` draws both; Settings has them too.
- **Per-app settings.** Settings gets an Apps group: Chat (what was "Chat
  behavior") and Workbench (orchestrator, executor, harness tiles).
- **Harness tiles.** A canvas can hold any harness in its own chat thread
  (pane kind `harness`, with `thread`, `label`, `coordinator` on `PaneSpec`).
  The open tab's tiles are written to `<data>/relay/tiles.json`
  (`crates/core/src/relay.rs`). `@codex …` typed in a tile goes to the `codex`
  tile with the sender's last answer as context, and the answer is posted back
  without a turn; a harness can do the same itself through `backspace mcp`'s
  `list_tiles` / `message_tile` (or `backspace tile send` for CLIs without
  MCP), and then the answer comes back as its next turn. Right-click a tile's
  header to make it the tab's coordinator: its brief says to split the work
  and hand it out.
- **What cannot work:** nothing reaches a CLI mid-turn. Each turn is its own
  headless process, so a letter for a busy tile waits in `Chats::inbox` until
  the turn ends (`Fleet::schedule_relay` pumps every 700 ms). A chain of
  hand-offs stops at 6 (`relay::MAX_HOPS`). Terminal canvases are untouched:
  typing into a PTY running an interactive TUI has no way to know when it is
  ready or to read an answer back.
- **Fixed on the way:** a reply that failed at once could remove itself from
  `Chats::running` before it was added, leaving the thread busy for good; the
  handle is now added under the lock.

**Ran:** `cargo test -p backspace-core` (new tests for the relay, the queue,
the hop cap, `@tile` hand-off, a resumed session reading what it missed, and
router exclusions), `cargo check -p backspace-tauri`. The real `ui/` in
`tools/uiharness` with `tools/verify/harness-tiles.js` (two tiles, the
coordinator badge, the `@` chips, a waiting count, the header menu), the ADE
executor menu and Settings → Workbench. **Not run:** real CLIs talking to each
other — no signed-in CLI here.

**Session, 22 Oct 2026 — Our own sea.**

- **The sharks are ours now.** stackedlol/ascii-sharks has no licence, so its
  hammerhead is gone and `crates/tauri-app/ui/ocean.js` draws the Sharks and
  Koi backdrops from scratch: each animal is a spine of points following its
  head (so turns travel down the body), its outline is laid along that spine
  and inked into a character grid. Sharks hunt three schools of small fish
  that scatter from them and leave a wake; koi meander and avoid each other.
  `Art.apply` starts it for a shark or koi backdrop and stops it otherwise.
  The ascii-sharks entry left THIRD_PARTY_NOTICES.md.

**Ran:** the real `ui/` in headless Chromium: Sharks white (dark), Sharks
black (light) and Koi animate at ~55-60 fps with no page errors; with
reduced motion the frame is still; switching to Plain removes the canvas.

**Session, 22 Oct 2026 — the first terminal is Brainpowr Code.**

- **A project's first terminal canvas opens Brainpowr Code** instead of a bare
  shell (`brainpowrPane()` in `extensions/code/code.js`, styles in the new
  `extensions/code/code.css`). It is drawn like a TUI: sessions and running
  tasks in a plum sidebar, a pink ASCII brain and a pixel wordmark in the
  middle, the slash commands, a prompt box and the model under it.
- **It is HTML, not a process in the PTY.** The terminal canvas is not an
  emulator (`TERM=dumb`), so a real full-screen TUI cannot draw there yet.
- **Sessions are the project's chat threads** (`chat_new` with
  `scope.project`), so they are the same threads the ADE lists. `/model` is the
  chat route picker; `/tasks` opens the tickets drawer; `/shell` turns the
  canvas into a plain shell and typing `brainpowr` there turns it back.
  "First" is the first terminal pane across the project's tabs, in order;
  every other terminal stays a shell.

**Ran:** the real `ui/` in `tools/uiharness` with mocked threads, tickets and a
two-terminal tab at 1440 and 1053: home screen, opening a session (markdown,
streaming spinner), the `/` list with arrows, `/help`, `/shell` and back, and
esc folding the sidebar; no console errors. **Not run:** a real CLI reply —
no Tauri here.

**Session, 22 Oct 2026 — Code owns its engine.**

- **The workbench moved into the app.** Everything Code draws is now
  `crates/tauri-app/ui/extensions/code/code.js`: the per-project tab layout
  and its persistence, the tab strip (drag, rename, the new-tab presets), the
  canvas tree and grid, every pane (session, files, terminal, browser,
  diagram, docs, options, trace), the wall, the tickets drawer and the view
  menu. `app.js` went from 2,528 lines to about 1,085 and keeps the runtime —
  agents, approvals, projects, machines, prefs, settings, updates, the
  sidebar's rows.
- **The project title is the app's head.** `Frame.register` takes `head()` and
  `windowTitle()`; the shell asks the frame instead of building Code's folder
  button itself. `codeTitle()` is gone from `shell.js`. A `head()` that returns
  `null` (Chat) leaves the head empty.
- **The seams are explicit.** The ADE sidebar, Home and Chat call the code
  app's public object (`Code.tab()`, `Code.openView()`, `Code.renderGrid()`,
  `Code.setDrawer()`); the frame asks `Code.layout()` on a layout pass and
  `Code.afterData()` after a refresh. No app function is global any more.
- **One formatting note:** the engine's body sits at column 0 inside its IIFE
  — a 1,500-line re-indent would have buried the move in the diff. It can be
  formatted on its own later.

**Ran:** the real `ui/` in the T3 preview against the `__TAURI__` stub. Booted
to Chat, switched to Code, forced a project and a two-pane tab, then exercised
the tab strip (switch, unnamed layout icon), the New-tab presets (a 2×2 tab
appearing and switching), add-canvas (Browser added as a third pane), the
tickets drawer, the wall view and Chat ↔ Code switching; no console errors
beyond the stale one from before the reload, and the head/title read the
project (`Workbench`, `Workbench · Backspace`). **Not run:** Rust — no
toolchain here; the macOS workflow compiles and publishes `mac-latest`.

**Session, 22 Oct 2026 — the window becomes a frame.**

- **The window hosts apps.** `crates/tauri-app/ui/frame.js` reads
  `extensions/index.json`, gives each app a section in `#body`, loads its style
  before `home.css` (the cascade did not move) and then its script, and knows
  the app only by its manifest. Chat moved to `extensions/chat/`
  (`chat.js`, `chat.css`, `extension.json`); Code's edge is
  `extensions/code/code.js` (its engine is still `app.js`).
- **The title bar names the app.** A chip after the switcher holds the name, an
  On switch and New. On is `prefs.uses` (Settings → General keeps the same
  toggles); New is the app's action — a chat, a tab. Off apps stay in the
  switcher, dimmed; clicking one turns it on. The last app cannot be turned
  off, and going to an off app turns it on.
- **The shell stopped hardcoding the two.** `setMode` takes its ids from
  `Frame.ids()`/`Frame.onIds()`, `Shell.elFor` asks the app which section is its
  face (Code's ADE pair mode still uses the chat face), window classes come
  from `Frame.syncClasses`, and the first paint of a visible section is
  `Frame.paint` instead of a `Chat.render()` branch. `Workbench` no longer
  repeats over the project in Code's title.
- **Narrow bars.** The chip put the 1053px bar over budget; below 1180 the
  search button is its icon, below 1060 the chip drops its On/New words, below
  900 its name. `docs/extensions.md` has the manifest and the hooks.

**Ran:** the real `ui/` in the T3 preview against the `__TAURI__` stub at 1053
and 1366 in both themes — On/off (from the chip, a dimmed switcher icon, and
Settings), New (a tab's layout popover, a chat), Memory/Apps hiding the chip,
and the bar measured for overflow. **Not run:** Rust — no toolchain here, so
the macOS workflow is the compile.

**Session, 22 Oct 2026 — the update installs itself, and Chat sits on paper.**

- **Updating is no longer manual.** The pill still finds a newer commit on
  `mac-latest`; clicking it now calls `install_update` (new
  `crates/tauri-app/src/update_install.rs`). It downloads the release's own
  `Backspace-mac.app.zip` — the artefact a person gets — streams it into
  `$TMPDIR/backspace-update-<pid>`, unpacks it with `ditto -x -k`, checks that
  `CFBundleIdentifier` is `computer.backspace.app`, then writes `swap.sh` and
  quits into it. The script waits for this pid, moves the old bundle aside,
  copies the new one in, puts the old one back if that copy fails, strips
  quarantine, opens the app and deletes the temp folder. Every failure after
  the quit ends with a bundle reopened: there is no window left to show an
  error in.
- **It refuses where it cannot write.** The bundle's parent is probed with a
  real file before 25 MB is spent, so a build running from a mounted dmg says
  so instead of unpacking into nowhere.
- **Progress.** An `update:progress` event (`{stage, pct}`) drives the pill and
  Settings → Updates: Preparing, Downloading 42%, Unpacking, Installing,
  Restarting. The button reads "Install & Restart". There is no done state: on
  success the window is gone and a new one is what comes back.
- **The one-time install stands.** Only builds from `08b0371` on can replace
  themselves; an installed 0.1.0 still opens the release page. One manual
  install (v0.1.1 still carries the bridge build), then the pill does the rest.
- **Chat → Agents is paper.** The chat zone floated translucent over the
  backdrop painting, which washed the prose out at any real wallpaper. It now
  uses the sheet Apps and Memory use (`--paper`, 12px radius, a hairline, the
  composer as the only card), and the chat face's own theme
  (`crates/tauri-app/web/src/styles.css`, rebuilt into `ui/chat-web`) takes the
  app's paper/card/line/text tokens, so the pane, bubbles, menus, dialogs and
  code chrome match in light and dark. `npm run build` reproduces the committed
  bundle byte for byte — check that before hand-editing anything under
  `ui/chat-web`.

**Ran:** the macOS workflow on both branches (`39756652257` compiled the
installer, `39760466628` built and published `c30c3ff` as `mac-latest`),
`npm run build`, and the real `ui/` in a headless page against a stubbed
`__TAURI__`, measuring computed styles in both themes. **Not run:** nothing
local in Rust — no toolchain here, so the workflow is the only compile.

**Session, 21 Oct 2026 — the update check, and the branch caught up.**
Branch `nightly/feature/delegation`, now merged with `origin/main` (the Chat
face redesign, PR #6). The owner asked why updating from inside the app never
worked, and whether pushing a release was the fix.

- **Why it never worked.** `update.rs` asked
  `/repos/chamsco/ohMyHarness/releases/latest`. Two faults, either fatal: the
  repo is the pre-rename name, and `/releases/latest` ignores pre-releases —
  and the only release is the rolling `mac-latest` pre-release. The endpoint
  404'd, which the code read as "nothing newer", so every published build has
  said "Up to date v0.1.0". The tag is not a version either, so the
  dotted-version comparison could not have matched even with a 200.
- **What it does now.** The check asks
  `/repos/chamsco/Workbench/releases/tags/mac-latest` and compares *commits*.
  The workflow writes the commit it is building into
  `crates/core/src/build_stamp.rs` before building (`GITHUB_SHA`), the release
  carries its commit in `target_commitish`, and a different commit means main
  moved and a newer build is out. A hand build has no stamp and is never offered
  an update. `UpdateInfo` gained `build`; both shells' copy now says "build", not
  "version". The binary keeps a `#[used]` copy of the commit as well: a release
  build folds the seven bytes the check actually uses into machine instructions,
  so without it no one -- the workflow included -- can read a shipped app's
  commit back out of it.
- **Still manual.** "Download & Update" opens the release page; nothing replaces
  the app in place. There is no `tauri-plugin-updater`, and an unsigned,
  un-notarized bundle is a poor candidate for one. It is in the README's
  "What it doesn't do".
- **The one-time catch, done.** Every app published before this fix has the old
  check compiled into it, so none of them can see the fix: the old check reads
  `/releases/latest` and skips pre-releases, which is all this project
  published. `v0.1.1` is the bridge — one non-pre-release carrying the same
  build as `mac-latest` (`ba5c695`) — so an already-installed app is offered it.
  It stays the repository's "latest" release; rolling builds keep going to the
  `mac-latest` pre-release, which is what every fixed build asks for.
- **The merge.** `Agents.tsx`, `bridge.ts` and the committed bundle conflicted
  with main's new sidebar. Both sides had only added fields to `bridge.ts`; main
  had moved "New bot" out of the sidebar to the home page's KPI line, so the
  Delegate row now sits under "New chat". The bundle was regenerated with
  `npm run build`, not hand-merged. main changed no Rust files at all.

**Ran:** `npx tsc --noEmit`; `npm run build`; the update logic re-run against
the live release payload (`target_commitish` parses, the same commit is not
newer, a different one is). **Not run:** anything `cargo` — this machine has no
Rust toolchain, so the Rust above was written but never compiled. `test.yml`
runs `cargo test` on a pull request, and `macos.yml` has a new step that fails
the build if the app does not carry its commit; those are the first real checks.

**Session, 18 Oct 2026 — the phase audit, skills, and the Delegate sheet.**
Branch `nightly/feature/delegation`. The owner sent the Delegate mockup
(unassigned tasks on the left, bots with model, `$/min`, `Busy until` and a
queue on the right, drag to assign, an Auto switch) and asked three things:
how the decision model is wired per phase and where it breaks, whether skills
can be added/deleted, and a nightly branch where routing is visible and
overridable. Audit, decisions and limits: `docs/DELEGATION-2026-10-18.md`.

- **The audit's headline** (verified against the code, not the plan): routing
  is per *role*, never per phase; `route_for(role, job, task, context)` has no
  phase field (`router.rs:187`). `Phase::Test`/`Ship`/`Done` are unreachable;
  gates 3–5 have no code. Plan mode's spec prompt and read-only permission are
  set *after* the non-CLI early return (`chat.rs:1532` vs `1548-1557`), so with
  an API/local route no `SPEC.md` is written and gate 1 never runs. A failed
  check and a human rejection do **not** escalate, although
  `default-config.toml` says they do (`harness.rs:2438-2496`). `pin_main`
  discards Jev's model choice (`router.rs:287-299`). `may_plan()` is dead code.
- **Skills.** Built-ins are compiled in (`include_str!`, `skills.rs:11-34`) and
  can only be shadowed, never deleted or disabled. A user folder adds one; a
  `<project>/.backspace/skills` folder silently overrides a built-in for every
  role, and agents can write there. Details and the minimal safe change (a
  `disabled_skills` list plus add/delete/enable commands) are in the doc; none
  of it is built.
- **The Delegate sheet.** `crates/core/src/delegate.rs`: a `Board` of tasks in
  `<data>/delegation.json`, `auto_assign` (shortest queue, then cheaper minute,
  then older bot, with the reason recorded), and `snapshot` (each bot's queue
  priced at `(in·4k + out·1.2k)/1e6` a minute; a CLI is free). `from_tickets`
  imports the open project's unsettled tickets once each. Commands in
  `main.rs` (`delegate_state/add/update/delete/assign/auto/import`).
  `web/src/app/Delegate.tsx` draws the mockup: drag a card to a bot or back,
  chips with the assignment's note on hover, a select for keyboard/touch, Auto,
  and "From plan". Opened from Chat → Agents → Delegate.
- **Deliberately not done:** nothing dispatches (assigning does not spawn a
  worker); the price is an estimate; Auto is a two-line rule, not Jev. Wiring
  it to the router needs the phase field first.

**Ran:** `cargo test -p backspace-core` (the crate's suites, including 7 new
`delegate` unit tests); `cargo check -p backspace-tauri` with
`TAURI_CONFIG='{"app":{"macOSPrivateApi":false}}'` (the plain `cargo check`
trips tauri-build's private-API allowlist; the real build is through
`tauri build`); `tsc --noEmit`; a production `vite build` into
`ui/chat-web/`; and `tools/verify/delegate.js` in the uiharness — **22 checks,
no page errors**, screenshot `docs/assets/delegate.png`. Note for the next
session: on this machine esbuild hangs when its cwd is inside the workspace,
so the web bundle was built from a `/tmp` copy of `web/` and copied back.

Still open from this work: the four gaps above; a phase field through
`Router` and one `[router.phases]` table; decision spans + a memo; human
override at the spec gate (`record_spec` accepts only `Rules`); and making a
delegated task actually open a bot thread and send it.

---

**Session, 17 Oct 2026 — bots, projects and routines in Chat's Agents side.**
The owner sent Kuvo Code screenshots (bots with a name, face and job; a "Start
From" sheet; a bot page with "Start its job"; a bot that asks what its routine
should cover and then wakes itself). Design and decisions:
`docs/AGENTS-PROJECTS-ROUTINES-2026-10-17.md`. What landed:

- **Projects are a folder plus the bots assigned to it** (`crates/core/src/projects.rs`,
  `<data>/agent-projects.json`). Deliberately separate from Workbench projects:
  no tickets, branches or approvals. One project per canonical folder; a bot can
  be on several. Agents assigned to a project and talked to from it run in the
  project folder instead of their own workspace (`chat.rs` cwd choice) and read
  project memory.
- **Routines are part of a bot** (`Agent.personality`, `Agent.routines`), because
  a routine needs the bot's model, rules, memory, folder and computer. Own
  creation flow (`RoutineDialog`: what → when → where) reachable from the bot
  page, the advanced editor and Upcoming routines. `fleet.rs` fires due routines
  while the app is open: 30 s weak-ref tick, posts `Routine · <name>` into the
  bot's (project) thread via the same `chats.event` the webhook uses, then
  advances the wake; a busy bot stays due. Manual "Run now" posts without moving
  the wake.
- **New-bot sheet** (`bot-create.tsx`): Start From (7 templates) → Name & job →
  Look (colour + face) → Personality (9 chips) → Job → Model → Lives On (This
  Mac / a project), plus "add its first routine". `Agent.job` stores the
  one-liner, a blank line, then the long instructions; every subtitle shows the
  first line.
- **Bot page** (`BotHome`) for an empty agent thread: avatar, job, status,
  the Kuvo copy, "Start its job", routine cards with next wake / Run now / edit.
- **Sidebar/home** (`Agents.tsx`, `projects.tsx`): New bot, filter, **Bots** with
  Working… / Needs you / Wakes … / Watching status lines, **Chats** + Groups,
  **Projects**; home has bot cards, Upcoming routines, project cards. The "Bots"
  label is the way home (and from a project thread it lands on the project page).
- **Commands** (`main.rs`): `agent_projects`, `agent_project_save`,
  `agent_project_delete`, `agent_project_assign`, `routine_save`,
  `routine_delete`, `routine_run`, `routines_sync`. The bridge is in
  `web/src/bridge.ts` (`Project`, `Routine`, project page state, status and wake
  helpers).
- **The reference's question interaction is a message primitive now**: a bot can
  put fenced `ask` / `routine` JSON blocks in a reply and the chat face draws
  them as a question card (options, Skip, Answer) and a routine proposal (Add
  routine / Not now); `Agents::brief_in` teaches both conventions, "Start its
  job" invites the ask block, and `web/src/app/ask.tsx` is the renderer. Answer
  sends `Answer: …`, Skip sends `Skip this question.`, Add routine goes through
  the same `routine_save` as the sheet. Invalid JSON is one quiet line, never
  source.
- **A shell/Tailwind collision found and fixed**: `ui/style.css`'s shell-wide
  `.grid { height: 100% }` (Code's canvas container) is unlayered, so it beat
  Tailwind's `.grid` utility everywhere in the window; inside the chat face
  every grid inherited the full height (the ask card's checkbox drew 2px of
  border). The rule is now scoped to `#grid` — the only element that carries it
  (`app.js`/`home.js` set `#grid.className`) — and the uiharness asserts the
  checkbox is 16x16 so it cannot come back silently.

**Ran:** `cargo test -p backspace-core` green (113 lib + every integration
suite, including the new routine next-wake maths with a negative UTC offset,
routine save/booking, brief/personality/project text, and the projects store);
`cargo check -p backspace-tauri` green; `npx tsc --noEmit` green;
`npm run build` wrote `ui/chat-web`; the existing Code-surface verify
(`sidebar-and-canvas.js`) still runs clean after the CSS scoping. The uiharness
acceptance is `tools/verify/agents-projects.js` — **83 checks, four runs, all
green, no page errors** — covering the switch, sidebar statuses, home, new-bot
sheet, bot page + Start its job really sending, 3-step routine dialog, project
page + back, the ask card answering with the real payload, the routine proposal
adding with the real payload, and the checkbox regression. Screenshots in
`tools/verify/out/agents-{home,bot-page,new-bot,ask,project-page,routine}.png`.
**Not yet done:** nobody has clicked it in a real window. The chat face changed,
so a running app needs a restart; the Tauri shell assets (`style.css`) are plain
assets and need no build.

Still open from this work: routines only run while Backspace is open; a Chat
project's `machine` is stored and shown but only "this Mac" is settable; opening
a Chat project in Workbench (tickets, branches, approvals) is the natural next
step; the ask card takes choices only (no free-text "Other" field).

---

**Earlier session, 17 Oct 2026** (commits `cae2c37`, `dd9bbbb`, and the revert
that parked stacks). Three things, no new crate:

- **Ghost chats are one-off, and now behave like Whirl's incognito.** Two
  defects, both reproduced in the UI harness before the fix: turning the mode
  on left the sidebar up until the first message was sent, and leaving a ghost
  by anything other than its own toggle left the thread alive in the core
  (`chat_delete` was never called, measured as `deletesOnLeave: 0`). Now the
  sidebar collapses as the mode goes on, `goHome`/`openThread`/`setScope`
  destroy the ghost they leave behind, and `Chats::purge_ghosts` (new command
  `chat_purge_ghosts`) scrubs leftovers as the mode is entered, which is
  Whirl's `purgeIncognito` on enter. `tools/verify/ghost-flow.js` drives the
  whole flow through `window.WhirlChat` and the real composer.
- **Panes stack, parked.** The accordion was built and verified, then set
  aside on the owner's call: it lives on branch **`pane-stacks`**
  (`b25a509`), not on `main`. That commit makes a stack a column of cards, one
  open and the rest folded to a 30px header, adds `Node::activate`, lets a
  drop on the middle of a canvas stack, and raises a tab to eight canvases.
  `tools/verify/pane-stack.js` and `pane-stack-shot.js` are in that commit and
  are deleted here with it. To bring it back: `git cherry-pick b25a509`, but
  read the revert first, because `main` has moved since.
- **A comment-prune plan**, `docs/COMMENT-PRUNE-2026-10-17.md`, from a swarm
  in the pstack shape (13 auditors, 13 verifiers). 233 delete, 26 refactor,
  23 keep. Raw slices in `docs/comment-prune/`. The 15 Oct audit's fuller
  census (1718/57/333) is listed as the backlog, and the four places the two
  disagree are called out rather than merged. **Nothing was deleted.**
- **Team chat is no longer offered.** The canvas type and its renderer stay, so
  a tab that already holds one keeps working; it is gone from the "+" picker
  and from the empty-canvas chooser. Put the `board` entry back in `VIEWS`
  (`ui/app.js`) to offer it again.

**The chat face changed, so a running app must be restarted.** `web/` was
touched (the ghost fix), so `ui/chat-web` was rebuilt and is committed; the
Tauri shell (`app.js`, `chat.js`, `style.css`) is plain assets and needs no
build, but the running binary has to be restarted to pick them up.

What was verified this round, and what was not:

- **Ran:** `cargo test --workspace --exclude backspace` (166 passed, 0 failed)
  against the stack and ghost work, and `cargo check -p backspace` (the GPUI
  shell still compiles against the changed layout). The stacks themselves are
  now off `main`, so on this tree the layout covers the old leaf/split shapes
  only.
- **Ran:** `tools/verify/ghost-flow.js` in the uiharness, green, plus the
  older `ghost-real.js` for the collapse, and a boot check confirming `VIEWS`
  no longer offers Team chat.
- **Not done:** nobody has clicked either feature in a real window. The ghost
  work was checked against the real face bundle but not against a real core, so
  `chat_purge_ghosts` is only covered by its Rust test.

Still open, and worth doing next (in the order I would take them):

1. **The phase plan** (`docs/PLAN-PHASES-2026-10-17.md`): specification,
   execution, testing, deployment, plus a fifth that runs underneath all of
   them, the **Librarian** (the single writer to the memory repo: subagents
   propose, the main agent compiles, the Librarian decides, places, supersedes
   and commits once per batch). The plan also picks the decision model per
   decision kind: rules, then a local model (this machine has qwen3:1.7b,
   granite3.2:2b, gemma4), then Jev, then the strong model, with a bake-off in
   `tools/decide-eval/` before any gate depends on a model. That is the next
   build, and the plan lists what already exists for each piece.
2. **The two security items in the prune plan**, which are code and not
   comments: `companion.rs:87` accepts a URL-borne token the comment says must
   not be a credential, and `prefs.rs:218` lets `allow_lan: false` sit beside a
   LAN address. One typed `Listen` closes the second.
3. The generated comments in `crates/core/src/harness.rs`, where the numbered
   steps lost one of their number and the compiler note at 2792 is a fragment.
4. The three big refactors the audits name and no sweep will fix: `ToolDef`
   forcing `Box::leak`, the duplicated layout grammar in `ui/app.js`, and the
   two chat renderers behind `chat.js`'s `whirl` fork.
4. Everything already open from the 14 Oct block below.

---

**Previous session, 14 Oct 2026.** Everything in `docs/PLAN-NEW-FEATURES.md` is
shipped (see the table at the top of that file), plus two the plan did not
have. What changed:

- **New crate `crates/colony`** (`backspace-colony`, MIT): durable,
  chat-addressable workspaces — a WSL2 distro or Hyper-V VM named, created,
  used and destroyed by an agent through `colony_create` / `colony_exec` /
  `colony_list` / `colony_status` / `colony_destroy`. `Backend` trait with
  `VisorCli` (drives `capsule/`'s `visor-core` JSON CLI) and `FakeBackend`
  (25 tests, no VM). Docs: `docs/COLONY.md`; the landscape it borrows from:
  `docs/DURABLE-OBJECTS-2026-10.md`. `backspace-core` and `backspace-tauri`
  now depend on it; `cargo build --manifest-path capsule/core/Cargo.toml`
  builds the CLI it drives (that crate is its own workspace on purpose).
- **`crates/core/src/accounts.rs`**: several logins per CLI in their own
  profile dirs (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, …), health, cooldown, and
  `Automatic` / `Manual` / `Off`. Wired into `run_cli_turn` (`account_env`,
  health notes, a confirmed usage limit parks the login). Settings →
  **Accounts**. Never switches on a plain rate limit.
- **`crates/core/src/model_server.rs`**: finds Ollama, LM Studio, llama.cpp,
  MLX and loopback OpenAI endpoints, ranks them, marks the tiny ones (≤ 4B or
  a name/size heuristic), and recommends what to delegate to. Settings →
  **Local models**; `backspace-cli models` prints the same. The chosen model
  becomes `advisor.scout_model` (`local_subagent` in `harness.rs`).
- **Board threads**: `BoardMsg.reply_to` / `.thread`, `board::reply`,
  `conversations`, `between`; the `dm` and `read_conversation` tools (API,
  MCP and `msg send --reply`); a **Conversation** canvas beside Team chat.
- **Settings redesign**: grouped left nav (Personal / Integrations / Coding /
  System) and an Appearance panel (accent per theme, background, foreground,
  UI and code fonts, translucency, contrast) over `Prefs::appearance` +
  `set_appearance` (validated and clamped Rust-side).
- **Carry on, for workers too (audit P0-2).** A restart marks an agent
  `interrupted` instead of `failed`, its ticket goes back to `queued`, and
  `Harness::resume` re-enters the ordinary worker path: same worktree, same
  branch, and a CLI worker answers in the session saved on
  `AgentRecord::ses`, so the result still goes through check → review → merge.
  Inbox → *Needs you* has **Carry on**; a failed or still-running agent is
  refused. `crates/core/tests/interrupted.rs` covers it (the fake claude is
  node-based, so the test runs on Windows too).
- **ponytail skill** vendored (`crates/core/skills/ponytail/`, MIT) and
  injected into workers by default.
- **Terminal canvas** (`portable-pty`; `terminal_open/read/write/signal/close`
  in `crates/tauri-app/src/main.rs`, `terminalPane` in `app.js`): a real shell
  in the open project's folder, chat-free, with a bounded PTY test that runs
  no window. Home's empty state starts the same project by task *or* by view.
- Tests: `crates/core/tests/features.rs` covers conversations, the local
  scout model and account failover end to end without a network or a VM.

Still open, and worth doing next (in the order I would take them):

1. **Local routing fails hard when the runtime is down.** A job ticked in
   Settings → Local models is chosen *first*; if Ollama is stopped the job
   fails instead of falling back, and `backspace-cli models` still says
   "nothing running: small jobs stay on the main model" while a model is
   configured. One cached liveness probe per runtime would close both.
2. **State growth (P1-5)**: `board` is never trimmed, `parked` keeps finished
   conversations forever, `agents` is fully serialized on every save, and every
   event ships the whole `ProjectState` over IPC.
3. **Panic surface (P1-6)**: pervasive `lock().unwrap()`, direct indexing
   (`s.agents[id]`), `project.rs`'s `ancestry` on a stale parent.
4. **Half-accounted CLI workers (P1-8)**: their spend never reaches
   `total_cost_usd`; Codex's thread id is not captured, so a resumed Codex
   worker starts cold even though a Claude one does not.
5. `prefs.json` permissions on Windows (P0-4, partly), a per-turn timeout for a
   hung CLI, moving worktree workers onto `backspace-runner`, and
   checkpoint→switch→resume for accounts (today a turn that dies on a limit is
   re-asked on the new login rather than resumed).

**This round changed the Tauri shell, so the running app must be rebuilt**
(`cargo build --release -p backspace-tauri`) to see the Settings redesign, the
Conversation and Terminal canvases, and the Accounts/Local models panels. The
`web/` chat face was not touched, so `ui/chat-web` does not need a rebuild.

What was verified this round, and what was not:

- **Ran:** `cargo test --workspace --exclude backspace` — 133 tests green,
  including `crates/colony` (25), `crates/core/tests/features.rs` (11, among
  them an end-to-end proof that a `scout` delegation reaches the chosen local
  model over HTTP), the e2e suite, and the bounded PTY test in
  `crates/tauri-app`. `cargo check -p backspace-tauri` is clean.
- **Ran:** `cargo run -q -p backspace-cli -- models` against this machine —
  it finds the four runtimes and reports that none is up.
- **Ran:** `backspace-tauri.exe` (debug, isolated `BACKSPACE_DATA`) — it
  launched, painted (12 WebView2 processes), survived 18 seconds, and wrote
  nothing to stderr. The **new UI was not clicked through**: the Settings
  panels, the Conversation canvas and the Terminal canvas need the owner's
  eyes in a real session. On this host ConPTY opened but produced no output,
  which the canvas reports rather than hides.

---

State as of 7 Oct 2026, commit `56ff91a` on `main`.

## Where things are

- **Repo:** `github.com/chamsco/Workbench`. It was renamed from `chamsco/ohMyHarness`, which was earlier `chamsco/backspace`. The old local clone's remote still says `ohMyHarness`. Pushes work through GitHub's redirect, but the `gh api` proxy rejects the redirect. Run this first:
  `git remote set-url origin https://github.com/chamsco/Workbench`
  It also needs `chamsco/Workbench` added to the session's repo scope.
- **Branch:** work and push on `main`. The owner has asked for a push after each milestone.
- **Commit trailer:** every agent commit ends with its own attribution and a session line — `Co-Authored-By: Claude …` + `Claude-Session: <date>` from a Claude session, `Co-Authored-By: Backspace Agent <agent@backspace.invalid>` + `Agent-Session: <date>` from any other. Attribute the agent that actually wrote the commit. No model IDs appear in commits.
- **Related repo:** `chamsco/AnarchyMessenger`. It was compared with this one and never modified.
- **Not in the repo:** `HARNESSES.md` (the owner's roadmap note) was attached in chat, not committed. Ask for it if needed.

## What it is

The owner calls it "a bring-your-agent space": an orchestrator plus harness. It has two shells over one Rust core.

| Path | What |
|---|---|
| `crates/runner` | `backspace-runner` (MIT). Runs one turn on a CLI (Claude Code, Codex…), Ollama, an OpenAI-compatible API or an A2A agent, and returns a stream of `Event`s. Meant to be shared with Anarchy. |
| `crates/core` | Everything else: chat (`chat.rs`), the Workbench coding harness (`harness.rs`, `fleet.rs`, tickets, worktrees), the memory repo (`memory*.rs`), tracing (`trace.rs`), the MCP server (`mcp.rs`), the board (`board.rs`), CLI discovery (`harnesses.rs`), cloud, apps, the phone link (`companion.rs`), prefs. |
| `crates/tauri-app` | **The main app.** Tauri 2 with vanilla JS in `ui/`. The chat face is React/Vite in `web/`, built to `ui/chat-web/`, and the build output is committed. |
| `crates/app` | GPUI shell. It has only the coding workbench and lags behind Tauri. |
| `crates/cli` | Headless shell: `scan`, `serve`, `cloud`. |
| `.github/workflows/macos.yml` | Builds a universal `.dmg` on macos-14 and publishes the `mac-latest` pre-release. |

The README is current. Read its "Layout", "Run" and "Known limits" sections first.

## Shipped this session

- **Workbench.** Code's "Agents" view was renamed to Workbench.
- **Memory is an Agent Memory Repo.** `<data>/memory` is a git repo of markdown with metadata on each bullet. Each change is one commit.
  - Dreaming (a daily tidy, run by the model) is **off by default**. Undo is a `git revert`.
  - Notes you rejected are kept in `memory-rejected.json`, and they never come back.
- **MCP server** moved out of `board.rs` into `mcp.rs`.
- **The runner crate.** Chat runs on it, which made `chat.rs` about 480 lines shorter.
  - Child CLIs get no parent Claude Code session variables (CLAUDECODE, CLAUDE_CODE_SESSION_ID…). This fixed a real bug where a reply resumed the wrong session.
- **Tracing.** Each reply is a trace, with a span per tool call, saved in `<data>/traces/`.
  - The waterfall is in `web/src/app/Trace.tsx`.
  - Export is OTLP/HTTP JSON, set in Settings → Tracing.
- **Carry on.** A reply cut off by quitting is marked `interrupted`.
  - Claude Code resumes on its own session. Other agents get the transcript plus the partial reply.
  - Progress is saved before and after each tool call.
- **Bring your agent.**
  - A2A agents (`RouteKind::A2a`, `prefs.remote_agents`).
  - Presets for xAI, Meta Muse and OpenAI.
  - A "Your first agent" step in setup.
  - Remote agents start with memory **off**, so your notes don't go to third parties.
- **Mac build (CI).** Pushed but **never seen to pass**.

## Open, in priority order

1. **Mac build.** Check the first `macOS app` Actions run and fix it until it's green. Likely failures:
   - Tauri CLI feature checks on the `tauri` dependency, which appears twice in `crates/tauri-app/Cargo.toml`;
   - icon generation;
   - `bundle.active` being overridden with `-c`.

   Then have the owner test on their Mac. Things to check:
   - whether CLIs installed under nvm are found when the app is opened from Finder;
   - the see-through window and overlaid title bar (`macOSPrivateApi`);
   - ad-hoc signing on Apple Silicon.
2. **Security: the phone link (`companion.rs`).** It listens on `0.0.0.0` with a long-lived token in the pairing code, over plain HTTP, and chats on it drive agents that run commands. This was flagged as "fix first" in HARNESSES.md. Possible fixes:
   - short-lived pairing plus a separate session token;
   - bind only when pairing is on;
   - consider TLS or Tailscale only.
3. **Workbench workers.** `harness.rs`/`fleet.rs` workers don't use `backspace-runner` yet, and they don't resume after a restart. Moving them onto the runner would give them traces and Carry on.
4. **treg.** It's untested because `treg.to` didn't resolve from the sandbox. Get its `llms.txt` from the owner and check that it accepts OTLP/HTTP JSON, or add an exporter.
5. **A2A gaps.** No `message/stream` streaming, no push notifications, no file parts. Only tested against a local Python agent.
6. **Agents the owner named that can't be connected yet.** OpenAI Dots and xAI Grok Bot are consumer apps with no API. WorkBuddy is **Tencent's**, not Alibaba's, and its open platform is for partners only. Setup says so. Revisit if APIs appear.
7. **Scope.** The owner likes the "orchestrator plus harness" direction. The previous session pushed back: the app now spans Workbench, Chat, Agents, Memory, Apps, Cloud and Phone. The case for bringing an agent here is memory, groups, traces and Carry on across agents, so lead with those. Consider dropping Cloud and ads, and freezing the GPUI shell.

## Build, test, run

```sh
cargo test                                   # runner + core: 49 lib + agents, bya, e2e, resume, runner http
cargo test -p backspace-runner
(cd crates/tauri-app/web && npm ci && npm run build)   # after editing web/, commit ui/chat-web
cargo run --release -p backspace-tauri       # Linux needs libwebkit2gtk-4.1-dev
cargo run -p backspace-runner --example ask -- claude "hi"
```

- **Sandbox tips.**
  - The sandbox runs out of disk. Delete `target/*/incremental` and old GPUI debug binaries when rustc segfaults or ld bus-errors.
  - The real app was driven under Xvfb with xdotool. WebKit typing lags, so wait before taking screenshots.
  - Kill processes by PID: `pkill -f "claude -p"` kills your own shell.
- **Tests that need no network or keys.**
  - Fake `claude` scripts go through `BACKSPACE_CLI_PATH` (see `crates/core/tests/resume.rs`).
  - Local TCP servers stand in for A2A and OpenAI (see `tests/bya.rs` and `crates/runner/tests/http.rs`).

## Working with the owner

- Be concise.
- Don't flatter.
- Stress-test their ideas and lead with what's wrong.
- They write fast and informally, so take the intent, not the literal words.
- They expect work committed and pushed, not just proposed.
