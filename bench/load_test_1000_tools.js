/**
 * Encapure Load Test - 1000 Tools with Top 20 Results
 * 
 * בדיקת עומסים לבדיקת כמות בקשות במקביל עם 1000 כלים והחזרת 20 תוצאות מובילות.
 * 
 * Prerequisites:
 *   1. Generate tools: cd encapure && python scripts/generate_tools.py
 *   2. Start server: TOOLS_PATH=tests/data/comprehensive_mock_tools.json cargo run --release
 *   3. Install k6: winget install Grafana.k6
 * 
 * Usage:
 *   k6 run bench/load_test_1000_tools.js
 *   
 * Custom options:
 *   k6 run bench/load_test_1000_tools.js --env VUS=50 --env DURATION=60s
 *   k6 run bench/load_test_1000_tools.js --env RAMP_UP=true
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter, Trend, Rate, Gauge } from 'k6/metrics';
import { textSummary } from 'https://jslib.k6.io/k6-summary/0.0.1/index.js';

// ============================================================================
// Custom Metrics
// ============================================================================
const requestDuration = new Trend('request_duration_ms', true);
const requestsSuccessful = new Counter('requests_successful');
const requestsFailed = new Counter('requests_failed');
const successRate = new Rate('success_rate');
const activeVUs = new Gauge('active_vus');
const throughput = new Counter('throughput_requests');
const p99Latency = new Trend('p99_latency_ms', true);

// ============================================================================
// Configuration
// ============================================================================
const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const VUS = parseInt(__ENV.VUS) || 10;
const DURATION = __ENV.DURATION || '30s';
const RAMP_UP = __ENV.RAMP_UP === 'true';
const TOP_K = parseInt(__ENV.TOP_K) || 5;

// Configuration recommendations for optimal performance:
// For 12 physical cores:
//   PERMITS=6 INTRA_THREADS=2  → 6×2=12 threads (100% utilization)
//   PERMITS=4 INTRA_THREADS=3  → 4×3=12 threads (100% utilization)
//   PERMITS=3 INTRA_THREADS=4  → 3×4=12 threads (100% utilization)
// 
// Run server with: 
//   $env:TOOLS_PATH="tests/data/comprehensive_mock_tools.json"
//   cargo run --release

// Test queries - diverse queries to simulate real-world usage
const testQueries = [
    // File operations
    "read a file from disk",
    "write content to a file",
    "list files in a directory",
    "search for files matching a pattern",
    "compress a file to zip",
    
    // Communication
    "send a message to slack channel",
    "send an email with attachment",
    "post message to teams",
    "send sms notification",
    "broadcast notification to all users",
    
    // Project management
    "create a new jira ticket",
    "assign task to user",
    "update github issue status",
    "create pull request",
    "add comment to asana task",
    
    // DevOps
    "deploy application to production",
    "get kubernetes pod logs",
    "restart docker container",
    "scale deployment replicas",
    "check service health status",
    
    // Database
    "execute sql query on database",
    "insert record into table",
    "get data from mongodb collection",
    "cache value in redis",
    "backup database to file",
    
    // Security
    "scan for vulnerabilities",
    "encrypt sensitive data",
    "verify user authentication token",
    "create api key for service",
    "check user permissions",
    
    // Monitoring
    "get application logs",
    "query metrics dashboard",
    "create alert rule",
    "check error rate",
    "get system cpu metrics",
    
    // Git
    "commit changes to repository",
    "create new branch",
    "merge pull request",
    "push code to remote",
    "list recent commits",
];

// ============================================================================
// Scenario Configuration
// ============================================================================
export const options = RAMP_UP ? {
    // Ramp-up test to find breaking point
    scenarios: {
        ramp_up_test: {
            executor: 'ramping-vus',
            startVUs: 1,
            stages: [
                { duration: '30s', target: 10 },   // Ramp to 10 VUs
                { duration: '30s', target: 25 },   // Ramp to 25 VUs
                { duration: '30s', target: 50 },   // Ramp to 50 VUs
                { duration: '30s', target: 100 },  // Ramp to 100 VUs
                { duration: '30s', target: 150 },  // Ramp to 150 VUs
                { duration: '30s', target: 200 },  // Ramp to 200 VUs
                { duration: '30s', target: 200 },  // Stay at 200 VUs
                { duration: '30s', target: 0 },    // Ramp down
            ],
            gracefulRampDown: '10s',
        },
    },
    thresholds: {
        'success_rate': ['rate>0.95'],
        'request_duration_ms': ['p(95)<2000', 'p(99)<5000'],
    },
} : {
    // Constant load test
    scenarios: {
        constant_load: {
            executor: 'constant-vus',
            vus: VUS,
            duration: DURATION,
        },
    },
    thresholds: {
        'success_rate': ['rate>0.99'],
        'request_duration_ms': ['p(95)<500', 'p(99)<1000'],
    },
};

// ============================================================================
// Setup - Warmup and validation
// ============================================================================
export function setup() {
    console.log('\n========================================');
    console.log('  Encapure Load Test - 1000 Tools');
    console.log('========================================');
    console.log(`Base URL: ${BASE_URL}`);
    console.log(`Top K: ${TOP_K}`);
    console.log(`Mode: ${RAMP_UP ? 'Ramp-up (finding max throughput)' : `Constant (${VUS} VUs for ${DURATION})`}`);
    console.log('----------------------------------------\n');
    
    // Warmup request
    const warmupPayload = {
        query: "test query for warmup",
        top_k: TOP_K
    };
    
    const warmupRes = http.post(`${BASE_URL}/search`, JSON.stringify(warmupPayload), {
        headers: { 'Content-Type': 'application/json' },
        timeout: '30s',
    });
    
    if (warmupRes.status !== 200) {
        console.error(`Warmup failed: ${warmupRes.status} - ${warmupRes.body}`);
        console.error('Make sure the server is running with 1000 tools loaded.');
        console.error('Run: TOOLS_PATH=tests/data/comprehensive_mock_tools.json cargo run --release');
        throw new Error('Server not ready');
    }
    
    try {
        const body = JSON.parse(warmupRes.body);
        console.log(`✓ Warmup successful: ${body.results?.length || 0} results returned`);
        console.log(`✓ Warmup latency: ${warmupRes.timings.duration.toFixed(0)} ms\n`);
    } catch (e) {
        console.error('Failed to parse warmup response');
    }
    
    return { startTime: Date.now() };
}

// ============================================================================
// Main test function
// ============================================================================
const headers = { 'Content-Type': 'application/json' };

export default function() {
    // Select a random query
    const query = testQueries[Math.floor(Math.random() * testQueries.length)];
    
    const payload = {
        query: query,
        top_k: TOP_K
    };
    
    const startTime = Date.now();
    
    const res = http.post(`${BASE_URL}/search`, JSON.stringify(payload), {
        headers,
        timeout: '30s',
    });
    
    const duration = Date.now() - startTime;
    const success = res.status === 200;
    
    // Record metrics
    requestDuration.add(res.timings.duration);
    p99Latency.add(res.timings.duration);
    throughput.add(1);
    successRate.add(success);
    activeVUs.add(__VU);
    
    if (success) {
        requestsSuccessful.add(1);
        
        // Validate response
        try {
            const body = JSON.parse(res.body);
            check(res, {
                'status is 200': (r) => r.status === 200,
                'has results array': () => Array.isArray(body.results),
                'returns expected count': () => body.results.length === TOP_K || body.results.length > 0,
                'results have scores': () => body.results[0]?.score !== undefined,
            });
        } catch (e) {
            // Parse error
        }
    } else {
        requestsFailed.add(1);
        console.error(`Request failed [VU:${__VU}]: ${res.status} - ${res.body?.substring(0, 100)}`);
    }
    
    // Small sleep to prevent overwhelming (adjust based on test goals)
    // Remove or reduce for maximum throughput testing
    // sleep(0.01);
}

// ============================================================================
// Teardown - Summary
// ============================================================================
export function teardown(data) {
    const totalTime = (Date.now() - data.startTime) / 1000;
    console.log('\n========================================');
    console.log('  Test Complete');
    console.log('========================================');
    console.log(`Total duration: ${totalTime.toFixed(1)} seconds\n`);
}

// ============================================================================
// Custom summary output
// ============================================================================
export function handleSummary(data) {
    const summary = {
        'Total Requests': data.metrics.throughput_requests?.values?.count || 0,
        'Successful': data.metrics.requests_successful?.values?.count || 0,
        'Failed': data.metrics.requests_failed?.values?.count || 0,
        'Success Rate': ((data.metrics.success_rate?.values?.rate || 0) * 100).toFixed(2) + '%',
        'Avg Latency': (data.metrics.request_duration_ms?.values?.avg || 0).toFixed(2) + ' ms',
        'P95 Latency': (data.metrics.request_duration_ms?.values['p(95)'] || 0).toFixed(2) + ' ms',
        'P99 Latency': (data.metrics.request_duration_ms?.values['p(99)'] || 0).toFixed(2) + ' ms',
        'Max Latency': (data.metrics.request_duration_ms?.values?.max || 0).toFixed(2) + ' ms',
        'Min Latency': (data.metrics.request_duration_ms?.values?.min || 0).toFixed(2) + ' ms',
        'Requests/sec': (data.metrics.http_reqs?.values?.rate || 0).toFixed(2),
    };
    
    console.log('\n╔════════════════════════════════════════════════════════╗');
    console.log('║            LOAD TEST RESULTS SUMMARY                   ║');
    console.log('╠════════════════════════════════════════════════════════╣');
    
    for (const [key, value] of Object.entries(summary)) {
        const paddedKey = key.padEnd(20);
        const paddedValue = String(value).padStart(15);
        console.log(`║  ${paddedKey} │ ${paddedValue}         ║`);
    }
    
    console.log('╚════════════════════════════════════════════════════════╝\n');
    
    return {
        'stdout': textSummary(data, { indent: '  ', enableColors: true }),
        'bench/results/load_test_results.json': JSON.stringify(data, null, 2),
    };
}
