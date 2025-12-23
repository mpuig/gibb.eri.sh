mod download;

use std::path::PathBuf;

pub use download::download_model;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttModel {
    // Whisper models (whisper.cpp GGML format)
    WhisperTiny,
    WhisperTinyEn,
    WhisperBase,
    WhisperBaseEn,
    WhisperSmall,
    WhisperSmallEn,
    WhisperLargeTurbo,
    // Parakeet models (ONNX, cross-platform)
    ParakeetCtc,
    ParakeetTdt,
    /// Real-time streaming model with end-of-utterance detection
    ParakeetEou,
}

impl SttModel {
    pub fn name(&self) -> &'static str {
        match self {
            Self::WhisperTiny => "whisper-tiny",
            Self::WhisperTinyEn => "whisper-tiny.en",
            Self::WhisperBase => "whisper-base",
            Self::WhisperBaseEn => "whisper-base.en",
            Self::WhisperSmall => "whisper-small",
            Self::WhisperSmallEn => "whisper-small.en",
            Self::WhisperLargeTurbo => "whisper-large-v3-turbo",
            Self::ParakeetCtc => "parakeet-ctc",
            Self::ParakeetTdt => "parakeet-tdt",
            Self::ParakeetEou => "parakeet-eou",
        }
    }

    pub fn dir_name(&self) -> &'static str {
        match self {
            Self::WhisperTiny => "whisper-tiny",
            Self::WhisperTinyEn => "whisper-tiny.en",
            Self::WhisperBase => "whisper-base",
            Self::WhisperBaseEn => "whisper-base.en",
            Self::WhisperSmall => "whisper-small",
            Self::WhisperSmallEn => "whisper-small.en",
            Self::WhisperLargeTurbo => "whisper-large-v3-turbo",
            Self::ParakeetCtc => "parakeet-ctc-0.6b",
            Self::ParakeetTdt => "parakeet-tdt-0.6b",
            Self::ParakeetEou => "parakeet-eou-120m",
        }
    }

    pub fn huggingface_repo(&self) -> &'static str {
        match self {
            Self::WhisperTiny
            | Self::WhisperTinyEn
            | Self::WhisperBase
            | Self::WhisperBaseEn
            | Self::WhisperSmall
            | Self::WhisperSmallEn
            | Self::WhisperLargeTurbo => "ggerganov/whisper.cpp",
            // ONNX versions from community repos
            Self::ParakeetCtc => "onnx-community/parakeet-ctc-0.6b-ONNX",
            Self::ParakeetTdt => "istupakov/parakeet-tdt-0.6b-v3-onnx",
            Self::ParakeetEou => "CHRV/parakeet_realtime_eou_120m-v1-onnx",
        }
    }

    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::WhisperTiny | Self::WhisperTinyEn => 75_000_000,
            Self::WhisperBase | Self::WhisperBaseEn => 142_000_000,
            Self::WhisperSmall | Self::WhisperSmallEn => 466_000_000,
            Self::WhisperLargeTurbo => 1_600_000_000,
            // FP16 model size (1.22 GB) - INT8 has ONNX compatibility issues
            Self::ParakeetCtc => 1_220_000_000,
            Self::ParakeetTdt => 700_000_000,
            Self::ParakeetEou => 140_000_000, // 132MB encoder + 5MB decoder
        }
    }

    pub fn is_whisper(&self) -> bool {
        matches!(
            self,
            Self::WhisperTiny
                | Self::WhisperTinyEn
                | Self::WhisperBase
                | Self::WhisperBaseEn
                | Self::WhisperSmall
                | Self::WhisperSmallEn
                | Self::WhisperLargeTurbo
        )
    }

    pub fn is_parakeet(&self) -> bool {
        matches!(self, Self::ParakeetCtc | Self::ParakeetTdt | Self::ParakeetEou)
    }
}

pub fn models_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gibberish")
        .join("models")
}

pub fn model_path(model: SttModel) -> PathBuf {
    models_dir().join(model.dir_name())
}

pub fn is_downloaded(model: SttModel) -> bool {
    let dir = model_path(model);
    if !dir.exists() {
        return false;
    }

    // Check for required files based on model type
    match model {
        SttModel::ParakeetCtc => {
            // Check for model_fp16.onnx (original filename to match external data reference)
            dir.join("model_fp16.onnx").exists() && dir.join("tokenizer.json").exists()
        }
        SttModel::ParakeetTdt => {
            dir.join("encoder-model.onnx").exists()
                && dir.join("decoder_joint-model.onnx").exists()
                && dir.join("vocab.txt").exists()
        }
        SttModel::ParakeetEou => {
            // EOU uses different file names (no dashes)
            dir.join("encoder.onnx").exists()
                && dir.join("decoder_joint.onnx").exists()
                && dir.join("vocab.txt").exists()
        }
        _ => {
            // Whisper models
            dir.join("model.bin").exists()
        }
    }
}
