use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Listener, Manager, Runtime};
use tokio::sync::Mutex;

mod commands;
mod state;
mod wikipedia;
mod router;
mod functiongemma;
mod functiongemma_download;
mod functiongemma_models;

pub use state::WikiSummaryDto;

const PLUGIN_NAME: &str = "gibberish-tools";

pub type SharedState = Mutex<state::ToolsState>;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new(PLUGIN_NAME)
        .invoke_handler(tauri::generate_handler![
            commands::wikipedia_city_lookup,
            commands::get_action_router_settings,
            commands::set_action_router_settings,
            commands::get_functiongemma_status,
            commands::cancel_functiongemma_download,
            commands::list_functiongemma_models,
            commands::download_functiongemma_model,
            commands::load_functiongemma_model,
            commands::unload_functiongemma_model,
            commands::get_current_functiongemma_model,
        ])
        .setup(|app, _api| {
            app.manage(SharedState::default());

            let app_handle = app.app_handle().clone();
            app.listen_any("stt:stream_commit", move |event| {
                let payload = event.payload();
                if payload.trim().is_empty() {
                    return;
                }
                router::on_stt_stream_commit(&app_handle, payload);
            });
            Ok(())
        })
        .build()
}
