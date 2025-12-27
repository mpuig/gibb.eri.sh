fn main() {
    tauri_plugin::Builder::new(&[
        "start_recording",
        "stop_recording",
        "start_listening",
        "stop_listening",
        "promote_to_recording",
        "get_recording_state",
        "list_audio_devices",
        "has_virtual_device",
    ])
    .build();
}
