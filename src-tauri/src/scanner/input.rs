use controller_core::scanner::input::InputAction;

#[tauri::command]
pub fn scan_inputs(path: String) -> Result<Vec<InputAction>, String> {
    controller_core::scanner::input::scan_inputs(path)
}
