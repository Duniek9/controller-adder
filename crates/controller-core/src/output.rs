//! Shared paths and non-overwriting export helpers.
use serde::Serialize;
use std::{fs, io::Write, path::{Path, PathBuf}};

#[derive(Debug, Clone, Copy)]
pub enum OutputKind { Scan, Profile, Patch }

fn safe_name(name: &str) -> String {
    let clean: String = name.chars().map(|c| {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }
    }).take(100).collect();
    if clean.is_empty() { "game".into() } else { clean }
}

/// Use the nearest Controller Adder workspace, or the current directory
/// for standalone CLI use. Never embed the build machine's directory.
pub fn output_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    for dir in cwd.ancestors() {
        if dir.join("crates/controller-core/Cargo.toml").is_file()
            && dir.join("crates/controller-cli/Cargo.toml").is_file() {
            return Ok(dir.join("output"));
        }
    }
    Ok(cwd.join("output"))
}

pub fn default_path(kind: OutputKind, game: &Path, label: &str) -> Result<PathBuf, String> {
    let name = safe_name(&game.file_name().unwrap_or_default().to_string_lossy());
    let directory = output_root()?;
    Ok(match kind {
        OutputKind::Scan => directory.join("scans").join(format!("{name}-{}.json", safe_name(label))),
        OutputKind::Profile => directory.join("profiles").join(format!("{name}-profile.json")),
        OutputKind::Patch => directory.join("patches").join(name).join("ControllerAdder.js"),
    })
}

/// Create directories as necessary and never overwrite an existing export.
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
    }
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
    if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("Cannot write {}: {e}", path.display()));
    }
    Ok(())
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    write_new(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn safe_filename() {
        assert_eq!(safe_name("../a b"), "___a_b");
        assert_eq!(safe_name(""), "game");
    }
    #[test]
    fn writes_and_refuses_overwrite() {
        let dir = std::env::temp_dir().join(format!("controller-adder-output-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let file = dir.join("nested/report.json");
        write_json(&file, &vec![1, 2]).unwrap();
        assert!(write_new(&file, b"other").is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "[\n  1,\n  2\n]\n");
        fs::remove_dir_all(dir).unwrap();
    }
}
