# Headless Engine

The core transcription engine has **zero dependencies on Tauri or UI**. You can use it standalone.

## Why Headless?

- **CLI tools**: Build command-line transcription utilities
- **Server applications**: Run transcription as a service
- **Mobile apps**: Wrap with FFI for iOS/Android
- **Testing**: Unit test without UI overhead

## Architecture

```
┌─────────────────────────────────────────┐
│           Your Application              │
│  ┌───────────────────────────────────┐  │
│  │     gibberish-application         │  │
│  │  (Orchestration & State Machine)  │  │
│  └───────────────────────────────────┘  │
│                   │                     │
│     ┌─────────────┼─────────────┐      │
│     ▼             ▼             ▼      │
│  ┌──────┐    ┌─────────┐   ┌───────┐  │
│  │ bus  │    │   stt   │   │  vad  │  │
│  └──────┘    └─────────┘   └───────┘  │
└─────────────────────────────────────────┘
         (No Tauri, No React)
```

## Example: CLI Transcriber

Here's a minimal CLI that transcribes a WAV file:

```rust
// examples/cli_transcribe.rs

use gibberish_audio::load_wav;
use gibberish_sherpa::WhisperEngine;
use gibberish_stt::SttEngine;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let wav_path = args.get(1).expect("Usage: cli_transcribe <file.wav>");

    // Load audio
    let audio = load_wav(wav_path)?;

    // Initialize engine
    let engine = WhisperEngine::new("path/to/whisper-small")?;

    // Transcribe
    let segments = engine.transcribe(&audio)?;

    // Print results
    for segment in segments {
        println!("[{:.2}s - {:.2}s] {}",
            segment.start_ms as f64 / 1000.0,
            segment.end_ms as f64 / 1000.0,
            segment.text
        );
    }

    Ok(())
}
```

Run it:

```bash
cargo run --example cli_transcribe recording.wav
```

## Example: Real-Time Streaming

```rust
use gibberish_audio::{AudioCapture, AudioConfig};
use gibberish_bus::{AudioBus, AudioChunk};
use gibberish_sherpa::ZipformerEngine;
use gibberish_vad::SileroVad;

fn main() -> anyhow::Result<()> {
    // Set up audio capture
    let config = AudioConfig {
        sample_rate: 16000,
        channels: 1,
    };
    let capture = AudioCapture::new(config)?;

    // Set up bus
    let (bus, mut listener) = AudioBus::new(100);

    // Set up VAD and STT
    let mut vad = SileroVad::new()?;
    let engine = ZipformerEngine::new("path/to/zipformer")?;

    // Start capture
    capture.start(move |samples| {
        let chunk = AudioChunk::new(samples);
        let _ = bus.publish(chunk);
    })?;

    // Processing loop
    loop {
        if let Some(chunk) = listener.recv().await {
            if vad.is_speech(&chunk.samples)? {
                let result = engine.transcribe_streaming(&chunk.samples)?;
                if !result.text.is_empty() {
                    print!("{}", result.text);
                }
            }
        }
    }
}
```

## FFI: Using from Swift/Kotlin

For mobile apps, expose a C-compatible interface:

### Rust Side

```rust
// src/ffi.rs

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn gibberish_init(model_path: *const c_char) -> *mut Engine {
    let path = unsafe { CStr::from_ptr(model_path) }.to_str().unwrap();
    let engine = Box::new(Engine::new(path).unwrap());
    Box::into_raw(engine)
}

#[no_mangle]
pub extern "C" fn gibberish_transcribe(
    engine: *mut Engine,
    audio: *const f32,
    length: usize,
) -> *mut c_char {
    let engine = unsafe { &*engine };
    let samples = unsafe { std::slice::from_raw_parts(audio, length) };

    let result = engine.transcribe(samples).unwrap();
    CString::new(result.text).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn gibberish_free(engine: *mut Engine) {
    unsafe { drop(Box::from_raw(engine)); }
}

#[no_mangle]
pub extern "C" fn gibberish_free_string(s: *mut c_char) {
    unsafe { drop(CString::from_raw(s)); }
}
```

### Swift Side

```swift
// Gibberish.swift

import Foundation

class Gibberish {
    private var engine: OpaquePointer?

    init(modelPath: String) {
        engine = gibberish_init(modelPath)
    }

    deinit {
        if let engine = engine {
            gibberish_free(engine)
        }
    }

    func transcribe(audio: [Float]) -> String {
        guard let engine = engine else { return "" }

        let result = audio.withUnsafeBufferPointer { ptr in
            gibberish_transcribe(engine, ptr.baseAddress, ptr.count)
        }

        defer { gibberish_free_string(result) }
        return String(cString: result!)
    }
}
```

### Building for iOS

```bash
# Add iOS targets
rustup target add aarch64-apple-ios

# Build static library
cargo build --release --target aarch64-apple-ios

# The library will be at:
# target/aarch64-apple-ios/release/libgibberish.a
```

## Using UniFFI (Recommended)

For production FFI, use [UniFFI](https://mozilla.github.io/uniffi-rs/) to auto-generate bindings:

```toml
# Cargo.toml
[dependencies]
uniffi = "0.25"

[build-dependencies]
uniffi = { version = "0.25", features = ["build"] }
```

```rust
// src/lib.rs

#[uniffi::export]
pub fn transcribe(model_path: String, audio: Vec<f32>) -> String {
    let engine = Engine::new(&model_path).unwrap();
    engine.transcribe(&audio).unwrap().text
}
```

UniFFI generates Swift, Kotlin, Python, and Ruby bindings automatically.

## Performance Considerations

When running headless:

1. **Thread management**: You control threading, not Tauri
2. **Memory**: No WebView overhead (~100MB savings)
3. **Startup**: No UI initialization (~500ms faster)

For servers, consider:
- Connection pooling for engines (expensive to create)
- Request queuing during high load
- Graceful degradation when overloaded
