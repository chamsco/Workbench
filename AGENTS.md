# Working in this repository

Notes for coding agents (Claude Code, Cauldron's own agents, OpenCode, Codex).

## Tests

Before you implement a feature, fix a bug, refactor, or write or change a unit or integration
test, read [`crates/core/skills/sharp-testing/SKILL.md`](crates/core/skills/sharp-testing/SKILL.md)
and follow it. It is the house rule for tests:

- The default for a change is no new test. A test must name the plausible mistake it catches that
  no other check catches.
- Expected values come from the requirement, never from the code's own output.
- No assertions on labels, help text or other authored copy; no unit tests for rendering.
- A bug fix gets one regression test at the owning boundary, which fails before the fix.

Where `sharp-testing` and the `tdd` skill disagree on whether a test should exist,
`sharp-testing` decides. Once a test is justified, writing it first is fine.

Pull requests that touch tests are also reviewed against these rules in CI
(`.github/workflows/test-review.yml`). That review is advisory: it comments, it does not block.

## Other rules

- Stage files by explicit path. `crates/tauri-app/` is not part of this workspace.
- Every UI change ships with before/after screenshots.
