#[derive(Debug, thiserror::Error)]
pub enum DiarizationError {
    #[error("model not loaded")]
    ModelNotLoaded,
    #[error("processing error: {0}")]
    ProcessingError(String),
}

pub type Result<T> = std::result::Result<T, DiarizationError>;

#[derive(Debug, Clone)]
pub struct SpeakerSegment {
    pub speaker_id: i32,
    pub start_ms: u64,
    pub end_ms: u64,
}

pub trait Diarizer: Send + Sync {
    fn process(&self, audio: &[f32], num_speakers: Option<i32>) -> Result<Vec<SpeakerSegment>>;
}
