param(
    [switch] $BuildAndroidSlices,
    [switch] $RunFlutterAndroidSmoke,
    [switch] $RunAndroidGradleBuild,
    [string] $GradlePath = $env:MERMAN_GRADLE
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$flutterRoot = Join-Path $repoRoot "platforms\flutter"
$androidRoot = Join-Path $repoRoot "platforms\android"
$appleRoot = Join-Path $repoRoot "platforms\apple"
$androidJarOut = Join-Path $repoRoot "target\platforms\android\merman-android.jar"
$flutterJarOut = Join-Path $repoRoot "target\platforms\flutter\merman-flutter-android-plugin.jar"

function Step {
    param([string] $Name)
    Write-Host ""
    Write-Host "==> $Name"
}

function Invoke-Native {
    param(
        [string] $FilePath,
        [string[]] $ArgumentList
    )

    & $FilePath @ArgumentList
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $FilePath $($ArgumentList -join ' ')"
    }
}

function Resolve-GradleCommand {
    param([string] $Path)

    if ($Path) {
        $resolved = Resolve-Path -LiteralPath $Path
        if ((Get-Item -LiteralPath $resolved.Path).PSIsContainer) {
            $gradleBat = Join-Path $resolved.Path "gradle.bat"
            if (Test-Path -LiteralPath $gradleBat) {
                return $gradleBat
            }
            $gradle = Join-Path $resolved.Path "gradle"
            if (Test-Path -LiteralPath $gradle) {
                return $gradle
            }
            throw "Gradle executable not found under: $($resolved.Path)"
        }
        return $resolved.Path
    }

    $cmd = Get-Command gradle -ErrorAction SilentlyContinue
    if (-not $cmd) {
        throw "gradle not found. Pass -GradlePath or set MERMAN_GRADLE."
    }
    $cmd.Source
}

Push-Location $repoRoot
try {
    Step "Rust FFI host tests"
    Invoke-Native "cargo" @("nextest", "run", "-p", "merman-ffi")

    Step "Android Rust target check"
    Invoke-Native "rustup" @("target", "add", "aarch64-linux-android")
    Invoke-Native "cargo" @("check", "-p", "merman-ffi", "--target", "aarch64-linux-android")
    Invoke-Native "cargo" @("clippy", "-p", "merman-ffi", "--target", "aarch64-linux-android", "--", "-D", "warnings")

    Step "Android Kotlin wrapper compile"
    New-Item -ItemType Directory -Force -Path (Split-Path $androidJarOut) | Out-Null
    Invoke-Native "kotlinc" @(
        (Join-Path $androidRoot "src\main\kotlin\io\merman\MermanException.kt"),
        (Join-Path $androidRoot "src\main\kotlin\io\merman\MermanEngine.kt"),
        "-d",
        $androidJarOut
    )

    if ($BuildAndroidSlices) {
        Step "Android native slices"
        & (Join-Path $androidRoot "build-android.ps1") -Targets aarch64-linux-android,x86_64-linux-android -Profile release | Out-Host
    }

    Step "Flutter/Dart package checks"
    Push-Location $flutterRoot
    try {
        Invoke-Native "flutter" @("pub", "get")
        Invoke-Native "flutter" @("analyze")
        Invoke-Native "dart" @("format", "--set-exit-if-changed", "lib", "example")
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
    Invoke-Native "kotlinc" @(
        (Join-Path $flutterRoot "android\src\main\kotlin\io\merman\flutter\MermanFlutterPlugin.kt"),
        "-classpath",
        $flutterJar,
        "-d",
        $flutterJarOut
    )

    Step "Dart FFI native smoke"
    Invoke-Native "cargo" @("build", "-p", "merman-ffi")
    Push-Location $flutterRoot
    try {
        Invoke-Native "dart" @("run", "example/smoke.dart", "..\..\target\debug\merman_ffi.dll")
    }
    finally {
        Pop-Location
    }

    if ($RunAndroidGradleBuild) {
        $arm64Lib = Join-Path $androidRoot "src\main\jniLibs\arm64-v8a\libmerman_ffi.so"
        $x64Lib = Join-Path $androidRoot "src\main\jniLibs\x86_64\libmerman_ffi.so"
        if (-not (Test-Path -LiteralPath $arm64Lib) -or -not (Test-Path -LiteralPath $x64Lib)) {
            Step "Android native slices for Gradle"
            & (Join-Path $androidRoot "build-android.ps1") -Targets aarch64-linux-android,x86_64-linux-android -Profile release
        }

        Step "Android Gradle library assemble"
        $gradle = Resolve-GradleCommand $GradlePath
        Invoke-Native $gradle @("-p", $androidRoot, "assembleRelease", "--stacktrace")
    }

    Step "Apple Swift package scaffold checks"
    $bash = Get-Command bash -ErrorAction SilentlyContinue
    if (-not $bash) {
        throw "bash not found; required for Apple scaffold syntax checks."
    }
    $appleBuildScript = Join-Path $repoRoot "scripts\build-apple-xcframework.sh"
    $iosBuildScript = Join-Path $repoRoot "platforms\ios\build-ios.sh"
    $swiftWrapper = Join-Path $appleRoot "Sources\Merman\MermanEngine.swift"
    foreach ($path in @(
            (Join-Path $repoRoot "Package.swift"),
            $appleBuildScript,
            $iosBuildScript,
            $swiftWrapper,
            (Join-Path $repoRoot "crates\merman-ffi\include\merman.h")
        )) {
        if (-not (Test-Path -LiteralPath $path)) {
            throw "Required Apple binding file not found: $path"
        }
    }
    Invoke-Native $bash.Source @("-n", "scripts/build-apple-xcframework.sh")
    Invoke-Native $bash.Source @("-n", "platforms/ios/build-ios.sh")

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
