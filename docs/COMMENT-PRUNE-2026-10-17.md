# Comment prune: what goes, and what has to change shape first

Run 17 Oct 2026 on the working tree at `b25a509`, as a swarm: 13 auditors (one
per slice, every cited line read) followed by 13 adversarial verifiers that
re-opened each line, ruled on it, and wrote the slice out. This is the pstack
shape the owner asked for (`/poteto-mode` → `/swarm` → `/no-comments`), run
with this session's tooling. Raw slice files: `docs/comment-prune/*.md`.

A comment survives only if it states a non-obvious WHY the code cannot show.
Everything else goes.

## Totals

| Verdict | Count | Meaning |
|---|---:|---|
| DELETE | 233 | the comment goes, no code changes |
| REFACTOR | 26 | the comment is load-bearing because the code is a workaround |
| KEEP | 23 | proven against something this repo cannot change |
| rejected | 3 | a verifier could not confirm the citation |

Per slice:

| Slice | DELETE | REFACTOR | KEEP |
|---|---:|---:|---:|
| `crates/core/src/harness.rs`, `fleet.rs` | 16 | 5 | 1 |
| `chat.rs`, `agents.rs`, `board.rs`, `ticket.rs`, `project.rs` | 19 | 2 | 0 |
| `memory*.rs`, `skills.rs`, `trace.rs`, `diagram.rs`, `artifact.rs` | 16 | 1 | 4 |
| `harnesses.rs`, `model_server.rs`, `router.rs`, `provider.rs`, `accounts.rs`, `options.rs` | 17 | 2 | 2 |
| `prefs.rs`, `cloud.rs`, `companion.rs`, `remote.rs`, `config.rs`, `apps.rs`, `mcp.rs`, `git.rs`, `files.rs`, `update.rs` | 16 | 3 | 0 |
| `crates/core/tests/*` | 18 | 2 | 2 |
| `crates/tauri-app/src/main.rs`, `crates/cli` | 21 | 0 | 1 |
| `crates/tauri-app/ui/*.js`, `index.html` | 22 | 2 | 0 |
| `crates/tauri-app/ui/*.css` | 18 | 3 | 1 |
| `crates/tauri-app/web/src/app/*`, `bridge.ts`, `main.tsx` | 15 | 2 | 5 |
| `crates/app/src/*` (GPUI) | 18 | 1 | 3 |
| `crates/colony`, `crates/runner`, `capsule/` | 19 | 1 | 2 |
| `tools/`, `apps/sdk.js` | 18 | 2 | 2 |

**Read the numbers as a floor, not a census.** Each auditor was capped at 22
findings, so every slice that reported a full 22 has more behind it. The 15 Oct
audit (`COMMENT-AUDIT-2026-10-15.md`) was a census of the same tree and found
1718 DELETE / 57 REFACTOR / 333 KEEP. This run is the ranked head of that
distribution, with fresh line numbers and an adversarial pass, and it is the
list to work from first.

## Where the bulk sits

By kind, among the 233 deletes: `restates-code` 103, `section-banner` 78,
`obvious` 19, `history-narration` 18, `self-praise` 5, `tombstone` 3,
`test-narration` 3.

The files with the most deletes, which is also where a sweep pays first:
`crates/tauri-app/src/main.rs` 20, `crates/core/src/chat.rs` 14,
`crates/core/src/harness.rs` 13, `crates/app/src/main.rs` 9,
`crates/core/src/memory.rs` 8, `crates/tauri-app/ui/chat.css` 8,
`crates/core/tests/features.rs` 7, `crates/tauri-app/ui/shell.js` 7.

## 1. Straight up delete (233)

Nothing here needs code to change. Five patterns, in order of volume.

