use std::sync::{Arc, Mutex};

use gibberish_bus::{AudioBus, AudioBusConfig, PipelineStatus};
use tauri::Manager;
use tracing_subscriber::EnvFilter;

/// Type alias for the shared pipeline status.
/// Using Arc allows lock-free atomic updates from the audio processing thread.
pub type SharedPipelineStatus = Arc<PipelineStatus>;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,gibberish=debug,ort=error")),
        )
        .init();

    // Suppress verbose ONNX Runtime logging (must be called before any ORT sessions are created)
    let _ = ort::init().commit();
    if let Ok(env) = ort::environment::get_environment() {
        env.set_log_level(ort::logging::LogLevel::Warning);
    }

    tracing::info!("Starting gibberish desktop app");

    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_gibberish_tray::init())
        .plugin(tauri_plugin_gibberish_recorder::init())
        .plugin(tauri_plugin_gibberish_stt::init())
        .plugin(tauri_plugin_gibberish_tools::init())
        .plugin(tauri_plugin_gibberish_permissions::init())
        .plugin(tauri_plugin_gibberish_detect::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Create the audio bus for real-time audio streaming.
            let config = AudioBusConfig {
                capacity_ms: 1500, // 1.5s buffer
                chunk_size_ms: 50, // 50ms chunks
            };
            let mut bus = AudioBus::with_config(config);

            // Manage the sender (cloneable, recorder uses this).
            app.manage(bus.sender());

            // Manage the receiver wrapped in Arc<Mutex<Option<...>>> for restartability.
            // The listener task returns the receiver here when it stops, enabling reuse.
            app.manage(Arc::new(Mutex::new(bus.take_receiver())));

            // Manage pipeline status as Arc for lock-free atomic updates.
            // The audio listener thread shares this Arc and updates metrics atomically.
            app.manage(Arc::new(PipelineStatus::default()));

            tracing::info!("Audio bus initialized");

            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
