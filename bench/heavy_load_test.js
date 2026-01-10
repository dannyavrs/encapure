/**
 * Encapure Load Test - k6
 *
 * Simulates Vector DB payloads: 8 documents x ~256 tokens each
 * Tests 3 concurrency stages against 10 physical cores.
 *
 * Prerequisites:
 *   - k6 installed: winget install Grafana.k6
 *
 * Usage:
 *   k6 run bench/heavy_load_test.js
 *   k6 run bench/heavy_load_test.js --env BASE_URL=http://localhost:8080
 *   k6 run bench/heavy_load_test.js --env STAGE=under    # Run single stage
 *   k6 run bench/heavy_load_test.js --env STAGE=ideal
 *   k6 run bench/heavy_load_test.js --env STAGE=saturation
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

// Test parameters
const DOCS_PER_REQUEST = 8;
const TOKENS_PER_DOC = 256;
const CHARS_PER_DOC = 1000;  // ~256 tokens ≈ 1000 chars
const TOKENS_PER_REQUEST = DOCS_PER_REQUEST * TOKENS_PER_DOC;  // 2,048

// =============================================================================
// Test Data Generation
// =============================================================================

// Generate realistic ML/AI themed paragraphs (~4000 chars each)
function generateDocument(index) {
    const topics = [
        `Document ${index}: Machine learning algorithms form the backbone of modern artificial intelligence systems. These sophisticated computational methods enable computers to learn patterns from data without being explicitly programmed for each specific task. The field has evolved dramatically over the past decades, with deep learning architectures achieving remarkable breakthroughs in computer vision, natural language processing, and reinforcement learning. Neural networks with multiple hidden layers can automatically discover hierarchical feature representations, eliminating the need for manual feature engineering that was traditionally required. Gradient descent optimization, combined with backpropagation, allows these networks to adjust millions of parameters to minimize prediction errors. The introduction of techniques like batch normalization, dropout regularization, and residual connections has made training very deep networks feasible. Modern frameworks such as TensorFlow and PyTorch provide automatic differentiation capabilities that simplify the implementation of complex architectures. Transfer learning has become particularly important, allowing models pretrained on large datasets to be fine-tuned for specific downstream tasks with limited labeled data. The attention mechanism, originally introduced for machine translation, has revolutionized natural language processing through transformer architectures. Self-supervised learning methods have emerged as powerful alternatives when labeled data is scarce, learning useful representations from unlabeled examples.`,

        `Document ${index}: Natural language processing represents one of the most challenging and impactful areas of artificial intelligence research. Understanding human language requires handling ambiguity, context dependence, and the vast complexity of linguistic structures across different languages and domains. Modern NLP systems leverage large language models trained on billions of text tokens, capturing statistical patterns that approximate linguistic competence. Tokenization strategies, from simple whitespace splitting to sophisticated subword algorithms like BPE and SentencePiece, determine how text is converted to numerical representations. Word embeddings like Word2Vec and GloVe capture semantic relationships in dense vector spaces where similar words have similar representations. Contextual embeddings from models like BERT and GPT dynamically adjust word representations based on surrounding context, handling polysemy and other linguistic phenomena. Named entity recognition, part-of-speech tagging, and dependency parsing form the foundation of information extraction pipelines. Sentiment analysis and opinion mining help businesses understand customer feedback at scale. Question answering systems combine information retrieval with reading comprehension capabilities. Machine translation has achieved near-human quality for many language pairs through encoder-decoder architectures with attention mechanisms. Text generation models can produce coherent long-form content, raising both opportunities and concerns about authenticity.`,

        `Document ${index}: Computer vision has undergone a revolutionary transformation with the advent of convolutional neural networks. These architectures exploit the spatial structure of images through local receptive fields and weight sharing across locations. The seminal AlexNet architecture demonstrated the power of deep learning for image classification on the ImageNet benchmark, sparking widespread adoption. Subsequent architectures like VGG, ResNet, and EfficientNet have pushed accuracy boundaries while improving computational efficiency. Object detection combines classification with localization, identifying multiple objects and their bounding boxes within images. Architectures like YOLO and Faster R-CNN achieve real-time detection performance suitable for autonomous vehicles and surveillance systems. Semantic segmentation assigns class labels to every pixel, enabling precise scene understanding for medical imaging and robotics applications. Instance segmentation extends this by distinguishing individual object instances of the same class. Generative models like GANs and VAEs can synthesize realistic images, enabling applications in art, design, and data augmentation. Self-supervised methods like contrastive learning have reduced dependence on labeled data by learning from image transformations. Vision transformers have recently challenged CNN dominance, demonstrating that attention mechanisms can effectively process image patches.`,

        `Document ${index}: Reinforcement learning addresses sequential decision-making problems where an agent learns through interaction with an environment. The agent receives observations, takes actions, and obtains rewards that guide learning toward optimal behavior. The exploration-exploitation tradeoff is fundamental: agents must try new actions to discover better strategies while also exploiting known good actions. Value-based methods like Q-learning estimate the expected cumulative reward for state-action pairs, enabling greedy action selection. Policy gradient methods directly optimize the policy function that maps states to actions, handling continuous action spaces naturally. Actor-critic architectures combine both approaches, using a critic to reduce variance in policy gradient estimates. Deep reinforcement learning applies neural networks as function approximators, enabling learning in high-dimensional state spaces like raw pixels. Notable achievements include superhuman performance in Atari games, Go, and complex strategy games like StarCraft II. Model-based reinforcement learning learns environment dynamics to enable planning and improve sample efficiency. Offline reinforcement learning aims to learn from previously collected datasets without additional environment interaction. Multi-agent reinforcement learning studies scenarios where multiple agents interact, leading to emergent cooperative or competitive behaviors.`,

        `Document ${index}: The deployment of machine learning models in production systems requires careful consideration of numerous engineering challenges. Model serving infrastructure must handle variable request loads while maintaining low latency guarantees for real-time applications. Containerization through Docker and orchestration via Kubernetes has become standard practice for scalable deployment. Model optimization techniques including quantization, pruning, and knowledge distillation reduce computational requirements for edge deployment. ONNX provides a standardized format for model interoperability across different frameworks and runtimes. A/B testing frameworks enable controlled experimentation to validate model improvements before full rollout. Feature stores centralize feature computation and serving, ensuring consistency between training and inference pipelines. Model monitoring systems track prediction distributions, detecting data drift and concept drift that can degrade performance. Explainability tools help interpret model decisions, supporting debugging and regulatory compliance requirements. MLOps practices adapt DevOps principles for machine learning, emphasizing reproducibility, version control, and automated pipelines. Continuous training pipelines automatically retrain models as new data becomes available, maintaining accuracy over time. The full machine learning lifecycle spans data collection, feature engineering, model training, evaluation, deployment, and ongoing monitoring.`,
    ];

    // Select topic based on index, repeat and vary if needed
    const baseText = topics[index % topics.length];

    // Pad or extend to reach ~4000 characters
    let text = baseText;
    while (text.length < CHARS_PER_DOC) {
        text += ` Furthermore, advanced techniques in ${['optimization', 'regularization', 'architecture design', 'hyperparameter tuning', 'data augmentation'][index % 5]} continue to push the boundaries of what is achievable. `;
    }

    return text.substring(0, CHARS_PER_DOC);
}

// Generate all 50 documents
const documents = [];
for (let i = 0; i < DOCS_PER_REQUEST; i++) {
    documents.push(generateDocument(i));
}

// The heavy payload request
const heavyPayload = {
    query: "What are the most effective techniques for training and deploying large-scale machine learning models in production environments with strict latency requirements?",
    documents: documents,
};

const payloadJson = JSON.stringify(heavyPayload);
const payloadSizeInKb = payloadJson.length / 1024;

// =============================================================================
// Scenario Configuration
// =============================================================================

export const options = {
    scenarios: {
        // Stage 1: Under-saturation (5 VUs < 10 physical cores)
        stage_under_saturation: {
            executor: 'constant-vus',
            vus: 5,
            duration: '60s',
            startTime: '0s',
            exec: 'heavyLoadTest',
            tags: { stage: 'under' },
            env: { STAGE_NAME: 'under_saturation' },
        },
        // Stage 2: Ideal 1:1 mapping (10 VUs = 10 physical cores)
        stage_ideal_load: {
            executor: 'constant-vus',
            vus: 10,
            duration: '60s',
            startTime: '65s',
            exec: 'heavyLoadTest',
            tags: { stage: 'ideal' },
            env: { STAGE_NAME: 'ideal_load' },
        },
        // Stage 3: Saturation (20 VUs > 10 physical cores)
        stage_saturation: {
            executor: 'constant-vus',
            vus: 20,
            duration: '60s',
            startTime: '130s',
            exec: 'heavyLoadTest',
            tags: { stage: 'saturation' },
            env: { STAGE_NAME: 'saturation' },
        },
    },
    thresholds: {
        // Overall thresholds
        'http_req_duration': ['p(95)<10000'],           // P95 < 10s overall
        'http_req_failed': ['rate<0.05'],               // <5% error rate
        'success_rate': ['rate>0.95'],                  // >95% success

        // Per-stage P95 thresholds
        'http_req_duration{stage:under}': ['p(95)<3000'],      // Under-saturation: P95 < 3s
        'http_req_duration{stage:ideal}': ['p(95)<5000'],      // Ideal load: P95 < 5s
        'http_req_duration{stage:saturation}': ['p(95)<10000'], // Saturation: P95 < 10s
    },

    // Network settings for large payloads
    noConnectionReuse: false,
    userAgent: 'k6-heavy-load-test/1.0',
};

// Override to run single stage if specified
if (STAGE !== 'all') {
    const stageMap = {
        'under': 'stage_under_saturation',
        'ideal': 'stage_ideal_load',
        'saturation': 'stage_saturation',
    };
    const selectedScenario = stageMap[STAGE];
    if (selectedScenario && options.scenarios[selectedScenario]) {
        const scenario = options.scenarios[selectedScenario];
        scenario.startTime = '0s';  // Start immediately for single stage
        options.scenarios = { [selectedScenario]: scenario };
    }
}

// =============================================================================
// Setup - Print test info
// =============================================================================

export function setup() {
    console.log('\n========================================');
    console.log('  ENCAPURE HEAVY PAYLOAD LOAD TEST');
    console.log('========================================\n');
    console.log(`Target:           ${BASE_URL}/rerank`);
    console.log(`Documents/req:    ${DOCS_PER_REQUEST}`);
    console.log(`Tokens/doc:       ~${TOKENS_PER_DOC}`);
    console.log(`Tokens/req:       ${TOKENS_PER_REQUEST.toLocaleString()}`);
    console.log(`Payload size:     ${payloadSizeInKb.toFixed(1)} KB`);
    console.log(`\nStages:`);
    console.log(`  1. Under-saturation: 5 VUs  (60s)`);
    console.log(`  2. Ideal load:       10 VUs (60s)`);
    console.log(`  3. Saturation:       20 VUs (60s)`);
    console.log('\n----------------------------------------\n');

    // Warmup request
    console.log('Performing warmup request...');
    const warmupRes = http.post(`${BASE_URL}/rerank`, payloadJson, {
        headers: { 'Content-Type': 'application/json' },
        timeout: '120s',
    });

    if (warmupRes.status === 200) {
        console.log(`Warmup complete: ${warmupRes.timings.duration.toFixed(0)} ms\n`);
    } else {
        console.error(`Warmup failed: ${warmupRes.status} - ${warmupRes.body}`);
    }

    return { payloadSize: payloadSizeInKb };
}

// =============================================================================
// Main Test Function
// =============================================================================

export function heavyLoadTest(data) {
    const url = `${BASE_URL}/rerank`;

    const res = http.post(url, payloadJson, {
        headers: { 'Content-Type': 'application/json' },
        timeout: '120s',  // 2 minute timeout for large payloads
    });

    const success = res.status === 200;
    const durationSec = res.timings.duration / 1000;

    // Record metrics
    successRate.add(success);
    payloadSizeKb.add(data.payloadSize);

    if (success) {
        requestsSuccessful.add(1);

        // Calculate tokens per second for this request
        const tps = TOKENS_PER_REQUEST / durationSec;
        tokensPerSecond.add(tps);
        totalTokensProcessed.add(TOKENS_PER_REQUEST);

        // Validate response structure
        check(res, {
            'status is 200': (r) => r.status === 200,
            'has 8 results': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.results && body.results.length === DOCS_PER_REQUEST;
                } catch {
                    return false;
                }
            },
            'results are sorted by score': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    for (let i = 1; i < body.results.length; i++) {
                        if (body.results[i].score > body.results[i-1].score) {
                            return false;
                        }
                    }
                    return true;
                } catch {
                    return false;
                }
            },
        });
    } else {
        requestsFailed.add(1);
        console.error(`[VU ${__VU}] Request failed: ${res.status} - ${res.body?.substring(0, 200)}`);
    }
}

// =============================================================================
// Summary Handler
// =============================================================================

export function handleSummary(data) {
    const metrics = data.metrics;

    // Extract per-stage metrics
    const underDuration = metrics['http_req_duration{stage:under}'];
    const idealDuration = metrics['http_req_duration{stage:ideal}'];
    const saturationDuration = metrics['http_req_duration{stage:saturation}'];
    const tps = metrics['tokens_per_second'];
    const totalTokens = metrics['total_tokens_processed'];

    let summary = `
========================================
  HEAVY PAYLOAD LOAD TEST RESULTS
========================================

Test Configuration:
  Documents/request:  ${DOCS_PER_REQUEST}
  Tokens/document:    ~${TOKENS_PER_DOC}
  Tokens/request:     ${TOKENS_PER_REQUEST.toLocaleString()}
  Payload size:       ${payloadSizeInKb.toFixed(1)} KB

`;

    // Stage results
    summary += `Stage Results:\n`;
    summary += `-`.repeat(60) + `\n`;

    if (underDuration) {
        const avgTps = tps ? (tps.values.avg || 0) : 0;
        summary += `
  Stage 1: Under-saturation (5 VUs)
    Requests:        ${underDuration.values.count}
    P50 Latency:     ${underDuration.values.med?.toFixed(0) || 'N/A'} ms
    P95 Latency:     ${underDuration.values['p(95)']?.toFixed(0) || 'N/A'} ms
    P99 Latency:     ${underDuration.values['p(99)']?.toFixed(0) || 'N/A'} ms
    Max Latency:     ${underDuration.values.max?.toFixed(0) || 'N/A'} ms
`;
    }

    if (idealDuration) {
        summary += `
  Stage 2: Ideal Load (10 VUs = 10 cores)
    Requests:        ${idealDuration.values.count}
    P50 Latency:     ${idealDuration.values.med?.toFixed(0) || 'N/A'} ms
    P95 Latency:     ${idealDuration.values['p(95)']?.toFixed(0) || 'N/A'} ms
    P99 Latency:     ${idealDuration.values['p(99)']?.toFixed(0) || 'N/A'} ms
    Max Latency:     ${idealDuration.values.max?.toFixed(0) || 'N/A'} ms
`;
    }

    if (saturationDuration) {
        summary += `
  Stage 3: Saturation (20 VUs > 10 cores)
    Requests:        ${saturationDuration.values.count}
    P50 Latency:     ${saturationDuration.values.med?.toFixed(0) || 'N/A'} ms
    P95 Latency:     ${saturationDuration.values['p(95)']?.toFixed(0) || 'N/A'} ms
    P99 Latency:     ${saturationDuration.values['p(99)']?.toFixed(0) || 'N/A'} ms
    Max Latency:     ${saturationDuration.values.max?.toFixed(0) || 'N/A'} ms
`;
    }

    // Tokens per second metrics
    if (tps && tps.values.count > 0) {
        summary += `
Throughput (Tokens Processed):
-`.repeat(30) + `
  Avg Tokens/sec:    ${Math.round(tps.values.avg).toLocaleString()}
  Min Tokens/sec:    ${Math.round(tps.values.min).toLocaleString()}
  Max Tokens/sec:    ${Math.round(tps.values.max).toLocaleString()}
  Total Tokens:      ${Math.round(totalTokens?.values?.count * TOKENS_PER_REQUEST || 0).toLocaleString()}
`;
    }

    // Overall summary
    const httpReqs = metrics['http_reqs'];
    const successRateValue = metrics['success_rate'];
    const httpFailed = metrics['http_req_failed'];

    summary += `
Overall:
-`.repeat(30) + `
  Total Requests:    ${httpReqs?.values?.count || 0}
  Success Rate:      ${((successRateValue?.values?.rate || 0) * 100).toFixed(1)}%
  Failed Requests:   ${metrics['requests_failed']?.values?.count || 0}

`;

    // Performance assessment
    summary += `Performance Assessment:\n`;
    summary += `-`.repeat(60) + `\n`;

    const thresholdsPassed = [];
    const thresholdsFailed = [];

    if (underDuration) {
        const p95 = underDuration.values['p(95)'];
        if (p95 < 3000) {
            thresholdsPassed.push(`  [PASS] Stage 1 P95 < 3s (got ${p95?.toFixed(0)}ms)`);
        } else {
            thresholdsFailed.push(`  [FAIL] Stage 1 P95 >= 3s (got ${p95?.toFixed(0)}ms)`);
        }
    }

    if (idealDuration) {
        const p95 = idealDuration.values['p(95)'];
        if (p95 < 5000) {
            thresholdsPassed.push(`  [PASS] Stage 2 P95 < 5s (got ${p95?.toFixed(0)}ms)`);
        } else {
            thresholdsFailed.push(`  [FAIL] Stage 2 P95 >= 5s (got ${p95?.toFixed(0)}ms)`);
        }
    }

    if (saturationDuration) {
        const p95 = saturationDuration.values['p(95)'];
        if (p95 < 10000) {
            thresholdsPassed.push(`  [PASS] Stage 3 P95 < 10s (got ${p95?.toFixed(0)}ms)`);
        } else {
            thresholdsFailed.push(`  [FAIL] Stage 3 P95 >= 10s (got ${p95?.toFixed(0)}ms)`);
        }
    }

    if (successRateValue && successRateValue.values.rate >= 0.95) {
        thresholdsPassed.push(`  [PASS] Success rate > 95% (got ${(successRateValue.values.rate * 100).toFixed(1)}%)`);
    } else {
        thresholdsFailed.push(`  [FAIL] Success rate < 95% (got ${((successRateValue?.values?.rate || 0) * 100).toFixed(1)}%)`);
    }

    summary += thresholdsPassed.join('\n') + '\n';
    if (thresholdsFailed.length > 0) {
        summary += thresholdsFailed.join('\n') + '\n';
    }

    summary += `\n========================================\n`;

    console.log(summary);

    return {
        'stdout': summary,
        'bench/heavy_load_results.json': JSON.stringify(data, null, 2),
    };
}
