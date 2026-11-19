use serde::Serialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Serialize)]
pub struct KeyBinding {
    pub key_code: u32,
    pub key_name: String,
}

#[derive(Debug, Serialize)]
pub struct InputAction {
    pub action: String,
    pub keyboard_keys: Vec<KeyBinding>,
    pub source: String,
    pub confidence: String,
}

pub fn scan_inputs(path: String) -> Result<Vec<InputAction>, String> {
    let game_path = Path::new(&path);

    if !game_path.is_dir() {
        return Err("Game directory does not exist.".into());
    }

    let possible_files = [
        game_path.join("js/rmmz_core.js"),
        game_path.join("www/js/rmmz_core.js"),
        game_path.join("js/rpg_core.js"),
        game_path.join("www/js/rpg_core.js"),
    ];

    let core_path = possible_files
        .iter()
        .find(|file| file.is_file())
        .ok_or("Could not find rmmz_core.js")?;

    let content =
        fs::read_to_string(core_path).map_err(|err| format!("Failed to read file: {err}"))?;

    let mapping = extract_key_mapper(&content)?;

    // Key = game action, Value = keyboard bindings
    let mut grouped: BTreeMap<String, Vec<KeyBinding>> = BTreeMap::new();

    for line in mapping.lines() {
        // Remove standard single-line comments.
        let clean = line
            .split("//")
            .next()
            .unwrap_or("")
            .trim()
            .trim_end_matches(',')
            .trim();

        let Some((key, value)) = clean.split_once(':') else {
            continue;
        };

        // Standard MZ uses numeric virtual key codes.
        let Ok(key_code) = key.trim().trim_matches(['"', '\'']).parse::<u32>() else {
            continue;
        };

        let action = value.trim().trim_matches(['"', '\'']).trim();

        if action.is_empty() {
            continue;
        }

        let binding = KeyBinding {
            key_code,
            key_name: key_name(key_code),
        };

        let bindings = grouped.entry(action.to_string()).or_default();

        // Avoid storing the same physical key twice.
        if !bindings.iter().any(|b| b.key_code == key_code) {
            bindings.push(binding);
        }
    }

    if grouped.is_empty() {
        return Err("No keyboard mappings were found.".into());
    }

    let mut actions = Vec::new();

    for (action, mut keyboard_keys) in grouped {
        keyboard_keys.sort_by_key(|key| (key_priority(key.key_code), key.key_code));

        actions.push(InputAction {
            action,
            keyboard_keys,
            source: core_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            confidence: "confirmed_declaration".to_string(),
        });
    }

    Ok(actions)
}

fn extract_key_mapper(content: &str) -> Result<&str, String> {
    // Locate the standard assignment.
    let marker = "Input.keyMapper";

    let start = content
        .find(marker)
        .ok_or("Input.keyMapper was not found")?;

    let remainder = &content[start + marker.len()..];

    let assignment = remainder
        .trim_start()
        .strip_prefix('=')
        .ok_or("Could not read keyMapper assignment")?
        .trim_start();

    let mapping_start = assignment
        .strip_prefix('{')
        .ok_or("Expected a JavaScript object")?;

    let end = mapping_start
        .find('}')
        .ok_or("Could not find end of keyMapper object")?;

    Ok(&mapping_start[..end])
}

fn key_name(code: u32) -> String {
    match code {
        8 => "Backspace".into(),
        9 => "Tab".into(),
        13 => "Enter".into(),
        16 => "Shift".into(),
        17 => "Ctrl".into(),
        18 => "Alt".into(),
        27 => "Escape".into(),
        32 => "Space".into(),
        33 => "Page Up".into(),
        34 => "Page Down".into(),
        35 => "End".into(),
        36 => "Home".into(),
        37 => "Left Arrow".into(),
        38 => "Up Arrow".into(),
        39 => "Right Arrow".into(),
        40 => "Down Arrow".into(),
        45 => "Insert".into(),
        46 => "Delete".into(),

        48..=57 | 65..=90 => char::from_u32(code).unwrap_or('?').to_string(),

        96..=105 => format!("Numpad {}", code - 96),
        112..=123 => format!("F{}", code - 111),

        _ => format!("Key {code}"),
    }
}

fn key_priority(code: u32) -> u8 {
    match code {
        // Common keyboard gameplay controls
        87 | 65 | 83 | 68 => 0, // WASD
        32 => 1,                // Space
        69 | 70 | 82 => 2,      // E, F, R

        // Arrow keys
        37..=40 => 3,

        // Other letter and number keys
        65..=90 | 48..=57 => 4,

        // Enter, Escape, Shift, Ctrl, etc.
        13 | 27 | 16 | 17 => 5,

        // Page Up / Page Down
        33 | 34 => 6,

        // Everything else
        _ => 7,
    }
}
