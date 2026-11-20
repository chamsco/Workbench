# Cauldron vs Orca / T3 Code: what stops it working, and what I built

Checked on 20 Nov 2026 against `claude/project-thread-gg3ogp` (c71dbfe), by running the real GPUI app (Linux, Xvfb) and the CLI against the real `claude` CLI on a subscription login. Branch with the fixes: `claude/project-thread-0cb445`.

## What already worked (it is less of a shell than it looks)

- The runner drives Claude Code, Codex, Cursor, OpenCode and Grok headless, one process per turn, resumable by session id (`crates/runner`). Same "drive the official CLI" model as Orca and T3.
- A new stream in the GPUI app answered for real: "Reply with exactly: pong" came back from Claude in ~10 s.
- The harness (familiars / `backspace-cli <folder> "<goal>"`) ran on the Claude subscription with no API key: it edited the file in a worktree, ran a check and committed.
- Multi-account storage existed (`accounts.rs`: one profile folder per login, health, cooldown, failover), but only the legacy harness used it.

## Why it felt like a shell on your machine (most likely, in order)

1. **Only Claude can ever answer.** The new-stream box picks Claude if `claude` is on PATH, else nothing ("Connect a subscription to start a thread"). There is no model or provider picker anywhere in the GPUI app, so Codex, OpenCode, Gemini, Grok and Cursor are unreachable even when installed.
2. **No way to sign in or add an account from the app.** Settings > Subscriptions says accounts "appear here once added" and offers no button.
3. **Windows CLI discovery was thin.** The extra search folders were keyed on `HOME`, which Windows does not set, and `%APPDATA%\npm` (where `npm i -g` puts `claude.cmd`, `codex.cmd`) was not searched. An app started from the Start menu with a stale PATH would find no CLI at all.
4. **Accounts never reached chat streams**, so a second Claude or Codex login could not be used outside the legacy harness.
5. **Sub-agents in streams were not live**: the + menu's "Spawn a subagent" is marked not-live, and the GPUI shell never pumped the relay, so even tile hand-offs did not run there.
6. **Gemini was missing** from the runner and the CLI list.
7. Bug: `backspace-cli` busy-looped forever once stdin closed (580 failed `claude --resume` launches in 10 minutes in my test).

I can't see your machine's logs from here. If it was something else (an error text under the composer, a crash), tell me what you saw.

## Built on this branch (tests green, verified live where marked)

| Gap | Change | Proof |
|---|---|---|
| 4 | `Accounts.active` per CLI ("system" = the machine's login); every CLI turn in a stream gets the picked account's profile variable (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `GEMINI_CLI_HOME`...) | unit test |
| 2 (core) | `harnesses::auth_with(id, env)`: signed-in state and who, per account folder. `open_login_with(id, env)`: opens the CLI's own login command with that folder set, so the login lands there and your normal login is untouched | unit tests |
| 3 | Windows: `%APPDATA%\npm`, `USERPROFILE` home folders, WinGet links, pnpm, Volta | needs a Windows run |
| 5 | `delegate` + `list_providers` MCP tools for Claude and Codex streams. A sub-agent is a new stream (`↳ title`) on any CLI, in the same project, given only the task; its answer comes back as the parent's next turn. Hops capped at 2. The GPUI shell pumps every second | **live**: a Claude stream delegated "17 × 23" to a sub-agent stream and replied "The sub-agent says 391" |
| 6 | Gemini CLI: `gemini -p --output-format stream-json`, approval mode from the trust setting, sign-in detection from `~/.gemini` | unit test with recorded line shapes; not run against a real Gemini CLI (not installed here) |
| 7 | CLI stops at end of input and never auto-approves on EOF | live run exits cleanly |

## Still to build (next on this thread)

1. **Model and account picker** in the new-stream box and the stream composer: provider, model, account, with not-installed / not-signed-in states. (Gap 1. Design board first.)
2. **Subscriptions page**: every CLI with installed / signed-in / who, Sign in, Add account, pick active account. (Gap 2 UI.)
3. **Appearance page** modelled on Capy's, and the **centred composer** after Uplight's.
4. Sub-agent rows under the parent in the sidebar, and the + menu's "Spawn a subagent" wired to `spawn_sub`.
5. Long-lived sessions (Codex `app-server`, Claude stream-json input) for mid-turn steering and approvals. Today every turn is a fresh process, so a stream can't be steered mid-turn and approvals are decided by the trust setting up front. This is the biggest remaining difference from T3 and Orca; it's a larger rewrite of the runner, and I recommend it after the pickers.
6. Usage per account from the CLIs' own reports (Codex `/status`, Claude rate-limit events) to drive automatic failover.

## Not verified

- No Windows or macOS run of these changes yet (CI builds Windows; the `windows-preview` download will carry them once merged).
- Codex, Cursor, OpenCode, Grok and Gemini were not run for real here: only Claude is installed and signed in in this container.
- The harness's long path (tickets, Rampart, parallel workers) was not exercised end to end; the short path was.
