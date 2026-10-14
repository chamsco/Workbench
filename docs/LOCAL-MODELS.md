Local tiny models for sub-agents, on this machine

Audited 16 Oct 2026 (hardware, runtimes and registry tags checked directly).
Companion to `docs/PLAN-NEW-FEATURES.md` item 7 and `crates/core/src/model_server.rs`.

## This machine

| | |
|---|---|
| CPU | AMD Ryzen 7 7800X3D, 8 cores / 16 threads |
| RAM | 31.2 GiB |
| GPU | AMD Radeon RX 6950 XT, ~16 GiB VRAM |
| Backend | **Vulkan** — `offloaded 43/43 layers to GPU`, 8B Q4 measured at 91 tok/s |
| Ollama | 0.40.0, listening on 11434 |

ROCm/HIP is off (`AMD driver is too old` on 32.0.21045.5002); Vulkan carries
inference. Everything below fits in 15.2 GiB of available VRAM, so nothing needs
CPU offload here.

Windows' `AdapterRAM` reported 4 GiB for a 16 GiB card because the field wraps
above 4 GiB. The real figure came from the registry and from Ollama's own log.

## What Workbench sees

`backspace-cli models` after pulling two:

```
ollama  http://localhost:11434  running
    granite3.2:2b   tiny  1.5 GB
    qwen3:1.7b      tiny  1.4 GB
    gemma4:latest          9.6 GB
small jobs could run on qwen3:1.7b at http://localhost:11434
```

`is_tiny` counts a model at or under 4B parameters. The installed 8B gemma4 is
not tiny, so before these pulls `recommend()` offered gemma4 for delegated work
even though the codebase would never pick it for a small job.

## Pull these

```powershell
ollama pull qwen3:1.7b        # 1.4 GB — best all-round tiny; on the Extract and Summarize lists
ollama pull granite3.2:2b     # 1.5 GB — Classify; strong labels and JSON
ollama pull gemma3:1b         # 0.75 GB — fastest with real quality
ollama pull llama3.2:3b       # 1.9 GB — dependable generalist
ollama pull qwen3:4b          # 2.3 GB — the largest still counted tiny
ollama pull phi4-mini:3.8b    # 2.3 GB — tool use
ollama pull smollm2:360m      # 0.68 GB — CPU-only fallback
```

The first two are pulled. `qwen3:1.7b` is the default recommendation: it is on
both the Extract and the Summarize preference lists in `model_server.rs`.

**`lfm2.5:2.6b` does not exist.** The tag is hard-coded at `model_server.rs:631`
and in tests, and it 404s, as do `lfm2:2.6b` / `lfm2:1.2b` / `lfm2:700m`. The
only LFM2.5 tag, `lfm2.5:latest`, is an 8.5B MoE (4.8 GB), which `is_tiny`
rejects. That preference is therefore unsatisfiable as written and should be
swapped for a real family.

## Wiring a chosen model to the jobs

Settings → Local models picks the model and ticks which jobs take it: **scouts**
(read-only lookups, the safe default), **triage** (classify a filed ticket),
**workers** (build a ticket). `prefs.subagent_model` + `prefs.local_tasks` feed
`harness::local_jobs` → `Router::route_for`.

Two things to know since 16 Oct:

- Workers were never actually routed locally: `run_worker_with` called
  `router.route()`, which ignores the local picks. Fixed; `route_for` is used,
  so the workers tick does what it says.
- A runtime that is not running used to be handed work anyway, because a
  keyless loopback provider counts as usable. `route_for` now checks something
  is listening (400 ms TCP connect, cached 15 s) and falls back with an error
  that names the model it skipped.

## Not done

- No embedding model is pulled, so `memory` search is unaffected but there is no
  local semantic search.
- MLX is macOS-only and absent here; LM Studio and llama.cpp servers are absent
  too. `model_server.rs` discovers all four, so installing any of them needs no
  code change.
- Ollama listens on `OLLAMA_HOST=http://0.0.0.0:11434`, i.e. every interface,
  not loopback only. Worth changing to `127.0.0.1:11434` unless something else
  on the LAN needs it.
