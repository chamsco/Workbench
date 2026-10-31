# M1 closure: Codex model pinning

Branch `claude/project-thread-dpmr8e`. **M1 is not approved. It stays pending the owner's decision.** Nothing from M2 or M3 was started. Read with `M1-BUDGET-BOUNDARIES.md` (section *Coding-CLI models*) and `M1-HARDENING-NOTES.md` (section *Codex closure pass*).

Commits (verified with `git log`): baseline green `b36ad3e`, previous report `27430bb`; prep `40bcbc4` (command building moved into `build_cli`, no behaviour change); red `577f819`; green `0bc6c87`. The commit that adds this file is docs only.

## Baseline

HEAD was `27430bb`, which contains `b36ad3e`; branch `claude/project-thread-dpmr8e`, level with `origin`; working tree clean; nothing committed since the report. The 491-test baseline held: `cargo test --no-fail-fast` from the repo root, 46 result lines (test binaries and doc-tests), 491 passed, 0 failed, summed from the full output (in `M1-codex-closure-suite-output.txt`). My first count, 482, came from output I had cut with `head`, and was wrong.

## A. Codex investigation

| Question | Answer | Evidence |
|---|---|---|
| Version and invocation targeted | The runner builds `codex exec --json --skip-git-repo-check`, a sandbox or approval flag, `--model <m>` when pinned, `-c` overrides, then `-` or `resume <session> -`. It targets no pinned Codex version. The latest package on npm on 2026-10-31 is `@openai/codex` 0.166.0. | `crates/runner/src/lib.rs`; `npm view` |
| Is `--model` supported | Yes. The official command reference lists `--model, -m` ("Override the model set in configuration", and under `codex exec` "Override the configured model for this run"). | learn.chatgpt.com developer commands page |
| Is `-m` supported and equivalent | Yes, by source: one clap argument, `#[arg(long, short = 'm')] pub model: Option<String>` ("Model the agent should use"), shared by `codex` and `codex exec`. The 0.166.0 binary contains the same help string. **Not confirmed by `codex exec --help`**: see below. | `codex-rs/utils/cli/src/shared_options.rs` (main branch); string search of the binary |
| Does the CLI argument beat user `config.toml`, project config, environment, profiles | Documented order, highest first: CLI flags and `--config` overrides; project `.codex/config.toml`; profile files; user `config.toml`; cloud-managed defaults; system config; built-in defaults. Project config, profile and user config: the flag wins. Environment: the page read names no variable that selects a model (the environment-variables page was not read). In the source as read, `--model` is its own override, separate from `-c`. | learn.chatgpt.com config-basic page; `codex-rs/exec/src/lib.rs` |
| Can managed or organization configuration override it | `managed_config.toml` and macOS managed preferences override the user's `config.toml` and `--config` overrides at startup. `requirements.toml` lists approval, sandbox and similar keys and has **no model key**. Whether a managed layer overrides `--model` is **not documented**. | learn.chatgpt.com managed-configuration page |
| Does the runner pass the approved identifier unchanged | Yes. `Config::cli_pin` trims the entry's `model`; that string is checked against the role's list and the same string is `req.model`, then `--model`. The runner does not transform it. Codex documents no aliases. | `cli_pin`, `cli_model`, `run_cli_turn`, `build_cli` |

**Local help output.** No Codex is installed on this machine. I downloaded 0.166.0 from npm into a scratch directory and tried to run `codex --help`; the safety check refused to execute a downloaded binary, and I did not work around it. So there is no `--help` output. To close that: `codex --version` and `codex exec --help | grep -i model` on a machine with Codex. The Claude Code that is installed (2.1.316) was run for `--help` only.

