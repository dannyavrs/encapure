#!/bin/bash
# Investor Demo Preview - Clear and Simple Output

ENDPOINT="http://127.0.0.1:8080/search"

echo ""
echo "╔══════════════════════════════════════════════════════════════════════════════╗"
echo "║                        ENCAPURE ACCURACY DEMONSTRATION                       ║"
echo "║                     Context-Aware Semantic Tool Search                       ║"
echo "╚══════════════════════════════════════════════════════════════════════════════╝"
echo ""

run_test() {
    local num="$1"
    local query="$2"
    local context="$3"
    local expected="$4"
    local category="$5"

    echo "┌──────────────────────────────────────────────────────────────────────────────┐"
    echo "│ TEST $num - $category"
    echo "├──────────────────────────────────────────────────────────────────────────────┤"
    echo "│ QUERY:         \"$query\""

    if [ -z "$context" ]; then
        echo "│ AGENT CONTEXT: (none)"
        body="{\"query\": \"$query\", \"top_k\": 3}"
    else
        echo "│ AGENT CONTEXT: \"$context\""
        body="{\"query\": \"$query\", \"top_k\": 3, \"agent_description\": \"$context\"}"
    fi

    echo "│ EXPECTED:      $expected"
    echo "├──────────────────────────────────────────────────────────────────────────────┤"
    echo "│ RESULTS:"

    response=$(curl -s -X POST "$ENDPOINT" -H "Content-Type: application/json" -d "$body")

    # Extract tool names and scores using grep/sed
    names=$(echo "$response" | grep -oE '"name":"[^"]*"' | sed 's/"name":"//g' | sed 's/"//g')
    scores=$(echo "$response" | grep -oE '"score":[0-9.]+' | sed 's/"score"://g')

    # Convert to arrays
    IFS=$'\n' read -rd '' -a name_arr <<< "$names"
    IFS=$'\n' read -rd '' -a score_arr <<< "$scores"

    match_found=0
    for i in "${!name_arr[@]}"; do
        name="${name_arr[$i]}"
        score="${score_arr[$i]}"
        pct=$(echo "$score * 100" | bc 2>/dev/null | cut -d. -f1)
        [ -z "$pct" ] && pct=$(printf "%.0f" $(echo "$score * 100" | awk '{print $1}'))

        # Check if this tool is expected
        marker=""
        if echo ",$expected," | grep -q ",$name,"; then
            marker=" ✓"
            match_found=1
        fi

        echo "│   $((i+1)). $name$marker"
        echo "│      Score: ${pct}%"
    done

    echo "├──────────────────────────────────────────────────────────────────────────────┤"
    if [ $match_found -eq 1 ]; then
        echo -e "│ STATUS: \e[32mPASS ✓\e[0m"
        echo "└──────────────────────────────────────────────────────────────────────────────┘"
        echo ""
        return 0
    else
        echo -e "│ STATUS: \e[31mFAIL ✗\e[0m"
        echo "└──────────────────────────────────────────────────────────────────────────────┘"
        echo ""
        return 1
    fi
}

PASS=0
TOTAL=0

echo "════════════════════════════════════════════════════════════════════════════════"
echo " DEMO 1: Context-Aware Search"
echo " The SAME query returns DIFFERENT tools based on agent context"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""

((TOTAL++)); run_test 1 "send message" "" "send_message,send_sms,send_notification" "No Context (Generic)" && ((PASS++))
((TOTAL++)); run_test 2 "send message" "Slack communication bot for workplace chat" "send_slack_message,send_slack_dm,post_slack_message" "Slack Context" && ((PASS++))
((TOTAL++)); run_test 3 "send message" "Email automation assistant" "send_email,send_email_notification,send_html_email" "Email Context" && ((PASS++))
((TOTAL++)); run_test 4 "send message" "Microsoft Teams bot" "send_teams_message,send_teams_chat,send_teams_channel_message" "Teams Context" && ((PASS++))

echo "════════════════════════════════════════════════════════════════════════════════"
echo " DEMO 2: Cloud Provider Disambiguation"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""

((TOTAL++)); run_test 5 "create server" "AWS cloud infrastructure bot" "aws_create_instance,aws_start_instance" "AWS Context" && ((PASS++))
((TOTAL++)); run_test 6 "create server" "Azure cloud management assistant" "azure_create_vm,azure_start_vm" "Azure Context" && ((PASS++))
((TOTAL++)); run_test 7 "create server" "GCP compute engine operator" "gcp_create_instance,gcp_start_instance" "Google Cloud Context" && ((PASS++))

echo "════════════════════════════════════════════════════════════════════════════════"
echo " DEMO 3: Project Management Tools"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""

((TOTAL++)); run_test 8 "create task" "Jira project management" "jira_create_task,jira_create_issue,jira_create_subtask" "Jira Context" && ((PASS++))
((TOTAL++)); run_test 9 "create task" "Asana productivity assistant" "asana_create_task,asana_create_subtask" "Asana Context" && ((PASS++))

echo "════════════════════════════════════════════════════════════════════════════════"
echo " DEMO 4: Database Technology Selection"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""

((TOTAL++)); run_test 10 "query data" "PostgreSQL administrator" "query_database,run_sql_query,execute_sql" "SQL Context" && ((PASS++))
((TOTAL++)); run_test 11 "find documents" "MongoDB data engineer" "mongodb_find,mongodb_find_one,mongodb_aggregate" "MongoDB Context" && ((PASS++))

echo ""
echo "╔══════════════════════════════════════════════════════════════════════════════╗"
echo "║                              FINAL RESULTS                                   ║"
echo "╠══════════════════════════════════════════════════════════════════════════════╣"
echo "║                                                                              ║"
printf "║   Tests Passed:  %2d / %2d                                                    ║\n" $PASS $TOTAL
ACCURACY=$((PASS * 100 / TOTAL))
printf "║   Accuracy:      %3d%%                                                       ║\n" $ACCURACY
echo "║                                                                              ║"
echo "╠══════════════════════════════════════════════════════════════════════════════╣"
echo "║   KEY INSIGHT: Same query + different context = different tools returned    ║"
echo "╚══════════════════════════════════════════════════════════════════════════════╝"
echo ""
