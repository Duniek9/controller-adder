//! Conservative installation of generated RPG Maker controller plugins.
use crate::{mapping::MappingProfile, patch::generate_plugin};
use serde::Serialize;
use serde_json::{Value, json};
use std::{fs, io::Write, path::{Path, PathBuf}};

#[derive(Debug, Serialize)]
pub struct InstallReport {
    pub engine: String,
    pub plugin: PathBuf,
    pub registry: PathBuf,
    pub backup: PathBuf,
}

fn exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("Cannot inspect {}: {e}", path.display())),
    }
}

fn plain(path: &Path, directory: bool) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("Cannot inspect {}: {e}", path.display()))?;
    if metadata.file_type().is_symlink() || (if directory { !metadata.is_dir() } else { !metadata.is_file() }) {
        return Err(format!("Expected a regular {}: {}", if directory { "directory" } else { "file" }, path.display()));
    }
    Ok(())
}

// Accept the standard generated registry, not arbitrary executable JavaScript.
// Preserve the entire original text, inserting only our entry inside the array.
fn append_entry(source: &str) -> Result<String, String> {
    let declaration = regex::Regex::new(r"(?m)^[ \t]*var[ \t]+\$plugins[ \t]*=[ \t\r\n]*").unwrap();
    let marker = declaration.find(source).ok_or("Unsupported plugins.js declaration")?;
    let prefix = source[..marker.start()].trim_start_matches('\u{feff}');
    if prefix.lines().any(|line| {
        let line = line.trim();
        !line.is_empty() && !line.starts_with("//")
    }) { return Err("Unsupported content before plugin registry".into()); }
    let tail = &source[marker.end()..];
    if !tail.starts_with('[') { return Err("Expected plugin array".into()); }
    let mut parser = serde_json::Deserializer::from_str(tail).into_iter::<Vec<Value>>();
    let entries = parser.next().ok_or("Missing plugin array")?
        .map_err(|e| format!("Unsupported plugin registry: {e}"))?;
    let end = parser.byte_offset();
    let suffix = tail[end..].trim();
    if suffix != ";" && !suffix.is_empty() { return Err("Unsupported content after plugin registry".into()); }
    for item in &entries {
        let name = item.get("name").and_then(Value::as_str).ok_or("Invalid plugin entry")?;
        if name.eq_ignore_ascii_case("ControllerAdder") { return Err("ControllerAdder is already registered; refusing duplicate installation".into()); }
    }
    let close = marker.end() + end - 1;
    let entry = json!({"name":"ControllerAdder","status":true,"description":"Native controller support","parameters":{}});
    let separator = if entries.is_empty() { "" } else { "," };
    // Insert before trailing whitespace so the original entries remain untouched.
    let insert = source[..close].trim_end().len();
    let newline = if source.contains("\r\n") { "\r\n" } else { "\n" };
    Ok(format!("{}{separator}{newline}{entry}{}{}", &source[..insert],
        if insert == close { newline } else { "" }, &source[insert..]))
}

fn create_file(path: &Path, bytes: &[u8], permissions: Option<fs::Permissions>) -> Result<(), String> {
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(bytes)?;
        if let Some(permissions) = permissions { file.set_permissions(permissions)?; }
        file.sync_all()
    })();
    if let Err(e) = result {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("Cannot write {}: {e}", path.display()));
    }
    Ok(())
}

