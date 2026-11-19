use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::{collections::{BTreeMap, BTreeSet}, path::Path};

use super::{
    input::{scan_inputs, InputAction, KeyBinding},
    javascript::{scan_javascript, JavaScriptInputFinding, SkippedPlugin},
    parameters::{scan_parameters, InputEvidence},
};
use crate::detect_engine;

#[derive(Debug, Serialize)]
pub struct ResolvedAction {
    pub action: String,
    pub keyboard_keys: Vec<KeyBinding>,
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Serialize)]
pub struct PluginInputCandidate {
    pub plugin: String,
    pub parameters: Vec<InputEvidence>,
    pub javascript_findings: Vec<JavaScriptInputFinding>,
    pub status: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ActionKey {
    pub key_code: u32,
    pub key_name: String,
}

#[derive(Debug, Serialize)]
pub struct ActionReference {
    pub plugin: String,
    pub source_file: String,
    pub line: usize,
    pub method: String,
}

#[derive(Debug, Serialize)]
pub struct ActionCatalogEntry {
    pub action: String,
    pub classification: String,
    pub confidence: String,
    pub keyboard_keys: Vec<ActionKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias_of: Option<String>,
    pub binding_status: String,
    pub references: Vec<ActionReference>,
}

/// Source-level mapper assignment evidence. This is NOT a confirmed runtime binding.
#[derive(Debug, Serialize)]
pub struct MapperAssignmentEvidence {
    pub plugin: String,
    pub source_file: String,
    pub line: usize,
    pub action: String,
    pub confidence: String,
}

/// Possible override/collision, grouped by input device and numeric input index.
/// Conditions, execution order, and later modifications are NOT evaluated.
#[derive(Debug, Serialize)]
pub struct MappingConflict {
    pub input_kind: String,
    pub input_index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_name: Option<String>,
    pub conflict_types: Vec<String>,
    pub certainty: String,
    pub core_actions: Vec<String>,
    pub plugin_assignments: Vec<MapperAssignmentEvidence>,
}

/// Configuration-specific evaluation of an inline Yanfly BCE key assignment.
/// This verifies a numeric plugin setting, not actual runtime execution.
#[derive(Debug, Serialize)]
pub struct ConditionalMappingEvaluation {
    pub plugin: String,
    pub input_index: u32,
    pub action: String,
    pub parameter: String,
    pub configured_value: String,
    pub status: String,
    pub context: String,
}

#[derive(Debug, Serialize)]
pub struct InputResolutionReport {
    pub schema_version: u32,
    pub engine: String,
    pub core_actions: Vec<ResolvedAction>,
    pub plugin_candidates: Vec<PluginInputCandidate>,
    pub action_catalog: Vec<ActionCatalogEntry>,
    pub mapping_conflicts: Vec<MappingConflict>,
    pub conditional_mapping_evaluations: Vec<ConditionalMappingEvaluation>,
    pub plugins_scanned: usize,
    pub skipped_plugins: Vec<SkippedPlugin>,
    pub warnings: Vec<String>,
}

/// Resolve *evidence* of game actions. Does not modify or run the game.
pub fn resolve_inputs(path: &Path) -> Result<InputResolutionReport, String> {
    if !path.is_dir() {
        return Err("Game directory does not exist".into());
    }
    let engine = detect_engine(path)?;
    if engine != "RPG Maker MV" && engine != "RPG Maker MZ" {
        return Err(format!("Input resolution is not supported for {engine}"));
    }

    let mut warnings = Vec::new();
    let core_actions = match scan_inputs(path.to_string_lossy().into_owned()) {
        Ok(actions) => resolve_core_actions(actions),
        Err(err) => {
            warnings.push(format!("Core input scan failed: {err}"));
            Vec::new()
        }
    };
    let parameters = match scan_parameters(path) {
        Ok(evidence) => evidence,
        Err(err) => {
            warnings.push(format!("Parameter scan failed: {err}"));
            Vec::new()
        }
    };
    let (findings, plugins_scanned, skipped_plugins) = match scan_javascript(path) {
        Ok(report) => (report.findings, report.plugins_scanned, report.skipped),
        Err(err) => {
            warnings.push(format!("JavaScript scan failed: {err}"));
            (Vec::new(), 0, Vec::new())
        }
    };

    let plugin_candidates = group_plugin_evidence(parameters, findings);
    let action_catalog = build_action_catalog(&core_actions, &plugin_candidates);
    let conditional_mapping_evaluations = evaluate_yanfly_config(path, &plugin_candidates);
    let mapping_conflicts = filter_disabled_conflicts(
        detect_mapping_conflicts(&core_actions, &plugin_candidates),
        &conditional_mapping_evaluations,
    );

    if !skipped_plugins.is_empty() {
        warnings.push(format!(
            "{} plugin entries could not be analyzed",
            skipped_plugins.len()
        ));
    }

    Ok(InputResolutionReport {
        schema_version: 4,
        engine,
        core_actions,
        plugin_candidates,
        action_catalog,
        mapping_conflicts,
        conditional_mapping_evaluations,
        plugins_scanned,
        skipped_plugins,
        warnings,
    })
}

fn resolve_core_actions(actions: Vec<InputAction>) -> Vec<ResolvedAction> {
    actions
        .into_iter()
        .map(|action| ResolvedAction {
            action: action.action,
            keyboard_keys: action.keyboard_keys,
            source: action.source,
            confidence: "declared_core_mapping".into(),
        })
        .collect()
}

/// Group by plugin without claiming the parameters and JS findings are related.
fn group_plugin_evidence(
    parameters: Vec<InputEvidence>,
    findings: Vec<JavaScriptInputFinding>,
) -> Vec<PluginInputCandidate> {
    let mut grouped: BTreeMap<String, PluginInputCandidate> = BTreeMap::new();
    for parameter in parameters {
        let name = parameter.plugin.clone();
        grouped
            .entry(name.clone())
            .or_insert_with(|| PluginInputCandidate {
                plugin: name,
                parameters: Vec::new(),
                javascript_findings: Vec::new(),
                status: String::new(),
            })
            .parameters
            .push(parameter);
    }
    for finding in findings {
        let name = finding.plugin.clone();
        grouped
            .entry(name.clone())
            .or_insert_with(|| PluginInputCandidate {
                plugin: name,
                parameters: Vec::new(),
                javascript_findings: Vec::new(),
                status: String::new(),
            })
            .javascript_findings
            .push(finding);
    }
    for candidate in grouped.values_mut() {
        candidate.status = if !candidate.parameters.is_empty()
            && !candidate.javascript_findings.is_empty()
        {
            "both_evidence_types"
        } else if !candidate.javascript_findings.is_empty() {
            "javascript_only"
        } else {
            "parameters_only"
        }
        .into();
    }
    grouped.into_values().collect()
}

fn copy_core_keys(action: &ResolvedAction) -> Vec<ActionKey> {
    action
        .keyboard_keys
        .iter()
        .map(|key| ActionKey {
            key_code: key.key_code,
            key_name: key.key_name.clone(),
        })
        .collect()
}

fn new_catalog_entry(
    name: &str,
    core: Option<&ResolvedAction>,
    escape: Option<&ResolvedAction>,
) -> ActionCatalogEntry {
    if let Some(core_action) = core {
        return ActionCatalogEntry {
            action: name.into(),
            classification: "core_action".into(),
            confidence: "declared_core_mapping".into(),
            keyboard_keys: copy_core_keys(core_action),
            alias_of: None,
            binding_status: "declared_core_mapping".into(),
            references: Vec::new(),
        };
    }
    if name == "cancel" || name == "menu" {
        return ActionCatalogEntry {
            action: name.into(),
            classification: "engine_escape_compatible".into(),
            confidence: "engine_compatibility_rule".into(),
            keyboard_keys: escape.map(copy_core_keys).unwrap_or_default(),
            alias_of: Some("escape".into()),
            binding_status: if escape.is_some() {
                "escape_compatible_core_mapping"
            } else {
                "unresolved_mapping"
            }
            .into(),
            references: Vec::new(),
        };
    }
    ActionCatalogEntry {
        action: name.into(),
        classification: "custom_or_unmapped".into(),
        confidence: "static_literal_reference".into(),
        keyboard_keys: Vec::new(),
        alias_of: None,
        binding_status: "unresolved_mapping".into(),
        references: Vec::new(),
    }
}

/// Only `input_check` calls can populate the action catalog in v2.
/// Mapper assignments are preserved inside plugin_candidates for the next phase.
fn build_action_catalog(
    core_actions: &[ResolvedAction],
    plugin_candidates: &[PluginInputCandidate],
) -> Vec<ActionCatalogEntry> {
    let mut catalog: BTreeMap<String, ActionCatalogEntry> = BTreeMap::new();
    let core_lookup: BTreeMap<&str, &ResolvedAction> = core_actions
        .iter()
        .map(|item| (item.action.as_str(), item))
        .collect();
    let escape = core_lookup.get("escape").copied();

    for item in core_actions {
        catalog.insert(
            item.action.clone(),
            new_catalog_entry(&item.action, Some(item), escape),
        );
    }
    for plugin in plugin_candidates {
        for finding in &plugin.javascript_findings {
            if finding.category != "input_check" {
                continue;
            }
            let Some(name) = finding.action.as_deref() else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let core = core_lookup.get(name).copied();
            catalog
                .entry(name.into())
                .or_insert_with(|| new_catalog_entry(name, core, escape))
                .references
                .push(ActionReference {
                    plugin: finding.plugin.clone(),
                    source_file: finding.source_file.clone(),
                    line: finding.line,
                    method: finding.pattern.clone(),
                });
        }
    }
    catalog.into_values().collect()
}

/// Contrast statically discovered assignments with engine defaults and each other.
/// A mapping can be conditional, unreachable, or overwritten at runtime; therefore
/// all matches are informational candidates, not verified runtime conflicts.
fn detect_mapping_conflicts(
    core_actions: &[ResolvedAction],
    plugin_candidates: &[PluginInputCandidate],
) -> Vec<MappingConflict> {
    // Keep keyboard and gamepad in separate namespaces even when indices match.
    let mut core_by_key: BTreeMap<u32, (Option<String>, BTreeSet<String>)> = BTreeMap::new();
    for action in core_actions {
        for key in &action.keyboard_keys {
            let entry = core_by_key
                .entry(key.key_code)
                .or_insert_with(|| (Some(key.key_name.clone()), BTreeSet::new()));
            entry.1.insert(action.action.clone());
        }
    }

    let mut by_index: BTreeMap<(String, u32), Vec<MapperAssignmentEvidence>> = BTreeMap::new();
    for plugin in plugin_candidates {
        for finding in &plugin.javascript_findings {
            let kind = match finding.category.as_str() {
                "keyboard_mapping_assignment" => "keyboard",
                "gamepad_mapping_assignment" => "gamepad",
                _ => continue,
            };
            let (Some(index), Some(action)) =
                (finding.mapper_index, finding.action.as_deref())
            else {
                continue;
            };
            by_index
                .entry((kind.into(), index))
                .or_default()
                .push(MapperAssignmentEvidence {
                    plugin: finding.plugin.clone(),
                    source_file: finding.source_file.clone(),
                    line: finding.line,
                    action: action.to_string(),
                    confidence: finding.confidence.clone(),
                });
        }
    }

    let mut conflicts = Vec::new();
    for ((kind, index), mut assignments) in by_index {
        assignments.sort_by(|a, b| {
            a.plugin.cmp(&b.plugin)
                .then_with(|| a.source_file.cmp(&b.source_file))
                .then_with(|| a.line.cmp(&b.line))
                .then_with(|| a.action.cmp(&b.action))
        });
        assignments.dedup_by(|a, b| {
            a.plugin == b.plugin
                && a.source_file == b.source_file
                && a.line == b.line
                && a.action == b.action
        });

        let baseline = if kind == "keyboard" {
            core_by_key.get(&index)
        } else {
            None // Gamepad core mappings aren't scanned yet.
        };
        let core_names: BTreeSet<String> = baseline
            .map(|(_, names)| names.clone())
            .unwrap_or_default();
        let plugin_names: BTreeSet<&str> = assignments
            .iter()
            .map(|item| item.action.as_str())
            .collect();

        let core_override = !core_names.is_empty()
            && plugin_names.iter().any(|name| !core_names.contains(*name));
        let plugin_collision = plugin_names.len() > 1;
        if !core_override && !plugin_collision {
            continue;
        }

        let mut conflict_types = Vec::new();
        if core_override {
            conflict_types.push("potential_core_override".into());
        }
        if plugin_collision {
            conflict_types.push("potential_plugin_collision".into());
        }

        conflicts.push(MappingConflict {
            input_kind: kind,
            input_index: index,
            key_name: baseline.and_then(|(label, _)| label.clone()),
            conflict_types,
            certainty: "potential_static_evidence".into(),
            core_actions: core_names.into_iter().collect(),
            plugin_assignments: assignments,
        });
    }
    conflicts
}

/// Evaluate only the verified YEP_ButtonCommonEvents pattern:
/// `BCEList.foo = Number(Yanfly.Parameters['Key ...'])` and
/// `if (BCEList['foo'] !== 0) Input.keyMapper[n] = 'foo'`.
/// Any unsupported syntax or missing configuration stays unknown.
fn evaluate_yanfly_config(
    game: &Path,
    candidates: &[PluginInputCandidate],
) -> Vec<ConditionalMappingEvaluation> {
    let Some(plugin) = candidates.iter().find(|p| p.plugin == "YEP_ButtonCommonEvents") else {
        return Vec::new();
    };
    let Some(js_dir) = [game.join("js"), game.join("www/js")]
        .into_iter()
        .find(|dir| dir.join("plugins.js").is_file())
    else {
        return Vec::new();
    };
    let Ok(registry_source) = fs::read_to_string(js_dir.join("plugins.js")) else {
        return Vec::new();
    };
    let (Some(start), Some(end)) = (registry_source.find('['), registry_source.rfind(']')) else {
        return Vec::new();
    };
    if end < start { return Vec::new(); }
    let Ok(registry) = serde_json::from_str::<Value>(&registry_source[start..=end]) else {
        return Vec::new();
    };
    let Some(config) = registry.as_array().and_then(|entries| entries.iter().find(|entry| {
        entry.get("name").and_then(Value::as_str) == Some("YEP_ButtonCommonEvents")
            && entry.get("status") == Some(&Value::Bool(true))
    })) else {
        return Vec::new();
    };
    let Some(parameters) = config.get("parameters").and_then(Value::as_object) else {
        return Vec::new();
    };
    let Ok(source) = fs::read_to_string(js_dir.join("plugins/YEP_ButtonCommonEvents.js")) else {
        return Vec::new();
    };
    let definitions = Regex::new(
        r#"(?m)^\s*([A-Za-z0-9_]+)\s*:\s*Number\(Yanfly\.Parameters\['([^']+)'\]\)"#,
    ).unwrap();
    let assignments = Regex::new(
        r#"if\s*\(\s*Yanfly\.Param\.BCEList\['([^']+)'\]\s*!==\s*0\s*\)\s*Input\.keyMapper\[(\d+)\]\s*=\s*'([^']+)'"#,
    ).unwrap();

    let names: BTreeMap<String, String> = definitions.captures_iter(&source).map(|cap| {
        (cap[1].to_string(), cap[2].replace("\\\\", "\\"))
    }).collect();
    let guarded: BTreeSet<(String, u32)> = assignments.captures_iter(&source).filter_map(|cap| {
        let code = cap[2].parse::<u32>().ok()?;
        if cap[1] != cap[3] { return None; }
        Some((cap[3].to_string(), code))
    }).collect();

    plugin.javascript_findings.iter().filter_map(|finding| {
        if finding.category != "keyboard_mapping_assignment"
            || finding.confidence != "static_conditional_assignment"
        {
            return None;
        }
        let action = finding.action.as_ref()?;
        let index = finding.mapper_index?;
        if !guarded.contains(&(action.clone(), index)) { return None; }
        let parameter = names.get(action)?;
        let value = parameters.get(parameter).and_then(Value::as_str)?;
        let status = if value.trim().is_empty() {
            "disabled_by_configuration"
        } else if let Ok(number) = value.trim().parse::<f64>() {
            if number.is_nan() { "unknown" }
            else if number == 0.0 { "disabled_by_configuration" }
            else { "enabled_by_configuration" }
        } else {
            "unknown"
        };
        Some(ConditionalMappingEvaluation {
            plugin: plugin.plugin.clone(),
            input_index: index,
            action: action.clone(),
            parameter: parameter.clone(),
            configured_value: value.to_string(),
            status: status.into(),
            context: "YEP_ButtonCommonEvents map shortcut; not confirmed at runtime".into(),
        })
    }).collect()
}

/// Remove only *proven disabled-by-configuration* assignments from the
/// potential-conflict view; retain other plugins and unresolved conditions.
fn filter_disabled_conflicts(
    conflicts: Vec<MappingConflict>,
    evaluations: &[ConditionalMappingEvaluation],
) -> Vec<MappingConflict> {
    let disabled: BTreeSet<(&str, u32, &str)> = evaluations.iter()
        .filter(|e| e.status == "disabled_by_configuration")
        .map(|e| (e.plugin.as_str(), e.input_index, e.action.as_str()))
        .collect();
    conflicts.into_iter().filter_map(|mut conflict| {
        if conflict.input_kind == "keyboard" {
            conflict.plugin_assignments.retain(|a| {
                !disabled.contains(&(a.plugin.as_str(), conflict.input_index, a.action.as_str()))
            });
        }
        let plugin_actions: BTreeSet<&str> = conflict.plugin_assignments.iter()
            .map(|a| a.action.as_str()).collect();
        let baseline: BTreeSet<&str> = conflict.core_actions.iter().map(String::as_str).collect();
        let override_core = !baseline.is_empty()
            && plugin_actions.iter().any(|action| !baseline.contains(action));
        let collision = plugin_actions.len() > 1;
        conflict.conflict_types.clear();
        if override_core { conflict.conflict_types.push("potential_core_override".into()); }
        if collision { conflict.conflict_types.push("potential_plugin_collision".into()); }
        if conflict.conflict_types.is_empty() { None } else { Some(conflict) }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_action(name: &str, code: u32, key: &str) -> ResolvedAction {
        ResolvedAction {
            action: name.into(),
            keyboard_keys: vec![KeyBinding {
                key_code: code,
                key_name: key.into(),
            }],
            source: "rpg_core.js".into(),
            confidence: "declared_core_mapping".into(),
        }
    }

    fn finding(plugin: &str, action: Option<&str>) -> JavaScriptInputFinding {
        JavaScriptInputFinding {
            plugin: plugin.into(),
            source_file: format!("js/plugins/{plugin}.js"),
            line: 42,
            pattern: "Input.isTriggered".into(),
            category: "input_check".into(),
            confidence: if action.is_some() {
                "static_literal"
            } else {
                "static_candidate"
            }
            .into(),
            action: action.map(str::to_string),
            mapper_index: None,
        }
    }

    #[test]
    fn preserves_multiple_keys_per_action() {
        let actions = vec![InputAction {
            action: "up".into(),
            keyboard_keys: vec![
                KeyBinding { key_code: 87, key_name: "W".into() },
                KeyBinding { key_code: 38, key_name: "Up Arrow".into() },
            ],
            source: "rpg_core.js".into(),
            confidence: "confirmed_declaration".into(),
        }];
        let resolved = resolve_core_actions(actions);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].keyboard_keys.len(), 2);
    }

    #[test]
    fn parameters_are_not_promoted_to_core_actions() {
        let params = vec![InputEvidence {
            plugin: "ExamplePlugin".into(),
            parameter: "Open Menu Key".into(),
            value: "A".into(),
            kind: "key_binding_candidate".into(),
            confidence: "needs_verification".into(),
        }];
        let candidates = group_plugin_evidence(params, Vec::new());
        assert!(build_action_catalog(&[], &candidates).is_empty());
        assert_eq!(candidates[0].status, "parameters_only");
    }

    #[test]
    fn groups_both_evidence_types_by_plugin() {
        let params = vec![InputEvidence {
            plugin: "ExamplePlugin".into(),
            parameter: "Attack Key".into(),
            value: "Q".into(),
            kind: "key_binding_candidate".into(),
            confidence: "needs_verification".into(),
        }];
        let candidates = group_plugin_evidence(params, vec![finding("ExamplePlugin", Some("attack"))]);
        assert_eq!(candidates[0].status, "both_evidence_types");
        let catalog = build_action_catalog(&[], &candidates);
        assert_eq!(catalog[0].action, "attack");
        assert!(catalog[0].keyboard_keys.is_empty());
    }

    #[test]
    fn matches_declared_core_actions() {
        let core = vec![core_action("ok", 13, "Enter")];
        let candidates = group_plugin_evidence(Vec::new(), vec![finding("Fishing", Some("ok"))]);
        let catalog = build_action_catalog(&core, &candidates);
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].classification, "core_action");
        assert_eq!(catalog[0].keyboard_keys[0].key_code, 13);
        assert_eq!(catalog[0].references.len(), 1);
    }

    #[test]
    fn recognizes_escape_compatible_actions() {
        let core = vec![core_action("escape", 27, "Escape")];
        let candidates = group_plugin_evidence(Vec::new(), vec![
            finding("Menu", Some("cancel")),
            finding("Menu", Some("menu")),
        ]);
        let catalog = build_action_catalog(&core, &candidates);
        for name in ["cancel", "menu"] {
            let item = catalog.iter().find(|item| item.action == name).unwrap();
            assert_eq!(item.classification, "engine_escape_compatible");
            assert_eq!(item.alias_of.as_deref(), Some("escape"));
            assert_eq!(item.keyboard_keys[0].key_code, 27);
        }
    }

    #[test]
    fn keeps_custom_actions_case_sensitive() {
        let candidates = group_plugin_evidence(Vec::new(), vec![
            finding("PluginA", Some("q")),
            finding("PluginA", Some("Q")),
            finding("PluginB", Some("q")),
        ]);
        let catalog = build_action_catalog(&[], &candidates);
        assert_eq!(catalog.len(), 2);
        assert_eq!(catalog.iter().find(|item| item.action == "q").unwrap().references.len(), 2);
    }

    #[test]
    fn dynamic_arguments_remain_unresolved() {
        let candidates = group_plugin_evidence(Vec::new(), vec![finding("PluginA", None)]);
        assert!(build_action_catalog(&[], &candidates).is_empty());
        assert_eq!(candidates[0].javascript_findings.len(), 1);
    }

    #[test]
    fn mapper_assignments_are_preserved_but_not_promoted() {
        let mut mapping = finding("PluginA", Some("q"));
        mapping.category = "keyboard_mapping_assignment".into();
        mapping.mapper_index = Some(81);
        let candidates = group_plugin_evidence(Vec::new(), vec![mapping]);
        assert!(build_action_catalog(&[], &candidates).is_empty());
        assert_eq!(candidates[0].javascript_findings[0].mapper_index, Some(81));
    }

    fn mapping(plugin: &str, kind: &str, index: u32, action: &str, conditional: bool) -> JavaScriptInputFinding {
        JavaScriptInputFinding {
            plugin: plugin.into(),
            source_file: format!("js/plugins/{plugin}.js"),
            line: 807,
            pattern: if kind == "keyboard" { "Input.keyMapper" } else { "Input.gamepadMapper" }.into(),
            category: if kind == "keyboard" { "keyboard_mapping_assignment" } else { "gamepad_mapping_assignment" }.into(),
            confidence: if conditional { "static_conditional_assignment" } else { "static_assignment" }.into(),
            action: Some(action.into()),
            mapper_index: Some(index),
        }
    }

    #[test]
    fn reports_yanfly_conditional_override_without_claiming_runtime_binding() {
        let core = vec![core_action("pageup", 81, "Q")];
        let plugins = group_plugin_evidence(Vec::new(), vec![
            mapping("YEP_ButtonCommonEvents", "keyboard", 81, "q", true),
        ]);
        let conflicts = detect_mapping_conflicts(&core, &plugins);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].input_kind, "keyboard");
        assert_eq!(conflicts[0].input_index, 81);
        assert_eq!(conflicts[0].core_actions, vec!["pageup"]);
        assert_eq!(conflicts[0].conflict_types, vec!["potential_core_override"]);
        assert_eq!(conflicts[0].certainty, "potential_static_evidence");
        assert_eq!(conflicts[0].plugin_assignments[0].action, "q");
        assert_eq!(conflicts[0].plugin_assignments[0].confidence, "static_conditional_assignment");
    }

    #[test]
    fn identical_mapping_is_not_a_conflict() {
        let core = vec![core_action("pageup", 81, "Q")];
        let plugins = group_plugin_evidence(Vec::new(), vec![
            mapping("PluginA", "keyboard", 81, "pageup", false),
            mapping("PluginB", "keyboard", 81, "pageup", true),
        ]);
        assert!(detect_mapping_conflicts(&core, &plugins).is_empty());
    }

    #[test]
    fn detects_two_plugins_assigning_different_actions() {
        let plugins = group_plugin_evidence(Vec::new(), vec![
            mapping("PluginA", "keyboard", 82, "r", false),
            mapping("PluginB", "keyboard", 82, "reload", true),
        ]);
        let conflicts = detect_mapping_conflicts(&[], &plugins);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_types, vec!["potential_plugin_collision"]);
        assert_eq!(conflicts[0].plugin_assignments.len(), 2);
    }

    #[test]
    fn keeps_keyboard_and_gamepad_indices_separate() {
        let core = vec![core_action("ok", 1, "Unknown key 1")];
        let plugins = group_plugin_evidence(Vec::new(), vec![
            mapping("PluginA", "gamepad", 1, "cancel", false),
            mapping("PluginB", "keyboard", 1, "ok", false),
        ]);
        assert!(detect_mapping_conflicts(&core, &plugins).is_empty());
    }

    #[test]
    fn detects_gamepad_plugin_collisions_without_core_gamepad_data() {
        let plugins = group_plugin_evidence(Vec::new(), vec![
            mapping("PluginA", "gamepad", 0, "ok", false),
            mapping("PluginB", "gamepad", 0, "attack", false),
        ]);
        let conflicts = detect_mapping_conflicts(&[], &plugins);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].input_kind, "gamepad");
        assert!(conflicts[0].core_actions.is_empty());
    }

    #[test]
    fn ignores_unresolved_mapper_accesses() {
        let mut dynamic = mapping("PluginA", "keyboard", 81, "q", false);
        dynamic.category = "keyboard_mapping_access".into();
        let plugins = group_plugin_evidence(Vec::new(), vec![dynamic]);
        assert!(detect_mapping_conflicts(&[core_action("pageup", 81, "Q")], &plugins).is_empty());
    }

    #[test]
    fn disabled_yanfly_mapping_does_not_count_as_override() {
        let core = vec![core_action("pageup", 81, "Q")];
        let plugins = group_plugin_evidence(Vec::new(), vec![
            mapping("YEP_ButtonCommonEvents", "keyboard", 81, "q", true),
        ]);
        let evaluations = vec![ConditionalMappingEvaluation {
            plugin: "YEP_ButtonCommonEvents".into(),
            input_index: 81,
            action: "q".into(),
            parameter: "Key Q (PageUp)".into(),
            configured_value: "".into(),
            status: "disabled_by_configuration".into(),
            context: "map shortcut".into(),
        }];
        let conflicts = filter_disabled_conflicts(
            detect_mapping_conflicts(&core, &plugins), &evaluations,
        );
        assert!(conflicts.is_empty());
    }

    #[test]
    fn unknown_yanfly_mapping_remains_a_potential_conflict() {
        let core = vec![core_action("pageup", 81, "Q")];
        let plugins = group_plugin_evidence(Vec::new(), vec![
            mapping("YEP_ButtonCommonEvents", "keyboard", 81, "q", true),
        ]);
        let conflicts = filter_disabled_conflicts(
            detect_mapping_conflicts(&core, &plugins), &[],
        );
        assert_eq!(conflicts.len(), 1);
    }

}
