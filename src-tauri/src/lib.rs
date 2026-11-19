mod scanner;

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
            scanner::javascript::scan_javascript
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
