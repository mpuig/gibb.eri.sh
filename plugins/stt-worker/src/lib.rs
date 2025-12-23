mod commands;
mod services;
mod state;

use state::SttState;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use gibberish_application::{StreamingTranscriber, TimedWord};
pub use services::ModelService;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("gibberish-stt")
        .setup(|app, _api| {
            let state = SttState::new();
            app.manage(state);

            // Initialize database in background
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(app_data_dir) = app_handle.path().app_data_dir().ok() {
                    let state = app_handle.state::<SttState>();
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
        ])
        .build()
}
