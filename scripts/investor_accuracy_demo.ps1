# ============================================================================
# Encapure Accuracy Demo for Investors
# ============================================================================
# This script demonstrates Encapure's context-aware semantic search capability.
# It shows how the same query returns different tools based on agent context.
# ============================================================================

param(
    [string]$Endpoint = "http://127.0.0.1:8080/search",
    [int]$TopK = 3
)

$Host.UI.RawUI.WindowTitle = "Encapure Accuracy Demo"

function Write-Header {
    param([string]$Text)
    Write-Host ""
    Write-Host ("=" * 80) -ForegroundColor Cyan
    Write-Host "  $Text" -ForegroundColor Cyan
    Write-Host ("=" * 80) -ForegroundColor Cyan
    Write-Host ""
}

function Write-TestCase {
    param(
        [int]$TestNumber,
        [string]$Query,
        [string]$AgentDescription,
        [string[]]$ExpectedTools,
        [string]$Category
    )

    Write-Host "┌──────────────────────────────────────────────────────────────────────────────┐" -ForegroundColor DarkGray
    Write-Host "│ TEST $TestNumber - $Category" -ForegroundColor White
    Write-Host "├──────────────────────────────────────────────────────────────────────────────┤" -ForegroundColor DarkGray
    Write-Host "│ " -NoNewline -ForegroundColor DarkGray
    Write-Host "QUERY: " -NoNewline -ForegroundColor Yellow
    Write-Host "`"$Query`"" -ForegroundColor White
    Write-Host "│ " -NoNewline -ForegroundColor DarkGray
    Write-Host "AGENT CONTEXT: " -NoNewline -ForegroundColor Yellow
    if ($AgentDescription) {
        Write-Host "`"$AgentDescription`"" -ForegroundColor Magenta
    } else {
        Write-Host "(none)" -ForegroundColor DarkGray
    }
    Write-Host "│ " -NoNewline -ForegroundColor DarkGray
    Write-Host "EXPECTED: " -NoNewline -ForegroundColor Yellow
    Write-Host ($ExpectedTools -join ", ") -ForegroundColor Gray
    Write-Host "├──────────────────────────────────────────────────────────────────────────────┤" -ForegroundColor DarkGray

    # Make API call
    $Body = @{ query = $Query; top_k = $TopK }
    if ($AgentDescription) {
        $Body.agent_description = $AgentDescription
    }

    try {
        $Response = Invoke-RestMethod -Uri $Endpoint -Method Post -ContentType "application/json" -Body ($Body | ConvertTo-Json) -TimeoutSec 30

        Write-Host "│ " -NoNewline -ForegroundColor DarkGray
        Write-Host "RESULTS:" -ForegroundColor Green

        $MatchFound = $false
        $Position = 1
        foreach ($Result in $Response.results) {
            $ToolName = $Result.name
            $Score = [math]::Round($Result.score * 100, 1)

            $IsExpected = $ExpectedTools -contains $ToolName
            if ($IsExpected) { $MatchFound = $true }

            $ScoreBar = "█" * [math]::Floor($Score / 5)
            $ScoreColor = if ($Score -ge 80) { "Green" } elseif ($Score -ge 50) { "Yellow" } else { "Red" }
            $NameColor = if ($IsExpected) { "Green" } else { "White" }
            $MatchIndicator = if ($IsExpected) { " ✓" } else { "" }

            Write-Host "│   " -NoNewline -ForegroundColor DarkGray
            Write-Host "$Position. " -NoNewline -ForegroundColor Gray
            Write-Host "$ToolName" -NoNewline -ForegroundColor $NameColor
            Write-Host "$MatchIndicator" -NoNewline -ForegroundColor Green
            Write-Host ""
            Write-Host "│      " -NoNewline -ForegroundColor DarkGray
            Write-Host "Score: " -NoNewline -ForegroundColor Gray
            Write-Host "$Score%" -NoNewline -ForegroundColor $ScoreColor
            Write-Host " $ScoreBar" -ForegroundColor $ScoreColor

            $Position++
        }

        Write-Host "├──────────────────────────────────────────────────────────────────────────────┤" -ForegroundColor DarkGray
        Write-Host "│ " -NoNewline -ForegroundColor DarkGray
        if ($MatchFound) {
            Write-Host "STATUS: PASS ✓" -ForegroundColor Green
        } else {
            Write-Host "STATUS: FAIL ✗" -ForegroundColor Red
        }
        Write-Host "└──────────────────────────────────────────────────────────────────────────────┘" -ForegroundColor DarkGray
        Write-Host ""

        return $MatchFound
    }
    catch {
        Write-Host "│ " -NoNewline -ForegroundColor DarkGray
        Write-Host "ERROR: $($_.Exception.Message)" -ForegroundColor Red
        Write-Host "└──────────────────────────────────────────────────────────────────────────────┘" -ForegroundColor DarkGray
        Write-Host ""
        return $false
    }
}

