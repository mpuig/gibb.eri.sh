# Smart Turn Detection

> Standard VAD detects *silence*. Smart Turn detects *completion*.

## The Problem

Voice Activity Detection (VAD) detects silence. Humans detect pauses.

We pause for many reasons:

- **Thinking**: "I want to... [pause] ...explain something"
- **Breathing**: Natural respiratory pauses
- **Emphasis**: "This is... [dramatic pause] ...important"
- **Completion**: "That's all I have to say."

Standard VAD treats all pauses the same. This leads to:
- Sentences being split mid-thought
- Awkward commit timing
- User frustration

## The Solution

We implement a **Neural Turn Detector** inspired by [Daily.co's VAD 3.1 research](https://www.daily.co/blog/smarter-voice-ai-with-the-new-daily-vad/).

Instead of just measuring silence, we analyze:
1. **Acoustic features**: Pitch contour, energy decay
2. **Timing**: Duration and pattern of the pause
3. **Semantic probability**: Is this a likely sentence ending?

## The Algorithm

```
if (Silence > 300ms AND Probability(EndOfSentence) > 0.5):
    Commit()
else:
    Wait()
```

### Components

| Component | Role |
|-----------|------|
| Silero VAD | Detects raw silence |
| Smart Turn Model | Predicts sentence completion |
| Redemption Timer | Grace period before commit |

## Implementation

The Smart Turn detector lives in `crates/smart-turn`:

```rust
pub struct SmartTurnV31Cpu {
    session: Mutex<Session>,  // ONNX Runtime session
    input_name: String,
    output_name: String,
}

impl TurnDetector for SmartTurnV31Cpu {
    fn predict_endpoint_probability(
        &self,
        audio_16k_mono: &[f32]
    ) -> Result<f32, TurnError> {
        // Returns probability 0.0-1.0 that speaker is done
    }
}
```

## Configuration

Users can tune the behavior via Settings:

### Redemption Time

The grace period after silence begins before we even consider committing.

| Setting | Value | Effect |
|---------|-------|--------|
| Fast | 200ms | Quick commits, may split sentences |
| Balanced | 300ms | Default, good for most users |
| Relaxed | 500ms | Waits longer, better for slow speakers |

### Sensitivity

How confident must we be that the sentence is complete?

| Setting | Threshold | Effect |
|---------|-----------|--------|
| Aggressive | 0.3 | Commits on weak signals |
| Normal | 0.5 | Balanced |
| Conservative | 0.7 | Only commits on strong endings |

## The Flow

```mermaid
graph TD
    A[Audio Input] --> B{VAD: Speech?}
    B -->|Yes| C[Buffer Audio]
    B -->|No| D{Silence > Redemption?}
    D -->|No| C
    D -->|Yes| E[Smart Turn Analysis]
    E --> F{P(End) > Threshold?}
    F -->|Yes| G[Commit Text]
    F -->|No| C
    G --> H[Reset State]
```

## Real-World Impact

Without Smart Turn:
```
User: "I think we should... [thinking pause]"
System: COMMIT → "I think we should"
User: "...consider the alternatives"
System: COMMIT → "consider the alternatives"
```

With Smart Turn:
```
User: "I think we should... [thinking pause] ...consider the alternatives"
System: (waiting, P(End) = 0.2)
System: (waiting, P(End) = 0.3)
User: [longer pause, falling intonation]
System: (P(End) = 0.7) COMMIT → "I think we should consider the alternatives"
```
