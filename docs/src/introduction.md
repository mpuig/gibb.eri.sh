# gibb.eri.sh

> **"Intelligence without the cloud."**

Technical documentation for gibb.eri.sh, a local-first voice assistant built with Rust and Tauri.

## What is gibb.eri.sh?

gibb.eri.sh is a desktop application that transcribes speech to text in real-time, entirely on your device. No internet connection required. No data leaves your machine.

### Features

- Voice processed locally, never uploaded
- ~45ms response time
- English, Catalan, and 99+ languages via Whisper
- Detects when you've finished speaking, not just when you stop
- Voice commands via local LLM ("Search Wikipedia for Rust")

### Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | Rust (native threads) |
| Frontend | React + TypeScript |
| Desktop | Tauri v2 (15MB binary) |
| Inference | ONNX Runtime + CoreML |
| Models | Sherpa-ONNX, Whisper, Parakeet |

## Hardware Requirements

Optimized for Apple Silicon with Apple Neural Engine (ANE) acceleration. Tested on M1/M2/M3 Macs.

The architecture is efficient enough for lower-end hardware with smaller models—swap the ~600MB Parakeet model for the ~100MB Sherpa Zipformer if CPU budget is tight.

| Hardware | Recommended Model | Notes |
|----------|-------------------|-------|
| M1+ Mac | Parakeet TDT 0.6B | Full accuracy, ANE accelerated |
| Intel Mac | Sherpa Zipformer | Lower accuracy, but runs smoothly |
| 8GB RAM | Either | Models are loaded on-demand |

## Who is this for?

- **Users** who want private, fast transcription
- **Developers** who want to understand how to build local-first AI
- **Contributors** who want to extend gibb.eri.sh

## Quick Links

- [Design Principles](./philosophy/index.html) — Why we built it this way
- [Features](./features/index.html) — What it can do
- [Architecture](./architecture/index.html) — How it works
- [Developer Guide](./guides/index.html) — How to extend it
