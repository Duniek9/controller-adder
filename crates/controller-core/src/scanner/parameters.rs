use serde::Serialize;
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize)]
pub struct InputEvidence {
    pub plugin: String,
    pub parameter: String,
    pub value: String,
    pub kind: String,
    pub confidence: String,
}

const MAX_DEPTH: usize = 16;

/// Scan enabled RPG Maker MV/MZ plugin parameters.
///
/// This performs static analysis only.
/// It does not execute JavaScript or modify game files.
pub fn scan_parameters(game_path: &Path) -> Result<Vec<InputEvidence>, String> {
    let possible_paths = [
        game_path.join("js/plugins.js"),
        game_path.join("www/js/plugins.js"),
    ];

    let plugins_path = possible_paths
        .iter()
        .find(|path| path.is_file())
        .ok_or_else(|| "Could not find js/plugins.js or www/js/plugins.js".to_string())?;

    let contents =
        fs::read_to_string(plugins_path).map_err(|err| format!("Cannot read plugins.js: {err}"))?;

    // RPG Maker normally stores the registry as:
    // var $plugins = [ ... ];
    let start = contents
        .find('[')
        .ok_or_else(|| "Could not locate the plugin registry array".to_string())?;

    let end = contents
        .rfind(']')
        .ok_or_else(|| "Could not locate the end of the plugin registry".to_string())?;

    if end < start {
        return Err("Invalid plugin registry bounds".into());
    }

    let registry: Value = serde_json::from_str(&contents[start..=end])
        .map_err(|err| format!("Invalid plugin registry JSON: {err}"))?;

    let plugins = registry
        .as_array()
        .ok_or_else(|| "Plugin registry is not an array".to_string())?;

    let mut evidence = Vec::new();

    for plugin in plugins {
        let enabled = match plugin.get("status") {
            Some(Value::Bool(value)) => *value,
            Some(Value::String(value)) => value.eq_ignore_ascii_case("true"),
            _ => false,
        };

        if !enabled {
            continue;
        }

        let Some(name) = plugin.get("name").and_then(Value::as_str) else {
            continue;
        };

        let Some(parameters) = plugin.get("parameters") else {
            continue;
        };

        walk_parameters(name, parameters, "", 0, &mut evidence);
    }

    Ok(evidence)
}

/// Recursively inspect nested objects, arrays, and
/// JSON strings commonly found in RPG Maker parameters.
fn walk_parameters(
    plugin: &str,
    value: &Value,
    path: &str,
    depth: usize,
    evidence: &mut Vec<InputEvidence>,
) {
    if depth >= MAX_DEPTH {
        return;
    }

    match value {
        Value::Object(object) => {
            for (name, child) in object {
                let child_path = if path.is_empty() {
                    name.clone()
                } else {
                    format!("{path}.{name}")
                };

                walk_parameters(plugin, child, &child_path, depth + 1, evidence);
            }
        }

        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                let child_path = format!("{path}[{index}]");

                walk_parameters(plugin, child, &child_path, depth + 1, evidence);
            }
        }

        Value::String(text) => {
            let trimmed = text.trim();

            // Decode nested RPG Maker JSON structures.
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                if let Ok(decoded) = serde_json::from_str::<Value>(trimmed) {
                    walk_parameters(plugin, &decoded, path, depth + 1, evidence);
                    return;
                }
            }

            if let Some((kind, confidence)) = classify_parameter(path, trimmed) {
                evidence.push(InputEvidence {
                    plugin: plugin.to_string(),
                    parameter: path.to_string(),
                    value: trimmed.to_string(),
                    kind: kind.to_string(),
                    confidence: confidence.to_string(),
                });
            }
        }

        Value::Number(number) => {
            let text = number.to_string();

            if let Some((kind, confidence)) = classify_parameter(path, &text) {
                evidence.push(InputEvidence {
                    plugin: plugin.to_string(),
                    parameter: path.to_string(),
                    value: text,
                    kind: kind.to_string(),
                    confidence: confidence.to_string(),
                });
            }
        }

        _ => {}
    }
}

/// Split identifiers such as:
/// "Open Tool Ring Key" -> ["open", "tool", "ring", "key"]
/// "ButtonFadeSpeed"    -> ["button", "fade", "speed"]
/// "KeyCode65"          -> ["key", "code", "65"]
fn tokenize(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();

    for (index, &ch) in chars.iter().enumerate() {
        if !ch.is_ascii_alphanumeric() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            continue;
        }

        if let Some(&previous) = index.checked_sub(1).and_then(|i| chars.get(i)) {
            let case_boundary = ch.is_ascii_uppercase() && previous.is_ascii_lowercase();

            let digit_boundary = ch.is_ascii_digit() != previous.is_ascii_digit()
                && previous.is_ascii_alphanumeric();

            if !current.is_empty() && (case_boundary || digit_boundary) {
                tokens.push(current.clone());
                current.clear();
            }
        }

        current.push(ch.to_ascii_lowercase());
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Reject settings that are clearly about presentation,
/// not actual key bindings.
fn is_display_metadata(tokens: &[String]) -> bool {
    const DISPLAY_WORDS: &[&str] = &[
        "icon",
        "text",
        "label",
        "fade",
        "speed",
        "opacity",
        "font",
        "color",
        "colour",
        "image",
        "picture",
        "sprite",
        "width",
        "height",
        "offset",
        "padding",
        "tooltip",
        "description",
        "graphic",
    ];

    if tokens
        .iter()
        .any(|token| DISPLAY_WORDS.contains(&token.as_str()))
    {
        return true;
    }

    // VisuStella ButtonAssist stores display labels
    // such as KeyA = A, not gameplay bindings.
    tokens
        .windows(2)
        .any(|pair| pair[0] == "button" && pair[1] == "assist")
}

fn is_key_event_path(tokens: &[String]) -> bool {
    // Example: "Key E" = 7 means a possible
    // keyboard-to-common-event mapping.
    if tokens.len() == 2
        && tokens[0] == "key"
        && tokens[1].len() == 1
        && tokens[1].chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        return true;
    }

    let has_event = tokens
        .iter()
        .any(|token| token == "event" || token == "commonevent");

    let has_key = tokens
        .iter()
        .any(|token| matches!(token.as_str(), "key" | "hotkey" | "keyboard"));

    has_event && has_key
}

