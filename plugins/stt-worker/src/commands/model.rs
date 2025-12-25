use crate::dto::ModelInfoDto;
use crate::error::{Result, SttError};
use crate::services::ModelService;
use crate::state::SttState;
use gibberish_models::model_path;
use gibberish_sherpa::SherpaStreamingEngine;
use std::sync::Arc;
use tauri::{Emitter, Runtime, State};

#[tauri::command]
pub async fn list_models() -> Vec<ModelInfoDto> {
    ModelService::list_available_models()
        .into_iter()
        .map(ModelInfoDto::from)
        .collect()
}

#[tauri::command]
pub async fn download_model<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, Arc<SttState>>,
    model_name: String,
) -> Result<String> {
    let model = ModelService::parse_model_name(&model_name)
        .map_err(|e| SttError::InvalidModelName(e.to_string()))?;

    if ModelService::is_model_downloaded(model) {
        let path = ModelService::get_model_path(model);
        return Ok(path.to_string_lossy().to_string());
    }

    if state.has_download(&model_name).await {
        return Err(SttError::DownloadInProgress);
    }

    let cancel_token = state.start_download(model_name.clone()).await;
    tracing::info!("Downloading model: {}", model_name);

    let app_handle = app.clone();
    let model_name_for_progress = model_name.clone();
    let cancel_token_for_progress = cancel_token.clone();

    let download_future = gibberish_models::download_model(model, move |downloaded, total| {
        if cancel_token_for_progress.is_cancelled() {
            return;
        }
        let progress = if total > 0 {
            ((downloaded.min(total) as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as u32
        } else {
            0
        };
        let _ = app_handle.emit(
            "stt:download-progress",
            (model_name_for_progress.clone(), progress),
        );
    });

    let result = tokio::select! {
        res = download_future => res,
        _ = cancel_token.cancelled() => {
            Err(gibberish_models::ModelError::DownloadFailed("Cancelled".to_string()))
        }
    };

    state.finish_download(&model_name).await;

    match result {
        Ok(path) => {
            tracing::info!("Model downloaded to: {:?}", path);
            Ok(path.to_string_lossy().to_string())
        }
        Err(e) => {
            if e.to_string().contains("Cancelled") {
                let dir = model_path(model);
                let _ = std::fs::remove_dir_all(&dir);
            }
            Err(SttError::from(e))
        }
    }
}

#[tauri::command]
pub async fn cancel_download(state: State<'_, Arc<SttState>>, model_name: String) -> Result<()> {
    if state.cancel_download(&model_name).await {
        tracing::info!("Download cancelled for: {}", model_name);
        Ok(())
    } else {
        Err(SttError::NotDownloading)
    }
}

#[tauri::command]
pub async fn is_downloading(state: State<'_, Arc<SttState>>, model_name: String) -> Result<bool> {
    Ok(state.has_download(&model_name).await)
}

#[tauri::command]
pub async fn load_model(state: State<'_, Arc<SttState>>, model_name: String) -> Result<()> {
    let model = ModelService::parse_model_name(&model_name)
        .map_err(|e| SttError::InvalidModelName(e.to_string()))?;

    // Get language preference from state
    let language = state.get_language().await;

    // Load engine using the registry (Dependency Inversion)
    let registry = state.engine_registry();
    let engine = ModelService::load_engine_with_registry(registry, model, &language)
        .map_err(|e| SttError::Model(e.to_string()))?;

    // Create a non-blocking worker for streaming-capable engines
    // This uses runtime type inspection rather than hardcoded model checks
    if registry.is_streaming(model.name()) {
        if let Some(sherpa) = engine.as_any().downcast_ref::<SherpaStreamingEngine>() {
            match sherpa.create_worker() {
                Ok(worker) => {
                    state.set_sherpa_worker(worker);
                    tracing::info!("Created non-blocking Sherpa worker");
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to create Sherpa worker, falling back to blocking: {}",
                        e
                    );
                }
            }
        }
    }

    state.set_engine(engine).await;
    state.set_current_model(model).await;

    Ok(())
}

#[tauri::command]
pub async fn unload_model(state: State<'_, Arc<SttState>>) -> Result<()> {
    state.clear_sherpa_worker();
    state.clear_engine().await;
    state.clear_current_model().await;
    tracing::info!("Model unloaded");
    Ok(())
}

#[tauri::command]
pub async fn get_current_model(state: State<'_, Arc<SttState>>) -> Result<Option<String>> {
    Ok(state
        .get_current_model()
        .await
        .map(|m| m.name().to_string()))
}

#[tauri::command]
pub async fn get_language(state: State<'_, Arc<SttState>>) -> Result<String> {
    Ok(state.get_language().await)
}

#[tauri::command]
pub async fn set_language(state: State<'_, Arc<SttState>>, language: String) -> Result<()> {
    // Validate language code
    let valid_languages = [
        "auto", "en", "es", "ca", "fr", "de", "it", "pt", "zh", "ja", "ko",
    ];
    if !valid_languages.contains(&language.as_str()) {
        return Err(SttError::InvalidModelName(format!(
            "Invalid language code: {}",
            language
        )));
    }

    state.set_language(language.clone()).await;
    tracing::info!(language = %language, "Language preference updated");

    // If a model is currently loaded, reload it with the new language
    if let Some(model) = state.get_current_model().await {
        tracing::info!(model = %model.name(), "Reloading model with new language");

        // Clear current engine
        state.clear_sherpa_worker();
        state.clear_engine().await;

        // Reload with new language
        let registry = state.engine_registry();
        let engine = ModelService::load_engine_with_registry(registry, model, &language)
            .map_err(|e| SttError::Model(e.to_string()))?;

        // Recreate streaming worker if applicable
        if registry.is_streaming(model.name()) {
            if let Some(sherpa) = engine.as_any().downcast_ref::<SherpaStreamingEngine>() {
                if let Ok(worker) = sherpa.create_worker() {
                    state.set_sherpa_worker(worker);
                }
            }
        }

        state.set_engine(engine).await;
    }

    Ok(())
}
