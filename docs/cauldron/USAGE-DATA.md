# Usage data: what core can truthfully tell the Usage page (2026-11-14)
**Short answer:** from files already on disk, core can report counts and tokens per tool, not a
limit, percentage left, reset time or cost. Those need something Charles must provide (last
section); until then the app keeps saying "No data yet" for them.

## What is real now
`usage::tool_usage_summary(home, now_ms)` returns, per tool, `today` (last 24 h, rolling) and
`week` (last 7 days): sessions, prompts, tool calls, tokens (input, output, cache read, cache
write), plus `source_path` and `as_of` (when the windows end). No network, no login.

| Tool | Read from | Each number comes from |
|---|---|---|
| Claude Code | `~/.claude/projects/**/*.jsonl` | `message.usage` once per `message.id`; prompts are typed user turns; subagent tokens included |
| Codex | `~/.codex/sessions/**/*.jsonl` | `token_usage_record.usage`, never cumulative `thread_token_usage`; cached input is cache read |
| OpenCode | `~/.local/share/opencode/opencode.db` | `message` and `part` rows, opened read-only |

A tool whose folder is missing is left out (UI: "No data yet"); a folder with nothing in the
window gives zeros. Tests use `crates/core/tests/fixtures/usage_home`, never the real home.

## What does not exist
- **Limit percentages ("62% left this week") and reset times:** no log the repo reads holds
  them. `accounts.json` `Usage` is filled only from a confirmed limit error (the CLI's own words
  in `report`); `used` and `limit` are never set in core, so Settings rightly shows "No data yet".
- **A quota to divide by** (the plan's allowance is in no local file, so tokens give no percentage)
  **or a cost in dollars** (no subscription price table). **Cursor, Gemini CLI:** no reader yet.

## Leads (recalled, not verified: no real Claude Code or Codex home on this machine)
- **Codex:** rollout files may carry rate-limit snapshots (used percent, reset) in token-count
  events. Core does not parse them and the fixtures do not have them.
- **Claude Code:** the status line command gets JSON on stdin that, on recent versions with a
  Pro or Max login, includes 5-hour and 7-day usage percent and resets. It is written to no log,
  so it exists only if a status line command saves it. An undocumented usage endpoint also exists
  (network and the CLI's OAuth token); not used.

## What Charles would have to provide
1. Codex: one redacted `rollout-*.jsonl` from a recent session. With `rate_limits` in it,
   reading it is a small change; without, Codex limits are not available locally.
2. Claude Code: either agree to a `statusLine` command in his Claude settings that appends its
   JSON to a file in the Cauldron data folder, or allow one call to the usage endpoint.
3. Without one of these nothing more is possible: counts and tokens are the whole truth.
