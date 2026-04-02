param(
    [string]$BaseUrl = "http://127.0.0.1:3000",
    [string]$Scenario = "login",
    [string]$BaselinePath = "",
    [string]$ResultsDir = "perf/results"
)

$ErrorActionPreference = "Stop"

function Get-MetricValue {
    param(
        [Parameter(Mandatory = $true)]
        $Metric,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    if ($null -eq $Metric) {
        throw "Missing metric values for '$Name'."
    }

    $container = $Metric
    if ($null -ne $Metric.values) {
        $container = $Metric.values
    }

    $prop = $container.PSObject.Properties[$Name]
    if ($null -eq $prop) {
        throw "Metric '$Name' was not present in the k6 summary. Check summaryTrendStats."
    }

    return [double]$prop.Value
}

function Get-ScalarMetricValue {
    param(
        [Parameter(Mandatory = $true)]
        $Metric,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    if ($null -eq $Metric) {
        throw "Missing scalar metric '$Name'."
    }

    $prop = $Metric.PSObject.Properties[$Name]
    if ($null -eq $prop) {
        throw "Scalar metric '$Name' was not present in the k6 summary."
    }

    return [double]$prop.Value
}

function Compare-Metric {
    param(
        [string]$Name,
        [double]$Current,
        [double]$Baseline,
        [double]$WarnPct,
        [double]$FailPct,
        [bool]$HigherIsWorse = $true
    )

    if ($Baseline -le 0) {
        return [pscustomobject]@{
            name = $Name
            baseline = $Baseline
            current = $Current
            regression_pct = $null
            status = "no-baseline"
        }
    }

    if ($HigherIsWorse) {
        $deltaPct = (($Current - $Baseline) / $Baseline) * 100.0
    }
    else {
        $deltaPct = (($Baseline - $Current) / $Baseline) * 100.0
    }

    $status = "pass"
    if ($deltaPct -ge $FailPct) {
        $status = "fail"
    }
    elseif ($deltaPct -ge $WarnPct) {
        $status = "warn"
    }

    return [pscustomobject]@{
        name = $Name
        baseline = [math]::Round($Baseline, 6)
        current = [math]::Round($Current, 6)
        regression_pct = [math]::Round($deltaPct, 2)
        status = $status
    }
}

function Compare-FailureRate {
    param(
        [double]$Current,
        [double]$WarnAbs,
        [double]$FailAbs
    )

    $status = "pass"
    if ($Current -ge $FailAbs) {
        $status = "fail"
    }
    elseif ($Current -ge $WarnAbs) {
        $status = "warn"
    }

    return [pscustomobject]@{
        name = "failure_rate"
        baseline = $null
        current = [math]::Round($Current, 6)
        regression_pct = $null
        status = $status
    }
}

$k6 = Get-Command k6 -ErrorAction SilentlyContinue
if ($null -eq $k6) {
    throw "k6 was not found on PATH. Install k6 and rerun perf/run.ps1."
}

New-Item -ItemType Directory -Force -Path $ResultsDir | Out-Null

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$tempSummaryPath = Join-Path $ResultsDir "k6-summary-$timestamp.json"
$resultPath = Join-Path $ResultsDir "$Scenario-$timestamp.json"

$gitSha = (git rev-parse --short HEAD 2>$null)
if (-not $gitSha) {
    $gitSha = "unknown"
}

$loginUrl = "$($BaseUrl.TrimEnd('/'))/login"

$scenarioScript = switch ($Scenario) {
    "login" { "perf/login.js" }
    "login-failure" { "perf/login-failure.js" }
    default { throw "Unknown scenario '$Scenario'. Supported scenarios: login, login-failure." }
}

if (-not $BaselinePath) {
    $BaselinePath = if ($Scenario -eq "login") {
        "perf/baseline.json"
    }
    else {
        "perf/baseline-$Scenario.json"
    }
}

& k6 run $scenarioScript `
    --env "LOGIN_BASE_URL=$loginUrl" `
    --summary-export $tempSummaryPath

if ($LASTEXITCODE -ne 0) {
    throw "k6 run failed with exit code $LASTEXITCODE."
}

$summary = Get-Content $tempSummaryPath -Raw | ConvertFrom-Json

$httpDuration = $summary.metrics.http_req_duration
$httpFailed = $summary.metrics.http_req_failed
$httpReqs = $summary.metrics.http_reqs

$result = [pscustomobject]@{
    scenario = $Scenario
    timestamp = (Get-Date).ToString("o")
    git_sha = $gitSha
    base_url = $BaseUrl
    metrics = [pscustomobject]@{
        p50_ms = [math]::Round((Get-MetricValue $httpDuration "p(50)"), 6)
        p95_ms = [math]::Round((Get-MetricValue $httpDuration "p(95)"), 6)
        p99_ms = [math]::Round((Get-MetricValue $httpDuration "p(99)"), 6)
        avg_ms = [math]::Round((Get-MetricValue $httpDuration "avg"), 6)
        throughput_rps = [math]::Round((Get-ScalarMetricValue $httpReqs "rate"), 6)
        failure_rate = [math]::Round((Get-ScalarMetricValue $httpFailed "value"), 6)
    }
}

$comparison = $null
$exitCode = 0

if (Test-Path $BaselinePath) {
    $baseline = Get-Content $BaselinePath -Raw | ConvertFrom-Json
    $warnPct = [double]$baseline.guardrails.warn_regression_pct
    $failPct = [double]$baseline.guardrails.fail_regression_pct
    $warnFailure = [double]$baseline.guardrails.failure_rate_warn_abs
    $failFailure = [double]$baseline.guardrails.failure_rate_fail_abs

    $comparison = @(
        Compare-Metric "p95_ms" $result.metrics.p95_ms $baseline.metrics.p95_ms $warnPct $failPct $true
        Compare-Metric "p99_ms" $result.metrics.p99_ms $baseline.metrics.p99_ms $warnPct $failPct $true
        Compare-Metric "avg_ms" $result.metrics.avg_ms $baseline.metrics.avg_ms $warnPct $failPct $true
        Compare-Metric "throughput_rps" $result.metrics.throughput_rps $baseline.metrics.throughput_rps $warnPct $failPct $false
        Compare-FailureRate $result.metrics.failure_rate $warnFailure $failFailure
    )

    if ($comparison.status -contains "fail") {
        $exitCode = 2
    }
    elseif ($comparison.status -contains "warn") {
        $exitCode = 1
    }
}

$output = [pscustomobject]@{
    run = $result
    comparison = $comparison
}

$output | ConvertTo-Json -Depth 6 | Set-Content $resultPath
Remove-Item $tempSummaryPath -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "Stored perf summary:" $resultPath
Write-Host ""
Write-Host ($result | ConvertTo-Json -Depth 4)

if ($comparison) {
    Write-Host ""
    Write-Host "Comparison against baseline:"
    $comparison | Format-Table -AutoSize | Out-String | Write-Host
}

exit $exitCode
