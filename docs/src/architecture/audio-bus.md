# Audio Bus

The audio bus distributes microphone data to multiple consumers (VAD, STT, visualizer) using shared memory.

## Why Shared Memory?

At 16kHz mono, audio is only ~32KB/sec—not "big data." The issue isn't throughput, it's *latency consistency*. Without shared memory, audio gets copied at each boundary (Mic → JS → Rust → Model → UI), and each copy can introduce jitter. Unpredictable delays destroy the real-time feel even if average latency is low.

Using `Arc<[f32]>` means one allocation, shared by all consumers. No copying, no jitter from allocations.

## Design

Audio is allocated once and shared via `Arc<[f32]>`:

```
Mic → Recorder → Arc<[f32]> ─┬─▶ VAD
                             ├─▶ STT
                             └─▶ Visualizer
```

All consumers read the same memory.

## Implementation

### AudioChunk

```rust
pub struct AudioChunk {
    pub seq: u64,            // Monotonic sequence number
    pub ts_ms: i64,          // Capture timestamp
    pub sample_rate: u32,    // Always 16000 Hz
    pub samples: Arc<[f32]>, // The actual audio data
}
```

`Arc<[f32]>` is an atomically reference-counted slice. Memory is freed when the last consumer drops its reference.

### AudioBus

```rust
pub struct AudioBus {
    tx: mpsc::Sender<AudioChunk>,
    config: BusConfig,
}

impl AudioBus {
    pub fn publish(&self, chunk: AudioChunk) -> Result<()> {
        self.tx.send(chunk)?;
        Ok(())
    }
}
```

### Listener

```rust
pub struct Listener {
    rx: mpsc::Receiver<AudioChunk>,
    dropped: Arc<AtomicU64>,
}

impl Listener {
    pub async fn recv(&mut self) -> Option<AudioChunk> {
        self.rx.recv().await
    }

    pub fn drain_to_latest(&mut self) -> Option<AudioChunk> {
        // Skip old chunks, return only the newest
        let mut latest = None;
        while let Ok(chunk) = self.rx.try_recv() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            latest = Some(chunk);
        }
        latest
    }
}
```

## Backpressure

What if STT can't keep up with audio? Options:

1. **Block**: Producer waits for consumer (bad: causes audio drops)
2. **Buffer**: Queue grows unbounded (bad: uses memory, increases latency)
3. **Drop**: Discard old data, keep real-time (good: for live transcription)

We use **bounded channels** with drop policy:

```rust
let (tx, rx) = mpsc::channel(BUFFER_SIZE); // e.g., 100 chunks

// If buffer is full, oldest chunks are available to drain
```

The `drain_to_latest()` method lets slow consumers catch up by skipping to the newest audio.

## Pipeline Status

Performance metrics are tracked with atomic counters:

```rust
pub struct PipelineStatus {
    audio_lag_ms: AtomicI64,      // How far behind real-time
    inference_time_ms: AtomicU64, // Last model execution time
    dropped_chunks: AtomicU64,    // Backpressure indicator
}
```

## Diagram

```mermaid
graph LR
    Mic[Microphone] -->|Raw Samples| Recorder
    Recorder -->|Arc&lt;[f32]&gt;| Bus[MPSC Channel]
    Bus -->|recv| VAD[Silero VAD]
    Bus -->|recv| STT[STT Engine]
    STT -->|Text Event| UI[Frontend]
```
