# Crate Structure

The `crates/` directory contains the domain logic. Each crate has a single responsibility and zero dependencies on Tauri or the UI.

## Overview

```
crates/
├── application/     # Orchestration & State Machine
├── audio/           # Capture, AGC, Resampling
├── bus/             # Zero-copy Audio Pipeline
├── context/         # OS Awareness (Active App, Mic State)
├── detect/          # Meeting App Logic
├── events/          # Shared Event Contracts (DTOs)
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

## Core Components

### bus
The nervous system. Delivers audio from recorder to consumers.
**Key feature**: Uses `Arc<[f32]>` so audio is allocated once and shared across all consumers.

### context
The senses. Aggregates system state to drive the context engine.
- **Active App**: Which window has focus?
- **Mic State**: Is a meeting app using the hardware?
- **Mode**: Derives intent (Dev, Meeting, Global).

### stt
Defines the `SttEngine` trait. Infrastructure crates (`sherpa`, `parakeet`) implement this.

### audio
Handles microphone capture and preprocessing:
- **Resampling**: `rubato` for high-quality sample rate conversion.
- **AGC**: Automatic gain control with soft-clipping.

### vad
Wraps Silero VAD for voice activity detection.

---

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
```

The `application` crate never imports `sherpa` or `parakeet` directly—only their traits.