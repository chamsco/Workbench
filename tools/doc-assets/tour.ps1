<#
  tour.ps1 — build docs/assets/tour.gif: a short, looping tour of the Workbench
  design (design/workbench.html), driven by tools/doc-assets/tour-driver.js.

  Frames are rendered one at a time by shoot.ps1 so each one is reproducible;
  the state the mock shows and the position of the on-screen pointer are
  computed here, not faked with an overlay afterwards.

  Storyboard: Terminals -> Tickets -> Browser -> Diagram (plan review) ->
  PLAN.md -> back to Terminals, so the last frame matches the first.
#>
[CmdletBinding()]
param(
  [int]$Width = 1440,
  [int]$Height = 900,
  [int]$Fps = 10,
  [double]$Seconds = 8.0,
  [int]$GifWidth = 1000,
  [string]$Out = "docs/assets/tour.gif",
  [switch]$KeepFrames
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$html = Join-Path $root "design/workbench.html"
$driver = Join-Path $PSScriptRoot "tour-driver.js"
$frames = Join-Path $root "target/tour-frames"
$tmpOut = Join-Path $root "target/tour-frames/raw"

Remove-Item -Recurse -Force $frames -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $tmpOut | Out-Null

# Each scene: the click that opens it, where the pointer is, and what the mock shows.
$scenes = @(
  [pscustomobject]@{ at = 1.2; cursor = @(786, 48);   query = "env=terminals&side=projects"; label = "Terminals" }
  [pscustomobject]@{ at = 2.4; cursor = @(203, 87);   query = "env=terminals&side=tickets";  label = "Tickets" }
  [pscustomobject]@{ at = 3.6; cursor = @(879, 48);   query = "env=browser&side=projects&btab=main"; label = "Browser" }
  [pscustomobject]@{ at = 4.8; cursor = @(970, 48);   query = "env=diagram&side=projects&review=plan"; label = "Diagram" }
  [pscustomobject]@{ at = 6.0; cursor = @(1045, 48);  query = "env=docs&side=projects";    label = "PLAN.md" }
  [pscustomobject]@{ at = 7.2; cursor = @(786, 48);   query = "env=terminals&side=projects"; label = "Terminals" }
)

$start = @(700, 640)          # where the pointer enters
$ringSeconds = 0.14           # how long the click ripple lives

# The skill's strong ease-out for on-screen movement: cubic-bezier(.23, 1, .32, 1).
function Ease-Out([double]$x) { return 1 - [Math]::Pow(1 - $x, 3.0) }
function Lerp([double]$a, [double]$b, [double]$f) { return $a + ($b - $a) * $f }

$total = [int][Math]::Round($Seconds * $Fps)
Write-Host "rendering $total frames at ${Width}x${Height}..."

for ($i = 0; $i -lt $total; $i++) {
  $t = $i / $Fps

  # Which scene are we in, and what are we moving toward?
  $idx = -1
  for ($s = 0; $s -lt $scenes.Count; $s++) { if ($t -ge $scenes[$s].at) { $idx = $s } }
  if ($idx -lt 0) { $query = "env=terminals&side=projects" } else { $query = $scenes[$idx].query }

  $from = $start; $fromT = 0.0
  $to = $scenes[0].cursor; $toT = $scenes[0].at
  if ($idx -ge 0 -and $idx + 1 -lt $scenes.Count) {
    $from = $scenes[$idx].cursor; $fromT = $scenes[$idx].at
    $to = $scenes[$idx + 1].cursor; $toT = $scenes[$idx + 1].at
  } elseif ($idx -ge 0) {
    $from = $scenes[$idx].cursor; $fromT = $scenes[$idx].at
    $to = $scenes[$idx].cursor; $toT = $scenes[$idx].at + 1
  }

  $f = if ($toT -gt $fromT) { [Math]::Max(0.0, [Math]::Min(1.0, ($t - $fromT) / ($toT - $fromT))) } else { 1.0 }
  $e = Ease-Out $f
  $cx = [int][Math]::Round((Lerp $from[0] $to[0] $e))
  $cy = [int][Math]::Round((Lerp $from[1] $to[1] $e))

  $q = "$query&cursor=$cx,$cy&nospin=1"

  # The ripple is relative to the click that just happened.
  foreach ($s in $scenes) {
    $dt = $t - $s.at
    if ($dt -ge 0 -and $dt -lt $ringSeconds) { $q += "&ring=" + [Math]::Round($dt / $ringSeconds, 3) }
  }

  $frame = Join-Path $tmpOut ("f{0:d4}.png" -f $i)
  & (Join-Path $PSScriptRoot "shoot.ps1") -Html $html -Out $frame -Inject $driver -Query $q `
      -Width $Width -Height $Height -Scale 1 -BudgetMs 2500 | Out-Null
}

$outPath = Join-Path $root $Out
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $outPath) | Out-Null

$ffmpeg = (Get-Command ffmpeg -ErrorAction SilentlyContinue).Source
if (-not $ffmpeg) {
  $cand = Get-ChildItem "$env:LOCALAPPDATA\ms-playwright" -Recurse -Filter "ffmpeg*.exe" -ErrorAction SilentlyContinue |
    Select-Object -First 1
  if ($cand) { $ffmpeg = $cand.FullName }
}
if (-not $ffmpeg) { throw "ffmpeg not found; frames are in $tmpOut" }

$vf = "scale=${GifWidth}:-1:flags=lanczos,split[a][b];" +
      "[a]palettegen=max_colors=128:stats_mode=diff[p];" +
      "[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle"

& $ffmpeg -y -loglevel error -framerate $Fps -i (Join-Path $tmpOut "f%04d.png") -vf $vf -loop 0 $outPath

if (-not $KeepFrames) { Remove-Item -Recurse -Force $tmpOut -ErrorAction SilentlyContinue }
Write-Host ("{0}  {1:N2} MB" -f $Out, ((Get-Item $outPath).Length / 1MB))
