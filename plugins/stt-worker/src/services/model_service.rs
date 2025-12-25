use gibberish_models::{is_downloaded, model_path, SttModel};
use gibberish_stt::{EngineLoader, SttEngine};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub dir_name: String,
    pub is_downloaded: bool,
    pub size_bytes: u64,
    /// Supported language codes. Empty means multilingual with auto-detect.
    pub supported_languages: Vec<String>,
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
    #[error("no loader found for model: {0}")]
    NoLoaderFound(String),
}

/// Registry of engine loaders for creating STT engines.
///
/// This follows Dependency Inversion - the application layer depends on
/// this abstraction, not concrete engine types.
pub struct EngineRegistry {
    loaders: Vec<Box<dyn EngineLoader>>,
}

impl EngineRegistry {
    pub fn new() -> Self {
        Self {
            loaders: Vec::new(),
        }
    }

    /// Register a loader for a specific engine type.
    pub fn register(&mut self, loader: Box<dyn EngineLoader>) {
        tracing::debug!("Registering engine loader: {}", loader.name());
        self.loaders.push(loader);
    }

    /// Find a loader that can handle the given model ID.
    pub fn find_loader(&self, model_id: &str) -> Option<&dyn EngineLoader> {
        self.loaders
            .iter()
            .find(|l| l.can_load(model_id))
            .map(|l| l.as_ref())
    }

    /// Check if any loader can handle the given model ID.
    pub fn can_load(&self, model_id: &str) -> bool {
        self.loaders.iter().any(|l| l.can_load(model_id))
    }

    /// Check if the model supports streaming inference.
    pub fn is_streaming(&self, model_id: &str) -> bool {
        self.loaders
            .iter()
            .find(|l| l.can_load(model_id))
            .map(|l| l.is_streaming(model_id))
            .unwrap_or(false)
    }
}

impl Default for EngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ModelService;

impl ModelService {
    pub fn list_available_models() -> Vec<ModelInfo> {
        use SttModel::*;
        // Include Whisper ONNX, Parakeet, Sherpa streaming, and NeMo CTC models
        let models = [
            WhisperOnnxSmall,
            ParakeetTdt,
            SherpaZipformerEn,
            NemoConformerCatalan,
        ];

        models
            .iter()
            .map(|m| ModelInfo {
                name: m.name().to_string(),
                dir_name: m.dir_name().to_string(),
                is_downloaded: is_downloaded(*m),
                size_bytes: m.size_bytes(),
                supported_languages: m
                    .supported_languages()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            })
            .collect()
    }

    pub fn parse_model_name(name: &str) -> Result<SttModel, ModelError> {
        match name {
            "whisper-onnx-small" => Ok(SttModel::WhisperOnnxSmall),
            "parakeet-tdt" => Ok(SttModel::ParakeetTdt),
            "sherpa-zipformer-en" => Ok(SttModel::SherpaZipformerEn),
            "nemo-conformer-ca" => Ok(SttModel::NemoConformerCatalan),
            _ => Err(ModelError::UnknownModel(name.to_string())),
        }
    }

    pub fn is_model_downloaded(model: SttModel) -> bool {
        is_downloaded(model)
    }

    pub fn get_model_path(model: SttModel) -> PathBuf {
        model_path(model)
    }

    /// Load an engine using the registry (preferred method).
    ///
    /// This decouples engine creation from concrete types, following
    /// the Dependency Inversion principle.
    ///
    /// The `language` parameter is passed to the engine loader and used
    /// by multilingual models (e.g., Whisper). Use "auto" or empty string
    /// for automatic language detection.
    pub fn load_engine_with_registry(
        registry: &EngineRegistry,
        model: SttModel,
        language: &str,
    ) -> Result<Arc<dyn SttEngine>, ModelError> {
        if !is_downloaded(model) {
            return Err(ModelError::NotDownloaded(model.name().to_string()));
        }

        let model_id = model.name();
        let path = model_path(model);

        tracing::info!(
            model = model_id,
            language = language,
            path = ?path,
            "Loading model"
        );

        let loader = registry
            .find_loader(model_id)
            .ok_or_else(|| ModelError::NoLoaderFound(model_id.to_string()))?;

        let engine = loader
            .load(model_id, &path, language)
            .map_err(|e| ModelError::LoadFailed(e.to_string()))?;

        tracing::info!("Model loaded: {} (via {})", model_id, loader.name());
        Ok(Arc::from(engine))
    }
}

/// Create a registry with all available engine loaders.
///
/// This is called at plugin initialization to wire up all concrete
/// engine implementations.
pub fn create_default_registry() -> EngineRegistry {
    use gibberish_parakeet::ParakeetTdtLoader;
    use gibberish_sherpa::{SherpaNemoCtcLoader, SherpaWhisperLoader, SherpaZipformerLoader};

    let mut registry = EngineRegistry::new();
    registry.register(Box::new(SherpaZipformerLoader));
    registry.register(Box::new(SherpaWhisperLoader));
    registry.register(Box::new(SherpaNemoCtcLoader));
    registry.register(Box::new(ParakeetTdtLoader));
    registry
}
