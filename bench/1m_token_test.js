/**
 * 1M token streaming benchmark
 *
 * Strategy: many tiny requests (4 docs each) to keep latency low and CPUs busy.
 * 977 requests * 4 docs * 256 tokens = 1,000,448 tokens.
 */

import http from 'k6/http';
import { check } from 'k6';
import { Counter, Trend } from 'k6/metrics';

// Configuration
const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const DOCS_PER_REQUEST = 4;
const TOKENS_PER_DOC = 256;
const CHARS_PER_DOC = 900; // ~3.5 chars/token buffer
const TOTAL_REQUESTS_FOR_MILLION = 977;
const MAX_VUS = Number(__ENV.VUS || 15); // High parallelism

const TARGET_TOKENS = DOCS_PER_REQUEST * TOKENS_PER_DOC * TOTAL_REQUESTS_FOR_MILLION;

// Custom metrics
const totalTokens = new Counter('total_tokens_processed');
const requestDuration = new Trend('request_duration_ms');

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

function generateDocument(index) {
    const words = [];
    let charCount = 0;

    words.push(`[Doc${index}]`);
    charCount += 8;

    while (charCount < CHARS_PER_DOC) {
        const word = WORD_POOL[(index + words.length) % WORD_POOL.length];
        words.push(word);
        charCount += word.length + 1;
    }

    return words.join(' ');
}

function generateDocuments(startIndex) {
    const docs = [];
    for (let i = 0; i < DOCS_PER_REQUEST; i++) {
        docs.push(generateDocument(startIndex + i));
    }
    return docs;
}

// High-parallelism, small-payload scenario
export const options = {
    scenarios: {
        million_tokens: {
            executor: 'shared-iterations',
            vus: MAX_VUS,
            iterations: TOTAL_REQUESTS_FOR_MILLION,
            maxDuration: '15m',
        },
    },
    thresholds: {
        http_req_duration: ['p(95)<250'], // keep single-request latency snappy
        http_req_failed: ['rate==0'],
        request_duration_ms: ['p(95)<250'],
    },
};

export default function() {
    // __ITER is the global iteration counter across VUs
    const documents = generateDocuments(__ITER * DOCS_PER_REQUEST);

    const payload = {
        query: 'What are the key concepts in machine learning and deep neural networks?',
        documents,
    };

    const payloadStr = JSON.stringify(payload);

    const startTime = Date.now();
    const res = http.post(`${BASE_URL}/rerank`, payloadStr, {
        headers: { 'Content-Type': 'application/json' },
        timeout: '120s',
    });
    const duration = Date.now() - startTime;
    requestDuration.add(duration);

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
                const hasCorrect = body.results.length === DOCS_PER_REQUEST;
                if (!hasCorrect) {
                    console.error(`Expected ${DOCS_PER_REQUEST} results, got ${body.results.length}`);
                }
                return hasCorrect;
            } catch {
                return false;
            }
        },
        'all scores are valid': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.results.every((result) =>
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
                    if (body.results[i].score > body.results[i - 1].score) {
                        console.error(`Sort error at index ${i}: ${body.results[i].score} > ${body.results[i - 1].score}`);
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
        totalTokens.add(DOCS_PER_REQUEST * TOKENS_PER_DOC);
    }
}
