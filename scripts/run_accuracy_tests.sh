#!/bin/bash
# Encapure Full Accuracy Test Runner
# Runs all 50 test cases from accuracy_test_cases.json

ENDPOINT="http://127.0.0.1:8080/search"
TOP_K=3

PASSED=0
PARTIAL=0
FAILED=0
TOTAL=0

echo "========================================================================"
echo "Encapure Full Accuracy Test Suite"
echo "========================================================================"
echo ""

run_test() {
    local id="$1"
    local query="$2"
    local agent_desc="$3"
    local expected="$4"
    local desc="$5"
    local category="$6"

    ((TOTAL++))

    if [ -z "$agent_desc" ] || [ "$agent_desc" == "null" ]; then
        local body="{\"query\": \"$query\", \"top_k\": $TOP_K}"
    else
        local body="{\"query\": \"$query\", \"top_k\": $TOP_K, \"agent_description\": \"$agent_desc\"}"
    fi

    local response=$(curl -s -X POST "$ENDPOINT" -H "Content-Type: application/json" -d "$body" 2>/dev/null)
    local tools=$(echo "$response" | grep -o '"name":"[^"]*"' | head -$TOP_K | sed 's/"name":"//g' | sed 's/"//g' | tr '\n' ',' | sed 's/,$//')

    # Check if any expected tool is in results
    local match_count=0
    local expected_count=0
    for exp in $(echo "$expected" | tr ',' ' '); do
        ((expected_count++))
        if echo ",$tools," | grep -qi ",$exp,"; then
            ((match_count++))
        fi
    done

    if [ $match_count -ge 1 ]; then
        echo -e "\e[32m[PASS]\e[0m Test $id: $desc"
        echo "       Matches: $match_count/$expected_count | Got: $tools"
        ((PASSED++))
    else
        echo -e "\e[31m[FAIL]\e[0m Test $id: $desc"
        echo "       Matches: $match_count/$expected_count | Expected: $expected"
        echo "       Got: $tools"
        ((FAILED++))
    fi
}

# Context switching tests (1-8)
echo "--- Context Switching Tests ---"
run_test 1 "send message" "" "send_message,send_sms,send_notification" "Generic message without context" "context_switching"
run_test 2 "send message" "Slack communication bot for workplace chat" "send_slack_message,send_slack_dm,post_slack_message" "Message with Slack context" "context_switching"
run_test 3 "send message" "Email automation assistant for newsletters" "send_email,send_email_notification,send_html_email" "Message with Email context" "context_switching"
run_test 4 "send message" "Microsoft Teams bot for enterprise collaboration" "send_teams_message,send_teams_chat,send_teams_channel_message" "Message with Teams context" "context_switching"
run_test 5 "send message" "Discord community manager bot" "send_discord_message,send_discord_dm,send_discord_embed" "Message with Discord context" "context_switching"
run_test 6 "create file" "" "create_file,write_file,write_text_file" "Generic file creation" "context_switching"
run_test 7 "upload file" "AWS S3 storage manager" "aws_s3_upload_file,aws_s3_copy_object" "File upload in S3 context" "cloud_storage"
run_test 8 "upload file" "Google Cloud Storage operator" "gcp_upload_to_gcs,gcp_download_from_gcs" "File upload in GCS context" "cloud_storage"
echo ""

# DevOps tests (9-18)
echo "--- DevOps Context Tests ---"
run_test 9 "create server" "AWS cloud infrastructure bot" "aws_create_instance,aws_start_instance" "AWS server creation" "devops_context"
run_test 10 "create server" "Azure cloud management assistant" "azure_create_vm,azure_start_vm" "Azure VM creation" "devops_context"
run_test 11 "create server" "GCP compute engine operator" "gcp_create_instance,gcp_start_instance" "GCP instance creation" "devops_context"
run_test 12 "deploy application" "Kubernetes cluster operator" "k8s_apply_manifest,deploy_container,k8s_rollout_restart" "K8s deployment" "devops_context"
run_test 13 "deploy application" "Docker container management" "docker_start_container,docker_compose_up,deploy_container" "Docker deployment" "devops_context"
run_test 14 "get instance status" "AWS infrastructure monitoring" "aws_describe_instance,aws_get_instance,aws_get_instance_status" "AWS status check" "devops_context"
run_test 15 "get pods" "Kubernetes monitoring agent" "k8s_get_pods,k8s_list_pods,k8s_describe_pod" "K8s pod status" "devops_context"
run_test 16 "scale deployment" "Kubernetes autoscaling manager" "k8s_scale_deployment,k8s_rollout_restart" "K8s scaling" "devops_context"
run_test 17 "build image" "Docker CI/CD pipeline" "docker_build_image,docker_push_image,docker_tag_image" "Docker image build" "devops_context"
run_test 18 "invoke function" "AWS Lambda serverless manager" "aws_lambda_invoke,aws_lambda_get_function" "Lambda invocation" "devops_context"
echo ""

