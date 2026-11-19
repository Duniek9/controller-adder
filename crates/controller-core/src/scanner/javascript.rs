use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::{fs, path::Path};

const MAX_PLUGIN_SIZE: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct JavaScriptInputFinding {
    pub plugin: String,
    pub source_file: String,
    pub line: usize,
    pub pattern: String,
    pub category: String,
    pub confidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapper_index: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkippedPlugin {
    pub plugin: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaScriptScanReport {
    pub plugins_scanned: usize,
    pub findings: Vec<JavaScriptInputFinding>,
    pub skipped: Vec<SkippedPlugin>,
}

/// Read enabled RPG Maker MV/MZ plugins and inspect their JavaScript files.
/// This is static analysis: no game code is executed or modified.
pub fn scan_javascript(game_path: &Path) -> Result<JavaScriptScanReport, String> {
    let registry_path = [
        game_path.join("js/plugins.js"),
        game_path.join("www/js/plugins.js"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or_else(|| "Could not find js/plugins.js or www/js/plugins.js".to_string())?;

    let registry_text = fs::read_to_string(&registry_path)
        .map_err(|e| format!("Cannot read plugins.js: {e}"))?;
    let start = registry_text
        .find('[')
        .ok_or_else(|| "Plugin registry array not found".to_string())?;
    let end = registry_text
        .rfind(']')
        .ok_or_else(|| "Plugin registry end not found".to_string())?;
    if end < start {
        return Err("Invalid plugin registry bounds".into());
    }
    let registry: Value = serde_json::from_str(&registry_text[start..=end])
        .map_err(|e| format!("Invalid plugin registry JSON: {e}"))?;
    let entries = registry
        .as_array()
        .ok_or_else(|| "Plugin registry must be an array".to_string())?;
    let js_dir = registry_path
        .parent()
        .ok_or_else(|| "Invalid plugin registry directory".to_string())?;

    let mut report = JavaScriptScanReport {
        plugins_scanned: 0,
        findings: Vec::new(),
        skipped: Vec::new(),
    };

    for entry in entries {
        let enabled = match entry.get("status") {
            Some(Value::Bool(value)) => *value,
            Some(Value::String(value)) => value.eq_ignore_ascii_case("true"),
            _ => false,
        };
        if !enabled {
            continue;
        }
        let Some(name) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        report.plugins_scanned += 1;

        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\\')
            || name.contains('\0')
        {
            report.skipped.push(SkippedPlugin {
                plugin: name.into(),
                reason: "Unsafe plugin name".into(),
            });
            continue;
        }

        let plugin_path = js_dir.join("plugins").join(format!("{name}.js"));
        match scan_plugin(name, js_dir, &plugin_path) {
            Ok(found) => report.findings.extend(found),
            Err(reason) => report.skipped.push(SkippedPlugin {
                plugin: name.into(),
                reason,
            }),
        }
    }
    Ok(report)
}

fn scan_plugin(
    plugin: &str,
    js_dir: &Path,
    plugin_path: &Path,
) -> Result<Vec<JavaScriptInputFinding>, String> {
    let metadata = fs::metadata(plugin_path)
        .map_err(|e| format!("Cannot access plugin: {e}"))?;
    if !metadata.is_file() {
        return Err("Plugin path is not a file".into());
    }
    if metadata.len() > MAX_PLUGIN_SIZE {
        return Err("Plugin exceeds 4 MiB scan limit".into());
    }

    let plugins_dir = fs::canonicalize(js_dir.join("plugins"))
        .map_err(|e| format!("Cannot resolve plugins directory: {e}"))?;
    let canonical = fs::canonicalize(plugin_path)
        .map_err(|e| format!("Cannot resolve plugin path: {e}"))?;
    if !canonical.starts_with(&plugins_dir) {
        return Err("Plugin file escapes plugins directory".into());
    }

    let source = fs::read_to_string(plugin_path)
        .map_err(|e| format!("Cannot read plugin source: {e}"))?;
    if source.len() as u64 > MAX_PLUGIN_SIZE {
        return Err("Plugin exceeds 4 MiB scan limit".into());
    }

    let relative = plugin_path.strip_prefix(js_dir).unwrap_or(plugin_path);
    let source_file = format!("js/{}", relative.to_string_lossy().replace('\\', "/"));
    Ok(analyze_source(plugin, &source_file, &source))
}

/// Replace comment and string contents with ASCII spaces, preserving byte
/// offsets and line breaks. The original string is used for extracting values.
/// Note: this is a lightweight lexer, not a full JavaScript parser.
fn mask_non_code(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut masked = bytes.to_vec();
    let mut index = 0;
    let mut state = 0u8; // 0 code, 1 ', 2 ", 3 `, 4 //, 5 /* */

    while index < bytes.len() {
        let current = bytes[index];
        let next = bytes.get(index + 1).copied();
        match state {
            0 => {
                if current == b'/' && next == Some(b'/') {
                    state = 4;
                    masked[index] = b' ';
                } else if current == b'/' && next == Some(b'*') {
                    state = 5;
                    masked[index] = b' ';
                } else if current == b'\'' {
                    state = 1;
                    masked[index] = b' ';
                } else if current == b'"' {
                    state = 2;
                    masked[index] = b' ';
                } else if current == b'`' {
                    state = 3;
                    masked[index] = b' ';
                }
            }
            1 | 2 | 3 => {
                let closing = match state {
                    1 => b'\'',
                    2 => b'"',
                    _ => b'`',
                };
                if current != b'\n' && current != b'\r' {
                    masked[index] = b' ';
                }
                if current == b'\\' {
                    if index + 1 < bytes.len() {
                        index += 1;
                        if bytes[index] != b'\n' && bytes[index] != b'\r' {
                            masked[index] = b' ';
                        }
                    }
                } else if current == closing {
                    state = 0;
                }
            }
            4 => {
                if current == b'\n' {
                    state = 0;
                } else if current != b'\r' {
                    masked[index] = b' ';
                }
            }
            5 => {
                if current == b'*' && next == Some(b'/') {
                    masked[index] = b' ';
                    masked[index + 1] = b' ';
                    index += 1;
                    state = 0;
                } else if current != b'\n' && current != b'\r' {
                    masked[index] = b' ';
                }
            }
            _ => unreachable!(),
        }
        index += 1;
    }
    String::from_utf8(masked).expect("ASCII masking preserves UTF-8")
}

/// Parse only a simple, unescaped literal and require a permitted delimiter.
fn literal_at(source: &str, offset: usize, delimiters: &[u8]) -> Option<String> {
    let raw = source.as_bytes();
    let mut pos = offset;
    while pos < raw.len() && raw[pos].is_ascii_whitespace() {
        pos += 1;
    }
    let quote = *raw.get(pos)?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }
    pos += 1;
    let start = pos;
    while pos < raw.len() {
        if raw[pos] == b'\\' || raw[pos] == b'\n' || raw[pos] == b'\r' {
            return None;
        }
        if raw[pos] == quote {
            let action = source.get(start..pos)?;
            if action.is_empty() || action.len() > 128 || action.chars().any(char::is_control) {
                return None;
            }
            pos += 1;
            while pos < raw.len() && raw[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos == raw.len() || delimiters.contains(&raw[pos]) {
                return Some(action.into());
            }
            return None;
        }
        pos += 1;
    }
    None
}

fn extract_literal_action(source: &str, argument_start: usize) -> Option<String> {
    // A call's first argument must terminate before a ')' or ','.
    // Don't permit end-of-file for input calls.
    let value = literal_at(source, argument_start, b"),")?;
    let raw = source.as_bytes();
    let mut pos = argument_start;
    while pos < raw.len() && raw[pos].is_ascii_whitespace() { pos += 1; }
    let quote = *raw.get(pos)?;
    pos += 1;
    while pos < raw.len() && raw[pos] != quote { pos += 1; }
    pos += 1;
    while pos < raw.len() && raw[pos].is_ascii_whitespace() { pos += 1; }
    if pos < raw.len() && (raw[pos] == b')' || raw[pos] == b',') { Some(value) } else { None }
}

fn extract_mapper_value(source: &str, assignment_end: usize) -> Option<String> {
    literal_at(source, assignment_end, b";,}")
}

fn line_number(source: &str, offset: usize) -> usize {
    source.as_bytes()[..offset].iter().filter(|&&b| b == b'\n').count() + 1
}

fn analyze_source(plugin: &str, source_file: &str, source: &str) -> Vec<JavaScriptInputFinding> {
    let masked = mask_non_code(source);
    let mut findings = Vec::new();

    let checks = Regex::new(
        r"\bInput\s*\.\s*(isTriggered|isPressed|isRepeated|isLongPressed)\s*\("
    ).unwrap();
    for capture in checks.captures_iter(&masked) {
        let full = capture.get(0).unwrap();
        let method = capture.get(1).unwrap().as_str();
        let action = extract_literal_action(source, full.end());
        findings.push(JavaScriptInputFinding {
            plugin: plugin.into(),
            source_file: source_file.into(),
            line: line_number(source, full.start()),
            pattern: format!("Input.{method}"),
            category: "input_check".into(),
            confidence: if action.is_some() { "static_literal" } else { "static_candidate" }.into(),
            action,
            mapper_index: None,
        });
    }

    let mapper_access = Regex::new(r"\bInput\s*\.\s*(keyMapper|gamepadMapper)\b").unwrap();
    let mapper_assignment = Regex::new(
    r"\bInput\s*\.\s*(keyMapper|gamepadMapper)\s*\[\s*(\d{1,5})\s*\]\s*="
    ).unwrap();
    for capture in mapper_access.captures_iter(&masked) {
        let full = capture.get(0).unwrap();
        let mapper = capture.get(1).unwrap().as_str();
        let mut action = None;
        let mut mapper_index = None;
        let mut category = if mapper == "keyMapper" {
            "keyboard_mapping_access"
        } else {
            "gamepad_mapping_access"
        };
        let mut confidence = "static_candidate";

        if let Some(assignment) = mapper_assignment.captures_at(&masked, full.start()) {
            let whole = assignment.get(0).unwrap();
            if whole.start() == full.start() {
                let index_text = assignment.get(2).unwrap().as_str();
                if let Ok(index) = index_text.parse::<u32>() {
                    let index_valid = if mapper == "keyMapper" { index <= 255 } else { index <= 31 };
                    // Reject `==`, `=>`, and other non-simple operators.
                    let following = masked.as_bytes().get(whole.end());
                    if index_valid && !matches!(following, Some(b'=' | b'>')) {
                        if let Some(value) = extract_mapper_value(source, whole.end()) {
                            mapper_index = Some(index);
                            action = Some(value);
                            category = if mapper == "keyMapper" {
                                "keyboard_mapping_assignment"
                            } else {
                                "gamepad_mapping_assignment"
                            };
                            confidence = "static_assignment";
                        }
                    }
                }
            }
        }

        findings.push(JavaScriptInputFinding {
            plugin: plugin.into(),
            source_file: source_file.into(),
            line: line_number(source, full.start()),
            pattern: format!("Input.{mapper}"),
            category: category.into(),
            confidence: confidence.into(),
            action,
            mapper_index,
        });
    }

    let other_patterns = [
        (r"\baddEventListener\s*\(", "addEventListener", "event_listener_candidate"),
        (r"\bonkeydown\b|\bonkeyup\b", "keyboard_handler", "keyboard_handler_candidate"),
    ];
    for (expression, pattern, category) in other_patterns {
        let regex = Regex::new(expression).unwrap();
        for found in regex.find_iter(&masked) {
            findings.push(JavaScriptInputFinding {
                plugin: plugin.into(),
                source_file: source_file.into(),
                line: line_number(source, found.start()),
                pattern: pattern.into(),
                category: category.into(),
                confidence: "static_candidate".into(),
                action: None,
                mapper_index: None,
            });
        }
    }

    findings.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.pattern.cmp(&b.pattern)));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn findings(source: &str) -> Vec<JavaScriptInputFinding> {
        analyze_source("Test", "js/plugins/Test.js", source)
    }

    #[test]
    fn extracts_literal_actions() {
        let items = findings("Input.isTriggered(\"attack\"); Input.isPressed('ok'); Input.isRepeated(\"down\"); Input.isLongPressed('shift');");
        let actions: Vec<_> = items.iter().filter_map(|item| item.action.as_deref()).collect();
        assert_eq!(actions, vec!["shift", "ok", "down", "attack"]);
    }

    #[test]
    fn keeps_dynamic_arguments_unresolved() {
        let items = findings("Input.isTriggered(actionName); Input.isPressed(settings.key); Input.isRepeated(getAction());");
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|item| item.action.is_none()));
    }

    #[test]
    fn ignores_comments_and_strings() {
        let items = findings("// Input.isTriggered('fake')\nconst s = 'Input.isPressed(\"fake\")'; /* Input.isRepeated('fake') */ Input.isTriggered('real');");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].action.as_deref(), Some("real"));
    }

    #[test]
    fn tracks_multiple_calls_on_same_line() {
        let items = findings("Input.isPressed('left') || Input.isPressed('right')");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].action.as_deref(), Some("left"));
        assert_eq!(items[1].action.as_deref(), Some("right"));
    }

    #[test]
    fn preserves_unknown_mapping_access() {
        let items = findings("console.log(Input.keyMapper); console.log(Input.gamepadMapper);");
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| item.mapper_index.is_none()));
    }

    #[test]
    fn extracts_keyboard_mapper_assignments() {
        let items = findings("Input.keyMapper[81] = \"q\"; Input.keyMapper[69] = 'e';");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].category, "keyboard_mapping_assignment");
        assert_eq!(items[0].mapper_index, Some(81));
        assert_eq!(items[0].action.as_deref(), Some("q"));
        assert_eq!(items[1].mapper_index, Some(69));
        assert_eq!(items[1].action.as_deref(), Some("e"));
    }

    #[test]
    fn extracts_gamepad_mapper_assignments() {
        let items = findings("Input.gamepadMapper[0] = 'ok'; Input.gamepadMapper[1] = \"cancel\";");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].category, "gamepad_mapping_assignment");
        assert_eq!(items[0].mapper_index, Some(0));
        assert_eq!(items[0].action.as_deref(), Some("ok"));
        assert_eq!(items[1].mapper_index, Some(1));
        assert_eq!(items[1].action.as_deref(), Some("cancel"));
    }

    #[test]
    fn preserves_dynamic_mapper_access() {
        let items = findings("Input.keyMapper[keyCode] = actionName; Input.gamepadMapper[index] = getAction();");
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| item.mapper_index.is_none() && item.action.is_none()));
    }

    #[test]
    fn rejects_nonliteral_and_comparison_expressions() {
        let items = findings("Input.keyMapper[81] = 'q' + suffix; Input.keyMapper[69] == 'e';");
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| item.category == "keyboard_mapping_access"));
    }
}