**a. Restatements (103).** The comment says what the next statement plainly
says. `crates/core/src/harness.rs:2007` ("Messages from other agents join this
turn's input") over the code that builds exactly that string;
`crates/core/src/project.rs:33` ("Is the agent still doing something?") on
`is_active`. The field name, the type and the body already answer.

**b. Banner dividers (78).** `// ---- agents`, `// ---- store`, `// ---- Cloud`
and friends. `crates/tauri-app/src/main.rs` holds 20 of the 233 on its own.
A grep for `^// -` deletes them in one pass with no reader losing anything.

**c. Obvious (19) and self-praise (5).** `/// How many variants one set may
hold.` on `MAX_VARIANTS`. Prose that argues the design is good rather than
saying how to use it.

**d. History narration (18).** What the code used to be, or a migration story
told at the wrong layer. `tools/verify/sidebar-tree.js:65` ("even where the
vanilla list used to be"), `crates/core/src/router.rs:183` (a past-tense bug
story whose mechanism is already stated 60 lines above). History belongs in
commits.

**e. Tombstones and test narration (6).** Deprecated-comment leftovers and test
fixtures narrating what the fixture does (`crates/core/src/memory.rs:889`
"Reopened from disk." directly above `Memory::open(dir.clone())`).

**Two rules for the sweep.**

1. Some deletions are coupled. The numbered steps in `harness.rs` are four
   comments and one orphan: deleting 2412, 2455 and 2484 without 2433 leaves a
   lone `// 2.` behind. The slice files name these couplings in the `why` cell.
2. A comment that is *false* beats a comment that is merely redundant. Land
   those first, because they cost a reader real time. `crates/core/src/chat.rs:286`
   documents bare-name `@` matching that `named_in` never does;
   `crates/core/src/router.rs:128` says every delegated small job goes through
   `route_for`, which `local_subagent` does not do.
   `crates/core/src/harness.rs:2792` is a truncated compiler note with the rest
   of the sentence missing.

## 2. Refactor first (26)

These are comments whose reason to exist is a shape. Fix the shape and the
comment goes with it, taking the hazard underneath. Ordered by what I would
take first.

**Security and exposure (3).** These are the ones where the comment is the only
thing standing between the reader and a real hole.

- `crates/core/src/fleet.rs:405` + `crates/core/src/prefs.rs:218`. The loopback
  invariant is documented where it is used instead of where it is decided.
  `addr` and `allow_lan` are two independent fields for one decision, so
  `allow_lan: false` with a LAN address is representable and only `bind_addr`
  resolves it. One typed choice (`Listen::Loopback { port }` or
  `Listen::Lan(addr)`) makes an exposed bind unrepresentable, and both comments
  go. The 15 Oct audit found the sharper version of this at
  `companion.rs:87`, where a comment says a URL token is not a credential and
  the code accepts one anyway; that is a code bug, not a comment bug, and it is
  still open.
- `crates/core/src/companion.rs:63` and `crates/core/tests/webhook.rs:89`. The
  rule "an app chat, a named-agent thread or a group is not the phone's
  contract" is argued twice by citing a past audit ticket. Both want the same
  fix: a named predicate (`ThreadInfo::phone_visible()`, `Scope::phone_visible()`)
  that `phone_may_read` and the listing call, plus one line saying why agent
  threads are excluded.

**A workaround that should be a type (5).**

- `crates/core/src/prefs.rs:304`, MCP command lines split on whitespace, so a
  quoted argument is silently mangled. Parse properly, or take
  `{ command, args, env }`.
- `crates/core/src/memory_dream.rs:406`, `let _ = a.scope;` thrown away to
  excuse a field the prompt never asks for. Delete `Add::scope` and the
  apology. (The 15 Oct audit reached the same conclusion independently.)
- `crates/core/src/harnesses.rs:499`, "first terminal found wins" as a fixed
  four-name list. Add `prefs.terminal`, resolve it first, fall back to PATH.
- `crates/tauri-app/ui/picker.js:165`, `const PLANS = ["claude", "codex"]` with
  a caption. Rename to `PLANNER_CLIS`, or have the harness scan report planner
  capability, which `harness.rs` already owns.
- `crates/app/src/canvas.rs:720`, inline `360. + 28. + 2. * f32::from(w)` with a
  box-model note. One named width constant with that note as its doc.

**Magic numbers that never got a home (4).** Same shape, different subject.
`crates/core/src/fleet.rs:431` (a 16k message cap) and
`crates/core/src/harness.rs:1684` (the CLI stall limit) both want to be read
from prefs or env, the way the neighbouring limits already are.
`crates/tauri-app/ui/rpanel.js:32` throttles git status to one every 3s to
absorb a refresh fired on every snapshot; coalesce the refresh at its source
and the throttle and its comment both disappear.

**Correct the code, not the doc (3).** `crates/core/src/chat.rs:286` (documented
`@Name` contract versus a substring match), `crates/core/src/harness.rs:1215`
("always WSL" is already false at line 1218), and `crates/core/src/router.rs:142`
(plural and singular job keys tolerated by `trim_end_matches('s')`). One
spelling, one enum, one implementation of the contract the doc claims.

**Test doubles that force weak assertions (2).**
`crates/core/tests/features.rs:553`, the stand-in counts a bare TCP connect as
a request, so the assertion had to weaken to `any(...)`. Record only well-formed
chat requests and assert the model's own name. The same note is duplicated at
680-682.

**Front-end shape (4).**

- `crates/tauri-app/web/src/app/ThreadView.tsx:102`, an `epoch` prop exists only
  to force re-renders; it is never read in the body. Subscribe to the bridge
  store, or pass the resolved data down, and both the prop and its comment go.
- `crates/tauri-app/web/src/app/RichText.tsx:182`, text is rewritten to inject
  fences, so an already-fenced block has to be special-cased. Recognise bare
  `root = Card(...)` at the block level instead of rewriting a string.
- `tools/uiharness/preload.js:27` and `:55`, the fixture's comments explain that
  `snap.approvals` and `prefs.tabs` are read unguarded. Normalise at that
  boundary in `app.js` (one snapshot merge, one prefs loader that reinstalls the
  accessors) and the fixture carries values only.

**Two trims, not refactors (3).** `crates/tauri-app/ui/shell.css:1`,
`shell.css:330` and `style.css:640` each hold one live fact (the cascade order,
the scan gap, the mode scoping) buried in a paragraph about what the code used
to look like. Cut to the fact. These are the cheapest items in this section.

**Already named by the 15 Oct audit, still open, and bigger than one comment
(3).** They are worth stating here because no sweep will fix them:
`ToolDef`'s `&'static str` forcing `Box::leak` (`provider.rs:65-72`);
`crates/tauri-app/ui/chat.js` keeping two chat renderers alive behind a `whirl`
fork; `crates/tauri-app/ui/app.js` re-implementing the layout grammar that
`core::layout` owns, which is why `L.valid` and `tree()` exist at all.

## 3. Keep (23)

Grouped by the clause that protects them. Each is a fact about something this
repo does not control, or an invariant a future editor would otherwise undo.

| Clause | Items |
|---|---|
| OS and platform behaviour | `preload.js`-adjacent Windows `which`/PATHEXT (`tests/interrupted.rs:42`), `wsl.exe` UTF-16 output (`capsule/core/src/providers/wsl.rs:20`), macOS translucency (`app/src/ui.rs:63`), Tauri v2 running non-async commands on the UI thread (`tauri-app/src/main.rs:527`), `matchMedia` pinning the sidebar in a screenshot fixture (`tools/verify/shot.js:85`) |
| Upstream tools and CLIs | Codex dropping `--full-auto` (`runner/src/lib.rs:407`), npm's extensionless POSIX shim (`harnesses.rs:197`), Claude Code's per-tool switch bypasses (`web/src/app/Agents.tsx:503,525`), Chromium's capture-time relayout (`uiharness/run.mjs:429`), Ollama omitting `context_length` on some builds (`model_server.rs:406`) |
| A CSS rule the platform imposes | an unregistered custom property is not interpolable, which is why the sidebar width is set on a class (`style.css:180`) |
| Wire and cross-version contracts | `Artifact::kind` stays a `String` so an older shell still reads a newer project (`artifact.rs:33`), the Agent Memory Repo's on-disk layout (`memory.rs:334`) |
| Invariants that must outlive the next tidy-up | one forward pass is enough for diagram depth because edges only point forward (`diagram.rs:72`); the drag's hit-test tree must be the pre-drag snapshot (`app/src/arrange.rs:26`); process-wide env vars are why the test lock is partial (`tests/features.rs:22`); a restart is the OS killing the process, which is why an interrupted turn is not a failed one (`harness.rs:282`) |
| Licence and attribution | the MIT and upstream-file headers on the Whirl-derived `web/src/app/*.tsx` containers (`AssistantMessage.tsx:1`, `Composer.tsx:1`, `RoutePicker.tsx:1`) |

## 4. Where the two audits disagree

Worth settling by hand rather than merging blindly.

| Item | 15 Oct | 17 Oct | Note |
|---|---|---|---|
| `crates/core/src/memory.rs:334` legacy `memory.json` import | delete as dead weight, no install has the file | keep, it is a one-time migration for users' data dirs | The code deletes after importing, so it is one-shot; whether any install still has the file is a fact about users, not the tree |
| `crates/core/src/harness.rs:2792` (15 Oct cited it as 2732; the lines have drifted) | delete as a stale truncated note | refactor, restore the sentence git still has in `ea7152a` and put it on `run_worker_with` | The fragment guards a real recursive-async workaround; the reason is missing, not the code |
| `crates/core/src/provider.rs:65-72` | refactor `ToolDef` to `Cow` | not in the capped top 22 | Treat the 15 Oct list as the fuller refactor backlog |
| `crates/core/src/prefs.rs:298` / `:304` | refactor, MCP command splitting | same | Same finding, line drifted four lines |

## 5. Sequence I would use

1. **The false comments and the coupled deletions.** Minutes each, no design,
   stops readers being misled (`chat.rs:286`, `router.rs:128`,
   `harness.rs:2792`, and the numbered steps with their orphan).
2. **The mechanical DELETE sweep, one pattern per commit.** Banners first
   (78), then restatements (103). Each commit only removes lines, so a reviewer
   can read it as a diff of deletions. `cargo test --workspace --exclude backspace`
   plus `node tools/uiharness/run.mjs` after each, since the UI slices are in
   scope and the harness is already here.
3. **The two security refactors.** `Listen` for the companion bind, and
   `phone_visible()` for the phone's contract. Both are small and both close a
   hole the comments currently paper over.
4. **The type-shaped workarounds**, one at a time: `Add::scope`, `prefs.terminal`,
   `PLANNER_CLIS`, the 16k and stall limits, `ToolDef` to `Cow`.
5. **The two front-end shape fixes** (`ThreadView`'s phantom `epoch`,
   `RichText`'s fence injection) and the harness-stub normalisation, which make
   the fixtures stop carrying prose.
6. **The three big ones last, each with its own plan**: the duplicated layout
   grammar, the two chat renderers, and `ToolDef`.

Nothing in this document was executed. No comment was deleted and no code was
changed by the audit; the only files it wrote are the slice reports under
`docs/comment-prune/`.
