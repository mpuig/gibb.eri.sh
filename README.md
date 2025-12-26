# gibb.eri.sh

Local-first speech-to-text built with Rust and Tauri v2.

Runs entirely on your device. Audio never leaves localhost.

## Why?

Most transcription apps send your audio to cloud APIs. This one doesn't.

It uses a zero-copy audio bus to stream microphone data directly to local AI models, bypassing the JavaScript bridge.

## Features

- **Private**: Audio processed locally, never uploaded
- **Fast**: <200ms latency, words appear as you speak
- **Smart endpointing**: Neural analysis detects sentence completion (inspired by Daily.co VAD 3.1)
- **Voice commands**: Local LLM (FunctionGemma) executes intents like "Search Wikipedia for..."
- **Context-aware modes**: Tools filtered by mode (Global, Dev, Meeting) before LLM inference
- **Hybrid inference**: Streaming (Sherpa) for instant feedback, batch (Parakeet/Whisper) for accuracy

## Requirements

- macOS 13+ (Apple Silicon recommended)
- Rust 1.70+
- Node.js 18+

## Installation

```bash
# Clone the repository
git clone https://github.com/mpuig/gibb.eri.sh.git
cd gibb.eri.sh

# Install frontend dependencies
cd apps/desktop
npm install

# Build and run
npm run tauri dev
```

## Project Structure

```
gibb.eri.sh/
├── apps/
│   └── desktop/          # Tauri + React frontend
│       ├── src/          # React components, hooks, stores
│       └── src-tauri/    # Tauri app configuration
├── crates/
│   ├── audio/            # Audio capture and processing
│   ├── context/          # Context detection (active app, mode)
│   ├── diarization/      # Speaker diarization
│   ├── events/           # Shared event DTOs across plugins
│   ├── models/           # Model management and downloads
│   ├── parakeet/         # ONNX-based STT engine (Parakeet)
│   ├── sherpa/           # Sherpa-ONNX STT engine (Zipformer transducer)
│   ├── storage/          # SQLite persistence
│   ├── stt/              # STT engine traits and abstractions
│   ├── transcript/       # Transcript data structures
│   └── vad/              # Voice Activity Detection (Silero)
└── plugins/
    ├── permissions/      # macOS permission handling
    ├── recorder/         # Audio recording plugin
    ├── stt-worker/       # STT processing plugin
    ├── tools/            # Voice command tools and router
    └── tray/             # Menu bar integration
```

## Architecture

The app follows clean architecture principles:

- **Domain Layer** (`crates/`) - Core business logic and traits
- **Infrastructure Layer** (`crates/storage`, `crates/parakeet`) - Concrete implementations
- **Application Layer** (`plugins/`) - Tauri plugins bridging UI and domain
- **Presentation Layer** (`apps/desktop/src/`) - React UI components

Key design decisions:
- `SttEngine` trait allows swapping speech recognition backends
- `TranscriptRepository` trait decouples storage from domain
- Service layer in plugins separates business logic from Tauri commands
- Zustand stores for frontend state management

## Models

gibb.eri.sh uses NVIDIA Parakeet models via ONNX Runtime:

| Model | Size | Description |
|-------|------|-------------|
| Parakeet TDT 0.6B V2 | ~600MB | Fast, streaming-capable (Recommended) |
| Parakeet CTC 0.6B | ~600MB | Higher accuracy, batch processing |
| Parakeet TDT 1.1B | ~1.1GB | Best accuracy, requires more memory |
| Sherpa Zipformer (EN) | ~250MB | Low-latency streaming transducer (English) |
| **NeMo Conformer** | CTC | **Catalan** (Specialized) | ~500MB |

Models are downloaded on first use and cached in `~/Library/Application Support/gibberish/models/`.

## Usage

1. **First Launch** - Select and download a model in Settings
2. **Recording** - Click the record button or use the menu bar icon
3. **View Transcript** - Text appears in real-time during recording
4. **Browse Sessions** - Access past recordings in the Sessions tab
5. **Export** - Use the export menu to save as TXT, SRT, or JSON

## Development

```bash
# Run in development mode
cd apps/desktop
npm run tauri dev

# Check Rust compilation
cargo check -p notary-desktop

# Check TypeScript
npx tsc --noEmit

# Build for release
npm run tauri build
```

## Tech Stack

**Backend (Rust)**
- Tauri 2.0 - Desktop app framework
- ONNX Runtime - ML inference
- cpal - Cross-platform audio
- Silero VAD - Voice activity detection
- rusqlite - SQLite database

**Frontend (TypeScript)**
- React 19
- Zustand - State management
- Tailwind CSS - Styling
- Vite - Build tool

## License

MIT

## Acknowledgments

- [NVIDIA Parakeet](https://catalog.ngc.nvidia.com/orgs/nvidia/teams/nemo/models/parakeet-tdt-1.1b) - Speech recognition models
- [Silero VAD](https://github.com/snakers4/silero-vad) - Voice activity detection
- [Tauri](https://tauri.app/) - Desktop app framework
