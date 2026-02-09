param(
  [string]$OutDir = "target/png-preview/by-diagram",
  [string]$FixturesDir = "fixtures",
  [float]$Scale = 2,
  [string]$Background = "white",
  [ValidateSet("deterministic", "vendored")]
  [string]$TextMeasurer = "vendored",
  [switch]$AllFixtures,
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

# Some fixtures are intentionally "empty output" specs (e.g. accessibility metadata only).
# Prefer picking a more representative fixture for preview export when available.
$preferredByDiagram = @{
  "journey"  = @("upstream_docs_userjourney_user_journey_diagram_002.mmd")
  "mindmap"  = @("upstream_docs_example_icons_br.mmd")
  "gitgraph" = @("upstream_docs_examples_a_commit_flow_diagram_018.mmd")
  "info"     = @("upstream_info_show_info_multiline_spec.mmd", "upstream_info_show_info_spec.mmd")
}

$skipByDiagram = @{
  "architecture" = @("upstream_architecture_acc_title_and_descr_spec.mmd")
  "packet"       = @("upstream_packet_beta_header_spec.mmd")
  "journey"      = @(
    "upstream_accdescr_block_title_acctitle_section.mmd",
    "upstream_accdescr_single_line.mmd",
    "upstream_acctitle_only.mmd",
    "upstream_section_only.mmd",
    "upstream_title.mmd"
  )
  "gitgraph"     = @("upstream_accessibility_and_warnings.mmd", "upstream_accessibility_single_line_accdescr_spec.mmd")
  "info"         = @("upstream_info_empty_leading_newline_spec.mmd")
  "kanban"       = @("metadata.mmd")
  "mindmap"      = @("upstream_docs_unclear_indentation.mmd")
}

function Export-OnePng {
  param(
    [Parameter(Mandatory = $true)][string]$CliPath,
    [Parameter(Mandatory = $true)][string]$InputMmd,
    [Parameter(Mandatory = $true)][string]$OutPng
  )

  New-Item -ItemType Directory -Force -Path (Split-Path $OutPng) | Out-Null

  $msg = & $CliPath render --format png --scale $Scale --background $Background --text-measurer $TextMeasurer --out $OutPng $InputMmd 2>&1
  if ($LASTEXITCODE -ne 0) {
    if ($msg) {
      return (($msg | Select-Object -Last 1) -as [string])
    }
    return "unknown error"
  }

  if (-not (Test-Path $OutPng)) {
    return "render reported success but output file is missing"
  }

  return $null
}

if ($AllFixtures) {
  $fixturesRoot = (Resolve-Path $FixturesDir).Path
  $all = Get-ChildItem $FixturesDir -Recurse -File -Filter *.mmd |
    Where-Object { $_.FullName -notmatch "\\\\upstream-svgs\\\\" } |
    Sort-Object FullName

  $failCount = 0
  foreach ($mmd in $all) {
    $rel = $mmd.FullName.Substring($fixturesRoot.Length).TrimStart('\', '/')
    $relDir = Split-Path $rel -Parent
    $outDir = if ($relDir) { Join-Path $OutDir $relDir } else { $OutDir }
    $outPath = Join-Path $outDir (([IO.Path]::GetFileNameWithoutExtension($mmd.Name)) + ".png")

    $err = Export-OnePng -CliPath $cli -InputMmd $mmd.FullName -OutPng $outPath
    if ($err) {
      $failCount += 1
      Write-Host ("[fail] " + $rel + ": " + $err)
    }
  }

  Write-Host ""
  Write-Host "PNG preview written to: $OutDir"
  Write-Host "Failures: $failCount / $($all.Count)"
  exit 0
}

foreach ($dir in $diagramDirs) {
  $allFiles = @(
    Get-ChildItem $dir.FullName -Recurse -Filter *.mmd |
      Sort-Object FullName
  )

  $files = @(
    $allFiles |
      Where-Object { $_.Name -notmatch "^upstream_docs_examples_" } |
      Sort-Object FullName
  )

  if ($files.Count -eq 0) {
    $files = @($allFiles)
  }

  if ($skipByDiagram.ContainsKey($dir.Name)) {
    $skips = $skipByDiagram[$dir.Name]
    $filtered = @($files | Where-Object { $skips -notcontains $_.Name })
    if ($filtered.Count -gt 0) {
      $files = $filtered
    }
  }

  if ($preferredByDiagram.ContainsKey($dir.Name)) {
    foreach ($preferredName in $preferredByDiagram[$dir.Name]) {
      $hit = $allFiles | Where-Object { $_.Name -eq $preferredName } | Select-Object -First 1
      if ($hit) {
        $files = @($hit) + @($files | Where-Object { $_.FullName -ne $hit.FullName })
        break
      }
    }
  }

  $picked = $null
  $pickedOut = $null
  $lastErr = $null

  foreach ($mmd in $files) {
    $outDiagramDir = Join-Path $OutDir $dir.Name
    New-Item -ItemType Directory -Force -Path $outDiagramDir | Out-Null

    $outPath = Join-Path $outDiagramDir (([IO.Path]::GetFileNameWithoutExtension($mmd.Name)) + ".png")

    $err = Export-OnePng -CliPath $cli -InputMmd $mmd.FullName -OutPng $outPath
    if (-not $err) {
      $picked = $mmd
      $pickedOut = $outPath
      break
    }

    $lastErr = $err
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
