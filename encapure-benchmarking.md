# Encapure MVP Implementation Guide for Claude Code

## Project Overview

Encapure is a high-performance reranking microservice in Rust that serves the BAAI/bge-reranker-v2-m3 model via ONNX Runtime. The codebase is well-structured but has critical issues preventing it from meeting production benchmarks.

## MVP Success Criteria

1. Python script creates `models/model_quantized.onnx` (~150-200MB)
2. `cargo build --release` succeeds
3. `cargo clippy -- -D warnings` passes with zero warnings
4. `cargo test` passes all tests
5. Server starts in less than 5 seconds
6. `curl localhost:8080/health` returns HTTP 200
7. `curl localhost:8080/metrics` returns Prometheus format
8. Rerank request returns documents sorted by relevance score
9. Memory usage under 500MB, P99 latency under 50ms, throughput over 200 requests per second
10. Graceful shutdown properly drains connections

**Important constraint:** No request batching - this adds debugging complexity and latency that is not acceptable for MVP.

---

## Current State Assessment

### What Already Works

- Project structure is clean with proper separation of concerns
- Axum web framework setup is correct in `src/main.rs`
- Prometheus metrics endpoint exists at `/metrics`
- Health endpoint exists at `/health`
- Graceful shutdown signal handling is implemented
- Error handling with custom error types in `src/error.rs`
- Configuration loading from environment variables in `src/config.rs`
- Python export script in `python/export_model.py` is ready
- Release profile in `Cargo.toml` has LTO and aggressive optimizations enabled

### Critical Issues Blocking Benchmarks

#### Issue 1: Mutex Bottleneck in Model Inference (HIGHEST PRIORITY)

**Location:** `src/inference/model.rs`

**Problem:** The `RerankerModel` struct wraps the ONNX Session in a `std::sync::Mutex`. This serializes ALL inference requests - only one request can run inference at a time, regardless of available CPU cores. The Semaphore in `src/state.rs` allows N concurrent requests to proceed, but they all queue up waiting for the single Mutex lock.

**Impact:** Maximum throughput is approximately 10-20 requests per second instead of the target 200+ requests per second. This is the primary blocker for the performance benchmark.

**Solution:** Replace the single Mutex-wrapped Session with a session pool. Create N separate ONNX Session instances (where N equals CPU core count) and manage access through an index-based pool using a lock-free queue. Each concurrent request gets its own dedicated Session instance, enabling true parallel inference.

**Implementation location:** Rewrite the `RerankerModel` struct and its `load` and `inference` methods in `src/inference/model.rs`. Add `crossbeam` crate to `Cargo.toml` for the `ArrayQueue` data structure.

---

#### Issue 2: Missing Unit and Integration Tests

**Location:** No test files exist in the project

**Problem:** The `cargo test` milestone cannot pass because there are no tests. The project needs comprehensive test coverage for validation logic, tokenization, inference, and API endpoints.

**Solution:**

Create a library crate by adding `src/lib.rs` that re-exports all modules as public. This allows integration tests to import project types.

Create `tests/integration_test.rs` with the following test cases:

- Basic rerank request with query and multiple documents returns sorted results
- Empty query string returns validation error
- Empty documents list returns validation error
- More than 100 documents returns validation error
- Results are sorted by score in descending order
- Multiple concurrent requests all succeed without errors
- Relevance scores make semantic sense (ML-related docs score higher for ML queries than unrelated docs)

**Implementation locations:**

- Create new file `src/lib.rs`
- Create new directory `tests/` and file `tests/integration_test.rs`

---

#### Issue 3: Clippy Warnings from Unsafe Code

**Location:** `src/inference/tokenize.rs`

**Problem:** The file contains `unsafe impl Send for TokenizerWrapper` and `unsafe impl Sync for TokenizerWrapper`. This is unnecessary because the `tokenizers::Tokenizer` type from HuggingFace already implements Send and Sync. The unsafe impl blocks may trigger clippy warnings and represent potential unsoundness.

**Solution:** Remove both unsafe impl blocks from `src/inference/tokenize.rs`. The wrapper struct will automatically be Send and Sync because its inner Tokenizer field is Send and Sync.

**Implementation location:** Delete lines containing `unsafe impl Send` and `unsafe impl Sync` from `src/inference/tokenize.rs`.

---

#### Issue 4: Missing Model Warmup for Fast Startup

**Location:** `src/state.rs`

**Problem:** The first inference request after server start experiences cold-start latency because ONNX Runtime lazily initializes certain optimizations. While total startup may be under 5 seconds, the first request will have elevated latency.

**Solution:** Add a warmup method to `AppState` that runs a dummy inference immediately after loading the model. Call this warmup in the `AppState::new` constructor before returning. The warmup should tokenize a simple query-document pair and run inference, discarding the results.

**Implementation location:** Add a private `warmup` method to `impl AppState` in `src/state.rs` and call it at the end of `AppState::new`.

---

#### Issue 5: No Timeout on Inference Operations

**Location:** `src/handlers/rerank.rs`

**Problem:** The `spawn_blocking` call that runs tokenization and inference has no timeout. If the model hangs or takes too long, the request holds a semaphore permit indefinitely, eventually exhausting all permits and blocking the entire service.

**Solution:** Wrap the `spawn_blocking` future in `tokio::time::timeout` with a reasonable duration (30 seconds). If timeout occurs, return a `ResourceError` with an appropriate message.