# ============================================================================
# MAIN DEMO
# ============================================================================

Clear-Host

Write-Host ""
Write-Host "    ███████╗███╗   ██╗ ██████╗ █████╗ ██████╗ ██╗   ██╗██████╗ ███████╗" -ForegroundColor Cyan
Write-Host "    ██╔════╝████╗  ██║██╔════╝██╔══██╗██╔══██╗██║   ██║██╔══██╗██╔════╝" -ForegroundColor Cyan
Write-Host "    █████╗  ██╔██╗ ██║██║     ███████║██████╔╝██║   ██║██████╔╝█████╗  " -ForegroundColor Cyan
Write-Host "    ██╔══╝  ██║╚██╗██║██║     ██╔══██║██╔═══╝ ██║   ██║██╔══██╗██╔══╝  " -ForegroundColor Cyan
Write-Host "    ███████╗██║ ╚████║╚██████╗██║  ██║██║     ╚██████╔╝██║  ██║███████╗" -ForegroundColor Cyan
Write-Host "    ╚══════╝╚═╝  ╚═══╝ ╚═════╝╚═╝  ╚═╝╚═╝      ╚═════╝ ╚═╝  ╚═╝╚══════╝" -ForegroundColor Cyan
Write-Host ""
Write-Host "                    Context-Aware Semantic Tool Search" -ForegroundColor White
Write-Host "                         Accuracy Demonstration" -ForegroundColor Gray
Write-Host ""

# Check server health
Write-Host "Checking server connection... " -NoNewline
try {
    $Health = Invoke-RestMethod -Uri "http://127.0.0.1:8080/health" -TimeoutSec 5
    Write-Host "Connected ✓" -ForegroundColor Green
    Write-Host ""
} catch {
    Write-Host "FAILED ✗" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please start the Encapure server first:" -ForegroundColor Yellow
    Write-Host '  $env:ENCAPURE_MODE = "single"' -ForegroundColor Gray
    Write-Host '  $env:TOOLS_PATH = "tests/data/comprehensive_mock_tools.json"' -ForegroundColor Gray
    Write-Host '  .\target\release\encapure.exe' -ForegroundColor Gray
    Write-Host ""
    exit 1
}

$PassCount = 0
$TotalCount = 0

# ============================================================================
# DEMO 1: Context Switching - Same Query, Different Results
# ============================================================================

Write-Header "DEMO 1: Context-Aware Search - Same Query, Different Results"

Write-Host "  The SAME query 'send message' returns DIFFERENT tools based on agent context." -ForegroundColor White
Write-Host "  This demonstrates how Encapure understands the user's working environment." -ForegroundColor Gray
Write-Host ""

$TotalCount++
if (Write-TestCase -TestNumber 1 -Query "send message" -AgentDescription $null `
    -ExpectedTools @("send_message", "send_sms", "send_notification") -Category "No Context (Generic)") {
    $PassCount++
}

Read-Host "Press Enter to continue..."

$TotalCount++
if (Write-TestCase -TestNumber 2 -Query "send message" -AgentDescription "Slack communication bot for workplace chat" `
    -ExpectedTools @("send_slack_message", "send_slack_dm", "post_slack_message") -Category "Slack Context") {
    $PassCount++
}

Read-Host "Press Enter to continue..."

$TotalCount++
if (Write-TestCase -TestNumber 3 -Query "send message" -AgentDescription "Email automation assistant for newsletters" `
    -ExpectedTools @("send_email", "send_email_notification", "send_html_email") -Category "Email Context") {
    $PassCount++
}

Read-Host "Press Enter to continue..."

$TotalCount++
if (Write-TestCase -TestNumber 4 -Query "send message" -AgentDescription "Microsoft Teams bot for enterprise collaboration" `
    -ExpectedTools @("send_teams_message", "send_teams_chat", "send_teams_channel_message") -Category "Microsoft Teams Context") {
    $PassCount++
}

# ============================================================================
# DEMO 2: Cloud Provider Context
# ============================================================================

Write-Header "DEMO 2: Cloud Provider Disambiguation"

Write-Host "  The query 'create server' returns provider-specific tools based on context." -ForegroundColor White
Write-Host "  Encapure understands which cloud platform the agent is managing." -ForegroundColor Gray
Write-Host ""

Read-Host "Press Enter to continue..."

