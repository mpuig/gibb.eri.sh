# Hybrid Inference Engine

gibb.eri.sh supports two modes of operation, selectable at runtime. Each has trade-offs.

## Streaming Mode (Sherpa-ONNX Zipformer)

**Best for:** Dictation, live captioning, instant feedback

```
Audio ─▶ [Transducer] ─▶ Partial results every ~50ms
```

### How It Works

The Zipformer model uses a **transducer** architecture:
- Processes audio in small chunks (10-20ms)
- Maintains internal state between chunks
- Emits partial hypotheses continuously
- Refines predictions as context grows

### Characteristics

| Aspect | Value |
|--------|-------|
| Latency | ~50ms per update |
| Accuracy | Good (may miss proper nouns) |
| Languages | English (primary) |
| Model Size | ~100MB |

### When Streaming Struggles

- **Proper nouns**: "Kubernetes" might become "Cooper Netties"
- **Rare words**: Technical jargon may be misheard
- **Accents**: Less training data for non-standard speech

## Batch Mode (Parakeet / Whisper)

**Best for:** Meetings, archival, accuracy-critical tasks

```
Audio ─▶ [VAD Buffer] ─▶ [Encoder-Decoder] ─▶ Final text on pause
```

### How It Works

Batch models see the **entire utterance** before producing output:
- VAD detects speech boundaries
- Audio is buffered during speech
- Model processes the complete segment
- Result is highly accurate

### Characteristics

| Aspect | Value |
|--------|-------|
| Latency | ~500ms after speech ends |
| Accuracy | Excellent |
| Languages | 99+ (Whisper) |
| Model Size | ~500MB |

### Simulated Streaming

Users want batch accuracy with streaming feel. We fake it:

1. Run partial inference every 500ms on the growing buffer
2. Display "volatile" text (gray, may change)
3. On VAD trigger, run final inference
4. Replace volatile text with "stable" text (black, final)

```
Speaking: "The quick brown fox"

Time 0ms:   [           ] (buffering)
Time 500ms: [The quick  ] volatile
Time 1000ms:[The quick brown] volatile
Time 1200ms: (pause detected)
Time 1400ms:[The quick brown fox.] stable ✓
```

## Switching Modes

Users can switch modes at runtime via the Settings sheet:

```typescript
// Frontend
await invoke('plugin:stt|set_mode', { mode: 'streaming' });
await invoke('plugin:stt|set_mode', { mode: 'batch' });
```

The backend handles the transition gracefully, draining any buffered audio.

## Model Recommendations

We've tested many models. Here are our picks:

| Use Case | Recommended Model |
|----------|-------------------|
| General dictation | Sherpa Zipformer (streaming) |
| Meetings | Whisper Small (batch) |
| Non-English | Whisper Small (batch) |
| Low-end hardware | Sherpa Zipformer (streaming) |
