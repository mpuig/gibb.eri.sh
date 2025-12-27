//! Menu bar tray plugin for gibb.eri.sh.
//!
//! Implements Option C UX pattern:
//! - App lives in menu bar
//! - Click icon shows/hides window
//! - Window visible = listening, hidden = not listening

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    plugin::{Builder, TauriPlugin},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Runtime, WebviewWindow,
};
use tauri_plugin_positioner::{Position, WindowExt};

/// Tray icon states for visual feedback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayIconState {
    /// Default state - app is idle/listening
    Idle,
    /// Actively recording a session
    Recording,
}

pub struct TrayState {
    is_recording: AtomicBool,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            is_recording: AtomicBool::new(false),
        }
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("gibberish-tray")
        .setup(|app, _api| {
            let state = TrayState::default();
            app.manage(Arc::new(state));
            setup_tray(app)?;
            setup_window_focus_handler(app)?;
            hide_dock_icon(app);
            Ok(())
        })
        .build()
}

/// Hide the app from the macOS dock.
fn hide_dock_icon<R: Runtime>(app: &tauri::AppHandle<R>) {
    #[cfg(target_os = "macos")]
    {
        // Hide from dock - app will only appear in menu bar
        // ActivationPolicy::Accessory makes the app a "menu bar only" app
        if let Err(e) = app.set_activation_policy(tauri::ActivationPolicy::Accessory) {
            tracing::warn!("Failed to set activation policy: {}", e);
        }
    }
}

/// Set up the system tray icon with menu bar behavior.
fn setup_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let start_item = MenuItem::with_id(app, "start", "Start Recording", true, None::<&str>)?;
    let stop_item = MenuItem::with_id(app, "stop", "Stop Recording", true, None::<&str>)?;
    let separator = MenuItem::with_id(app, "sep", "─────────────", false, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&start_item, &stop_item, &separator, &quit_item])?;

    let icon = create_tray_icon(TrayIconState::Idle);

    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "start" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.emit("tray:start-recording", ());
                }
            }
            "stop" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.emit("tray:stop-recording", ());
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Forward tray events to positioner for position tracking
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);

            // Handle left-click to toggle window visibility
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    toggle_window_visibility(&window);
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Toggle window visibility with proper positioning.
fn toggle_window_visibility<R: Runtime>(window: &WebviewWindow<R>) {
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        // Emit event when window hides (stop listening)
        let _ = window.emit("tray:window-hidden", ());
    } else {
        // Position window near tray icon before showing
        if let Err(e) = window.move_window(Position::TrayBottomCenter) {
            tracing::warn!("Failed to position window: {}", e);
        }
        let _ = window.show();
        let _ = window.set_focus();
        // Emit event when window shows (start listening)
        let _ = window.emit("tray:window-shown", ());
    }
}

/// Set up handler to hide window when it loses focus.
fn setup_window_focus_handler<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = app.get_webview_window("main") {
        let window_clone = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::Focused(false) = event {
                // Hide window when it loses focus (click outside)
                let _ = window_clone.hide();
                let _ = window_clone.emit("tray:window-hidden", ());
            }
        });
    }
    Ok(())
}

/// Create a tray icon for the given state.
fn create_tray_icon(state: TrayIconState) -> Image<'static> {
    // For macOS menu bar, icons should be 22x22 (or 44x44 @2x)
    let size = 22;
    let mut rgba = vec![0u8; size * size * 4];

    let color = match state {
        TrayIconState::Recording => [0xef, 0x44, 0x44, 0xff], // Red when recording
        TrayIconState::Idle => [0x3b, 0x82, 0xf6, 0xff],      // Blue default (listening)
    };

    let cx = size / 2;
    let cy = size / 2;
    let r = size / 2 - 2;

    for y in 0..size {
        for x in 0..size {
            let dx = x as i32 - cx as i32;
            let dy = y as i32 - cy as i32;
            let dist = ((dx * dx + dy * dy) as f64).sqrt();

            if dist <= r as f64 {
                let idx = (y * size + x) * 4;
                rgba[idx] = color[0];
                rgba[idx + 1] = color[1];
                rgba[idx + 2] = color[2];
                rgba[idx + 3] = color[3];
            }
        }
    }

    Image::new_owned(rgba, size as u32, size as u32)
}

/// Update the tray icon to reflect recording state.
pub fn set_recording_state<R: Runtime>(
    app: &tauri::AppHandle<R>,
    recording: bool,
) -> Result<(), String> {
    if let Some(state) = app.try_state::<Arc<TrayState>>() {
        state.is_recording.store(recording, Ordering::SeqCst);
    }

    if let Some(tray) = app.tray_by_id("main") {
        let icon_state = if recording {
            TrayIconState::Recording
        } else {
            TrayIconState::Idle
        };
        let icon = create_tray_icon(icon_state);
        tray.set_icon(Some(icon)).map_err(|e| e.to_string())?;
    }

    Ok(())
}
