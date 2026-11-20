# How Orca and T3 Code actually run agents

Read from source on 20 Nov 2026: `stablyai/orca` (v1.4.214, Electron, MIT) and `pingdotgg/t3code` (Effect/TS server + web/desktop/mobile, MIT). Both are MIT; this is a study of their approach, no code copied.

## The one idea both share

**Neither app owns a model login. They drive the vendor's own CLI, signed in with the vendor's own login command.** T3's README says it directly: "T3 Code does not resell tokens. It drives the agents you are already signed in to." Every "account" in either app is a separate config folder for the official CLI.

## 1. How each provider is driven

| Provider | T3 Code | Orca (structured chat) | Login command |
|---|---|---|---|
| Claude | `@anthropic-ai/claude-agent-sdk`, which spawns the installed `claude` binary (stream-json both ways) | same SDK, `claude-stream-json-connection.ts`; terminal mode runs the raw TUI | `claude auth login` (Orca adds `--claudeai`) |
| Codex | `codex app-server` (JSON-RPC over stdio: threads, turns, approvals, `/goal`) | `codex app-server` too | `codex login` / `codex login --device-auth` |
| Cursor | official `@cursor/sdk` (local agent runtime); older path was ACP | `cursor-agent --disable-auto-update acp` | `agent login` or `CURSOR_API_KEY` |
| Grok | `grok agent stdio` (ACP) | ACP / terminal + hooks | `grok login` |
| OpenCode | `opencode serve` HTTP server per thread (1.x) or per instance (2.x), its SDK | `opencode` / `opencode2` | `opencode auth login` |
| Gemini / Google | Antigravity: Google's official ACP agent, installed and signed in by T3 | Gemini CLI terminal; Antigravity ACP | Google sign-in in the agent, or Gemini API key, or Vertex |
| Others | Kiro (`kiro-cli acp`), Pi (RPC), any ACP agent | ~40 CLIs, most as plain terminals | each CLI's own |

Two transport families matter:
- **Long-lived structured sessions** (Codex app-server, Claude SDK stream-json, ACP, OpenCode HTTP). One process per thread; you can steer mid-turn, answer approvals and questions, interrupt.
- **One process per turn** (`claude -p`, `codex exec --json`, `cursor-agent -p`, `opencode run`, `grok -p`). Simpler, resumable by session id, but no mid-turn steering. **This is what Cauldron uses today.**

## 2. Multiple accounts

Same pattern in both, per CLI:
- Claude: one `CLAUDE_CONFIG_DIR` per account (never change `HOME`: on macOS that hides the Keychain entry). The app runs `claude auth login` with that variable set, then launches every turn with it.
- Codex: one `CODEX_HOME` per account. Orca mirrors your real `~/.codex/config.toml` into each managed home so settings stay one place. T3 also has a "shadow home" so two Codex logins can continue the same threads.
- Antigravity: one profile per instance, forced to file credentials (shared Keychain would leak between accounts), ambient Google credentials removed from the env.
- Cursor: per-instance login, or `CURSOR_API_KEY` per instance.
- "System default" = the CLI's normal folder; added accounts never rewrite it.
- Switching: new turns use the selected account; a running turn finishes on the old one. Orca guards against overlapping token refreshes while switching. Quota is never pooled.

## 3. Sub-agent delegation and orchestration

**T3 (orchestration v2):** the app runs an MCP server and injects it into every provider session with a credential scoped to (environment, parent thread, provider instance, session). Tools: `orchestrator_capabilities`, `delegate_task`, `task_status`, `task_cancel`, `create_threads`, `t3_thread_send/wait/read/interrupt`. `delegate_task` creates an app-owned **child thread on any provider** (Claude can hand work to Codex), with only the task prompt (parent history is not copied), optional worktree isolation, and `mode: wait | async`. In async mode the parent ends its turn and is **woken with the result** when the child finishes. Child questions are routed to the parent. Rules: child permissions may only narrow; results are idempotent by `clientRequestId`.