**Implementation location:** Modify the `rerank_handler` function in `src/handlers/rerank.rs` to add timeout around the spawn_blocking call.

---

#### Issue 6: Readiness Probe Does Not Verify Model Health

**Location:** `src/handlers/health.rs`

**Problem:** The `/ready` endpoint always returns "ready" without actually checking if the model loaded successfully or can perform inference. Kubernetes would route traffic to a pod that may have a broken model.

**Solution:** The warmup in Issue 4 serves as implicit verification. If warmup fails during startup, the server will not start. Optionally, add an `AtomicBool` flag to `AppState` that is set to true only after successful warmup, and check this flag in `ready_handler`.

**Implementation location:** Optionally modify `src/state.rs` to add a ready flag and modify `src/handlers/health.rs` to check it.

---

## Required Dependency Changes

**Location:** `Cargo.toml`

Add these dependencies:

- `crossbeam` version 0.8 - for lock-free ArrayQueue used in session pool
- `parking_lot` version 0.12 - optional, provides faster Mutex if needed elsewhere

---

## File-by-File Implementation Checklist

### src/inference/model.rs

1. Remove the Mutex wrapper around Session
2. Change struct to hold a Vec of Session instances (the pool)
3. Add an ArrayQueue field to track available session indices
4. Modify `load` function to become `load_pool` that creates N sessions
5. Modify `inference` function to acquire a session index from the queue, use that session, then return the index to the queue
6. Add proper Send and Sync implementations with safety comments explaining the pool guarantees exclusive access

### src/inference/tokenize.rs

1. Remove the two unsafe impl lines for Send and Sync

### src/state.rs

1. Update `AppState::new` to call the new pool-based model loader
2. Add a private warmup method that runs dummy inference
3. Call warmup at the end of new before returning Ok
4. Optionally add an AtomicBool ready flag

### src/handlers/rerank.rs

1. Wrap the spawn_blocking call in tokio::time::timeout
2. Add timing metrics for inference duration and semaphore wait time
3. Ensure all metrics use the metrics crate macros consistently

### src/handlers/health.rs

1. Optionally check a ready flag from AppState in ready_handler
2. Accept State extractor if checking AppState

### src/lib.rs (NEW FILE)

1. Declare all modules as public
2. Re-export key types for use in integration tests

### tests/integration_test.rs (NEW FILE)

1. Create test helper function that constructs test AppState
2. Implement all test cases listed in Issue 2
3. Use tokio::test attribute for async tests
4. Tests should run with `--test-threads=1` to avoid resource conflicts

### Cargo.toml

1. Add crossbeam dependency
2. Optionally add parking_lot dependency
3. Verify all existing dependencies are correct

---

## Validation Commands

After implementation, run these commands in order:

1. `python python/export_model.py` - Creates the quantized model (run once)
2. `cargo build --release` - Must succeed with no errors
3. `cargo clippy -- -D warnings` - Must produce zero warnings
4. `cargo test -- --test-threads=1` - All tests must pass
5. `cargo run --release` - Server must start and log "Server listening" within 5 seconds
6. `curl http://localhost:8080/health` - Must return 200 with JSON containing "healthy"
7. `curl http://localhost:8080/metrics` - Must return Prometheus text format
8. Send POST to `/rerank` with query and documents - Must return sorted results

For performance validation, use a load testing tool like `oha` or `wrk`:

- Target: 200+ requests per second sustained
- P99 latency under 50ms
- Memory stays under 500MB during load test

---

## Performance Tuning Notes

If benchmarks are not met after implementing the session pool:

### If throughput is still below 200 req/s:

- Verify session pool size matches CPU core count
- Check that intra_threads is set to 1 for each session
- Profile with flamegraph to find unexpected bottlenecks
- Consider reducing max_sequence_length from 8192 to 512 for faster tokenization

### If P99 latency exceeds 50ms:

- Ensure batch sizes in requests are reasonable (under 20 documents)
- Check semaphore wait time metrics - if high, increase pool size
- Verify model is INT8 quantized, not FP32

### If memory exceeds 500MB:

- Verify using quantized model not original
- Reduce session pool size if necessary
- Check for memory leaks with valgrind or heaptrack

---

## Architecture Principles to Maintain

1. **No batching across requests** - Each request is processed independently for predictable latency
2. **Semaphore controls concurrency** - Never exceed CPU core count for concurrent inference
3. **Session pool enables parallelism** - One session per concurrent inference slot
4. **spawn_blocking for CPU work** - Never block the tokio runtime with inference
5. **Graceful degradation** - Return 503 when overloaded rather than queue indefinitely

---

## Summary of Changes Required

| Priority | File                      | Change                          |
| -------- | ------------------------- | ------------------------------- |
| CRITICAL | src/inference/model.rs    | Replace Mutex with session pool |
| CRITICAL | Cargo.toml                | Add crossbeam dependency        |
| HIGH     | tests/integration_test.rs | Create comprehensive test suite |
| HIGH     | src/lib.rs                | Create library crate for tests  |
| MEDIUM   | src/inference/tokenize.rs | Remove unsafe impl blocks       |
| MEDIUM   | src/state.rs              | Add model warmup                |
| MEDIUM   | src/handlers/rerank.rs    | Add inference timeout           |
| LOW      | src/handlers/health.rs    | Verify model in readiness check |

The session pool implementation is the single most important change. Without it, the service cannot achieve the throughput target regardless of other optimizations.
