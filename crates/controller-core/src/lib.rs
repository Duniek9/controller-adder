pub mod engines;
pub mod mapping;
pub mod patch;
pub mod scanner;

use std::path::Path;

/// Detects which game engine is used.
///
/// Returns None when no supported engine is found.
pub fn detect_engine(path: &Path) -> Result<String, String> {
    if !path.is_dir() {
        return Err("Selected directory does not exist.".into());
    }

    if let Some(engine) = engines::rpg_maker::detect(path) {
        return Ok(engine.to_string());
    }

    Ok("Unknown engine".to_string())
}
