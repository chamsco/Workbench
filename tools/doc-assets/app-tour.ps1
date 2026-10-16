<#
  app-tour.ps1 — build docs/assets/tour.gif from stills of the REAL app.

  Each input is one state captured by cdp-shoot.mjs during a single run of
  backtrace-tauri (see run-app.ps1). Crossfades rather than cuts, because the
  beats are large view changes and a hard cut between them reads as a glitch.

  Example:
    ./app-tour.ps1 -Frames target/app-shots/01-canvases.png,target/app-shots/02-wall.png,...
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$Frames,
  [string]$Out = "docs/assets/tour.gif",
  [double]$Hold = 2.0,
  [double]$Fade = 0.35,
  [int]$Fps = 12,
  [int]$GifWidth = 1000
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$list = $Frames -split "," | ForEach-Object { (Resolve-Path $_.Trim()).Path }
foreach ($f in $list) { if (-not (Test-Path $f)) { throw "missing frame $f" } }

$ffmpeg = (Get-Command ffmpeg -ErrorAction SilentlyContinue).Source
if (-not $ffmpeg) {
  $cand = Get-ChildItem "$env:LOCALAPPDATA\ms-playwright" -Recurse -Filter "ffmpeg*.exe" -ErrorAction SilentlyContinue |
    Select-Object -First 1
  if ($cand) { $ffmpeg = $cand.FullName }
}
if (-not $ffmpeg) { throw "ffmpeg not found" }

$inputs = @()
foreach ($f in $list) { $inputs += @("-loop", "1", "-t", "$Hold", "-i", $f) }

# offset for the nth crossfade, 1-based: n * (hold - fade)
$step = $Hold - $Fade
# Built with -f, not interpolation: PowerShell reads "$prev[$i" inside a
# double-quoted string as an index into $prev, which silently corrupts the graph.
$filter = ""
$prev = "[0:v]"
for ($i = 1; $i -lt $list.Count; $i++) {
  $offset = [Math]::Round($i * $step, 3)
  $last = ($i -eq $list.Count - 1)
  # NOT $out: PowerShell variable names are case-insensitive, so $out would
  # clobber the $Out parameter and the output path would become "[x]".
  $label = if ($last) { "[x]" } else { "[x$i]" }
  $filter += ("{0}[{1}:v]xfade=transition=fade:duration={2}:offset={3}{4};" -f $prev, $i, $Fade, $offset, $label)
  if (-not $last) { $prev = "[x$i]" }
}
$filter += ("[x]scale={0}:-1:flags=lanczos,split[a][b];" -f $GifWidth) +
           "[a]palettegen=max_colors=160:stats_mode=diff[p];" +
           "[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle"

$outPath = Join-Path $root $Out
$ffargs = @("-y", "-loglevel", "error") + $inputs +
          @("-filter_complex", [string]$filter, "-r", "$Fps", "-loop", "0", $outPath)
& $ffmpeg @ffargs
if ($LASTEXITCODE -ne 0) { throw "ffmpeg failed" }
Write-Host ("{0}  {1:N2} MB  ({2} beats, {3:N1}s)" -f $Out, ((Get-Item $outPath).Length / 1MB), $list.Count, ($list.Count * $step + $Fade))
