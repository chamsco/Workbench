# Bots, Projects and Routines in Chat's Agents side

Written 17 Oct 2026, against `1fb2ce0`, from the Kuvo Code reference the owner
sent (X post by @beentherebtw: bots with a name, a face and a job; a "Start
From" template sheet; a bot page with "Start its job"; a bot that asks what its
routine should cover, then wakes itself up).

This document is the build spec for this round: the model, the flows, the exact
interfaces, and who writes which file. It also answers the two questions the
owner left open — **how a project is structured**, and **whether routines are
their own flow or part of an agent**.

## The two decisions

### 1. A project is a folder plus the bots assigned to it

A project is not a container of chats and not a container of agents. It is the
**scope work happens in**:

```
Project { id, name, folder, machine, agents[], created, updated }
```

- `folder` is an absolute path on this Mac (a remote machine later, reusing
  `machine` from prefs; empty `machine` means this Mac).
- `agents` is the list of bot ids assigned to it. A bot can be on several
  projects; a project can host several bots.
- One project per folder. A second project on the same path is an error, not a
  second entry.
- A bot keeps its own folder and its own memory. **When it works on a project,
  the project folder is its working directory**, and project memory is in
  scope. Its rules (`off`), apps, computer and route are unchanged.
- A thread that belongs to a bot and a project carries both: `agent` and
  `project` (the folder, as threads already store `project` as a path). It is
  listed on the project page and in the bot's chats, and it does not leak into
  Code → ADE, which only lists threads with no agent.
- Projects are stored once, in `<data>/agent-projects.json`. They are not
  tabs, not `prefs.projects`, and they do not open Harness worktrees. This is
  the Chat side's own lightweight project, deliberately separate from the
  heavy Workbench project in Code.

Why this shape: the owner asked for agents to work on projects. The cheapest
honest version is "the bot runs in the project folder and the project page
collects its bots, chats and routines". Anything more (tickets, branches,
approvals) already exists in Code → IDE and should not be duplicated in Chat.
When a Chat project needs the heavy machinery, the door is "Open in Workbench"
— a later step, not this one.

### 2. Routines are part of a bot — with their own creation flow

A routine is the bot's job on a clock. It needs the bot's model, rules, memory,
folder and computer, so storing it anywhere but on the bot means the first
question of every routine is "which bot runs this?". The decision:

- **Owned by the bot:** `Agent.routines[]`, persisted in the agent's own
  `agent.json`. Deleting a bot deletes its routines; there is no orphan list.
- **Its own flow, not a field buried in the editor:** a stepped `RoutineDialog`
  (What it does → When → Where) reachable from the bot page ("+ Add routine"),
  from the bot's first-run page ("Start its job" when the bot was created from a
  template with a routine), and from "Upcoming routines" on the Agents home
  (which first asks which bot, then opens the same dialog).
- **Project optional:** a routine may run in one of the bot's projects. When it
  does, its message lands in the bot's project thread (created on first fire)
  and the bot works in that folder.

Why not a standalone routines store with a "assign to agent" step: it would
duplicate attribution and every surface would need to resolve a routine to a
bot anyway. The "assign" moment still exists — it is the bot preselected at the
top of the dialog, and the picker when you start from Upcoming routines.

### The reference, kept and adapted

| Kuvo | Here |
|---|---|
| Sidebar: New Session, Search, **Bots**, Chats, Projects, Connect an Account | New bot, filter box, **Bots** (status line), Chats (one-to-one + groups), **Projects**, no account row (ours is in Settings) |
| Bot row: avatar, name, "Watching" / "Wakes in 2 days" / "Working…" | same status language, computed from thread busy/unread + next routine |
| New Bot sheet: Start From grid → Name/Job → Look (colour, shape, face) → Personality → Job → Model → Lives On (This Mac / machine + folder) | same sections in one scrolling sheet; shape/face becomes our emoji + colour faces; "Lives On" = This Mac (own folder) or a project folder |
| Bot page: avatar, name, one-line job, "Start its job" / give it a task; "keeps this conversation, remembers what it learns, and can wake itself up later" | bot page is the empty agent thread: same copy, "Start its job", routine cards with next wake and Run now |
| Bot asks "Which topics should your digest cover?" with Skip/Answer | the bot's first routine is configured in the Rationale sheet — a template's routine fields (name, instructions, when) are prefilled and the user edits topics there. Chat-driven structured answer cards are a later step (see Not in this cut) |
| "Connect an Account" | existing Settings → Providers/Connections, left alone |

