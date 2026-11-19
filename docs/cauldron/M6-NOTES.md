# M6 notes: execution engine and herdr adapter

PLAN §12: "Sandboxed worktrees. Executors respect allowlists. herdr behind a swappable adapter, version pinned." Sources: §5.5, §9.2, §11, §14 ("No sandbox in agent core: containerize execution, test isolation"), and the M5 decisions (M5-NOTES, 17 Nov).

Started 17 Nov 2026. Charles waived per-milestone review that morning ("I won't ask you for a mandatory review per gate for M1 to M13. I want a finished result, and I will test the finished result."), so M6 goes straight on from M5.

## What was built

### The sandbox (`crates/core/src/sandbox.rs`)

Every command an agent runs goes through `Workspace::sh`, and the harness now gives every workspace a jail: a sandbox plus a profile. That covers:
- the `bash` tool for the lead, workers, scouts and triage;
- ticket checks;
- the baseline capture;
- Rampart's vetting runs;
- the sealed-test, flake and mutation runs.

A coding-CLI worker's whole process is also started inside the sandbox (`Request.wrap` in the runner).

| Backend | Where | What it does |
|---|---|---|
| bubblewrap | Linux, when `bwrap` runs (user namespaces on) | New mount and PID namespaces. The system is read-only and `/tmp` is private. The project is read-only. The agent's worktree, git's shared directory and the home folder are writable. `.backspace/` is replaced by an empty folder, with the agent's worktree bound back in. Hidden paths are covered. Read-only files are bound over themselves. With `network = false` there is an empty network namespace. |
| Seatbelt | macOS (`sandbox-exec`) | The same rules as a profile. Writes are denied everywhere except the worktree, git's directory, the home folder, `/dev` and a private temp folder (`TMPDIR`). Reads and writes of hidden paths are denied. `(deny network*)` applies when the network is off. |
| low integrity | Windows | The command starts from a token lowered to the Low mandatory level, in a job object that is killed with it. A launcher does this: `backspace-jail`, or any Backspace binary given `__backspace_jail` first (Cauldron.exe included). Windows' mandatory integrity control then refuses every write to anything not labelled Low. Before each command, the worktree and a private temp folder are labelled Low. The policy, git's hooks, config and info, and the sealed tests are labelled Medium (read-only). Backspace's files (everything in `.backspace/` except the worktrees and Rampart's checkout), its data and the credential folders are labelled Medium no-read-up (hidden). |
| none | Linux without bubblewrap | Nothing isolates. Secrets are still taken out of the environment. |

The probe runs once per process, and it really starts the sandbox: being installed is not enough.

**Profiles:**
- **agent:**
  - Hidden: the project's `.backspace/` and Backspace's data folder (accounts, policy locks).
  - Also hidden: `~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.netrc`, `~/.git-credentials`, `~/.config/gh`, `~/.config/gcloud`, `~/.azure`, `~/.kube`, `~/.docker/config.json`, `~/.npmrc`, `~/.pypirc` and cargo credentials. Coding CLIs' logins are hidden too (`~/.claude`, `~/.claude.json`, `~/.codex`, `~/.config/opencode`, `~/.cursor`, `~/.grok`), as is anything in `sandbox.hide`.
  - Read-only: `backspace.toml` in the worktree and the root, and every sealed test in the worktree.
  - Read-only in git's shared directory: `hooks/`, `config` and `info/`. Otherwise an agent could plant a hook or a `core.hooksPath`/`fsmonitor` setting, and Backspace's own unsandboxed git would then run it.
- **rampart:**
  - The same, with the network off. This is M5 decision 2.
  - Rampart's own shell sees its checkout read-only. The vetting runs see it writable (tests may build) and also have no network.
- **cli:**
  - The CLI keeps its environment, which holds its login for the model the gateway approved, and the network that model is reached over.
  - Its binary's folder is visible even under `/tmp`.
  - Of Backspace's data, only the policy locks are hidden, because CLI logins can live in `accounts/`.

