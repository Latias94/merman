param(
    [switch] $BuildAndroidSlices,
    [switch] $RunFlutterAndroidSmoke
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$flutterRoot = Join-Path $repoRoot "platforms\flutter"
$androidRoot = Join-Path $repoRoot "platforms\android"
$androidJarOut = Join-Path $repoRoot "target\platforms\android\merman-android.jar"
$flutterJarOut = Join-Path $repoRoot "target\platforms\flutter\merman-flutter-android-plugin.jar"

function Step {
    param([string] $Name)
    Write-Host ""
    Write-Host "==> $Name"
}

Push-Location $repoRoot
try {
    Step "Rust FFI host tests"
    cargo nextest run -p merman-ffi | Out-Host

    Step "Android Rust target check"
    rustup target add aarch64-linux-android | Out-Host
    cargo check -p merman-ffi --target aarch64-linux-android | Out-Host
    cargo clippy -p merman-ffi --target aarch64-linux-android -- -D warnings | Out-Host

    Step "Android Kotlin wrapper compile"
    New-Item -ItemType Directory -Force -Path (Split-Path $androidJarOut) | Out-Null
    kotlinc `
        (Join-Path $androidRoot "src\main\kotlin\io\merman\MermanException.kt") `
        (Join-Path $androidRoot "src\main\kotlin\io\merman\MermanEngine.kt") `
        -d $androidJarOut | Out-Host

    if ($BuildAndroidSlices) {
        Step "Android native slices"
        & (Join-Path $androidRoot "build-android.ps1") -Targets aarch64-linux-android,x86_64-linux-android -Profile release | Out-Host
    }

    Step "Flutter/Dart package checks"
    Push-Location $flutterRoot
    try {
        flutter pub get | Out-Host
        flutter analyze | Out-Host
        dart format --set-exit-if-changed lib example | Out-Host
    }
    finally {
        Pop-Location
    }

    Step "Flutter Android plugin Kotlin compile"
    $flutterJar = Join-Path $env:FLUTTER_ROOT "bin\cache\artifacts\engine\android-arm64\flutter.jar"
    if (-not $env:FLUTTER_ROOT -or -not (Test-Path -LiteralPath $flutterJar)) {
        $flutterExe = (Get-Command flutter).Source
        $flutterBin = Split-Path $flutterExe
        $flutterHome = Resolve-Path (Join-Path $flutterBin "..")
        $flutterJar = Join-Path $flutterHome "bin\cache\artifacts\engine\android-arm64\flutter.jar"
    }
    if (-not (Test-Path -LiteralPath $flutterJar)) {
        throw "Flutter Android embedding jar not found. Set FLUTTER_ROOT or run flutter doctor."
    }
    New-Item -ItemType Directory -Force -Path (Split-Path $flutterJarOut) | Out-Null
    kotlinc `
        (Join-Path $flutterRoot "android\src\main\kotlin\io\merman\flutter\MermanFlutterPlugin.kt") `
        -classpath $flutterJar `
        -d $flutterJarOut | Out-Host

    Step "Dart FFI native smoke"
    cargo build -p merman-ffi | Out-Host
    Push-Location $flutterRoot
    try {
        dart run example/smoke.dart ..\..\target\debug\merman_ffi.dll | Out-Host
    }
    finally {
        Pop-Location
    }

    if ($RunFlutterAndroidSmoke) {
        Step "Flutter Android APK packaging smoke"
        & (Join-Path $flutterRoot "tool\android-smoke.ps1") -Targets aarch64-linux-android | Out-Host
    }
}
finally {
    Pop-Location
}

Write-Host ""
Write-Host "Platform binding verification completed."
