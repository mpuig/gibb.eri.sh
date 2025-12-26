# Core Features

gibb.eri.sh isn't just a transcription tool—it's an intelligent voice interface.

## Feature Overview

### [Hybrid Inference Engine](./hybrid-inference.md)

Choose your trade-off: instant feedback or maximum accuracy.

- **Streaming Mode**: Words appear in real-time (~50ms updates)
- **Batch Mode**: Higher accuracy, processed on pauses

### [Smart Turn Detection](./smart-turn.md)

Standard voice detection only hears silence. gibb.eri.sh hears *completion*.

- Knows when you're thinking vs. when you're done
- Uses neural analysis, not just timers
- Configurable sensitivity profiles

### [Agentic Tools](./agentic-tools.md)

A local LLM understands your intent and executes actions.

- "Search Wikipedia for Rust" → Opens browser with results
- Runs entirely offline
- Extensible tool system

## Feature Matrix

| Feature | Streaming | Batch | Notes |
|---------|-----------|-------|-------|
| Real-time display | ✓ | Simulated | Batch shows "draft" text |
| Accuracy | Good | Excellent | Batch wins on proper nouns |
| Latency | ~50ms | ~500ms | Per-update latency |
| Languages | English | 99+ | Whisper supports many |
| Smart Turn | ✓ | ✓ | Works with both modes |
| Agentic | ✓ | ✓ | Triggers on commit |

## Coming Soon

- **Speaker Diarization** — "Who said what?"
- **Punctuation Restoration** — Automatic commas and periods
- **Custom Wake Words** — "Hey gibb.eri.sh"