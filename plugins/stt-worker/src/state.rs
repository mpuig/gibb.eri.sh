use gibberish_application::StreamingTranscriber;
use gibberish_models::SttModel;
use gibberish_storage::Database;
use gibberish_stt::SttEngine;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Application state for the STT plugin
pub struct SttState {
    /// The loaded transcription engine
    engine: RwLock<Option<Arc<dyn SttEngine>>>,
    /// Currently loaded model identifier
    current_model: RwLock<Option<SttModel>>,
    /// Active download cancellation tokens
    download_tokens: RwLock<HashMap<String, CancellationToken>>,
    /// Streaming transcription state
    streaming: RwLock<StreamingTranscriber>,
    /// Database for transcript storage
    database: RwLock<Option<Arc<Database>>>,
}

impl Default for SttState {
    fn default() -> Self {
        Self {
            engine: RwLock::new(None),
            current_model: RwLock::new(None),
            download_tokens: RwLock::new(HashMap::new()),
            streaming: RwLock::new(StreamingTranscriber::new()),
            database: RwLock::new(None),
        }
    }
}

impl SttState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn init_database(&self, app_data_dir: PathBuf) -> Result<(), String> {
        let db_path = app_data_dir.join("gibberish.db");

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let db = Database::open(&db_path).map_err(|e| e.to_string())?;
        let mut lock = self.database.write().await;
        *lock = Some(Arc::new(db));
        tracing::info!("Database initialized at {:?}", db_path);
        Ok(())
    }

    pub async fn get_database(&self) -> Option<Arc<Database>> {
        self.database.read().await.clone()
    }
}

impl SttState {
    // Engine management

    pub async fn get_engine(&self) -> Option<Arc<dyn SttEngine>> {
        self.engine.read().await.clone()
    }

    pub async fn set_engine(&self, engine: Arc<dyn SttEngine>) {
        let mut lock = self.engine.write().await;
        *lock = Some(engine);
    }

    pub async fn clear_engine(&self) {
        let mut lock = self.engine.write().await;
        *lock = None;
    }

    // Model management

    pub async fn get_current_model(&self) -> Option<SttModel> {
        *self.current_model.read().await
    }

    pub async fn set_current_model(&self, model: SttModel) {
        let mut lock = self.current_model.write().await;
        *lock = Some(model);
    }

    pub async fn clear_current_model(&self) {
        let mut lock = self.current_model.write().await;
        *lock = None;
    }

    // Download token management

    pub async fn has_download(&self, model_name: &str) -> bool {
        self.download_tokens.read().await.contains_key(model_name)
    }

    pub async fn start_download(&self, model_name: String) -> CancellationToken {
        let token = CancellationToken::new();
        let mut tokens = self.download_tokens.write().await;
        tokens.insert(model_name, token.clone());
        token
    }

    pub async fn cancel_download(&self, model_name: &str) -> bool {
        let tokens = self.download_tokens.read().await;
        if let Some(token) = tokens.get(model_name) {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub async fn finish_download(&self, model_name: &str) {
        let mut tokens = self.download_tokens.write().await;
        tokens.remove(model_name);
    }

    // Streaming transcription

    pub async fn with_streaming<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&StreamingTranscriber) -> R,
    {
        let streaming = self.streaming.read().await;
        f(&streaming)
    }

    pub async fn with_streaming_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut StreamingTranscriber) -> R,
    {
        let mut streaming = self.streaming.write().await;
        f(&mut streaming)
    }
}
