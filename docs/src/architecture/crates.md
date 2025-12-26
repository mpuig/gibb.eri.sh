# Crate Structure

The `crates/` directory contains all domain logic. Each crate has a single responsibility.

## Overview

```
crates/
├── application/     # Orchestration & State Machine
├── audio/           # Capture, AGC, Resampling
├── bus/             # Audio Pipeline
├── detect/          # Meeting App Detection
├── diarization/     # Speaker ID (Traits)
├── models/          # Model Registry & Downloads
├── parakeet/        # NVIDIA Parakeet Backend
├── sherpa/          # Sherpa-ONNX Backend
├── smart-turn/      # Semantic Endpointing
├── storage/         # SQLite Persistence
├── stt/             # Engine Traits & Abstractions
├── transcript/      # Data Structures
├── turn/            # Turn Detection Traits
└── vad/             # Silero VAD Integration
```

## Core Crates

### `gibberish-bus`

Delivers audio from recorder to consumers (VAD, STT, visualizer).

```rust
pub struct AudioChunk {
    pub seq: u64,           // Sequence number
    pub ts_ms: i64,         // Timestamp
    pub sample_rate: u32,   // Always 16000
    pub samples: Arc<[f32]>, // Shared audio data
}

pub struct AudioBus {
    tx: mpsc::Sender<AudioChunk>,
    // ...
}
```

**Key feature**: `Arc<[f32]>` means audio is allocated once and shared across all consumers.

### `gibberish-stt`

Defines the `SttEngine` trait that all backends implement:

```rust
pub trait SttEngine: Send + Sync {
    fn transcribe(&self, audio: &[f32]) -> Result<Vec<Segment>>;
    fn is_streaming_capable(&self) -> bool;
    fn model_name(&self) -> &str;
    fn supported_languages(&self) -> Vec<&'static str>;
}
```

### `gibberish-audio`

Handles microphone capture and preprocessing:

- **Capture**: Uses `cpal` for cross-platform audio
- **Resampling**: `rubato` for high-quality sample rate conversion
- **AGC**: Automatic gain control for consistent levels

### `gibberish-vad`

Wraps Silero VAD for voice activity detection:

```rust
pub trait VoiceActivityDetector: Send + Sync {
    fn is_speech(&mut self, audio: &[f32]) -> Result<bool>;
    fn reset(&mut self);
}
```

## Backend Crates

### `gibberish-sherpa`

Integrates [Sherpa-ONNX](https://github.com/k2-fsa/sherpa-onnx) for streaming transcription:

- Zipformer transducer (English)
- Whisper encoder-decoder (multilingual)
- NeMo CTC (Catalan, other languages)

### `gibberish-parakeet`

Integrates NVIDIA's Parakeet TDT model for high-accuracy batch transcription.

## Support Crates

### `gibberish-models`

Registry of available models with download URLs:

```rust
pub struct ModelMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub language: &'static str,
    pub url: &'static str,
    pub model_type: ModelType,
}
```

### `gibberish-storage`

SQLite-based session persistence:

```rust
pub trait SessionStorage {
    fn save_session(&self, session: &Session) -> Result<()>;
    fn load_session(&self, id: &str) -> Result<Option<Session>>;
    fn list_sessions(&self) -> Result<Vec<SessionSummary>>;
}
```

### `gibberish-transcript`

Data structures for transcription results:

```rust
pub struct Segment {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub is_final: bool,
}
```

## Dependency Graph

```
application
    ├── bus
    ├── stt (trait only)
    ├── vad (trait only)
    └── turn (trait only)

sherpa
    └── stt (implements SttEngine)

parakeet
    └── stt (implements SttEngine)

smart-turn
    └── turn (implements TurnDetector)
```

The `application` crate never imports `sherpa` or `parakeet` directly—only their traits.
