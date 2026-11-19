use std::path::Path;

use controller_core::scanner::effective::{analyze_effective_inputs, EffectiveInputReport};

/// Read-only static analysis. Does not modify any game files.
#[tauri::command]
pub fn scan_effective_inputs(game_path: String) -> Result<EffectiveInputReport, String> {
    let trimmed = game_path.trim();
    if trimmed.is_empty() {
        return Err("Choose a game directory first.".to_string());
    }
    let path = Path::new(trimmed);
    if !path.is_dir() {
        return Err(format!("Not a directory: {}", path.display()));
    }
    analyze_effective_inputs(path)
}
