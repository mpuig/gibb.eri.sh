use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use gibberish_context::{platform::PlatformProvider, ContextChangedEvent, ContextPoller};
use gibberish_input::{start_panic_hotkey_listener, PanicHotkeyHandle};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Listener, Manager, Runtime};
use tokio::sync::Mutex;

mod adapters;
mod commands;
mod context_injector;
mod deictic;
mod environment;
mod error;
mod executor;
mod functiongemma;
mod functiongemma_download;
mod functiongemma_models;
mod inference;
mod parser;
mod pipeline;
mod policy;
mod prompt_builder;
mod registry;
mod router;
mod router_logic;
mod state;
mod tool_manifest;
mod tools;
mod wikipedia;
mod skill_tool;
mod skill_loader;
mod skill_watcher;
mod tool_pack;
mod tool_pack_loader;

pub use error::{Result, ToolsError};
pub use skill_tool::GenericSkillTool;
pub use skill_loader::{SkillManager, LoadedSkill, ReloadResult};
pub use skill_watcher::{SkillWatcher, get_skill_directories};
pub use tool_pack::{ToolPack, ToolPackTool, CommandDef, PolicyDef, OutputDef};
pub use tool_pack_loader::{ToolPackManager, get_tool_pack_directories};

use adapters::TauriEventBus;
use gibberish_events::event_names;

pub use functiongemma::{FunctionGemmaRunner, ModelOutput, Proposal};

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

/// Wrapper to keep the panic hotkey listener alive for the app's lifetime.
struct PanicHotkeyListenerHandle(PanicHotkeyHandle);

impl std::fmt::Debug for PanicHotkeyListenerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PanicHotkeyListenerHandle")
            .field("running", &self.0.is_running())
            .finish()
    }
}

/// Wrapper to keep the skill watcher alive for the app's lifetime.
struct SkillWatcherHandle(Option<skill_watcher::SkillWatcher>);

impl std::fmt::Debug for SkillWatcherHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillWatcherHandle")
            .field("active", &self.0.is_some())
            .finish()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new(PLUGIN_NAME)
        .invoke_handler(tauri::generate_handler![
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
            commands::reload_skills,
            commands::list_skills,
            commands::reload_tool_packs,
            commands::list_tool_packs,
            commands::reload_all_tools,
        ])
        .setup(|app, _api| {
            // Create shared abort flag for panic hotkey
            let global_abort = Arc::new(AtomicBool::new(false));

            // Start panic hotkey listener (Esc x3)
            let hotkey_handle = start_panic_hotkey_listener(Arc::clone(&global_abort));
            app.manage(PanicHotkeyListenerHandle(hotkey_handle));
            tracing::info!("Panic hotkey listener started (Esc x3 to abort)");

            let event_bus = Arc::new(TauriEventBus::new(app.clone()));
            let state = Arc::new(Mutex::new(state::ToolsState::with_abort_flag(
                event_bus,
                global_abort,
            )));
            app.manage(state.clone());

            // Start skill watcher for hot reloading
            start_skill_watcher(app, state.clone());

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

fn start_skill_watcher<R: Runtime>(app: &tauri::AppHandle<R>, state: SharedState) {
    let directories = skill_watcher::get_skill_directories();

    if directories.is_empty() {
        tracing::warn!("No skill directories to watch");
        app.manage(SkillWatcherHandle(None));
        return;
    }

    // Create callback that reloads skills
    let state_clone = state.clone();
    let callback = move || {
        // Use blocking_lock since we're in a sync callback
        if let Ok(mut guard) = state_clone.try_lock() {
            let result = guard.reload_skills();
            tracing::info!(
                skill_count = result.skill_count,
                tool_count = result.tool_count,
                "Skills hot-reloaded"
            );
        } else {
            tracing::warn!("Failed to acquire lock for skill reload");
        }
    };

    match skill_watcher::SkillWatcher::new(directories, callback) {
        Ok(watcher) => {
            tracing::info!(
                paths = ?watcher.watched_paths(),
                "Skill watcher started"
            );
            app.manage(SkillWatcherHandle(Some(watcher)));
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to start skill watcher");
            app.manage(SkillWatcherHandle(None));
        }
    }
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
                            // Build registry with all tool sources to avoid borrow conflicts
                            let registry = crate::registry::ToolRegistry::build_all_sources(
                                &guard.skills,
                                &guard.tool_packs,
                            );
                            guard.router.update_with_registry(&registry, new_mode);
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
