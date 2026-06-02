param(
    [string[]] $Targets = @("aarch64-linux-android", "x86_64-linux-android"),
    [string] $ProjectName = "merman_smoke",
    [string] $Python = "python"
)

$ErrorActionPreference = "Stop"

$args = @(
    (Join-Path $PSScriptRoot "android-smoke.py"),
    "--targets"
) + $Targets + @(
    "--project-name",
    $ProjectName
)

& $Python @args
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
