use std::any::Any;
use std::path::Path;
use std::sync::Mutex;

use gibberish_stt::EngineLoader;
pub use parakeet_rs::{
    ExecutionProvider, ParakeetTDT, TimestampMode, Transcriber, TranscriptionResult,
};

#[derive(Debug, thiserror::Error)]
pub enum ParakeetError {
    #[error("model not loaded")]
    ModelNotLoaded,
    #[error("transcription failed: {0}")]
    TranscriptionFailed(String),
    #[error("lock poisoned")]
    LockPoisoned,
}

pub type Result<T> = std::result::Result<T, ParakeetError>;

pub struct ParakeetEngine {
    model: Mutex<ParakeetTDT>,
    model_name: String,
}

impl ParakeetEngine {
    pub fn new(model_dir: impl AsRef<Path>) -> Result<Self> {
        let model_dir = model_dir.as_ref();

        // TDT model files: encoder-model.onnx, decoder_joint-model.onnx, vocab.txt
        if !model_dir.join("encoder-model.onnx").exists() {
            return Err(ParakeetError::TranscriptionFailed(
                "TDT model files not found (encoder-model.onnx required)".to_string(),
            ));
        }

        let model = ParakeetTDT::from_pretrained(model_dir, None)
            .map_err(|e| ParakeetError::TranscriptionFailed(e.to_string()))?;

        let model_name = model_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("parakeet-tdt")
            .to_string();

        Ok(Self {
            model: Mutex::new(model),
            model_name,
        })
    }

    pub fn with_execution_provider(
        model_dir: impl AsRef<Path>,
        provider: ExecutionProvider,
    ) -> Result<Self> {
        let model_dir = model_dir.as_ref();
        let config = parakeet_rs::ExecutionConfig::new().with_execution_provider(provider);

        if !model_dir.join("encoder-model.onnx").exists() {
            return Err(ParakeetError::TranscriptionFailed(
                "TDT model files not found (encoder-model.onnx required)".to_string(),
            ));
        }

        let model = ParakeetTDT::from_pretrained(model_dir, Some(config))
            .map_err(|e| ParakeetError::TranscriptionFailed(e.to_string()))?;

        let model_name = model_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("parakeet-tdt")
            .to_string();

        Ok(Self {
            model: Mutex::new(model),
            model_name,
        })
    }

    pub fn transcribe_file(&self, path: impl AsRef<Path>) -> Result<TranscriptionResult> {
        let mut model = self.model.lock().map_err(|_| ParakeetError::LockPoisoned)?;

        model
            .transcribe_file(path, Some(TimestampMode::Words))
            .map_err(|e| ParakeetError::TranscriptionFailed(e.to_string()))
    }

    pub fn transcribe_samples(
        &self,
        audio: &[f32],
        sample_rate: u32,
    ) -> Result<TranscriptionResult> {
        let mut model = self.model.lock().map_err(|_| ParakeetError::LockPoisoned)?;

        model
            .transcribe_samples(audio.to_vec(), sample_rate, 1, Some(TimestampMode::Words))
            .map_err(|e| ParakeetError::TranscriptionFailed(e.to_string()))
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }
}

impl gibberish_stt::SttEngine for ParakeetEngine {
    fn transcribe(&self, audio: &[f32]) -> gibberish_stt::Result<Vec<gibberish_stt::Segment>> {
        let result = self
            .transcribe_samples(audio, 16000)
            .map_err(|e| gibberish_stt::SttError::TranscriptionFailed(e.to_string()))?;

        Ok(vec![transcription_result_to_segment(result)])
    }

    /// Override default to use Parakeet's native file transcription.
    fn transcribe_file(&self, path: &Path) -> gibberish_stt::Result<Vec<gibberish_stt::Segment>> {
        let result = self
            .transcribe_file(path)
            .map_err(|e| gibberish_stt::SttError::TranscriptionFailed(e.to_string()))?;

        Ok(vec![transcription_result_to_segment(result)])
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn supported_languages(&self) -> Vec<&'static str> {
        vec!["en"]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn sec_to_ms_range(start_s: f32, end_s: f32) -> (u64, u64) {
    let start_ms = (start_s.max(0.0) * 1000.0).round() as u64;
    let mut end_ms = (end_s.max(0.0) * 1000.0).round() as u64;
    if end_ms <= start_ms {
        end_ms = start_ms + 1;
    }
    (start_ms, end_ms)
}

fn transcription_result_to_segment(result: TranscriptionResult) -> gibberish_stt::Segment {
    let words: Vec<gibberish_stt::Word> = result
        .tokens
        .iter()
        .map(|t| {
            let (start_ms, end_ms) = sec_to_ms_range(t.start, t.end);
            gibberish_stt::Word {
                text: t.text.clone(),
                start_ms,
                end_ms,
                confidence: 1.0,
            }
        })
        .collect();

    let start_ms = words.first().map(|w| w.start_ms).unwrap_or(0);
    let end_ms = words.last().map(|w| w.end_ms).unwrap_or(0);

    gibberish_stt::Segment {
        text: result.text,
        start_ms,
        end_ms,
        words,
        speaker: None,
    }
}

unsafe impl Send for ParakeetEngine {}
unsafe impl Sync for ParakeetEngine {}

/// Loader for Parakeet TDT models.
pub struct ParakeetTdtLoader;

impl EngineLoader for ParakeetTdtLoader {
    fn name(&self) -> &str {
        "Parakeet TDT"
    }

    fn can_load(&self, model_id: &str) -> bool {
        model_id == "parakeet-tdt"
    }

    fn load(
        &self,
        _model_id: &str,
        model_path: &Path,
        _language: &str,
    ) -> gibberish_stt::Result<Box<dyn gibberish_stt::SttEngine>> {
        // Parakeet TDT is English-only, ignore language parameter
        let engine = ParakeetEngine::new(model_path)
            .map_err(|e| gibberish_stt::SttError::TranscriptionFailed(e.to_string()))?;
        Ok(Box::new(engine))
    }

    fn is_streaming(&self, _model_id: &str) -> bool {
        false // TDT is batch-only
    }
}
