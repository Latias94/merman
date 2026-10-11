param(
    [Parameter(Mandatory)][string]$Cli,
    [string]$RepoRoot = (Join-Path $PSScriptRoot '../../../..'),
    [string]$OutputPath = 'samples.json',
    [ValidateRange(1, 10000)][int]$Samples = 30,
    [ValidateRange(1, 100)][int]$Repetitions = 3
)
$ErrorActionPreference = 'Stop'
$executable = [IO.Path]::GetFullPath($Cli)
$repository = [IO.Path]::GetFullPath($RepoRoot)
$configuration = Join-Path $PSScriptRoot 'config.json'
$font = Join-Path $repository 'crates/merman-export/tests/fonts/Cousine-Regular.ttf'
$inputs = [ordered]@{}
foreach ($family in @('flowchart', 'er', 'sequence')) {
    $inputs[$family] = Join-Path $repository "docs/assets/native-font-measurement/$family.mmd"
}
$outputHashes = @{}
function Get-Digest([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Invoke-Sample([string]$Family, [string]$Lane, [int]$Repetition, [int]$Index) {
    $start = [Diagnostics.ProcessStartInfo]::new($executable)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.WorkingDirectory = $repository
    $arguments = @('render', $inputs[$Family], '--format', 'svg', '--svg-id', 'explicit-probe', '--config-file', $configuration, '--output', '-')
    $arguments += if ($Lane -eq 'native') {
        @('--font-file', $font, '--font-system', 'off')
    } else {
        @('--font-measurement', 'deterministic')
    }
    foreach ($argument in $arguments) { $start.ArgumentList.Add($argument) }
    $timestamp = Get-Date -Format o
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $process = [Diagnostics.Process]::Start($start)
    $stdout = [IO.MemoryStream]::new()
    try {
        $copy = $process.StandardOutput.BaseStream.CopyToAsync($stdout)
        $stderr = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        $copy.GetAwaiter().GetResult() | Out-Null
        $errorText = $stderr.GetAwaiter().GetResult()
        $clock.Stop()
        if ($process.ExitCode -ne 0 -or $stdout.Length -eq 0) {
            throw "$Family/$Lane failed or emitted empty stdout (exit $($process.ExitCode)): $errorText"
        }
        $hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stdout.ToArray())).ToLowerInvariant()
        $key = "$Family/$Lane"
        if ($outputHashes.ContainsKey($key) -and $outputHashes[$key] -ne $hash) {
            throw "$key output changed across samples"
        }
        $outputHashes[$key] = $hash
        return [pscustomobject]@{
            timestamp = $timestamp
            family = $Family
            lane = $Lane
            repetition = $Repetition
            index = $Index
            exit_code = $process.ExitCode
            milliseconds = $clock.Elapsed.TotalMilliseconds
            output_bytes = $stdout.Length
            output_sha256 = $hash
        }
    } finally {
        $stdout.Dispose()
        $process.Dispose()
    }
}
$startedAt = Get-Date -Format o
$identities = [ordered]@{
    executable_sha256 = Get-Digest $executable
    executable_bytes = (Get-Item -LiteralPath $executable).Length
    font_sha256 = Get-Digest $font
    config_sha256 = Get-Digest $configuration
    harness_sha256 = Get-Digest $PSCommandPath
    lockfile_sha256 = Get-Digest (Join-Path $repository 'Cargo.lock')
    input_sha256 = [ordered]@{}
}
foreach ($family in $inputs.Keys) { $identities.input_sha256[$family] = Get-Digest $inputs[$family] }
$rows = [Collections.Generic.List[object]]::new()
foreach ($family in $inputs.Keys) {
    foreach ($warmup in 1..3) {
        Invoke-Sample $family 'deterministic' 0 $warmup | Out-Null
        Invoke-Sample $family 'native' 0 $warmup | Out-Null
    }
    foreach ($repetition in 1..$Repetitions) {
        foreach ($index in 1..$Samples) {
            $order = if (($index + $repetition) % 2 -eq 0) { @('deterministic', 'native') } else { @('native', 'deterministic') }
            foreach ($lane in $order) { $rows.Add((Invoke-Sample $family $lane $repetition $index)) }
        }
        Write-Host "$family repetition $repetition/$Repetitions completed"
    }
}
$summary = @($rows | Group-Object family, lane | ForEach-Object {
    $times = @($_.Group.milliseconds | Sort-Object)
    $middle = [int][Math]::Floor($times.Count / 2)
    $median = if ($times.Count % 2) { $times[$middle] } else { ($times[$middle - 1] + $times[$middle]) / 2 }
    [pscustomobject]@{
        family = $_.Group[0].family
        lane = $_.Group[0].lane
        samples = $times.Count
        median_ms = $median
        p95_ms = $times[[int][Math]::Ceiling(0.95 * $times.Count) - 1]
    }
})
[ordered]@{
    started_at = $startedAt
    completed_at = Get-Date -Format o
    identities = $identities
    warmups_per_family_per_lane = 3
    samples_per_family_per_lane_per_repetition = $Samples
    repetitions = $Repetitions
    cache_policy = 'warm OS filesystem cache; fresh CLI process for every sample'
    order = 'family blocks; alternating AB/BA by index and repetition'
    recipe = 'same executable: render INPUT --format svg --svg-id explicit-probe --config-file config.json --output -; deterministic: --font-measurement deterministic; native: --font-file Cousine-Regular.ttf --font-system off'
    summary = $summary
    rows = $rows
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $OutputPath -Encoding utf8
$summary | Format-Table -AutoSize
