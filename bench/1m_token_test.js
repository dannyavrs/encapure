/**
 * 1M Token Benchmark Test
 *
 * Tests reranking 1,953 documents x 512 tokens = ~1M tokens
 * Uses server-side batching (32 docs per batch = 62 batches)
 *
 * Expected performance:
 * - ~500ms per batch (based on previous benchmarks)
 * - 62 batches total
 * - ~31 seconds total processing time
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter, Trend } from 'k6/metrics';

// Configuration
const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const DOCS_COUNT = 1953;           // 1M tokens / 512 tokens per doc
const TOKENS_PER_DOC = 512;
const CHARS_PER_DOC = 2000;        // ~512 tokens (approx 4 chars per token)

// Custom metrics
const totalTokens = new Counter('total_tokens_processed');
const requestDuration = new Trend('giant_request_duration_ms');

// Word pool for generating realistic documents
const WORD_POOL = [
    'machine', 'learning', 'neural', 'network', 'deep', 'algorithm',
    'training', 'inference', 'model', 'data', 'feature', 'vector',
    'embedding', 'transformer', 'attention', 'encoder', 'decoder',
    'classification', 'regression', 'optimization', 'gradient', 'descent',
    'backpropagation', 'activation', 'layer', 'neuron', 'weight', 'bias',
    'loss', 'accuracy', 'precision', 'recall', 'f1', 'score',
    'batch', 'epoch', 'learning', 'rate', 'momentum', 'regularization',
    'dropout', 'normalization', 'convolution', 'pooling', 'recurrent',
    'sequence', 'tokenization', 'vocabulary', 'prediction', 'probability'
];

/**
 * Generate a single document with approximately the target token count.
 * @param {number} index - Document index for variation
 * @returns {string} Generated document text
 */
function generateDocument(index) {
    const words = [];
    let charCount = 0;

    // Add document identifier for debugging
    words.push(`[Doc${index}]`);
    charCount += 8;

    // Fill with words until we reach target character count
    while (charCount < CHARS_PER_DOC) {
        const word = WORD_POOL[(index + words.length) % WORD_POOL.length];
        words.push(word);
        charCount += word.length + 1; // +1 for space
    }

    return words.join(' ');
}

/**
 * Generate all documents for the 1M token test.
 * This is called once per iteration.
 * @returns {string[]} Array of 1953 documents
 */
function generateDocuments() {
    const docs = [];
    for (let i = 0; i < DOCS_COUNT; i++) {
        docs.push(generateDocument(i));
    }
    return docs;
}

// Test configuration
// Single VU, single iteration - this is a stress test, not a load test
export const options = {
    scenarios: {
        single_giant_request: {
            executor: 'shared-iterations',
            vus: 1,
            iterations: 1,
            maxDuration: '10m',  // 10 minute timeout
        },
    },
    thresholds: {
        'http_req_duration': ['p(95)<300000'],  // P95 < 5 minutes
        'http_req_failed': ['rate==0'],          // No failures allowed
        'giant_request_duration_ms': ['p(95)<300000'],
    },
};

export default function() {
    console.log(`Generating ${DOCS_COUNT} documents (~${DOCS_COUNT * TOKENS_PER_DOC} tokens)...`);

    const documents = generateDocuments();

    console.log(`Documents generated. Payload size: ~${Math.round(JSON.stringify(documents).length / 1024)} KB`);

    const payload = {
        query: "What are the key concepts in machine learning and deep neural networks?",
        documents: documents,
    };

    const payloadStr = JSON.stringify(payload);
    console.log(`Total payload size: ${Math.round(payloadStr.length / 1024)} KB`);
    console.log(`Sending request to ${BASE_URL}/rerank...`);

    const startTime = Date.now();

    const res = http.post(`${BASE_URL}/rerank`, payloadStr, {
        headers: { 'Content-Type': 'application/json' },
        timeout: '600s',  // 10 minute client timeout
    });

    const duration = Date.now() - startTime;
    requestDuration.add(duration);

    console.log(`Response received in ${duration}ms (${(duration/1000).toFixed(2)}s)`);
    console.log(`Status: ${res.status}`);

    if (res.status !== 200) {
        console.error(`Error response: ${res.body}`);
    }

    const checks = check(res, {
        'status is 200': (r) => r.status === 200,
        'has results array': (r) => {
            try {
                const body = JSON.parse(r.body);
                return Array.isArray(body.results);
            } catch {
                return false;
            }
        },
        'has correct number of results': (r) => {
            try {
                const body = JSON.parse(r.body);
                const hasCorrect = body.results.length === DOCS_COUNT;
                if (!hasCorrect) {
                    console.error(`Expected ${DOCS_COUNT} results, got ${body.results.length}`);
                }
                return hasCorrect;
            } catch {
                return false;
            }
        },
        'all scores are valid': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.results.every(result =>
                    typeof result.score === 'number' &&
                    result.score >= 0 &&
                    result.score <= 1
                );
            } catch {
                return false;
            }
        },
        'results are sorted by score descending': (r) => {
            try {
                const body = JSON.parse(r.body);
                for (let i = 1; i < body.results.length; i++) {
                    if (body.results[i].score > body.results[i-1].score) {
                        console.error(`Sort error at index ${i}: ${body.results[i].score} > ${body.results[i-1].score}`);
                        return false;
                    }
                }
                return true;
            } catch {
                return false;
            }
        },
    });

    if (checks) {
        totalTokens.add(DOCS_COUNT * TOKENS_PER_DOC);
        const tokensPerSecond = (DOCS_COUNT * TOKENS_PER_DOC) / (duration / 1000);
        console.log(`\n=== 1M TOKEN BENCHMARK RESULTS ===`);
        console.log(`Total documents: ${DOCS_COUNT}`);
        console.log(`Tokens per document: ${TOKENS_PER_DOC}`);
        console.log(`Total tokens: ${DOCS_COUNT * TOKENS_PER_DOC}`);
        console.log(`Processing time: ${(duration/1000).toFixed(2)}s`);
        console.log(`Throughput: ${Math.round(tokensPerSecond)} tokens/second`);
        console.log(`=================================\n`);
    }
}
