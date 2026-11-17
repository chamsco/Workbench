# M5 notes: Builder and Rampart

PLAN §12: "Two separated orchestrators. Tests sealed and hash-locked. Ramparts fail on baseline, flake check, mutation score. Escalation triggers per 7.4." Sources: §6 (lifecycle), §7.1 to §7.4, §3.1 and §3.4 (cost ladder), `PLANNING-CONTRACT.md` (spec.md is the only input).

Started 16 Nov 2026 on Charles's go ("Open the PR and start M5"), which waived the earlier order (router eval reviewed before M5). Intake stays off by default.

## What was built

`crates/core/src/rampart.rs` (new), wired into `harness.rs`.

- **Two orchestrators.** Builder is the lead and its workers, as before. Rampart is a new agent kind (`AgentKind::Rampart`), counted as an orchestrator by the gateway.
  - On the long path, the lead's first `work_tickets` starts Rampart and is refused until a suite is sealed. The lead can still plan and make tickets meanwhile.
  - Rampart's first message is `rampart::brief`: the approved spec.md (hash checked against the user's yes) and baseline.json. Nothing else: no request, no tickets, no lead conversation.
  - Its directory is its own checkout of the baseline at `.backspace/rampart/tree`: a detached worktree at the lead's branch as committed when Rampart starts. A shared (non-git) project gets a copy instead.
  - In code, Rampart may call `read`, `list` and `submit_ramparts` only. Anything else is refused in `run_tool`, whatever it was offered. It has no bash.
  - It starts below the frontier (`cheap_rung`), after the usual start caps.
  - Builder never receives Rampart's brief, tool results or reasoning. Workers get the sealed test files (in their branch) and a list of path, criteria and command.
- **Validation before sealing** (`submit_ramparts`):
  1. Shape and traceability (`shape_gaps`): paths stay in the project and out of `.backspace`/`.git`, no duplicates, no empty test or command, every named criterion exists, and every criterion A1… of spec.md has a test.
  2. Every test is run `runs` times (3) on the baseline tree (`vet`). It must fail every time. A test that passes is refused ("proves nothing"). Mixed results are refused as flaky. The tree is put back after.
  3. A refusal goes back to Rampart with the output, and it submits again.
- **Human spot-check.** The first `spot_checks` suites (3, counted per project) are put to the user with what ran. Only a plain yes seals the suite. Anything else starts a new Rampart with the user's words. After that, suites are sealed at once and the lead is told through its inbox.
- **Seal** (`seal`, `write`, `place`):
  - `.backspace/rampart/seal.json` holds a SHA-256 per file, the suite hash, the spec hash, the base commit, Rampart's model and the vetting runs.
  - The seal is also kept in `ProjectState.rampart`, and there are copies under `.backspace/rampart/sealed/`.
  - The files are committed onto the lead's branch, only those paths, so every worker forks with them.
- **Hash lock.**
  - Any agent's `write`/`edit` of a sealed path is refused.
  - Every worker delivery first re-hashes every sealed file in its worktree (`verify`). A changed or missing file refuses the delivery, naming it, and counts as a failed attempt.
- **Which tickets are held.** Only the long task's own tickets are held to the seal: the ones the lead planned from spec.md (`ProjectState.rampart.tickets`). A task started alongside with `start_task` is neither held nor gated. The first full run caught this: M3's `no_agent_is_offered_a_way_to_start_a_task` timed out, because the side task's dispatch was refused.
- **Builder against the seal** (`sealed_gate`, in `submit_as` before the ticket check):
  - The tests run are the ones for the criteria the ticket `covers`. A ticket that names none runs the whole suite.
  - Each runs `runs` times. A failure on a later run is reported as flaky.
  - On a pass, the change is mutated and the result recorded as `Deliverable.mutation`.
- **Mutation score.**
  - `changed_lines` comes from `git diff --no-index -U0` of each changed file against Rampart's tree.
  - `mutants` makes one-site textual changes on those lines: `==`/`!=`, `<=`/`>`, `>=`/`<`, `&&`/`||`, `+`/`-`, `true`/`false`, and an integer literal n becomes n+1. Comments are skipped, and at most `max_mutants` (12) are made.
  - `mutation_score` runs the scoped sealed tests on each mutant. Score = killed / total, with the survivors listed. Every file is put back.
  - It is recorded, not a gate.
- **§7.4 limits** (`Limit::record`, per ticket in `ProjectState.rampart.limits`). After each failed attempt, the first trigger that holds wins:
  1. Stagnation: the same failure signature twice in a row. The signature is a hash of the failing commands and the last 40 output lines, with numbers, timings and hex ids taken out.
  2. N failures (`max_failures`, 3).
  3. The cheap budget spent (`cheap_budget_usd`, off by default).
  4. Another cheap retry is expected to cost more than the frontier. This uses `ladder::escalate_sooner` with this ticket's measured attempts against an unmeasured frontier (p = 0.5) and an estimated frontier call: the conversation size plus max output, at the cheapest allowed frontier model's prices.
