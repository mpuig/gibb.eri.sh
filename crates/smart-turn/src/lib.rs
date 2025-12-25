//! Smart Turn v3.1 semantic endpoint detection.
//!
//! Uses a small on-device model to predict whether a speech pause is a true
//! end-of-turn or just a mid-utterance pause.

mod features;

use features::{compute_input_features, FEATURE_SHAPE};
use gibberish_turn::{TurnDetector, TurnError};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum SmartTurnError {
    #[error("failed to load model: {0}")]
    Model(String),
    #[error("inference failed: {0}")]
    Inference(String),
}

#[derive(Debug)]
pub struct SmartTurnV31Cpu {
    session: Mutex<Session>,
    input_name: String,
    output_name: String,
}

impl SmartTurnV31Cpu {
    pub fn load(model_path: impl AsRef<Path>) -> Result<Self, SmartTurnError> {
        // Recommended by Daily: reduce OpenMP contention for consistent CPU inference.
        if std::env::var("OMP_NUM_THREADS").is_err() {
            std::env::set_var("OMP_NUM_THREADS", "1");
        }
        if std::env::var("OMP_WAIT_POLICY").is_err() {
            std::env::set_var("OMP_WAIT_POLICY", "PASSIVE");
        }

        let session = Session::builder()
            .map_err(|e| SmartTurnError::Model(e.to_string()))?
            .with_parallel_execution(false)
            .map_err(|e| SmartTurnError::Model(e.to_string()))?
            .with_inter_threads(1)
            .map_err(|e| SmartTurnError::Model(e.to_string()))?
            .with_intra_threads(1)
            .map_err(|e| SmartTurnError::Model(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| SmartTurnError::Model(e.to_string()))?
            .commit_from_file(model_path.as_ref())
            .map_err(|e| SmartTurnError::Model(e.to_string()))?;

        let input_name = session
            .inputs
            .iter()
            .find(|i| i.name == "input_features")
            .map(|i| i.name.clone())
            .or_else(|| session.inputs.first().map(|i| i.name.clone()))
            .ok_or_else(|| SmartTurnError::Model("model has no inputs".to_string()))?;

        let output_name = session
            .outputs
            .iter()
            .find(|o| o.name == "logits")
            .map(|o| o.name.clone())
            .or_else(|| session.outputs.first().map(|o| o.name.clone()))
            .ok_or_else(|| SmartTurnError::Model("model has no outputs".to_string()))?;

        Ok(Self {
            session: Mutex::new(session),
            input_name,
            output_name,
        })
    }

    pub fn predict_probability(&self, audio_16k_mono: &[f32]) -> Result<f32, SmartTurnError> {
        let features = compute_input_features(audio_16k_mono);

        let input = Tensor::from_array((
            [1i64, FEATURE_SHAPE.0 as i64, FEATURE_SHAPE.1 as i64],
            features,
        ))
        .map_err(|e| SmartTurnError::Inference(e.to_string()))?;

        let mut session = self
            .session
            .lock()
            .map_err(|_| SmartTurnError::Inference("lock poisoned".to_string()))?;

        let outputs = session
            .run(ort::inputs![self.input_name.as_str() => input])
            .map_err(|e| SmartTurnError::Inference(e.to_string()))?;

        let output = outputs
            .get(self.output_name.as_str())
            .ok_or_else(|| SmartTurnError::Inference("missing model output".to_string()))?;

        let (_shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| SmartTurnError::Inference(e.to_string()))?;
        let p = data
            .first()
            .copied()
            .ok_or_else(|| SmartTurnError::Inference("empty output".to_string()))?;
        Ok(p)
    }
}

impl TurnDetector for SmartTurnV31Cpu {
    fn name(&self) -> &'static str {
        "smart-turn-v3.1-cpu"
    }

    fn predict_endpoint_probability(&self, audio_16k_mono: &[f32]) -> Result<f32, TurnError> {
        self.predict_probability(audio_16k_mono)
            .map_err(|e| TurnError::Inference(e.to_string()))
    }
}