**Environment:** `is_secret_name` drops any variable whose name has a `KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `PASSWD`, `PASSPHRASE`, `CREDENTIAL(S)`, `AUTH`, `COOKIE` or `PAT` part, or contains `API_KEY`, `APIKEY`, `ACCESS_KEY` or `PRIVATE_KEY`. That includes Backspace's own bridge token. `sandbox.pass_env` lists exceptions. git's `GIT_CONFIG_COUNT/KEY_n/VALUE_n` are kept as a set and renumbered, less any setting carrying a credential. git refuses to run with a gap in them; the first full run showed that. `BACKSPACE_SANDBOX` tells the command which sandbox it is in.

**Modes** (`[sandbox] mode`):
- `auto` is the default.
- `required` refuses the command, and nothing runs. A CLI turn is refused too.
- `off` behaves as before M6.

The main log says which sandbox runs, for example "sandbox: bwrap; agents write only to their worktree…".

### Executors respect allowlists

M1 already checks every model in code before a call or a CLI launch. M6 closes the ways around that from inside an agent:
- No model's API key reaches an agent's shell. It cannot call a model the gateway did not approve with credentials from the environment.
- No CLI login reaches an agent's shell either: `~/.claude`, `~/.codex` and the rest are hidden.
- A CLI worker keeps only the login it was launched with, for the model on its command line.
- The policy file, the policy lock and Backspace's state are out of reach, so an agent cannot widen its own lists. `policy_guard` stays as the backstop for `mode = "off"` and for machines with no sandbox.

### herdr (`crates/core/src/herdr.rs`)

- `Supervisor` is the one interface. `Native` is the default. `Herdr` talks to herdr's socket: newline-delimited JSON, and only the documented `ping` and `agent.list`.
- **Pinned.** `[herdr] version = "0.9.5"` is the current documented release. The version is read from `herdr --version`. Any other version, a missing binary, or a socket that does not answer `ping` falls back to native, with a notice in the main log.
- **Never a gate.** `Harness::supervisor_signals()` returns herdr's idle/working/blocked/done states for a person to look at. Nothing in the lifecycle reads them.
- It is off by default.
- Windows uses a named pipe. The adapter does not support that yet: it falls back to native and says so.

### M5 decisions built here

1. Rampart prefers a model other than the lead's (`rampart::pick_cheap`). It falls back to the same model when that is the only cheap one allowed.
2. Rampart gets `bash` when the sandbox isolates. It has no network, and its checkout is read-only. Where nothing isolates, it keeps M5's read/list/submit only.

## Tests

- `crates/core/tests/sandbox.rs`: 20 tests. They run on Linux (bubblewrap), macOS (Seatbelt) and Windows (low integrity) in CI, with the sandbox required. Where a backend cannot do something (Windows: switch the network off, let agents commit), the test checks what it does instead.
- `crates/core/tests/herdr.rs`: 8 tests.
- A runner unit test: a wrapped CLI runs inside the sandbox command.
- A unit test of git's common directory.

Red (`0be05b6`): 25 of 27 new tests failed against stubs, and the 2 shipped-config guards passed. The full suite at red was 814 passed, 25 failed, all of them new tests.

The sandbox tests that need isolation skip where none runs, unless `BACKSPACE_SANDBOX_TESTS=require`. CI sets it: the Linux job installs bubblewrap, and a new macOS job runs the sandbox and herdr tests under Seatbelt.

Older tests changed:
- `rampart.rs`:
  - Rampart's tool list and the confinement test follow M5 decision 2.
  - The tamper test runs with the sandbox off. The sandbox now stops the bash rewrite at the source, so with it on, the delivery-time hash lock would have nothing to catch.
- Nine test files whose fake CLIs keep records beside themselves or in the project root run with `[sandbox] mode = "off"`, as do the policy-guard tests. Both kinds write where the sandbox now forbids. Each file says why.
- `e2e.rs`'s CLI worker test runs inside the sandbox unchanged.

## Limits

1. **Windows is weaker than Linux and macOS.**
   - It cannot take the network away. Rampart therefore gets no shell on Windows, and `network = false` is not enforced there.
   - git's store stays read-only to agents, so they cannot commit. Backspace commits their work, as it already did.
   - A read-only file (the policy, a sealed test) cannot be edited in place, but it can be deleted or replaced, because the folder it sits in is writable. The policy guard restores the policy file, a merge that edits it is held, and the hash lock refuses a delivery whose sealed tests changed. CI showed this.
   - The labels are written to disk and stay there:
     - Credential folders and Backspace's files keep a no-read-up label. That only affects low-integrity processes (browser sandboxes and the like). Your own programs, at Medium, are unaffected.
     - A file Backspace writes into `.backspace/` afresh after a command (rather than rewriting in place) is readable until the next command relabels it.
   - Git for Windows' bash is an MSYS program. While a normal-integrity MSYS process (a Git Bash window, or a test's own unjailed bash) is running, a low-integrity bash cannot open MSYS's shared objects and dies at start-up (`NtCreateDirectoryObject … 0xC0000022`, exit 0xC0000142). CI hit this intermittently (19 Nov). The launcher now retries for about 6 seconds, which is safe because nothing has run yet. If a Git Bash window stays open, the command still fails, with a message that says why.
   - The Windows sandbox has been tested only by CI on windows-latest, not on Charles's machine.
2. The home folder stays writable (caches, a CLI's own state). The rest of the system is readable. Only the listed credential paths are hidden.
3. The network is on by default. What keeps executors to the approved model is that no other credentials reach them, not a network allowlist. `network = false` blocks everything, the board bridge included. Rampart always runs with the network off.
4. The seatbelt profile was written for macOS and runs in CI on macos-14. It was not run on a Mac by hand.
5. herdr was tested against a stand-in that speaks its documented protocol. The real 0.9.5 binary could not be downloaded here (herdr.dev and github.com are blocked by this environment's network policy).
6. The research and one-shot paths still refuse a coding-CLI planner. A read-only CLI profile could lift that later.
7. bubblewrap on Ubuntu 24.04 needs `kernel.apparmor_restrict_unprivileged_userns=0`, or an AppArmor profile for bwrap. Without it, `auto` falls back to no isolation and says so in the log.
8. Mock models only. Tokens: not measured. Real frontier calls: 0.
