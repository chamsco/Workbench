# Plan: new functions (12 Oct 2026)

**Shipped in the same session (14 Oct 2026)** — every one of the six, plus two
the plan did not have:

| # | Feature | Where |
|---|---|---|
| 6 | ponytail skill | `crates/core/skills/ponytail/`, injected into workers (`config.rs`), adapter note in `skills.rs` |
| 5 | Settings left nav + Appearance | `SET_NAV`/`appearancePanel` in `crates/tauri-app/ui/app.js`, `.setnav`/`.ap` in `style.css`, `Prefs::appearance` + `set_appearance` |
| 2 | Visual options + compare | already shipped as `d5b78ac` (`options.rs`, `optionsPane`) |
| 1 | Multi-account + failover | `crates/core/src/accounts.rs` (registry, health, policy, `env_for`), wired into `run_cli_turn` (`account_env`, health notes, usage-limit parking), Settings → Accounts |
| 3 | Agent↔agent conversations | `board.rs` (`reply`, `thread_of`, `conversations`, `between`), `dm`/`read_conversation` tools, MCP + `msg send --reply`, the **Conversation** canvas |
| 4 | Chat-free canvases + start by view or agent | Conversation/Terminal/Empty canvases hold no chat; a real **PTY terminal** pane (`portable-pty` + `terminal_*` commands in `crates/tauri-app/src/main.rs`, `terminalPane` in `app.js`); Home's empty state starts the same project by task or by view (`startByView` in `home.js`) |
| 7 | Local tiny models for subagents | `crates/core/src/model_server.rs` (detect/rank/recommend), Settings → Local models, `prefs.subagent_model` → scouts (`local_subagent` in `harness.rs`) |
| 8 | Colony workspaces | `crates/colony/` (registry + visor-core backend + `colony_*` tools), `docs/COLONY.md`, Settings → Workspaces and Colony |

Read the original plan below for the reasoning; the file paths in it were
written before the code moved and are kept as history.

Six features. Ordered by leverage, not by the order you listed them. Each one
names the files it touches, so we can pick a starting point without guessing.

Guiding rule from `ponytail`: the smallest complete change. Several of these
are mostly wiring, because the core already has the shape (projects, tickets,
worktrees, canvases, the board).

---

## 1. Multi-account subscriptions + auto-failover  *(the big one)*

**Goal.** Several accounts per coding CLI/provider, kept apart, with health
and usage tracked, switching automatically when one hits its limit — the csm
principle, native in Rust.

**Design.**
- New `crates/core/src/accounts.rs`: `Account { id, name, provider, profile_dir,
  enabled, priority, health, cooldown_until, usage, last_used, last_ok,
  last_fail }`. Stored as `accounts.json` under the data dir; profiles live in
  `<data>/accounts/<id>/`.