$TotalCount++
if (Write-TestCase -TestNumber 5 -Query "create server" -AgentDescription "AWS cloud infrastructure bot" `
    -ExpectedTools @("aws_create_instance", "aws_start_instance") -Category "AWS Context") {
    $PassCount++
}

Read-Host "Press Enter to continue..."

$TotalCount++
if (Write-TestCase -TestNumber 6 -Query "create server" -AgentDescription "Azure cloud management assistant" `
    -ExpectedTools @("azure_create_vm", "azure_start_vm") -Category "Azure Context") {
    $PassCount++
}

Read-Host "Press Enter to continue..."

$TotalCount++
if (Write-TestCase -TestNumber 7 -Query "create server" -AgentDescription "GCP compute engine operator" `
    -ExpectedTools @("gcp_create_instance", "gcp_start_instance") -Category "Google Cloud Context") {
    $PassCount++
}

# ============================================================================
# DEMO 3: Project Management Tools
# ============================================================================

Write-Header "DEMO 3: Project Management Platform Selection"

Write-Host "  The query 'create task' returns platform-specific tools." -ForegroundColor White
Write-Host "  Each project management tool has its own naming conventions." -ForegroundColor Gray
Write-Host ""

Read-Host "Press Enter to continue..."

$TotalCount++
if (Write-TestCase -TestNumber 8 -Query "create task" -AgentDescription "Jira project management assistant" `
    -ExpectedTools @("jira_create_task", "jira_create_issue", "jira_create_subtask") -Category "Jira Context") {
    $PassCount++
}

Read-Host "Press Enter to continue..."

$TotalCount++
if (Write-TestCase -TestNumber 9 -Query "create task" -AgentDescription "Asana productivity assistant" `
    -ExpectedTools @("asana_create_task", "asana_create_subtask") -Category "Asana Context") {
    $PassCount++
}

Read-Host "Press Enter to continue..."

$TotalCount++
if (Write-TestCase -TestNumber 10 -Query "create issue" -AgentDescription "Linear issue tracking assistant" `
    -ExpectedTools @("linear_create_issue", "linear_create_bug", "linear_create_feature") -Category "Linear Context") {
    $PassCount++
}

# ============================================================================
# DEMO 4: Database Operations
# ============================================================================

Write-Header "DEMO 4: Database Technology Selection"

Write-Host "  Database queries are routed to the correct backend (SQL vs NoSQL)." -ForegroundColor White
Write-Host ""

Read-Host "Press Enter to continue..."

$TotalCount++
if (Write-TestCase -TestNumber 11 -Query "query data" -AgentDescription "PostgreSQL database administrator" `
    -ExpectedTools @("query_database", "run_sql_query", "execute_sql") -Category "SQL Context") {
    $PassCount++
}

Read-Host "Press Enter to continue..."

$TotalCount++
if (Write-TestCase -TestNumber 12 -Query "find documents" -AgentDescription "MongoDB data engineer" `
    -ExpectedTools @("mongodb_find", "mongodb_find_one", "mongodb_aggregate") -Category "MongoDB Context") {
    $PassCount++
}

# ============================================================================
# FINAL RESULTS
# ============================================================================

Write-Header "FINAL RESULTS"

$Accuracy = [math]::Round(($PassCount / $TotalCount) * 100, 1)

Write-Host "  Total Tests:  $TotalCount" -ForegroundColor White
Write-Host "  Passed:       $PassCount" -ForegroundColor Green
Write-Host "  Failed:       $($TotalCount - $PassCount)" -ForegroundColor $(if ($TotalCount - $PassCount -eq 0) { "Gray" } else { "Red" })
Write-Host ""

# Accuracy bar
$BarLength = 40
$FilledLength = [math]::Floor($Accuracy / 100 * $BarLength)
$Bar = ("█" * $FilledLength) + ("░" * ($BarLength - $FilledLength))

Write-Host "  Accuracy: " -NoNewline -ForegroundColor White
Write-Host "$Bar" -NoNewline -ForegroundColor $(if ($Accuracy -ge 90) { "Green" } elseif ($Accuracy -ge 70) { "Yellow" } else { "Red" })
Write-Host " $Accuracy%" -ForegroundColor $(if ($Accuracy -ge 90) { "Green" } elseif ($Accuracy -ge 70) { "Yellow" } else { "Red" })

Write-Host ""
Write-Host ("=" * 80) -ForegroundColor Cyan
Write-Host ""
Write-Host "  KEY TAKEAWAY:" -ForegroundColor Yellow
Write-Host "  Encapure's context-aware search correctly routes queries to domain-specific" -ForegroundColor White
Write-Host "  tools based on the agent's role, eliminating ambiguity and improving accuracy." -ForegroundColor White
Write-Host ""