**Orca:** the agent itself uses the `orca` CLI (taught by a skill): `run-create`, `task-create`, `worker-start --agent codex --worktree new-child`, then ends its turn. Workers report with `orca orchestration send --type worker_done --outcome ...` (must include task and dispatch ids), ask with `orca orchestration ask`, and the coordinator is woken by inbox mail. Decision gates block tasks until answered. Permission prompts escalate to the person; coordinators may approve only if the person turned that on.

**Shared lesson:** delegation is an app-owned tool the agent calls, the child runs as a normal thread you can open, and the parent is *woken* by the result rather than polling.

## 4. Auth and the terms-of-service question (honest version)

| Provider | Using your subscription from a third-party app | Risk |
|---|---|---|
| **Claude** (Pro/Max) | Anthropic formally banned using subscription OAuth tokens in third-party apps (Feb 2026; OpenCode/OpenClaw-style direct token use was cut off). Launching the **official `claude` binary** with its own login is what T3 and Orca do and is widely treated as allowed, but it is not explicitly blessed for wrappers. Never read or reuse Claude's token ourselves. | **Medium.** Safe path: official CLI only, or an Anthropic API key. |
| **ChatGPT / Codex** | OpenAI supports Codex CLI login with a ChatGPT plan. T3 additionally uses an OpenAI-provided "dynamic agent client" sign-in (`chatgpt.tokens.use.direct` scope) that asks you to allow plan sharing. | **Low** via the official `codex` CLI. The direct-token program needs OpenAI's client registration; unverified for us. |
| **Google / Gemini** | Google has banned accounts that used Gemini CLI / Antigravity OAuth through third-party tools (OpenClaw, early 2026). Running Google's **own** Gemini CLI or Antigravity agent binary with its own login is the official path; T3 does exactly that. | **Medium-high** for anything that touches Google's OAuth ourselves; **low-medium** for the official binary. API key is safest. |
| **Grok** | Official `grok` CLI with `grok login`. | Low-medium (young product, terms still moving). |
| **Cursor** | Official `cursor-agent` / `@cursor/sdk`, needs a paid plan, or `CURSOR_API_KEY`. | Low. |
| **OpenCode** | Open source; it brings its own provider logins. Using OpenCode with a **Claude subscription** is the case Anthropic shut down. | Low for OpenCode with API keys or OpenCode Zen; **high** with a Claude subscription. |

Rule for Cauldron: **never handle a vendor's subscription token. Spawn the vendor's official CLI, signed in by its own login command, in a per-account config folder.** Offer an API key route for each provider as the zero-risk fallback.

Sources: [WinBuzzer, Feb 2026](https://winbuzzer.com/2026/02/19/anthropic-bans-claude-subscription-oauth-in-third-party-apps-xcxwbn/), [AlternativeTo](https://alternativeto.net/news/2026/2/anthropic-officially-bans-using-subscription-authentication-for-third-party-claude-use), [gemini-cli discussion #20632](https://github.com/google-gemini/gemini-cli/discussions/20632), [openclaw #14203](https://github.com/openclaw/openclaw/issues/14203), [gemini-cli ToS doc](https://github.com/google-gemini/gemini-cli/blob/HEAD/docs/resources/tos-privacy.md). These are press and community sources, not the vendors' legal text; read the current terms before shipping to anyone else.

## 5. Other things worth copying

- **Setup must not be a health-check side effect** (T3): probing a provider must not start MCP servers, hooks or a login browser.
- **Updates only through the installer that owns the binary** (T3): prove Homebrew/npm/mise ownership from the real path, otherwise show the command.
- **Usage per account** read from the CLI's own reports, never estimated (Cauldron already has this rule).
- **Chat or terminal for the same session** (Orca): the transcript view and the raw TUI are two views of one conversation.
- **Permissions**: both default to the CLI's bypass flag in worktrees but warn that a worktree is not a sandbox. Cauldron is ahead here (bwrap / Seatbelt / Windows low-integrity jail).
