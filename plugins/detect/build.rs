fn main() {
    tauri_plugin::Builder::new(&[
        "list_installed_applications",
        "list_mic_using_applications",
        "set_ignored_bundle_ids",
        "list_default_ignored_bundle_ids",
    ])
    .build();
}
