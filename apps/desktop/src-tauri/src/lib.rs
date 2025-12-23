#[cfg(debug_assertions)]
use tauri::Manager;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,gibberish=debug"))
        )
        .init();

    tracing::info!("Starting gibberish desktop app");

    tauri::Builder::default()
        .plugin(tauri_plugin_gibberish_tray::init())
        .plugin(tauri_plugin_gibberish_recorder::init())
        .plugin(tauri_plugin_gibberish_stt::init())
        .plugin(tauri_plugin_gibberish_tools::init())
        .plugin(tauri_plugin_gibberish_permissions::init())
        .plugin(tauri_plugin_gibberish_detect::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|_app| {
            #[cfg(debug_assertions)]
            {
                let window = _app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