fn has_explicit_key_context(tokens: &[String]) -> bool {
    tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "key" | "hotkey" | "keyboard" | "shortcut" | "keybind" | "binding"
        )
    })
}

fn is_valid_key_value(value: &str, numeric_keycode: bool) -> bool {
    let value = value.trim();

    if value.is_empty() {
        return false;
    }

    if let Ok(number) = value.parse::<u32>() {
        // Explicit keyboard-code fields may use
        // numeric virtual key codes.
        return numeric_keycode && (1..=255).contains(&number);
    }

    if value.len() == 1 {
        return value.chars().all(|ch| ch.is_ascii_alphanumeric());
    }

    let normalized = value.to_ascii_lowercase();

    const NAMED_KEYS: &[&str] = &[
        "space",
        "enter",
        "return",
        "escape",
        "esc",
        "shift",
        "ctrl",
        "control",
        "alt",
        "tab",
        "backspace",
        "delete",
        "insert",
        "home",
        "end",
        "up",
        "down",
        "left",
        "right",
        "arrowup",
        "arrowdown",
        "arrowleft",
        "arrowright",
        "pageup",
        "pagedown",
    ];

    if NAMED_KEYS.contains(&normalized.as_str()) {
        return true;
    }

    if let Some(number) = normalized.strip_prefix('f') {
        if let Ok(number) = number.parse::<u8>() {
            return (1..=24).contains(&number);
        }
    }

    if let Some(number) = normalized.strip_prefix("numpad") {
        return number
            .parse::<u8>()
            .map(|number| number <= 9)
            .unwrap_or(false);
    }

    false
}

/// Return an evidence classification only when both
/// the parameter name and its value are plausible.
fn classify_parameter(path: &str, value: &str) -> Option<(&'static str, &'static str)> {
    let tokens = tokenize(path);

    if tokens.is_empty() || is_display_metadata(&tokens) {
        return None;
    }

    // Event IDs are not keyboard key codes.
    if is_key_event_path(&tokens) {
        if let Ok(event_id) = value.parse::<u32>() {
            if event_id > 0 {
                return Some(("key_event_candidate", "needs_verification"));
            }
        }

        return None;
    }

    if !has_explicit_key_context(&tokens) {
        return None;
    }

    let has_key_code = tokens
        .windows(2)
        .any(|pair| pair[0] == "key" && pair[1] == "code");

    if is_valid_key_value(value, has_key_code) {
        return Some(("key_binding_candidate", "needs_verification"));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_display_only_parameters() {
        assert_eq!(
            classify_parameter("KeyCode65:struct.ButtonText:str", "A"),
            None
        );

        assert_eq!(
            classify_parameter("KeyCode65:struct.ButtonIcon:num", "0"),
            None
        );

        assert_eq!(
            classify_parameter("MenuLayout:struct.Title:struct.ButtonFadeSpeed:num", "4"),
            None
        );
    }

    #[test]
    fn excludes_visustella_button_labels() {
        assert_eq!(
            classify_parameter("ButtonAssist:struct.KeyA:str", "A"),
            None
        );
    }

    #[test]
    fn detects_custom_key_binding() {
        assert_eq!(
            classify_parameter("Open Tool Ring Key", "A"),
            Some(("key_binding_candidate", "needs_verification"))
        );
    }

    #[test]
    fn detects_key_to_event_mapping() {
        assert_eq!(
            classify_parameter("Key E", "7"),
            Some(("key_event_candidate", "needs_verification"))
        );
    }

    #[test]
    fn detects_explicit_numeric_key_code() {
        assert_eq!(
            classify_parameter("Jump Key Code", "74"),
            Some(("key_binding_candidate", "needs_verification"))
        );
    }

    #[test]
    fn rejects_visual_or_unrelated_settings() {
        assert_eq!(classify_parameter("Window Width", "80"), None);

        assert_eq!(classify_parameter("Use Active Tool", "S"), None);
    }

    #[test]
    fn tokenizes_camel_case_identifiers() {
        assert_eq!(tokenize("ButtonFadeSpeed"), vec!["button", "fade", "speed"]);
    }
}
