# Scenario-based Load Test for Encapure v1.1
# Tests different request patterns to understand performance characteristics
#
# Usage: .\bench\scenario_test.ps1

param(
    [string]$Url = "http://localhost:8080/rerank"
)

$headers = @{ "Content-Type" = "application/json" }

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Encapure v1.1 Scenario Load Test" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# -----------------------------------------------------------------------------
# Scenario 1: Single LONG request (16 documents - max limit)
# -----------------------------------------------------------------------------
Write-Host "[Scenario 1] Single LONG Request (16 documents)" -ForegroundColor Yellow
Write-Host "Testing maximum batch size with longer documents..."
Write-Host ""

$longBody = @{
    query = "What are the key principles and best practices of machine learning model training and optimization?"
    documents = @(
        "Machine learning is a subset of artificial intelligence that enables systems to learn and improve from experience without being explicitly programmed. It focuses on developing algorithms that can access data and use it to learn for themselves."
        "Deep learning is a type of machine learning based on artificial neural networks with multiple layers. These deep neural networks attempt to simulate the behavior of the human brain in processing data and creating patterns for decision making."
        "The weather forecast for tomorrow indicates partly cloudy skies with a high temperature of 72 degrees Fahrenheit and a low of 58 degrees. There is a 20% chance of precipitation in the afternoon hours."
        "Natural language processing is a branch of AI that helps computers understand, interpret and manipulate human language. NLP draws from many disciplines including computer science and computational linguistics."
        "Supervised learning is the machine learning task of learning a function that maps an input to an output based on example input-output pairs. It infers a function from labeled training data consisting of a set of training examples."
        "My favorite recipe for chocolate chip cookies requires two cups of flour, one cup of butter, and a generous amount of chocolate chips. Bake at 350 degrees for exactly twelve minutes."
        "Reinforcement learning is an area of machine learning concerned with how intelligent agents ought to take actions in an environment in order to maximize the notion of cumulative reward through trial and error."
        "The stock market experienced significant volatility today with the S&P 500 index fluctuating between gains and losses throughout the trading session before closing marginally higher."
        "Transfer learning is a research problem in machine learning that focuses on storing knowledge gained while solving one problem and applying it to a different but related problem for improved performance."
        "Gradient descent is an optimization algorithm used to minimize some function by iteratively moving in the direction of steepest descent as defined by the negative of the gradient."
        "The hiking trail through the national park offers stunning views of the mountain range and is approximately five miles long with an elevation gain of two thousand feet."
        "Overfitting occurs when a statistical model describes random error or noise instead of the underlying relationship. Overfitting generally occurs when a model is excessively complex."
        "Cross-validation is a resampling procedure used to evaluate machine learning models on a limited data sample. It has a single parameter called k that refers to the number of groups."
        "The new smartphone model features an improved camera system with enhanced low-light performance and optical image stabilization for capturing better photos and videos."
        "Batch normalization is a technique for improving the performance and stability of artificial neural networks by normalizing the inputs to each layer within a mini-batch."
        "Feature engineering is the process of using domain knowledge to extract features from raw data via data mining techniques to improve machine learning model performance."
    )
} | ConvertTo-Json -Depth 3

# Warm-up request
Write-Host "  Warm-up..." -ForegroundColor Gray
try {
    $null = Invoke-RestMethod -Uri $Url -Method Post -Body $longBody -Headers $headers -TimeoutSec 60
} catch {
    Write-Host "  [ERROR] Warm-up failed: $_" -ForegroundColor Red
}

# Run 5 iterations
$longLatencies = @()
for ($i = 1; $i -le 5; $i++) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $response = Invoke-RestMethod -Uri $Url -Method Post -Body $longBody -Headers $headers -TimeoutSec 60
        $sw.Stop()
        $longLatencies += $sw.ElapsedMilliseconds
        Write-Host "  Run $i`: $($sw.ElapsedMilliseconds) ms" -ForegroundColor Green
    } catch {
        $sw.Stop()
        Write-Host "  Run $i`: FAILED - $_" -ForegroundColor Red
    }
}

$longAvg = if ($longLatencies.Count -gt 0) { [math]::Round(($longLatencies | Measure-Object -Average).Average, 2) } else { "N/A" }
$longMin = if ($longLatencies.Count -gt 0) { ($longLatencies | Measure-Object -Minimum).Minimum } else { "N/A" }
$longMax = if ($longLatencies.Count -gt 0) { ($longLatencies | Measure-Object -Maximum).Maximum } else { "N/A" }

Write-Host ""
Write-Host "  Results: Avg=$longAvg ms, Min=$longMin ms, Max=$longMax ms" -ForegroundColor Cyan
Write-Host ""

# -----------------------------------------------------------------------------
# Scenario 2: Two SMALL requests (3 documents each)
# -----------------------------------------------------------------------------
Write-Host "[Scenario 2] Two SMALL Requests (3 documents each)" -ForegroundColor Yellow
Write-Host "Testing typical small batch requests..."
Write-Host ""

$smallBody1 = @{
    query = "What is machine learning?"
    documents = @(
        "Machine learning is a type of artificial intelligence"
        "The weather is sunny today"
        "Neural networks learn from data"
    )
} | ConvertTo-Json

$smallBody2 = @{
    query = "How does deep learning work?"
    documents = @(
        "Deep learning uses multiple neural network layers"
        "I enjoy reading books"
        "Backpropagation trains neural networks"
    )
} | ConvertTo-Json

