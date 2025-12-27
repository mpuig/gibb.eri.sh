mod download;
mod metadata;
mod turn;

use std::path::PathBuf;

pub use download::download_model;
pub use metadata::{get_metadata, ModelCategory, ModelMetadata};
pub use turn::{download_turn_model, is_turn_model_downloaded, turn_model_path, TurnModel};

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model not found: {0}")]
    NotFound(String),
    #[error("download failed: {0}")]
    DownloadFailed(String),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ModelError>;

/// STT model identifier.
///
/// This enum represents domain identity only. Infrastructure details
/// (URLs, sizes, file paths) are stored in the metadata registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttModel {
    // Whisper models (whisper.cpp GGML format)
    WhisperSmall,
    WhisperSmallEn,
    WhisperLargeTurbo,
    // Whisper ONNX models (sherpa-onnx format, multilingual)
    WhisperOnnxSmall,
    // Parakeet models (ONNX, cross-platform)
    ParakeetCtc,
    ParakeetTdt,
    ParakeetEou,
    // Sherpa-ONNX streaming models
    SherpaZipformerEn,
    // NeMo CTC models
    NemoConformerCatalan,
}

impl SttModel {
    /// Get the unique identifier for this model.
    pub fn name(&self) -> &'static str {
        match self {
            Self::WhisperSmall => "whisper-small",
            Self::WhisperSmallEn => "whisper-small.en",
            Self::WhisperLargeTurbo => "whisper-large-v3-turbo",
            Self::WhisperOnnxSmall => "whisper-onnx-small",
            Self::ParakeetCtc => "parakeet-ctc",
            Self::ParakeetTdt => "parakeet-tdt",
            Self::ParakeetEou => "parakeet-eou",
            Self::SherpaZipformerEn => "sherpa-zipformer-en",
            Self::NemoConformerCatalan => "nemo-conformer-ca",
        }
    }

    /// Get the metadata for this model from the registry.
    pub fn metadata(&self) -> &'static ModelMetadata {
        get_metadata(self.name()).expect("all SttModel variants must have metadata")
    }

    /// Get the directory name for local storage (delegates to metadata).
    pub fn dir_name(&self) -> &'static str {
        self.metadata().dir_name
    }

    /// Get the HuggingFace repository (delegates to metadata).
    pub fn huggingface_repo(&self) -> &'static str {
        self.metadata().huggingface_repo
    }

    /// Get the approximate size in bytes (delegates to metadata).
    pub fn size_bytes(&self) -> u64 {
        self.metadata().size_bytes
    }

    /// Get the model category (delegates to metadata).
    pub fn category(&self) -> ModelCategory {
        self.metadata().category
    }

    /// Check if this is a Whisper GGML model.
    pub fn is_whisper(&self) -> bool {
        self.category() == ModelCategory::WhisperGgml
    }

    /// Check if this is a Whisper ONNX model (sherpa-onnx format).
    pub fn is_whisper_onnx(&self) -> bool {
        self.category() == ModelCategory::WhisperOnnx
    }

    /// Check if this is a Parakeet model.
    pub fn is_parakeet(&self) -> bool {
        self.category() == ModelCategory::Parakeet
    }

    /// Check if this is a Sherpa streaming model.
    pub fn is_sherpa(&self) -> bool {
        self.category() == ModelCategory::SherpaStreaming
    }

    /// Get supported languages for this model.
    ///
    /// Returns a list of ISO 639-1 language codes. An empty slice means
    /// the model supports automatic language detection (multilingual).
    pub fn supported_languages(&self) -> &'static [&'static str] {
        match self {
            // English-only models
            Self::WhisperSmallEn
            | Self::ParakeetCtc
            | Self::ParakeetTdt
            | Self::ParakeetEou
            | Self::SherpaZipformerEn => &["en"],

            // Catalan-only models
            Self::NemoConformerCatalan => &["ca"],

            // Multilingual models (empty = auto-detect)
            Self::WhisperSmall | Self::WhisperLargeTurbo | Self::WhisperOnnxSmall => &[],
        }
    }
}

pub fn models_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gibb.eri.sh")
        .join("models")
}

pub fn model_path(model: SttModel) -> PathBuf {
    models_dir().join(model.dir_name())
}

/// Check if a model is downloaded by verifying required files exist.
///
/// Uses the metadata registry to determine which files to check.
pub fn is_downloaded(model: SttModel) -> bool {
    let dir = model_path(model);
    if !dir.exists() {
        return false;
    }

    let metadata = model.metadata();
    (metadata.is_downloaded)(&dir)
}
