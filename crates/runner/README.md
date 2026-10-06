# backspace-runner

One turn on any model or agent, as one stream of events. MIT.

| Target | What it runs |
|---|---|
| `Cli { provider, bin }` | Claude Code (`claude -p`, stream-json, `--resume`), Codex (`codex exec --json`), Cursor, OpenCode, Grok, headless |
| `Ollama { url }` | Ollama's `/api/chat`, streamed |
| `OpenAi { base, key }` | Any OpenAI-compatible Chat Completions API (OpenRouter, LM Studio, vLLM, xAI, Meta's Muse API…) |
| `A2a { url, token }` | A remote agent over A2A: `message/send`, polling `tasks/get`; `a2a::card()` reads its agent card |

```rust
let req = Request::new(Target::Cli { provider: "claude".into(), bin }, vec![Turn::user("Fix the failing test")]);
run(&http, &req, &mut |e| println!("{e:?}")).await?;
```

Events: `Text`, `Break`, `Replace`, `Final`, `Model`, `Session` (pass it back as `req.session` to resume), `Cost`, `Usage`, `ToolStart`, `ToolEnd`.

It doesn't find CLIs, keep history or store anything; the caller does. Try it: `cargo run -p backspace-runner --example ask -- claude "hello"`.