## Data model

### Rust — `crates/core/src/agents.rs`

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Routine {
    #[serde(default)] pub id: String,
    #[serde(default)] pub name: String,          // "Morning digest"
    #[serde(default)] pub instructions: String,  // what to do when it fires
    /// "daily" | "weekdays" | "weekly" | "every"
    #[serde(default)] pub kind: String,
    /// "HH:MM" local, for daily/weekdays/weekly
    #[serde(default)] pub time: String,
    /// ISO weekdays, 1 = Mon .. 7 = Sun, for weekly
    #[serde(default)] pub days: Vec<u8>,
    /// every: minutes between runs
    #[serde(default)] pub every_minutes: u32,
    /// Project id this run belongs to; "" = the bot's own folder
    #[serde(default)] pub project: String,
    #[serde(default = "yes")] pub enabled: bool,
    #[serde(default)] pub last_run: u64,
    #[serde(default)] pub next_run: u64,        // absolute ms; computed by core
    #[serde(default)] pub runs: u32,
    /// Minutes east of UTC, sent by the UI (JS offset negated).
    #[serde(default)] pub utc_offset_minutes: i32,
    #[serde(default)] pub created: u64,
    #[serde(default)] pub updated: u64,
}

impl Routine {
    /// The first occurrence strictly after `from` (0 when disabled).
    pub fn next_after(&self, from: u64) -> u64;
    /// "Every day at 08:00", "Wakes Mon, Wed at 07:30", "Every 6 hours".
    pub fn summary(&self) -> String;
}
```

`Agent` gains, both with `#[serde(default)]`:

```rust
pub personality: String,   // the style text appended to the system prompt
pub routines: Vec<Routine>,
```

`Agents` gains methods (all take the agent's JSON lock):

- `routine_save(&self, agent: &str, r: Routine) -> Result<Agent>` — upsert by
  id, generate id/created, sanitise (`name` required, `kind` known, `time`
  `HH:MM`, weekly needs at least one day, every needs `every_minutes >= 5`),
  recompute `next_run = next_after(now)` for an enabled routine, save.
- `routine_delete(&self, agent: &str, routine: &str) -> Result<Agent>`.
- `routine_ran(&self, agent: &str, routine: &str, at: u64) -> Result<Agent>` —
  scheduler bookkeeping: `last_run = at`, `runs += 1`,
  `next_run = next_after(at)` (0 when disabled); no other change.
- `brief_in(&self, a: &Agent, project: Option<&Path>) -> String`; `brief`
  becomes `brief_in(a, None)`. It appends `personality`, says "You are working
  on the <name> project, in <folder>" when a project is given (the name comes
  from the folder's last component; the store gives the nicer name when the
  caller has it), and lists routines with their `summary()` and a line saying
  that a message starting `Routine · <name>` is a routine that is due, to be
  carried out now and reported in one line.
- `save` sanitises `routines` the same way (so an old editor round-trip cannot
  write garbage) and refreshes `next_run` for enabled routines whose time has
  passed.

Next-run maths live in a private `civil` helper in `agents.rs`: convert
`utc_ms + offset` to days and ms-of-day, Hinnant's `civil_from_days` /
`days_from_civil` (the pattern is already in `git.rs`), pick the next
candidate, convert back. Unit tests must cover: daily tomorrow, weekdays
skipping a weekend, weekly day selection, every-N, and a round trip in a
negative offset.

### Rust — `crates/core/src/projects.rs` (new, added to `lib.rs`)

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    #[serde(default)] pub id: String,
    pub name: String,
    pub folder: String,
    #[serde(default)] pub machine: String,
    #[serde(default)] pub agents: Vec<String>,
    #[serde(default)] pub created: u64,
    #[serde(default)] pub updated: u64,
}

pub struct Projects { /* path + Mutex */ }

