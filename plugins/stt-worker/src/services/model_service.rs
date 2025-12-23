use gibberish_models::{is_downloaded, model_path, SttModel};
use gibberish_parakeet::ParakeetEngine;
use gibberish_stt::SttEngine;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub dir_name: String,
    pub is_downloaded: bool,
    pub size_bytes: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("unknown model: {0}")]
    UnknownModel(String),
    #[error("model not downloaded: {0}")]
    NotDownloaded(String),
    #[error("download already in progress")]
    DownloadInProgress,
    #[error("download failed: {0}")]
    DownloadFailed(String),
    #[error("load failed: {0}")]
    LoadFailed(String),
}

pub struct ModelService;

impl ModelService {
    pub fn list_available_models() -> Vec<ModelInfo> {
        use SttModel::*;
        // Only TDT model - multilingual support (117 languages)
        let models = [ParakeetTdt];

        models
            .iter()
            .map(|m| ModelInfo {
                name: m.name().to_string(),
                dir_name: m.dir_name().to_string(),
                is_downloaded: is_downloaded(*m),
                size_bytes: m.size_bytes(),
            })
            .collect()
    }

    pub fn parse_model_name(name: &str) -> Result<SttModel, ModelError> {
        match name {
            "parakeet-tdt" => Ok(SttModel::ParakeetTdt),
            _ => Err(ModelError::UnknownModel(name.to_string())),
        }
    }

    pub fn is_model_downloaded(model: SttModel) -> bool {
        is_downloaded(model)
    }

    pub fn get_model_path(model: SttModel) -> PathBuf {
        model_path(model)
    }

    pub fn load_engine(model: SttModel) -> Result<Arc<dyn SttEngine>, ModelError> {
        if !is_downloaded(model) {
            return Err(ModelError::NotDownloaded(model.name().to_string()));
        }

        let path = model_path(model);
        tracing::info!("Loading model from: {:?}", path);

        let engine = ParakeetEngine::new(&path).map_err(|e| ModelError::LoadFailed(e.to_string()))?;

        tracing::info!("Model loaded: {}", model.name());
        Ok(Arc::new(engine))
    }
}
