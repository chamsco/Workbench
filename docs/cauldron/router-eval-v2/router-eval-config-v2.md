# Router eval config v2

**Historical-prompt replay evaluation — provisional pre-runtime baseline.**

| setting | value |
|---|---|
| decider | `KeywordDecider` (name `keyword`); ablation `KeywordDeciderNoLength` (`keyword-no-length`) |
| threshold | `DEFAULT_TRIAGE_THRESHOLD` = 0.7; others only in the exploratory table |
| triage | `Triage::from_distribution`: confident when top probability ≥ threshold, else ask |
| config | `DEFAULT_CONFIG`; `[intake] enabled = false` (unchanged) |
| cost | Not computed — insufficient measured token and execution data |
| provider / network calls | none |
| code HEAD at run | `af31eb33d59613acad831eb9e90dc939224f416a` |

## Corpus hashes (SHA-256)

| file | sha256 |
|---|---|
| `router-eval-v2-excluded.jsonl` | `50850af8b53c3035733d5a7b40191b1330ec6c6d2992c9279e835f413932ce88` |
| `router-eval-v2-owner-review-pool.jsonl` | `b6ff6d585d2a4361d58a86beafdd24b6be71eb494a9fb58f88481d5f1283b4ef` |
| `router-eval-v2-stress.jsonl` | `c4b9da64326013b6f14a3daf89d72d9f310508faaac2d3bc676007db369e2786` |
| `router-eval-v2.jsonl` | `52ba81e99671f06bd206f6dbf06d5ecfb820cd0caf30c963e8b9ab6314388522` |

Manifest: sha256 of the four `name  hash` lines (two spaces, newline-terminated, sorted by filename): `581fbbb8c3a97b341185fe3f87d69fde72e61ee15e0977226099e587cab134f6`.

v1 files are preserved unchanged in `docs/cauldron/router-eval-v1/` (primary `80c470606f3d3ad90c2cc2cae12a2133d41c2129ee3d6e7516060263801ef9d2`).

## Frontier and model-class policy audit (code as it is, not owner approval)

| item | state |
|---|---|
| tier 0 local, 1–3 mid, 4 frontier by default | implemented in `ladder::class_of`; tested (`model_class_policy.rs` table over tiers 0–4, shipped defaults, `ladder.rs`) |
| `model_class` (alias `class`) overrides tier | implemented (serde alias on `ModelSpec.class`); tested, including both keys at once being a config error |
| unknown class is a config error | implemented (`ModelClass` is a serde enum); tested for several bad strings and a number |
| used consistently | `within`, `cheapest_on`, `planner_start`, and `Ledger.frontier_calls` all use `class_of`; tested in one test |
| documented | `ladder.rs`/`config.rs` doc comments; `docs/cauldron/M2-REPORT.md` item 5 marked superseded |
| owner approval | **not recorded as approved.** The tier-4-only rule came from an instruction in Charles's 2026-11-14 spec; no explicit sign-off on the resulting behaviour exists. Awaiting approval |
| validated by this replay | **no.** The replay ran no provider and measured no task outcome |

Side effect to review: the default model entries at tier 3 (Opus, the coding CLIs) are now mid. Code on the M5 branch that caps generic escalation below the frontier (`class_of(m) < Frontier`) therefore now permits them. That is a consequence of the policy, not a change made here.

## Commands

```
cargo run -p backspace-core --example router_eval_report -- docs/cauldron/router-eval-v2/router-eval-v2.jsonl --threshold 0.7
cargo run -p backspace-core --example router_eval_report -- docs/cauldron/router-eval-v2/router-eval-v2.jsonl --threshold 0.7 --no-length
cat router-eval-v2.jsonl router-eval-v2-stress.jsonl > primary+stress.jsonl   # then the same two commands
cargo run -p backspace-core --example router_eval_report -- docs/cauldron/router-eval-v2/router-eval-v2-excluded.jsonl --controls [--no-length]
target/debug/backspace-cli eval docs/cauldron/router-eval-v2/router-eval-v2.jsonl --threshold 0.7
cargo test -p backspace-core --test router_eval_v2
```
