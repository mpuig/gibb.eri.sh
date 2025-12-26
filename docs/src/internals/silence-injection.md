# Silence Injection

> The "clear throat" hack that prevents hallucinations.

## The Problem

Streaming decoders maintain internal state. When speech ends, this state can get "stuck" in a loop:

```
User says: "Hello world"
User stops: (silence)
Model outputs: "Hello world. Thank you. Thank you. Thank you..."
```

The model is hallucinating. It expects more input and fills the gap with plausible-sounding garbage.

## Why It Happens

Transducer models have a "joiner" network that predicts the next token based on:
1. Acoustic features (from audio)
2. Previous predictions (from decoder state)

During silence, acoustic features are near-zero, but the decoder state still has momentum from the previous words. The model "invents" continuations.

## The Solution

Explicitly feed silence into the decoder to reset its state:

```rust
const SILENCE_DURATION_MS: usize = 100;
const SILENCE_SAMPLES: usize = SILENCE_DURATION_MS * 16; // 16 samples/ms at 16kHz

pub fn inject_silence(&mut self) {
    let silence = vec![0.0f32; SILENCE_SAMPLES];
    self.recognizer.accept_waveform(&silence);

    // Force decoder to flush
    self.recognizer.input_finished();
}
```

## When to Inject

Trigger silence injection when:
1. VAD detects speech-end (transition from speech to silence)
2. A configurable grace period has passed (e.g., 300ms)
3. Before requesting final output

```rust
impl StreamingTranscriber {
    pub fn on_vad_speech_end(&mut self) {
        // Wait for Smart Turn confirmation
        if self.smart_turn.is_likely_complete() {
            self.inject_silence();
            let final_text = self.recognizer.get_result();
            self.emit_commit(final_text);
            self.reset_state();
        }
    }
}
```

## The "Digital Silence"

We inject zeros, not actual recorded silence. Why?

| Type | Contents | Effect |
|------|----------|--------|
| Recorded silence | Room noise, hum | Model might hear "words" in noise |
| Digital silence | Pure zeros | Unambiguous "nothing to hear" |

```rust
// Good: Pure digital silence
let silence = vec![0.0f32; 1600];

// Bad: Recorded silence (might contain noise)
let silence = record_ambient_audio(100);
```

## How Much Silence?

We experimentally tuned to 100ms:

| Duration | Effect |
|----------|--------|
| 50ms | Sometimes not enough to reset |
| 100ms | Reliable reset, minimal delay |
| 200ms | Works but adds unnecessary latency |

```rust
const SILENCE_MS: usize = 100;
```

## Interaction with Smart Turn

Silence injection happens *after* Smart Turn confirms completion:

```mermaid
graph TD
    A[VAD: Silence Detected] --> B{Smart Turn?}
    B -->|P(End) < 0.5| C[Keep Listening]
    B -->|P(End) >= 0.5| D[Inject Silence]
    D --> E[Get Final Result]
    E --> F[Emit Commit]
    F --> G[Reset State]
```

If we inject too early, we cut off the user mid-sentence.

## Code

```rust
// crates/sherpa/src/streaming.rs

impl StreamingRecognizer {
    pub fn end_utterance(&mut self) -> String {
        // Inject silence to flush decoder
        let silence = vec![0.0f32; 1600]; // 100ms
        self.accept_waveform(&silence);

        // Mark input as finished
        self.input_finished();

        // Get final result
        let result = self.final_result();

        // Reset for next utterance
        self.reset();

        result
    }
}
```

## Without Silence Injection

```
Input:  "The quick brown fox"
Output: "The quick brown fox jumps over the lazy dog thank you thank you"
                              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                              Hallucination
```

## With Silence Injection

```
Input:  "The quick brown fox"
Output: "The quick brown fox"
                              (clean end)
```

The difference is dramatic for user experience.
