//! Read-only input inventory built from the existing schema-v4 resolver.
//!
//! IMPORTANT: This is an evidence model, not a runtime input simulator.
//! Static analysis cannot prove which JavaScript statements execute or which
//! plugin wins if multiple writers modify the same mapper at runtime.

use std::{collections::{BTreeMap, BTreeSet}, path::Path};

use serde::Serialize;

use super::resolver::{resolve_inputs, InputResolutionReport};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EffectiveMapperCandidate {
    pub plugin: String,
    pub source_file: String,
    pub line: usize,
    pub action: String,
    pub assignment_confidence: String,
    /// "enabled_by_configuration", "disabled_by_configuration", or "unknown".
    pub configuration_status: String,
    /// Explicitly distinguishes Yanfly map shortcuts from global bindings.
    pub context: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EffectiveInputSlot {
    pub input_kind: String,
    pub input_index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_name: Option<String>,
    /// Declarations in the RPG Maker core; NOT necessarily current bindings.
    pub core_default_actions: Vec<String>,
    /// All observed plugin writes, including disabled ones (for auditability).
    pub plugin_candidates: Vec<EffectiveMapperCandidate>,
    /// Candidate status, not a claim about the active game runtime.
    pub status: String,
    /// A plain-language warning suitable for a GUI details panel.
    pub note: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UnresolvedInputAction {
    pub action: String,
    pub classification: String,
    pub references: usize,
}

#[derive(Debug, Serialize)]
pub struct EffectiveInputReport {
    pub schema_version: u32,
    pub engine: String,
    pub resolver_schema_version: u32,
    pub inputs: Vec<EffectiveInputSlot>,
    pub unresolved_actions: Vec<UnresolvedInputAction>,
    pub potential_conflict_count: usize,
    pub warnings: Vec<String>,
}

/// Public entry point for a future CLI / Tauri command.
/// Runs the existing resolver once; never writes to the game directory.
pub fn analyze_effective_inputs(path: &Path) -> Result<EffectiveInputReport, String> {
    let resolution = resolve_inputs(path)?;
    Ok(build_effective_input_report(&resolution))
}

/// Pure transformation, easy to unit test without game files.
pub fn build_effective_input_report(report: &InputResolutionReport) -> EffectiveInputReport {
    let mut slots: BTreeMap<(String, u32), EffectiveInputSlot> = BTreeMap::new();

    // Seed the declared keyboard baseline. Do not seed a gamepad baseline:
    // the current core scanner doesn't extract RPG Maker's gamepadMapper.
    for action in &report.core_actions {
        for key in &action.keyboard_keys {
            let entry = slots.entry(("keyboard".into(), key.key_code)).or_insert_with(|| {
                EffectiveInputSlot {
                    input_kind: "keyboard".into(),
                    input_index: key.key_code,
                    key_name: Some(key.key_name.clone()),
                    core_default_actions: Vec::new(),
                    plugin_candidates: Vec::new(),
                    status: String::new(),
                    note: String::new(),
                }
            });
            if !entry.core_default_actions.contains(&action.action) {
                entry.core_default_actions.push(action.action.clone());
            }
        }
    }

    // Configuration evaluations are keyed by plugin + keyboard code + action.
    // Unknown is the safe default for any other or unmatched plugin pattern.
    let evaluations: BTreeMap<(&str, u32, &str), &str> = report
        .conditional_mapping_evaluations
        .iter()
        .map(|eval| {
            ((eval.plugin.as_str(), eval.input_index, eval.action.as_str()), eval.status.as_str())
        })
        .collect();

    for plugin in &report.plugin_candidates {
        for finding in &plugin.javascript_findings {
            let kind = match finding.category.as_str() {
                "keyboard_mapping_assignment" => "keyboard",
                "gamepad_mapping_assignment" => "gamepad",
                _ => continue,
            };
            let (Some(index), Some(action)) = (finding.mapper_index, finding.action.as_deref())
            else {
                continue;
            };
            let status = if kind == "keyboard" {
                evaluations
                    .get(&(finding.plugin.as_str(), index, action))
                    .copied()
                    .unwrap_or("unknown")
            } else {
                "unknown"
            };
            let context = if finding.plugin == "YEP_ButtonCommonEvents"
                && finding.confidence == "static_conditional_assignment"
            {
                "map_context_candidate"
            } else {
                "runtime_context_unknown"
            };
            let slot = slots.entry((kind.into(), index)).or_insert_with(|| {
                EffectiveInputSlot {
                    input_kind: kind.into(),
                    input_index: index,
                    key_name: None,
                    core_default_actions: Vec::new(),
                    plugin_candidates: Vec::new(),
                    status: String::new(),
                    note: String::new(),
                }
            });
            let candidate = EffectiveMapperCandidate {
                plugin: finding.plugin.clone(),
                source_file: finding.source_file.clone(),
                line: finding.line,
                action: action.to_string(),
                assignment_confidence: finding.confidence.clone(),
                configuration_status: status.into(),
                context: context.into(),
            };
            if !slot.plugin_candidates.contains(&candidate) {
                slot.plugin_candidates.push(candidate);
            }
        }
    }

    for slot in slots.values_mut() {
        slot.core_default_actions.sort();
        slot.plugin_candidates.sort_by(|a, b| {
            a.plugin.cmp(&b.plugin)
                .then_with(|| a.source_file.cmp(&b.source_file))
                .then_with(|| a.line.cmp(&b.line))
                .then_with(|| a.action.cmp(&b.action))
        });
        let candidates = &slot.plugin_candidates;
        let relevant: Vec<_> = candidates.iter()
            .filter(|item| item.configuration_status != "disabled_by_configuration")
            .collect();
        let distinct_actions: BTreeSet<_> = relevant.iter().map(|item| item.action.as_str()).collect();
        let mismatches_core = !slot.core_default_actions.is_empty()
            && distinct_actions.iter().any(|action| !slot.core_default_actions.iter().any(|core| core.as_str() == *action));
        let conflicting = distinct_actions.len() > 1 || mismatches_core;
        let has_unknown = relevant.iter().any(|item| item.configuration_status == "unknown");
        let has_conditional = relevant.iter().any(|item| {
            item.assignment_confidence == "static_conditional_assignment"
                || item.context == "map_context_candidate"
        });

        let (status, note) = if relevant.is_empty() && !slot.core_default_actions.is_empty() {
            ("core_default_only", "No enabled or unknown literal plugin writes detected; dynamic runtime changes remain possible.")
        } else if relevant.is_empty() {
            ("disabled_plugin_candidates_only", "All observed plugin assignments were disabled by configuration.")
        } else if conflicting {
            ("potential_override_or_collision", "One or more candidate actions differ from the core default or from each other; actual runtime outcome is unverified.")
        } else if has_unknown {
            ("unverified_plugin_assignment", "Plugin assignment found, but its enabling condition or runtime execution is not verified.")
        } else if has_conditional {
            ("configured_conditional_candidate", "Configuration enables this conditional assignment, but scene and runtime execution still matter.")
        } else {
            ("plugin_assignment_candidate", "A static plugin assignment was found; runtime execution and later overwrites are not proven.")
        };
        slot.status = status.into();
        slot.note = note.into();
    }

    let unresolved_actions = report.action_catalog.iter()
        .filter(|action| action.binding_status == "unresolved_mapping")
        .map(|action| UnresolvedInputAction {
            action: action.action.clone(),
            classification: action.classification.clone(),
            references: action.references.len(),
        })
        .collect();

    let mut warnings = report.warnings.clone();
    warnings.push("Static input inventory: runtime mapper values and plugin execution order are not verified.".into());
    warnings.push("Core gamepad defaults are not yet scanned; gamepad slots show plugin evidence only.".into());

    EffectiveInputReport {
        schema_version: 1,
        engine: report.engine.clone(),
        resolver_schema_version: report.schema_version,
        inputs: slots.into_values().collect(),
        unresolved_actions,
        potential_conflict_count: report.mapping_conflicts.len(),
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{
        input::KeyBinding,
        javascript::JavaScriptInputFinding,
        resolver::{ActionCatalogEntry, ConditionalMappingEvaluation, PluginInputCandidate, ResolvedAction},
    };

    fn report() -> InputResolutionReport {
        InputResolutionReport {
            schema_version: 4,
            engine: "RPG Maker MV".into(),
            core_actions: vec![ResolvedAction {
                action: "pageup".into(),
                keyboard_keys: vec![KeyBinding { key_code: 81, key_name: "Q".into() }],
                source: "rpg_core.js".into(),
                confidence: "declared_core_mapping".into(),
            }],
            plugin_candidates: Vec::new(),
            action_catalog: Vec::new(),
            mapping_conflicts: Vec::new(),
            conditional_mapping_evaluations: Vec::new(),
            plugins_scanned: 0,
            skipped_plugins: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn assignment(kind: &str, index: u32, action: &str, conditional: bool) -> JavaScriptInputFinding {
        JavaScriptInputFinding {
            plugin: "YEP_ButtonCommonEvents".into(),
            source_file: "js/plugins/YEP_ButtonCommonEvents.js".into(),
            line: 807,
            pattern: if kind == "keyboard" { "Input.keyMapper" } else { "Input.gamepadMapper" }.into(),
            category: if kind == "keyboard" { "keyboard_mapping_assignment" } else { "gamepad_mapping_assignment" }.into(),
            confidence: if conditional { "static_conditional_assignment" } else { "static_assignment" }.into(),
            action: Some(action.into()),
            mapper_index: Some(index),
        }
    }

    fn add_candidate(report: &mut InputResolutionReport, finding: JavaScriptInputFinding) {
        report.plugin_candidates.push(PluginInputCandidate {
            plugin: finding.plugin.clone(),
            parameters: Vec::new(),
            javascript_findings: vec![finding],
            status: "javascript_only".into(),
        });
    }

    #[test]
    fn preserves_declared_core_baseline() {
        let output = build_effective_input_report(&report());
        assert_eq!(output.inputs.len(), 1);
        assert_eq!(output.inputs[0].status, "core_default_only");
        assert_eq!(output.inputs[0].core_default_actions, vec!["pageup".to_string()]);
    }

    #[test]
    fn disabled_conditional_assignment_does_not_override_core() {
        let mut input = report();
        add_candidate(&mut input, assignment("keyboard", 81, "q", true));
        input.conditional_mapping_evaluations.push(ConditionalMappingEvaluation {
            plugin: "YEP_ButtonCommonEvents".into(),
            input_index: 81,
            action: "q".into(),
            parameter: "Key Q".into(),
            configured_value: "0".into(),
            status: "disabled_by_configuration".into(),
            context: "map shortcut".into(),
        });
        let output = build_effective_input_report(&input);
        assert_eq!(output.inputs[0].status, "core_default_only");
        assert_eq!(output.inputs[0].plugin_candidates.len(), 1);
        assert_eq!(output.inputs[0].plugin_candidates[0].configuration_status, "disabled_by_configuration");
    }

    #[test]
    fn enabled_conditional_differs_from_core_and_is_not_confirmed_runtime() {
        let mut input = report();
        add_candidate(&mut input, assignment("keyboard", 81, "q", true));
        input.conditional_mapping_evaluations.push(ConditionalMappingEvaluation {
            plugin: "YEP_ButtonCommonEvents".into(),
            input_index: 81,
            action: "q".into(),
            parameter: "Key Q".into(),
            configured_value: "7".into(),
            status: "enabled_by_configuration".into(),
            context: "map shortcut".into(),
        });
        let output = build_effective_input_report(&input);
        assert_eq!(output.inputs[0].status, "potential_override_or_collision");
        assert_eq!(output.inputs[0].plugin_candidates[0].context, "map_context_candidate");
    }

    #[test]
    fn unknown_mapping_must_not_be_assumed_enabled() {
        let mut input = report();
        add_candidate(&mut input, assignment("keyboard", 82, "r", true));
        let output = build_effective_input_report(&input);
        let slot = output.inputs.iter().find(|slot| slot.input_index == 82).unwrap();
        assert_eq!(slot.status, "unverified_plugin_assignment");
        assert_eq!(slot.plugin_candidates[0].configuration_status, "unknown");
    }

    #[test]
    fn keyboard_and_gamepad_indices_are_separate() {
        let mut input = report();
        add_candidate(&mut input, assignment("gamepad", 81, "q", false));
        let output = build_effective_input_report(&input);
        assert_eq!(output.inputs.len(), 2);
        assert!(output.inputs.iter().any(|slot| slot.input_kind == "keyboard"));
        assert!(output.inputs.iter().any(|slot| slot.input_kind == "gamepad"));
    }

    #[test]
    fn exposes_unresolved_catalog_actions_without_inventing_keys() {
        let mut input = report();
        input.action_catalog.push(ActionCatalogEntry {
            action: "attack".into(),
            classification: "custom_or_unmapped".into(),
            confidence: "static_literal_reference".into(),
            keyboard_keys: Vec::new(),
            alias_of: None,
            binding_status: "unresolved_mapping".into(),
            references: Vec::new(),
        });
        let output = build_effective_input_report(&input);
        assert_eq!(output.unresolved_actions.len(), 1);
        assert_eq!(output.unresolved_actions[0].action, "attack");
    }
}