pub fn install_plugin(profile: &MappingProfile) -> Result<InstallReport, String> {
    let generated = generate_plugin(profile)?;
    let game = fs::canonicalize(&profile.game_path).map_err(|e| format!("Invalid game directory: {e}"))?;
    let candidates: Vec<_> = [game.clone(), game.join("www")].into_iter().filter_map(|root| {
        if !root.join("data/System.json").is_file() { return None; }
        let engine = match (root.join("js/rmmz_core.js").is_file(), root.join("js/rpg_core.js").is_file()) {
            (true, false) => "RPG Maker MZ",
            (false, true) => "RPG Maker MV",
            _ => return None,
        };
        Some((root, engine))
    }).collect();
    if candidates.len() != 1 { return Err("Expected exactly one supported RPG Maker game root".into()); }
    let (root, engine) = &candidates[0];
    if profile.engine != *engine { return Err(format!("Profile engine does not match game: {engine}")); }
    plain(root, true)?;
    let js = root.join("js");
    let plugins = js.join("plugins");
    plain(&js, true)?;
    plain(&plugins, true)?;
    let registry = js.join("plugins.js");
    plain(&registry, false)?;
    let plugin = plugins.join("ControllerAdder.js");
    let backup = js.join("plugins.js.controller-adder.bak");
    let staged = js.join(".plugins.js.controller-adder.tmp");
    for entry in fs::read_dir(&plugins).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().eq_ignore_ascii_case("ControllerAdder.js") {
            return Err("ControllerAdder.js already exists; refusing overwrite".into());
        }
    }
    for path in [&backup, &staged] {
        if exists(path)? { return Err(format!("Existing backup or staging file: {}. Inspect before retrying.", path.display())); }
    }
    let original = fs::read(&registry).map_err(|e| format!("Cannot read registry: {e}"))?;
    let source = std::str::from_utf8(&original).map_err(|_| "Registry is not UTF-8")?;
    let updated = append_entry(source)?;
    let permissions = fs::metadata(&registry).map_err(|e| e.to_string())?.permissions();
    // The exclusive backup also prevents two installer instances from proceeding.
    create_file(&backup, &original, Some(permissions.clone()))?;
    let mut stage_created = false;
    let mut plugin_created = false;
    let result = (|| -> Result<(), String> {
        create_file(&staged, updated.as_bytes(), Some(permissions))?;
        stage_created = true;
        create_file(&plugin, generated.as_bytes(), None)?;
        plugin_created = true;
        if fs::read(&registry).map_err(|e| e.to_string())? != original {
            return Err("Registry changed during installation; refusing to replace it".into());
        }
        // Same-directory rename replaces the registry only after a complete write.
        fs::rename(&staged, &registry).map_err(|e| format!("Cannot replace registry: {e}"))?;
        Ok(())
    })();
    if let Err(error) = result {
        let mut cleanup = Vec::new();
        if plugin_created {
            if let Err(e) = fs::remove_file(&plugin) { cleanup.push(format!("{}: {e}", plugin.display())); }
        }
        if stage_created {
            if let Err(e) = fs::remove_file(&staged) { cleanup.push(format!("{}: {e}", staged.display())); }
        }
        // Retain the original backup for recovery on any failed installation.
        return Err(format!("{error}. Original backup: {}. Cleanup errors: {}", backup.display(), cleanup.join("; ")));
    }
    Ok(InstallReport { engine: engine.to_string(), plugin, registry, backup })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::{PhysicalInput, FORMAT_VERSION};
    use std::collections::BTreeMap;
    #[test]
    fn registry_preserves_existing_entries() {
        let source = "// Generated\r\nvar $plugins =\r\n[\r\n{\"name\":\"Other\",\"status\":true,\"parameters\":{\"a\":\"]\"}}\r\n];\r\n";
        let next = append_entry(source).unwrap();
        assert!(next.contains("{\"name\":\"Other\",\"status\":true,\"parameters\":{\"a\":\"]\"}}"));
        assert!(next.contains("ControllerAdder"));
        assert!(append_entry(&next).is_err());
        assert!(append_entry("var $plugins = []; alert(1);").is_err());
        assert!(append_entry("var $plugins = [];").is_ok());
    }
    #[test]
    fn installs_and_refuses_duplicate() {
        let root = std::env::temp_dir().join(format!("controller-adder-install-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        for (folder, engine, core) in [("mz", "RPG Maker MZ", "rmmz_core.js"), ("mv/www", "RPG Maker MV", "rpg_core.js")] {
            let game = root.join(folder);
            fs::create_dir_all(game.join("js/plugins")).unwrap();
            fs::create_dir_all(game.join("data")).unwrap();
            fs::write(game.join("data/System.json"), "{}").unwrap();
            fs::write(game.join("js").join(core), "").unwrap();
            let original = "var $plugins = [];\n";
            fs::write(game.join("js/plugins.js"), original).unwrap();
            let profile = MappingProfile { format_version: FORMAT_VERSION, engine: engine.into(),
                game_path: if folder == "mv/www" { root.join("mv") } else { game.clone() }.to_string_lossy().into_owned(),
                bindings: BTreeMap::from([("ok".into(), vec![PhysicalInput::Button { index: 0 }])]) };
            let report = install_plugin(&profile).unwrap();
            assert_eq!(fs::read_to_string(&report.backup).unwrap(), original);
            assert!(report.plugin.is_file());
            assert!(fs::read_to_string(&report.registry).unwrap().contains("ControllerAdder"));
            assert!(install_plugin(&profile).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
