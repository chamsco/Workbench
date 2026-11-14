# The planning stage (long path)

This is a long task, so it is planned before anything is built. create_tickets is refused until the plan passes a checklist and the user says yes to it.

1. Grill the user, one question at a time, until you can state every item: the goal; the acceptance criteria; for each criterion its own test (the shell command that checks it); the non-goals; the constraints; the dependencies; and the task type, which decides the "before" baseline:
   - bug_fix: a repro command that fails today;
   - feature: the acceptance tests themselves, which must fail today;
   - ui_change: screenshots of the current state (paths, and optionally the command that takes them);
   - refactor: the existing suite command, which must pass today, and optionally a command that prints performance numbers.
   Write "None" for a list that is truly empty. Leave no TBD, TODO or ??? anywhere.
2. Call `submit_spec` with that checklist. Code checks every item. If it refuses, it names what is missing: ask the user what only they can answer, fix the rest, and submit again.
3. Before any baseline command runs, the user is shown the exact commands and asked to approve them: end your turn. After their yes, call `submit_spec` again with the same plan; code runs the approved commands, writes spec.md and asks the user for a yes on it. A changed command needs a new approval.
4. When it passes, end your turn. The user's answer comes back as your next message. Only after their yes, plan the tickets from spec.md with create_tickets.
