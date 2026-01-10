/**
 * Encapure v1.1 Scenario Load Test - k6 Edition
 * 
 * This script replicates the scenarios from scenario_test.ps1 using k6,
 * which provides true parallel execution without PowerShell Job overhead.
 * 
 * Installation:
 *   - Windows: winget install Grafana.k6
 *   - Or download from: https://k6.io/docs/get-started/installation/
 * 
 * Usage:
 *   k6 run bench/scenario_test.js
 *   k6 run bench/scenario_test.js --env BASE_URL=http://localhost:8080
 * 
 * Run specific scenario:
 *   k6 run bench/scenario_test.js --env SCENARIO=long
 *   k6 run bench/scenario_test.js --env SCENARIO=small
 *   k6 run bench/scenario_test.js --env SCENARIO=concurrent
 */

import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Counter, Trend, Rate } from 'k6/metrics';

// Custom metrics
const requestDuration = new Trend('request_duration_ms');
const requestsSuccessful = new Counter('requests_successful');
const requestsFailed = new Counter('requests_failed');
const successRate = new Rate('success_rate');

// Configuration
const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const SCENARIO = __ENV.SCENARIO || 'all';

// Test data - matches scenario_test.ps1 exactly
const longRequest = {
    query: "What are the key principles and best practices of machine learning model training and optimization?",
    documents: [
        "Machine learning is a subset of artificial intelligence that enables systems to learn and improve from experience without being explicitly programmed. It focuses on developing algorithms that can access data and use it to learn for themselves.",
        "Deep learning is a type of machine learning based on artificial neural networks with multiple layers. These deep neural networks attempt to simulate the behavior of the human brain in processing data and creating patterns for decision making.",
        "The weather forecast for tomorrow indicates partly cloudy skies with a high temperature of 72 degrees Fahrenheit and a low of 58 degrees. There is a 20% chance of precipitation in the afternoon hours.",
        "Natural language processing is a branch of AI that helps computers understand, interpret and manipulate human language. NLP draws from many disciplines including computer science and computational linguistics.",
        "Supervised learning is the machine learning task of learning a function that maps an input to an output based on example input-output pairs. It infers a function from labeled training data consisting of a set of training examples.",
        "My favorite recipe for chocolate chip cookies requires two cups of flour, one cup of butter, and a generous amount of chocolate chips. Bake at 350 degrees for exactly twelve minutes.",
        "Reinforcement learning is an area of machine learning concerned with how intelligent agents ought to take actions in an environment in order to maximize the notion of cumulative reward through trial and error.",
        "The stock market experienced significant volatility today with the S&P 500 index fluctuating between gains and losses throughout the trading session before closing marginally higher.",
        "Transfer learning is a research problem in machine learning that focuses on storing knowledge gained while solving one problem and applying it to a different but related problem for improved performance.",
        "Gradient descent is an optimization algorithm used to minimize some function by iteratively moving in the direction of steepest descent as defined by the negative of the gradient.",
        "The hiking trail through the national park offers stunning views of the mountain range and is approximately five miles long with an elevation gain of two thousand feet.",
        "Overfitting occurs when a statistical model describes random error or noise instead of the underlying relationship. Overfitting generally occurs when a model is excessively complex.",
        "Cross-validation is a resampling procedure used to evaluate machine learning models on a limited data sample. It has a single parameter called k that refers to the number of groups.",
        "The new smartphone model features an improved camera system with enhanced low-light performance and optical image stabilization for capturing better photos and videos.",
        "Batch normalization is a technique for improving the performance and stability of artificial neural networks by normalizing the inputs to each layer within a mini-batch.",
        "Feature engineering is the process of using domain knowledge to extract features from raw data via data mining techniques to improve machine learning model performance."
    ]
};

const smallRequest1 = {
    query: "What is machine learning?",
    documents: [
        "Machine learning is a type of artificial intelligence",
        "The weather is sunny today",
        "Neural networks learn from data"
    ]
};

const smallRequest2 = {
    query: "How does deep learning work?",
    documents: [
        "Deep learning uses multiple neural network layers",
        "I enjoy reading books",
        "Backpropagation trains neural networks"
    ]
};

// Scenario configurations
export const options = {
    scenarios: {
        // Scenario 1: Single long request (sequential, 5 iterations after warmup)
        long_request: {
            executor: 'shared-iterations',
            vus: 1,
            iterations: 6, // 1 warmup + 5 test runs
            maxDuration: '5m',
            exec: 'scenarioLong',
            startTime: '0s',
            tags: { scenario: 'long' },
        },
        // Scenario 2: Two small requests (sequential)
        small_requests: {
            executor: 'shared-iterations',
            vus: 1,
            iterations: 5,
            maxDuration: '2m',
            exec: 'scenarioSmall',
            startTime: '0s',
            tags: { scenario: 'small' },
        },
        // Scenario 3: Concurrent small requests (10 parallel VUs)
        concurrent_requests: {
            executor: 'shared-iterations',
            vus: 10,
            iterations: 10,
            maxDuration: '1m',
            exec: 'scenarioConcurrent',
            startTime: '0s',
            tags: { scenario: 'concurrent' },
        },
    },
    thresholds: {
        // Performance targets
        'request_duration_ms{scenario:small}': ['avg<200'],
        'request_duration_ms{scenario:concurrent}': ['avg<500'],
        'success_rate': ['rate>0.95'],
    },
};

