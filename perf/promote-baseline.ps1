param(
    [string]$ResultPath,
    [string]$ResultsDir = "perf/results",
    [string]$BaselinePath = "perf/baseline.json"
)

$ErrorActionPreference = "Stop"

if (-not $ResultPath) {
    $latest = Get-ChildItem $ResultsDir -Filter "login-*.json" |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if ($null -eq $latest) {
        throw "No perf result files were found in $ResultsDir."
    }

    $ResultPath = $latest.FullName
}

if (-not (Test-Path $ResultPath)) {
    throw "Result file not found: $ResultPath"
}

if (-not (Test-Path $BaselinePath)) {
    throw "Baseline file not found: $BaselinePath"
}

$result = Get-Content $ResultPath -Raw | ConvertFrom-Json
$baseline = Get-Content $BaselinePath -Raw | ConvertFrom-Json

$baseline.scenario = $result.run.scenario
$baseline.metrics = [pscustomobject]@{
    p50_ms = [double]$result.run.metrics.p50_ms
    p95_ms = [double]$result.run.metrics.p95_ms
    p99_ms = [double]$result.run.metrics.p99_ms
    avg_ms = [double]$result.run.metrics.avg_ms
    throughput_rps = [double]$result.run.metrics.throughput_rps
    failure_rate = [double]$result.run.metrics.failure_rate
}

$baseline | ConvertTo-Json -Depth 6 | Set-Content $BaselinePath

Write-Host "Promoted baseline from:" $ResultPath
Write-Host "Updated baseline:" $BaselinePath
Write-Host ""
Write-Host ($baseline | ConvertTo-Json -Depth 6)
