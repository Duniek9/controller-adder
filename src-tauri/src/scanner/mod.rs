pub mod input;
pub mod javascript;
pub mod parameters;
pub mod plugins;

#[tauri::command]
pub fn detect_engine(path: String) -> Result<String, String> {
    controller_core::detect_engine(std::path::Path::new(&path))
}
