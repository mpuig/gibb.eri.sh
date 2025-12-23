use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    plugin::{Builder, TauriPlugin},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Runtime,
};

pub struct TrayState {
    _is_recording: AtomicBool,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            _is_recording: AtomicBool::new(false),
        }
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("gibberish-tray")
        .setup(|app, _api| {
            let state = TrayState::default();
            app.manage(Arc::new(state));
            setup_tray(app)?;
            Ok(())
        })
        .build()
}

fn setup_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let start_item = MenuItem::with_id(app, "start", "Start Recording", true, None::<&str>)?;
    let stop_item = MenuItem::with_id(app, "stop", "Stop Recording", true, None::<&str>)?;
    let separator = MenuItem::with_id(app, "sep", "─────────────", false, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&start_item, &stop_item, &separator, &quit_item])?;

    let icon = create_tray_icon(false);

    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
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
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn create_tray_icon(is_recording: bool) -> Image<'static> {
    // For macOS menu bar, icons should be 22x22 (or 44x44 @2x)
    let size = 22;
    let mut rgba = vec![0u8; size * size * 4];

    let color = if is_recording {
        [0xef, 0x44, 0x44, 0xff] // red when recording
    } else {
        [0x3b, 0x82, 0xf6, 0xff] // blue default
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

pub fn set_recording_state<R: Runtime>(
    app: &tauri::AppHandle<R>,
    recording: bool,
) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main") {
        let icon = create_tray_icon(recording);
        tray.set_icon(Some(icon)).map_err(|e| e.to_string())?;
    }

    Ok(())
}
