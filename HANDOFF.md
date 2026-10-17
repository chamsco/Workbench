# Handoff: Backspace (ohMyHarness → Workbench), v0

**Latest session, 17 Oct 2026** (commits `cae2c37`, `dd9bbbb`, and the revert
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
- **Commit trailer:** every commit ends with `Co-Authored-By: Claude …` and a `Claude-Session:` line. No model IDs appear in commits.
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
