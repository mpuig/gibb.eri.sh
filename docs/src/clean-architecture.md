# Clean Architecture

We use Dependency Inversion to keep the codebase maintainable. Here's the pattern.

## The Problem We Avoided

Imagine adding a new STT engine:

```rust
// BAD: Direct dependencies everywhere
match config.engine {
    Engine::Sherpa => sherpa::transcribe(&audio),
    Engine::Parakeet => parakeet::transcribe(&audio),
    Engine::NewEngine => new_engine::transcribe(&audio), // ADD HERE
}
// ... and here, and here, and here
```

Every new engine means touching multiple files. Tests break. Things get coupled.

## The Solution: Trait-Based Abstraction

Define a trait. Implement it. Inject the implementation.

### The `SttEngine` Trait

```rust
// crates/stt/src/engine.rs

pub trait SttEngine: Send + Sync {
    fn transcribe(&self, audio: &[f32]) -> Result<Vec<Segment>>;
    fn is_streaming_capable(&self) -> bool;
    fn model_name(&self) -> &str;
    fn supported_languages(&self) -> Vec<&'static str>;
}
```

### Implementations

```rust
// crates/sherpa/src/zipformer.rs
impl SttEngine for ZipformerEngine {
    fn transcribe(&self, audio: &[f32]) -> Result<Vec<Segment>> {
        // Sherpa-specific implementation
    }
    // ...
}

// crates/parakeet/src/lib.rs
impl SttEngine for ParakeetEngine {
    fn transcribe(&self, audio: &[f32]) -> Result<Vec<Segment>> {
        // Parakeet-specific implementation
    }
    // ...
}
```

### Usage

The application layer never knows which engine it's using:

```rust
// crates/application/src/transcriber.rs

pub struct Transcriber {
    engine: Box<dyn SttEngine>,
}

impl Transcriber {
    pub fn new(engine: Box<dyn SttEngine>) -> Self {
        Self { engine }
    }

    pub fn process(&self, audio: &[f32]) -> Result<Vec<Segment>> {
        self.engine.transcribe(audio)
    }
}
```

## The Factory Pattern

How do we create the right engine at runtime?

```rust
// crates/stt/src/loader.rs

pub trait EngineLoader: Send + Sync {
    fn name(&self) -> &str;
    fn can_load(&self, model_id: &str) -> bool;
    fn load(&self, model_path: &Path) -> Result<Box<dyn SttEngine>>;
}

// Usage
pub fn create_engine(
    loaders: &[Box<dyn EngineLoader>],
    model_id: &str,
    path: &Path,
) -> Result<Box<dyn SttEngine>> {
    for loader in loaders {
        if loader.can_load(model_id) {
            return loader.load(path);
        }
    }
    Err(Error::UnknownModel(model_id.to_string()))
}
```

## Adding a New Engine

Adding `WhisperTurbo` requires:

1. Create `crates/whisper-turbo/`
2. Implement `SttEngine`
3. Implement `EngineLoader`
4. Register the loader at startup

No changes to `crates/application/`. No changes to existing engines. No changes to the UI.

```rust
// crates/whisper-turbo/src/lib.rs

pub struct WhisperTurboEngine { /* ... */ }

impl SttEngine for WhisperTurboEngine {
    fn transcribe(&self, audio: &[f32]) -> Result<Vec<Segment>> {
        // Implementation
    }
    // ...
}

pub struct WhisperTurboLoader;

impl EngineLoader for WhisperTurboLoader {
    fn name(&self) -> &str { "whisper-turbo" }

    fn can_load(&self, model_id: &str) -> bool {
        model_id.starts_with("whisper-turbo")
    }

    fn load(&self, path: &Path) -> Result<Box<dyn SttEngine>> {
        Ok(Box::new(WhisperTurboEngine::new(path)?))
    }
}
```

## The Dependency Graph

```
                    ┌─────────────────┐
                    │   application   │
                    │  (orchestration)│
                    └────────┬────────┘
                             │ depends on trait
                             ▼
                    ┌─────────────────┐
                    │       stt       │
                    │   (SttEngine)   │
                    └────────┬────────┘
                             │ implemented by
            ┌────────────────┼────────────────┐
            ▼                ▼                ▼
     ┌──────────┐     ┌──────────┐     ┌──────────┐
     │  sherpa  │     │ parakeet │     │ whisper  │
     └──────────┘     └──────────┘     └──────────┘
```

`application` never imports `sherpa`, `parakeet`, or `whisper` directly. It only knows `SttEngine`.

## Testing

Trait-based design makes testing simple:

```rust
struct MockEngine {
    response: Vec<Segment>,
}

impl SttEngine for MockEngine {
    fn transcribe(&self, _audio: &[f32]) -> Result<Vec<Segment>> {
        Ok(self.response.clone())
    }
    // ...
}

#[test]
fn test_transcriber() {
    let mock = MockEngine {
        response: vec![Segment { text: "hello".into(), ..Default::default() }],
    };

    let transcriber = Transcriber::new(Box::new(mock));
    let result = transcriber.process(&[0.0; 1600]).unwrap();

    assert_eq!(result[0].text, "hello");
}
```

No model files needed. No inference overhead. Fast tests.

## Other Traits

The same pattern applies elsewhere:

| Trait | Location | Implementations |
|-------|----------|-----------------|
| `SttEngine` | `crates/stt` | Sherpa, Parakeet |
| `VoiceActivityDetector` | `crates/vad` | Silero |
| `TurnDetector` | `crates/turn` | SmartTurn, Simple |
| `SessionStorage` | `crates/storage` | SQLite |

## The Trade-Off

Trait objects have runtime cost:
- Dynamic dispatch (vtable lookup)
- Can't be inlined

For inference (already 50-200ms), this overhead is negligible. We measured <1μs per trait call.

If performance mattered here, we'd use generics:

```rust
// Generic (faster, but less flexible)
pub struct Transcriber<E: SttEngine> {
    engine: E,
}

// Trait object (our choice: flexible, dynamic)
pub struct Transcriber {
    engine: Box<dyn SttEngine>,
}
```

We chose flexibility. Runtime engine switching is worth the microseconds.
