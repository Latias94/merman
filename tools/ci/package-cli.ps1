param(
  [Parameter(Mandatory = $true)]
  [string]$PackageName,

  [Parameter(Mandatory = $true)]
  [string]$Target,

  [Parameter(Mandatory = $true)]
  [string]$Tag
)

$ErrorActionPreference = "Stop"

function Get-BinPath {
  param([string]$PackageName, [string]$Target)

  $exeSuffix = ""
  if ($IsWindows) {
    $exeSuffix = ".exe"
  }

  $path = Join-Path -Path "target" -ChildPath (Join-Path -Path $Target -ChildPath (Join-Path -Path "release" -ChildPath ($PackageName + $exeSuffix)))
  if (!(Test-Path $path)) {
    throw "Binary not found: $path"
  }

  return $path
}

$binPath = Get-BinPath -PackageName $PackageName -Target $Target

New-Item -ItemType Directory -Force -Path "dist" | Out-Null

$baseName = "$PackageName-$Tag-$Target"
$zipPath = Join-Path -Path "dist" -ChildPath ($baseName + ".zip")

if (Test-Path $zipPath) {
  Remove-Item -Force $zipPath
}

$tempRoot = $env:RUNNER_TEMP
if ([string]::IsNullOrWhiteSpace($tempRoot)) {
  $tempRoot = [System.IO.Path]::GetTempPath()
}

$staging = Join-Path -Path $tempRoot -ChildPath $baseName
if (Test-Path $staging) {
  Remove-Item -Recurse -Force $staging
}
New-Item -ItemType Directory -Force -Path $staging | Out-Null

Copy-Item -Force $binPath (Join-Path -Path $staging -ChildPath (Split-Path $binPath -Leaf))

Compress-Archive -Path (Join-Path -Path $staging -ChildPath "*") -DestinationPath $zipPath

$hash = (Get-FileHash -Algorithm SHA256 $zipPath).Hash.ToLowerInvariant()
$shaPath = Join-Path -Path "dist" -ChildPath ($baseName + ".zip.sha256")
Set-Content -NoNewline -Path $shaPath -Value $hash

Write-Host "Wrote: $zipPath"
Write-Host "Wrote: $shaPath"
