# Harness review, 18–25 Oct 2026

Built from `harness-summary.json` (7-day window ending 2026-10-25). Nothing in CLAUDE.md or AGENTS.md has been changed. Everything below is a proposal for you to review.

**Read this first: the sample is small.** The window holds 8 sessions plus 1 subagent run. Only 3 sessions are interactive working sessions (plus their subagent), and they come from 2 projects. The other 5 are short automated runs: 4 t3code title-generation calls and 1 Codex session sent by the Backspace app. The report's own session (`2cdebcce`) is excluded. Treat every suggestion as a hypothesis to confirm over the next few weekly runs.

Session ids are the first 8 characters. Click any of them in `dashboard.html` to see its waterfall.

## Headline numbers

| Metric | Value | Notes |
|---|---|---|
| Tool calls | **585** | All Claude Code. Codex logged 0 tool calls in the window. |
| Prompts | **20** | 10 typed by you, 1 sent by the Backspace app through Codex, 5 auto-continue or peer prompts, 4 from the t3code title generator |
| Tool error rate | **2.2%** | 13 failed calls. There were also 7 `usage_limit_reached` API stops, which are not tool errors. |
| Tokens | **87.4M** | 96.7% of these are cache reads: each turn re-reads the cached context. |
| p95 tool latency | **17.5 s** | 9.1 s if you exclude subagent and advisor calls. Advisor calls alone have a p50 of 75 s. |

## 1. Model routing

| Model | Sessions | Tokens | Tool error rate | Where it was used |
|---|---|---|---|---|
| claude-opus-5-5 | 3 main + 1 title run | 75.0M | 2.3% (11 of 488) | All your interactive sessions: UI/debug in `b6a817e5`, game build in `c09c048d` and `2def0bbc` |
| claude-sonnet-5-5 | 1 subagent | 12.4M | 2.1% (2 of 97) | The godot-arena subagent `ae17d3df` |
| gpt-6-luna (Codex) | 1 | 18.7k | not logged (0 tool calls) | `01a16f3f`, a single Kira prompt from the Backspace app |

Proposals:
- **Keep Opus for interactive UI, debug and build sessions.** Its error rate matches Sonnet's, and every human-driven task ran on it.
- **Keep Sonnet for subagent exploration.** `ae17d3df` made 97 calls at 2.1% errors, which is as good as Opus at lower cost. This matches your existing "scout for discovery" rule.
- **Route the t3code title generator to Haiku.** `b5ef403a`, `cc882199`, `eaff4d1b` and `77aceeeb` are 1-prompt "generate a title" runs. One of them (`b5ef403a`) ran on Opus. This is a t3code setting, not a CLAUDE.md line.
- The data can't support per-task routing yet. No task type had failures concentrated on one model.

## 2. Tools

**Keep** (heavily used, with few or no failures):

| Tool | Calls | Failure rate |
|---|---|---|
| Bash | 199 | 3.0% |
| PowerShell | 53 | 0% |
| Read | 44 | 0% |
| Grep | 32 | 0% |
| Write | 29 | 0% |
| Edit | 20 | 0% |
| hearthbot reply/status | 60+ | 0% |

**Fix** (the failures are concentrated here):

| Tool | Failures | Error types | Sessions |
|---|---|---|---|
| Bash | 6 of 199 | 3 shell syntax (unexpected EOF in long inline scripts), 1 Python traceback, 1 non-zero exit, 1 permission denied | `b6a817e5`, `ae17d3df` |
| Claude_Browser javascript_tool | 2 of 42 (4.8%) | `TypeError` reading properties of null or undefined | `b6a817e5` |
| Claude_Browser preview_start | 1 of 3 | missing `.claude/launch.json` | `b6a817e5` |
| computer-use request_access | 1 of 5 | requested a browser through computer use. p95 wait was 290 s. | `b6a817e5` |

**Cut, or deny if never wanted** (1 call or fewer in 7 days, and some failed):

| Tool | Calls | Outcome |
|---|---|---|
| `computer` | 1 | failed: no such tool |
| Monitor | 1 | failed: input validation |
| hearthbot fetch_thread | 1 | failed: invalid id |
| Agent, SendMessage, docs connector | 1–4 each | all ok |

These counts are too small to justify cutting anything yet. Recheck them next week.

## 3. Skills

The only task type you repeated 3 or more times is **writing**: 3 prompts. Two are yours: "write a plan.md of all the things you did" in `b6a817e5` and "write a handoff file" in `2def0bbc`. The third (`01a16f3f`) is an app-generated Kira prompt. That makes this a borderline candidate:

