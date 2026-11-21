mod scanner;
mod effective_command;
mod mapping_command;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            scanner::detect_engine,
            scanner::input::scan_inputs,
            scanner::plugins::scan_plugins,
            scanner::parameters::scan_parameters,
            scanner::javascript::scan_javascript,
            effective_command::scan_effective_inputs,
            mapping_command::scan_mapping_actions,
            mapping_command::suggest_controller_mapping,
            mapping_command::validate_controller_mapping,
            mapping_command::export_mapping_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
