# gibb.eri.sh // Local Real-Time AI

**Intelligence without the cloud.**

Speech-to-text for people who trust their CPU more than the cloud.
A local-first, zero-latency voice assistant engine built with **Rust** and **Tauri v2**.
Zero-copy audio pipeline. 100% Private.

## // Why Gibberish?

Most "real-time" transcription apps are just wrappers around cloud APIs. They leak your data and lag behind your voice.

Gibberish is different. It runs **entirely on your device**.
It uses a **Zero-Copy Audio Bus** to stream microphone data directly to local AI models, bypassing the slow JavaScript bridge entirely. The result is transcription that feels instant.

### ✨ Key Features

- **🔒 100% Private:** Your audio never leaves `localhost`. No servers, no tracking.
- **⚡️ Zero-Latency:** Words appear char-by-char as you speak (<200ms lag).
- **🧠 Smart Turn Detection:** Uses semantic analysis (inspired by **Daily.co VAD 3.1**) to know exactly when you've finished a sentence.
- **🤖 Agentic Tools:** Integrated with **FunctionGemma** to detect intents and execute tools (e.g., "Search Wikipedia for...") directly from your voice commands.
- **🎧 Hybrid Inference:** Choose your engine:
    - **Streaming (Sherpa):** For instant "Matrix-style" feedback.
    - **Batch (Parakeet/Whisper):** For maximum accuracy with VAD-triggered updates.

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
│   ├── diarization/      # Speaker diarization
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