impl Projects {
    pub fn open(data_dir: PathBuf) -> Self;      // <data>/agent-projects.json
    pub fn list(&self) -> Vec<Project>;           // newest first
    pub fn get(&self, id: &str) -> Option<Project>;
    pub fn save(&self, p: Project) -> Result<Project>;  // name + existing dir;
                                                        // one project per folder
    pub fn delete(&self, id: &str) -> Result<()>;
    pub fn for_agent(&self, agent: &str) -> Vec<Project>;
    pub fn assign(&self, project: &str, agent: &str, on: bool) -> Result<Project>;
}
```

Tests: save/list/rename, folder required and must exist, duplicate folder
rejected, assign/idempotent, delete.

### Rust — `chat.rs`

- `request()`: working directory becomes
  `self.project_of(id).or_else(|| agent_dir)`. That is the whole "agents work on
  projects" switch: a thread with a project folder runs there; a bot thread
  without one keeps its own workspace.
- `system_text()`: `call.agent`'s brief is built with the thread's project path
  when it is a real directory (`brief_in`), so the bot knows its project.
- The `Scope` from `chat_new` already allows `project` and `agent` together;
  no validation change. `inScope` on the UI side is what changes.

### Rust — `fleet.rs`

- field `projects: Arc<Projects>`, accessor `projects()`; build it in `new`
  from `Prefs::data_dir()`.
- `pub fn run_routine(self: &Arc<Self>, agent: &str, routine: &str) -> Result<()>`
  — fire now (manual "Run now"): find the bot and routine, post it, book it.
- private `fire_routine(&self, a: &Agent, r: &Routine) -> Result<()>`:
  - target thread: latest listed thread with `agent == a.id`, same `project`
    folder (the routine's project folder, or none), and no members; create one
    with `create_in` when there is none.
  - post with `chats.event(id, "Routine · <name>", text, ctx)` where text is
    `A routine is due. Do it now, as your job says: <instructions or name>.
     When you are done, say in one line what you did.` — not fenced as outside
    data; it is the user's own instruction. `chats.event` already answers.
  - on success call `agents.routine_ran(...)`, then nudge `notify`.
  - if the thread is busy, leave it due and retry on the next tick.
- private `schedule_routines(self: &Arc<Self>)`, started at the end of `new`:
  a weak thread that sleeps 30 s and, for every agent and enabled routine whose
  `next_run` is non-zero and `<= now`, fires it. One failure logs nothing
  (best effort) and must not kill the loop.
- `routines_sync(&self, offset_minutes: i32) -> Vec<Agent>`: store the offset on
  every routine and recompute `next_run` for enabled ones; save only when
  something changed. Called by the UI once on mount (and on save via the
  routine payload's own offset).

### Tauri — `crates/tauri-app/src/main.rs`

New commands, all `async fn` with `blocking(f, ...)` where they do file I/O:

```
agent_projects(f) -> Vec<Project>
agent_project_save(f, project: Project) -> Res<Project>
agent_project_delete(f, id: String) -> Res<()>
agent_project_assign(f, project: String, agent: String, on: bool) -> Res<Project>
routine_save(f, agent: String, routine: Routine) -> Res<Agent>
routine_delete(f, agent: String, routine: String) -> Res<Agent>
routine_run(f, agent: String, routine: String) -> Res<()>
routines_sync(f, offset_minutes: i32) -> Vec<Agent>
```

Register them in `generate_handler!` next to `agents_list`. `routines_sync` is
allowed to be sync (no blocking I/O beyond small JSON files, but it writes, so
use `blocking` like the others). The fleet exposes what they need; no new state.

## The flows

### New bot (one scrolling sheet, closing to a summary once a template is picked)

1. **Start From** — 7 tiles: Blank ("start from nothing"), Morning digest
   (Herald), Research (Sage), Build watcher (Keeper), Inbox triage (Courier),
   Explainer (Tutor), Standup (Tally). Each tile: emoji face on the bot's
   colour, name, one-line. Choosing one prefills: name, one-line job,
   instructions, personality, colour, and (for Herald/Tally/Keeper/Courier) a
   suggested routine the review step offers to add.
2. **Name & job** — name (40 chars) + one line (the sidebar subtitle). Avatar
   preview to the left, swapping face/colour by the section below.
3. **Look** — 8 colours + 12 faces (the existing `Avatar` vocabulary).
4. **Personality** — 8 chips + None; each is a sentence appended to the system
   prompt. The chosen text sits under the chips, editable in Job.
5. **Job** — the long instructions textarea (today's `job` field).
6. **Model** — `RoutePicker` (existing).
7. **Lives On** — "This Mac" (its own folder) or one of the existing projects
   (that folder). If the wizard was opened from a project, that project is
   preselected and the row says so.
8. **Footer** — Cancel / **Create bot**. When the template suggested a routine,
   a "Add its first routine" switch (on by default) creates it with the
   template's schedule. Creating saves the bot (and routine), assigns it to the
   project when one was picked, opens its thread (new, empty → bot page), and
   leaves the sheet closed.

Editing an existing bot keeps the current advanced editor (Rules, Connected
apps, Reach this agent, computer, memory): the new sheet is for creation, and
the editor gains a personality field and a "Routines" section that lists the
bot's routines with edit/delete plus "+ Add routine".

### Bot page (an agent thread with no messages, and a compact header after)

- Avatar 56, name, one-line job, status chip (`Watching`, `Working…`,
  `Needs you`, `Wakes tomorrow at 08:00`).
- Copy: "Start {name} on its job, or give it a task of your own. It keeps this
  conversation, remembers what it learns, and can wake itself up later to carry
  on." — same as the reference, same reason.
- Primary **Start its job**: sends `Start your job now. Tell me anything you
  need from me, and keep it short.` into the thread. If the bot came from a
  template with a routine and has no routine yet, it opens the prefilled
  `RoutineDialog` instead (the "bot asks what its routine should cover"
  moment, as a form).
- **Routine cards** under it: name, "Every day at 08:00 · next wake tomorrow
  08:00", Run now, edit (pencil), enable/disable switch, "+ Add routine".
- Composer doubles as "give it a task", as today.
- A bot thread with messages: avatar 40, name, one-line job, status; routine
  cards collapse into a single row ("2 routines · next wake tomorrow 08:00")
  that opens the routines section in the editor.

### Routine dialog (3 steps)

1. **What it does** — name + instructions (template-prefilled when launched
   from a bot with no routine and the bot was created from a template).
2. **When** — kind segmented control (Every day / Weekdays / Weekly / Every N
   hours), `<input type="time">` for the time kinds, weekday toggles for
   weekly, number + unit for every; a live line "Next wake: tomorrow at
   08:00" computed in JS with `next_after`-equivalent maths *for display only*
   — the backend is authoritative on save and rewrites `next_run`.
3. **Where** — project picker (own folder, or one of the bot's projects) +
   enabled switch + a summary line (name · when · where). Save.

Manual "Run now" posts immediately without changing `next_run` except
`last_run`/`runs` (it is a "do it now", not a reschedule): call
`routine_run`, then refresh.

### Sidebar (Agents side of Chat)

Top: **New bot** (primary row). A filter box. Then:

- **Bots** — every agent, avatar + name + status line. Status precedence:
  busy thread → "Working…"; unread thread → "Needs you"; next routine →
  "Wakes tomorrow at 08:00" (or "Wakes in 3 days"); else "Watching".
  A bot assigned to the open project is marked with a small folder dot.
- **Chats** — bot one-to-one threads, newest first (preview + unread dot), then
  groups under a "Groups" label; "New group" row.
- **Projects** — "Projects" label + "+ New project"; one row per project:
  folder icon, name, machine tag (this Mac when empty), bot count. Clicking
  opens the project page; right-click / long-press → rename, remove.

### Agents home (no bot open, no project open)

- "Your bots" + New bot.
- Bot cards (avatar, name, one-line job, status/routine line, Message/Edit).
- **Upcoming routines** — the next few across all bots (time, bot, name; click
  opens the bot); "New routine" asks which bot, then opens the dialog.
- **Projects** — one card per project (folder name, path, bots, "Open").
- Groups (existing block, unchanged).

### Project page (a project open)

- Header: folder icon, name, machine tag, path; "Open folder" (reveals in
  Finder/Explorer via the existing `open_url`/shell helper if available — if
  not, omit), "Edit", "Remove".
- **Bots on this project** — avatar cards; "Add bot" (assign an existing bot
  via a small picker, or create one with the project preselected).
- **Chats** — bot threads with this project (title, bot, updated), "New chat"
  (pick a bot, then open its project thread).
- **Routines on this project** — its bots' routines, with Run now.
- Removing a project deletes the project record only; bots, folders and
  threads stay, and those threads keep their project path (they fall out of the
  project page because the project no longer exists).

## Interfaces (exact)

### `web/src/bridge.ts` additions

```ts
export type Routine = {
  id: string; name: string; instructions: string;
  kind: "daily" | "weekdays" | "weekly" | "every";
  time: string; days: number[]; every_minutes: number;
  project: string; enabled: boolean;
  last_run: number; next_run: number; runs: number;
  utc_offset_minutes: number; created: number; updated: number;
};
export type Project = {
  id: string; name: string; folder: string; machine: string;
  agents: string[]; created: number; updated: number;
};
// Agent gains: personality: string; routines: Routine[].

