use crate::SharedState;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tauri::{Emitter, Manager, Runtime, State};

const FUNCTIONGEMMA_DEFAULT_VARIANT: &str = "model_fp16";

fn functiongemma_base_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("functiongemma"))
}

fn functiongemma_variant_dir<R: Runtime>(
    app: &tauri::AppHandle<R>,
    variant: &str,
) -> Result<PathBuf, String> {
    Ok(functiongemma_base_dir(app)?.join(variant))
}

fn functiongemma_model_paths<R: Runtime>(
    app: &tauri::AppHandle<R>,
    variant: &str,
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let dir = functiongemma_variant_dir(app, variant)?;
    let model_path = dir.join(format!("{variant}.onnx"));
    let data_path = dir.join(format!("{variant}.onnx_data"));
    let tokenizer_path = dir.join("tokenizer.json");
    Ok((model_path, data_path, tokenizer_path))
}

fn functiongemma_is_downloaded_files(model: &Path, data: &Path, tok: &Path) -> bool {
    fn ok(p: &Path) -> bool {
        std::fs::metadata(p)
            .map(|m| m.is_file() && m.len() > 1024)
            .unwrap_or(false)
    }
    ok(model) && ok(data) && ok(tok)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FunctionGemmaStatusDto {
    pub loaded: bool,
    pub loaded_variant: Option<String>,
    pub default_variant: String,
    pub downloading: bool,
    pub download_progress: Option<u32>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub download_file: Option<String>,
    pub last_error: Option<String>,
}

#[tauri::command]
pub async fn get_functiongemma_status<R: Runtime>(
    _app: tauri::AppHandle<R>,
    state: State<'_, SharedState>,
) -> Result<FunctionGemmaStatusDto, String> {
    let default_variant = FUNCTIONGEMMA_DEFAULT_VARIANT.to_string();

    let guard = state.lock().await;
    let (downloading, download_progress, downloaded_bytes, total_bytes, download_file) =
        if let Some(dl) = &guard.functiongemma.download {
            let progress = if dl.total_bytes > 0 {
                Some(
                    (((dl.downloaded_bytes as f64 / dl.total_bytes as f64) * 100.0) as u32)
                        .min(100),
                )
            } else {
                None
            };
            (
                true,
                progress,
                Some(dl.downloaded_bytes),
                Some(dl.total_bytes).filter(|t| *t > 0),
                dl.current_file.clone(),
            )
        } else {
            (false, None, None, None, None)
        };

    Ok(FunctionGemmaStatusDto {
        loaded: guard.functiongemma.model.is_some(),
        loaded_variant: guard
            .functiongemma
            .model
            .as_ref()
            .map(|m| m.variant.clone()),
        default_variant,
        downloading,
        download_progress,
        downloaded_bytes,
        total_bytes,
        download_file,
        last_error: guard.functiongemma.last_error.clone(),
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionGemmaModelInfoDto {
    pub variant: String,
    pub is_downloaded: bool,
    pub size_bytes: u64,
    pub is_downloading: bool,
}

#[tauri::command]
pub async fn list_functiongemma_models<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, SharedState>,
) -> Result<Vec<FunctionGemmaModelInfoDto>, String> {
    let mut out = Vec::new();
    for spec in crate::functiongemma_models::FUNCTIONGEMMA_SPECS {
        let (model, data, tok) = functiongemma_model_paths(&app, spec.variant)?;
        let is_downloaded = functiongemma_is_downloaded_files(&model, &data, &tok);
        let guard = state.lock().await;
        let is_downloading = guard
            .functiongemma
            .download
            .as_ref()
            .map(|d| d.variant == spec.variant)
            .unwrap_or(false);
        drop(guard);
        out.push(FunctionGemmaModelInfoDto {
            variant: spec.variant.to_string(),
            is_downloaded,
            size_bytes: spec.size_bytes,
            is_downloading,
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn download_functiongemma_model<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, SharedState>,
    variant: String,
) -> Result<String, String> {
    if !crate::functiongemma_models::is_supported_variant(&variant) {
        return Err(format!("Unsupported FunctionGemma variant: {variant}"));
    }

    let (model_path, data_path, tokenizer_path) = functiongemma_model_paths(&app, &variant)?;
    if functiongemma_is_downloaded_files(&model_path, &data_path, &tokenizer_path) {
        return Ok(functiongemma_variant_dir(&app, &variant)?
            .to_string_lossy()
            .to_string());
    }

    {
        let guard = state.lock().await;
        if let Some(dl) = &guard.functiongemma.download {
            if dl.variant == variant {
                return Err("Download already in progress".to_string());
            }
            return Err(format!(
                "Another FunctionGemma download is in progress: {}",
                dl.variant
            ));
        }
    }

    let cancel = tokio_util::sync::CancellationToken::new();
    {
        let mut guard = state.lock().await;
        guard.functiongemma.last_error = None;
        guard.functiongemma.download = Some(crate::state::FunctionGemmaDownload {
            variant: variant.clone(),
            downloaded_bytes: 0,
            total_bytes: 0,
            current_file: None,
            started_at: Instant::now(),
            cancel: cancel.clone(),
        });
    }

    let client = { state.lock().await.client.clone() };
    let base_dir = functiongemma_variant_dir(&app, &variant)?;
    tokio::fs::create_dir_all(&base_dir)
        .await
        .map_err(|e| e.to_string())?;

    let plan =
        crate::functiongemma_download::FunctionGemmaDownloadPlan::for_variant(&base_dir, &variant);

    let (tx, mut rx) =
        tokio::sync::mpsc::unbounded_channel::<crate::functiongemma_download::DownloadProgress>();
    let app_for_events = app.clone();
    let variant_for_events = variant.clone();
    let progress_task = tauri::async_runtime::spawn(async move {
        let mut last_emit = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(1))
            .unwrap_or_else(std::time::Instant::now);
        let mut last_progress: Option<u32> = None;
        let mut last_file: Option<String> = None;

        while let Some(p) = rx.recv().await {
            let progress = if p.total_bytes > 0 {
                (((p.downloaded_bytes as f64 / p.total_bytes as f64) * 100.0) as u32).min(100)
            } else if p.file_total_bytes > 0 {
                (((p.file_downloaded_bytes as f64 / p.file_total_bytes as f64) * 100.0) as u32)
                    .min(100)
            } else {
                0
            };

            let file_changed = last_file.as_deref() != Some(p.file.as_str());
            let progress_changed = last_progress.map(|lp| lp != progress).unwrap_or(true);
            let time_ready = last_emit.elapsed() >= std::time::Duration::from_millis(250);

            if file_changed || (progress_changed && time_ready) {
                last_emit = std::time::Instant::now();
                last_progress = Some(progress);
                last_file = Some(p.file.clone());

                let _ = app_for_events.emit(
                    "tools:functiongemma_download_progress",
                    serde_json::json!({
                        "variant": variant_for_events,
                        "file": p.file,
                        "downloaded_bytes": p.downloaded_bytes,
                        "total_bytes": p.total_bytes,
                        "progress": progress,
                        "file_downloaded_bytes": p.file_downloaded_bytes,
                        "file_total_bytes": p.file_total_bytes,
                    }),
                );
            }
        }
    });

    let download_fut =
        crate::functiongemma_download::download_functiongemma(&client, &plan, &cancel, |p| {
            let _ = tx.send(p);
        });

    let result = tokio::select! {
        res = download_fut => res,
        _ = cancel.cancelled() => Err(crate::functiongemma_download::DownloadError::Cancelled),
    };

    drop(tx);
    let _ = progress_task.await;

    match result {
        Ok(_) => {
            {
                let mut guard = state.lock().await;
                guard.functiongemma.download = None;
                guard.functiongemma.last_error = None;
            }
            let _ = app.emit(
                "tools:functiongemma_download_complete",
                serde_json::json!({ "variant": variant }),
            );
            Ok(base_dir.to_string_lossy().to_string())
        }
        Err(err) => {
            let msg = err.to_string();
            {
                let mut guard = state.lock().await;
                guard.functiongemma.download = None;
                guard.functiongemma.last_error = Some(msg.clone());
            }
            let _ = app.emit(
                "tools:functiongemma_download_error",
                serde_json::json!({ "variant": variant, "error": msg }),
            );
            Err(msg)
        }
    }
}

#[tauri::command]
pub async fn cancel_functiongemma_download(
    state: State<'_, SharedState>,
    variant: Option<String>,
) -> Result<bool, String> {
    let guard = state.lock().await;
    if let Some(dl) = &guard.functiongemma.download {
        if variant.as_deref().map(|v| v == dl.variant).unwrap_or(true) {
            dl.cancel.cancel();
            return Ok(true);
        }
    }
    Ok(false)
}

#[tauri::command]
pub async fn load_functiongemma_model<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, SharedState>,
    variant: String,
) -> Result<FunctionGemmaStatusDto, String> {
    if !crate::functiongemma_models::is_supported_variant(&variant) {
        return Err(format!("Unsupported FunctionGemma variant: {variant}"));
    }

    let (model_path, data_path, tokenizer_path) = functiongemma_model_paths(&app, &variant)?;
    if !functiongemma_is_downloaded_files(&model_path, &data_path, &tokenizer_path) {
        return Err("Model not downloaded yet".to_string());
    }

    let model_path_for_load = model_path.clone();
    let tokenizer_path_for_load = tokenizer_path.clone();
    let runner = tokio::task::spawn_blocking(move || {
        crate::functiongemma::FunctionGemmaRunner::load(
            &model_path_for_load,
            &tokenizer_path_for_load,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    {
        let mut guard = state.lock().await;
        guard.functiongemma.last_error = None;
        guard.functiongemma.model = Some(crate::state::FunctionGemmaModel {
            variant: variant.clone(),
            model_path,
            tokenizer_path,
            runner: Arc::new(runner),
        });
    }

    let _ = app.emit(
        "tools:functiongemma_loaded",
        serde_json::json!({ "variant": variant }),
    );

    get_functiongemma_status(app, state).await
}

#[tauri::command]
pub async fn unload_functiongemma_model(state: State<'_, SharedState>) -> Result<(), String> {
    let mut guard = state.lock().await;
    guard.functiongemma.model = None;
    Ok(())
}

#[tauri::command]
pub async fn get_current_functiongemma_model(
    state: State<'_, SharedState>,
) -> Result<Option<String>, String> {
    let guard = state.lock().await;
    Ok(guard
        .functiongemma
        .model
        .as_ref()
        .map(|m| m.variant.clone()))
}
