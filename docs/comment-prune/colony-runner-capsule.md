# colony-runner-capsule

I opened all 22 cited sites in `crates/colony/src/*`, `crates/runner/src/lib.rs`, `crates/runner/src/a2a.rs` and `capsule/core/src/{powershell,images}.rs`, reading 15 lines around each, and re-derived every verdict from the code rather than the audit's summary. Every line number and quote matched the file verbatim, so no finding was dropped: `rejected` is 0. One verdict moved: `capsule/core/src/images.rs:91` was proposed delete, but the comment's "the archive contains a single \*.vhd" is an assumption about the upstream Ubuntu azure tarball that the code cannot state, and it exists because the code scans a shared cache directory instead of naming the member — a workaround, so it is demoted to `refactor`. Both proposed `keep`s survived the strict test: `crates/runner/src/lib.rs:407` records an upstream Codex flag removal, and `capsule/core/src/providers/wsl.rs:20` records an OS-tool encoding quirk. The remaining 19 deletes each lose only text a reader can already reconstruct from names, types, bodies and tests.

## Delete (19)

| file:line | kind | the comment | why it goes |
| --- | --- | --- | --- |
| `crates/runner/src/lib.rs:181` | section-banner | `// ------------------------------------------------------------ helpers` | Pure `----` divider; `plain`/`truncate` group themselves and the banner adds no fact. |
| `crates/runner/src/lib.rs:302` | section-banner | `// ------------------------------------------------------------ CLIs` | Divider before `KillTree`/`run_cli`; both names already say "CLI" and the file keeps no banner convention this row carries. |
| `crates/runner/src/lib.rs:656` | section-banner | `// ------------------------------------------------------------ HTTP` | Divider before `Lines`, `run_ollama`, `run_openai`; HTTP is already the shared subject of those names. |
| `crates/colony/src/tools.rs:11` | restates-code | ``/// `colony_create`, `colony_exec`, `colony_list`, `colony_status`,`` | The `vec![...]` two lines down lists these names in this order and `five_tools_with_the_right_names` asserts exactly that. |
| `crates/colony/src/colony.rs:11` | restates-code | `/// The state file inside the colony directory.` | `STATE_FILE` and `"colony.json"` say "state file"; `Colony::path`'s `dir.join(STATE_FILE)` says "inside the colony directory". |
| `crates/colony/src/model.rs:198` | obvious | ``/// Unix milliseconds. The registry stamps `created` with it.`` | `now_ms`, `UNIX_EPOCH` and `as_millis()` give the unit and era; the caller sentence is just a name drop. |
| `crates/colony/src/backend.rs:28` | restates-code | ``/// The `visor-core` executable's name on this host.`` | `pub const EXE` with its `cfg!(windows)` branch shows both the name and the host dependence. |
| `crates/colony/src/backend.rs:69` | restates-code | ``/// Run the CLI. `Ok(None)` when the command prints nothing (delete).`` | `fn run` plus the `text.is_empty() -> Ok(None)` path state both halves; `Backend::delete` discards that `Option` in code. |
| `crates/colony/src/backend.rs:241` | restates-code | `/// PATH → VISOR_CORE → next to the current executable.` | That sequence is the body of `find_exe`, and `VisorCli::discover`'s doc at line 52 already spells out the same order. |
| `crates/colony/src/fake.rs:17` | obvious | `/// A backend that only exists in memory.` | Module doc line 1 already says "An in-memory Backend"; the struct name and `Arc<Mutex<Fake>>` field repeat it. |
| `crates/colony/src/fake.rs:169` | restates-code | `/// A WSL spec, as tests would write it.` | The body is `CreateSpec::new(name)`, whose `ProviderKind::default()` is `Wsl`; "as tests would write it" is not a property of the fn. |
| `capsule/core/src/powershell.rs:18` | restates-code | `/// Run PowerShell and parse stdout as JSON.` | `ps_json` over `ps(script)` plus `serde_json::from_str` is verbatim the doc, including the `Ok(None)`-on-empty case. |
| `crates/runner/src/a2a.rs:141` | restates-code | `// A plain message: that's the answer.` | The branch returns `Ok(())` right after emitting the parts text; the immediate `return` is "that's the answer". |
| `crates/runner/src/a2a.rs:146` | restates-code | `// A task: wait until it's done.` | The `loop`, the `DONE` set and the `ToolEnd`/`Event::Text` below already read as a poll-until-done task. |
| `capsule/core/src/images.rs:11` | section-banner | `/// Image catalog. URLs verified against cloud-images.ubuntu.com listings.` | A banner over `wsl_image`, one 7-line match arm, not a catalog; the "URLs verified" sentence is an undated past claim no reader can check or keep true. |
| `crates/colony/src/colony.rs:66` | restates-code | `/// The state file's path.` | `path()` returning `self.dir.join(STATE_FILE)` is the whole body; the name and return type are the story. |
| `crates/colony/src/colony.rs:76` | obvious | ``/// One workspace, or `None`. Never panics; see [`Colony::get_or_err`].`` | `Option<&Workspace>` promises `None` and the one-line `iter().find()` cannot panic; the link is a cross-reference, not information. |
| `crates/colony/src/colony.rs:81` | restates-code | `/// The host's capabilities, straight from the backend.` | The entire body is `self.backend.capabilities()`; "host" and "from the backend" are the call itself. |
| `crates/runner/src/lib.rs:244` | restates-code | `/// What to send: the last turn alone with a session, the transcript without.` | The `match (&req.session, req.history.last())` and its two arms are the sentence. |

## Refactor first (1)

| file:line | the comment | what the code should become |
| --- | --- | --- |
| `capsule/core/src/images.rs:91` | `// the archive contains a single *.vhd; find it` | The scan is a workaround for not naming the tar member. Extract into a private temp dir (as `ensure_wsl_rootfs` does with `extract-tmp`) and open the one derived name `noble-server-cloudimg-amd64-azure.vhd` from `ImageDef.file`, instead of `read_dir` over the shared cache dir picking the first `*.vhd`. |

## Keep (2)

| file:line | the comment | what cannot change |
| --- | --- | --- |
| `crates/runner/src/lib.rs:407` | ``// `--full-auto` is gone from recent Codex; the sandbox says it.`` | The Codex CLI is upstream; it dropped `--full-auto` and the flag set here is the replacement. Only this line stops a later editor from adding the flag back. |
| `capsule/core/src/providers/wsl.rs:20` | `// wsl.exe emits UTF-16LE on some invocations` | `wsl.exe` is an OS tool whose output encoding varies per invocation; that is why `decode_wsl_output` sniffs BOM/zero bytes, and no name or type can carry it. |
