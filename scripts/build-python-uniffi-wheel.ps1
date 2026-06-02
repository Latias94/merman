param(
    [string] $PackageDir = (Join-Path $PSScriptRoot "..\platforms\python\merman"),
    [string] $WheelDir = "target\python-wheels",
    [string] $Python = "python",
    [switch] $RunSmoke
)

$ErrorActionPreference = "Stop"

$args = @(
    (Join-Path $PSScriptRoot "build-python-uniffi-wheel.py"),
    "--package-dir",
    $PackageDir,
    "--wheel-dir",
    $WheelDir,
    "--python",
    $Python
)

if ($RunSmoke) {
    $args += "--run-smoke"
}

& $Python @args
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
