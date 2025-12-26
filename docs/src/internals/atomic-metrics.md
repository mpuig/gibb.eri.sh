# Lock-Free Metrics

The audio pipeline updates metrics frequently. Using mutexes would cause contention between the audio thread and UI thread, so we use atomic types instead.

## Data Structure

```rust
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

pub struct PipelineStatus {
    pub audio_lag_ms: AtomicI64,
    pub inference_time_ms: AtomicU64,
    pub dropped_chunks: AtomicU64,
    pub total_chunks: AtomicU64,
}
```

Atomic operations compile to single CPU instructions and don't block.

## Implementation

### Writing (Audio Thread)

```rust
impl PipelineStatus {
    pub fn update_lag(&self, lag_ms: i64) {
        self.audio_lag_ms.store(lag_ms, Ordering::Relaxed);
    }

    pub fn record_inference(&self, time_ms: u64) {
        self.inference_time_ms.store(time_ms, Ordering::Relaxed);
    }

    pub fn increment_dropped(&self) {
        self.dropped_chunks.fetch_add(1, Ordering::Relaxed);
    }
}
```

### Reading (UI Thread)

```rust
impl PipelineStatus {
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            audio_lag_ms: self.audio_lag_ms.load(Ordering::Relaxed),
            inference_time_ms: self.inference_time_ms.load(Ordering::Relaxed),
            dropped_chunks: self.dropped_chunks.load(Ordering::Relaxed),
            total_chunks: self.total_chunks.load(Ordering::Relaxed),
        }
    }
}
```

## Memory Ordering

We use `Ordering::Relaxed` because:
1. We don't need synchronization between different metrics
2. We only care about "eventually consistent" values
3. It's the fastest ordering

For metrics dashboards, slightly stale data is acceptable.

## Sharing Across Threads

```rust
use std::sync::Arc;

// Create shared status
let status = Arc::new(PipelineStatus::default());

// Clone for audio thread
let audio_status = Arc::clone(&status);
std::thread::spawn(move || {
    loop {
        // Update metrics without blocking
        audio_status.update_lag(compute_lag());
    }
});

// Clone for UI polling
let ui_status = Arc::clone(&status);
tokio::spawn(async move {
    loop {
        let snapshot = ui_status.snapshot();
        emit_metrics(&snapshot);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
});
```

## What We Track

| Metric | Type | Meaning |
|--------|------|---------|
| `audio_lag_ms` | i64 | Time since audio was captured |
| `inference_time_ms` | u64 | Last model execution time |
| `dropped_chunks` | u64 | Backpressure indicator |
| `total_chunks` | u64 | For calculating drop rate |

## Derived Metrics

```rust
impl MetricsSnapshot {
    pub fn drop_rate(&self) -> f64 {
        if self.total_chunks == 0 {
            0.0
        } else {
            self.dropped_chunks as f64 / self.total_chunks as f64
        }
    }

    pub fn real_time_factor(&self) -> f64 {
        // RTF < 1.0 means faster than real-time
        self.inference_time_ms as f64 / 1000.0 / CHUNK_DURATION_SECONDS
    }
}
```

## UI Display

```typescript
function MetricsDisplay() {
    const [metrics, setMetrics] = useState<Metrics | null>(null);

    useEffect(() => {
        const unlisten = listen<Metrics>('metrics:update', (event) => {
            setMetrics(event.payload);
        });
        return () => { unlisten.then(f => f()); };
    }, []);

    if (!metrics) return null;

    return (
        <div className="text-xs text-gray-500">
            Latency: {metrics.audio_lag_ms}ms |
            RTF: {metrics.real_time_factor.toFixed(2)} |
            Drops: {(metrics.drop_rate * 100).toFixed(1)}%
        </div>
    );
}
```

## Debugging Tip

When logging metrics, take a snapshot first rather than loading individual atomics separately:

```rust
let snap = status.snapshot();
debug!("Metrics: {:?}", snap);
```
