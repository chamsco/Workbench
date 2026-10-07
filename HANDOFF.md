# Handoff: Backspace (ohMyHarness → Workbench), v0

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
