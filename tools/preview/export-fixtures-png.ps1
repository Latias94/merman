param(
  [string]$OutDir = "target/png-preview/by-diagram",
  [string]$FixturesDir = "fixtures",
  [float]$Scale = 2,
  [string]$Background = "white",
  [ValidateSet("deterministic", "vendored")]
  [string]$TextMeasurer = "vendored",
  [switch]$BuildReleaseCli,
  [switch]$CleanOutDir
)

$ErrorActionPreference = "Stop"

# Note:
# - Raster rendering is best-effort (see docs/rendering/RASTER_OUTPUT.md).
# - Many upstream Mermaid diagrams use SVG <foreignObject> HTML labels; pure-Rust rasterizers do
#   not fully support this, so `merman-cli` applies a raster-only text fallback conversion.

function Resolve-CliPath {
  $candidates = @(
    "target/release/merman-cli.exe",
    "target/release/merman-cli"
  )

  foreach ($c in $candidates) {
    if (Test-Path $c) {
      return $c
    }
  }

  throw "Cannot find merman-cli binary. Build it first (e.g. 'cargo build --release -p merman-cli')."
}

if ($BuildReleaseCli) {
  cargo build --release --locked -p merman-cli
  if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed"
  }
}

$cli = Resolve-CliPath

if ($CleanOutDir -and (Test-Path $OutDir)) {
  Remove-Item -Recurse -Force $OutDir
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$diagramDirs = Get-ChildItem $FixturesDir -Directory | Where-Object { $_.Name -ne "upstream-svgs" }

$results = @()

foreach ($dir in $diagramDirs) {
  $files = @(
    Get-ChildItem $dir.FullName -Recurse -Filter *.mmd |
      Where-Object { $_.Name -notmatch "^upstream_docs_examples_" } |
      Sort-Object FullName
  )

  if ($files.Count -eq 0) {
    $files = @(
      Get-ChildItem $dir.FullName -Recurse -Filter *.mmd |
        Sort-Object FullName
    )
  }

  $picked = $null
  $pickedOut = $null
  $lastErr = $null

  foreach ($mmd in $files) {
    $outDiagramDir = Join-Path $OutDir $dir.Name
    New-Item -ItemType Directory -Force -Path $outDiagramDir | Out-Null

    $outPath = Join-Path $outDiagramDir (([IO.Path]::GetFileNameWithoutExtension($mmd.Name)) + ".png")

    $msg = & $cli render --format png --scale $Scale --background $Background --text-measurer $TextMeasurer --out $outPath $mmd.FullName 2>&1
    if ($LASTEXITCODE -eq 0 -and (Test-Path $outPath)) {
      $picked = $mmd
      $pickedOut = $outPath
      break
    }

    if ($msg) {
      $lastErr = (($msg | Select-Object -Last 1) -as [string])
    } else {
      $lastErr = "unknown error"
    }
  }

  if ($picked) {
    $results += [PSCustomObject]@{
      diagram = $dir.Name
      input  = (Split-Path -Leaf $picked.FullName)
      output = (Split-Path -Leaf $pickedOut)
      kb     = [Math]::Round(((Get-Item $pickedOut).Length / 1024.0), 1)
      status = "ok"
    }
  } else {
    $results += [PSCustomObject]@{
      diagram = $dir.Name
      input  = $null
      output = $null
      kb     = $null
      status = ("fail: " + $lastErr)
    }
  }
}

$results | Sort-Object diagram | Format-Table -AutoSize

$fails = ($results | Where-Object { $_.status -ne "ok" }).Count
Write-Host ""
Write-Host "PNG preview written to: $OutDir"
Write-Host "Failures: $fails / $($results.Count)"
