use controller_core::scanner::plugins::PluginInfo;

#[tauri::command]
pub fn scan_plugins(path: String) -> Result<Vec<PluginInfo>, String> {
    controller_core::scanner::plugins::scan_plugins(path)
}
