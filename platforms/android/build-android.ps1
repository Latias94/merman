param(
    [string[]] $Targets = @("aarch64-linux-android", "x86_64-linux-android"),
    [ValidateSet("debug", "release")]
    [string] $Profile = "release",
    [string] $NdkHome = $env:ANDROID_NDK_HOME,
    [string] $Python = "python"
)

$ErrorActionPreference = "Stop"

$args = @(
    (Join-Path $PSScriptRoot "build-android.py"),
    "--targets"
) + $Targets + @(
    "--profile",
    $Profile
)

if ($NdkHome) {
    $args += @("--ndk-home", $NdkHome)
}

& $Python @args
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
