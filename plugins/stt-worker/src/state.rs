//! Plugin state management for the STT worker.

use crate::audio_listener::AudioListenerHandle;
use crate::download_tracker::DownloadTracker;
use crate::services::{create_default_registry, EngineRegistry};
use gibberish_application::StreamingTranscriber;
use gibberish_models::SttModel;
use gibberish_models::TurnModel;
use gibberish_sherpa::SherpaWorker;
use gibberish_storage::Database;
use gibberish_stt::SttEngine;
use gibberish_turn::TurnDetector;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy)]
pub struct TurnSettings {
    pub enabled: bool,
    pub threshold: f32,
}

impl Default for TurnSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 0.5,
        }
    }
}

/// Application state for the STT plugin
pub struct SttState {
    /// Registry of engine loaders for creating STT engines.
    engine_registry: EngineRegistry,
    /// The loaded transcription engine
    engine: RwLock<Option<Arc<dyn SttEngine>>>,
    /// Currently loaded model identifier
    current_model: RwLock<Option<SttModel>>,
    /// Language code for transcription (e.g., "en", "es", "ca", or "auto")
    language: RwLock<String>,
    /// STT model download tracker
    stt_downloads: DownloadTracker,
    /// Turn model download tracker
    turn_downloads: DownloadTracker,
    /// Streaming transcription state
    streaming: RwLock<StreamingTranscriber>,
    /// Database for transcript storage
    database: RwLock<Option<Arc<Database>>>,
    /// Loaded turn detector (semantic endpoint detection)
    turn_detector: RwLock<Option<Arc<dyn TurnDetector>>>,
    /// Currently loaded turn model
    current_turn_model: RwLock<Option<TurnModel>>,
    /// Turn detection settings
    turn_settings: RwLock<TurnSettings>,
    /// Timestamps (ms) where Smart Turn detected end-of-turn during streaming.
    turn_boundaries_ms: RwLock<Vec<u64>>,
    /// Channel-based Sherpa worker for non-blocking streaming inference.
    /// Uses std::sync::Mutex because SherpaWorker is not Sync (mpsc::Receiver).
    sherpa_worker: std::sync::Mutex<Option<SherpaWorker>>,
    /// Handle to control the audio bus listener task.
    audio_listener_handle: Arc<AudioListenerHandle>,
}

impl Default for SttState {
    fn default() -> Self {
        Self {
            engine_registry: create_default_registry(),
            engine: RwLock::new(None),
            current_model: RwLock::new(None),
            language: RwLock::new("auto".to_string()),
            stt_downloads: DownloadTracker::new(),
            turn_downloads: DownloadTracker::new(),
            streaming: RwLock::new(StreamingTranscriber::new()),
            database: RwLock::new(None),
            turn_detector: RwLock::new(None),
            current_turn_model: RwLock::new(None),
            turn_settings: RwLock::new(TurnSettings::default()),
            turn_boundaries_ms: RwLock::new(Vec::new()),
            sherpa_worker: std::sync::Mutex::new(None),
            audio_listener_handle: Arc::new(AudioListenerHandle::new()),
        }
    }
}

impl SttState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the engine registry for loading STT engines.
    pub fn engine_registry(&self) -> &EngineRegistry {
        &self.engine_registry
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

    // Sherpa worker management (for non-blocking streaming)

    pub fn set_sherpa_worker(&self, worker: SherpaWorker) {
        if let Ok(mut guard) = self.sherpa_worker.lock() {
            *guard = Some(worker);
        }
    }

    pub fn clear_sherpa_worker(&self) {
        if let Ok(mut guard) = self.sherpa_worker.lock() {
            *guard = None;
        }
    }

    /// Check if a streaming-capable worker is available.
    pub fn has_streaming_worker(&self) -> bool {
        self.sherpa_worker
            .lock()
            .ok()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Execute a function with the sherpa worker, if available.
    /// Returns None if no worker is loaded or the lock is poisoned.
    pub fn with_sherpa_worker<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&SherpaWorker) -> R,
    {
        self.sherpa_worker
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(f))
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

    // Language management

    pub async fn get_language(&self) -> String {
        self.language.read().await.clone()
    }

    pub async fn set_language(&self, lang: String) {
        let mut lock = self.language.write().await;
        *lock = lang;
    }

    // STT download management (delegates to DownloadTracker)

    pub async fn has_download(&self, model_name: &str) -> bool {
        self.stt_downloads.has(model_name).await
    }

    pub async fn start_download(&self, model_name: String) -> CancellationToken {
        self.stt_downloads.start(model_name).await
    }

    pub async fn cancel_download(&self, model_name: &str) -> bool {
        self.stt_downloads.cancel(model_name).await
    }

    pub async fn finish_download(&self, model_name: &str) {
        self.stt_downloads.finish(model_name).await
    }

    // Turn model download management (delegates to DownloadTracker)

    pub async fn has_turn_download(&self, model_name: &str) -> bool {
        self.turn_downloads.has(model_name).await
    }

    pub async fn start_turn_download(&self, model_name: String) -> CancellationToken {
        self.turn_downloads.start(model_name).await
    }

    pub async fn cancel_turn_download(&self, model_name: &str) -> bool {
        self.turn_downloads.cancel(model_name).await
    }

    pub async fn finish_turn_download(&self, model_name: &str) {
        self.turn_downloads.finish(model_name).await
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

    // Turn detection

    pub async fn get_turn_detector(&self) -> Option<Arc<dyn TurnDetector>> {
        self.turn_detector.read().await.clone()
    }

    pub async fn set_turn_detector(&self, detector: Arc<dyn TurnDetector>) {
        let mut lock = self.turn_detector.write().await;
        *lock = Some(detector);
    }

    pub async fn clear_turn_detector(&self) {
        let mut lock = self.turn_detector.write().await;
        *lock = None;
    }

    pub async fn get_current_turn_model(&self) -> Option<TurnModel> {
        *self.current_turn_model.read().await
    }

    pub async fn set_current_turn_model(&self, model: TurnModel) {
        let mut lock = self.current_turn_model.write().await;
        *lock = Some(model);
    }

    pub async fn clear_current_turn_model(&self) {
        let mut lock = self.current_turn_model.write().await;
        *lock = None;
    }

    pub async fn get_turn_settings(&self) -> TurnSettings {
        *self.turn_settings.read().await
    }

    pub async fn set_turn_settings(&self, settings: TurnSettings) {
        let mut lock = self.turn_settings.write().await;
        *lock = settings;
    }

    pub async fn clear_turn_boundaries(&self) {
        let mut lock = self.turn_boundaries_ms.write().await;
        lock.clear();
    }

    pub async fn record_turn_boundary(&self, end_ms: u64) {
        let mut lock = self.turn_boundaries_ms.write().await;
        lock.push(end_ms);
        lock.sort_unstable();
        lock.dedup();
    }

    pub async fn get_turn_boundaries(&self) -> Vec<u64> {
        self.turn_boundaries_ms.read().await.clone()
    }

    // Audio listener management

    pub fn audio_listener_handle(&self) -> Arc<AudioListenerHandle> {
        Arc::clone(&self.audio_listener_handle)
    }

    pub fn is_audio_listener_running(&self) -> bool {
        self.audio_listener_handle.is_running()
    }

    pub fn stop_audio_listener(&self) {
        self.audio_listener_handle.stop();
    }
}