# Run 5 iterations of each small request
$small1Latencies = @()
$small2Latencies = @()

for ($i = 1; $i -le 5; $i++) {
    # Request 1
    $sw1 = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $null = Invoke-RestMethod -Uri $Url -Method Post -Body $smallBody1 -Headers $headers -TimeoutSec 60
        $sw1.Stop()
        $small1Latencies += $sw1.ElapsedMilliseconds
    } catch {
        $sw1.Stop()
    }

    # Request 2
    $sw2 = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $null = Invoke-RestMethod -Uri $Url -Method Post -Body $smallBody2 -Headers $headers -TimeoutSec 60
        $sw2.Stop()
        $small2Latencies += $sw2.ElapsedMilliseconds
    } catch {
        $sw2.Stop()
    }

    Write-Host "  Run $i`: Request1=$($sw1.ElapsedMilliseconds) ms, Request2=$($sw2.ElapsedMilliseconds) ms" -ForegroundColor Green
}

$small1Avg = if ($small1Latencies.Count -gt 0) { [math]::Round(($small1Latencies | Measure-Object -Average).Average, 2) } else { "N/A" }
$small2Avg = if ($small2Latencies.Count -gt 0) { [math]::Round(($small2Latencies | Measure-Object -Average).Average, 2) } else { "N/A" }

Write-Host ""
Write-Host "  Results: Request1 Avg=$small1Avg ms, Request2 Avg=$small2Avg ms" -ForegroundColor Cyan
Write-Host ""

# -----------------------------------------------------------------------------
# Scenario 3: Concurrent small requests (simulate real load)
# -----------------------------------------------------------------------------
Write-Host "[Scenario 3] Concurrent Small Requests" -ForegroundColor Yellow
Write-Host "Running 10 small requests in parallel..."
Write-Host ""

$concurrentScript = {
    param($Url, $Body, $Headers)
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $null = Invoke-RestMethod -Uri $Url -Method Post -Body $Body -Headers $Headers -TimeoutSec 60
        $sw.Stop()
        return @{ Success = $true; Latency = $sw.ElapsedMilliseconds }
    } catch {
        $sw.Stop()
        return @{ Success = $false; Latency = $sw.ElapsedMilliseconds }
    }
}

$jobs = @()
$sw = [System.Diagnostics.Stopwatch]::StartNew()

for ($i = 0; $i -lt 10; $i++) {
    $body = if ($i % 2 -eq 0) { $smallBody1 } else { $smallBody2 }
    $jobs += Start-Job -ScriptBlock $concurrentScript -ArgumentList $Url, $body, $headers
}

$jobs | Wait-Job | Out-Null
$sw.Stop()

$concurrentLatencies = @()
$successCount = 0
foreach ($job in $jobs) {
    $result = Receive-Job -Job $job
    if ($result.Success) {
        $successCount++
        $concurrentLatencies += $result.Latency
    }
}
$jobs | Remove-Job

$concurrentAvg = if ($concurrentLatencies.Count -gt 0) { [math]::Round(($concurrentLatencies | Measure-Object -Average).Average, 2) } else { "N/A" }
$concurrentMax = if ($concurrentLatencies.Count -gt 0) { ($concurrentLatencies | Measure-Object -Maximum).Maximum } else { "N/A" }
$totalTime = $sw.ElapsedMilliseconds
$throughput = if ($totalTime -gt 0) { [math]::Round($successCount / ($totalTime / 1000), 2) } else { 0 }

Write-Host "  Total time: $totalTime ms" -ForegroundColor Green
Write-Host "  Successful: $successCount / 10" -ForegroundColor Green
Write-Host "  Avg latency: $concurrentAvg ms" -ForegroundColor Green
Write-Host "  Max latency: $concurrentMax ms" -ForegroundColor Green
Write-Host "  Throughput: $throughput req/s" -ForegroundColor Green
Write-Host ""

# -----------------------------------------------------------------------------
# Summary
# -----------------------------------------------------------------------------
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  SUMMARY" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Scenario 1 (16 docs):     Avg $longAvg ms" -ForegroundColor White
Write-Host "  Scenario 2 (3 docs):      Avg $small1Avg ms / $small2Avg ms" -ForegroundColor White
Write-Host "  Scenario 3 (concurrent):  $throughput req/s, Avg $concurrentAvg ms" -ForegroundColor White
Write-Host ""

# Performance assessment
Write-Host "  Performance Assessment:" -ForegroundColor Yellow
if ($longAvg -ne "N/A" -and $longAvg -lt 500) {
    Write-Host "  [GOOD] Long requests under 500ms" -ForegroundColor Green
} elseif ($longAvg -ne "N/A") {
    Write-Host "  [SLOW] Long requests over 500ms (got $longAvg ms)" -ForegroundColor Yellow
}

if ($small1Avg -ne "N/A" -and $small1Avg -lt 200) {
    Write-Host "  [GOOD] Small requests under 200ms" -ForegroundColor Green
} elseif ($small1Avg -ne "N/A") {
    Write-Host "  [SLOW] Small requests over 200ms (got $small1Avg ms)" -ForegroundColor Yellow
}

if ($throughput -ge 20) {
    Write-Host "  [GOOD] Concurrent throughput >= 20 req/s" -ForegroundColor Green
} else {
    Write-Host "  [NEEDS WORK] Concurrent throughput < 20 req/s (got $throughput req/s)" -ForegroundColor Yellow
}

Write-Host ""