- Each account is a **profile directory**, injected per spawn via the agent's
  own env var: `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `GEMINI_CLI_HOME`,
  `COPILOT_HOME`. Identity is verified per profile before use.
- `run_cli_turn` (`harness.rs:1330`) and `harnesses.rs` gain an account
  argument; the chosen profile's env is layered onto the spawn.
- Health/usage: parse the CLI's own report (Claude `StopFailure` hook /
  status line; Codex app-server `account/rateLimits/read`). Never estimate.
- Failover: on a *confirmed* usage limit only, checkpoint → switch to the
  best account (healthy → unknown → warning; fewer 24h failures; priority) →
  resume the same session. Policy in prefs: `auto` / `manual` / `off`.
- Optional: hand off to another **agent** when every account is out, reusing
  the existing cross-agent path in `chat.rs` + a redacted task brief.

**Touches:** new `accounts.rs`; `harnesses.rs` (profile discovery, identity),
`harness.rs` (`finish_cli`, `run_cli_turn`), `provider.rs` (key rotation for
API providers), `prefs.rs` (policy + account list), `tauri-app` (Settings →
Accounts UI, a `set-account`/`next-account` command).

**Smallest first slice:** profiles + manual switch (`next`/`use`) for Claude
Code and Codex, no auto-failover. Then add detection, then failover.

---

## 2. Visual options + old-vs-new comparison

**Goal.** A coding agent shows several design options as **screenshots** to
pick or iterate from, and can put an old and a new screenshot side by side.

**Design.**
- A new **`options` artifact**: the agent calls a tool (e.g. `show_options`)
  with N named variants, each a URL + label + note. The harness screenshots
  each (headless browser already implied by the browser canvas) and returns an
  id.
- A new canvas kind **`options`**: a row of thumbnails with a "Choose" pill
  (image 1), and a **compare** mode: side-by-side with a draggable split
  (image 2). Choosing returns the picked variant to the agent as a tool result.
- Reuse the browser canvas (`app.js:1117` `<iframe>`) for capture, and the
  lightbox/`chat_file` data-URL path (`chat.js:438`) for rendering images.

**Touches:** `harness.rs`/`tools.rs` (new tool), `mcp.rs` (expose to CLI
workers), `app.js` (`VIEWS`/pane kinds at `app.js:533`, `buildPane`/`fillPane`),
`style.css`, a screenshot helper in `core` (headless capture).

**Smallest first slice:** a static `options` canvas that renders N images the
agent wrote to `.backspace/options/<id>/` plus a `show_options` tool; choosing
posts the pick back on the board. Compare mode after.

---

## 3. Agent-to-agent conversations (main ↔ sub, sub ↔ sub)

**Goal.** Real conversations, not just a flat board.

**Design.**
- Extend `board.rs`: add `to` (direct) and `thread`/`reply_to` fields to
  `BoardMsg`; keep the broadcast as a default. Add `dm` tool alongside
  `send_message`.
- Surface it: a **Conversation** canvas that renders a thread between two
  agents (or the main agent and a worker), reusing the chat message CSS.
- Main↔worker: today the main agent only sees worker logs at turn boundaries
  (`harness.rs:1510` poll-at-turn-start). Add a push path so a worker can ask
  the main agent a question mid-ticket.

**Touches:** `board.rs` (message shape + cursors), `mcp.rs` (new tool),
`harness.rs` (delivery + a mid-ticket question), `app.js` (canvas kind),
`style.css`.

**Smallest first slice:** direct messages + a Conversation canvas. Push from
worker→main after.

---

## 4. Canvases free of chat + agentic view split from terminals

**Goal.** Chat-free, multipliable canvases; an agentic view separate from
terminal windows; a project startable by view *or* by agent.

**Design.**
- Add a **`terminal` pane** — the one thing the design target had that we
  don't. Needs a PTY (`portable-pty`) + `xterm.js`, and workers that can run
  interactively. Gate it behind a feature flag so the shell keeps working.
- Add a **chat-free canvas mode**: a canvas that never hosts a chat, only
  view panes (board/files/browser/diagram/docs/options/terminal). Mostly a
  filter on the existing `chooser` (`app.js:821`).
- A **project start** entry that opens either a view (canvas layout) or an
  agent (Workbench run) for the same folder — one dialog, two buttons.

**Touches:** `crates/tauri-app` (new pane + `portable-pty` dependency),
`app.js` (`VIEWS`, `chooser`, `buildPane`), `index.html` (xterm asset),
`shell.css`/`style.css`.

**Smallest first slice:** chat-free canvas mode + project-start-by-view-or-
agent. The PTY terminal is its own slice (it needs a new dependency and a
capability change).

---

## 5. Settings redesign (match the reference)

**Goal.** Grouped left nav and a real Appearance panel.

**Design.**
- Left nav groups: **Personal** (General, Profile, Appearance, Notifications,
  Chat behavior, Keybindings, Usage & limits), **Integrations** (Connections,
  MCP), **Coding** (Agent providers, Models & writing, Agent skills, Managed
  worktrees), **System** (System tools), **Archived** (Archived threads).
  The existing sections already carry stable `id="set-*"` anchors to hang on.
- Appearance: theme swatches (System/Light/Dark), then per-theme **Accent /
  Background / Foreground** color pickers, **UI font**, **Code font**,
  **Translucent sidebar** toggle, **Contrast** slider. New prefs fields:
  `appearance: { accent, bg, fg, ui_font, code_font, translucent, contrast }`,
  mapped onto the existing CSS variables in `style.css:16-70`.

**Touches:** `app.js` (`renderSettings` `app.js:1393`, `applyTheme`
`app.js:221`), `style.css` (vars + new panel CSS), `prefs.rs` (new struct +
`set_appearance` command), `main.rs` (command registration).

**Smallest first slice:** the grouped left nav over the existing sections
(pure `app.js`/CSS, no backend), then the Appearance pickers + prefs.

---

## 6. Vendor the ponytail skill

**Goal.** The "lazy senior dev" ruleset available to every agent.

**Design.** Drop `skills/ponytail/SKILL.md` + `AGENTS.md` into
`crates/core/skills/` next to the mattpocock ones, add the adapter note that
maps its commands onto Backspace's `skill` tool, and add it to the default
injected set where it makes sense (implementation workers). It is MIT, so
vendoring is fine — keep the license header and the upstream commit like the
others.

**Touches:** `crates/core/skills/` (+ `UPSTREAM`), `skills.rs`,
`config.rs` (`workflow.*_skills`).

---

## Suggested order

1. **#6 ponytail** — an hour, unblocks better code in everything below.
2. **#5 Settings left nav** — self-contained UI, immediately visible.
3. **#2 Visual options** — self-contained, high demo value, reuses the
   browser canvas.
4. **#1 Multi-account** — the biggest, start with profiles + manual switch.
5. **#3 Agent conversations** — builds on the board, pairs with #1's handoff.
6. **#4 Terminal + chat-free canvases** — the PTY is its own project.

Items 2, 5 and 6 are independent and safe. Item 1 is the one with real design
risk (CLI-specific profile vars and limit detection), so it wants its own
session.
