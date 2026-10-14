# Colony — a durable workspace per agent

Colony is the part of Workbench that gives an agent **a computer of its own**
instead of the host it happens to be running on. A workspace is a WSL2 distro
or a Hyper-V VM, created when a task needs one, addressable by name in chat,
and deleted when the task is done.

This document is the honest state of it: what exists, what is tested, and what
is still a sketch. It is written for whoever picks this up next — including the
agent reading it.

---

## Where it lives

```
crates/colony/          Rust library — the Workbench-side colony (this doc)
  src/model.rs            Workspace, State, CreateSpec, ExecOut, Capabilities
  src/backend.rs          Backend trait, VisorCli, parse_error, create_args
  src/colony.rs           Colony registry, colony.json, lifecycle
  src/fake.rs             FakeBackend — the same trait, in memory
  src/prompt.rs           chat_prompt(): the agent's system prompt
  src/tools.rs            to_tools(): the five tool schemas
capsule/                The pre-existing Colony project (visor + agentd)
  core/                   visor-core — JSON CLI over Hyper-V and WSL2
  agentd/                 Go API, agent runtime, task engine, embedded UI :7700
  README.md               what capsule does today
docs/COLONY.md          this file
```

`crates/colony` is a normal member of the root cargo workspace
(`cargo test -p backspace-colony`). `capsule/core` is a *separate* cargo
workspace with its own lockfile, so visor-core builds standalone and does not
drag the Workbench tree into its dependency graph.

The two halves talk over one narrow contract: visor-core's stdout is JSON and
its stderr is `{"error": "..."}`. `crates/colony` owns the registry and the
agent-facing surface; `capsule` owns the hypervisor.

---

## The landscape

Every "give the agent a machine" product is a choice of two axes: **what the
unit is** (a sandboxed process, a container, a VM) and **how long it lives**
(one call, one session, one project). Colony is in this space:

