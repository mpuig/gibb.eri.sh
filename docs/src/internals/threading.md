# Threading Model

We use `std::thread` for inference, not `tokio::spawn`. Here's why.

## The Mistake We Made

Our first version looked like this:

```rust
// DON'T DO THIS
tokio::spawn(async move {
    loop {
        let chunk = rx.recv().await?;
        let result = engine.transcribe(&chunk.samples)?; // BLOCKS FOR 100ms!
        app.emit("stt:update", &result)?;
    }
});
```

This worked... until it didn't. Under load, the UI froze. Audio dropped. Everything felt sluggish.

## The Problem

ONNX Runtime inference is **CPU-bound and blocking**. A single `transcribe()` call might take 50-200ms of pure CPU work.

Tokio's async runtime assumes tasks yield frequently. When a task blocks for 100ms, it starves other tasks:

```
Task A: transcribe() ──────────────────────────────────▶ done
Task B: (waiting for audio)  ..........................  (finally runs)
Task C: (waiting for UI event) ........................  (finally runs)
                              ▲
                              100ms of nothing happening
```

The Tokio docs explicitly warn about this.

## The Fix

Move blocking work to dedicated OS threads:

```rust
// Dedicated thread for inference
std::thread::spawn(move || {
    loop {
        // Block here - it's fine, we're on our own thread
        let chunk = rx.blocking_recv().unwrap();
        let result = engine.transcribe(&chunk.samples).unwrap();

        // Send result back to async world
        result_tx.blocking_send(result).unwrap();
    }
});

// Async task just forwards results
tokio::spawn(async move {
    while let Some(result) = result_rx.recv().await {
        app.emit("stt:update", &result)?;
    }
});
```

## Thread Allocation

| Thread | Purpose | Priority |
|--------|---------|----------|
| Main | Tauri/UI event loop | Normal |
| Audio | cpal callback | High (OS-managed) |
| STT | ONNX inference | Normal |
| VAD | Silero inference | Normal |

We don't set thread priorities manually—the OS scheduler handles it well enough for our needs.

## Why Not `spawn_blocking`?

Tokio provides `spawn_blocking()` for blocking tasks:

```rust
tokio::task::spawn_blocking(move || {
    engine.transcribe(&samples)
}).await?
```

This works, but:

1. Creates a new thread per call (overhead)
2. Limited by `max_blocking_threads` (defaults to 512)
3. Threads are pooled but not reused predictably

For a continuous stream of inference calls, a dedicated thread is simpler and more predictable.

## Channel Selection

We need channels that bridge sync and async:

```rust
// Option 1: tokio::sync::mpsc (what we use)
let (tx, mut rx) = tokio::sync::mpsc::channel(100);
// tx.blocking_send() from sync thread
// rx.recv().await from async task

// Option 2: crossbeam + tokio wrapper
// More complex, no real benefit for our use case
```

## Memory Considerations

Each thread has its own stack (default 2MB on macOS). With 4 threads:
- Audio thread: ~2MB
- STT thread: ~2MB + model memory
- VAD thread: ~2MB + model memory
- Main thread: ~2MB

The model memory dominates. Thread stacks are negligible.

## Debugging

Thread bugs are subtle. Tools that help:

```bash
# See thread count
ps -M <pid>

# Profile with Instruments
xcrun xctrace record --template "Time Profiler" --launch ./gibberish

# Logging (add to Cargo.toml)
# tracing = "0.1"
# tracing-subscriber = "0.3"
```

## Error Handling

Threads don't propagate panics to the main thread. Handle errors explicitly:

```rust
std::thread::spawn(move || {
    let result = std::panic::catch_unwind(|| {
        // Inference loop
    });

    if let Err(e) = result {
        eprintln!("STT thread panicked: {:?}", e);
        // Notify main thread via channel
        error_tx.send(SttError::ThreadPanic).ok();
    }
});
```

## Code Reference

The actual implementation lives in `plugins/stt-worker/src/worker.rs`.
