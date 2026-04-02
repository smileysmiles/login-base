param(
    [string]$ResultPath,
    [string]$ResultsDir = "perf/results",
    [string]$BaselinePath = ""
)

$ErrorActionPreference = "Stop"

if (-not $ResultPath) {
    $latest = Get-ChildItem $ResultsDir -Filter "*.json" |
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

$result = Get-Content $ResultPath -Raw | ConvertFrom-Json

if (-not $BaselinePath) {
    $BaselinePath = if ($result.run.scenario -eq "login") {
        "perf/baseline.json"
    }
    else {
        "perf/baseline-$($result.run.scenario).json"
    }
}

if (-not (Test-Path $BaselinePath)) {
    $baseline = [pscustomobject]@{
        scenario = $result.run.scenario
        notes = "Set this after a known-good intentional perf run. Values are regression indicators, not production benchmarks."
        metrics = [pscustomobject]@{
            p50_ms = 0
            p95_ms = 0
            p99_ms = 0
            avg_ms = 0
            throughput_rps = 0
            failure_rate = 0
        }
        guardrails = [pscustomobject]@{
            warn_regression_pct = 10
            fail_regression_pct = 20
            failure_rate_warn_abs = 0.01
            failure_rate_fail_abs = 0.05
        }
    }
}
else {
    $baseline = Get-Content $BaselinePath -Raw | ConvertFrom-Json
}

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