# Project management tests (19-26)
echo "--- Project Management Tests ---"
run_test 19 "create task" "Jira project management assistant" "jira_create_task,jira_create_issue,jira_create_subtask" "Jira task creation" "project_management"
run_test 20 "create task" "Asana productivity assistant" "asana_create_task,asana_create_subtask,asana_create_project" "Asana task creation" "project_management"
run_test 21 "create card" "Trello board management bot" "trello_create_card,trello_create_list,trello_create_board" "Trello card creation" "project_management"
run_test 22 "create issue" "Linear issue tracking assistant" "linear_create_issue,linear_create_bug,linear_create_feature" "Linear issue creation" "project_management"
run_test 23 "assign issue" "Jira workflow automation" "jira_assign_issue,jira_update_issue,jira_transition_issue" "Jira assignment" "project_management"
run_test 24 "complete task" "Asana task manager" "asana_complete_task,asana_update_task" "Asana task completion" "project_management"
run_test 25 "add comment" "GitHub code review assistant" "github_add_comment,github_add_pr_comment" "GitHub commenting" "project_management"
run_test 26 "create pull request" "GitHub workflow automation" "github_create_pr,github_open_pull_request" "GitHub PR creation" "project_management"
echo ""

# Database tests (27-34)
echo "--- Database Context Tests ---"
run_test 27 "query data" "PostgreSQL database administrator" "query_database,run_sql_query,execute_sql" "SQL query" "database_context"
run_test 28 "find documents" "MongoDB data engineer" "mongodb_find,mongodb_find_one,mongodb_aggregate" "MongoDB query" "database_context"
run_test 29 "get value" "Redis cache administrator" "redis_get,redis_hget,redis_hgetall" "Redis get" "database_context"
run_test 30 "insert record" "PostgreSQL data manager" "insert_record,insert_into_table,execute_sql" "SQL insert" "database_context"
run_test 31 "insert document" "MongoDB collection manager" "mongodb_insert,mongodb_insert_many" "MongoDB insert" "database_context"
run_test 32 "update record" "SQL database maintenance bot" "update_record,update_table,execute_sql" "SQL update" "database_context"
run_test 33 "update document" "MongoDB data engineer" "mongodb_update,mongodb_update_one" "MongoDB update" "database_context"
run_test 34 "delete record" "Database cleanup automation" "delete_record,delete_from_table,execute_sql" "SQL delete" "database_context"
echo ""

# Cloud storage tests (35-38)
echo "--- Cloud Storage Tests ---"
run_test 35 "list objects" "AWS S3 bucket manager" "aws_s3_list_objects,aws_s3_list_buckets" "S3 listing" "cloud_storage"
run_test 36 "download file" "AWS S3 storage assistant" "aws_s3_download_file,aws_s3_get_presigned_url" "S3 download" "cloud_storage"
run_test 37 "upload blob" "Azure Blob storage assistant" "azure_blob_upload,azure_blob_download" "Azure blob upload" "cloud_storage"
run_test 38 "list buckets" "GCP Cloud Storage operator" "gcp_list_buckets,gcp_upload_to_gcs" "GCS bucket listing" "cloud_storage"
echo ""

# Monitoring tests (39-43)
echo "--- Monitoring and Alerting Tests ---"
run_test 39 "create alert" "Infrastructure monitoring assistant" "create_alert,enable_alert,update_alert" "Alert creation" "monitoring"
run_test 40 "get logs" "Application logging assistant" "get_logs,get_application_logs,get_error_logs" "Log retrieval" "monitoring"
run_test 41 "get metrics" "Performance monitoring agent" "get_metrics,query_metrics,get_cpu_metrics" "Metrics query" "monitoring"
run_test 42 "search logs" "Log analysis assistant" "search_logs,get_logs,filter_logs" "Log search" "monitoring"
run_test 43 "create dashboard" "Visualization manager" "create_dashboard,update_dashboard,add_dashboard_panel" "Dashboard creation" "monitoring"
echo ""

# Notification tests (44-46)
echo "--- Notification Tests ---"
run_test 44 "send notification" "Mobile push notification service" "send_push_notification,send_notification,send_in_app_notification" "Push notification" "notifications"
run_test 45 "send notification" "Desktop alert system" "send_desktop_notification,send_browser_notification" "Desktop notification" "notifications"
run_test 46 "send alert" "SMS alerting service" "send_sms_alert,send_sms,send_sms_notification" "SMS alert" "notifications"
echo ""

# Git operations tests (47-49)
echo "--- Git Operations Tests ---"
run_test 47 "commit changes" "Git workflow assistant" "git_commit,git_add,git_push" "Git commit" "git_operations"
run_test 48 "create branch" "Git repository manager" "git_branch,git_create_branch,git_checkout" "Git branch creation" "git_operations"
run_test 49 "merge code" "Git integration bot" "git_merge,git_rebase,github_merge_pr" "Git merge" "git_operations"
echo ""

# Edge case test (50)
echo "--- Edge Case Tests ---"
run_test 50 "send" "Slack bot" "send_slack_message,send_slack_dm,slack_send_file" "Minimal query with context" "edge_cases"
echo ""

echo "========================================================================"
echo "RESULTS SUMMARY"
echo "========================================================================"
echo -e "\e[32mPassed:  $PASSED\e[0m"
echo -e "\e[31mFailed:  $FAILED\e[0m"
echo ""
ACCURACY=$(awk "BEGIN {printf \"%.1f\", ($PASSED / $TOTAL) * 100}")
echo "Accuracy: $ACCURACY% ($PASSED/$TOTAL)"
echo "========================================================================"
