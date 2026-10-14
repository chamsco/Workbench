# Durable objects / tiny VMs an agent can be given — landscape, 14 Oct 2026

Every claim links to its source. `[secondary]` marks facts taken from a third-party roundup rather than a vendor page. Unverified items are listed in §7.

---

## 1. Cloudflare

**Durable Object (DO)** — a uniquely-named Worker instance with attached strongly-consistent storage, auto-provisioned near the first request, shut down when idle ([overview](https://developers.cloudflare.com/durable-objects/)). Storage is SQLite (`sql.exec`), GA on Free and Paid. Limits ([1](https://developers.cloudflare.com/durable-objects/platform/limits/)): 10 GB/object, 500 classes (Paid), 2 MB key+value, 30 s CPU/request default (`limits.cpu_ms`, up to 5 min), alarms 15 min wall time — but **DO RPC/HTTP has no wall-time limit** while a caller stays connected. WebSocket Hibernation evicts the DO from memory while keeping sockets open. Rust via [`workers-rs`](https://github.com/cloudflare/workers-rs).

**Containers** — every container is managed and proxied by a DO; the process runs in a Linux VM, and the sandbox docs call each instance "a microVM with its own kernel and network" ([sandbox overview](https://developers.cloudflare.com/sandbox/)). The DO-side API is the design surface worth copying ([DO Container API](https://developers.cloudflare.com/containers/api/durable-object-container/)):

```js
container.start({ image: container.images.base, entrypoint: ["sleep","infinity"], enableInternet: false });
const p = await container.exec(["node","--version"], { cwd: "/workspace", user: "1000:1000" });
const out = await p.output();          // { stdout, stderr, exitCode }
const snap = await container.snapshotContainer({ name: "before-upgrade" });
await container.setInactivityTimeout(10 * 60 * 1000); // max 6 h
await container.interceptOutboundHttps("api.example.com", worker); // credential-brokering proxy
```

`exec()` runs the executable directly — no shell, no pipes, no expansion. `snapshotContainer()` captures the **writable root filesystem only** (no memory, no processes; entrypoint re-runs), max 20 GB, 30-day TTL refreshed on restore ([limits](https://developers.cloudflare.com/containers/platform/limits/)). `monitor()` blocks eviction up to 15 min; egress interceptors are capped at 128. Docs caveat: under the `durable_object` scheduling policy **every process has root capabilities regardless of the `user` option**.

Instance types run `lite` (1/16 vCPU, 256 MiB, 2 GB) → `standard-4` (4 vCPU, 12 GiB, 20 GB). Pricing ([1](https://developers.cloudflare.com/containers/platform/pricing/)): $5/mo Workers Paid includes 25 GiB-hours memory, 375 vCPU-min, 200 GB-hours disk; overage $0.0000025/GiB-s memory, $0.00002/vCPU-s CPU (active CPU only), $0.00000007/GB-s disk; egress $0.025/GB NA/EU.

**`@cloudflare/sandbox` 1.0.x is no longer the sandbox** — it is helper classes (`Files`, `S3Mount`, `DirectoryBackup`) that drive an already-running container through `exec()` using a statically-linked `sandbox-shim` binary copied from the matching `cloudflare/sandbox:<VERSION>` image; it needs `nodejs_compat` and does not start/stop/monitor the container ([reference](https://developers.cloudflare.com/sandbox/reference/)). Two sandbox environments exist: Containers (Linux microVM) and **Dynamic Workers** — a Worker created at runtime for JS/Python/WASM with `globalOutbound: null`, i.e. V8-isolate isolation.

**Code Mode** — expose an API as typed code the model writes against instead of N tool definitions ("an entire API in 1,000 tokens") ([blog](https://blog.cloudflare.com/code-mode/), [MCP variant](https://blog.cloudflare.com/code-mode-mcp/)). Anthropic cites it by name.

**Durable execution with fibers** is Cloudflare's most copyable primitive ([docs](https://developers.cloudflare.com/agents/runtime/execution/durable-execution/)). DOs are evicted on ~70–140 s inactivity, 1–2×/day code updates, or a 15-minute alarm timeout. `keepAlive()` installs a 30 s alarm heartbeat; `runFiber(name, fn)` registers the task in SQLite *before* running and hands the callback `ctx.stash(data)` — a **synchronous SQLite write that fully replaces** the previous snapshot — and on the next activation calls `onFiberRecovered(ctx)` with the last snapshot. `startFiber({idempotencyKey})` adds a retained status row so a retried webhook is accepted exactly once, plus `inspectFiber`/`listFibers`/`cancelFiber`/`resolveFiber`/`deleteFibers`. `AIChatAgent` and `Think` wrap every chat turn in a fiber. **Project Think** ([blog](https://blog.cloudflare.com/project-think/)) is Cloudflare's agent-runtime announcement built on this; I could not retrieve the post body, so treat details beyond "agent runtime + DO lifecycle hooks" as low-confidence.

## 2. Managed sandbox vendors

| Product | Isolation | State across stop | Local? | License |
|---|---|---|---|---|
| [E2B](https://docs.e2b.dev/sandbox/persistence.md) | Firecracker microVM | `pause()` saves **filesystem + memory**, kept indefinitely | self-host (Terraform, GCP; AWS in progress) | Apache-2.0 [secondary] |
| [Fly Machines](https://docs.fly.io/machines/overview.md) | microVM | rootfs ephemeral; attach Volume; `suspend` snapshots RAM | no | proprietary |
| [Fly Sprites](https://sprites.dev) | Firecracker microVM | 100 GB ext4, no expiry; checkpoints coW ~300 ms | no | proprietary |
| [Modal](https://modal.com/docs/guide/sandbox-snapshots.md) | gVisor [secondary] | fs/dir snapshots (30 d TTL, opt-out), memory snapshots alpha (7 d) | no | proprietary |
| [Daytona](https://www.daytona.io/dotfiles/updates/daytona-is-going-closed-source) | containers/OCI [secondary] | persistent + archivable | **no** since Jun 2026 | proprietary (repo frozen AGPL-3.0) |
| [Runloop](https://docs.runloop.ai/docs/devboxes/snapshots) | devboxes + disk snapshots | snapshots | no | **winding down; support ends 2026-11-13** |
| [Morph](https://cloud.morph.so/docs/documentation/instances/creating-snapshot) | VM | snapshots = immutable memory **+ disk**, used as instance templates | no | proprietary |
| [Vercel Sandbox](https://vercel.com/docs/sandbox/concepts/persistent-sandboxes) | Firecracker microVM, own kernel | stop auto-snapshots **filesystem only**; 64 GB NVMe; 30 d TTL | no | SDK Apache-2.0, platform proprietary |
| [Deno Sandbox](https://docs.deno.com/sandbox/) | Firecracker microVM | ephemeral by default, volumes for durable storage | no | JS SDK MIT, platform proprietary |
| Koyeb Sandboxes | bare-metal microVM [secondary] | session-scoped | no | Apache-2.0 SDK [secondary] |

Notable specifics:

- **E2B** — `Sandbox.create()` → `pause()` → `connect()` resumes in place. Pause costs ~4 s/GiB RAM, resume ~1 s; paused sandboxes have **no TTL** and are deleted only by `kill()`. Continuous runtime caps: 24 h Pro / 1 h Hobby, reset by pause/resume. `lifecycle.onTimeout` defaults to `"kill"`; set `"pause"` plus optional `autoResume` for a durable loop.
- **Fly Sprites** (launched 9 Jan 2026) — `sprite create`, persistent VM, own HTTPS URL that **wakes the VM on the inbound request**, checkpoints mounted at `/.sprite/checkpoints`, supervised "services" with dependency ordering, sleeps after ~30 s idle, scale-to-zero billing; JSON API `PUT /v1/sprites/{name}`, `POST /v1/sprites/{name}/exec`, `POST /v1/sprites/{name}/policy/network` with domain allow/deny; Go/TS SDKs ([Simon Willison](https://simonwillison.net/2026/Jan/9/sprites-dev/), [Fly blog](https://fly.io/blog/code-and-let-live/)).
- **Vercel** — default timeout 5 min, max session 24 h Pro / 45 min Hobby, reset on resume; `onResume` hook exists precisely because a stop does not preserve processes; firewall `transform` rules terminate TLS outside the sandbox and inject credentials so "secrets never enter the sandbox" ([Fly comparison, sourced to Vercel docs](https://fly.io/learn/fly-vs-vercel/)).
- **Deno Sandbox** (beta) — `Sandbox.create({allowNet:["api.openai.com"], secrets:{…}})`; secrets substituted only for approved outbound hosts; limits 768 MB–4 GB RAM, 2 vCPU, ≤30 min lifetime, 10 GB disk, 5 concurrent sandboxes/org; regions `ams`/`ord`.
- **Modal** — `snapshot_filesystem()`, `snapshot_directory()` + `mount_image()`, and experimental memory snapshots that restore running processes but expire in 7 days and currently terminate the source sandbox.

## 3. Hyperscalers and model labs

- **AWS Bedrock AgentCore** (GA 2025-10-13) — Code Interpreter and Browser use one Firecracker microVM per session with nothing persisting across sessions; AgentCore Runtime adds an interactive shell whose state lasts the session [secondary]. Verified API shape: `start_code_interpreter_session(codeInterpreterIdentifier, name, sessionTimeoutSeconds)` — **default 900 s, max 28 800 s (8 h)** — plus `InvokeCodeInterpreter`, `StopCodeInterpreterSession`, mountable S3 Files/EFS access points, and Secrets Manager certificates ([boto3](https://docs.aws.amazon.com/boto3/latest/reference/services/bedrock-agentcore/client/start_code_interpreter_session.html)). SDK Apache-2.0, service proprietary.
- **AWS Lambda MicroVMs** — real, announced June 2026: "Lambda MicroVMs for isolated execution of user and AI-generated code" ([what's-new](https://aws.amazon.com/about-aws/whats-new/2026/06/aws-lambda-microvms/)); boto3 exposes a `LambdaMicroVMs` service. I could not read the announcement body (truncated), so no limits claimed.
- **Google Cloud Run** — gVisor is the isolation layer for Cloud Run, App Engine and Cloud Functions [secondary, [gvisor.dev](https://gvisor.dev/)]. "Worker pools" GA is asserted only by a third-party post — see §7.
- **Anthropic Agent Skills** — a skill is a directory with `SKILL.md` whose YAML frontmatter (`name`, `description`) is preloaded into the system prompt; progressive disclosure has three+ levels (metadata → body → bundled files), and skills may bundle code the model executes as tools ([engineering post](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills), [docs](https://docs.claude.com/en/docs/agents-and-tools/agent-skills/overview), [open standard](https://agentskills.io/)). Security guidance: install only from trusted sources and audit bundled code.
- **Anthropic "Code execution with MCP"** (2025-11-04) — present MCP servers as a filesystem of typed code instead of tool definitions; cuts a worked example from 150 000 to 2 000 tokens (98.7%); intermediate results stay in the execution environment (privacy benefit, PII tokenisation); agents persist state *and their own reusable code* to the workspace; explicitly warns this "requires a secure execution environment with appropriate sandboxing, resource limits, and monitoring" ([post](https://www.anthropic.com/engineering/code-execution-with-mcp), [Claude Code sandboxing](https://www.anthropic.com/engineering/claude-code-sandboxing)).
- **OpenAI** — Codex cloud runs in isolated OpenAI-managed containers: a **setup phase** with network access to install dependencies, then an **agent phase that is offline by default**, with secrets removed before the agent phase. Codex CLI/IDE uses OS-level enforcement: Seatbelt/`sandbox-exec` on macOS, `bwrap`+seccomp on Linux, a native Windows sandbox or the WSL2 Linux sandbox on Windows; default is no network plus workspace-write; `[features.network_proxy]` adds a domain allowlist (deny wins, DNS-rebinding checks, loopback/private blocked by default); `.git`, `.agents`, `.codex` are read-only inside writable roots ([agent approvals & security](https://developers.openai.com/codex/agent-approvals-security)). **AgentKit** (Agent Builder, ChatKit, Connector Registry, Evals) is covered only by secondary press — §7.

## 4. Local-first isolation for a Rust desktop app

| Mechanism | Host | Embeddable from Rust? |
|---|---|---|
| [Firecracker](https://github.com/firecracker-microvm/firecracker) | Linux/KVM only | REST-API process; no Windows/macOS |
| [Cloud Hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor) | KVM **or Microsoft Hypervisor (MSHV)** | Rust project on `rust-vmm` crates; no macOS |
| [libkrun](https://github.com/libkrun/libkrun) | **KVM on Linux, HVF on macOS/ARM64** | yes — it *is* a dynamic library with a stable C API; Apache-2.0 |
| [microsandbox](https://github.com/microsandbox/microsandbox) | libkrun | Apache-2.0 self-hosted Rust server, persistent `./menv` or ephemeral `msx` [secondary] |
| [gVisor](https://gvisor.dev/) | Linux only | no hardware virt needed, supports checkpoint/restore; not on macOS/Windows |
| [Kata Containers](https://katacontainers.io/) | Linux, containerd/CRI-O | VM-isolated containers but needs container orchestration |
| [Apple container](https://github.com/apple/container) | **Apple silicon, macOS 26 only** | Swift CLI + `containerization` package; one Linux VM per container via Virtualization.framework; Apache-2.0; drive it via CLI/XPC |
| WSL2 / Windows Hyper-V | Windows | `wsl --import/--export`; HCS APIs via [hcsshim](https://pkg.go.dev/github.com/Microsoft/hcsshim) |
| Docker Sandboxes (`sbx`) | all three | proprietary, free for individuals, account login required; own VMM against Hypervisor.framework / Windows Hypervisor Platform / KVM [secondary] |

**Verdict.** macOS: libkrun/HVF is the only realistic Rust-embeddable microVM (it powers microsandbox); otherwise shell out to `container` (macOS 26 + Apple silicon) or Lima/Colima. Windows: there is no Firecracker analogue — use WSL2, the Windows Hypervisor Platform directly, or Cloud Hypervisor on MSHV. Cross-platform you either maintain three backends behind one trait, or adopt Docker's approach and write your own VMM against each native hypervisor.

## 5. Durable execution

| Engine | Rust | State store | Embed in a single-user desktop app? |
|---|---|---|---|
| [Temporal](https://temporal.io/blog/build-durable-applications-rust-temporal-rust-sdk-now-generally-available) | **SDK GA 2026-09-23** | Event History | No — needs a server (Cloud or self-hosted). Best docs; macros `#[workflow]`/`#[activity]`, Tokio workers, deterministic timers, nondeterminism detector, `WorkflowReplayer` |
| [Restate](https://github.com/restatedev/restate/blob/main/LICENSE) | [`restate-sdk`](https://crates.io/crates/restate-sdk) | journal/replay | Closest to a sidecar: single binary, but **BSL 1.1**, not OSI-open |
| [DBOS](https://docs.rs/dbos/latest/dbos/) | **`dbos` v0.5.0, MIT, 2026-09-09** | Postgres system DB | Yes if you ship Postgres: workflows, steps, queues, messaging, recovery, client present; **scheduler not yet**; shares one schema with Python/TS/Go/Java |
| Inngest | none found | step state | self-hostable (Helm); license not verified |

The practical read: all four are *services*, not libraries. For a desktop app the cheapest correct answer is to implement the Cloudflare fiber pattern yourself over SQLite — register the task, `stash()` a full snapshot synchronously, expose an `on_recover` hook, dedupe by idempotency key — and keep Temporal's semantics (deterministic replay, idempotent activities) in the interface so you can swap in a real engine later. DBOS-Rust is the only engine that is MIT and structurally embeddable today.

## 6. What Workbench/Backspace should copy — 8 primitives

1. **`Workspace`** — the durable, addressable unit; one per sub-agent or conversation, with a stable name (`{parent}/{name}`) used as the chat address, exactly like a Durable Object's globally-unique name. Everything else hangs off it. The chat *is* the address; a message to a workspace is what wakes it (Sprites' wake-on-request, not Vercel's wake-on-SDK-call).
2. **`Journal`** — one SQLite file per workspace in WAL mode, single-writer, holding an append-only step log plus a snapshot table. Copy Cloudflare's `stash()` contract precisely: **synchronous, full-replace, JSON-serialisable** checkpoints. This is your replay substrate, audit log, and crash recovery in one artifact you can back up by copying a file.
3. **`Fiber`** — `fiber.run(name, idempotency_key, fn)` / `start_fiber` / `on_recover(ctx)` / `inspect` / `cancel`. Adopt Cloudflare's vocabulary verbatim (it is the clearest published naming) and its key insight: **the closure is gone after a crash** — recovery is a hook with a snapshot, not a resumed stack. That single constraint shapes every long-running sub-agent loop.
4. **`Session`** — the compute handle: `{image, vcpu, mem_mb, disk_mb, net_policy, state ∈ created|running|suspended|stopped}` with `start/exec/suspend/resume/snapshot/restore/destroy`. Prefer **memory + disk** snapshots (Sprites, E2B, Morph) since they preserve running processes; always keep a filesystem-only fallback (Cloudflare, Vercel) for cheap portability and tiering.
5. **`Exec`** — a single argv-based primitive, `exec(argv, {cwd, env, pty, timeout, on_stdout}) -> {exit_code, pid}`, **without** implicit shell interpretation (Cloudflare's `exec` does not start a shell). One code path for tools, skills, and the agent's own commands; streams, doesn't buffer.
6. **`Broker`** — the egress and credential boundary: default-deny per-workspace domain allowlist plus a TLS-terminating proxy that substitutes real credentials only for approved destinations, so no workspace ever holds a usable secret. Copy Vercel's `transform` rules, Deno's `allowNet` + per-host secrets, Sprites' domain policy, and Cloudflare's `interceptOutbound*`. This is the highest-leverage safety primitive in the whole survey — every serious vendor converged on it independently.
7. **`Checkpoint`** — user- and agent-visible `create / list / restore <id>`, cheap copy-on-write (Sprites: ~300 ms), with the last N mounted read-only inside the workspace so the agent can diff against its own past. Rolls back a poisoned run without destroying the workspace, which is what makes YOLO-mode agents acceptable.
8. **`Skills`** — `SKILL.md` files *inside the workspace*, loaded by progressive disclosure (frontmatter in prompt, body on demand, bundled files rarely). Ship skills that teach the agent how its own workspace works — Sprites pre-installs exactly this — and let the agent write new skills into the same directory, so capability accumulates per workspace instead of in your binary.

**Rust implementation note.** Put all of 1–8 behind a `WorkspaceBackend` trait with three implementations: `local` (libkrun/microsandbox on macOS and Linux; WSL2 or the Windows Hypervisor Platform on Windows), `cloud` (Fly Sprites, E2B, or Cloudflare Containers — all three expose the same start/exec/snapshot/wake shape), and `mock` for tests. The journal, fiber, broker, and skills layers are backend-independent and should never know which one is underneath.

## 7. Explicitly unverified / caution

- **Runloop is shutting down** — its docs carry a banner: support for existing customers ends 2026-11-13, with migration guides. Do not build on it.
- **AWS Lambda MicroVMs** — the announcement page exists (June 2026) and boto3 ships a `LambdaMicroVMs` client, but the page body was truncated on fetch, so I claim no limits or pricing.
- **Cloud Run "worker pools" GA** — asserted only by a third-party Japanese blog post; no Google primary page verified.
- **OpenAI AgentKit** and the reported **OpenAI Agents API** (hosted Codex harness, optional sandbox, Sept 2026) — secondary coverage only; no primary OpenAI page fetched.
- **Inngest** license/self-host terms, **Temporal server** license, **Cloud Hypervisor** exact license text, **Lima/Colima** macOS `vz` default, **Northflank** isolation details, **Cloudflare Project Think** body, and the **Daytona closed-source announcement** itself were not verified from primary sources this session.
- Facts marked `[secondary]` come from the [awesome-sandbox roundup](https://github.com/restyler/awesome-sandbox) (a maintained community list), not from vendor documentation.
