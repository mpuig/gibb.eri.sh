use std::any::Any;
use std::borrow::Cow;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Word {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct Segment {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub words: Vec<Word>,
    pub speaker: Option<i32>,
}

/// Standard sample rate for STT processing.
pub const STT_SAMPLE_RATE: u32 = 16000;

/// Silence injection duration in milliseconds (for VAD transitions).
pub const SILENCE_INJECTION_MS: u32 = 100;

/// Number of silence samples to inject.
pub const SILENCE_INJECTION_SAMPLES: usize =
    (STT_SAMPLE_RATE as usize * SILENCE_INJECTION_MS as usize) / 1000;

pub trait SttEngine: Send + Sync {
    /// Transcribe audio samples (expected at 16kHz mono).
    fn transcribe(&self, audio: &[f32]) -> crate::Result<Vec<Segment>>;

    /// Transcribe an audio file directly.
    ///
    /// Default implementation reads the WAV file and calls `transcribe()`.
    /// Engines with native file support can override for better performance.
    fn transcribe_file(&self, path: &Path) -> crate::Result<Vec<Segment>> {
        let samples = read_wav_mono_f32_16k(path)?;
        self.transcribe(&samples)
    }

    fn is_streaming_capable(&self) -> bool {
        false
    }

    fn model_name(&self) -> &str;

    fn supported_languages(&self) -> Vec<&'static str> {
        vec!["en"]
    }

    /// Downcast to concrete type for engine-specific streaming operations.
    ///
    /// This should only be used at the infrastructure layer (plugins) for
    /// accessing streaming-specific methods. Domain/application code should
    /// use trait methods only.
    fn as_any(&self) -> &dyn Any;
}

/// Factory trait for creating STT engines.
///
/// Infrastructure crates implement this to register their engine types.
/// This follows Dependency Inversion - the application layer depends on
/// this abstraction, not concrete engine types.
pub trait EngineLoader: Send + Sync {
    /// Human-readable name of the engine type (e.g., "Whisper ONNX", "Parakeet TDT").
    fn name(&self) -> &str;

    /// Check if this loader can handle the given model identifier.
    fn can_load(&self, model_id: &str) -> bool;

    /// Load an engine for the given model.
    ///
    /// The `model_path` is the directory containing model files.
    /// The `language` parameter is an optional language code (e.g., "en", "es", "ca").
    /// Pass empty string or "auto" for automatic language detection.
    fn load(
        &self,
        model_id: &str,
        model_path: &Path,
        language: &str,
    ) -> crate::Result<Box<dyn SttEngine>>;

    /// Check if the model supports real-time streaming inference.
    fn is_streaming(&self, _model_id: &str) -> bool {
        false
    }
}

/// Resample audio using linear interpolation.
fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Cow<'_, [f32]> {
    if from_rate == to_rate {
        return Cow::Borrowed(samples);
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = (samples.len() as f64 * ratio) as usize;
    let mut output = Vec::with_capacity(new_len);
    for i in 0..new_len {
        let src_idx = i as f64 / ratio;
        let idx = src_idx.floor() as usize;
        let frac = src_idx.fract() as f32;
        let sample = if idx + 1 < samples.len() {
            samples[idx] * (1.0 - frac) + samples[idx + 1] * frac
        } else if idx < samples.len() {
            samples[idx]
        } else {
            0.0
        };
        output.push(sample);
    }
    Cow::Owned(output)
}

/// Read a WAV file and return mono f32 samples at 16kHz.
fn read_wav_mono_f32_16k(path: &Path) -> crate::Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| crate::SttError::TranscriptionFailed(e.to_string()))?;
    let spec = reader.spec();

    let channels = spec.channels.max(1) as usize;
    let sample_rate = spec.sample_rate;

    let raw: Vec<i16> = reader
        .samples::<i16>()
        .map(|s| s.map_err(|e| crate::SttError::TranscriptionFailed(e.to_string())))
        .collect::<Result<_, _>>()?;

    let mut mono = Vec::with_capacity(raw.len() / channels);
    for frame in raw.chunks(channels) {
        let sum: i32 = frame.iter().map(|s| *s as i32).sum();
        let avg = sum as f32 / channels as f32;
        mono.push(avg / i16::MAX as f32);
    }

    Ok(resample_linear(&mono, sample_rate, STT_SAMPLE_RATE).into_owned())
}
