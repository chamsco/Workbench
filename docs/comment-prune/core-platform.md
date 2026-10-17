# core-platform

I opened all eleven audited files at every cited line and read at least fifteen lines around each, plus the in-repo proofs the findings leaned on (features.rs:254, the module headers, the skip list, `fill()`). Every one of the nineteen quotes and line numbers checks out verbatim, so nothing was rejected and no finding was dropped. I did narrow four: the stray `git.rs` half-block is L270-273, of which only L270-271 is the duplicate; `prefs.rs:498` is a two-line sentence and it is false, because the unseen-project arm returns `TabSpec::defaults()` and never the global `self.tabs`; `prefs.rs:409` runs L409-411; and the `cloud.rs:165` finding was argued as deleting the whole `Decision` variant-doc block, which I refused because L169 and L171 carry plan facts (that `NeedsUpgrade` also covers the past-allowance case, and that `OutOfReplies` lasts until the reset) that no name or type states. No comment survives as keep: each is in-repo text this repo can change, and none documents a platform, upstream, wire-format, external-tool or licence constraint. Three are load-bearing because the code is a shortcut or lets a bad state be represented, so they are refactors with a named root cause rather than deletions. Counts: 16 delete, 3 refactor, 0 keep, 0 rejected.

## Delete (16)

| file:line | kind | the comment | why it goes |
|---|---|---|---|
| config.rs:39 | self-praise | `/// "Plan on high. Delegate on medium. Keep Opus on call."` | A slogan, not information: it names no field and no effort value, and the `model`/`cli`/`scout_model` Options above already say which model fills which role. |
| git.rs:270 | orphan-doc-duplicate | ``/// What is uncommitted in `dir` (staged, unstaged and untracked), and the`` | Stray first half of a doc block: L272-273 already documents `changes`, and the only unique words ("the side panel's Changes") name a call site, not a constraint. Delete L270-271. |
| prefs.rs:498-499 | false-doc | `/// The tabs to show for a workspace: its own, made from the global default the first time that project is opened.` | False against the code: the unseen-project arm returns `TabSpec::defaults()` (L505-508), never the global `self.tabs`; features.rs:254 pins that behaviour, so the doc invents a rule the code does not have. |
| prefs.rs:409-411 | self-praise | `// One empty canvas to start. The IDE asks what you want to open here` | L409 restates the literal `vec![TabSpec::new(None, vec![PaneSpec::of("empty")])]`, and L410-411 only argue the product choice is fine; nothing here tells an editor what breaks if the default changes. |
| cloud.rs:165 | restates-code | `/// Answer, then show an ad.` | `WithAd` plus `fill()`'s `m.with_ads = d == Decision::WithAd` already say it. Only this line goes: L169/L171 carry plan facts absent from the names. |
| cloud.rs:224 | section-banner | `// ---------------------------------------------------------------- client` | Divider that adds nothing: the public fns below are already signup, account, set_plan and call. |
| cloud.rs:303 | section-banner | `// ---------------------------------------------------------------- dev server` | The same divider pattern inside one file; `Server` and `serve` already name the section. |
| remote.rs:80 | section-banner | `// ---------------------------------------------------------------- server` | Divider heading; the module doc and the names read_request/serve/respond/handle carry the structure. |
| remote.rs:297 | section-banner | `// ---------------------------------------------------------------- client` | Divider heading; `Link`, `Remote` and `probe` are self-describing. |
| apps.rs:383 | section-banner | `// ------------------------------------------------------------ storage` | Divider heading; store_path/store/storage_get/storage_set already name the section. |
| cloud.rs:150 | restates-code | `/// A sponsored card.` | A gloss on the type name: `advertiser`, `cta` and `url` on `Ad` say the same thing. |
| prefs.rs:248 | restates-code | `/// Is this address on this machine only?` | The fn is `is_loopback`; the doc asks the name again and never mentions `bind_addr`, which is the only non-obvious part. |
| git.rs:414 | test-narration | `// Your checkout is untouched: no commit on it, nothing staged.` | Narration over the two asserts directly under it (HEAD verify fails, run-dir contents); it names no fact those calls do not. |
| cloud.rs:722 | test-narration | `// Free: small models with an ad, nothing bigger, stop at the limit.` | A paraphrase of the three asserts below, and the module header (L3-7) already tabulates the same plan rules. |
| apps.rs:559 | test-narration | `// Reinstalling keeps stored values.` | Restates the `storage_get` assert two lines down; the test name install_serve_store_remove announces the scenario. |
| files.rs:17 | restates-code | `/// Depth-first walk, skipping build output and VCS internals.` | `walk`'s recursion at L49-51 is the depth-first evidence and the skip list is the next lines; `scan` plus FileEntry's `depth` carry the summary. |

## Refactor first (3)

| file:line | the comment | what the code should become |
|---|---|---|
| companion.rs:63 | `/// a folder, so the audit's finding stands — they are not part of the phone's` | The sentence (L61-64) is the only record of why those threads are withheld, so it stays; make the contract code rather than a cited audit. Give `ThreadInfo` a named visibility predicate (e.g. `phone_visible()`, or a `Visibility` set where the thread is created) and have `phone_may_read` return that by name; the doc can then state "an app chat, a named-agent thread or a group can drive a CLI that edits your folder, so they are not part of the phone's contract" with no appeal to a past audit. |
| prefs.rs:218 | `/// audit's point stands: plain HTTP with a token, chats that drive agents,` | `addr` plus `allow_lan` are two independent fields for one decision, so `allow_lan: false` with a LAN address (or the legacy `0.0.0.0` upgrade path) is representable and only `bind_addr` resolves it. Replace them with one typed listener choice (`Listen::Loopback { port }` \| `Listen::Lan(addr)`); then an exposed LAN bind cannot be an accident and the doc states the exposure rule without the audit. |
| prefs.rs:304 | `/// ponytail: the command splits on spaces; quoted arguments and env come when a server needs them.` | `server()` is a shortcut around `split_whitespace()`, which silently mangles quoted arguments and cannot pass env. Parse the target with shell-style word splitting, or accept an explicit `{command, args, env}` object and emit that; then the note goes. |

## Keep (0)

| file:line | the comment | what cannot change |
|---|---|---|
