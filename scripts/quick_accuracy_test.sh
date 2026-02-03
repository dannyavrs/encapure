#!/bin/bash
# Quick accuracy test for Encapure search
# Usage: ./quick_accuracy_test.sh

ENDPOINT="http://127.0.0.1:8080/search"
PASSED=0
FAILED=0

test_search() {
    local id="$1"
    local query="$2"
    local agent_desc="$3"
    local expected="$4"
    local desc="$5"

    if [ -z "$agent_desc" ]; then
        local body="{\"query\": \"$query\", \"top_k\": 3}"
    else
        local body="{\"query\": \"$query\", \"top_k\": 3, \"agent_description\": \"$agent_desc\"}"
    fi

    local response=$(curl -s -X POST "$ENDPOINT" -H "Content-Type: application/json" -d "$body")
    local tools=$(echo "$response" | grep -o '"name":"[^"]*"' | head -3 | sed 's/"name":"//g' | sed 's/"//g' | tr '\n' ',' | sed 's/,$//')

    # Check if any expected tool is in results
    local found=0
    for exp in $(echo "$expected" | tr ',' ' '); do
        if echo "$tools" | grep -q "$exp"; then
            found=1
            break
        fi
    done

    if [ $found -eq 1 ]; then
        echo -e "\e[32m[PASS]\e[0m Test $id: $desc"
        echo "       Query: '$query' | Context: ${agent_desc:-none}"
        echo "       Got: $tools"
        ((PASSED++))
    else
        echo -e "\e[31m[FAIL]\e[0m Test $id: $desc"
        echo "       Query: '$query' | Context: ${agent_desc:-none}"
        echo "       Expected one of: $expected"
        echo "       Got: $tools"
        ((FAILED++))
    fi
    echo ""
}

echo "========================================"
echo "Encapure Quick Accuracy Test"
echo "========================================"
echo ""

# Context switching tests - using actual tool names from comprehensive_mock_tools.json
test_search 1 "send message" "" "send_sms,send_message,send_notification" "Generic message (no context)"
test_search 2 "send message" "Slack bot for team communication" "send_slack_message,send_slack_dm,post_slack_message" "Slack context"
test_search 3 "send message" "Email automation assistant" "send_email,send_email_notification,send_html_email" "Email context"
test_search 4 "send message" "Microsoft Teams bot" "send_teams_message,send_teams_chat,send_teams_channel_message" "Teams context"

# DevOps tests
test_search 5 "create server" "AWS infrastructure bot" "aws_create_instance,aws_start_instance" "AWS context"
test_search 6 "deploy application" "Kubernetes operator" "k8s_apply_manifest,deploy_container,k8s_rollout_restart" "K8s context"

# Project management tests
test_search 7 "create task" "Jira project management" "jira_create_task,jira_create_issue,jira_create_subtask" "Jira context"
test_search 8 "create task" "Asana productivity assistant" "asana_create_task,asana_create_subtask" "Asana context"

# Database tests
test_search 9 "query data" "PostgreSQL administrator" "query_database,run_sql_query,execute_sql" "SQL context"
test_search 10 "find documents" "MongoDB data engineer" "mongodb_find,mongodb_find_one,mongodb_aggregate" "MongoDB context"

echo "========================================"
echo "Results: $PASSED passed, $FAILED failed"
echo "========================================"
