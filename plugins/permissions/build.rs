fn main() {
    tauri_plugin::Builder::new(&[
        "request_microphone",
        "check_microphone",
        "check_screen_recording_permission",
        "request_screen_recording_permission",
        "is_screen_capture_available",
    ])
    .build();
}
