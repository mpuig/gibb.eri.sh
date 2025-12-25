use crate::dto::{TurnModelInfoDto, TurnSettingsDto};
use crate::error::{Result, SttError};
use crate::state::{SttState, TurnSettings};
use gibberish_models::{is_turn_model_downloaded, turn_model_path, TurnModel};
use gibberish_smart_turn::SmartTurnV31Cpu;
use std::sync::Arc;
use tauri::{Emitter, Runtime, State};

fn list_available_turn_models() -> Vec<TurnModel> {
    vec![TurnModel::SmartTurnV31Cpu]
}

fn parse_turn_model_name(name: &str) -> std::result::Result<TurnModel, String> {
    match name {
        "smart-turn-v3.1-cpu" => Ok(TurnModel::SmartTurnV31Cpu),
        other => Err(format!("unknown turn model: {other}")),
    }
}

#[tauri::command]
pub async fn list_turn_models() -> Vec<TurnModelInfoDto> {
    list_available_turn_models()
        .into_iter()
        .map(|m| TurnModelInfoDto {
            name: m.name().to_string(),
            dir_name: m.dir_name().to_string(),
            is_downloaded: is_turn_model_downloaded(m),
            size_bytes: m.size_bytes(),
        })
        .collect()
}

#[tauri::command]
pub async fn get_current_turn_model(state: State<'_, Arc<SttState>>) -> Result<Option<String>> {
    Ok(state
        .get_current_turn_model()
        .await
        .map(|m| m.name().to_string()))
}

#[tauri::command]
pub async fn get_turn_settings(state: State<'_, Arc<SttState>>) -> Result<TurnSettingsDto> {
    let s = state.get_turn_settings().await;
    Ok(TurnSettingsDto {
        enabled: s.enabled,
        threshold: s.threshold,
    })
}

#[tauri::command]
pub async fn set_turn_settings(
    state: State<'_, Arc<SttState>>,
    enabled: bool,
    threshold: f32,
) -> Result<TurnSettingsDto> {
    let threshold = threshold.clamp(0.0, 1.0);
    let settings = TurnSettings { enabled, threshold };
    state.set_turn_settings(settings).await;
    Ok(TurnSettingsDto { enabled, threshold })
}

#[tauri::command]
pub async fn is_turn_downloading(
    state: State<'_, Arc<SttState>>,
    model_name: String,
) -> Result<bool> {
    Ok(state.has_turn_download(&model_name).await)
}

#[tauri::command]
pub async fn cancel_turn_download(
    state: State<'_, Arc<SttState>>,
    model_name: String,
) -> Result<()> {
    if state.cancel_turn_download(&model_name).await {
        tracing::info!("Turn model download cancelled for: {}", model_name);
        Ok(())
    } else {
        Err(SttError::NotDownloading)
    }
}

#[tauri::command]
pub async fn download_turn_model<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, Arc<SttState>>,
    model_name: String,
) -> Result<String> {
    let model = parse_turn_model_name(&model_name)
        .map_err(|e| SttError::InvalidModelName(e.to_string()))?;

    if is_turn_model_downloaded(model) {
        let path = turn_model_path(model);
        return Ok(path.to_string_lossy().to_string());
    }

    if state.has_turn_download(&model_name).await {
        return Err(SttError::DownloadInProgress);
    }

    let cancel_token = state.start_turn_download(model_name.clone()).await;
    tracing::info!("Downloading turn model: {}", model_name);

    let app_handle = app.clone();
    let model_name_for_progress = model_name.clone();
    let cancel_token_for_progress = cancel_token.clone();

    let download_future = gibberish_models::download_turn_model(model, move |downloaded, total| {
        if cancel_token_for_progress.is_cancelled() {
            return;
        }
        let progress = if total > 0 {
            ((downloaded.min(total) as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as u32
        } else {
            0
        };
        let _ = app_handle.emit(
            "stt:turn-download-progress",
            (model_name_for_progress.clone(), progress),
        );
    });

    let result = tokio::select! {
        res = download_future => res,
        _ = cancel_token.cancelled() => {
            Err(gibberish_models::ModelError::DownloadFailed("Cancelled".to_string()))
        }
    };

    state.finish_turn_download(&model_name).await;

    match result {
        Ok(path) => Ok(path.to_string_lossy().to_string()),
        Err(e) => {
            if e.to_string().contains("Cancelled") {
                let dir = turn_model_path(model);
                let _ = std::fs::remove_dir_all(&dir);
            }
            Err(SttError::from(e))
        }
    }
}

#[tauri::command]
pub async fn load_turn_model(state: State<'_, Arc<SttState>>, model_name: String) -> Result<()> {
    let model = parse_turn_model_name(&model_name)
        .map_err(|e| SttError::InvalidModelName(e.to_string()))?;

    if !is_turn_model_downloaded(model) {
        return Err(SttError::Model(format!(
            "turn model not downloaded: {}",
            model.name()
        )));
    }

    let model_path = turn_model_path(model).join(model.local_filename());
    let detector = SmartTurnV31Cpu::load(&model_path)
        .map_err(|e| SttError::Turn(format!("failed to load {}: {}", model.name(), e)))?;

    state.set_turn_detector(std::sync::Arc::new(detector)).await;
    state.set_current_turn_model(model).await;

    Ok(())
}

#[tauri::command]
pub async fn unload_turn_model(state: State<'_, Arc<SttState>>) -> Result<()> {
    state.clear_turn_detector().await;
    state.clear_current_turn_model().await;
    tracing::info!("Turn model unloaded");
    Ok(())
}
