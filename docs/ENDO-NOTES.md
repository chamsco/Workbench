# What [endo](https://github.com/longsurf-ai/endo) does differently

Read on **16 Oct 2026** at `main`. Endo is a TypeScript/Electron "Higher Order
Harness" by longsurf-ai: it composes coding agents (Codex, Claude Code,
Antigravity) into workflows inside one chat surface. Same category as Workbench,
opposite stack, and further along on packaging and docs discipline.

Everything below was fetched live. This is a curated shortlist, not a dump — for
each idea: what they do, why it might fit here, and what it costs.

## 1. What their README does that ours did not

Our README was 579 lines of accurate, dense prose that explained the system to
someone who already understood it. Endo's is 6.7 KB and explains it to a
stranger. Concrete moves, all now applied to our README:

| Their move | What we did |
|---|---|
| Centered `logo → H1 → one-line tagline → hero loop → nav row → badges` | Same skeleton: badge row, an animated hero diagram, a nav row of anchor links |
| A hero **GIF** directly under the tagline | `docs/assets/how-it-works.svg` (animated, 8 KB — cheaper than their 3.4 MB GIF) plus `docs/assets/tour.gif` |
| **"What is this" opens with a definitional contrast**, not a feature list (`"A harness runs one agent… Endo is a Higher Order Harness"`) | Opens with "What is Workbench?" and a *"why not just a chat window?"* table |
| **Alt text written as a caption sentence** (`action → visible result`) | Every image alt text is now a full sentence describing the action and the outcome |
| **One GIF per feature under a benefit-titled `###`** | One screenshot per feature group, under `<details>` headings so the page stays scannable |
| **Quickstart ends with a real example prompt and the screen it lands on** | Added an actual goal to type ("make an empty title a 400, and add a test that proves it") and what happens next |
| **"Building on it" as a file-path map** for contributors | Added a *"Where to change what"* table |
| **One-line maturity disclaimer** ("Endo is at 0.1.x, so expect breaking changes") | Added, plus a `status: 0.1` badge |
| **Credits section** naming the upstreams they borrowed from | Added — we borrow from Whirl, mattpocock, ponytail, Pi, AMR |
| **A README for the media folder** with the exact encode recipe | `tools/doc-assets/` — the pipeline that generates every image; see its README |
| **Mandated Mermaid for diagrams in docs** | Our architecture is now an inline Mermaid block, not just a link to a 47 KB HTML file |
| **Tables for anything a reader looks up** | Glossary table, provider table, limits as bullets, "where to change what" table |

Their README also has real gaps we should **not** copy: no architecture diagram,
no tech-stack line (Electron appears only in `docs/development.md`), no TOC, no
comparison, no FAQ, and a 1.7 MB GIF of a cursor clicking *Star*.

## 2. Technical ideas worth taking, ranked

### P0 — cheap, clearly right, fits what is already here

| Idea | What they do | Why it matters here |
|---|---|---|
| **Fix the skill frontmatter parser** | They parse YAML properly | **This is a live bug in our checkout, found while researching this doc.** `fn skill()` (`crates/core/src/skills.rs:207-222`) reads the first line starting with `description:`. Upstream ponytail writes `description: >` with the text on the next indented lines, so all six ponytail skills — *including the one already vendored* — resolve to the single character `>`. Settings → Agent skills shows `>`, and every agent's system prompt index renders `- ponytail: >`. The test passes because it only asserts non-empty. See [`RESEARCH-UPSTREAM-SKILLS.md`](RESEARCH-UPSTREAM-SKILLS.md) for the byte-level evidence and the fix. |
| **Structured, typed tool errors end to end** | They patched `@ag-ui/*` to keep an `error` field on the tool-result event so status survives rendering | We currently collapse a tool failure into a message string in places. A typed error on the record means the UI renders from data instead of parsing prose. |
| **Record the tool call *before* running it** | "Tools are recorded before they run"; the processor waits for the commit before invoking the callback | A crash mid-tool is then always explainable from the transcript. We already snapshot around tool calls for Carry on — this closes the remaining window. |
| **Snapshot-on-connect, committed-state-only events, no replay buffer** | "Events publish only committed state"; on reconnect the client re-reads current state and no event is replayed | We ship the whole `ProjectState` over IPC on every event (already flagged in `HANDOFF.md`). Their model — fetch a snapshot, then consume deltas — removes an ordering bug class *and* the payload growth. |
| **Optimistic concurrency with structured conflicts** | `expectedRevision` on writes; a stale write returns `{workspaceId, path, current}` | The file-tool version guard already exists in this workspace. Returning the current value instead of a generic failure lets the agent rebase instead of giving up. |
| **One classifier for every error** | `failureFor` classifies once for transport and logging; handlers do not add their own | Small, mechanical, and it is the difference between a debuggable app and one that says "something went wrong". |

### P1 — real design wins, more work

