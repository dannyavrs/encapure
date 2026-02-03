# ENCAPURE ACCURACY DEMONSTRATION FOR INVESTORS

param(
    [switch]$AutoRun
)

$Endpoint = "http://127.0.0.1:8080/search"

function Show-TestResult {
    param(
        [int]$TestNum,
        [string]$Query,
        [string]$AgentContext,
        [string[]]$Expected,
        [string]$Category
    )

    Write-Host ""
    Write-Host "+-------------------------------------------------------------------------+" -ForegroundColor DarkGray
    Write-Host "| TEST $TestNum - $Category" -ForegroundColor White
    Write-Host "+-------------------------------------------------------------------------+" -ForegroundColor DarkGray

    Write-Host "| QUERY: " -NoNewline -ForegroundColor Yellow
    Write-Host "`"$Query`"" -ForegroundColor White

    Write-Host "| CONTEXT: " -NoNewline -ForegroundColor Yellow
    if ($AgentContext) {
        Write-Host "`"$AgentContext`"" -ForegroundColor Magenta
    } else {
        Write-Host "(none - generic search)" -ForegroundColor DarkGray
    }

    Write-Host "| EXPECTED: " -NoNewline -ForegroundColor Yellow
    Write-Host ($Expected -join ", ") -ForegroundColor Gray

    Write-Host "+-------------------------------------------------------------------------+" -ForegroundColor DarkGray

    $Body = @{ query = $Query; top_k = 5 }
    if ($AgentContext) { $Body.agent_description = $AgentContext }

    try {
        $Stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        $Response = Invoke-RestMethod -Uri $Endpoint -Method Post -Body ($Body | ConvertTo-Json) -ContentType "application/json"
        $Stopwatch.Stop()
        $Latency = $Stopwatch.ElapsedMilliseconds

        Write-Host "| RETURNED TOOLS:" -ForegroundColor Green

        $MatchFound = $false
        $Rank = 1
        foreach ($Result in $Response.results) {
            $Name = $Result.name
            $Score = [math]::Round($Result.score * 100, 1)
            $IsMatch = $Expected -contains $Name
            if ($IsMatch) { $MatchFound = $true }

            $MatchMark = if ($IsMatch) { " [MATCH]" } else { "" }
            $NameColor = if ($IsMatch) { "Green" } else { "White" }
            $ScoreColor = if ($Score -ge 70) { "Green" } elseif ($Score -ge 40) { "Yellow" } else { "Gray" }

            Write-Host "|   $Rank. " -NoNewline -ForegroundColor Gray
            Write-Host "$Name" -NoNewline -ForegroundColor $NameColor
            Write-Host "$MatchMark" -ForegroundColor Green
            Write-Host "|      Confidence: " -NoNewline -ForegroundColor Gray
            Write-Host "$Score%" -ForegroundColor $ScoreColor

            $Rank++
        }

        Write-Host "+-------------------------------------------------------------------------+" -ForegroundColor DarkGray

        $LatencyColor = if ($Latency -lt 100) { "Green" } elseif ($Latency -lt 200) { "Yellow" } else { "Red" }
        $script:TotalLatency += $Latency

        if ($MatchFound) {
            Write-Host "| RESULT: PASS" -NoNewline -ForegroundColor Green
            Write-Host "                                        LATENCY: " -NoNewline -ForegroundColor Gray
            Write-Host "${Latency}ms" -ForegroundColor $LatencyColor
        } else {
            Write-Host "| RESULT: FAIL" -NoNewline -ForegroundColor Red
            Write-Host "                                        LATENCY: " -NoNewline -ForegroundColor Gray
            Write-Host "${Latency}ms" -ForegroundColor $LatencyColor
        }
        Write-Host "+-------------------------------------------------------------------------+" -ForegroundColor DarkGray

        return $MatchFound
    }
    catch {
        Write-Host "| ERROR: $($_.Exception.Message)" -ForegroundColor Red
        Write-Host "+-------------------------------------------------------------------------+" -ForegroundColor DarkGray
        return $false
    }
}

# MAIN
Clear-Host

Write-Host ""
Write-Host "=========================================================================" -ForegroundColor Cyan
Write-Host "              ENCAPURE - ACCURACY DEMONSTRATION" -ForegroundColor Cyan
Write-Host "          Context-Aware Semantic Tool Search Engine" -ForegroundColor Cyan
Write-Host "=========================================================================" -ForegroundColor Cyan
Write-Host ""

# Check server
Write-Host "Connecting to Encapure server... " -NoNewline
try {
    $null = Invoke-RestMethod -Uri "http://127.0.0.1:8080/health" -TimeoutSec 3
    Write-Host "Connected!" -ForegroundColor Green
}
catch {
    Write-Host "FAILED" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please start the server first:" -ForegroundColor Yellow
    Write-Host '  $env:ENCAPURE_MODE = "single"'
    Write-Host '  $env:TOOLS_PATH = "tests/data/comprehensive_mock_tools.json"'
    Write-Host '  .\target\release\encapure.exe'
    exit 1
}

$Passed = 0
$Total = 0
$script:TotalLatency = 0

# DEMO 1
Write-Host ""
Write-Host "=========================================================================" -ForegroundColor White
Write-Host " DEMO 1: Context-Aware Routing" -ForegroundColor White
Write-Host " The SAME query returns DIFFERENT tools based on agent context" -ForegroundColor Gray
Write-Host "=========================================================================" -ForegroundColor White

