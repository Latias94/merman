param(
    [string] $PackageDir = (Join-Path $PSScriptRoot "..\bindings\python\merman"),
    [string] $WheelDir = "target\python-wheels",
    [string] $Python = "python",
    [switch] $RunSmoke
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

function Resolve-OutputPath {
    param([string] $Path)

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    Join-Path $repoRoot $Path
}

function Venv-Python {
    param([string] $VenvDir)

    $windowsPython = Join-Path $VenvDir "Scripts\python.exe"
    if (Test-Path -LiteralPath $windowsPython) {
        return $windowsPython
    }

    $unixPython = Join-Path $VenvDir "bin\python"
    if (Test-Path -LiteralPath $unixPython) {
        return $unixPython
    }

    throw "Python executable not found in venv: $VenvDir"
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$packagePath = Resolve-Path -LiteralPath $PackageDir
$wheelPath = Resolve-OutputPath $WheelDir

Push-Location $repoRoot
try {
    Invoke-Native "cargo" @("build", "-p", "merman-uniffi", "--features", "bindgen-smoke")
    Invoke-Native "cargo" @(
        "run",
        "-p",
        "merman-uniffi",
        "--features",
        "bindgen-smoke",
        "--example",
        "generate_python_package",
        "--",
        "--package-dir",
        $packagePath
    )

    New-Item -ItemType Directory -Force -Path $wheelPath | Out-Null
    Invoke-Native $Python @("-m", "pip", "wheel", $packagePath, "--no-deps", "--wheel-dir", $wheelPath)

    if ($RunSmoke) {
        $wheel = Get-ChildItem -LiteralPath $wheelPath -Filter "merman-*.whl" |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if (-not $wheel) {
            throw "No merman wheel found under $wheelPath"
        }

        $venvDir = Join-Path $repoRoot "target\python-wheel-smoke"
        if (Test-Path -LiteralPath $venvDir) {
            Remove-Item -LiteralPath $venvDir -Recurse -Force
        }
        Invoke-Native $Python @("-m", "venv", $venvDir)
        $venvPython = Venv-Python $venvDir
        Invoke-Native $venvPython @("-m", "pip", "install", "--no-deps", $wheel.FullName)
        Invoke-Native $venvPython @(
            "-c",
            "import merman; e = merman.MermanEngine(); assert e.render_svg('flowchart TD\nA[Hello]', None).startswith('<svg')"
        )
    }
}
finally {
    Pop-Location
}
