//! EngineLoader implementations for Sherpa engines.
//!
//! Provides factory implementations for creating Sherpa-based STT engines
//! following the Dependency Inversion principle.

use std::path::Path;

use gibberish_stt::{EngineLoader, SttEngine};

use crate::{SherpaNemoCtcEngine, SherpaStreamingEngine, SherpaWhisperEngine};

/// Loader for Sherpa Zipformer streaming transducer models.
pub struct SherpaZipformerLoader;

impl EngineLoader for SherpaZipformerLoader {
    fn name(&self) -> &str {
        "Sherpa Zipformer Transducer"
    }

    fn can_load(&self, model_id: &str) -> bool {
        model_id == "sherpa-zipformer-en"
    }

    fn load(
        &self,
        _model_id: &str,
        model_path: &Path,
        _language: &str,
    ) -> gibberish_stt::Result<Box<dyn SttEngine>> {
        // Zipformer models are language-specific, ignore language parameter
        let engine = SherpaStreamingEngine::new_zipformer_transducer(model_path)
            .map_err(|e| gibberish_stt::SttError::TranscriptionFailed(e.to_string()))?;
        Ok(Box::new(engine))
    }

    fn is_streaming(&self, _model_id: &str) -> bool {
        true
    }
}

/// Loader for Sherpa Whisper ONNX models.
pub struct SherpaWhisperLoader;

impl EngineLoader for SherpaWhisperLoader {
    fn name(&self) -> &str {
        "Sherpa Whisper ONNX"
    }

    fn can_load(&self, model_id: &str) -> bool {
        matches!(
            model_id,
            "whisper-onnx-tiny" | "whisper-onnx-base" | "whisper-onnx-small"
        )
    }

    fn load(
        &self,
        _model_id: &str,
        model_path: &Path,
        language: &str,
    ) -> gibberish_stt::Result<Box<dyn SttEngine>> {
        // Use provided language or empty string for auto-detect
        let lang = if language == "auto" { "" } else { language };
        let engine = SherpaWhisperEngine::new(model_path, lang)
            .map_err(|e| gibberish_stt::SttError::TranscriptionFailed(e.to_string()))?;
        Ok(Box::new(engine))
    }

    fn is_streaming(&self, _model_id: &str) -> bool {
        false // Whisper is batch-only
    }
}

/// Loader for NeMo CTC models (e.g., Catalan Conformer).
pub struct SherpaNemoCtcLoader;

impl EngineLoader for SherpaNemoCtcLoader {
    fn name(&self) -> &str {
        "NeMo CTC"
    }

    fn can_load(&self, model_id: &str) -> bool {
        model_id == "nemo-conformer-ca"
    }

    fn load(
        &self,
        _model_id: &str,
        model_path: &Path,
        _language: &str,
    ) -> gibberish_stt::Result<Box<dyn SttEngine>> {
        // NeMo CTC models are language-specific (Catalan for this model)
        let engine = SherpaNemoCtcEngine::new(model_path)
            .map_err(|e| gibberish_stt::SttError::TranscriptionFailed(e.to_string()))?;
        Ok(Box::new(engine))
    }

    fn is_streaming(&self, _model_id: &str) -> bool {
        false // CTC is batch-only
    }
}
