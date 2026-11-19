use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Deserialize)]
struct PluginEntry {
    name: String,
    status: bool,

    #[serde(default)]
    description: String,
}

#[derive(Debug, Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub enabled: bool,
    pub description: String,
    pub file_exists: bool,
}

pub fn scan_plugins(path: String) -> Result<Vec<PluginInfo>, String> {
    let game_path = Path::new(&path);

    if !game_path.is_dir() {
        return Err("Game directory does not exist.".into());
    }

    let possible_roots = [game_path.to_path_buf(), game_path.join("www")];

    let root = possible_roots
        .iter()
        .find(|root| root.join("js/plugins.js").is_file())
        .ok_or("Could not find plugins.js")?;

    let plugins_path = root.join("js/plugins.js");

    let content = fs::read_to_string(&plugins_path)
        .map_err(|err| format!("Cannot read plugins.js: {err}"))?;

    // Locate the $plugins array assignment.
    let marker = content
        .find("$plugins")
        .ok_or("Plugin declaration was not found")?;

    let remainder = &content[marker + "$plugins".len()..];

    let assignment = remainder
        .find('=')
        .ok_or("Plugin assignment was not found")?;

    let after_assignment = &remainder[assignment + 1..];

    let array_start = after_assignment
        .find('[')
        .ok_or("Plugin array was not found")?;

    let json = &after_assignment[array_start..];

    // Parse the JSON-compatible JavaScript array.
    // The deserializer reads one complete value,
    // so the trailing semicolon is not included.
    let entries = serde_json::Deserializer::from_str(json)
        .into_iter::<Vec<PluginEntry>>()
        .next()
        .ok_or("Plugin array is empty or invalid")?
        .map_err(|err| format!("Invalid plugin data: {err}"))?;

    let mut plugins = Vec::new();

    for entry in entries {
        // Don't allow a plugin name to escape
        // the game's plugins directory.
        let safe_name = !entry.name.is_empty()
            && !entry.name.contains('/')
            && !entry.name.contains('\\')
            && entry.name != "."
            && entry.name != "..";

        let file_exists = safe_name
            && root
                .join("js/plugins")
                .join(format!("{}.js", entry.name))
                .is_file();

        plugins.push(PluginInfo {
            name: entry.name,
            enabled: entry.status,
            description: entry.description,
            file_exists,
        });
    }

    Ok(plugins)
}
