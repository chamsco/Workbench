# Workbench v0 audit (7 Oct 2026)

What works, what is broken or missing, how it compares with similar tools,
and what connecting people's own agents (ChatGPT, Claude, Grok) really takes.
The most important items come first in each section.

## Verdict

The core loop works and is well tested: goal → grill → tickets → parallel
workers in worktrees → check gate → review → merge, with escalation and traces.
On Linux and macOS, `cargo test` passes. **On Windows the coding side didn't
work at all** (five separate bugs, all fixed here). The biggest product gap
remains: **the planner needs an API key**. That locks out exactly the people
who would sign in with a ChatGPT or Claude subscription.

## Fixed in this pass

| # | Bug | Effect | Fix |
|---|---|---|---|
| 1 | `crates/tauri-app/icons/icon.ico` missing | The Tauri app did not build on Windows | Generated it from `icon.png` |
| 2 | `canonicalize()` returns `\\?\C:\…` on Windows | Every `git worktree add` failed, so no ticket could start | `dunce::canonicalize` (`harness.rs`, `fleet.rs`) |
| 3 | `which()` returned npm's extensionless shell shim (`claude`, `codex`) before `claude.cmd` | Every CLI spawn failed with os error 193 (scan, Pair, Chat, CLI workers) | Check `.exe`/`.cmd` first on Windows (`harnesses.rs`) |
| 4 | A bare `bash` resolves to System32's WSL launcher before PATH | API workers' `bash` tool ran inside WSL on translated paths | Prefer Git for Windows' bash (`tools.rs`) |
| 5 | `<html>` got class `win`, which also matches `.win` (the window) | `.win:not(.m-code) .code-only` hid Code's tab bar, `+` and sidebar switch on Windows | The platform class is now `os-<platform>` |
| 6 | Tests assumed LF checkouts and a POSIX fake `claude` | 2 unit and 3 e2e failures on Windows | Normalised CRLF; the shell-script CLI test is now unix-only |

`cargo test -p backspace-core -p backspace-runner` now passes on Windows: 49
unit tests plus bya, e2e and http.

## Holes still open, by priority

1. **The planner can't run on a subscription.** The main agent needs Backspace's
   own tools (`create_tickets`, `work_tickets`), so `router.rs` refuses to put it
   on a CLI. With no Anthropic or OpenRouter key, Workbench can't start. The
   `backspace mcp` bridge already exposes the board to CLIs. Expose
   `create_tickets` / `work_tickets` / `revise_ticket` the same way, and the
   planner can run on Claude Code or Codex. This is the one change that makes
   "sign in and go" true.
2. **Opening a project commits your work.** `git::ensure_repo` runs `git add -A`
   and commits on your current branch ("snapshot before run") whenever the
   tree is dirty. That includes untracked files your `.gitignore` misses, such
   as a stray `.env`. Instead, fork the run from a temporary commit object
   (`git stash create`) or a `backspace/base` branch, or ask first.
3. **Nothing survives a restart.** `state.json` is written and never read
   (`save()` says "not a resumable checkpoint"). After a restart, tickets,
   approvals and in-flight workers are gone, and approval `oneshot`s die with
   the process. This is HANDOFF item 3: put workers on `backspace-runner` to
   get Carry on and traces for free.
4. **No Stop for a project or a ticket.** The harness has no cancel. A hung CLI
   worker (no timeout in `run_cli_turn`) holds its ticket until you quit.
5. **The state fan-out grows without bound.** Agent logs are never capped by
   count. Every event clones the whole `ProjectState` (all logs) and sends it
   over IPC, and `save()` rewrites the full pretty JSON on each status change.
   Fine at 6 agents; a Wall of 20 chatty agents will feel it. Send deltas, or
   cap logs and send the tail only.
6. **CLI workers are only half-accounted.**
   - Their spend never reaches `total_cost_usd`, so budgets don't apply.
   - Codex's thread id isn't captured, so feedback rounds start cold.
   - Claude workers get unrestricted `Bash` with `acceptEdits`. A worktree
     isolates the git branch, not the machine.
7. **The phone link and machine sharing are plain HTTP with long-lived bearer
   tokens.** The companion binds `0.0.0.0`, and chats on it drive agents that
   run commands (HANDOFF item 2). Fix this before the phone app ships.
8. **Workbench workers have no traces.** Traces exist for Chat replies, not for
   workers, which run outside the runner. "Trace your agentic workflow" is
   true for Chat only today.
9. **Scope.** Seven zones: Workbench, Pair, Chat, Agents, Memory, Apps, Cloud.
   The GPUI shell lags the Tauri one. Freeze GPUI and park Cloud and ads until
   items 1–4 are done.

## Compared with similar tools

