//! Commands for controlling the audio bus listener.

use std::sync::Arc;

use gibberish_bus::{PipelineStatus, PipelineStatusSnapshot};
use tauri::{Runtime, State};

use crate::audio_listener::{start_audio_listener, ReceiverStorage};
use crate::error::Result;
use crate::state::SttState;

/// Start listening to the audio bus for autonomous STT streaming.
#[tauri::command]
pub async fn stt_start_listening<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, Arc<SttState>>,
    receiver_storage: State<'_, ReceiverStorage>,
    pipeline_status: State<'_, Arc<PipelineStatus>>,
) -> Result<bool> {
    // Check if already running
    if state.is_audio_listener_running() {
        tracing::debug!("Audio listener already running");
        return Ok(false);
    }

    // Try to take the receiver (will be returned when listener stops)
    let receiver = {
        let mut guard = receiver_storage
            .lock()
            .map_err(|_| crate::error::SttError::Transcription("Lock poisoned".to_string()))?;
        guard.take()
    };

    let Some(receiver) = receiver else {
        tracing::warn!("Audio bus receiver already taken or not available");
        return Ok(false);
    };

    // Clone the Arc<SttState> for the listener task - this shares the same state
    let state_arc = Arc::clone(&state);

    // Clone the receiver storage Arc so listener can return it
    let storage_arc = Arc::clone(&receiver_storage);

    // Clone the Arc<PipelineStatus> - this shares the same atomic status
    let pipeline_arc = Arc::clone(&pipeline_status);

    // Get the listener handle
    let handle = state.audio_listener_handle();

    tracing::info!("Starting audio listener");
    start_audio_listener(app, state_arc, receiver, storage_arc, pipeline_arc, handle);

    Ok(true)
}

/// Stop the audio bus listener.
#[tauri::command]
pub fn stt_stop_listening(state: State<'_, Arc<SttState>>) -> Result<bool> {
    if !state.is_audio_listener_running() {
        tracing::debug!("Audio listener not running");
        return Ok(false);
    }

    state.stop_audio_listener();
    tracing::info!("Audio listener stopped");
    Ok(true)
}

/// Check if the audio listener is running.
#[tauri::command]
pub fn stt_is_listening(state: State<'_, Arc<SttState>>) -> bool {
    state.is_audio_listener_running()
}

/// Get current pipeline status (lock-free atomic read).
#[tauri::command]
pub fn stt_get_pipeline_status(
    pipeline_status: State<'_, Arc<PipelineStatus>>,
) -> PipelineStatusSnapshot {
    pipeline_status.snapshot()
}
