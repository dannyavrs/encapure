# PowerShell Load Test Script for Encapure
# Usage: .\bench\load_test.ps1 -Concurrent 50 -Duration 30

param(
    [int]$Concurrent = 50,      # Number of concurrent requests
    [int]$Duration = 30,        # Test duration in seconds
    [string]$Url = "http://localhost:8080/rerank"
)

$body = @{
    query = "What is machine learning?"
    documents = @(
        "Machine learning is a type of artificial intelligence that learns from data"
        "The weather is nice today"
        "Deep learning uses neural networks for complex pattern recognition"
        "Python is a popular programming language"
        "Neural networks are inspired by biological neurons"
        "The stock market fluctuates daily"
        "Supervised learning requires labeled training data"
        "Coffee is a popular morning beverage"
    )
} | ConvertTo-Json

$headers = @{ "Content-Type" = "application/json" }

Write-Host "Starting load test..."
Write-Host "  URL: $Url"
Write-Host "  Concurrent: $Concurrent"
Write-Host "  Duration: $Duration seconds"
Write-Host ""

$startTime = Get-Date
$endTime = $startTime.AddSeconds($Duration)
$totalRequests = 0
$successCount = 0
$errorCount = 0
$latencies = [System.Collections.ArrayList]::new()

$jobs = @()

# Create worker script block
$workerScript = {
    param($Url, $Body, $Headers, $EndTime)

    $results = @{
        Success = 0
        Errors = 0
        Latencies = @()
    }

    while ((Get-Date) -lt $EndTime) {
        try {
            $sw = [System.Diagnostics.Stopwatch]::StartNew()
            $response = Invoke-RestMethod -Uri $Url -Method Post -Body $Body -Headers $Headers -TimeoutSec 60
            $sw.Stop()
            $results.Success++
            $results.Latencies += $sw.ElapsedMilliseconds
        }
        catch {
            $results.Errors++
        }
    }

    return $results
}

Write-Host "Spawning $Concurrent workers..."

# Start concurrent jobs
for ($i = 0; $i -lt $Concurrent; $i++) {
    $jobs += Start-Job -ScriptBlock $workerScript -ArgumentList $Url, $body, $headers, $endTime
}

Write-Host "Running for $Duration seconds..."

# Wait for all jobs
$jobs | Wait-Job | Out-Null

# Collect results
foreach ($job in $jobs) {
    $result = Receive-Job -Job $job
    $successCount += $result.Success
    $errorCount += $result.Errors
    $latencies.AddRange($result.Latencies)
}

$jobs | Remove-Job

$totalRequests = $successCount + $errorCount
$actualDuration = ((Get-Date) - $startTime).TotalSeconds

# Calculate statistics
$sortedLatencies = $latencies | Sort-Object
$p50 = if ($sortedLatencies.Count -gt 0) { $sortedLatencies[[math]::Floor($sortedLatencies.Count * 0.50)] } else { 0 }
$p95 = if ($sortedLatencies.Count -gt 0) { $sortedLatencies[[math]::Floor($sortedLatencies.Count * 0.95)] } else { 0 }
$p99 = if ($sortedLatencies.Count -gt 0) { $sortedLatencies[[math]::Floor($sortedLatencies.Count * 0.99)] } else { 0 }
$avg = if ($sortedLatencies.Count -gt 0) { ($sortedLatencies | Measure-Object -Average).Average } else { 0 }

Write-Host ""
Write-Host "===== Load Test Results ====="
Write-Host "Duration:        $([math]::Round($actualDuration, 2)) seconds"
Write-Host "Total Requests:  $totalRequests"
Write-Host "Successful:      $successCount"
Write-Host "Errors:          $errorCount"
Write-Host "Throughput:      $([math]::Round($totalRequests / $actualDuration, 2)) req/s"
Write-Host ""
Write-Host "Latency (ms):"
Write-Host "  Average:       $([math]::Round($avg, 2))"
Write-Host "  P50:           $p50"
Write-Host "  P95:           $p95"
Write-Host "  P99:           $p99"
Write-Host ""

# Check against targets
$throughput = $totalRequests / $actualDuration
if ($throughput -ge 200) {
    Write-Host "[PASS] Throughput >= 200 req/s" -ForegroundColor Green
} else {
    Write-Host "[FAIL] Throughput < 200 req/s (target: 200+)" -ForegroundColor Red
}

if ($p99 -le 50) {
    Write-Host "[PASS] P99 <= 50ms" -ForegroundColor Green
} else {
    Write-Host "[WARN] P99 > 50ms (target: <50ms)" -ForegroundColor Yellow
}