type State = {
  // ...existing
  projects: Project[];
  /** The project page is open (null: the agents home). */
  project: Project | null;
  /** The new-bot sheet is open, optionally for a project. */
  creating: { project: string } | null;
  /** The routine sheet is open: a bot's routine, or "new" for that bot. */
  routineEdit: { agent: string; routine: Routine | "new" } | null;
};

export function useChat(): State;
export const projectById = (id?: string | null) => Project | undefined;
export const startBot = (project?: string) => void;       // set creating
export const cancelBot = () => void;
export const openProject = (p: Project | null) => void;    // set project, go home
export const editRoutine = (agent: string, routine?: Routine) => void;
export const saveAgentProject = (p: Project) => Promise<Project>;
export const deleteAgentProject = (id: string) => Promise<void>;
export const assignProject = (project: string, agent: string, on: boolean) => Promise<void>;
export const saveRoutine = (agent: string, r: Routine) => Promise<void>;
export const removeRoutine = (agent: string, r: string) => Promise<void>;
export const runRoutine = (agent: string, r: string) => Promise<void>;
export async function syncRoutines(): Promise<void>;       // called once on mount
export async function startJob(agent: Agent, thread: Thread): Promise<void>;
export const openAgentThread = (a: Agent, project?: Project | null) => Promise<void>;

