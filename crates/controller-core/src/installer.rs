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
fn parse_registry(source: &str) -> Result<(Vec<Value>, usize, usize), String> {
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
        if item.get("status").and_then(Value::as_bool).is_none() {
            return Err("Invalid plugin status".into());
        }
        item.get("name").and_then(Value::as_str).ok_or("Invalid plugin entry")?;
    }
    Ok((entries, marker.end(), marker.end() + end - 1))
}

fn append_entry(source: &str) -> Result<String, String> {
    let (entries, _, close) = parse_registry(source)?;
    for item in &entries {
        let name = item.get("name").and_then(Value::as_str).ok_or("Invalid plugin entry")?;
        if name.eq_ignore_ascii_case("ControllerAdder") { return Err("ControllerAdder is already registered; refusing duplicate installation".into()); }
    }
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
    let (engine, plugin, registry, backup, staged) = locate(Path::new(&profile.game_path))?;
    if profile.engine != engine { return Err(format!("Profile engine does not match game: {engine}")); }
    let plugins = plugin.parent().ok_or("Invalid plugin path")?;
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
    Ok(InstallReport { engine, plugin, registry, backup })
}


fn locate(game_path: &Path) -> Result<(String, PathBuf, PathBuf, PathBuf, PathBuf), String> {
    let game = fs::canonicalize(game_path).map_err(|e| format!("Invalid game directory: {e}"))?;
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
    Ok((engine.to_string(), plugin, registry, backup, staged))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchState { NotInstalled, Installed, Disabled, Incomplete, Conflict }

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub engine: String,
    pub state: PatchState,
    pub plugin: PathBuf,
    pub registry: PathBuf,
    pub backup: PathBuf,
    pub detail: String,
}

fn is_ours(item: &Value) -> bool {
    item.get("name").and_then(Value::as_str)
        .map_or(false, |name| name.eq_ignore_ascii_case("ControllerAdder"))
}

// Recognize the exact generated runtime, including its embedded mapping.
// A name or identifying comment alone is not enough to authorize deletion.
fn recognized_plugin(source: &str) -> bool {
    let template = include_str!("controller_adder.js");
    let Some((prefix, suffix)) = template.split_once("__CONTROLLER_ADDER_BINDINGS__") else { return false; };
    let Some(literal) = source.strip_prefix(prefix).and_then(|s| s.strip_suffix(suffix)) else { return false; };
    let Ok(data) = serde_json::from_str::<String>(literal) else { return false; };
    let Ok(bindings) = serde_json::from_str::<std::collections::BTreeMap<String, Vec<crate::mapping::PhysicalInput>>>(&data) else { return false; };
    let profile = MappingProfile {
        format_version: crate::mapping::FORMAT_VERSION,
        engine: "RPG Maker MZ".into(),
        game_path: ".".into(),
        bindings,
    };
    generate_plugin(&profile).map_or(false, |generated| generated == source)
}

pub fn patch_status(game_path: &Path) -> Result<StatusReport, String> {
    let (engine, plugin, registry, backup, staged) = locate(game_path)?;
    let mut report = StatusReport { engine, state: PatchState::Conflict, plugin, registry, backup, detail: String::new() };
    let assessment = (|| -> Result<(PatchState, &'static str), String> {
        if exists(&staged)? { return Err("A pending registry transaction exists".into()); }
        let source = fs::read_to_string(&report.registry).map_err(|e| e.to_string())?;
        let (entries, _, _) = parse_registry(&source)?;
        let matches: Vec<_> = entries.iter().filter(|item| is_ours(item)).collect();
        if matches.len() > 1 { return Err("Duplicate ControllerAdder entries".into()); }
        let mut file_found = false;
        for entry in fs::read_dir(report.plugin.parent().ok_or("Invalid plugin path")?).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_name().to_string_lossy().eq_ignore_ascii_case("ControllerAdder.js") {
                if entry.file_name() != std::ffi::OsStr::new("ControllerAdder.js") || file_found {
                    return Err("Conflicting plugin filename".into());
                }
                plain(&entry.path(), false)?;
                file_found = true;
            }
        }
        let has_backup = exists(&report.backup)?;
        if !file_found && matches.is_empty() && !has_backup {
            return Ok((PatchState::NotInstalled, "No Controller Adder installation"));
        }
        if file_found && !recognized_plugin(&fs::read_to_string(&report.plugin).map_err(|e| e.to_string())?) {
            return Err("Plugin contents do not match the supported generated runtime".into());
        }
        if has_backup {
            plain(&report.backup, false)?;
            let original = fs::read_to_string(&report.backup).map_err(|e| e.to_string())?;
            if parse_registry(&original)?.0.iter().any(is_ours) {
                return Err("Backup already contains a ControllerAdder entry".into());
            }
        }
        if !file_found || matches.is_empty() {
            return Ok((PatchState::Incomplete, "Plugin, registration or cleanup is incomplete"));
        }
        if !has_backup { return Err("Original registry backup is missing".into()); }
        if matches[0].get("name").and_then(Value::as_str) != Some("ControllerAdder") {
            return Err("Unexpected registry entry casing".into());
        }
        if matches[0].get("status").and_then(Value::as_bool) == Some(true) {
            Ok((PatchState::Installed, "Recognized plugin is registered and enabled"))
        } else {
            Ok((PatchState::Disabled, "Recognized plugin is registered but disabled"))
        }
    })();
    match assessment {
        Ok((state, detail)) => { report.state = state; report.detail = detail.into(); }
        Err(error) => report.detail = error,
    }
    Ok(report)
}

