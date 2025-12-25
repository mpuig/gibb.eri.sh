mod engine;

pub use engine::{
    EngineLoader, Segment, SttEngine, Word, SILENCE_INJECTION_MS, SILENCE_INJECTION_SAMPLES,
    STT_SAMPLE_RATE,
};

#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("model not loaded")]
    ModelNotLoaded,
    #[error("transcription failed: {0}")]
    TranscriptionFailed(String),
    #[error("invalid audio format")]
    InvalidAudioFormat,
}

pub type Result<T> = std::result::Result<T, SttError>;
