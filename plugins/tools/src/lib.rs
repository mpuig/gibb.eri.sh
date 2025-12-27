use std::sync::Arc;

use gibberish_context::{platform::PlatformProvider, ContextChangedEvent, ContextPoller};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Listener, Manager, Runtime};
use tokio::sync::Mutex;

mod adapters;
mod commands;
mod deictic;
mod environment;
mod error;
mod executor;
mod functiongemma;
mod functiongemma_download;
mod functiongemma_models;
mod parser;
mod policy;
mod prompt_builder;
mod registry;
mod router;
mod router_logic;
mod state;
mod tool_manifest;
mod tools;
mod wikipedia;

pub use error::{Result, ToolsError};

use adapters::TauriEventBus;
use gibberish_events::event_names;

pub use functiongemma::{FunctionGemmaRunner, ModelOutput, Proposal};
pub use state::WikiSummaryDto;

const PLUGIN_NAME: &str = "gibberish-tools";

pub type SharedState = Arc<Mutex<state::ToolsState>>;

/// Wrapper to keep the context poller alive for the app's lifetime.
struct ContextPollerHandle(ContextPoller);

impl std::fmt::Debug for ContextPollerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextPollerHandle")
            .field("running", &self.0.is_running())
            .finish()
    }
}

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
            commands::get_context,
            commands::pin_context_mode,
            commands::unpin_context_mode,
            commands::check_input_access,
            commands::request_input_access,
        ])
        .setup(|app, _api| {
            let event_bus = Arc::new(TauriEventBus::new(app.clone()));
            app.manage(Arc::new(Mutex::new(state::ToolsState::new(event_bus))));

            // Start context poller
            start_context_poller(app);

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

fn start_context_poller<R: Runtime>(app: &tauri::AppHandle<R>) {
    let provider = Arc::new(PlatformProvider::new());
    let mut poller = ContextPoller::new();

    let state = app.state::<SharedState>().inner().clone();

    // Get event bus reference upfront (will be cloned for each callback invocation)
    let event_bus = {
        // This blocking_lock is safe during setup - poller hasn't started yet
        let guard = state.blocking_lock();
        Arc::clone(&guard.event_bus)
    };

    let callback: gibberish_context::ContextCallback = Arc::new(
        move |event: ContextChangedEvent| {
            tracing::debug!(mode = %event.mode, app = ?event.active_app, "context changed");

            // Try to acquire lock with retry for critical mode changes
            let mut attempts = 0;
            const MAX_ATTEMPTS: u32 = 3;

            loop {
                attempts += 1;
                match state.try_lock() {
                    Ok(mut guard) => {
                        let prev_mode = guard.context.effective_mode();
                        let new_mode = if guard.context.pinned_mode.is_some() {
                            guard.context.pinned_mode.unwrap()
                        } else {
                            event.mode
                        };

                        guard.context.detected_mode = event.mode;
                        if let Some(ref bundle_id) = event.active_app {
                            guard.context.system.active_app = Some(gibberish_context::AppInfo {
                                bundle_id: bundle_id.clone(),
                                name: event.active_app_name.clone(),
                            });
                        } else {
                            guard.context.system.active_app = None;
                        }
                        guard.context.system.is_mic_active = event.is_meeting;
                        guard.context.system.timestamp_ms = event.timestamp_ms;

                        // Update router manifest if mode changed
                        if prev_mode != new_mode {
                            tracing::info!(prev = %prev_mode, new = %new_mode, "Mode changed, updating router manifest");
                            guard.router.update_for_mode(new_mode);
                        }
                        break;
                    }
                    Err(_) => {
                        if attempts >= MAX_ATTEMPTS {
                            tracing::warn!(
                                mode = %event.mode,
                                attempts,
                                "Context update skipped: state lock contention"
                            );
                            break;
                        }
                        // Brief yield before retry
                        std::thread::sleep(std::time::Duration::from_micros(100));
                    }
                }
            }

            // Always emit event to UI (even if state update was skipped due to lock contention)
            if let Ok(payload) = serde_json::to_value(&event) {
                event_bus.emit(event_names::CONTEXT_CHANGED, payload);
            }
        },
    );

    poller.start(provider, callback);

    // Store the poller handle to keep it alive
    app.manage(ContextPollerHandle(poller));
}
