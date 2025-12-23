fn main() {
    tauri_plugin::Builder::new(&[
        "wikipedia_city_lookup",
        "get_action_router_settings",
        "set_action_router_settings",
        "get_functiongemma_status",
        "cancel_functiongemma_download",
        "list_functiongemma_models",
        "download_functiongemma_model",
        "load_functiongemma_model",
        "unload_functiongemma_model",
        "get_current_functiongemma_model",
    ])
    .build();
}
