use crate::services::ModelService;
use crate::state::SttState;
use gibberish_application::TranscriptionService;
use gibberish_models::model_path;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Runtime, State};

// ============================================================================
// Response Types (Serializable DTOs for Tauri IPC)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegmentDto {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Option<i32>,
}

impl From<gibberish_application::TranscriptSegment> for TranscriptSegmentDto {
    fn from(seg: gibberish_application::TranscriptSegment) -> Self {
        Self {
            text: seg.text,
            start_ms: seg.start_ms,
            end_ms: seg.end_ms,
            speaker: seg.speaker,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfoDto {
    pub name: String,
    pub dir_name: String,
    pub is_downloaded: bool,
    pub size_bytes: u64,
}

impl From<crate::services::ModelInfo> for ModelInfoDto {
    fn from(info: crate::services::ModelInfo) -> Self {
        Self {
            name: info.name,
            dir_name: info.dir_name,
            is_downloaded: info.is_downloaded,
            size_bytes: info.size_bytes,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingResultDto {
    pub text: String,
    pub volatile_text: String,
    pub is_partial: bool,
    pub buffer_duration_ms: u64,
}

impl From<gibberish_application::StreamingResult> for StreamingResultDto {
    fn from(result: gibberish_application::StreamingResult) -> Self {
        Self {
            text: result.text,
            volatile_text: result.volatile_text,
            is_partial: result.is_partial,
            buffer_duration_ms: result.buffer_duration_ms,
        }
    }
}

// ============================================================================
// Model Management Commands
// ============================================================================

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
    state: State<'_, SttState>,
    model_name: String,
) -> Result<String, String> {
    use tauri::Emitter;

    let model = ModelService::parse_model_name(&model_name).map_err(|e| e.to_string())?;

    if ModelService::is_model_downloaded(model) {
        let path = ModelService::get_model_path(model);
        return Ok(path.to_string_lossy().to_string());
    }

    if state.has_download(&model_name).await {
        return Err("Download already in progress".to_string());
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
            (downloaded as f64 / total as f64 * 100.0) as u32
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
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn cancel_download(state: State<'_, SttState>, model_name: String) -> Result<(), String> {
    if state.cancel_download(&model_name).await {
        tracing::info!("Download cancelled for: {}", model_name);
        Ok(())
    } else {
        Err("Model is not being downloaded".to_string())
    }
}

#[tauri::command]
pub async fn is_downloading(state: State<'_, SttState>, model_name: String) -> Result<bool, String> {
    Ok(state.has_download(&model_name).await)
}

#[tauri::command]
pub async fn load_model(state: State<'_, SttState>, model_name: String) -> Result<(), String> {
    let model = ModelService::parse_model_name(&model_name).map_err(|e| e.to_string())?;
    let engine = ModelService::load_engine(model).map_err(|e| e.to_string())?;

    state.set_engine(engine).await;
    state.set_current_model(model).await;

    Ok(())
}

#[tauri::command]
pub async fn unload_model(state: State<'_, SttState>) -> Result<(), String> {
    state.clear_engine().await;
    state.clear_current_model().await;
    tracing::info!("Model unloaded");
    Ok(())
}

#[tauri::command]
pub async fn get_current_model(state: State<'_, SttState>) -> Result<Option<String>, String> {
    Ok(state.get_current_model().await.map(|m| m.name().to_string()))
}

// ============================================================================
// Batch Transcription Commands
// ============================================================================

#[tauri::command]
pub async fn transcribe_audio(
    state: State<'_, SttState>,
    audio_samples: Vec<f32>,
) -> Result<Vec<TranscriptSegmentDto>, String> {
    let engine = state
        .get_engine()
        .await
        .ok_or_else(|| "No model loaded".to_string())?;

    let segments =
        TranscriptionService::transcribe_samples(engine.as_ref(), &audio_samples)
            .map_err(|e| e.to_string())?;

    Ok(segments.into_iter().map(TranscriptSegmentDto::from).collect())
}

#[tauri::command]
pub async fn transcribe_file(
    state: State<'_, SttState>,
    file_path: String,
) -> Result<Vec<TranscriptSegmentDto>, String> {
    let engine = state
        .get_engine()
        .await
        .ok_or_else(|| "No model loaded".to_string())?;

    let segments = TranscriptionService::transcribe_file(engine, &file_path)
        .map_err(|e| e.to_string())?;

    Ok(segments.into_iter().map(TranscriptSegmentDto::from).collect())
}

// ============================================================================
// Streaming Transcription Commands
// ============================================================================

#[tauri::command]
pub async fn transcribe_streaming_chunk<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, SttState>,
    audio_chunk: Vec<f32>,
) -> Result<Option<StreamingResultDto>, String> {
    let engine = state.get_engine().await;

    let (result, committed_delta) = state
        .with_streaming_mut(|streamer| {
            let result = TranscriptionService::process_streaming_chunk(streamer, engine, &audio_chunk)?;
            let committed_delta = streamer.take_last_committed_delta();
            Ok::<_, gibberish_application::TranscriptionError>((result, committed_delta))
        })
        .await
        .map_err(|e| e.to_string())?;

    if let Some(delta) = committed_delta {
        #[derive(Clone, Serialize)]
        struct StreamingCommitPayload {
            text: String,
            ts_ms: i64,
        }

        let _ = app.emit(
            "stt:stream_commit",
            StreamingCommitPayload {
                text: delta,
                ts_ms: chrono::Utc::now().timestamp_millis(),
            },
        );
    }

    Ok(Some(StreamingResultDto::from(result)))
}

#[tauri::command]
pub async fn reset_streaming_buffer(state: State<'_, SttState>) -> Result<(), String> {
    state.with_streaming_mut(|s| s.reset()).await;
    tracing::debug!("Streaming state reset");
    Ok(())
}

#[tauri::command]
pub async fn get_streaming_buffer_duration(state: State<'_, SttState>) -> Result<u64, String> {
    Ok(state.with_streaming(|s| s.buffer_duration_ms()).await)
}

// ============================================================================
// Session Management Commands
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummaryDto {
    pub id: String,
    pub title: Option<String>,
    pub created_at: i64,
    pub duration_ms: u64,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDto {
    pub id: String,
    pub title: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub duration_ms: u64,
    pub segments: Vec<SessionSegmentDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSegmentDto {
    pub id: String,
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Option<i32>,
}

#[tauri::command]
pub async fn save_session(
    state: State<'_, SttState>,
    segments: Vec<SessionSegmentDto>,
    duration_ms: u64,
    title: Option<String>,
) -> Result<String, String> {
    use chrono::Utc;
    use gibberish_transcript::{Segment, Transcript, TranscriptRepository};
    use uuid::Uuid;

    let db = state
        .get_database()
        .await
        .ok_or_else(|| "Database not initialized".to_string())?;

    let now = Utc::now();
    let transcript = Transcript {
        id: Uuid::new_v4(),
        title,
        segments: segments
            .into_iter()
            .map(|s| Segment {
                id: Uuid::parse_str(&s.id).unwrap_or_else(|_| Uuid::new_v4()),
                text: s.text,
                start_ms: s.start_ms,
                end_ms: s.end_ms,
                words: Vec::new(),
                speaker: s.speaker,
                is_final: true,
            })
            .collect(),
        created_at: now,
        updated_at: now,
        duration_ms,
    };

    let id = transcript.id.to_string();
    db.save(&transcript).map_err(|e| e.to_string())?;
    tracing::info!("Session saved: {}", id);

    Ok(id)
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, SttState>) -> Result<Vec<SessionSummaryDto>, String> {
    use gibberish_transcript::TranscriptRepository;

    let db = state
        .get_database()
        .await
        .ok_or_else(|| "Database not initialized".to_string())?;

    let transcripts = db.list().map_err(|e| e.to_string())?;

    Ok(transcripts
        .into_iter()
        .map(|t| {
            let preview = t
                .segments
                .first()
                .map(|s| {
                    let text = &s.text;
                    if text.len() > 100 {
                        format!("{}...", &text[..100])
                    } else {
                        text.clone()
                    }
                })
                .unwrap_or_default();

            SessionSummaryDto {
                id: t.id.to_string(),
                title: t.title,
                created_at: t.created_at.timestamp(),
                duration_ms: t.duration_ms,
                preview,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn get_session(state: State<'_, SttState>, id: String) -> Result<SessionDto, String> {
    use gibberish_transcript::TranscriptRepository;
    use uuid::Uuid;

    let db = state
        .get_database()
        .await
        .ok_or_else(|| "Database not initialized".to_string())?;

    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let transcript = db.get(&uuid).map_err(|e| e.to_string())?;

    Ok(SessionDto {
        id: transcript.id.to_string(),
        title: transcript.title,
        created_at: transcript.created_at.timestamp(),
        updated_at: transcript.updated_at.timestamp(),
        duration_ms: transcript.duration_ms,
        segments: transcript
            .segments
            .into_iter()
            .map(|s| SessionSegmentDto {
                id: s.id.to_string(),
                text: s.text,
                start_ms: s.start_ms,
                end_ms: s.end_ms,
                speaker: s.speaker,
            })
            .collect(),
    })
}

#[tauri::command]
pub async fn delete_session(state: State<'_, SttState>, id: String) -> Result<(), String> {
    use gibberish_transcript::TranscriptRepository;
    use uuid::Uuid;

    let db = state
        .get_database()
        .await
        .ok_or_else(|| "Database not initialized".to_string())?;

    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    db.delete(&uuid).map_err(|e| e.to_string())?;
    tracing::info!("Session deleted: {}", id);

    Ok(())
}

#[tauri::command]
pub async fn update_session_title(
    state: State<'_, SttState>,
    id: String,
    title: String,
) -> Result<(), String> {
    use chrono::Utc;
    use gibberish_transcript::TranscriptRepository;
    use uuid::Uuid;

    let db = state
        .get_database()
        .await
        .ok_or_else(|| "Database not initialized".to_string())?;

    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let mut transcript = db.get(&uuid).map_err(|e| e.to_string())?;

    transcript.title = Some(title);
    transcript.updated_at = Utc::now();

    db.save(&transcript).map_err(|e| e.to_string())?;
    tracing::info!("Session title updated: {}", id);

    Ok(())
}

#[tauri::command]
pub async fn search_sessions(
    state: State<'_, SttState>,
    query: String,
) -> Result<Vec<SessionSummaryDto>, String> {
    use gibberish_transcript::TranscriptRepository;

    let db = state
        .get_database()
        .await
        .ok_or_else(|| "Database not initialized".to_string())?;

    let transcripts = db.list().map_err(|e| e.to_string())?;
    let query_lower = query.to_lowercase();

    Ok(transcripts
        .into_iter()
        .filter(|t| {
            let title_match = t
                .title
                .as_ref()
                .map(|title| title.to_lowercase().contains(&query_lower))
                .unwrap_or(false);

            let text_match = t
                .segments
                .iter()
                .any(|s| s.text.to_lowercase().contains(&query_lower));

            title_match || text_match
        })
        .map(|t| {
            let preview = t
                .segments
                .first()
                .map(|s| {
                    let text = &s.text;
                    if text.len() > 100 {
                        format!("{}...", &text[..100])
                    } else {
                        text.clone()
                    }
                })
                .unwrap_or_default();

            SessionSummaryDto {
                id: t.id.to_string(),
                title: t.title,
                created_at: t.created_at.timestamp(),
                duration_ms: t.duration_ms,
                preview,
            }
        })
        .collect())
}
