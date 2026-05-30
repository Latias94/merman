param(
    [string[]] $Targets = @("aarch64-linux-android", "x86_64-linux-android"),
    [string] $Profile = "release",
    [string] $NdkHome = $env:ANDROID_NDK_HOME
)

$ErrorActionPreference = "Stop"

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

function Abi-ForTarget {
    param([string] $Target)
    switch ($Target) {
        "aarch64-linux-android" { "arm64-v8a"; break }
        "x86_64-linux-android" { "x86_64"; break }
        "armv7-linux-androideabi" { "armeabi-v7a"; break }
        default { throw "unsupported Android Rust target: $Target" }
    }
}

function Clang-ForTarget {
    param([string] $Target, [string] $Ndk)
    $hostTag = if ($IsWindows -or $env:OS -eq "Windows_NT") { "windows-x86_64" } else { "linux-x86_64" }
    $bin = Join-Path $Ndk "toolchains\llvm\prebuilt\$hostTag\bin"
    $api = "23"
    $name = switch ($Target) {
        "aarch64-linux-android" { "aarch64-linux-android$api-clang.cmd"; break }
        "x86_64-linux-android" { "x86_64-linux-android$api-clang.cmd"; break }
        "armv7-linux-androideabi" { "armv7a-linux-androideabi$api-clang.cmd"; break }
        default { throw "unsupported Android Rust target: $Target" }
    }
    $clang = Join-Path $bin $name
    if (-not (Test-Path -LiteralPath $clang)) {
        throw "Android clang not found: $clang"
    }
    $clang
}

function Default-NdkHome {
    if ($NdkHome) {
        return $NdkHome
    }
    $sdk = $env:ANDROID_HOME
    if (-not $sdk) {
        $sdk = $env:ANDROID_SDK_ROOT
    }
    if (-not $sdk) {
        throw "ANDROID_NDK_HOME or ANDROID_HOME/ANDROID_SDK_ROOT must be set"
    }
    $ndkRoot = Join-Path $sdk "ndk"
    $latest = Get-ChildItem -LiteralPath $ndkRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1
    if (-not $latest) {
        throw "No Android NDK installation found under $ndkRoot"
    }
    $latest.FullName
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$jniLibs = Join-Path $PSScriptRoot "src\main\jniLibs"
$ndk = Default-NdkHome

Write-Host "Using Android NDK: $ndk"

foreach ($target in $Targets) {
    $abi = Abi-ForTarget $target
    $clang = Clang-ForTarget $target $ndk
    $envName = "CARGO_TARGET_$($target.ToUpperInvariant().Replace('-', '_'))_LINKER"
    Set-Item -Path "Env:$envName" -Value $clang

    Write-Host "Building merman-ffi for $target ($abi)"
    Invoke-Native "rustup" @("target", "add", $target)
    if ($Profile -eq "release") {
        Invoke-Native "cargo" @("build", "-p", "merman-ffi", "--target", $target, "--release", "--manifest-path", (Join-Path $repoRoot "Cargo.toml"))
        $artifact = Join-Path $repoRoot "target\$target\release\libmerman_ffi.so"
    } else {
        Invoke-Native "cargo" @("build", "-p", "merman-ffi", "--target", $target, "--manifest-path", (Join-Path $repoRoot "Cargo.toml"))
        $artifact = Join-Path $repoRoot "target\$target\debug\libmerman_ffi.so"
    }

    if (-not (Test-Path -LiteralPath $artifact)) {
        throw "expected Android library not found: $artifact"
    }

    $dest = Join-Path $jniLibs $abi
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
    Copy-Item -LiteralPath $artifact -Destination (Join-Path $dest "libmerman_ffi.so") -Force
    Write-Host "Copied $abi library to $dest"
}
