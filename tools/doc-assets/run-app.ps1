<#
  run-app.ps1 — launch the real Tauri shell against the deterministic mock model
  and photograph it through the WebView2 DevTools port.

  This is the honest version of the tour: not the design mock, the app, running
  a scripted six-ticket workload with four agents working in parallel. Everything
  is isolated — its own workspace, its own prefs and data dir — so nothing here
  touches the real ~/.config/backspace or your projects.

  Example:
    ./run-app.ps1 -Plan tools/doc-assets/app-plan.json -Out target/app-shots
    ./run-app.ps1 -Dump          # what is clickable in the running app?
#>
[CmdletBinding()]
param(
  [string]$Plan,
  [string]$Out = "target/app-shots",
  [int]$Port = 8777,
  [int]$DebugPort = 9222,
  [int]$Layout = 4,
  [int]$Steps = 12,
  [double]$Think = 0.35,
  [int]$SettleBeforePlan = 6,
  [switch]$Dump,
  [switch]$KeepRunning
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$app = Join-Path $root "target/release/backspace-tauri.exe"
if (-not (Test-Path $app)) {
  throw "No $app. Run: cargo build --release -p backspace-tauri"
}
$exe = Get-Item $app
Write-Host ("app built {0}" -f $exe.LastWriteTime)

$demo = Join-Path $root "target/app-demo"
Remove-Item -Recurse -Force $demo -ErrorAction SilentlyContinue
$ws = Join-Path $demo "ws"
New-Item -ItemType Directory -Force -Path $ws, (Join-Path $demo "data"), (Join-Path $root $Out) | Out-Null

# The same config bench.py uses: heuristic router, auto-approve, one mock provider.
$config = @"
[router]
backend = "heuristic"
jev_endpoint = ""
jev_model = ""
jev_api_key_env = "NONE"
jev_usd_per_mtok = 0.0
main_min_effort = "high"

[orchestrator]
max_parallel_calls = 4
max_subagents = 8
auto_approve = true
bash_timeout_secs = 10

[providers.mock]
kind = "openai"
base_url = "http://127.0.0.1:$Port/v1"
api_key_env = "NONE"

[[models]]
id = "mock-cheap"
provider = "mock"
tier = 1
description = "cheap"
input_usd_per_mtok = 1.0
output_usd_per_mtok = 2.0
max_output_tokens = 1000

[[models]]
id = "mock-strong"
provider = "mock"
tier = 4
description = "strong"
input_usd_per_mtok = 10.0
output_usd_per_mtok = 20.0
max_output_tokens = 1000
efforts = ["low", "medium", "high", "xhigh", "max"]
"@
Set-Content -Path (Join-Path $ws "backspace.toml") -Value $config -Encoding UTF8

$done = Join-Path $demo "done"
$ready = Join-Path $demo "ready"

# ---------------------------------------------------------------- mock model
$mock = Start-Process -FilePath "python" -PassThru -WindowStyle Hidden `
  -ArgumentList (Join-Path $root "bench/mock.py") `
  -WorkingDirectory $root `
  -Environment @{ MOCK_PORT = "$Port"; MOCK_STEPS = "$Steps"; MOCK_THINK = "$Think"; DONE_FILE = $done }
Start-Sleep -Milliseconds 800
if ($mock.HasExited) { throw "mock.py exited immediately" }
Write-Host "mock model on http://127.0.0.1:$Port (pid $($mock.Id))"

# ------------------------------------------------------------------- the app
$env:BACKSPACE_GOAL = "Build a todo app with an API and a web UI"
$env:BACKSPACE_LAYOUT = "$Layout"
$env:BACKSPACE_PREFS = Join-Path $demo "prefs.json"
$env:BACKSPACE_DATA = Join-Path $demo "data"
$env:BACKSPACE_READY_FILE = $ready
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$DebugPort"

$t0 = Get-Date
$proc = Start-Process -FilePath $app -PassThru -ArgumentList $ws
Write-Host "app pid $($proc.Id), waiting for first frame..."

$deadline = (Get-Date).AddSeconds(60)
while (-not (Test-Path $ready)) {
  if ($proc.HasExited) { throw "the app exited with $($proc.ExitCode)" }
  if ((Get-Date) -gt $deadline) { throw "no first frame after 60 s" }
  Start-Sleep -Milliseconds 150
}
Write-Host ("first frame after {0:N0} ms" -f ((Get-Date) - $t0).TotalMilliseconds)

try {
  if ($Dump) {
    Start-Sleep -Seconds $SettleBeforePlan
    node (Join-Path $PSScriptRoot "cdp-shoot.mjs") "--port=$DebugPort" --dump
  } else {
    Start-Sleep -Seconds $SettleBeforePlan
    $planArg = if ($Plan) { (Resolve-Path $Plan).Path } else { Join-Path $PSScriptRoot "app-plan.json" }
    node (Join-Path $PSScriptRoot "cdp-shoot.mjs") "--port=$DebugPort" "--out=$Out" "--plan=$planArg"
  }
} finally {
  if (-not $KeepRunning) {
    Get-Process -Id $proc.Id -ErrorAction SilentlyContinue | ForEach-Object { $_.CloseMainWindow() | Out-Null }
    Start-Sleep -Seconds 2
    Get-CimInstance Win32_Process -Filter "ParentProcessId=$($proc.Id)" -ErrorAction SilentlyContinue |
      ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    Stop-Process -Id $mock.Id -Force -ErrorAction SilentlyContinue
    Write-Host "stopped app and mock"
  }
}
