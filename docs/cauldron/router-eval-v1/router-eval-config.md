# Router eval: configuration

**Historical-prompt replay evaluation — provisional pre-runtime baseline.**

| setting | value |
|---|---|
| decider | `KeywordDecider` (name `keyword`), no network; ablation `KeywordDeciderNoLength` (name `keyword-no-length`) |
| confidence threshold | 0.7 (`DEFAULT_TRIAGE_THRESHOLD`); other thresholds only in the separate sensitivity table |
| triage | `Triage::from_distribution`: Confident when top probability ≥ threshold, else Ask |
| config | `DEFAULT_CONFIG` (`crates/core/default-config.toml`); `[intake] enabled = false` unchanged |
| frontier policy | tier 0 local, 1–3 mid, 4 frontier only; `model_class` (alias `class`) overrides; unknown class is a config error |
| cost | Not computed — insufficient measured token and execution data |
| provider calls | none |
| code HEAD at run | `a6145099664fa3ce24341e0548e36eac335b1562` on `claude/project-thread-xkq6x0` |

## Frozen corpus (SHA-256 of each file)

| file | sha256 |
|---|---|
| `router-eval-excluded.jsonl` | `d8c6a1c087ab899e90ff810cb37a8028a0fa146c69b97f3387cc65b1a0a20265` |
| `router-eval-owner-review-pool.jsonl` | `2e17cd01ab335622631fb91b6b1607ff18897cbf6ff50123f987d2689a78f53c` |
| `router-eval-stress.jsonl` | `676ba2144dc6200ee47d67a6d07b669ffa82faf7ddc54eb74de0ed66daf48ce5` |
| `router-eval-v1.jsonl` | `80c470606f3d3ad90c2cc2cae12a2133d41c2129ee3d6e7516060263801ef9d2` |

Manifest hash (sha256 of the four `name  hash` lines above, sorted by name): `beb71f3bead5b037997af3184a51197af5e5848a5449ca89d0750b800f234a13`.

## Commands

```
cargo run -p backspace-core --example router_eval_report -- router-eval-v1.jsonl --threshold 0.7
cargo run -p backspace-core --example router_eval_report -- router-eval-v1.jsonl --threshold 0.7 --no-length
cargo run -p backspace-core --example router_eval_report -- router-eval-stress+primary.jsonl --threshold 0.7   # cat v1 + stress
cargo run -p backspace-core --example router_eval_report -- router-eval-excluded.jsonl --controls
backspace-cli eval router-eval-v1.jsonl --threshold 0.7
```

Rows with extra fields parse fine: `Trace` reads `id`, `text`, `label` and optional token counts; no row has token counts.

## Row schema

`id, text, source_message_id, created_at, author, where, source_fetch, text_sha256, chars, label, group, draft_label, label_confidence, owner_review, label_changed, flags, rationale, label_status` (controls carry `expected = "ask"` and `control_reason`; pool rows carry `proposed_label`).