```markdown
---
name: handoff-writeup
description: Use when asked to write a plan.md, handoff file, or summary of the work done in this session.
---
1. List what changed: files, branches, PRs, and decisions with their reasons. Read git diff/log rather than relying on memory.
2. List what is unfinished or unverified, and what the next session needs to know first.
3. Write it to the path the user names (default: HANDOFF.md at the repo root), no more than one screen per section.
4. Reply with the path and the 3 most important open items.
```

The "other" bucket (12 prompts) is mostly automated: 4 title-generation runs, auto-continue messages, and short replies such as "Réessayer". It doesn't call for a skill.

## 4. Guardrails: top 3 failure patterns

1. **Long inline shell scripts break in Git Bash.** In `b6a817e5`, 3 "unexpected EOF" errors and 1 Python traceback each triggered a retry. *Check:* any script longer than about 5 lines goes into a file in the scratchpad and runs from there, and Windows-native work uses PowerShell.
2. **Browser JS clicks on elements that don't exist.** `b6a817e5` had 2 `TypeError` null/undefined failures, and both were retried. *Check:* confirm the element first with `find`/`read_page`, and null-guard inside `javascript_tool`.
3. **Calling tools before their prerequisites exist.** There were 5 cases: `preview_start` without a launch.json, the non-existent `computer` tool, `request_access` for a browser, a Monitor validation error, and an invalid thread id (`b6a817e5`, `c09c048d`, `ae17d3df`). *Check:* load deferred tools with ToolSearch before calling them, create launch.json before `preview_start`, and use browser tools rather than computer use for web pages.

A fourth pattern is outside the tool calls: **7 usage-limit stops**. Two were in `b6a817e5`, one in `c09c048d` and one in the subagent `ae17d3df`. The other three are the title runs `cc882199`, `eaff4d1b` and `77aceeeb`, which logged 0 tokens because they hit the limit. Both main sessions then auto-continued on their full context, and a handoff plus a fresh session would restart on a smaller one.

## 5. Budget: top 5 token sinks

| # | Sink | Tokens | One change |
|---|---|---|---|
| 1 | `b6a817e5` project-browser: 307 tool calls, 9 turns, 3.0 h | 47.8M (97% cache read) | Start a fresh session with a handoff file after about 150 tool calls or after a usage-limit stop. The whole context is re-read every turn. |
| 2 | `c09c048d` godot-arena: 148 calls, 4.2 h | 24.3M | Same rule. This session hit a usage limit and auto-continued on the full context. |
| 3 | `ae17d3df` godot-arena subagent: 97 calls on Sonnet | 12.4M | Give subagents a narrower brief and a "return a summary" rule (your scout pattern) so they stop sooner. |
| 4 | Bash output returned into context | 325k chars | Pipe through `head`/`tail`/`grep` and avoid dumping whole logs. |
| 5 | Read output returned into context | 274k chars | Read with `offset`/`limit` when you only need part of a file. |

Rows 4 and 5 measure characters of tool output, which is a proxy. The logs do not record tokens per tool.

## Proposed diff (not applied)

`~/.claude/CLAUDE.md`:

```diff
 # Advisor checkpoints
 - Consult the advisor before finalizing a multi-file plan.
 - When the same test or compiler error fails twice, consult the advisor before trying again.
 - Before declaring a task done or staging a commit, have the advisor audit the full diff.
 - Use the scout subagent for discovery; it returns summaries only.
+
+# Shell on Windows
+- Bash is Git Bash. Put any script longer than ~5 lines (heredocs, inline python) in a scratchpad file and run the file; use PowerShell for Windows-native work.
+- Trim command output (head/tail/grep) and read large files with offset/limit.
+
+# Browser and desktop tools
+- Before a javascript_tool click, confirm the element exists (find/read_page) and null-guard the script.
+- Load deferred tools with ToolSearch before calling them; create .claude/launch.json before preview_start; use browser tools, not computer-use, for web pages.
+
+# Long sessions
+- After ~150 tool calls or any usage-limit stop, write a handoff file and continue in a fresh session.
```

`AGENTS.md` (Codex): there is no proposal. The window holds 1 Codex session with 0 tool calls, and `~/.codex/history.jsonl` has no entries in the window.

## Rerun weekly

```
python harness_summary.py --days 7
python render_dashboard.py
```

Use `--exclude-session <id prefix>` to leave out a session that is still running.
