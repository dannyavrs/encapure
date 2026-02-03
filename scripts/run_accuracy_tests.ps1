# Encapure Accuracy Test Runner
# Runs 50 test cases and reports results

$TestFile = "tests/data/accuracy_test_cases.json"
$Endpoint = "http://127.0.0.1:8080/search"
$TopK = 3

# Load test cases
$TestData = Get-Content $TestFile | ConvertFrom-Json
$TestCases = $TestData.test_cases

Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host "Encapure Accuracy Test Suite" -ForegroundColor Cyan
Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host "Total test cases: $($TestCases.Count)"
Write-Host ""

$Results = @{
    Passed = 0
    PartialMatch = 0
    Failed = 0
    Errors = 0
}

$DetailedResults = @()

foreach ($Test in $TestCases) {
    $Body = @{
        query = $Test.query
        top_k = $TopK
    }

    if ($Test.agent_description) {
        $Body.agent_description = $Test.agent_description
    }

    try {
        $Response = Invoke-RestMethod -Uri $Endpoint -Method Post -ContentType "application/json" -Body ($Body | ConvertTo-Json)
        $ReturnedTools = $Response.results | ForEach-Object { $_.name }

        # Check matches
        $ExpectedSet = [System.Collections.Generic.HashSet[string]]::new([string[]]$Test.expected_tools)
        $ReturnedSet = [System.Collections.Generic.HashSet[string]]::new([string[]]$ReturnedTools)

        $Matches = $ExpectedSet | Where-Object { $ReturnedSet.Contains($_) }
        $MatchCount = ($Matches | Measure-Object).Count

        $Status = "FAIL"
        $Color = "Red"

        if ($MatchCount -eq $Test.expected_tools.Count) {
            $Status = "PASS"
            $Color = "Green"
            $Results.Passed++
        }
        elseif ($MatchCount -gt 0) {
            $Status = "PARTIAL"
            $Color = "Yellow"
            $Results.PartialMatch++
        }
        else {
            $Results.Failed++
        }

        $DetailedResults += [PSCustomObject]@{
            ID = $Test.id
            Category = $Test.category
            Status = $Status
            Query = $Test.query
            AgentContext = if ($Test.agent_description) { $Test.agent_description.Substring(0, [Math]::Min(30, $Test.agent_description.Length)) + "..." } else { "none" }
            Expected = ($Test.expected_tools -join ", ")
            Returned = ($ReturnedTools -join ", ")
            MatchCount = "$MatchCount/$($Test.expected_tools.Count)"
        }

        # Print progress
        Write-Host "[$Status] " -ForegroundColor $Color -NoNewline
        Write-Host "Test $($Test.id): $($Test.description)" -NoNewline
        Write-Host " | Matches: $MatchCount/$($Test.expected_tools.Count)" -ForegroundColor Gray

    }
    catch {
        $Results.Errors++
        Write-Host "[ERROR] " -ForegroundColor Magenta -NoNewline
        Write-Host "Test $($Test.id): $($_.Exception.Message)"
    }
}

Write-Host ""
Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host "RESULTS SUMMARY" -ForegroundColor Cyan
Write-Host "=" * 70 -ForegroundColor Cyan
Write-Host "Passed (all expected):    $($Results.Passed)" -ForegroundColor Green
Write-Host "Partial (some expected):  $($Results.PartialMatch)" -ForegroundColor Yellow
Write-Host "Failed (no match):        $($Results.Failed)" -ForegroundColor Red
Write-Host "Errors:                   $($Results.Errors)" -ForegroundColor Magenta
Write-Host ""
$Total = $Results.Passed + $Results.PartialMatch + $Results.Failed
$SuccessRate = [math]::Round(($Results.Passed + $Results.PartialMatch) / $Total * 100, 1)
Write-Host "Success Rate (Pass + Partial): $SuccessRate%" -ForegroundColor Cyan
Write-Host ""

# Group by category
Write-Host "Results by Category:" -ForegroundColor Cyan
$DetailedResults | Group-Object Category | ForEach-Object {
    $CategoryPassed = ($_.Group | Where-Object { $_.Status -eq "PASS" }).Count
    $CategoryTotal = $_.Count
    Write-Host "  $($_.Name): $CategoryPassed/$CategoryTotal passed"
}

# Export detailed results
$DetailedResults | Export-Csv -Path "accuracy_test_results.csv" -NoTypeInformation
Write-Host ""
Write-Host "Detailed results exported to: accuracy_test_results.csv" -ForegroundColor Gray