| | Parallel agents in worktrees | Bring your own CLI/subscription | Plan → tickets → review gate | Multi-model routing and escalation | Cross-agent chat and memory | Resumable after restart |
|---|---|---|---|---|---|---|
| **Backspace Workbench** | yes | CLIs for workers; **not the planner** | yes, diagram-first | yes (unique) | yes (board, AMR memory) | no |
| Conductor, Emdash, Superset, Crystal | yes | yes | no; you are the planner | no | no | yes (sessions) |
| Vibe Kanban | yes | yes | kanban plus review | no | no | yes |
| Claude Squad (tmux) | yes | yes | no | no | no | yes (tmux) |
| OpenAI Codex app, Cursor agents view | yes | their own models only | partial | no | no | yes |

Where Backspace is ahead:
- one planner for many agents and models;
- the check gate before review;
- start-cheap escalation;
- review that starts from a diagram;
- a shared memory repo.

Where it's behind is table stakes the others all have:
- **sessions survive restarts**;
- **real terminals** you can type into;
- **one-click sign-in**;
- **Stop**.

Close those four before adding zones.

## Connecting people's agents: what is actually allowed

| Provider | What exists (Oct 2026) | What Backspace does now | Recommended path |
|---|---|---|---|
| **ChatGPT** | "Sign in with ChatGPT" is official. It shipped in Codex, then opened to partners at DevDay on 29 Sep 2026; apps need a client ID from OpenAI. The Codex **app-server** (stdio JSON-RPC) runs login and credentials for embedders. | Settings → Coding CLIs → **Sign in with ChatGPT** opens `codex login` in a terminal. Codex workers then run on your plan. | Embed `codex app-server` (it owns login, streaming and approvals), or apply for a client ID to call Responses directly. Both are sanctioned. |
| **Claude** | Anthropic **bans subscription OAuth in third-party products**. The docs changed on 19 Feb 2026 and the ban was enforced from 4 Apr 2026. Third parties must use API keys (Console, Bedrock, Vertex). | **Sign in to Claude Code** opens `claude auth login`. Workbench then drives the user's own Claude Code headless. | **Risk:** driving Claude Code headless on a Pro/Max plan from another product is the kind of "third-party automation" the ban targets. Lead with an Anthropic API key for Claude, keep the CLI path as "your own Claude Code" at the user's discretion, and don't market "Sign in with Claude". This is your call. |
| **Grok** | xAI has a browser OAuth (PKCE, `accounts.x.ai`) for SuperGrok and X Premium+, used by Pi, Kilo Code and Hermes. I found no public client registration or terms for it. xAI's official CLI is **Grok Build**. | The scanned `grok` is the community `@vibe-kit/grok-cli` (API key only). The xAI router preset takes an API key. | Keep the API key for now. Wire Grok Build as a CLI once its headless flags are confirmed. Add OAuth only if xAI publishes a third-party program. |

Each sign-in opens the provider's **own** login in a visible terminal.
Backspace never sees or stores the credentials, and the providers' terms stay
with their own tools.

## The Workbench views (new)

The Workbench title bar now switches between three views. The choice is
saved in `prefs.wb_view`.

- **Canvases:** the tabs of split canvases, as before.
- **Wall:** every agent's live session tiled, as many as there are, each
  scrolling on its own. Needs-you tiles are outlined. Expand one into a canvas.
- **Inbox:** "Needs you" first (plans and work to review, failures, tickets
  needing info, the main agent waiting on an answer), then "Working", then
  "Done or idle". It has a box to talk to the main agent, and Review opens the
  diagram.

The Wall shows each agent's stream, not an interactive shell. Typing into a
worker's terminal would need a PTY (`portable-pty` plus xterm.js) and workers
running interactive CLIs. That's worth doing together with item 3.

Sources:
- [OpenAI: Unlocking the Codex harness (app-server)](https://openai.com/index/unlocking-the-codex-harness)
- [OpenAI: Sign in with ChatGPT preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)
- [Sign in with ChatGPT for Codex developers (DevDay 2026)](https://codex.danielvaughan.com/2026/08/02/sign-in-with-chatgpt-identity-platform-codex-cli-oauth-developer-authentication-strategy/)
- [AlternativeTo: Anthropic bans subscription auth for third-party use](https://alternativeto.net/news/2026/2/anthropic-officially-bans-using-subscription-authentication-for-third-party-claude-use)
- [WinBuzzer: Anthropic bans Claude subscription OAuth in third-party apps](https://winbuzzer.com/2026/02/19/anthropic-bans-claude-subscription-oauth-in-third-party-apps-xcxwbn/)
- [Hermes Agent: xAI Grok OAuth](https://hermesagent.org.cn/en/docs/guides/xai-grok-oauth)
- [Kilo Code: xAI provider](https://kilo.ai/docs/providers/xai)