**Conclusion.** `--model` is the documented flag and `-m` is the same argument, so the runner now passes `--model`. I kept Codex as a pinnable CLI: the documentation puts the command line above every layer Backspace can see. That is a judgment call, not a proof. Three things the documentation does not settle remain (managed layers versus `--model`, a resumed session's model, the endpoint serving the name); they are listed in *Unverified external CLI behaviour*. If you would rather treat Codex as unpinned until someone has run it, remove `codex` from `PINNABLE_CLIS`: every Codex entry then needs `unpinned_cli_models`, and the Codex pin tests change to match. I did not do that, because the objective is to pin where the documentation supports it.

## B. Changes

| File | Change |
|---|---|
| `crates/runner/src/lib.rs` | `build_cli` split out of `run_cli` (commit `40bcbc4`, no behaviour change). Codex gets `--model`, ahead of `resume`. `Request.env_remove`: variables the CLI is not given. 5 argument tests. |
| `crates/core/src/harness.rs` | Under an enforced policy, a pinned Claude Code, or one given a scout, is not handed `CLAUDE_CODE_SUBAGENT_MODEL`; the log says when it was set. The open-time notices include alias pins. |
| `crates/core/src/config.rs` | `CLAUDE_ALIASES`, `is_claude_alias`, `alias_cli_pins`; `PINNABLE_CLIS` documented with the evidence and what it does not settle. |
| `crates/core/src/gateway.rs` | `model_notices` takes the alias pins and says them (a note, not a warning, enforced policies only). |
| `crates/core/tests/cli_model_policy.rs` | 13 tests added (23 to 36); the Codex invocation assertion changed (below). |
| `crates/core/tests/policy_matrix.rs` | 3 tests: the new boundaries section has the five kinds in order, matches `PINNABLE_CLIS` and `CLAUDE_ALIASES`, names each unverified item. |
| `docs/cauldron/*` | Boundaries page: row edits and the section *Coding-CLI models*. Notes: closure section, test table, a wrong count corrected. The chat issue and its evidence. This report and the outputs. |

Behaviour changed: Codex is started with `--model <m>` instead of `-m <m>`; an enforced policy no longer hands `CLAUDE_CODE_SUBAGENT_MODEL` to those Claude Code turns; the project opens with a note for a Claude Code entry that pins an alias. Nothing is refused that was allowed, so no policy needs regenerating. Legacy and open policies pass no model and keep their environment, as before (tested).

## C. Policy verification

"Verified" means on Backspace's side: what is checked and what is passed.

- **Codex.** The entry's pinned `model` is read from the config the project opened with (editing `backspace.toml` later changes nothing), looked up as written in `execution_models` (worker) or `orchestration_models` (planner and ticket-creating worker), refused before the CLI starts if absent, and passed once as `--model <m>`. No `--profile`, `-p`, `--oss`, or model in `-c`. An entry with no `model` needs `unpinned_cli_models`.
- **Claude Code.** The same, with `--model <m>`. `--model` outranks `ANTHROPIC_MODEL` and the `model` setting in the documentation; the test runs with a hostile `ANTHROPIC_MODEL` visible to the CLI and checks the argument is still the approved one. `/model` ranks above `--model` and Backspace never sends it.
- **Advisor.** `[advisor] cli` is looked up at the role of the agent that would consult it and left off, with a log line, if the lists do not name it (`advisor_policy.rs`, `cli_policy.rs`). A managed `availableModels` that blocks it makes Claude Code exit with an error, per the documentation.
- **Scouts.** `[advisor] cli_scouts` is looked up at the execution role and left off if not named. New: `CLAUDE_CODE_SUBAGENT_MODEL` is not passed, because before Claude Code v2.1.251 it overrode a definition's model.
- **A boundary the earlier rounds did not state.** A subagent that the running Claude Code model starts with its own `model` parameter (`sonnet`, `opus`, `haiku`, `fable`) and Claude Code's built-in subagents are not seen or checked by Backspace. Closing it means switching the Agent tool off, which also removes the scouts. It is a known policy exception, not closed.
- **Unpinned entries.** Only the entry's name is checked. Cursor, Grok and OpenCode always count as unpinned. Under `legacy_open` nothing is passed.
- **Lists are not widened.** A list a person wrote is never widened by generation or by the exception (`cli_model_policy.rs`, `cli_policy.rs`); adding to `unpinned_cli_models` or `routing_models` is a loosening that waits for `policy adopt`.
- **Routing model.** Jev's own model is still governed by `routing_models` and its candidates by the role lists. I did not change `router.rs` or `project.rs`; `jev_settlement` (11), `jev_unknown_cost` (15) and `routing_policy` (18) pass. The four settlement outcomes and the atomic record-and-release are untouched.
- **Matrix.** Every row of the paths table was re-read against the code. Rows edited: CLI worker (the flag, the evidence) and Claude Code scouts (the variable, the invocation-time boundary); the planner row says "exactly as for a worker" and is unchanged. `policy_matrix` (17) and `budget_wording` (5) pass.
- **Decisions kept:** `max_project_tokens` kept, no `cli_turn_tokens`, serial CLI turns and refused-not-queued API calls accepted, advisor and scout gating fail-closed, Jev's model through `routing_models`, the heuristic router as the project-wide bypass, Jev tokens not counted, the restore guard not claimed as OS-level, M6 out of M1.

## D. Test evidence

- **Red `577f819`** (full suite, 46 result lines; `M1-codex-closure-red-output.txt`): 499 passed, 8 failed. The 8: the Codex invocation in a worker, a planner, with a hostile environment and after an edit of `backspace.toml` (4); two runner argument tests (`--model`, and ahead of `resume`) (2); the subagent variable reaching a pinned Claude Code (1); an alias pin not noted (1). Each fails on the intended assertion (`-m` present, the variable visible, no note).
- **Green `0bc6c87`:** targeted first (`cli_model_policy` 36, runner 8, `policy_matrix` 17), then `cargo test --no-fail-fast` from the repo root, **five consecutive runs, 512 passed and 0 failed each, exit 0, identical per-binary results**; a sixth run on the tree that became the docs commit is recorded in `M1-codex-closure-suite-output.txt`. 512 = 491 + 21.
- **New (21):** runner 5 (`--model`; ahead of `resume`; one model and no model in `-c`; Claude Code and none; `env_remove`). `cli_model_policy` 13 (Codex: no other way to pick a model, hostile environment, refusal of a worker and of a planner, split lists, edit after open, no policy; subagent variable from a pinned entry, from an excepted entry given a scout, kept for an excepted entry with no scout, kept without an enforced policy; alias noted; full id not noted). `policy_matrix` 3.
- **Existing tests:** all preserved, none skipped or deleted. Changed: `a_pinned_codex_entry_is_started_on_its_model` asserted `-m` and now asserts `--model` and the absence of `-m` (the requested change of the invocation); the fake CLI records one more variable. One of my own red-commit tests had a wrong premise (a scout in the lists makes the variable go) and became two; one count in the notes was wrong and is corrected (`policy_integrity_cli` 3 to 4, not 4 to 5).
- **Not built here:** `crates/app` (GPUI) and `crates/tauri-app` are not default workspace members. Tested crates: runner, core, cli, colony.
- **Working tree:** clean after the docs commit; branch pushed.
- **External validation:** none. No real model, CLI, provider or Jev call. Tokens not measured; frontier calls 0.
- **Separate issue, not fixed:** a chat reply can leave a thread busy for good (`M1-CHAT-BUSY-FLAG-ISSUE.md`): reproduced at about 3 in 12,000 trials on unmodified code; 99 of 100 with a delay injected between `spawn` and `insert`; 0 of 100 with the `running` lock held across both. The proposed fix needs your approval.

## E. Remaining risks

1. Codex was not run. `-m` equals `--model` by source and documentation, not by `--help`, and the source read is the main branch rather than the 0.166.0 tag.
2. A managed or organization layer may override `--model`: Claude Code's `availableModels` replaces it with a warning; Codex managed layers are undocumented for it. Backspace does not detect either.
3. A resumed Codex session may keep its saved model; the code that decides was not read.
4. The endpoint is not checked: an approved name can be served elsewhere (`ANTHROPIC_BASE_URL`, a Codex provider setting not read here).
5. A Claude Code alias names a family. Pins and the advisor and scout aliases are allowed and noted, not refused. Option for you: count an alias pin as unpinned.
6. Claude Code subagents started at invocation with their own model alias, and built-in subagents, are outside the lists. Option for you: switch the Agent tool off under an enforced policy.
7. A further option for you: read the model Claude Code reports at startup (its `system` init event carries one) and stop the turn on a mismatch. It would verify at run time instead of by documentation; it needs a decision on aliases and suffixes.
8. The policy restore guard is not an OS-level boundary; M6 is needed. `AgentChat` and `CloudChatServer` are unmetered and outside the policy.
9. Everything budget-related in the boundaries page still stands as written, with its assumptions (bytes per token, provider overhead, pricing, Codex usage semantics unverified).

**M1 recommendation:** ready for your decision. I recommend approving it if you accept items 1 to 6 as documented exceptions. Items 1 to 4 can only be closed by running the real tools; 5 to 7 are design choices that are yours.
