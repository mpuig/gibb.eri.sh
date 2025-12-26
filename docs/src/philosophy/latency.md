# Low Latency

Time-to-first-token should be < 200ms. Delays above this threshold are noticeable and disrupt the feedback loop.

## The Problem with Traditional Architectures

Most speech-to-text systems work like this:

```
Mic → JavaScript → JSON → HTTP → Server → Model → HTTP → JSON → UI
      ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
                        500-2000ms latency
```

Every boundary crossing adds latency:
- JS ↔ Native: Serialization overhead
- HTTP: Network round-trip
- JSON: Parsing overhead

## Our Implementation

Audio stays in Rust and uses shared memory pointers:

```
Mic → Rust → Arc<[f32]> → Model → Text → UI
      ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
            45ms latency
```

Key techniques:
- `Arc<[f32]>`: Shared memory pointers
- Bounded MPSC channels for backpressure
- Dedicated threads for inference

## Measuring Latency

We track latency at every stage using atomic counters:

```rust
pub struct PipelineStatus {
    audio_lag_ms: AtomicI64,      // Time since audio was captured
    inference_time_ms: AtomicU64, // Model execution time
    dropped_chunks: AtomicU64,    // Backpressure indicator
}
```

These metrics are lock-free—measuring latency doesn't add latency.

## The Streaming Advantage

Traditional "batch" transcription waits for you to finish speaking, then processes everything at once. You might wait 2-3 seconds for results.

**Streaming** transcription processes audio continuously:

| Time | Batch Model | Streaming Model |
|------|-------------|-----------------|
| 0ms | (waiting) | (waiting) |
| 100ms | (waiting) | "The" |
| 200ms | (waiting) | "The quick" |
| 500ms | (waiting) | "The quick brown" |
| 1000ms | (waiting) | "The quick brown fox" |
| 1500ms | "The quick brown fox" | "The quick brown fox jumps" |

Streaming provides immediate visual feedback.
