//! Whisper ONNX offline transcription engine.
//!
//! Uses sherpa-onnx's offline Whisper API for non-streaming transcription.
//! Whisper models are multilingual and provide high-quality transcription.

use std::any::Any;
use std::path::Path;
use std::sync::Mutex;

use gibberish_stt::{Segment, SttEngine, Word};
use sherpa_rs::whisper::{WhisperConfig, WhisperRecognizer};

use crate::{Result, SherpaError};

/// Whisper ONNX engine for offline (non-streaming) transcription.
///
/// This engine uses OpenAI's Whisper model converted to ONNX format,
/// providing high-quality multilingual transcription. It runs batch
/// inference, making it suitable for transcribing complete utterances
/// or audio files.
pub struct SherpaWhisperEngine {
    recognizer: Mutex<WhisperRecognizer>,
    model_name: String,
    language: String,
}

impl SherpaWhisperEngine {
    /// Create a new Whisper engine from model files.
    ///
    /// # Arguments
    /// * `model_dir` - Directory containing encoder.onnx, decoder.onnx, and tokens.txt
    /// * `language` - Language code (e.g., "en", "es", "fr") or empty for auto-detect
    ///
    /// # Model Files
    /// The model directory should contain:
    /// - `{prefix}-encoder.onnx` or `{prefix}-encoder.int8.onnx`
    /// - `{prefix}-decoder.onnx` or `{prefix}-decoder.int8.onnx`
    /// - `{prefix}-tokens.txt`
    ///
    /// Where prefix is typically "tiny", "base", "small", "medium", or "large".
    pub fn new(model_dir: impl AsRef<Path>, language: &str) -> Result<Self> {
        Self::new_with_prefix(model_dir, language, None)
    }

    /// Create a new Whisper engine with explicit model prefix.
    ///
    /// # Arguments
    /// * `model_dir` - Directory containing model files
    /// * `language` - Language code (e.g., "en", "es", "fr") or empty for auto-detect
    /// * `prefix` - Optional model prefix (e.g., "tiny", "base"). If None, auto-detected.
    pub fn new_with_prefix(
        model_dir: impl AsRef<Path>,
        language: &str,
        prefix: Option<&str>,
    ) -> Result<Self> {
        let model_dir = model_dir.as_ref();

        // Auto-detect prefix from available files if not specified
        let prefix = match prefix {
            Some(p) => p.to_string(),
            None => detect_model_prefix(model_dir)?,
        };

        // Prefer int8 quantized models for better performance
        let encoder = find_model_file(model_dir, &prefix, "encoder")?;
        let decoder = find_model_file(model_dir, &prefix, "decoder")?;
        let tokens = model_dir.join(format!("{prefix}-tokens.txt"));

        if !tokens.exists() {
            return Err(SherpaError::MissingFiles(format!(
                "tokens file not found: {}",
                tokens.display()
            )));
        }

        tracing::info!(
            encoder = %encoder.display(),
            decoder = %decoder.display(),
            tokens = %tokens.display(),
            language = language,
            "Loading Whisper ONNX model"
        );

        let config = WhisperConfig {
            encoder: encoder.to_string_lossy().to_string(),
            decoder: decoder.to_string_lossy().to_string(),
            tokens: tokens.to_string_lossy().to_string(),
            language: language.to_string(),
            num_threads: Some(2),
            ..Default::default()
        };

        let recognizer = WhisperRecognizer::new(config).map_err(|e| {
            tracing::error!(error = %e, "Failed to create Whisper recognizer");
            SherpaError::LoadFailed(e.to_string())
        })?;

        let model_name = model_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("whisper")
            .to_string();

        Ok(Self {
            recognizer: Mutex::new(recognizer),
            model_name,
            language: language.to_string(),
        })
    }

    /// Transcribe audio samples.
    pub fn transcribe_samples(&self, audio: &[f32], sample_rate: u32) -> Result<String> {
        let mut recognizer = self
            .recognizer
            .lock()
            .map_err(|_| SherpaError::TranscriptionFailed("lock poisoned".to_string()))?;

        let result = recognizer.transcribe(sample_rate, audio);
        Ok(result.text.trim().to_string())
    }
}

impl SttEngine for SherpaWhisperEngine {
    fn transcribe(&self, audio: &[f32]) -> gibberish_stt::Result<Vec<Segment>> {
        let text = self
            .transcribe_samples(audio, 16000)
            .map_err(|e| gibberish_stt::SttError::TranscriptionFailed(e.to_string()))?;

        let end_ms = (audio.len() as f64 / 16000.0 * 1000.0).round() as u64;

        Ok(vec![Segment {
            text,
            start_ms: 0,
            end_ms,
            words: Vec::<Word>::new(),
            speaker: None,
        }])
    }

    fn is_streaming_capable(&self) -> bool {
        false // Whisper is batch-only
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn supported_languages(&self) -> Vec<&'static str> {
        // Whisper multilingual supports 99+ languages
        // Return most common ones for UI purposes
        if self.language.is_empty() || self.language == "auto" {
            vec![
                "en", "es", "fr", "de", "it", "pt", "ru", "zh", "ja", "ko", "ar", "hi", "nl", "pl",
                "sv", "tr", "vi", "th", "id", "ms", "ca", "cs", "da", "fi", "el", "he", "hu", "no",
                "ro", "sk", "uk",
            ]
        } else {
            // English-only model
            vec!["en"]
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Detect the model prefix from available files in the directory.
fn detect_model_prefix(model_dir: &Path) -> Result<String> {
    let prefixes = [
        "tiny",
        "base",
        "small",
        "medium",
        "large",
        "turbo",
        "distil-small",
        "distil-medium",
        "distil-large-v2",
        "distil-large-v3",
    ];

    for prefix in prefixes {
        let encoder = model_dir.join(format!("{prefix}-encoder.onnx"));
        let encoder_int8 = model_dir.join(format!("{prefix}-encoder.int8.onnx"));
        if encoder.exists() || encoder_int8.exists() {
            return Ok(prefix.to_string());
        }
    }

    Err(SherpaError::MissingFiles(
        "Could not detect Whisper model prefix (expected tiny/base/small/medium/large encoder.onnx)"
            .to_string(),
    ))
}

/// Find the model file, preferring int8 quantized version.
fn find_model_file(model_dir: &Path, prefix: &str, component: &str) -> Result<std::path::PathBuf> {
    // Prefer int8 for better performance
    let int8_path = model_dir.join(format!("{prefix}-{component}.int8.onnx"));
    if int8_path.exists() {
        return Ok(int8_path);
    }

    let fp32_path = model_dir.join(format!("{prefix}-{component}.onnx"));
    if fp32_path.exists() {
        return Ok(fp32_path);
    }

    Err(SherpaError::MissingFiles(format!(
        "{} not found in {} (tried {}-{}.int8.onnx and {}-{}.onnx)",
        component,
        model_dir.display(),
        prefix,
        component,
        prefix,
        component
    )))
}