// display helpers, shared by both UI files
export const routineWhen = (r: Routine) => string;   // "Every day at 08:00"
export function nextWakeLabel(ms: number): string;   // "tomorrow at 08:00", "in 3 days", "now"
export function agentStatus(a: Agent, thread?: ThreadInfo | null): { label: string; kind: "working" | "needs" | "wake" | "idle" };
```

`refresh()` also fetches `agent_projects` into `projects`. `openAgentThread`:
with a project, latest thread matching agent + project folder, else create
`chat_new` with `scope { agent, project: folder }`. Without one, latest agent
thread *without* a project folder, else create with `{ agent }`.
`startJob`: `chat_send` on the thread id with the kickoff text, then `refresh`.
`syncRoutines` calls `routines_sync` with `-new Date().getTimezoneOffset()` and
refreshes agents.
`routineWhen`/`nextWakeLabel` only format; `next_wake` comes from Rust.

`inScope` changes to:

```ts
export const inScope = (t, scope, agentsMode) =>
  !t.app && (agentsMode ? isAgentThread(t)
                        : (t.project ?? null) === scope && !isAgentThread(t));
```

### `web/src/app/avatar.tsx` (new, extracted from Agents.tsx)

```tsx
export const COLORS: string[];
export const FACES: string[];        // 12 emoji faces (front-facing, calm)
export function Avatar(props: { agent?: Agent; size?: number; ring?: boolean }): JSX.Element;
export function Stack(props: { ids: string[]; size?: number }): JSX.Element;
```

### `web/src/app/bot-create.tsx` (new, bots-ui)

```tsx
export const BOT_TEMPLATES: BotTemplate[];
export const PERSONALITIES: { name: string; text: string }[];
export function NewBotDialog(): JSX.Element | null;   // reads creating from the store; null when closed
export function BotHome(props: { agent: Agent; thread: Thread }): JSX.Element;
```

`NewBotDialog` renders into a `<Dialog>` (whirl `components/ui/dialog`). It
saves with `saveAgent` (route from the host), then `assignProject` when a
project was chosen, then `saveRoutine` when "first routine" is on, then
`openAgentThread(saved, project)`.

### `web/src/app/routines.tsx` (new, bots-ui)

```tsx
export function RoutineDialog(): JSX.Element | null;      // reads routineEdit
export function RoutineCards(props: { agent: Agent }): JSX.Element;
export function UpcomingRoutines(props: { agents: Agent[] }): JSX.Element;
```

### `web/src/app/projects.tsx` (new, projects-ui)

```tsx
export function ProjectsSide(): JSX.Element;   // sidebar section for AgentsSide
export function ProjectHome(): JSX.Element;    // main column when store.project is set
export function ProjectDialog(): JSX.Element | null; // create/edit (store.creatingProject inside the file)
export const projectFolderName = (p: Project) => string;
```

### `web/src/app/Agents.tsx` (projects-ui, after bots-ui lands)

`AgentsSide` composes: New bot (`startBot()`), filter, Bots, Chats (+ groups),
`<ProjectsSide />`. `AgentsHome` composes bot cards, `<UpcomingRoutines />`,
projects cards, groups. `AgentThreadHeader` uses `<BotHome />` when the thread
has no messages, the compact header otherwise. `AgentEditors` renders
`<NewBotDialog />`, `<RoutineDialog />`, `<ProjectDialog />`, and the existing
advanced editor (gains personality + a Routines section). `Avatar`/`Stack`
import from `avatar.tsx` (re-export them from `Agents.tsx` so existing imports
keep working if anything else imports them).

### `ChatApp.tsx` (Lead)

- `<AgentsHome />` becomes `{project ? <ProjectHome /> : <AgentsHome />}`.
- `useEffect(() => { void syncRoutines(); }, [])` on mount.
- import `ProjectHome` from `projects.tsx`.

## Who writes what and in what order

| Task | Owner | Files (write scope) | Depends on |
|---|---|---|---|
| Backend: routines, projects, cwd, scheduler, commands, tests | teammate `backend` | `crates/core/src/agents.rs`, `crates/core/src/projects.rs`, `crates/core/src/lib.rs`, `crates/core/src/chat.rs`, `crates/core/src/fleet.rs`, `crates/tauri-app/src/main.rs`, `crates/core/tests/agents.rs` | this spec (Lead writes the bridge first) |
| Bot wizard, bot page, routine dialog/cards | teammate `bots-ui` | `crates/tauri-app/web/src/app/bot-create.tsx`, `routines.tsx` (both new) | bridge contract (Lead) |
| Sidebar/home/project surfaces | teammate `projects-ui` | `crates/tauri-app/web/src/app/projects.tsx` (new), `crates/tauri-app/web/src/app/Agents.tsx` | first task: projects.tsx; second: Agents.tsx after bots-ui lands |
| Bridge, avatar extraction, ChatApp, docs, build, integration | Lead | `crates/tauri-app/web/src/bridge.ts`, `avatar.tsx`, `ChatApp.tsx`, `docs/`, `HANDOFF.md` | — |
| uiharness fixture + screenshots + report | teammate `verify` | `tools/verify/agents-projects.js` (new), `tools/uiharness/evals/*.js`, `tools/verify/out/*.png` | chat-web built (Lead) |

Only the Lead runs `npm run build` (it empties `ui/chat-web`); teammates
typecheck with `npx tsc --noEmit` from `crates/tauri-app/web` and never write
`ui/`. The backend owns `cargo test -p backspace-core` and
`cargo check -p backspace-tauri`.

## Verification

1. `cargo test -p backspace-core` green (new `agents` routine tests, new
   `projects` tests, existing agent tests updated for the two new fields).
2. `cargo check -p backspace-tauri` green, then `cargo test -p backspace`
   if the shell touched anything shared (it should not).
3. `cd crates/tauri-app/web && npx tsc --noEmit` green, then Lead:
   `npm run build`.
4. `node tools/uiharness/run.mjs` with mock `agents_list`, `agent_projects`,
   `chat_list`; the verifier:
   - switches Chat → Agents, screenshots the sidebar (Bots/Chats/Projects) and
     the home;
   - opens New bot, walks the template sheet through all sections, screenshots;
   - opens a mocked bot thread (empty messages), screenshots the bot page with
     "Start its job" and a routine card;
   - opens the routine dialog, screenshots all three steps;
   - opens a mocked project, screenshots the project page;
   - asserts no page errors and that the mocked commands were called.
5. A note in `HANDOFF.md`: what was built, what was verified, what needs a
   restart, what is next.

## Message controls: a question with choices, and a routine proposal

The reference's "Which topics should your digest cover?" with Skip / Answer is
now a real message primitive, not a screenshot. A bot can put two fenced JSON
blocks in a reply; the chat face draws them as controls and every other reader
(a CLI, the phone, a plain Markdown viewer) sees a code block.

````
```ask
{"question": "Which topics should your digest cover?", "multiple": true,
 "options": ["AI and coding agents", "Apple and the Mac", "Open source"]}
```
````

````
```routine
{"name": "Morning digest", "instructions": "What to do when it fires.",
 "kind": "daily", "time": "08:00"}
```
````

- **Ask card** (`web/src/app/ask.tsx`): the question, its options as checkboxes
  (or radios when `multiple` is false), Skip and Answer. Answer sends
  `Answer: a, b` into the thread; Skip sends `Skip this question.`; a user
  message anywhere after the card marks it answered (ThreadView passes
  `answered` down), so it cannot be answered twice.
- **Routine proposal**: name, schedule (`routineWhen`), the instructions, and
  Add routine / Not now. Add routine saves it through the same
  `routine_save` path the sheet uses; the card then says Added because the bot
  has a routine with that name. A reply from a non-bot thread shows it without
  the Add button.
- **Teaching the model**: `Agents::brief_in` ends with both conventions — one
  question per reply, 2–6 short options, "a routine you propose is not scheduled
  until the user adds one". "Start its job" also invites the ask block
  explicitly. Invalid JSON is never shown as source: it is one quiet line.
- **Why fences and not tools**: a reply can be read back by another model or a
  CLI; a fence costs a line where a tool round-trip costs a turn and can fail.
  The same reasoning is why they are taught in the brief and not in OpenUI's
  richer language.
- **A shell CSS collision found on the way**: `ui/style.css` had a shell-wide
  `.grid { height: 100%; display: grid }` for Code's canvas container. Because
  it is unlayered it beat Tailwind's `.grid` utility in the chat face, so every
  Tailwind grid inherited the full height (the ask card's 16px checkbox drew a
  2px border). The shell rule is now scoped to `#grid`, which is the only
  element that ever carried it (`app.js`/`home.js` all set `#grid.className`).

## As built (same session)

Small deviations from the sketches above, recorded so the next reader does not
think they were missed:

- **`projectDialog` is in the bridge store**, like the other sheets:
  `state.projectDialog: Project | "new" | null`, `editProject(p)`,
  `cancelProject()`. `ProjectDialog()` takes no props and returns null when
  closed; the project page's "Edit" and the home's "New project" both open it.
- **`Agent.job` is one field**, so the sheet stores the one-line job, a blank
  line, then the long instructions. Every subtitle (sidebar, cards, bot page,
  project page) shows the first line only (`oneLine()`).
- **Routine dialog headings** are "What it does", "How often", "Where it works"
  with "Step N of 3"; the "Every N" kind uses a number + hours/minutes control.
  The sheet creates the first routine only when its template suggests one; the
  bot page's "Start its job" sends the kickoff message and, when the bot has no
  routine, says so in a toast (the prefilled-sheet moment lives in the new-bot
  sheet, which is the only place that knows the template).
- **`routine_save` returns the whole updated Agent**, `routine_run` resolves
  fast and posts asynchronously, `routines_sync` returns `Res<Vec<Agent>>`
  (Tauri needs a Result on async commands; the wire value is unchanged).
- **`agent_project_save` canonicalises the folder** (dunce). The UI uses the
  project from the refreshed store for thread matching, never the typed string.
- **A project's thread title on first routine fire** is `Routine · <name>` (a
  routine thread already has a message, so `send`'s auto-title never runs).
- **The project page does not reveal the folder in Finder/Explorer**: the only
  reveal command opens the heavy Workbench project, so it is omitted until
  there is a proper reveal.
- **`ProjectsSide`'s machine line** always shows something ("This Mac · N
  bots") rather than hiding when empty; it reads better than an empty gap and
  matches the reference's "acme-api @ devbox".
- **Bots can be created from a project page** (`startBot(project.id)`): the
  sheet preselects that project and assigns the bot on create.

## Not in this cut (named so nobody thinks it was forgotten)

- Running routines while the app is closed (the reference's machine-hosted
  bots). Ours fires while Backspace is open, like the rest of Fleet.
- Remote machines for Chat projects (`machine` is stored and shown, and
  "this Mac" is the only value the UI sets today).
- Opening a Chat project in Workbench (tickets, branches, approvals).
- Bots waking on an event other than a routine; webhooks already exist and are
  unchanged.
- Multi-account/model per routine; a routine uses its bot's route.
- A routine proposal cannot edit an existing routine of the same name; the
  card says Added instead. Editing goes through the routine sheet.
- The ask card collects choices only, not free text next to them; "Other" would
  need a text field in the card (the composer already covers it).
