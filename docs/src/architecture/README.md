# System Architecture

gibb.eri.sh is organized as a **Modular Monolith**—a single binary with strictly decoupled internal components.

## Why Modular Monolith?

| Architecture | Pros | Cons |
|--------------|------|------|
| Monolith | Simple deployment, shared memory | Tight coupling, hard to test |
| Microservices | Independent scaling, isolation | Network overhead, complexity |
| **Modular Monolith** | Best of both | Requires discipline |

Performance of a monolith. Maintainability of services.

## The Two Layers

```
┌─────────────────────────────────────────────────────────┐
│                     Tauri App                            │
│  ┌─────────────────────────────────────────────────────┐│
│  │                  plugins/                            ││
│  │  Adapters: Translate between crates and Tauri IPC   ││
│  │  • recorder/  • stt-worker/  • tools/               ││
│  └─────────────────────────────────────────────────────┘│
│                         │                                │
│                         ▼                                │
│  ┌─────────────────────────────────────────────────────┐│
│  │                   crates/                            ││
│  │  Pure Rust: Zero dependencies on Tauri or UI        ││
│  │  • audio/  • bus/  • stt/  • vad/  • sherpa/       ││
│  └─────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
```

### `crates/` — The Engine

Pure Rust libraries with **no UI dependencies**:
- Can be compiled to CLI tools
- Can be wrapped with FFI for iOS/Android
- Fully unit-testable

### `plugins/` — The Adapters

Tauri-specific glue code:
- Exposes crate functionality as Tauri commands
- Handles IPC serialization
- Manages permissions

## Key Design Patterns

### Dependency Inversion

High-level modules don't depend on low-level modules. Both depend on abstractions.

```rust
// crates/application doesn't know about Sherpa or Parakeet
// It only knows about the SttEngine trait
pub fn transcribe(engine: &dyn SttEngine, audio: &[f32]) -> Vec<Segment> {
    engine.transcribe(audio)
}
```

### Strategy Pattern

Swap implementations at runtime without changing calling code.

```rust
let engine: Box<dyn SttEngine> = match config.mode {
    Mode::Streaming => Box::new(SherpaEngine::new()?),
    Mode::Batch => Box::new(ParakeetEngine::new()?),
};
```

### Event-Driven Communication

Components communicate via events, not direct calls.

```rust
// Producer (STT Worker)
app.emit("stt:stream_commit", &segment)?;

// Consumer (Tools Plugin) - doesn't know about STT internals
app.listen("stt:stream_commit", |event| { ... });
```

## Deep Dives

- **[Crate Structure](./crates.md)** — What each crate does
- **[Audio Bus](./audio-bus.md)** — How audio is distributed to consumers
- **[Event System](./events.md)** — How components communicate
