#[derive(Debug, Clone, Copy)]
pub struct TurnPrediction {
    pub probability: f32,
    pub threshold: f32,
}

impl TurnPrediction {
    pub fn is_complete(&self) -> bool {
        self.probability >= self.threshold
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TurnError {
    #[error("model not loaded")]
    ModelNotLoaded,
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, TurnError>;

pub trait TurnDetector: Send + Sync {
    fn name(&self) -> &'static str;
    fn predict_endpoint_probability(&self, audio_16k_mono: &[f32]) -> Result<f32>;
}