- **One frontier call.**
  - On a trigger, `sealed_escalate` moves the worker to the cheapest frontier model its role may use and sends one message: "ESCALATED ONCE …" plus the failure digest (`digest`: goal, trigger, sealed tests, every attempt's signature, the last failure's output, capped at 6000 chars).
  - The next failure, or a trigger with no frontier model allowed, hands the ticket to the user. The worker stops with the digest, the lead's log gets it as a message, and the ticket is failed. No frontier loops.
  - Under a seal, the old generic escalation (effort, then a stronger model) can no longer reach a frontier model. The frontier is reached only through §7.4.
- Config: `[rampart]` with `runs`, `spot_checks`, `max_failures`, `cheap_budget_usd`, `max_mutants`, in `default-config.toml`.

## Tests

`crates/core/tests/rampart.rs`: 30 tests. 28 were written first (red `f2333ca`: 27 failed, and 1 guard passed, the shipped-config one). Two were added while going green, for ticket scoping:
- a pure test of `scoped`;
- an end-to-end test. With scoping disabled it timed out red (`M5-scope-red-output.txt`).

The end-to-end tests use a mock OpenAI-compatible server playing the lead, Rampart and the worker, and a real git workspace with real shell tests. They cover:
- the separation;
- the refusal of a baseline-passing test;
- the yes;
- auto-seal after the spot-checks;
- Rampart's tool confinement;
- the tamper refusal;
- 3 failures leading to one frontier call with the digest, then a person;
- stagnation leading to the frontier, which finishes;
- the mutation score on a delivered change.

## Limits and open choices

1. Like M4, it runs only when intake picks the long path, and intake is still off by default.
2. Rampart's separation holds for prompts, file tools and its tool list. Builder's `bash` is not sandboxed until M6, so a worker could still `cat` `.backspace/` (seal.json, state.json with Rampart's log). Rampart itself has no bash.
3. "Not the same model instance" is read as a separate agent and conversation. Rampart and Builder may run the same model id, and a one-model project would otherwise stall. **Open: require a different model?**
4. Rampart cannot run anything itself. It learns only from the vetting output after each submission. On a real repository it has to guess the build and test commands from reading. **Open:** give it a sandboxed shell in M6.
5. Rampart's tree is the lead's branch as committed when it starts. Uncommitted edits by the lead are not in it. A shared-mode copy does include them.
6. Ticket scoping means a plan split across tickets is never held to the whole suite at once. Running the whole suite before merge is Gate A, which is M7. A ticket with no `covers` is held to everything.
7. The mutation score is recorded, not enforced. There is no threshold yet: Gates A and B are M7 and M8. Mutants and the 3× flake runs execute unsandboxed in the worker's worktree, one after another. A slow suite costs `runs` × (1 + mutants) suite runs per delivery, bounded by `max_mutants`.
8. The mutation operators are textual and language-agnostic. They do not parse code, so a mutant can be a compile error: that counts as killed, which flatters the score.
9. A coding-CLI worker cannot switch model mid-session, so for it a §7.4 trigger goes straight to the user with the digest. There is no frontier call.
10. The §7.3 dispute path (third-model arbitration) is not built. It is not in M5's acceptance line.
11. The spot-check yes is a chat reply. There is no button.
12. Mock models only. Tokens: not measured. Real frontier calls: 0.

## Decisions on the open choices (17 Nov 2026)

Charles waived per-milestone review (17 Nov, 08:02Z) and asked for a finished app to test. These are Claude's picks, made so later milestones can build on them. Any of them can be overturned at his test.

1. **Rampart's model: prefer a different model id, don't require it.** When the project allows more than one model at Rampart's rung, Rampart takes one that differs from the lead's. With only one allowed, it runs the same id as a separate agent, as now. Requiring a different id would stall one-model and local-only projects, and the independence that matters most (no shared context) already holds. To build in M6.
2. **Rampart gets a sandboxed shell in M6.** Read-only on its own tree, no network, and only for running build and test commands. Today it guesses commands from reading, and each wrong guess costs a full vetting round. It comes with the M6 sandbox, not before, so it never runs unsandboxed.
3. **Mutation score: no threshold until Gate A (M7), then a soft one.** Gate A flags a delivery whose score is below 0.5 with at least 4 mutants, and lists the survivors for the lead to answer. It does not block. Textual mutants that fail to compile count as killed, so the score runs high and is not reliable enough to block on. Blocking is reconsidered at Gate B (M8) if mutants become syntax-aware.
