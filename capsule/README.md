# colony

Colony is a VM console with an agent chat, for Windows. Spin up Linux or Windows VMs, talk
to an agent in a side chat panel, or fire off a headless task from the home
page — visor provisions the right VM, runs the task, collects the result and
tears the VM down, all in the background.

## Layout

```
core/     Rust — visor-core CLI (Hyper-V + WSL2 providers, image cache, cloud-init seeds)
agentd/   Go   — API, agent runtime, task engine, SQLite, embedded UI (no build step)
```

## How Workbench uses this

Workbench (the Rust repo this folder sits in) drives **only visor-core's JSON
CLI**, through the `backspace-colony` crate at `crates/colony/`: `VisorCli`
shells out to `capabilities` / `create` / `start` / `stop` / `delete` / `list` /
`status` / `exec`, parses stdout as JSON, and turns stderr's `{"error": "..."}`
into the error message verbatim. Workbench keeps its own workspace registry
(`colony.json`, one per colony directory) and its own agent-facing tools
(`colony_create`, `colony_exec`, `colony_list`, `colony_status`,
`colony_destroy`).

Exactly which parts the Workbench integration uses today:

- `core/` (visor-core) — the CLI contract, and inside it the **WSL2 provider**
  and `exec` (which visor-core supports for WSL workspaces only).
- The **Hyper-V provider** is used as far as it works, but it needs Hyper-V
  enabled and an elevated host; nothing in Workbench tests it.
- `docs/COLONY.md` in the repo root — what the crate implements, what is
  tested, and what is still aspirational.

What the Workbench integration does **not** use: `agentd/` (the Go API, chat,
task engine and embedded UI on :7700) is untouched by Workbench. And nothing in
`crates/core` dispatches Colony's tools yet — the crate is wired but not yet
called from the app.

## Status

**Working today (tested end-to-end):**
- `visor-core`: capabilities / create / start / stop / delete / list / status / exec — JSON CLI
- WSL2 provider: clone an existing distro (`--os clone:Ubuntu`) or download the Ubuntu 24.04 cloud rootfs
- `agentd`: workspace CRUD API, chat with agent over SSE, headless task engine
  (provision → agent loop → result → teardown), Anthropic + OpenAI-compatible
  (incl. Ollama) LLM providers, SQLite persistence
- Remote connections in the same workspace rail: SSH remotes can be used by
  the agent over a configured private key, and RDP remotes launch the native
  Windows Remote Desktop client with `mstsc.exe`
- Embedded web UI at `http://localhost:7700`: home (headless tasks), console
  (workspaces + chat), settings

**Written, needs Hyper-V enabled to test:**
- Hyper-V provider: Ubuntu 24.04 cloud image → differencing VHDX + cloud-init
  NoCloud seed (FAT32 VHD, label CIDATA), Gen 2 VMs on Default Switch,
  IP discovery via `Get-VMNetworkAdapter`, SSH exec backend

**Not yet:**
- Windows guests (autounattend.xml + OpenSSH server)
- xterm.js embedded console, guest agent (screenshot/click/type), snapshots, MCP server

## Prerequisites

- Windows 10/11. WSL2 for the fast path; **Hyper-V** for full VMs
- `visor-core.exe` next to `agentd.exe` or on PATH (or set `VISOR_HOME`-relative `VISOR_CORE`)
- An LLM: Anthropic API key, or Ollama / any OpenAI-compatible endpoint

### Enabling Hyper-V (one-time, admin PowerShell, requires reboot)

```powershell
Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All
```

`agentd` must run elevated to use Hyper-V workspaces. WSL2 workspaces work
without elevation.

## Quick start

```powershell
cargo build --release --manifest-path core\Cargo.toml
cd agentd
go build -o agentd.exe .
.\agentd.exe   # serves http://localhost:7700
```

Open http://localhost:7700, set your LLM provider in **Settings** (for Ollama:
provider `openai-compatible`, base URL `http://localhost:11434/v1`, model e.g.
`gemma4:latest`), then type a task on the home page.

## CLI usage (visor-core)

```
visor-core capabilities
visor-core create --provider wsl --name dev --os clone:Ubuntu
visor-core create --provider wsl --name dev --os ubuntu-24.04   # downloads rootfs
visor-core create --provider hyperv --name vm1 --os ubuntu-24.04 --ssh-key $env:LOCALAPPDATA\visor\keys\id_ed25519.pub
visor-core start dev; visor-core exec dev bash -lc "uname -a"
visor-core stop dev; visor-core delete dev
```

## API sketch

```
GET/POST /api/workspaces            GET /api/capabilities
POST /api/workspaces/{n}/start|stop DELETE /api/workspaces/{n}
GET/POST /api/remotes               POST /api/remotes/{id}/test|open
POST /api/chat                      SSE stream of agent events
POST /api/tasks                     headless task; GET /api/tasks/{id}/events (SSE tail)
GET/PUT /api/settings
```

Data (settings, SQLite, SSH keypair, image cache) lives in
`%LOCALAPPDATA%\visor` (override with `VISOR_HOME`).