// Override options if specific scenario requested
if (SCENARIO !== 'all') {
    const selectedScenario = options.scenarios[`${SCENARIO}_request`] || 
                             options.scenarios[`${SCENARIO}_requests`] ||
                             options.scenarios[`concurrent_requests`];
    if (selectedScenario) {
        options.scenarios = { [SCENARIO]: selectedScenario };
    }
}

const headers = { 'Content-Type': 'application/json' };

// Helper function to make request and record metrics
function makeRequest(payload, scenarioTag) {
    const url = `${BASE_URL}/rerank`;
    const body = JSON.stringify(payload);
    
    const res = http.post(url, body, { 
        headers,
        timeout: '60s',
        tags: { scenario: scenarioTag }
    });
    
    const success = res.status === 200;
    
    requestDuration.add(res.timings.duration, { scenario: scenarioTag });
    successRate.add(success);
    
    if (success) {
        requestsSuccessful.add(1);
    } else {
        requestsFailed.add(1);
        console.error(`Request failed: ${res.status} - ${res.body}`);
    }
    
    check(res, {
        'status is 200': (r) => r.status === 200,
        'has results': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.results && body.results.length > 0;
            } catch {
                return false;
            }
        },
    });
    
    return res;
}

// Scenario 1: Single LONG request (16 documents)
export function scenarioLong() {
    const iteration = __ITER;
    
    if (iteration === 0) {
        console.log('\n[Scenario 1] Single LONG Request (16 documents)');
        console.log('Warm-up run...');
    }
    
    const res = makeRequest(longRequest, 'long');
    
    if (iteration === 0) {
        console.log(`  Warm-up: ${res.timings.duration.toFixed(0)} ms`);
    } else {
        console.log(`  Run ${iteration}: ${res.timings.duration.toFixed(0)} ms`);
    }
}

// Scenario 2: Two SMALL requests (3 documents each)
export function scenarioSmall() {
    const iteration = __ITER + 1;
    
    if (iteration === 1) {
        console.log('\n[Scenario 2] Two SMALL Requests (3 documents each)');
    }
    
    const res1 = makeRequest(smallRequest1, 'small');
    const res2 = makeRequest(smallRequest2, 'small');
    
    console.log(`  Run ${iteration}: Request1=${res1.timings.duration.toFixed(0)} ms, Request2=${res2.timings.duration.toFixed(0)} ms`);
}

// Scenario 3: Concurrent small requests
export function scenarioConcurrent() {
    const vuId = __VU;
    
    if (__ITER === 0 && vuId === 1) {
        console.log('\n[Scenario 3] Concurrent Small Requests (10 parallel)');
    }
    
    // Alternate between small requests
    const payload = vuId % 2 === 0 ? smallRequest1 : smallRequest2;
    const res = makeRequest(payload, 'concurrent');
    
    console.log(`  VU ${vuId}: ${res.timings.duration.toFixed(0)} ms`);
}

// Summary handler - prints results similar to PowerShell script
export function handleSummary(data) {
    const metrics = data.metrics;
    
    // Extract scenario-specific metrics
    const longDuration = metrics['request_duration_ms{scenario:long}'];
    const smallDuration = metrics['request_duration_ms{scenario:small}'];
    const concurrentDuration = metrics['request_duration_ms{scenario:concurrent}'];
    
    let summary = `
========================================
  SUMMARY (k6)
========================================

`;

    if (longDuration) {
        summary += `  Scenario 1 (16 docs):     Avg ${longDuration.values.avg.toFixed(1)} ms, Min ${longDuration.values.min.toFixed(0)} ms, Max ${longDuration.values.max.toFixed(0)} ms\n`;
    }
    
    if (smallDuration) {
        summary += `  Scenario 2 (3 docs):      Avg ${smallDuration.values.avg.toFixed(1)} ms\n`;
    }
    
    if (concurrentDuration) {
        const totalRequests = concurrentDuration.values.count;
        const totalTime = concurrentDuration.values.max / 1000; // Approximate total time in seconds
        const throughput = (totalRequests / totalTime).toFixed(2);
        summary += `  Scenario 3 (concurrent):  ${throughput} req/s, Avg ${concurrentDuration.values.avg.toFixed(1)} ms, Max ${concurrentDuration.values.max.toFixed(0)} ms\n`;
    }

    summary += `
  Overall:
    Total Requests: ${metrics.http_reqs.values.count}
    Success Rate:   ${(metrics.success_rate.values.rate * 100).toFixed(1)}%
    
  Performance Assessment:
`;

    // Assessment
    if (longDuration) {
        if (longDuration.values.avg < 500) {
            summary += `  [GOOD] Long requests under 500ms\n`;
        } else {
            summary += `  [SLOW] Long requests over 500ms (got ${longDuration.values.avg.toFixed(1)} ms)\n`;
        }
    }
    
    if (smallDuration) {
        if (smallDuration.values.avg < 200) {
            summary += `  [GOOD] Small requests under 200ms\n`;
        } else {
            summary += `  [SLOW] Small requests over 200ms (got ${smallDuration.values.avg.toFixed(1)} ms)\n`;
        }
    }
    
    if (concurrentDuration) {
        const throughput = concurrentDuration.values.count / (concurrentDuration.values.max / 1000);
        if (throughput >= 20) {
            summary += `  [GOOD] Concurrent throughput >= 20 req/s\n`;
        } else {
            summary += `  [NEEDS WORK] Concurrent throughput < 20 req/s (got ${throughput.toFixed(2)} req/s)\n`;
        }
    }

    console.log(summary);
    
    return {
        'stdout': summary,
        'bench/k6_results.json': JSON.stringify(data, null, 2),
    };
}
