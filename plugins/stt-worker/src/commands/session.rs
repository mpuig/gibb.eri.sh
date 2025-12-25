use crate::dto::{SessionDto, SessionSegmentDto, SessionSummaryDto};
use crate::error::{Result, SttError};
use crate::state::SttState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn save_session(
    state: State<'_, Arc<SttState>>,
    segments: Vec<SessionSegmentDto>,
    duration_ms: u64,
    title: Option<String>,
) -> Result<String> {
    use chrono::Utc;
    use gibberish_transcript::{Segment, Transcript, TranscriptRepository};
    use uuid::Uuid;

    let db = state
        .get_database()
        .await
        .ok_or(SttError::DatabaseNotInitialized)?;

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
    db.save(&transcript)?;
    tracing::info!("Session saved: {}", id);

    Ok(id)
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, Arc<SttState>>) -> Result<Vec<SessionSummaryDto>> {
    use gibberish_transcript::TranscriptRepository;

    let db = state
        .get_database()
        .await
        .ok_or(SttError::DatabaseNotInitialized)?;

    let transcripts = db.list()?;

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
pub async fn get_session(state: State<'_, Arc<SttState>>, id: String) -> Result<SessionDto> {
    use gibberish_transcript::TranscriptRepository;
    use uuid::Uuid;

    let db = state
        .get_database()
        .await
        .ok_or(SttError::DatabaseNotInitialized)?;

    let uuid = Uuid::parse_str(&id)?;
    let transcript = db.get(&uuid)?;

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
pub async fn delete_session(state: State<'_, Arc<SttState>>, id: String) -> Result<()> {
    use gibberish_transcript::TranscriptRepository;
    use uuid::Uuid;

    let db = state
        .get_database()
        .await
        .ok_or(SttError::DatabaseNotInitialized)?;

    let uuid = Uuid::parse_str(&id)?;
    db.delete(&uuid)?;
    tracing::info!("Session deleted: {}", id);

    Ok(())
}

#[tauri::command]
pub async fn update_session_title(
    state: State<'_, Arc<SttState>>,
    id: String,
    title: String,
) -> Result<()> {
    use chrono::Utc;
    use gibberish_transcript::TranscriptRepository;
    use uuid::Uuid;

    let db = state
        .get_database()
        .await
        .ok_or(SttError::DatabaseNotInitialized)?;

    let uuid = Uuid::parse_str(&id)?;
    let mut transcript = db.get(&uuid)?;

    transcript.title = Some(title);
    transcript.updated_at = Utc::now();

    db.save(&transcript)?;
    tracing::info!("Session title updated: {}", id);

    Ok(())
}

#[tauri::command]
pub async fn search_sessions(
    state: State<'_, Arc<SttState>>,
    query: String,
) -> Result<Vec<SessionSummaryDto>> {
    use gibberish_transcript::TranscriptRepository;

    let db = state
        .get_database()
        .await
        .ok_or(SttError::DatabaseNotInitialized)?;

    let transcripts = db.list()?;
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
