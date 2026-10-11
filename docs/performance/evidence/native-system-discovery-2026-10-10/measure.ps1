param(
    [Parameter(Mandatory)][string]$Baseline,
    [Parameter(Mandatory)][string]$Candidate,
    [Parameter(Mandatory)][string]$InputPath,
    [string]$OutputPath = 'samples.json',
    [ValidateRange(1, 10000)][int]$Samples = 30,
    [ValidateRange(1, 100)][int]$Repetitions = 3
)
$ErrorActionPreference = 'Stop'
$executables = @{ baseline = [IO.Path]::GetFullPath($Baseline); candidate = [IO.Path]::GetFullPath($Candidate) }
$inputPath = [IO.Path]::GetFullPath($InputPath)
function Invoke-Sample([string]$Lane) {
    $start = [Diagnostics.ProcessStartInfo]::new($executables[$Lane])
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in @('render', $inputPath, '--format', 'svg', '--svg-id', 'startup-probe', '--output', '-')) {
        $start.ArgumentList.Add($argument)
    }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $process = [Diagnostics.Process]::Start($start)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $text = $stdout.GetAwaiter().GetResult()
    $errorText = $stderr.GetAwaiter().GetResult()
    $clock.Stop()
    if ($process.ExitCode -ne 0) { throw "$Lane failed: $errorText" }
    $sample = [pscustomobject]@{
        lane = $Lane
        milliseconds = $clock.Elapsed.TotalMilliseconds
        output_sha256 = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($text))).ToLowerInvariant()
    }
    $process.Dispose()
    return $sample
}
foreach ($warmup in 1..3) {
    Invoke-Sample 'baseline' | Out-Null
    Invoke-Sample 'candidate' | Out-Null
}
$rows = [Collections.Generic.List[object]]::new()
foreach ($repetition in 1..$Repetitions) {
    foreach ($index in 1..$Samples) {
        $order = if (($index + $repetition) % 2 -eq 0) { @('baseline', 'candidate') } else { @('candidate', 'baseline') }
        foreach ($lane in $order) {
            $row = Invoke-Sample $lane
            $row | Add-Member repetition $repetition
            $row | Add-Member index $index
            $rows.Add($row)
        }
    }
    Write-Host "Completed repetition $repetition/$Repetitions"
}
if (@($rows.output_sha256 | Select-Object -Unique).Count -ne 1) { throw 'A/B SVG output differs' }
[ordered]@{
    date = (Get-Date -Format o)
    input_sha256 = (Get-FileHash $inputPath -Algorithm SHA256).Hash.ToLowerInvariant()
    baseline_sha256 = (Get-FileHash $executables.baseline -Algorithm SHA256).Hash.ToLowerInvariant()
    candidate_sha256 = (Get-FileHash $executables.candidate -Algorithm SHA256).Hash.ToLowerInvariant()
    warmups_per_lane = 3
    samples_per_lane_per_repetition = $Samples
    repetitions = $Repetitions
    cache_policy = 'warm OS filesystem cache; fresh CLI process for every sample'
    rows = $rows
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $OutputPath -Encoding utf8
foreach ($lane in @('baseline', 'candidate')) {
    $times = @($rows | Where-Object lane -EQ $lane | ForEach-Object milliseconds | Sort-Object)
    $middle = [int][Math]::Floor($times.Count / 2)
    $median = if ($times.Count % 2) { $times[$middle] } else { ($times[$middle - 1] + $times[$middle]) / 2 }
    Write-Host "$lane median_ms=$median samples=$($times.Count)"
}
