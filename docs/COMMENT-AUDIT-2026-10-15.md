# Comment audit: what to delete, what to refactor

Run 15 Oct 2026 on the working tree at `3e63468`, in poteto-mode, as a swarm of
comment-sicko reviewers (one per slice of the tree, 11 slices, every file read).
The ruleset is the pstack `comment-sicko` leash, not a matter of taste: a comment
survives only for a legal header, behavior forced by a dependency/platform/protocol
we cannot reshape, `// prettier-ignore`, a style-only lint suppression, a public API
contract, or a constraint link.

## Totals

| Verdict | Count | Meaning |
|---|---:|---|
| DELETE | 1718 | the comment goes, code untouched |
| REFACTOR | 57 | the comment can only go after a shape changes |
| KEEP | 333 | proven against the leash |
| UNCERTAIN | 43 | only a live run can settle it |

Per slice (DELETE / REFACTOR / KEEP / UNCERTAIN):

| Slice | Files | D | R | K | U |
|---|---|---:|---:|---:|---:|
| core harness, router, project, ticket | 4 | 169 | 4 | 21 | 2 |
| local models, accounts | 2 | 115 | 4 | 5 | 3 |
| colony | 7 | 31 | 2 | 37 | 4 |
| Tauri shell, CLI, config, prefs | 4 | 67 | 6 | 3 | 3 |
| chat, memory, agents, apps | 6 | 231 | 3 | 7 | 5 |
| fleet, remote, companion, mcp, board, options | 6 | 49 | 4 | 57 | 0 |
| small core modules | 13 | 70 | 3 | 38 | 3 |
| runner + frozen GPUI shell | 9 | 115 | 4 | 39 | 2 |
| tests | 8 | 78 | 1 | 23 | 1 |
| UI (vanilla `ui/`) | 13 | 168 | 3 | 8 | 0 |
| React chat face (`web/src`) | 6 | 43 | 1 | 23 | 0 |

Method note: the two swarm runs returned the tables above. Every line number and
quote below was re-read in the file during aggregation, and the eight items in
"Holes the audit found" were each confirmed by reading the code.

## 1. Straight up delete (1718)

Five patterns, in order of volume. None needs code to change.

1. **Doc comments that restate the item beneath them** (~700). The `///` walls over
   `ProjectState`/`AgentRecord` fields and enum variants
   (`crates/core/src/project.rs:63,75,111,113,117,122,144,155,170`), the `Inner`
   field docs in `harness.rs`, and every `/// One line...` on a private helper.
   They are the single largest category and the safest deletion: the field name and
   type already say it.

2. **Module headers that narrate design history** (~250). `crates/core/src/chat.rs`,
   `memory.rs`, `memory_dream.rs`, `memory_learn.rs`, `agents.rs`, `apps.rs`,
   `model_server.rs`, `accounts.rs`, `diagram.rs`, and the dated test header at
   `crates/core/tests/features.rs:1` ("The features added on 14 Oct 2026"). History
   belongs in commits. Keep the exceptions in §3.

3. **Banner dividers** (~120). `// ---- agents`, `// ---- store`, `// ---- machines`
   and friends. `crates/tauri-app/src/main.rs:195` plus 16 more in that file alone;
   5 in `harness.rs`; 8 in `chat`/`memory`; 12 in the GPUI shell; 9 in the UI and
   `web/src`. Zero information.

4. **Narration of the next statement** (~350). Includes the numbered phase comments in
   `harness.rs:2352,2373,2395,2424` ("// 1. Snapshot the work..."), which the leash
   names explicitly, and `crates/tauri-app/ui/picker.js:80,86,99`.

5. **Scent markers** (~120). `// ponytail:` appears 7 times
   (`harness.rs:1643`, `accounts.rs:25`, `chat.rs:363`, `fleet.rs:431`,
   `harnesses.rs:499`, `prefs.rs:298`, `rpanel.js:32`); plus "for now", "which is
   fine here" (`tests/webhook.rs:52`), and `audit P0-2` / `audit P0-3` prose
   citations that are not links (`harness.rs:282,311`, `ui/app.js:903`,
   `tests/interrupted.rs:1-4`, `tests/webhook.rs:89-91`).

Three of these are stale or actively false, so they are worth deleting first
because they cost a reader real time:

- `crates/core/src/harness.rs:2732` is a truncated orphan: "Explicit BoxFuture
  return type breaks the drive → work_tickets →" with no continuation, directly
  above `run_worker_with`. Verified in the file.
- `crates/tauri-app/ui/home.js:137` names "the canopies view", which does not
  exist; `WB` lists home/canvases/wall and the code below sets `wbView: "canvases"`.
- `crates/core/src/harness.rs:799` says "Record of the run, not a resumable
  checkpoint", which the resume path itself contradicts.

## 2. Straight up delete, with the code (dead weight)

- `crates/colony/src/fake.rs:169-173` `sample_spec` is a one-line pass-through to
  `CreateSpec::new`; tests can call that directly.
