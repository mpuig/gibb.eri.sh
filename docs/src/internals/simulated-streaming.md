# Simulated Streaming

Batch models are more accurate but have high latency. We decouple visual feedback from final transcription by running partial inference on growing audio buffers.

The "volatile" text shown during recording isn't a trick—it's a valid partial hypothesis based on audio heard so far. Human brains work similarly: we predict words before hearing them fully and revise as needed.

## The Problem

| Model Type | Accuracy | Latency | Feel |
|------------|----------|---------|------|
| Streaming | Good | ~50ms | Live, responsive |
| Batch | Excellent | ~2000ms | Sluggish, frustrating |

Users are impatient. A 2-second delay feels broken. But batch models are significantly more accurate, especially for:
- Proper nouns ("Kubernetes" vs "Cooper Netties")
- Rare words
- Accented speech

## The Solution

Run partial inference on the growing audio buffer every 500ms.

```
Time    Buffer              Display           State
─────────────────────────────────────────────────────
0ms     []                  (empty)           waiting
200ms   [audio...]          (empty)           buffering
500ms   [audio......]       "The quick"       volatile
1000ms  [audio..........]   "The quick brown" volatile
1200ms  (pause detected)    "The quick brown" processing
1400ms  (inference done)    "The quick brown fox." stable ✓
```

## Implementation

```rust
pub struct SimulatedStreamer {
    buffer: Vec<f32>,
    engine: Box<dyn SttEngine>,
    partial_interval: Duration,
    last_partial: Instant,
}

impl SimulatedStreamer {
    pub fn push_audio(&mut self, chunk: &[f32]) -> Option<PartialResult> {
        self.buffer.extend_from_slice(chunk);

        // Emit partial every 500ms
        if self.last_partial.elapsed() >= self.partial_interval {
            self.last_partial = Instant::now();

            let result = self.engine.transcribe(&self.buffer).ok()?;
            return Some(PartialResult {
                text: result.text,
                is_final: false,
            });
        }

        None
    }

    pub fn commit(&mut self) -> FinalResult {
        // Run final inference on complete buffer
        let result = self.engine.transcribe(&self.buffer).unwrap();

        // Clear for next utterance
        self.buffer.clear();

        FinalResult {
            text: result.text,
            is_final: true,
        }
    }
}
```

## UX: Volatile vs Stable Text

We visually distinguish draft from final:

```typescript
// Frontend
function TranscriptLine({ segment }: { segment: Segment }) {
    return (
        <span className={segment.is_final ? 'text-white' : 'text-gray-500'}>
            {segment.text}
        </span>
    );
}
```

- **Volatile (gray)**: Partial hypothesis, may be revised
- **Stable (white)**: Final transcription

## Edge Cases

### Partial Overwrites

Each partial replaces the previous:

```
Partial 1: "The quick"
Partial 2: "The quick brown"      // Replaces partial 1
Partial 3: "The quick brown fox"  // Replaces partial 2
Final:     "The quick brown fox." // Replaces partial 3
```

### Long Utterances

For very long speech (>30s), we chunk the buffer:

```rust
const MAX_BUFFER_SECONDS: usize = 30;

if self.buffer.len() > MAX_BUFFER_SECONDS * 16000 {
    // Force commit and start fresh
    self.commit();
}
```

### Rapid Corrections

If the user speaks, pauses briefly, then continues, we may commit prematurely. Smart Turn detection helps, but isn't perfect. We accept occasional mis-commits in exchange for responsiveness.

## Performance

| Metric | Pure Batch | Simulated Streaming |
|--------|------------|---------------------|
| Perceived latency | 2000ms | 500ms |
| Accuracy | Excellent | Excellent (same model) |
| CPU usage | Lower | Higher (repeated inference) |

The CPU trade-off is worth it for UX.

## When NOT to Use

Simulated streaming adds overhead. Skip it when:
- Processing pre-recorded files (no need for real-time feel)
- Running on low-end hardware (CPU budget matters)
- Accuracy is more important than speed (archival use case)
