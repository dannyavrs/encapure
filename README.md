# Encapure

High-performance, context-aware semantic tool search engine built in Rust. Designed for MCP (Model Context Protocol) environments where AI agents need to find the right tool from thousands of available options in sub-100ms latency.

**Stack:** Rust, Axum, ONNX Runtime (INT8 quantized), Tokio

---

## Table of Contents

- [Quick Start](#quick-start)
- [Installation](#installation)
- [Configuration](#configuration)
- [Operating Modes](#operating-modes)
- [API Reference](#api-reference)
- [Benchmarks](#benchmarks)
- [Architecture](#architecture)
- [Project Structure](#project-structure)

---

## Quick Start

```bash
git clone https://github.com/dannz0/encapure.git
cd encapure

# Download pre-quantized models (~400 MB)
bash scripts/download_models.sh

# Build
cargo build --release

# Start server (single-request mode)
ENCAPURE_MODE=single \
TOOLS_PATH=tests/data/comprehensive_mock_tools.json \
RUST_LOG=encapure=info \
./target/release/encapure
```

Verify it works:

```bash
curl http://localhost:8080/health

curl -X POST http://localhost:8080/search \
  -H "Content-Type: application/json" \
  -d '{"query": "send a message", "top_k": 5}'
```

---

## Installation

### Prerequisites

- **Rust 1.88+** (required by the `ort` crate for ONNX Runtime bindings)
- **cmake**, **pkg-config**, **libssl-dev**, **libclang-dev** (Linux build dependencies)
- **k6** (optional, for load testing): `winget install Grafana.k6` or `brew install k6`

### Step 1: Clone the Repository

```bash
git clone https://github.com/dannz0/encapure.git
cd encapure
```

### Step 2: Download Models

The ONNX model files are hosted as GitHub Release assets (too large for git). Run the download script:

**Linux / macOS:**
```bash
bash scripts/download_models.sh
```

**Windows (PowerShell):**
```powershell
powershell -ExecutionPolicy Bypass -File scripts/download_models.ps1
```

This downloads and extracts the following models into the project root:

| Directory | Model | Size | Purpose |
|---|---|---|---|
| `models/` | BGE-Reranker-v2-M3 (INT8) | ~266 MB | Cross-encoder for precise reranking |
| `bi-encoder-model/` | all-MiniLM-L6-v2 (INT8) | ~105 MB | Bi-encoder for fast semantic retrieval |

### Step 3: Build

```bash
cargo build --release
```

The first build downloads the ONNX Runtime library automatically via the `ort` crate's `download-binaries` feature.

### Step 4: Generate Embeddings Cache (Optional)

On first startup with a tools dataset, Encapure computes bi-encoder embeddings for all tools and caches them to `.encapure/embeddings.bin`. Subsequent startups load the cache instantly. The repository includes a pre-computed cache for the included 1,000-tool dataset.

---

## Configuration

All configuration is via environment variables.

### Core Settings

| Variable | Default | Description |
|---|---|---|
| `ENCAPURE_MODE` | _(custom)_ | Operating mode preset: `single`, `concurrent`, or unset for custom |
| `HOST` | `0.0.0.0` | Bind address |
| `PORT` | `8080` | Listen port |
| `RUST_LOG` | — | Log level filter (e.g. `encapure=info`, `encapure=debug`) |

### Model Paths

| Variable | Default | Description |
|---|---|---|
| `MODEL_PATH` | `./models/model_int8.onnx` | Cross-encoder ONNX model |
| `TOKENIZER_PATH` | `./models/tokenizer.json` | Cross-encoder tokenizer |
| `BI_ENCODER_MODEL_PATH` | `./bi-encoder-model/model_int8.onnx` | Bi-encoder ONNX model |
| `BI_ENCODER_TOKENIZER_PATH` | `./bi-encoder-model/tokenizerbiencoder.json` | Bi-encoder tokenizer |
| `EMBEDDINGS_CACHE_PATH` | `.encapure/embeddings.bin` | Pre-computed embeddings cache |

### Tools & Inference

| Variable | Default | Description |
|---|---|---|
| `TOOLS_PATH` | _(none)_ | Path to tools JSON file. Required for `/search` endpoint |
| `RETRIEVAL_CANDIDATES` | `20` | Bi-encoder top-K candidates passed to cross-encoder reranking |
| `MAX_SEQ_LENGTH` | `1024` | Maximum token sequence length per input |
| `MAX_DOCUMENTS` | `100000` | Maximum documents per `/rerank` request |
| `BATCH_SIZE` | `32` | Internal inference batch size |

### Performance Tuning

| Variable | Default | Description |
|---|---|---|
| `POOL_SIZE` | _(auto)_ | Number of ONNX Runtime sessions in the pool |
| `PERMITS` | _(auto)_ | Semaphore permits controlling max concurrent inferences |
| `INTRA_THREADS` | `8` | Threads per ONNX session (intra-op parallelism) |
| `SHUTDOWN_TIMEOUT` | `30` | Graceful shutdown timeout in seconds |

The relationship between these is: **`PERMITS x INTRA_THREADS <= physical CPU cores`**. Exceeding this causes CPU oversubscription and thrashing.

---

## Operating Modes

Encapure provides two preset configurations via the `ENCAPURE_MODE` environment variable, optimized for different workloads. You can also leave it unset and configure `POOL_SIZE`, `PERMITS`, and `INTRA_THREADS` individually.

### Single Mode

```
ENCAPURE_MODE=single
```

| Parameter | Value |
|---|---|
| `POOL_SIZE` | 1 |
| `PERMITS` | 1 |
| `INTRA_THREADS` | 8 |

**What it does:** Allocates all CPU threads to a single inference session. One request is processed at a time with maximum parallelism per request.

**Use when:** You need the lowest possible latency for individual requests (e.g. interactive demos, single-agent setups).

**Target performance:** ~70ms average per request.

Aliases: `single`, `low-latency`, `single-request`

### Concurrent Mode

```
ENCAPURE_MODE=concurrent
```

| Parameter | Value |
|---|---|
| `POOL_SIZE` | 10 |
| `PERMITS` | 6 |
| `INTRA_THREADS` | 2 |

**What it does:** Maintains a pool of 10 ONNX sessions with a semaphore limiting 6 concurrent inferences. Each session uses 2 threads (6 x 2 = 12 threads, matching a 12-core CPU).

**Use when:** Multiple agents or clients send requests simultaneously (e.g. production deployment, multi-agent orchestration).

**Target performance:** ~350ms average, 30+ req/sec sustained, handles hundreds of concurrent agents.

Aliases: `concurrent`, `high-throughput`, `multi`

### Why Single Mode Has Lower Latency

In single mode, all 8 threads work on one request simultaneously (full intra-op parallelism). In concurrent mode, each request only gets 2 threads — so individual requests are slower, but 6 can run in parallel, yielding higher total throughput.

---

## API Reference

| Method | Path | Description |
|---|---|---|
| `POST` | `/search` | Context-aware semantic tool search |
| `POST` | `/rerank` | Cross-encoder reranking (50 MB body limit) |
| `GET` | `/health` | Liveness check |
| `GET` | `/ready` | Readiness check |
| `GET` | `/metrics` | Prometheus metrics |

### POST /search

Search for tools from the loaded dataset. The `agent_description` field is the core differentiator — the same query returns different tools depending on the agent's context.

**Request:**
```json
{
  "query": "send a message",
  "top_k": 5,
  "agent_description": "Slack communication bot for workplace chat"
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `query` | string | yes | Natural language search query |
| `top_k` | integer | yes | Number of results to return |
| `agent_description` | string | no | Agent role/context that biases results toward relevant tools |

**Response:**
```json
{
  "results": [
    { "name": "send_slack_message", "description": "Send a message to a Slack channel", "score": 0.94 },
    { "name": "send_slack_dm", "description": "Send a direct message on Slack", "score": 0.89 },
    { "name": "post_slack_message", "description": "Post a message to Slack", "score": 0.85 }
  ]
}
```

**Context-awareness example:** The same query `"send message"` returns:
- No context → `send_message`, `send_sms`, `send_notification`
- Slack context → `send_slack_message`, `send_slack_dm`
- Email context → `send_email`, `send_email_notification`
- Teams context → `send_teams_message`, `send_teams_chat`

### POST /rerank

Rerank a list of documents against a query using the cross-encoder.

**Request:**
```json
{
  "query": "machine learning optimization",
  "documents": ["doc1 text...", "doc2 text...", "doc3 text..."]
}
```

**Response:**
```json
{
  "results": [
    { "index": 2, "score": 0.92 },
    { "index": 0, "score": 0.71 },
    { "index": 1, "score": 0.34 }
  ]
}
```

---

## Benchmarks

### Accuracy Test (Single Mode)

The accuracy demo (`scripts/investor_demo.ps1`) runs 11 test cases across 4 categories, demonstrating context-aware tool routing. It measures both correctness and per-request latency.

**Start the server in single mode:**

```powershell
$env:ENCAPURE_MODE = "single"
$env:TOOLS_PATH = "tests/data/comprehensive_mock_tools.json"
$env:RUST_LOG = "encapure=info"
.\target\release\encapure.exe
```

**Run the accuracy demo (separate terminal):**

```powershell
powershell -ExecutionPolicy Bypass -File scripts/investor_demo.ps1
```

Add `-AutoRun` to skip the interactive pauses:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/investor_demo.ps1 -AutoRun
```

**Test categories:**

| Demo | Query | Contexts Tested | Expected Behavior |
|---|---|---|---|
| 1. Context-Aware Routing | `"send message"` | None, Slack, Email, Teams | Different messaging tools per context |
| 2. Cloud Provider | `"create server"` | AWS, Azure, GCP | Provider-specific VM/instance tools |
| 3. Project Management | `"create task"` | Jira, Asana | Platform-specific task tools |
| 4. Database Selection | `"query data"`, `"find documents"` | PostgreSQL, MongoDB | SQL vs NoSQL tools |

**Expected results:**
- Accuracy: 90%+ (at least 10/11 tests pass)
- Average latency: ~70ms per request
- Latency color coding: green (<100ms), yellow (<200ms), red (>200ms)

There is also a bash version for Linux/macOS:

```bash
bash scripts/investor_demo_preview.sh
```

### Load Test (Concurrent Mode)

The load test (`bench/load_test_1000_tools.js`) uses [k6](https://k6.io/) to simulate concurrent agents querying the engine with diverse queries.

**Start the server in concurrent mode:**

```powershell
$env:ENCAPURE_MODE = "concurrent"
$env:TOOLS_PATH = "tests/data/comprehensive_mock_tools.json"
$env:RUST_LOG = "encapure=info"
.\target\release\encapure.exe
```

Or on Linux/macOS:

```bash
ENCAPURE_MODE=concurrent \
TOOLS_PATH=tests/data/comprehensive_mock_tools.json \
RUST_LOG=encapure=info \
./target/release/encapure
```

**Run the load test (separate terminal):**

```bash
# Default: 10 virtual users, 30 seconds
k6 run bench/load_test_1000_tools.js

# Custom: 50 virtual users, 60 seconds
k6 run bench/load_test_1000_tools.js --env VUS=50 --env DURATION=60s

# Ramp-up: 1 → 200 virtual users over 4 minutes (finds breaking point)
k6 run bench/load_test_1000_tools.js --env RAMP_UP=true
```

**Expected results (concurrent mode):**
- Average latency: ~350ms
- Throughput: 30+ requests/sec sustained
- Success rate: 99%+
- P95 latency: <500ms
- P99 latency: <1000ms

The test outputs a summary table with latency percentiles, throughput, and success rates. Results are also saved to `bench/results/load_test_results.json`.

### Full Accuracy Suite

For comprehensive validation, there are 50 test cases in `tests/data/accuracy_test_cases.json` covering context switching, DevOps, project management, databases, cloud storage, monitoring, notifications, git operations, and edge cases:

```bash
bash scripts/run_accuracy_tests.sh
```

Expected: 98%+ accuracy (49/50 tests pass).

---

## Architecture

### Two-Stage Retrieval Pipeline

```
Query + Agent Context
        |
        v
  ┌─────────────┐
  │  Bi-Encoder  │    Stage 1: Fast semantic search
  │  MiniLM-L6   │    Cosine similarity over pre-computed embeddings
  │  INT8 ONNX   │    Returns top 20 candidates (~5ms)
  └──────┬───────┘
         |
         v
  ┌─────────────┐
  │Cross-Encoder │    Stage 2: Precise reranking
  │BGE-Reranker  │    Full attention over (query, document) pairs
  │  v2-M3 INT8  │    Reranks 20 → returns top K (~60ms)
  └──────┬───────┘
         |
         v
    Ranked Results
```

**Stage 1 (Bi-Encoder):** Computes a single embedding for the query and compares it against pre-computed tool embeddings via cosine similarity. This is O(n) but extremely fast because it's just vector dot products. Returns the top 20 candidates.

**Stage 2 (Cross-Encoder):** Takes each (query, tool_description) pair and runs full transformer attention to compute a precise relevance score. This is much more accurate but O(k) in model inference cost. Reranks the 20 candidates and returns the top K.

### Context-Aware Search

When `agent_description` is provided, the query is augmented to: `"Agent Context: {description}. Query: {query}"`. This biases the bi-encoder and cross-encoder toward tools relevant to the agent's domain, enabling the same query to return different tools for different agent roles.

### Concurrency Model

- **Session Pool:** Multiple ONNX Runtime sessions (`Vec<UnsafeCell<Session>>`) with atomic round-robin index for lock-free selection
- **Semaphore:** `tokio::sync::Semaphore` limits concurrent inference to prevent CPU oversubscription
- **Thread Budget:** `PERMITS x INTRA_THREADS <= physical_cores` — this invariant prevents CPU thrashing
- **Embeddings Cache:** Pre-computed bi-encoder embeddings loaded from `.encapure/embeddings.bin` at startup, avoiding model loading when a cache exists

### Dataset

The included dataset (`tests/data/comprehensive_mock_tools.json`) contains **1,000 MCP tools** across 40+ categories: cloud providers (AWS, Azure, GCP), databases (PostgreSQL, MongoDB, Redis), messaging platforms (Slack, Teams, Email), DevOps (Kubernetes, Docker, Terraform), project management (Jira, Asana, Linear, Trello), and more.

---

## Project Structure

```
encapure/
├── src/
│   ├── main.rs                  # Server init, routing, graceful shutdown
│   ├── config.rs                # Env-based config, OperatingMode enum
│   ├── state.rs                 # Shared app state (Arc)
│   ├── error.rs                 # Error types → HTTP status mapping
│   ├── handlers/
│   │   ├── search.rs            # POST /search — context-aware tool search
│   │   ├── rerank.rs            # POST /rerank — cross-encoder reranking
│   │   └── health.rs            # GET /health, /ready
│   ├── inference/
│   │   ├── model.rs             # Cross-encoder session pool + inference
│   │   ├── bi_encoder.rs        # Bi-encoder session pool + embeddings
│   │   └── tokenize.rs          # Tokenization utilities
│   ├── ingestion/
│   │   ├── atomizer.rs          # Tool JSON ingestion + atomization
│   │   └── types.rs             # Tool data structures
│   └── persistence/
│       └── mod.rs               # Embeddings cache serialization
├── models/                      # Cross-encoder (BGE-Reranker-v2-M3, INT8)
├── bi-encoder-model/            # Bi-encoder (all-MiniLM-L6-v2, INT8)
├── .encapure/                   # Pre-computed embeddings cache
├── tests/data/
│   ├── comprehensive_mock_tools.json  # 1,000 MCP tools dataset
│   └── accuracy_test_cases.json       # 50 accuracy test cases
├── bench/
│   └── load_test_1000_tools.js  # k6 load test script
├── scripts/
│   ├── download_models.sh       # Model downloader (Linux/macOS)
│   ├── download_models.ps1      # Model downloader (Windows)
│   ├── investor_demo.ps1        # Accuracy demo (PowerShell)
│   ├── investor_demo_preview.sh # Accuracy demo (bash)
│   ├── run_accuracy_tests.sh    # Full 50-test accuracy suite
│   └── quick_accuracy_test.sh   # Quick 10-test validation
├── python/
│   └── export_model.py          # Model export + INT8 quantization script
├── Cargo.toml
└── README.md
```
