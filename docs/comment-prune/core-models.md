# core-models

All 22 findings were opened at their cited line with 15 lines of context in the seven named files (`harnesses.rs`, `model_server.rs`, `router.rs`, `provider.rs`, `accounts.rs`, `options.rs`; `effort.rs` carried no findings), and every quote and line number was confirmed verbatim before ruling. One finding was dropped: `router.rs:128` really does open with `with_local`'s sentence, but the paragraph's closing claim — every delegation path goes through `route_for`, so the Settings tick decides globally — is a repo-wide invariant that is not recoverable from `local_pick`, so a blanket delete would lose information; it is counted in rejected. Two verdicts changed after reading the code: `model_server.rs:406` moved to Keep because the absent `context_length` is Ollama's own upstream listing behaviour (the only field with no name fallback), and `harnesses.rs:60` stayed Delete only because "one-shot prompt" is already the code's own wording at L563, which removes the one useful-looking clause; `accounts.rs:260` was re-kinded to a stale rationale, since `remove` (L188-190) deliberately leaves the profile directory on disk. The Windows npm-shim comment keeps its Keep status on platform/upstream proof, and the two marked shortcuts are refactors because both root-cause fixes live in this repo.

## Delete (17)

| file:line | kind | the comment | why it goes |
|---|---|---|---|
| crates/core/src/harnesses.rs:24 | obvious | `/// A coding agent CLI (Codex, Claude Code...).` | `Kind::Cli` plus the CLIS table (codex, claude, cursor…) carry it; the example list is stale-prone. This row covers L24 only — the L26/L28/L30 variant docs are not part of it. |
| crates/core/src/harnesses.rs:60 | restates-code | `/// Can be used in Chat (one-shot prompts). All but Antigravity.` | "All but Antigravity" is the `chat: false` arm at L114, and "one-shot prompt" is already the code's own message at L563. Drift-prone, no new information. |
| crates/core/src/harnesses.rs:118 | restates-code | ``/// The executable for a CLI id ("cursor" is `cursor-agent`).`` | Both mappings are the match arms directly below (L121-122); `bin_for -> &str` says the rest. |
| crates/core/src/harnesses.rs:289 | section-banner | `// ---------------------------------------------------------------- auth` | Divider only; `jwt_claims`, `b64url` and the other helpers below name the region. Nothing here is unrecoverable. |
| crates/core/src/harnesses.rs:705 | restates-code | `/// Probe everything in parallel. Takes a few seconds at most.` | Parallelism is `std::thread::scope` + `s.spawn` at L708-715; "a few seconds" is an unverifiable timing promise. |
| crates/core/src/model_server.rs:9 | restates-code | ``//! [`is_tiny`] is the one heuristic in here: 4B parameters or less when the`` | A strict subset of `is_tiny`'s own doc (L115-119), including the 4B / name / 3.5 GiB thresholds; two copies of one threshold drift apart. |
| crates/core/src/model_server.rs:35 | section-banner | `// ---------------------------------------------------------------- shapes` | One of five identical dividers (L35, 101, 234, 374, 475); the type names below describe the region. |
| crates/core/src/model_server.rs:268 | restates-code | `/// The task-kind mapping, in one place. The named families are tried first,` | The table at L272-278 copies `need_preference`'s arms (L306-313) and `pick`'s window logic (L286-293). Drop the table, keep the prose contract. |
| crates/core/src/model_server.rs:483 | restates-code | `// (runtime id, name, base_url, api key): loopback routers first, then the` | The type annotation names nothing, but the closure below does (`r.id`, `r.name`, `r.base_url`, `r.api_key`) and `.filter(is_loopback).map(...).chain(extra_urls)` shows the order. |
| crates/core/src/router.rs:152 | restates-code | `// Only if the model really is in this config and usable right now.` | The `usable_models().iter().any(\|m\| m.id == model)` guard on the next line states exactly this. |
| crates/core/src/router.rs:183 | history-narration | `/// The local runtime is checked to be listening first. Without that, a` | Past-tense bug story; the check is L197 (`local_runtime_is_up`) and the counterintuitive part (a keyless loopback provider counts as usable) is already documented at L124-127. |
| crates/core/src/accounts.rs:20 | restates-code | ``/// A day in ms: the window `fails_24h` counts over.`` | `24 * 60 * 60 * 1000` and the field name `fails_24h` carry both halves of the sentence. |
| crates/core/src/accounts.rs:93 | restates-code | `/// Drop times older than the window and refresh the cached count.` | Line-by-line narration of `retain` by cutoff and `fails_24h = len` (L96-97); why decay matters already lives on `recent_fails` (L86-87). |
| crates/core/src/accounts.rs:260 | stale-rationale | `/// Where a new account's profile goes. One directory per provider+id under the` | The trailing clause ("so deleting an account can delete its login") is contradicted by `remove` (L188-190), which deliberately leaves the profile dir on disk. Path shape is visible at L263-265. |
| crates/core/src/accounts.rs:278 | restates-code | `// Counts carried over from a process that has been down must decay.` | Same rationale already on `recent_fails` (L86-87); `prune(now)` during `load` needs no narration. |
| crates/core/src/options.rs:18 | restates-code | `/// How many variants one set may hold.` | Restates the identifier; the `6` is the value and no rule explains why it is six. |
| crates/core/src/provider.rs:294 | section-banner | `// ------------------------------------------------------------------- openai` | Divider only; `openai_body` and `parse_openai` below name the region. |

## Refactor first (2)

| file:line | the comment | what the code should become |
|---|---|---|
| crates/core/src/router.rs:142 | `// Tolerate the plural and the singular, since the UI and the prefs file` | Normalise the job keys once, at the prefs/UI boundary — one spelling, or a `Job` enum. `local_pick` then becomes a plain `self.local.get(key)` and the `trim_end_matches('s')` / `format!("{k}s")` matching at L147-150 is deleted. |
| crates/core/src/harnesses.rs:499 | `// ponytail: first terminal found wins; add a pref if someone needs another.` | Add a `prefs.terminal` entry, resolve it first, then fall back to the PATH search. The four-name list at L500-503 is deferred debt, not a platform constraint. |

## Keep (2)

| file:line | the comment | what cannot change |
|---|---|---|
| crates/core/src/model_server.rs:406 | `// A few builds list it; when they do not, it stays unknown.` | Ollama's own `/api/tags` entry omits `context_length` on some builds — upstream wire behaviour this repo cannot change, and the one field with no name fallback (contrast L398-405). |
| crates/core/src/harnesses.rs:197 | `// On Windows npm puts a POSIX shell shim with no extension next to` | Windows/npm packaging: npm writes an extensionless POSIX shim beside `<bin>.cmd`, and spawning it fails with OS error 193 — hence only `exe`/`cmd` are tried (L199-205) and the bare path is gated on `cfg!(unix)`. |
