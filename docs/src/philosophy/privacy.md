# Privacy First

> **Your voice never leaves `localhost`.**

No OpenAI. No Google Speech API. No AWS Transcribe. No cloud anything.

If data doesn't leave the device, it cannot be intercepted, stored, or analyzed by third parties.

## Implementation

All models run on-device using:

- ONNX Runtime (cross-platform inference)
- Quantized int8 models
- CoreML backend on macOS (Apple Neural Engine)

```
┌─────────────────────────────────────────┐
│              Your Device                │
│  ┌─────────┐    ┌─────────┐    ┌─────┐ │
│  │   Mic   │───▶│  Model  │───▶│ Text│ │
│  └─────────┘    └─────────┘    └─────┘ │
│                                         │
│         Everything stays here           │
└─────────────────────────────────────────┘
              │
              ╳  No network calls
              │
```

## Trade-off

Users download ~500MB of model weights upfront. In exchange:

- No API bills
- No network round-trips (lower latency)
- No data exfiltration possible
- Works offline

## Why Not Hybrid?

"What if we use local for drafts and cloud for final polish?"

No. This creates a false sense of privacy. Users think they're protected, but their data still leaves the device. We reject half-measures.

## The Models We Use

| Model | Size | Use Case |
|-------|------|----------|
| Sherpa Zipformer | ~100MB | Real-time streaming |
| Whisper Small | ~500MB | High-accuracy batch |
| Silero VAD | ~2MB | Voice activity detection |
| FunctionGemma | ~200MB | Intent recognition |

All models are open-source and can be audited.
