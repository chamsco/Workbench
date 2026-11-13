# M3 sign-off

Charles, 13 Nov 2026, 16:05Z:

> The 591-test evidence and three consecutive green runs are sufficient for the declared M3 scope.

**M3 is accepted** for the scope in `M3-REPORT.md` and its follow-ups (`M3-FOLLOWUP-REPORT.md`, `M3-FOLLOWUP3-REPORT.md`, `M3-STARTAUTH-REPORT.md`, `M3-CLOSURE-REPORT.md`).

## Accepted limitations

- Research and one-shot are native/API paths. Their coding-CLI versions remain unavailable until M6.
- Bash remains unsandboxed (until M6).
- The filesystem checks are not race-proof.

## start_task

`start_task` remains an authenticated entry point for an existing ticket. It does not replace new-task intake.

The evidence establishes token authentication, and that agents cannot call it. It does not establish comprehensive cross-project authorization.

**Security follow-up, before shared multi-user workspaces are exposed:** prove cross-project authorization for `start_task` (a token for one project starts nothing in another, and the same for every entry point that takes a ticket).

## Status of the other milestones

- M0 is closed (30 Oct).
- M1 is not yet approved.
- M2 is not yet approved: the §10 eval on real traces is missing.
- M4 to M13 have not started.
