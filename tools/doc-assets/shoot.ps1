<#
  shoot.ps1 — render a local HTML file to a PNG with headless Chromium.

  Used to regenerate the images in docs/assets/. See tools/doc-assets/README.md.

  Examples
    ./shoot.ps1 -Html design/workbench.html -Out docs/assets/design.png
    ./shoot.ps1 -Html design/workbench.html -Out out/browser.png -Query "env=browser&btab=main"
    ./shoot.ps1 -Html design/workbench.html -Out out/tour.png -Inject tour-driver.js -Query "env=diagram&review=plan&cursor=980,40"
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$Html,
  [Parameter(Mandatory)][string]$Out,
  [string]$Query = "",
  [string]$Inject = "",
  [int]$Width = 1440,
  [int]$Height = 900,
  [double]$Scale = 2,
  [int]$BudgetMs = 5000
)

$ErrorActionPreference = "Stop"

function Resolve-Chrome {
  if ($env:DOC_ASSETS_CHROME -and (Test-Path $env:DOC_ASSETS_CHROME)) { return $env:DOC_ASSETS_CHROME }
  $roots = @(
    "$env:LOCALAPPDATA\ms-playwright",
    "$env:USERPROFILE\AppData\Local\ms-playwright"
  )
  foreach ($root in $roots) {
    if (-not (Test-Path $root)) { continue }
    $hit = Get-ChildItem $root -Recurse -Filter chrome.exe -ErrorAction SilentlyContinue |
      Sort-Object FullName -Descending | Select-Object -First 1
    if ($hit) { return $hit.FullName }
  }
  foreach ($p in @(
      "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
      "C:\Program Files\Microsoft\Edge\Application\msedge.exe",
      "C:\Program Files\Google\Chrome\Application\chrome.exe")) {
    if (Test-Path $p) { return $p }
  }
  throw "No Chromium-family browser found. Set `$env:DOC_ASSETS_CHROME to chrome.exe."
}

$chrome = Resolve-Chrome
$htmlPath = (Resolve-Path $Html).Path

# A copy carrying the injected driver, so the original files are never touched.
$staged = Join-Path ([System.IO.Path]::GetTempPath()) ("doc-assets-" + [guid]::NewGuid().ToString("N") + ".html")
$body = Get-Content -Raw $htmlPath
if ($Inject) {
  $js = Get-Content -Raw (Resolve-Path $Inject).Path
  # These mocks are fragments: they close nothing, so append when there is no </body>.
  $tag = if ($body -match "</body>") { "</body>" } elseif ($body -match "</html>") { "</html>" } else { $null }
  if ($tag) {
    $body = $body -replace "(?s)$([regex]::Escape($tag))", "<script>`n$js`n</script>`n$tag"
  } else {
    $body = $body + "`n<script>`n$js`n</script>`n"
  }
}
Set-Content -Path $staged -Value $body -Encoding UTF8

$outPath = [System.IO.Path]::GetFullPath($Out)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $outPath) | Out-Null
if (Test-Path $outPath) { Remove-Item -Force $outPath }

$uri = "file:///" + ($staged -replace '\\', '/')
if ($Query) { $uri += "?$Query" }

$args = @(
  "--headless=new", "--disable-gpu", "--hide-scrollbars", "--no-first-run",
  "--no-default-browser-check", "--disable-extensions", "--mute-audio",
  "--force-device-scale-factor=$Scale",
  "--window-size=$Width,$Height",
  "--virtual-time-budget=$BudgetMs",
  "--screenshot=$outPath",
  $uri
)

& $chrome @args 2>&1 | Where-Object { $_ -match "written to file|ERROR:sandbox" } | Out-Null
Remove-Item -Force $staged -ErrorAction SilentlyContinue

if (-not (Test-Path $outPath)) { throw "Chromium wrote no screenshot for $Html" }
Write-Host ("{0}  {1:N0} bytes" -f ($outPath -replace [regex]::Escape((Get-Location).Path), "."), (Get-Item $outPath).Length)