$Total++
if (Show-TestResult -TestNum 1 -Query "send message" -AgentContext $null -Expected @("send_message", "send_sms", "send_notification") -Category "No Context") { $Passed++ }

if (-not $AutoRun) { Read-Host "Press Enter to continue" }

$Total++
if (Show-TestResult -TestNum 2 -Query "send message" -AgentContext "Slack communication bot" -Expected @("send_slack_message", "send_slack_dm", "post_slack_message") -Category "Slack Context") { $Passed++ }

if (-not $AutoRun) { Read-Host "Press Enter to continue" }

$Total++
if (Show-TestResult -TestNum 3 -Query "send message" -AgentContext "Email automation assistant" -Expected @("send_email", "send_email_notification", "send_html_email") -Category "Email Context") { $Passed++ }

if (-not $AutoRun) { Read-Host "Press Enter to continue" }

$Total++
if (Show-TestResult -TestNum 4 -Query "send message" -AgentContext "Microsoft Teams bot" -Expected @("send_teams_message", "send_teams_chat", "send_teams_channel_message") -Category "Teams Context") { $Passed++ }

# DEMO 2
Write-Host ""
Write-Host "=========================================================================" -ForegroundColor White
Write-Host " DEMO 2: Cloud Provider Disambiguation" -ForegroundColor White
Write-Host " Query 'create server' routes to the correct cloud platform" -ForegroundColor Gray
Write-Host "=========================================================================" -ForegroundColor White

if (-not $AutoRun) { Read-Host "Press Enter to continue" }

$Total++
if (Show-TestResult -TestNum 5 -Query "create server" -AgentContext "AWS cloud infrastructure bot" -Expected @("aws_create_instance", "aws_start_instance") -Category "AWS") { $Passed++ }

if (-not $AutoRun) { Read-Host "Press Enter to continue" }

$Total++
if (Show-TestResult -TestNum 6 -Query "create server" -AgentContext "Azure cloud management" -Expected @("azure_create_vm", "azure_start_vm") -Category "Azure") { $Passed++ }

if (-not $AutoRun) { Read-Host "Press Enter to continue" }

$Total++
if (Show-TestResult -TestNum 7 -Query "create server" -AgentContext "GCP compute engine operator" -Expected @("gcp_create_instance", "gcp_start_instance") -Category "Google Cloud") { $Passed++ }

# DEMO 3
Write-Host ""
Write-Host "=========================================================================" -ForegroundColor White
Write-Host " DEMO 3: Project Management Platform Selection" -ForegroundColor White
Write-Host "=========================================================================" -ForegroundColor White

if (-not $AutoRun) { Read-Host "Press Enter to continue" }

$Total++
if (Show-TestResult -TestNum 8 -Query "create task" -AgentContext "Jira project management" -Expected @("jira_create_task", "jira_create_issue", "jira_create_subtask") -Category "Jira") { $Passed++ }

if (-not $AutoRun) { Read-Host "Press Enter to continue" }

$Total++
if (Show-TestResult -TestNum 9 -Query "create task" -AgentContext "Asana productivity assistant" -Expected @("asana_create_task", "asana_create_subtask") -Category "Asana") { $Passed++ }

# DEMO 4
Write-Host ""
Write-Host "=========================================================================" -ForegroundColor White
Write-Host " DEMO 4: Database Technology Selection" -ForegroundColor White
Write-Host "=========================================================================" -ForegroundColor White

if (-not $AutoRun) { Read-Host "Press Enter to continue" }

$Total++
if (Show-TestResult -TestNum 10 -Query "query data" -AgentContext "PostgreSQL database administrator" -Expected @("query_database", "run_sql_query", "execute_sql") -Category "SQL") { $Passed++ }

if (-not $AutoRun) { Read-Host "Press Enter to continue" }

$Total++
if (Show-TestResult -TestNum 11 -Query "find documents" -AgentContext "MongoDB data engineer" -Expected @("mongodb_find", "mongodb_find_one", "mongodb_aggregate") -Category "MongoDB") { $Passed++ }

# FINAL RESULTS
Write-Host ""
Write-Host "=========================================================================" -ForegroundColor Cyan
Write-Host "                         FINAL RESULTS" -ForegroundColor Cyan
Write-Host "=========================================================================" -ForegroundColor Cyan
Write-Host ""

$Accuracy = [math]::Round(($Passed / $Total) * 100)
$AccuracyColor = if ($Accuracy -ge 90) { "Green" } elseif ($Accuracy -ge 70) { "Yellow" } else { "Red" }
$AvgLatency = [math]::Round($script:TotalLatency / $Total)
$LatencyColor = if ($AvgLatency -lt 100) { "Green" } elseif ($AvgLatency -lt 200) { "Yellow" } else { "Red" }

Write-Host "   Tests Passed:  $Passed / $Total" -ForegroundColor $AccuracyColor
Write-Host "   Accuracy:      $Accuracy%" -ForegroundColor $AccuracyColor
Write-Host "   Avg Latency:   " -NoNewline
Write-Host "${AvgLatency}ms" -ForegroundColor $LatencyColor
Write-Host ""
Write-Host "   KEY INSIGHT:" -ForegroundColor Yellow
Write-Host "   Same query + different context = different tools returned"
Write-Host "   Encapure understands agent roles for intelligent tool routing"
Write-Host ""
Write-Host "=========================================================================" -ForegroundColor Cyan
Write-Host ""