| Product | Unit | Lifetime | Link |
| --- | --- | --- | --- |
| Cloudflare Durable Objects | single-threaded object + storage | per object, effectively forever | [developers.cloudflare.com/durable-objects](https://developers.cloudflare.com/durable-objects/) |
| Cloudflare Containers | container bound to a Durable Object | per object, scaled to zero | [developers.cloudflare.com/containers](https://developers.cloudflare.com/containers/) |
| E2B | Firecracker microVM sandbox | per session (seconds to hours) | [e2b.dev/docs](https://e2b.dev/docs) |
| Fly Machines | microVM (Firecracker) | per request, auto stop/start | [fly.io/docs/machines](https://fly.io/docs/machines/) |
| Modal Sandboxes | container with a filesystem | per call, snapshottable | [modal.com/docs/guide/sandboxes](https://modal.com/docs/guide/sandboxes) |
| AWS Bedrock AgentCore | managed agent runtime + code interpreter/browser sandboxes | per session | [aws.amazon.com/bedrock/agentcore](https://aws.amazon.com/bedrock/agentcore/) |
| Firecracker | microVM | whatever you build | [firecracker-microvm.github.io](https://firecracker-microvm.github.io/) |
| Cloud Hypervisor | microVM/VMM | whatever you build | [cloudhypervisor.org](https://www.cloudhypervisor.org/) |
| Apple Virtualization.framework | VM on macOS | whatever you build | [developer.apple.com/documentation/virtualization](https://developer.apple.com/documentation/virtualization) |
| WSL2 | distro sharing the host kernel | until you unregister it | [learn.microsoft.com/windows/wsl](https://learn.microsoft.com/en-us/windows/wsl/) |
| Hyper-V | full VM on Windows | until you remove it | [learn.microsoft.com/hyper-v](https://learn.microsoft.com/en-us/virtualization/hyper-v-on-windows/) |

What the hosted ones all have and Colony does not: many hosts, autoscaling,
per-second billing, snapshot/restore, and a network that makes a sandbox
reachable from anywhere. What Colony has: it is the machine the user already
owns, with the user's repositories, GPU, and secrets on it, and no per-hour
bill.

A fuller survey of this landscape — pricing, limits, snapshot semantics, with
sources — is in `docs/DURABLE-OBJECTS-2026-10.md`. This document is only about
what Colony does with the idea.

### The naming: durable object

The *durable object* idea (from Cloudflare's Durable Objects) is that identity
lives in a named, single-instance place that persists while the compute around
it comes and goes. Colony borrows exactly that part and nothing else:

- The durable object is **`colony.json`** — a named registry of workspaces,
  their provider, OS, state, creation time and purpose, in one directory per
  colony.
- The compute is disposable: the distro or VM behind a workspace can be
  created, stopped and deleted at any time.
- The name is the address. Chat says "run it in `dev`"; the agent with the
  `colony_exec` tool runs it in `dev`.

Colony is not a distributed object store, has no garbage collector, and runs on
one host.

---

## The design

**One workspace per agent or per project.** Per agent when the point is
isolation (a coding agent that breaks its own `/usr` should not break the one
next to it). Per project when the point is a shared, long-lived environment
(dependencies installed once, many agents, `colony_status` tells everyone what
is up). The registry does not care; it is a flat list of names in one colony
directory. A colony is deliberately a *directory*, so a per-project colony is
`<project>/.colony/` and a per-user one is under the data dir — the caller
chooses by choosing `dir`.

**Addressable by name.** Every tool takes `name`. `chat_prompt(ws)` tells the
agent what its own workspace is called and that the user can refer to it that
way, so "deploy it in `staging`" means workspace `staging`.

**Disposable.** `colony_destroy` is one call and is not undoable. Scratch work
belongs in the workspace and nowhere else; anything worth keeping goes back to
the repository or a script. This is the whole point: an agent can be brave
about `apt`, system config, and deleting directories because the machine
underneath is expendable.

**One seam for real and fake.** Everything the registry does goes through
eight methods:

```rust
pub trait Backend: Send + Sync {
    fn capabilities(&self) -> Result<Capabilities>;
    fn create(&self, spec: &CreateSpec) -> Result<Workspace>;
    fn start(&self, name: &str) -> Result<Workspace>;
    fn stop(&self, name: &str, force: bool) -> Result<Workspace>;
    fn delete(&self, name: &str) -> Result<()>;
    fn list(&self) -> Result<Vec<Workspace>>;
    fn status(&self, name: &str) -> Result<Workspace>;
    fn exec(&self, name: &str, cmd: &[String]) -> Result<ExecOut>;
}
```

`VisorCli` implements it by shelling out to `visor-core`; `FakeBackend`
implements it in memory and records every call. Tests take the same code path
as production, minus the VM.

**The backend is the truth about existence.** `Colony::list()` asks the
backend and adopts what it reports; the registry keeps colony's own fields
(`created`, `task`) by name. An operation on a name the registry has never
seen is an error, not an inference — run `colony_list` first.

**Errors are the hypervisor's own words.** visor-core exits 1 with
`{"error": "..."}` on stderr; `parse_error()` extracts it and `VisorCli`
returns it verbatim, so an agent that tries `colony_exec` on a Hyper-V
workspace reads "exec via visor-core is only for wsl workspaces; use SSH for
hyperv" rather than a wrapped Rust error.

---

## Implemented and tested

Verified on Windows with rustc/cargo 1.97.1.

| Piece | State |
| --- | --- |
| `Workspace`, `State`, `CreateSpec`, `ExecOut`, `Capabilities`, `ProviderKind` (serde, lowercase) | implemented + unit-tested |
| `Colony`: `open`, `with_backend`, `list`, `get`, `create`, `start`, `stop`, `status`, `exec`, `delete`, `save`, `load`, `path`, `workspaces`, `capabilities` | implemented + unit-tested over `FakeBackend` |
| `colony.json` persistence, provider remembered, corrupt file is a clean error, missing file is an empty colony | implemented + unit-tested |
| `Backend` trait; `FakeBackend` (records calls and argv) | implemented + unit-tested |
| `VisorCli`: discovery (PATH → `VISOR_CORE` → beside the exe), JSON stdout, `{"error": …}` stderr, exec WSL-only pass-through | implemented; parsing and argv unit-tested; **subprocess execution is not covered by any automated test** |
| `create_args`: the exact `visor-core create` argv | implemented + unit-tested |
| `chat_prompt` | implemented + unit-tested |
| `to_tools` — `colony_create`, `colony_exec`, `colony_list`, `colony_status`, `colony_destroy` | implemented + unit-tested (names, descriptions, required args) |
| visor-core (WSL2 provider) in `capsule/core` | pre-existing; working per `capsule/README.md` |
| visor-core (Hyper-V provider) in `capsule/core` | pre-existing; written, needs Hyper-V enabled to test |
| agentd (Go server, chat, task engine, `:7700` UI) in `capsule/agentd` | pre-existing; not used by Workbench yet |

Test count: **25 unit tests + 1 doc-test**, all green, with no VM, no WSL, no
Hyper-V and no network. They live in `src/*.rs` next to the code they test.

The CLI contract was checked by hand against a freshly built visor-core (this
is not part of `cargo test`, and nothing in the test suite invokes it):

```console
$ capsule\core\target\debug\visor-core.exe capabilities
{
  "elevated": false,
  "hyperv": false,
  "wsl": true
}
$ capsule\core\target\debug\visor-core.exe list
[]
$ capsule\core\target\debug\visor-core.exe status nosuch
{"error":"workspace 'nosuch' not found (no metadata): ..."}   # exit 1, stderr
```

## Not implemented (aspirational)

Say none of this is done:

- **Workbench wiring.** Nothing in `crates/core` calls Colony yet. There is no
  dispatcher for the five tools, no injection of `chat_prompt`, and no
  board/chat hook that lets one agent address another agent's workspace. The
  crate is a library with tests; it is not in the app.
- **Auto-provisioning.** No policy decides when an agent gets a workspace, how
  big, or which provider. A human (or the harness, later) passes a
  `CreateSpec`.
- **Hyper-V ergonomics.** `exec` is WSL-only, because visor-core says so;
  Hyper-V workspaces need the SSH path, which Colony does not implement.
  Windows guests are not supported by visor-core at all.
- **Pause, snapshots, resume.** A stopped WSL distro keeps its disk; there is
  no snapshot, clone, restore or "warm pool".
- **Garbage collection.** Nothing reaps orphaned workspaces, images or
  differencing disks. `colony_destroy` is the only cleanup.
- **Concurrency and locking.** `colony.json` is written whole. Two processes
  editing one colony directory will lose an update. One writer at a time.
- **Remote hosts / other providers.** No Firecracker, Cloud Hypervisor, Apple
  Virtualization.framework, Kubernetes or cloud sandbox backend. `ProviderKind`
  has exactly `wsl` and `hyperv`.
- **Cost, quotas, identity.** No per-workspace budget, no record of which human
  or agent created it, no audit log.
- **Reusing capsule's agentd API/UI.** Colony drives the CLI, not the Go
  server; the embedded `:7700` console and its chat are untouched by this work.

The honest summary: the registry, the seam, the CLI client and the agent-facing
text are done and tested; the product around them is not.

---

## Build and run

From the repository root (`C:\Users\chamy\Documents\code\Workbench`):

```powershell
# the Workbench-side crate
cargo test  -p backspace-colony
cargo build -p backspace-colony

# visor-core (separate cargo workspace, own lockfile)
cargo build --manifest-path capsule\core\Cargo.toml

# what the host can do
capsule\core\target\debug\visor-core.exe capabilities

# a workspace, by hand
capsule\core\target\debug\visor-core.exe create --provider wsl --name demo --os clone:Ubuntu
capsule\core\target\debug\visor-core.exe exec demo bash -lc "uname -a"
capsule\core\target\debug\visor-core.exe stop demo
capsule\core\target\debug\visor-core.exe delete demo
```

`Colony::open(dir)` finds `visor-core` on `PATH`, then via `VISOR_CORE` (a
binary, or a directory containing one), then next to the current executable.
`Colony::with_backend(dir, provider, backend)` skips discovery entirely — that
is how tests and other hosts use it.

Hyper-V workspaces additionally need Hyper-V enabled and an elevated process;
WSL workspaces do not.

## Public API

```rust
// registry
Colony::open(dir) -> Result<Colony>
Colony::with_backend(dir, provider: ProviderKind, backend: impl Backend + 'static) -> Result<Colony>
colony.list() / get(name) / workspaces() / path() / capabilities()
colony.create(&CreateSpec) -> Result<Workspace>
colony.start(name) / stop(name, force) / status(name) -> Result<Workspace>
colony.exec(name, &[String]) -> Result<ExecOut>
colony.delete(name) -> Result<()>
colony.save() / load()

// backends
trait Backend { capabilities, create, start, stop, delete, list, status, exec }
VisorCli::at(path) / VisorCli::discover() -> Result<VisorCli>
FakeBackend::new() / with_capabilities(caps) / seed(ws) / calls() / execs()

// agent-facing
chat_prompt(&Workspace) -> String
to_tools() -> Vec<serde_json::Value>        // {"name", "description", "schema"}

// parsing helpers (pure, used by VisorCli)
parse_error(stderr: &str) -> Option<String>
workspace_from_cli(&Value) -> Result<Workspace>
create_args(&CreateSpec) -> Vec<String>
```

`to_tools()` mirrors `crates/core/src/provider.rs::ToolDef` (`name`,
`description`, `schema`); the harness maps each entry into a `ToolDef` or into
a provider's tool body.

## Decisions worth keeping

- **`colony.json`, not a database.** It is small, human-readable, diffable and
  survives a crash. If a colony ever holds hundreds of workspaces, revisit.
- **Lossy state mapping, on purpose.** visor-core says `off`, `paused`,
  `saved`, `missing`; Colony has five states. `paused`/`saved` read as
  `stopped` (not running), `missing` reads as `failed` (the backend lost the
  thing). Everything unknown reads as `failed` rather than being guessed at.
- **The CLI never learns our bookkeeping.** `created` and `task` live in
  `colony.json`; `visor-core`'s `CreateSpec` and `Workspace` are untouched.
- **No tokio, no reqwest.** Driving a CLI is `std::process`; adding an async
  runtime to a library that shells out would buy nothing.
- **`exec` takes argv, not a shell string.** The caller decides whether to
  involve a shell (`["bash", "-lc", "..."]`), which keeps quoting honest.