- `crates/core/src/memory.rs:334` migrates `<data>/memory.json` into the git-repo
  format on every `open`. No install since the repo format landed has that file.
- `crates/core/tests/e2e.rs:603` and `:514` claim merge propagation the test never
  asserts (it reads `_run/leaf.txt` only). Either assert it or drop the claim.

## 3. Refactor rather than delete (57) - ranked by payoff

These are the items where the comment is a symptom. Reshaping the code is what
removes the comment and the hazard under it. Ordered by what I would fix first.

**1. `companion.rs:87` - the comment denies a credential the code accepts.** Highest
priority, and a correctness issue rather than a style one. The comment says a secret
in a URL is not a credential, then lines 95-99 accept `?token=<token>` and line 139
grants full API access on it. So the phone API is reachable with a URL-borne token,
which is the thing the comment says must not happen. Fix: accept the query token
only inside the `/hook/` branch (where a sender may not be able to set a header) and
make the general API bearer-only. Comment goes; the property becomes true.

**2. `provider.rs:65-72` - `leak()` exists to satisfy our own type.** `ToolDef`
declares `name`/`description` as `&'static str`, so a tool set built at runtime
(Colony's, from another crate) is `Box::leak`ed to fit. Change `ToolDef` to carry
`Cow<'static, str>` (or `String`) and delete `leak` and its justification. The
comment exists only because the type is wrong.

**3. `crates/tauri-app/ui/chat.js:153` - two chat implementations alive at once.**
Every entry point (`render`, `renderThreads`, `renderHead`, `renderMsgs`,
`renderComposer`, `refresh`, `open`, `newChat`) forks on `whirl`, so the vanilla
bubble/markdown/composer stack is a permanent second renderer. Pick the Whirl face
and delete the fork, or keep the vanilla face and drop the bundle. This is README's
own "two chat renderers" duplication made concrete.

**4. `crates/tauri-app/ui/app.js:136` - the layout grammar is written twice.**
`core::layout` owns the split tree and `app.js` re-derives validity, ratios and leaf
renumbering in `L`, which is why `L.valid`/`tree()` exist at all. Our own crate is
not on the leash, so this is a genuine refactor: one owner for the grammar.

**5. `prefs.rs:298` - "ponytail: the command splits on spaces".** `Connection::server`
splits an MCP command line on whitespace, so any quoted argument is silently
mangled. Parse the command line properly; the comment then has nothing to excuse.

**6. `cloud.rs:573` - fake streaming.** The SSE endpoint takes a completed answer and
chops it into 24-byte pieces. Stream deltas from the provider call. This is a lie to
the client, not a shortcut worth documenting.

**7. `harness.rs:1827,2750` + `project.rs` - resume eligibility is sniffed from log
text.** `resume` gates on `AgentStatus::Idle` *plus*
`log.iter().any(|l| l.text.starts_with("interrupted:"))`, and `run_worker_with`
reuses one path for fresh and resumed runs via `interrupted && path.is_dir()`. The
logging text is load-bearing state. Give the agent a typed marker (or make
`Interrupted` survive the load) and the two apologetic comments go.

**8. `model_server.rs:694` - a string is a control signal.** `get_json` returns the
literal `"not running"` and `not_running` re-compares it. A typed error variant
removes the sentinel and the paragraph explaining it.

**9. `model_server.rs:483` - a positional 4-tuple documented in prose.** "(runtime
id, name, base_url, api key)" is exactly what a named struct is for.

**10. `router.rs:261` - `let mut level: usize = 1; // medium`.** Name the ladder
position (`Effort::Medium.index()` or a table) and the comment dies.

**11. `tests/webhook.rs:118,120-131` - the helper discards the response body**, so
the test re-implements the request just to read it, and the comment explains the
duplication. Return `(status, body)` from `http_with`.

**12. `crates/colony/src/model.rs:163-175` - a hand-filled derived field.**
`Capabilities::providers` is populated only when visor-core omits it. Make it a
method over the vendor fields.

**13. `crates/colony/src/fake.rs:69` - a poisoned-mutex sentinel licenses swallowing
a test panic**, so a torn `FakeBackend` lets later assertions run on corrupt state.
Fix the locking; the comment goes with it.

**14. `accounts.rs:435` - the test helper exists because `order()` reads the clock
itself.** Take `now` like `best`/`ranked` do and the comment disappears.

**15. `prefs.rs:229` - `bind_addr()` silently rewrites a stored non-loopback address.**
Migration shims belong in a load-time migration, not in the accessor.

**16. `tauri-app/src/main.rs:974` - `TERM=dumb`** downgrades the terminal so the
canvas does not have to render ANSI. Either do the ANSI path or stop claiming a
terminal.

**17. `tauri-app/src/main.rs:1742` + the PTY test** currently records and never
asserts, so a broken PTY ships green. (This one I already know from own work: the
test asserts spawn + drain-or-report, and the host's silence is documented. The
comment can shrink to that.)

