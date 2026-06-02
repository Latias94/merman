param(
    [switch] $BuildAndroidSlices,
    [switch] $RunFlutterAndroidSmoke,
    [switch] $RunAndroidGradleBuild,
    [string] $GradlePath = $env:MERMAN_GRADLE,
    [switch] $BuildAppleXcframework,
    [ValidateSet("all", "ios", "macos")]
    [string] $ApplePlatform = "all",
    [string] $Python = "python"
)

$ErrorActionPreference = "Stop"

$args = @((Join-Path $PSScriptRoot "verify-platform-bindings.py"))

if ($BuildAndroidSlices) {
    $args += "--build-android-slices"
}
if ($RunFlutterAndroidSmoke) {
    $args += "--run-flutter-android-smoke"
}
if ($RunAndroidGradleBuild) {
    $args += "--run-android-gradle-build"
}
if ($GradlePath) {
    $args += @("--gradle-path", $GradlePath)
}
if ($BuildAppleXcframework) {
    $args += "--build-apple-xcframework"
}
if ($ApplePlatform) {
    $args += @("--apple-platform", $ApplePlatform)
}

& $Python @args
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
