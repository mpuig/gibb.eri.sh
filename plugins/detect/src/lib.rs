use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};
use tokio::sync::Mutex;

mod commands;
mod events;
mod handler;

pub use events::*;

const PLUGIN_NAME: &str = "gibberish-detect";

pub type SharedState = Mutex<State>;

#[derive(Default)]
pub struct State {
    pub(crate) detector: gibberish_detect::Detector,
    pub(crate) ignored_bundle_ids: Vec<String>,
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new(PLUGIN_NAME)
        .invoke_handler(tauri::generate_handler![
            commands::list_installed_applications,
            commands::list_mic_using_applications,
            commands::set_ignored_bundle_ids,
            commands::list_default_ignored_bundle_ids,
        ])
        .setup(move |app, _api| {
            let state = SharedState::default();
            app.manage(state);

            let app_handle = app.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = handler::setup(&app_handle).await {
                    tracing::error!("failed to setup detect handler: {:?}", e);
                }
            });

            Ok(())
        })
        .build()
}