**18. `tauri-app/src/main.rs:256` and `app/src/main.rs:663`** both veto a production
prefs write when a bench env var is set. Point the bench at its own
`BACKSPACE_PREFS` and the guard plus comment go.

**19. `board.rs:40` - `BoardMsg.thread` is derivable** from `from`/`to`/`reply_to`,
and every reader falls back to `thread_of(...)` anyway. Keep one representation.

**20. `chat.rs:1412` - the event contract is silently partial**, because `run`
rewrites `Final` into `Text`. Fold the conversion into the handler.

**21. `memory_dream.rs:406` - `let _ = a.scope;`** discards a field the model
contract still sends. Delete the field from the shape.

**22. `companion.rs:166` bypasses `phone_may_read`** and re-inlines the privacy rule,
so the rule lives twice and `phone_may_read` looks unused. Call it.

**23. `remote.rs:179` - `same()` is described as equal-length and branch-free**, but
it short-circuits on length. Either make the claim true or state the real property.

**24. `web/src/app/ChatApp.tsx:61`** carries a duplicate `eslint-disable-next-line`
that lands on a comment line and suppresses nothing. Delete the duplicate.

**25. `web/src/app/Composer.tsx:7,283`** feeds Whirl's `onQueue` a no-op. Drop the
prop rather than explaining the dropped feature.

## 4. Keep (333), with the clause that protects them

- **Vendor/platform facts code cannot express** - Ollama's `details.parameter_size`
  format and `/api/ps` behavior (`model_server.rs:171,613`), the `llama3.2:3b` family
  naming trap (`model_server.rs:138`), the `HTTP_PROXY` bypass for loopback
  (`model_server.rs:702`), Windows `.exe`/`.cmd` shim resolution
  (`harnesses.rs:197`), the Windows `bash` search order (`tools.rs:58`),
  WebKitGTK's DMA-BUF flag (`tauri-app/src/main.rs:1524`), macOS Dock launching
  without a shell environment (`prefs.rs:60`), tokio's stored `Notify` permit
  (`harness.rs:659`), dropping a runtime off its own runtime (`fleet.rs:524`).
- **Wire and cross-crate contracts** - the tri-state `picked` encoding
  (`options.rs:9`), the OpenTelemetry `gen_ai.*` attribute names
  (`trace.rs:3`), the Agent Memory Repo on-disk layout (`memory.rs:5`), the app
  manifest permission strings (`apps.rs:49`), the colony JSON-CLI shape
  (`colony/backend.rs:35,52`) and its search order, the canvas log protocol
  (`project.rs:177`), the MCP stdio contract, the runner's public API docs.
- **Legal headers** - the MIT attribution in `skills.rs:2`, in the vendored
  `skills/ponytail/`, and the Whirl-derived `web/src/app/*.tsx` headers.
- **Style-only suppressions** - `#[allow(clippy::too_many_arguments)]` at
  `harness.rs:1669` stays. Anything protecting correctness does not (see the
  `catch { /* the pane is closing */ }` at `ui/app.js:1424` and the eslint note
  above, both of which hide real signals).

## 5. Holes this audit found (not comment problems)

1. **`crates/core/tests/resume.rs` did not compile on unix.** `Ctx`, `Prefs` and
   `Status` were used at lines 67/71/78 with no import, because a previous cleanup
   removed them as "unused" on Windows, where the whole test is `#[cfg(unix)]`.
   Verified by reading the file. Fixed in this session: the imports and the three
   unix-only helpers are gated on `#[cfg(unix)]`, so Windows stays warning-free and
   unix compiles again. `cargo test --workspace --exclude backspace` is green (139
   tests).
2. `crates/core/src/companion.rs:95-99,139` - the query-token grant, per §3.1.
3. `cloud.rs:573` - a non-streaming call dressed as a stream, per §3.6.
4. `ui/chat.js` - a second chat renderer kept alive, per §3.3.
5. `app.js:136` - the layout grammar implemented twice, per §3.4.
6. `prefs.rs:299-305` - MCP command lines mangled by whitespace splitting.
7. `tests/e2e.rs:514,603` - comments claiming coverage the assertions do not have.
8. `ui/home.js:137` and `harness.rs:2732` - stale or truncated text that actively
   misleads (both in §1).

## What to do with this

Sequence I would use, since the first three are worth the whole audit on their own:

1. §3.1 the query-token grant (security, small diff).
2. §5.1 already fixed; land it with a test run on unix and Windows.
3. §3.2 `ToolDef` to `Cow`, which deletes `leak` and its justification.
4. §5.7, §1's three stale comments, §2's dead weight (minutes each, no design).
5. The bulk DELETE in five mechanical passes, one per pattern from §1, each its own
   commit so a reviewer can see only deletions. Start with the file that has the most
   (chat-memory, 231) and the UI (168).
6. §3.3, §3.4 and §3.6 as real refactors, each with its own plan.

I did not do any of this. Nothing in the repo was edited except the `resume.rs`
compile fix, and no comment was deleted.
