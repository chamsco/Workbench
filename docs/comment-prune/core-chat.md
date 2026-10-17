# core-chat

Opened every cited line in the files that carry findings (`crates/core/src/chat.rs`, `crates/core/src/agents.rs`, `crates/core/src/board.rs`, `crates/core/src/project.rs` — `crates/core/src/ticket.rs` has none) and read 15 lines around each: 22 findings, 22 rulings. Every quote is verbatim once the line is corrected, and eleven lines were wrong — `chat.rs:2193` is past the end of a 2087-line file (the `turns` doc is at 1394), `chat.rs:651` (the `Scope` doc is at 229), `chat.rs:32` (the `hidden` doc is at 223), `chat.rs:118` (the ad doc is at 97), `chat.rs:329` (the `app` doc is at 209), `chat.rs:525` (the `set_route` doc is at 719), `chat.rs:1043` (the `resume` doc is at 435), `chat.rs:516` (515), `project.rs:25` (33), `agents.rs:36` and `agents.rs:219` (37 and 188). I dropped one finding as rejected: `project.rs:125` quotes and cites correctly, but its own `why` concedes the first two sentences carry the reason `ses` survives a restart, so deleting that comment as cited loses information a reader cannot recover and fails the delete test; the audit-process residue itself (`/// the second half of the audit's P0-2.`, project.rs:128) is worth trimming in a separate edit, but that is a partial trim on a four-line doc, not a whole-comment delete. Nothing was promoted to Keep: no comment here is about a platform, upstream, wire format, external tool or licence this repo cannot change, and the two load-bearing ones are markers on code shortcuts, so they are Refactor rows.

## Delete (19)
| file:line | kind | the comment | why it goes |
|---|---|---|---|
| crates/core/src/chat.rs:331 | section-banner | `// ---------------------------------------------------------------- base64` | A divider over `b64_encode` / `b64_decode`, whose names already place them and which `grep base64` finds. |
| crates/core/src/chat.rs:1392 | section-banner | `// ------------------------------------------------------------ runner` | Pure divider before `turns`; the functions below name themselves. |
| crates/core/src/chat.rs:1110 | restates-code | `/// Mark a reply done or failed.` | `finish(tid, mid, res: Result<()>)` and its `Status::Done` / `Status::Error` arms are the sentence; the doc does not even mention the parts it does not say (`t.updated`, `touched`). |
| crates/core/src/chat.rs:515 | restates-code | ``/// `purge_expired` with the clock given, so a test can step past a deadline without waiting for it.`` | The `now: u64` parameter and `purge_expired` calling `purge_at(now_ms())` show the injection; tests are the only caller that could pass anything else. |
| crates/core/src/chat.rs:229 | obvious | ``/// Where a new thread lives: the plain chat list, a project (Pair) or an app.`` | `Scope`'s own fields (`project`, `app`, `agent`, `members`, `goal`) and `create_in(route, scope)` carry it. |
| crates/core/src/chat.rs:1394 | restates-code | `/// This thread's history as the runner's turns.` | `fn turns(&self, id: &str, hist: &[Msg]) -> Vec<Turn>` is the sentence as a signature, with the mapping visible below it. |
| crates/core/src/board.rs:74 | restates-code | `/// Check the recipient and append. Returns the message.` | `post` is a one-line `reply(s, from, to, text, None)`; the checks and the `Result<BoardMsg>` are documented on `reply`. |
| crates/core/src/chat.rs:105 | restates-code | `/// Who answered, when not the thread's own route (an @mention).` | Every writer sets `via` only for a mention (`msg.via = mentioned.then(...)`), and `Option<Route>` is the "who". |
| crates/core/src/chat.rs:60 | obvious | `/// Written, not yet picked up.` | `Msg::new(..., Status::Sending)` followed by `spawn_reply` → `set_text` shows the pre-runner stage; the variant name carries it. |
| crates/core/src/chat.rs:435 | restates-code | `/// Carry on with a stopped reply instead of starting a new one.` | `resume: bool` is `true` only from `Chats::resume` and read at `if call.resume`; the call sites plus the field name say it. |
| crates/core/src/agents.rs:265 | restates-code | `/// The brief a reply runs with: who it is, where it works, its computer.` | The `format!` below names the agent, its folder and its computer kind in order. |
| crates/core/src/chat.rs:1272 | restates-code | `// A reply to a specific message carries a quote of it, so the model knows which part of the conversation it answers.` | The loop's `format!("> Replying to {who}: {snip}")` is the quote, and feeding the model is the whole job of `history`. |
| crates/core/src/project.rs:33 | restates-code | `/// Is the agent still doing something?` | `is_active` matching `Queued \| Running \| AwaitingApproval` is the doc plus a question mark. |
| crates/core/src/agents.rs:188 | restates-code | `/// The agent's own folder (created on first use).` | `agent_dir(id).join("workspace")` plus `create_dir_all` is both halves ("own" and "created on first use"). |
| crates/core/src/agents.rs:37 | restates-code | `/// The backstory: who it is and what it does. Its system prompt.` | `brief()` splices `a.job` under "You are {name}" and that text becomes `req.system`; the field plus its one use is the doc. |
| crates/core/src/chat.rs:223 | restates-code | ``/// Never listed or written to disk: a one-off ask (`dreaming`), or a ghost chat.`` | `#[serde(skip)]` means not written, `list` / `save` filter on `!t.hidden`, and the two makers (`hidden: scope.life.is_ghost()`, `hidden: true` for `dreaming`) show the rest. |
| crates/core/src/chat.rs:97 | obvious | `/// A sponsored card shown under this reply (Cloud free tier and overage only). Never part of the reply text.` | `pub ad: Option<Ad>` plus `cloud.rs:150`'s `Ad` ("A sponsored card.") and `Decision::WithAd`; a separate field cannot be part of `text`. |
| crates/core/src/chat.rs:209 | restates-code | `/// The installed app this thread belongs to (its agent's chat).` | `pub app: Option<String>` sits beside `project`, `system`, `agent`, `members`, and is mirrored by `Scope.app` and `ThreadInfo.app`. |
| crates/core/src/chat.rs:719 | restates-code | `/// Switch who answers from the next message on. A CLI session does not carry over to another backend.` | The body clears `t.cli_session` exactly when kind or provider changes; `set_route` names the switch. |

## Refactor first (2)
| file:line | the comment | what the code should become |
|---|---|---|
| crates/core/src/chat.rs:286 | ``/// Members a message names, as `@Name` or the bare name at a word boundary.`` | Make the code match its own contract: `named_in` only does `low.find(&format!("@{n}"))`, so either tokenise the text and compare each word against the roster (bare-name matching as documented) or narrow the doc to `@Name`. A caller cannot tell which was meant today. |
| crates/core/src/chat.rs:414 | `// ponytail: one global for the one chat face; per-window if there are ever two.` | Carry the prompt on `Ctx` (per-send state, built at chat.rs:421) and read `ctx.genui_prompt` at chat.rs:1470 instead of a process-wide `static`; one window (`tauri-app/src/main.rs:1942` gets `"main"`) is the only reason the global works today. |

## Keep (0)
| file:line | the comment | what cannot change |
|---|---|---|
