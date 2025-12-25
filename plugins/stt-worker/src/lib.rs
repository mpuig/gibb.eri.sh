mod audio_listener;
mod batch_transcriber;
mod commands;
mod download_tracker;
mod dto;
mod error;
mod services;
mod state;
mod turn_listener;

pub use error::{Result, SttError};

use state::SttState;
use std::sync::Arc;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use gibberish_application::{StreamingTranscriber, TimedWord};
pub use services::ModelService;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("gibberish-stt")
        .setup(|app, _api| {
            // Wrap SttState in Arc for sharing with audio listener task
            let state = Arc::new(SttState::new());
            app.manage(Arc::clone(&state));

            // Start the event-driven turn detection listener
            let cancel_token = tokio_util::sync::CancellationToken::new();
            turn_listener::start_turn_listener(app.clone(), Arc::clone(&state), cancel_token);

            // Initialize database in background
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
                    let state = app_handle.state::<Arc<SttState>>();
                    if let Err(e) = state.init_database(app_data_dir).await {
                        tracing::error!("Failed to initialize database: {}", e);
                    }
                } else {
                    tracing::error!("Failed to get app data directory");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_models,
            commands::download_model,
            commands::cancel_download,
            commands::is_downloading,
            commands::load_model,
            commands::unload_model,
            commands::get_current_model,
            commands::get_language,
            commands::set_language,
            commands::list_turn_models,
            commands::download_turn_model,
            commands::cancel_turn_download,
            commands::is_turn_downloading,
            commands::load_turn_model,
            commands::unload_turn_model,
            commands::get_current_turn_model,
            commands::get_turn_settings,
            commands::set_turn_settings,
            commands::transcribe_audio,
            commands::transcribe_file,
            commands::transcribe_streaming_chunk,
            commands::reset_streaming_buffer,
            commands::get_streaming_buffer_duration,
            commands::save_session,
            commands::list_sessions,
            commands::get_session,
            commands::delete_session,
            commands::update_session_title,
            commands::search_sessions,
            commands::stt_start_listening,
            commands::stt_stop_listening,
            commands::stt_is_listening,
            commands::stt_get_pipeline_status,
        ])
        .build()
}
