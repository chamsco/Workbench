<#
  run-app.ps1 — launch the real Tauri shell against the deterministic mock model
  and photograph it through the WebView2 DevTools port.

  Not the design mock: the app, running a scripted six-ticket workload with four
  agents in parallel. Everything is isolated — its own workspace, its own prefs
  and data dir — so nothing here touches the real ~/.config/backspace or your
  projects.

  It deliberately does NOT set BACKSPACE_READY_FILE or BACKSPACE_LAYOUT: those
  put the shell into its bench mode, which adds a tab literally named "Bench".
  A crafted prefs.json gives the same starting layout and looks like a session.

  Example:
    ./run-app.ps1 -Plan tools/doc-assets/app-plan.json -Out target/app-shots
    ./run-app.ps1 -Dump -Text -SettleBeforePlan 14
#>
[CmdletBinding()]
param(
  [string]$Plan,
  [string]$Out = "target/app-shots",
  [int]$Port = 8777,
  [int]$DebugPort = 9222,
  [int]$Steps = 12,
  [double]$Think = 0.35,
  [int]$SettleBeforePlan = 8,
  [switch]$Dump,
  [switch]$Text,
  [switch]$Gated,
  [switch]$KeepRunning
)

$ErrorActionPreference = "Stop"

# With -Gated the run stops for a human: the plan waits for approval, and so does
# each ticket. That is what produces a populated Diagram review, because the
# diagram only has something to draw once a review is actually pending.
$autoApprove = if ($Gated) { "false" } else { "true" }
$planApproval = if ($Gated) { "true" } else { "false" }
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$app = Join-Path $root "target/release/backspace-tauri.exe"
if (-not (Test-Path $app)) {
  throw "No $app. Run: cargo build --release -p backspace-tauri"
}
Write-Host ("app built {0}" -f (Get-Item $app).LastWriteTime)

$demo = Join-Path $root "target/app-demo"
Remove-Item -Recurse -Force $demo -ErrorAction SilentlyContinue
$ws = Join-Path $demo "ws"
$data = Join-Path $demo "data"
$prefs = Join-Path $demo "prefs.json"
New-Item -ItemType Directory -Force -Path $ws, $data, (Join-Path $root $Out) | Out-Null

# The config bench.py uses, plus the two gates that would otherwise stop the run
# and wait for a human: grill (interview rounds) and plan approval.
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
auto_approve = $autoApprove
bash_timeout_secs = 10

[workflow]
grill = false
plan_approval = $planApproval
main_skills = ["grilling", "to-tickets"]
worker_skills = ["ponytail"]
triage_skills = ["triage"]

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

# A real-looking prefs file: already onboarded, dark, straight into the
# Workbench, one project, one tab holding four canvases (the 2x2 grid).
#
# The layout has to go in tabs_by_project: Prefs::tabs_for() answers a project
# that has no entry of its own with TabSpec::defaults(), NOT with the global
# tabs, so a global "tabs" would be ignored. The key is Prefs::tab_key().
$tabKey = ($ws -replace '\\', '/').TrimEnd('/').ToLower()
$runTab = [ordered]@{
  name   = "todo-app"
  layout = $null
  panes  = @(
    [ordered]@{ kind = "agent"; agent = 0; url = $null },
    [ordered]@{ kind = "agent"; agent = 1; url = $null },
    [ordered]@{ kind = "agent"; agent = 2; url = $null },
    [ordered]@{ kind = "agent"; agent = 3; url = $null }
  )
}
$prefsObj = [ordered]@{
  theme         = "dark"
  check_updates = $false
  onboarded     = $true
  mode          = "code"
  code_view     = "agents"
  wb_view       = "canvases"
  uses          = @("code")
  motion        = "full"
  memory_on     = $true
  memory_auto   = $false
  projects      = @($ws)
  active_tab    = 0
  tabs          = @($runTab)
  tabs_by_project = [ordered]@{ $tabKey = [ordered]@{ tabs = @($runTab); active = 0 } }
}
$prefsObj | ConvertTo-Json -Depth 10 | Set-Content -Path $prefs -Encoding UTF8

$done = Join-Path $demo "done"

# ---------------------------------------------------------------- mock model
$mock = Start-Process -FilePath "python" -PassThru -WindowStyle Hidden `
  -ArgumentList (Join-Path $root "bench/mock.py") -WorkingDirectory $root `
  -Environment @{ MOCK_PORT = "$Port"; MOCK_STEPS = "$Steps"; MOCK_THINK = "$Think"; DONE_FILE = $done }
Start-Sleep -Milliseconds 800
if ($mock.HasExited) { throw "mock.py exited immediately" }
Write-Host "mock model on http://127.0.0.1:$Port (pid $($mock.Id))"

# ------------------------------------------------------------------- the app
$env:BACKSPACE_GOAL = "Build a todo app with an API and a web UI"
$env:BACKSPACE_PREFS = $prefs
$env:BACKSPACE_DATA = $data
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$DebugPort"

$proc = Start-Process -FilePath $app -PassThru -ArgumentList $ws
Write-Host "app pid $($proc.Id)"

try {
  if ($Dump) {
    Start-Sleep -Seconds $SettleBeforePlan
    $dumpArgs = @("--port=$DebugPort", "--dump")
    if ($Text) { $dumpArgs += "--text" }
    node (Join-Path $PSScriptRoot "cdp-shoot.mjs") @dumpArgs
  } else {
    Start-Sleep -Seconds $SettleBeforePlan
    $planArg = if ($Plan) { (Resolve-Path $Plan).Path } else { Join-Path $PSScriptRoot "app-plan.json" }
    node (Join-Path $PSScriptRoot "cdp-shoot.mjs") "--port=$DebugPort" "--out=$Out" "--plan=$planArg"
  }
} finally {
  if (-not $KeepRunning) {
    Get-CimInstance Win32_Process -Filter "ParentProcessId=$($proc.Id)" -ErrorAction SilentlyContinue |
      ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    Stop-Process -Id $mock.Id -Force -ErrorAction SilentlyContinue
    Write-Host "stopped app and mock"
  } else {
    Write-Host "left running: app $($proc.Id), mock $($mock.Id)"
  }
}
