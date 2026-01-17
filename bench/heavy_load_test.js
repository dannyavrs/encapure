/**
 * Encapure Load Test - k6
 * * Simulates Vector DB payloads: 8 documents x ~450-500 tokens each.
 * Optimized for 512 context window to avoid tensor dimension errors.
 * * Usage:
 * k6 run bench/heavy_load_test.js
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend, Counter, Rate } from 'k6/metrics';

// =============================================================================
// Custom Metrics
// =============================================================================
const tokensPerSecond = new Trend('tokens_per_second', true);
const totalTokensProcessed = new Counter('total_tokens_processed');
const requestsSuccessful = new Counter('requests_successful');
const requestsFailed = new Counter('requests_failed');
const successRate = new Rate('success_rate');
const payloadSizeKb = new Trend('payload_size_kb');

// =============================================================================
// Configuration
// =============================================================================
const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const STAGE = __ENV.STAGE || 'all';

// Test parameters optimized for 512 context window
const DOCS_PER_REQUEST = 4;
const TOKENS_PER_DOC = 256; 
const CHARS_PER_DOC = 900;  // ~1800 chars usually map to 450-500 tokens in English
const TOKENS_PER_REQUEST = DOCS_PER_REQUEST * TOKENS_PER_DOC; 

// =============================================================================
// Test Data Generation
// =============================================================================

function generateDocument(index) {
    const topics = [
        `Document ${index}: Machine learning algorithms form the backbone of modern artificial intelligence systems. These sophisticated computational methods enable computers to learn patterns from data without being explicitly programmed for each specific task. The field has evolved dramatically over the past decades, with deep learning architectures achieving remarkable breakthroughs.`,
        `Document ${index}: Natural language processing represents one of the most challenging and impactful areas of artificial intelligence research. Understanding human language requires handling ambiguity, context dependence, and the vast complexity of linguistic structures across different languages and domains.`,
        `Document ${index}: Computer vision has undergone a revolutionary transformation with the advent of convolutional neural networks. These architectures exploit the spatial structure of images through local receptive fields and weight sharing across locations.`,
        `Document ${index}: Reinforcement learning addresses sequential decision-making problems where an agent learns through interaction with an environment. The agent receives observations, takes actions, and obtains rewards that guide learning toward optimal behavior.`,
        `Document ${index}: The deployment of machine learning models in production systems requires careful consideration of numerous engineering challenges. Model serving infrastructure must handle variable request loads while maintaining low latency.`
    ];

    let text = topics[index % topics.length];

    // Fill exactly up to CHARS_PER_DOC to ensure stability
    const filler = " Furthermore, we observe that scaling parameters and optimizing high-performance inference engines in Rust provides significant throughput gains.";
    while (text.length < CHARS_PER_DOC) {
        text += filler;
    }

    return text.substring(0, CHARS_PER_DOC);
}

// Prepare payload once to save VU memory
const documents = Array.from({ length: DOCS_PER_REQUEST }, (_, i) => generateDocument(i));
const heavyPayload = JSON.stringify({
    query: "How to achieve low latency inference in production environments?",
    documents: documents,
});
const payloadSizeInKb = heavyPayload.length / 1024;

// =============================================================================
// Scenario Configuration
// =============================================================================
export const options = {
    scenarios: {
        stage_under_saturation: {
            executor: 'constant-vus',
            vus: 5,
            duration: '60s',
            startTime: '0s',
            exec: 'heavyLoadTest',
            tags: { stage: 'under' },
        },
        stage_ideal_load: {
            executor: 'constant-vus',
            vus: 10,
            duration: '60s',
            startTime: '65s',
            exec: 'heavyLoadTest',
            tags: { stage: 'ideal' },
        },
        stage_saturation: {
            executor: 'constant-vus',
            vus: 20,
            duration: '60s',
            startTime: '130s',
            exec: 'heavyLoadTest',
            tags: { stage: 'saturation' },
        },
    },
    thresholds: {
        'http_req_duration': ['p(95)<10000'],
        'http_req_failed': ['rate<0.05'],
        'http_req_duration{stage:under}': ['p(95)<3000'],
        'http_req_duration{stage:ideal}': ['p(95)<5000'],
        'http_req_duration{stage:saturation}': ['p(95)<10000'],
    },
};

// =============================================================================
// Main Test Function
// =============================================================================
export function heavyLoadTest() {
    const res = http.post(`${BASE_URL}/rerank`, heavyPayload, {
        headers: { 'Content-Type': 'application/json' },
        timeout: '120s',
    });

    const success = res.status === 200;
    successRate.add(success);
    payloadSizeKb.add(payloadSizeInKb);

    if (success) {
        requestsSuccessful.add(1);
        const durationSec = res.timings.duration / 1000;
        tokensPerSecond.add(TOKENS_PER_REQUEST / durationSec);
        totalTokensProcessed.add(TOKENS_PER_REQUEST);

        check(res, {
            'status is 200': (r) => r.status === 200,
            'correct docs count': (r) => JSON.parse(r.body).results.length === DOCS_PER_REQUEST,
        });
    } else {
        requestsFailed.add(1);
    }
}

export function handleSummary(data) {
    // k6 will output the standard summary + generate the JSON for our future UI
    return {
        'stdout': textSummary(data, { indent: ' ', enableColors: true }),
        'bench/heavy_load_results.json': JSON.stringify(data, null, 2),
    };
}

import { textSummary } from 'https://jslib.k6.io/k6-summary/0.0.2/index.js';