| Idea | What they do | Why it matters here |
|---|---|---|
| **Dig In: selection → side conversation** | `agent.digInSession({sessionID, messageID, selection})` copies that reply and the history before it into a `kind: dig_in` **child session**, and appends an anchor to the parent. Never a modal. | The single best UX idea in their repo. We already have conversations, canvases and a board — this is a natural third pane, and it keeps the main thread clean. |
| **Turn-paginated transcripts** | `readTranscriptPage({cursor, turnLimit})`, default **1 complete turn**, returns `nextCursor` | Paginating by message splits a turn across pages. Ours renders a whole session; long runs are where that hurts. |
| **File references stored as paths, not snapshots** | `@file` saves a `:file[label]{name=/abs/path}` directive and never reads or uploads the content; links resolve against the message's persisted `cwd` | Our composer attaches files. Storing the path keeps history small and links honest. |
| **A permissions rule algebra** | Ordered rules, "last match wins; no match asks", **deny > ask > allow**, saved grants that cannot bypass a configured denial, three global modes (ask / auto / full-access) | We have "Known limits: no sandbox — `bash` runs as you". Their model does not make bash safe either, but the *rule shape* is the subtle part most harnesses get wrong, and we would need it the moment we add one. |
| **Durable run queue: admission separate from execution** | Submitting a run returns the persisted row without waiting; serial within a session, concurrent across sessions; startup marks orphaned runs `failed` and does **not** retry them ("tool side effects may already have happened") | We already mark interrupted workers rather than failing them (`HANDOFF.md`) — this generalises it to every send, and the "do not retry" reasoning matches our own Carry-on decision. |
| **A machine-readable changelog surfaced in-app** | `changelog.json` feeds Settings → Changelog; the newest entry must match the package version | We have no changelog at all. The version-match rule is what stops it drifting. |

### P2 — good ideas, weaker fit or expensive

- **One definition derives the API, the agent's tools, the prompt text and the UI
  invalidation.** Elegant; in Rust it means a proc-macro or a single
  serde-derived schema. Worth it only once the tool surface stops moving.
- **Workflow-as-a-file** (`agent()`, `parallel()`, `phase()`). Our equivalent is
  tickets plus `blocked_by`, which is more constrained on purpose. The
  transferable parts are: one file = one workflow, the file owns its argument
  schema, the loader rejects imports, and the executed source hash is recorded.
- **OTel spans as the workflow substrate** instead of a second graph format.
  We already emit OTel-shaped traces — the idea is to render the *orchestration*
  from the same spans rather than a parallel diagram model.
- **Per-call structured output with decode-once**, and the *caller* decides
  whether to repair. We do retry loops in places; their rule is no hidden retries.
- **Release engineering:** `SHA256SUMS`, a `SOURCE` file naming the commit, and
  "published versions are immutable — corrections require a higher version". We
  publish one rolling `mac-latest` pre-release, so this does not apply yet.
- **Per-directory, reference-counted, invalidate-only file watching** ("events
  only invalidate queries; they do not drive scans or indexing").

## 3. Things endo has that we should **not** copy

1. **Legacy naming bleeding into a public repo.** The repo was carved out of
   `OpenChart`; packages are still `@openchart/*`, the home dir is still
   `openchart.sqlite3`, release tooling still says `openchart-releases`.
   *This is exactly our situation* — `chamsco/Workbench` is a rename of
   `ohMyHarness` of `backspace`, and the README used to open with the old name.
   Endo shows where that ends up. Ours now says explicitly: Workbench is the
   product, `backspace` is the plumbing.
2. **Monster single-file docs.** `agent.md` is 45 KB, `models.md` 44 KB,
   `workspace.md` 41 KB — with no TOC and no index. Prefer many small docs.
3. **Stale, self-contradicting docs.** Their `agent.md` says `cwd` wiring is
   "still pending" while `workspace.md` says it is done; `workspace.md` has a
   duplicated sentence and skipped section numbers; the README's alt text says
   "Gemini" where the body says "Antigravity". Docs-as-contract only works with
   a consistency check.
4. **~16 MB of media in git** (four GIFs ≈ 13.8 MB, plus source clips). Our whole
   asset set is under 3 MB, and the hero animation is an 8 KB SVG.
5. **A star-CTA GIF** — 1.7 MB of a cursor clicking Star inside the docs.
6. **Deep framework coupling** (Effect `4.0.0-rc.112` in every service) and
   **shipped dependency patches** pinned to an unmerged upstream PR. The ideas
   (scopes, structured cancellation, typed errors) are good; the implementation
   is not portable and costs every outside contributor.
7. **No contribution scaffolding** — no `CONTRIBUTING.md`, no issue or PR
   templates, no CODE_OF_CONDUCT.
8. **"Trusted code, not a sandbox" as a stated posture.** Fine for a local
   single-user app; do not inherit it if we ever load a skill or workflow file
   from an untrusted source.

## 4. Small things we are missing that endo has

- **No repo description and no topics on GitHub.** `chamsco/Workbench` currently
  has none, so it is invisible in search and its social card is blank. One line
  each, set in the repo settings — a README cannot fix it.
- **No `LICENSE` file at the root.** `license = "MIT"` is declared in
  `Cargo.toml` and in every crate manifest, but GitHub reports the repo as
  unlicensed and there is nothing to link to. Add the MIT text as `LICENSE`.
- **No build badge** — the macOS workflow exists
  (`.github/workflows/macos.yml`) but `HANDOFF.md` records that it has never been
  seen to pass, so a badge would currently be red. Fix the workflow first.
- **No `docs/README.md` index.** We now have thirteen docs; a two-line index
  would help.
