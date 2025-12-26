# Changelog

All notable changes to the **Gibberish** project will be documented in this file.

## [0.7.0] - The "Context-Aware Voice OS" Release
**Date:** 2025-12-26

A major architectural upgrade adding context-aware tool dispatch and Clean Architecture principles.

### 🚀 Features
- **Dynamic Tool Registry:** Tools filtered by mode *before* LLM inference. The manifest is built dynamically based on current context (Global, Dev, Meeting).
- **Mode-Specific Tools:**
  - **Global:** `wikipedia_city_lookup`, `system_control` (volume/mute/sleep), `app_launcher`
  - **Dev:** `git_voice` (async git commands), `file_finder` (Spotlight search)
  - **Meeting:** `add_todo` (Apple Reminders), `transcript_marker`
- **Mode Badge UI:** New frontend component shows current mode with pin/unpin support.
- **Context Polling:** Background thread detects active app and mic state to auto-switch modes.

### 🏗 Architecture (Clean Code)
- **SystemEnvironment Trait:** Abstracts OS calls for testability. Tools no longer directly call `std::process::Command`.
- **Shared Events Crate:** `gibberish-events` defines cross-plugin DTOs to prevent runtime deserialization errors.
- **ExecutionMode Enum:** Replaces boolean `auto_run` parameter to eliminate "Boolean Blindness".
- **Event Constants:** Magic strings replaced with constants (`event_names::CONTEXT_CHANGED`).
- **Guard Clauses:** Flattened nested conditionals in router for readability.
- **Helper Functions:** Extracted `find_best_proposal()`, `is_tool_available_in_mode()` to reduce cognitive load.

### ⚡️ Performance
- **Async I/O:** `GitVoiceTool` now uses `tokio::process::Command` instead of blocking `std::process::Command`.
- **No Blocking on Async Runtime:** All subprocess calls are non-blocking.

### 🔒 Security
- **cwd Safety Check:** `GitVoiceTool` validates that working directory is under `$HOME` to prevent path traversal.

---

## [0.6.0] - The "Less is More" Release
**Date:** 2025-12-25

A complete UI overhaul focused on simplicity and usability.

### 🎨 UI/UX
- **Single-Screen Design:** Replaced tabbed navigation with a focused transcript view and bottom toolbar.
- **Settings Sheet:** Model management and turn detection controls in a slide-up modal.
- **Sessions Sheet:** Browse and search past transcripts with a clean list view.
- **Simplified Onboarding:** Auto-downloads recommended model (whisper-onnx-small) on first launch.

### 🧹 Cleanup
- **Model Pruning:** Removed Whisper Tiny, Base, and ONNX Tiny/Base variants to reduce choice overload.
- **Opinionated Defaults:** Single recommended speech model for new users.

### 🐛 Bug Fixes
- **NeMo CTC Export:** Added `normalize_type: per_feature` metadata for sherpa-onnx compatibility.

---

## [0.5.0] - The "Intelligent Agent" Release
**Date:** 2025-12-25

The app transcends simple transcription. It now understands intent and executes tools locally.

### 🚀 Features
- **Tools Plugin:** Introduced `plugins/tools`.
- **FunctionGemma Integration:** Local LLM inference runs on committed transcripts to detect user intent.
- **Event-Driven Router:** Decoupled architecture where the Tools plugin listens for `stt:stream_commit` events to trigger actions.
- **Wikipedia Tool:** Added a built-in tool for testing "Search for X" commands.

### ⚡️ Performance
- **Debounce Logic:** Added smart debouncing to the Router to prevent LLM overload during rapid speech.

---

## [0.4.1] - Stability Fixes
**Date:** 2025-12-20

Addressed critical stability issues discovered during stress testing.

### 🐛 Bug Fixes
- **Fix "Disconnected Metrics":** Resolved a bug where the UI showed 0ms latency because the background worker was updating a *copy* of the metrics state. Now sharing `Arc<PipelineStatus>`.
- **Fix "Use-After-Free" Crash:** Fixed a segfault in Sherpa when switching models. Implemented `Arc<RecognizerHandle>` to ensure the C++ pointer outlives the worker thread.

---

## [0.4.0] - Hybrid Inference Engine
**Date:** 2025-12-15

Users can now choose between "Instant Speed" and "Maximum Accuracy".

### 🚀 Features
- **Batch Model Support:** Added `crates/parakeet` and `crates/whisper` integration.
- **Simulated Streaming:** Implemented a VAD-Triggered Batching system.
    - Buffers audio during speech.
    - Runs partial inference every 500ms for "volatile" feedback.
    - Commits final text when Silero VAD detects silence.
- **Model Selector:** Added UI dropdown to switch between Sherpa (Streaming) and Parakeet (Batch) at runtime.

---

## [0.3.0] - The Zero-Copy Audio Bus
**Date:** 2025-12-10

A massive architectural rewrite to solve latency and GC jitter.

### ⚡️ Performance
- **Rust-Native Audio Bus:** Created `crates/bus`. Audio data now bypasses the JavaScript bridge entirely.
- **Zero-Copy Architecture:** Implemented `Arc<[f32]>` audio chunks. A single memory allocation at the microphone is shared across all threads (VAD, STT, Viz).
- **Atomic Observability:** Replaced Mutex-based metrics with `std::sync::atomic` types to remove all locking from the audio hot path.
- **Dedicated Inference Thread:** Moved `SherpaWorker` to a `std::thread` (off the Tokio runtime) to prevent CPU-heavy inference from blocking I/O.

### 💥 Breaking Changes
- Removed `recorder:audio-chunk` event payload (UI no longer receives raw audio).

---

## [0.2.0] - Smart Endpointing
**Date:** 2025-12-01

Improving the "feel" of transcription.

### 🚀 Features
- **Smart Turn Detection:** Integrated a lightweight classifier (`crates/smart-turn`) to verify if a sentence is semantically complete before committing.
- **Silence Injection:** Explicitly feeding silence frames to the decoder on VAD speech-end to reset internal state and prevent hallucinations.
- **Latency Profiles:** Added configurable endpointing settings ("Fast", "Balanced", "Accurate").

---

## [0.1.0] - Modular Monolith
**Date:** 2025-11-20

Refactored the initial prototype into a scalable workspace.

### 🏗 Architecture
- **Workspace Split:** Separated domain logic into `crates/` (audio, stt, vad) and application glue into `plugins/` (recorder, stt-worker).
- **VAD Integration:** Added `crates/vad` wrapping `silero-vad`.
- **Streaming Transcriber:** Implemented the core state machine for handling partial vs. committed text.

---

## [0.0.1] - Initial Prototype
**Date:** 2025-11-01

Proof of concept. It works, but it's heavy.

### 🚀 Features
- **Basic Recording:** Captures microphone audio using `cpal`.
- **Sherpa-ONNX:** Basic integration of streaming Zipformer model.
- **Naive Streaming:** Sends audio chunks to Frontend via events -> Frontend calls Backend command to transcribe. (Note: Highly inefficient, fixed in v0.3.0).
