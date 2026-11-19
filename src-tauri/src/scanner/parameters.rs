use controller_core::scanner::parameters::InputEvidence;
use std::path::Path;

#[tauri::command]
pub fn scan_parameters(path: String) -> Result<Vec<InputEvidence>, String> {
    controller_core::scanner::parameters::scan_parameters(Path::new(&path))
}
