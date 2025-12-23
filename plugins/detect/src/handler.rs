use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::{DetectEvent, SharedState};

pub fn default_ignored_bundle_ids() -> Vec<String> {
    let gibberish = ["sh.gibber.app", "sh.gibber.dev"];

    let dictation_apps = [
        "com.electron.wispr-flow",
        "com.seewillow.WillowMac",
        "com.superduper.superwhisper",
        "com.prakashjoshipax.VoiceInk",
        "com.goodsnooze.macwhisper",
        "com.descript.beachcube",
        "com.apple.VoiceMemos",
    ];

    let ides = [
        "dev.warp.Warp-Stable",
        "com.exafunction.windsurf",
        "com.microsoft.VSCode",
        "com.todesktop.230313mzl4w4u92",
    ];

    let screen_recording = [
        "so.cap.desktop",
        "com.timpler.screenstudio",
        "com.loom.desktop",
        "com.obsproject.obs-studio",
    ];

    let ai_assistants = ["com.openai.chat", "com.anthropic.claudefordesktop"];

    let other = ["com.raycast.macos", "com.apple.garageband10"];

    dictation_apps
        .into_iter()
        .chain(gibberish)
        .chain(ides)
        .chain(screen_recording)
        .chain(ai_assistants)
        .chain(other)
        .map(String::from)
        .collect()
}

pub async fn setup<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.app_handle().clone();
    let callback = gibberish_detect::new_callback(move |event| {
        let state = app_handle.state::<SharedState>();
        let state_guard = state.blocking_lock();

        let ignored = &state_guard.ignored_bundle_ids;
        let default_ignored = default_ignored_bundle_ids();

        let filtered_event = match event {
            gibberish_detect::DetectEvent::MicStarted(apps) => {
                let filtered: Vec<_> = apps
                    .into_iter()
                    .filter(|app| !ignored.contains(&app.id))
                    .filter(|app| !default_ignored.contains(&app.id))
                    .collect();

                if filtered.is_empty() {
                    tracing::info!(reason = "all_apps_filtered", "skip_notification");
                    return;
                }

                DetectEvent::MicStarted {
                    key: uuid::Uuid::new_v4().to_string(),
                    apps: filtered,
                }
            }
            gibberish_detect::DetectEvent::MicStopped(apps) => {
                let filtered: Vec<_> = apps
                    .into_iter()
                    .filter(|app| !ignored.contains(&app.id))
                    .filter(|app| !default_ignored.contains(&app.id))
                    .collect();

                if filtered.is_empty() {
                    tracing::info!(reason = "all_apps_filtered", "skip_mic_stopped");
                    return;
                }

                DetectEvent::MicStopped { apps: filtered }
            }
        };

        drop(state_guard);

        if let Err(e) = app_handle.emit("detect:event", &filtered_event) {
            tracing::error!("failed to emit detect event: {:?}", e);
        }
    });

    let state = app.state::<SharedState>();
    let mut state_guard = state.lock().await;
    state_guard.detector.start(callback);
    drop(state_guard);

    Ok(())
}
