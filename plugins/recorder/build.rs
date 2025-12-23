fn main() {
    tauri_plugin::Builder::new(&[
        "start_recording",
        "stop_recording",
        "get_recording_state",
        "list_audio_devices",
        "has_virtual_device",
    ])
    .build();
}
