# The Golden Path

We hold three core beliefs that drive every line of code in gibb.eri.sh. These aren't just preferences—they're non-negotiables that shape every architectural decision.

1. **[Privacy First](./privacy.md)** — Your voice never leaves localhost
2. **[Zero Latency](./latency.md)** — Transcription must feel instant
3. **[Rust + Tauri](./stack.md)** — We build for the metal, not the browser

These principles sometimes conflict with "easier" solutions. We choose the harder path because the result is worth it: **AI for your OS that serves you, not a corporation.**

## The Trade-offs We Accept

| We Sacrifice | We Gain |
|--------------|---------|
| Cloud scalability | Absolute privacy |
| Development speed | Runtime performance |
| Framework convenience | Memory efficiency |
| Model variety | Predictable latency |

## The Trade-offs We Reject

- **"Just use OpenAI"** — Privacy is not optional
- **"Electron is fine"** — RAM is not free
- **"Good enough latency"** — 500ms feels broken

Read on to understand why each principle matters and how we implement it.