// Locate individual JSON entries without reserializing other plugin objects.
fn remove_entry(source: &str) -> Result<String, String> {
    let (entries, open, close) = parse_registry(source)?;
    let indexes: Vec<_> = entries.iter().enumerate().filter(|(_, v)| is_ours(v)).map(|(i, _)| i).collect();
    if indexes.len() != 1 { return Err("Expected one ControllerAdder registry entry".into()); }
    let target = indexes[0];
    let mut cursor = open + 1;
    let mut ranges = Vec::new();
    for (i, _) in entries.iter().enumerate() {
        cursor += source[cursor..close].len() - source[cursor..close].trim_start().len();
        let start = cursor;
        let mut parser = serde_json::Deserializer::from_str(&source[start..close]).into_iter::<Value>();
        parser.next().ok_or("Missing registry entry")?.map_err(|e| e.to_string())?;
        cursor = start + parser.byte_offset();
        ranges.push((start, cursor));
        if i + 1 < entries.len() {
            cursor += source[cursor..close].len() - source[cursor..close].trim_start().len();
            if source.as_bytes().get(cursor) != Some(&b',') { return Err("Missing entry separator".into()); }
            cursor += 1;
        }
    }
    let (start, end) = if target + 1 < ranges.len() {
        (ranges[target].0, ranges[target + 1].0)
    } else if target > 0 {
        (ranges[target - 1].1, ranges[target].1)
    } else { ranges[target] };
    let updated = format!("{}{}", &source[..start], &source[end..]);
    let remaining = parse_registry(&updated)?.0;
    let expected: Vec<_> = entries.into_iter().filter(|v| !is_ours(v)).collect();
    if remaining != expected { return Err("Registry verification failed".into()); }
    Ok(updated)
}

#[derive(Debug, Serialize)]
pub struct UninstallReport {
    pub registry: PathBuf,
    pub removed_plugin: PathBuf,
    pub original_backup_archive: PathBuf,
    pub pre_uninstall_backup: PathBuf,
}

pub fn uninstall_plugin(game_path: &Path) -> Result<UninstallReport, String> {
    let status = patch_status(game_path)?;
    if !matches!(status.state, PatchState::Installed | PatchState::Disabled) {
        return Err(format!("Cannot safely uninstall: {:?}: {}", status.state, status.detail));
    }
    let (_, plugin, registry, backup, staged) = locate(game_path)?;
    let original = fs::read(&registry).map_err(|e| e.to_string())?;
    let updated = remove_entry(std::str::from_utf8(&original).map_err(|e| e.to_string())?)?;
    let plugin_bytes = fs::read(&plugin).map_err(|e| e.to_string())?;
    if !recognized_plugin(std::str::from_utf8(&plugin_bytes).map_err(|e| e.to_string())?) {
        return Err("Plugin changed before uninstall".into());
    }
    let backup_bytes = fs::read(&backup).map_err(|e| e.to_string())?;
    if parse_registry(std::str::from_utf8(&backup_bytes).map_err(|e| e.to_string())?)?.0.iter().any(is_ours) {
        return Err("Backup contains ControllerAdder".into());
    }
    let permissions = fs::metadata(&registry).map_err(|e| e.to_string())?.permissions();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let js = registry.parent().ok_or("Invalid registry path")?;
    let archive = js.join(format!("plugins.js.controller-adder.original-{stamp}.bak"));
    let before = js.join(format!("plugins.js.controller-adder.pre-uninstall-{stamp}.bak"));
    // Exclusive staging prevents concurrent uninstall operations.
    create_file(&staged, updated.as_bytes(), Some(permissions.clone()))?;
    let preparation = (|| -> Result<(), String> {
        create_file(&archive, &backup_bytes, Some(permissions.clone()))?;
        create_file(&before, &original, Some(permissions))?;
        if fs::read(&registry).map_err(|e| e.to_string())? != original
            || fs::read(&plugin).map_err(|e| e.to_string())? != plugin_bytes
            || fs::read(&backup).map_err(|e| e.to_string())? != backup_bytes {
            return Err("Game files changed during uninstall".into());
        }
        fs::rename(&staged, &registry).map_err(|e| format!("Cannot replace registry: {e}"))?;
        Ok(())
    })();
    if let Err(e) = preparation {
        let _ = fs::remove_file(&staged);
        return Err(format!("{e}. No plugin was removed; any recovery backups were retained."));
    }
    // Registry first: even if cleanup fails, the game will not load a missing plugin.
    fs::remove_file(&plugin).map_err(|e| format!("Registration removed, but plugin cleanup failed: {e}. Recovery: {}", before.display()))?;
    fs::remove_file(&backup).map_err(|e| format!("Plugin removed, but backup cleanup failed: {e}. Archive: {}", archive.display()))?;
    Ok(UninstallReport { registry, removed_plugin: plugin, original_backup_archive: archive, pre_uninstall_backup: before })
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
            let path = Path::new(&profile.game_path);
            assert_eq!(patch_status(path).unwrap().state, PatchState::Installed);
            let registry = fs::read_to_string(&report.registry).unwrap();
            let modified = registry.replace("\"status\":true", "\"status\":false");
            fs::write(&report.registry, &modified).unwrap();
            assert_eq!(patch_status(path).unwrap().state, PatchState::Disabled);
            let registry = modified.replace("];", ",{\"name\":\"Later\",\"status\":true,\"parameters\":{}}];");
            fs::write(&report.registry, &registry).unwrap();
            let removed = uninstall_plugin(path).unwrap();
            assert!(!report.plugin.exists());
            assert!(removed.original_backup_archive.is_file());
            assert!(removed.pre_uninstall_backup.is_file());
            assert!(fs::read_to_string(&report.registry).unwrap().contains("Later"));
            assert_eq!(patch_status(path).unwrap().state, PatchState::NotInstalled);
            assert!(install_plugin(&profile).is_ok());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
