mod scanner;
mod effective_command;
mod mapping_command;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn export_scan_report(game_path: String, kind: String, report: serde_json::Value) -> Result<String, String> {
    use controller_core::output::{default_path, write_json, OutputKind};
    if !matches!(kind.as_str(), "detect" | "inputs" | "plugins" | "parameters" | "javascript" | "effective" | "mapping" | "resolve") {
        return Err("Unsupported report kind".into());
    }
    let game = std::path::Path::new(&game_path);
    if !game.is_dir() { return Err("Invalid game directory".into()); }
    let path = default_path(OutputKind::Scan, game, &kind)?;
    write_json(&path, &report)?;
    Ok(path.to_string_lossy().into_owned())
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
            export_scan_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
