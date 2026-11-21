//! Shared, read-only controller profile discovery and validation.
use crate::scanner::effective::{analyze_effective_inputs, EffectiveInputReport};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, path::Path};

pub const FORMAT_VERSION: u32 = 1;
// Standard browser Gamepad API button indices (not keyboard codes).
pub const MAX_BUTTON: u32 = 15;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MappingProfile {
    pub format_version: u32,
    pub engine: String,
    pub game_path: String,
    pub bindings: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingActions {
    pub engine: String,
    pub game_path: String,
    pub actions: Vec<String>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub evidence: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ValidationIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MappingValidation {
    pub valid: bool,
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}

const DEFAULTS: &[(&str, u32)] = &[
    ("ok", 0), ("cancel", 1), ("shift", 2), ("menu", 3),
    ("pageup", 4), ("pagedown", 5),
    ("up", 12), ("down", 13), ("left", 14), ("right", 15),
];

fn issue(code: &str, message: impl Into<String>) -> ValidationIssue {
    ValidationIssue { code: code.into(), message: message.into() }
}

/// Keep action identifiers exact (case-sensitive); exclude proven disabled writes.
pub fn actions_from_report(report: &EffectiveInputReport) -> Vec<String> {
    let mut actions = BTreeSet::new();
    for slot in &report.inputs {
        actions.extend(slot.core_default_actions.iter().cloned());
        for candidate in &slot.plugin_candidates {
            if candidate.configuration_status != "disabled_by_configuration" {
                actions.insert(candidate.action.clone());
            }
        }
    }
    actions.extend(report.unresolved_actions.iter().map(|a| a.action.clone()));
    actions.extend(report.candidate_mapped_actions.iter().map(|a| a.action.clone()));
    if matches!(report.engine.as_str(), "RPG Maker MV" | "RPG Maker MZ") {
        actions.insert("cancel".into());
        actions.insert("menu".into());
    }
    actions.retain(|action| !action.trim().is_empty());
    actions.into_iter().collect()
}

pub fn scan_actions(path: &Path) -> Result<MappingActions, String> {
    let report = analyze_effective_inputs(path)?;
    let actions = actions_from_report(&report);
    let canonical = path.canonicalize().map_err(|e| format!("Cannot resolve game directory: {e}"))?;
    let game_path = canonical.to_str().ok_or("Game path is not valid UTF-8")?.to_owned();
    let mut evidence = BTreeMap::new();
    for action in &actions {
        let status = if report.inputs.iter().any(|slot| slot.core_default_actions.contains(action))
            || matches!(action.as_str(), "cancel" | "menu") {
            "core"
        } else if report.inputs.iter().flat_map(|slot| &slot.plugin_candidates)
            .any(|c| c.action == *action && c.configuration_status == "enabled_by_configuration") {
            "configured_plugin"
        } else if report.inputs.iter().flat_map(|slot| &slot.plugin_candidates)
            .any(|c| c.action == *action && c.configuration_status != "disabled_by_configuration") {
            "unverified_plugin"
        } else {
            "unresolved"
        };
        evidence.insert(action.clone(), status.to_owned());
    }
    Ok(MappingActions { engine: report.engine, game_path, actions, warnings: report.warnings, evidence })
}

/// Conservative suggestion: assign only recognized standard actions.
pub fn suggest_from_actions(catalog: &MappingActions) -> MappingProfile {
    let mut bindings = BTreeMap::new();
    for &(action, button) in DEFAULTS {
        if catalog.actions.iter().any(|a| a == action) {
            bindings.insert(action.to_owned(), button);
        }
    }
    MappingProfile { format_version: FORMAT_VERSION, engine: catalog.engine.clone(),
        game_path: catalog.game_path.clone(), bindings }
}

pub fn suggest_mapping(path: &Path) -> Result<MappingProfile, String> {
    Ok(suggest_from_actions(&scan_actions(path)?))
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionMode {
    Conservative,
    Automatic,
}

#[derive(Debug, Serialize)]
pub struct SuggestionReport {
    pub profile: MappingProfile,
    pub unassigned: Vec<String>,
    pub added: Vec<String>,
    pub warnings: Vec<String>,
}

/// Fill missing bindings only. Standard actions get their preferred buttons,
/// and automatic mode uses remaining non-directional buttons for plugin actions.
pub fn fill_suggestions(
    catalog: &MappingActions,
    existing: Option<MappingProfile>,
    mode: SuggestionMode,
) -> Result<SuggestionReport, String> {
    let mut profile = existing.unwrap_or_else(|| MappingProfile {
        format_version: FORMAT_VERSION,
        engine: catalog.engine.clone(),
        game_path: catalog.game_path.clone(),
        bindings: BTreeMap::new(),
    });
    if profile.engine != catalog.engine || profile.game_path != catalog.game_path {
        return Err("Profile belongs to a different game".into());
    }
    if !validate_mapping(&profile).valid {
        return Err("Resolve invalid existing bindings before suggesting".into());
    }
    let mut used: BTreeSet<u32> = profile.bindings.values().copied().collect();
    let mut added = Vec::new();
    for &(action, button) in DEFAULTS {
        if catalog.actions.iter().any(|a| a == action)
            && !profile.bindings.contains_key(action) && used.insert(button) {
            profile.bindings.insert(action.into(), button);
            added.push(action.into());
        }
    }
    let mut warnings = Vec::new();
    if mode == SuggestionMode::Automatic {
        let mut candidates: Vec<_> = catalog.actions.iter().filter(|a| {
            !profile.bindings.contains_key(*a)
                && matches!(catalog.evidence.get(*a).map(String::as_str),
                    Some("configured_plugin" | "unverified_plugin" | "unresolved"))
        }).collect();
        candidates.sort_by_key(|a| match catalog.evidence.get(*a).map(String::as_str) {
            Some("configured_plugin") => 0,
            Some("unverified_plugin") => 1,
            _ => 2,
        });
        for action in candidates {
            let Some(button) = [7, 6, 11, 10, 8, 9, 4, 5, 2, 3, 0, 1]
                .into_iter().find(|b| !used.contains(b)) else { break; };
            used.insert(button);
            profile.bindings.insert(action.clone(), button);
            added.push(action.clone());
            if catalog.evidence.get(action).map(String::as_str) != Some("configured_plugin") {
                warnings.push(format!("{action}: static evidence only; review before use"));
            }
        }
    }
    let unassigned = catalog.actions.iter().filter(|a| !profile.bindings.contains_key(*a))
        .cloned().collect();
    Ok(SuggestionReport { profile, unassigned, added, warnings })
}

pub fn validate_mapping(profile: &MappingProfile) -> MappingValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    if profile.format_version != FORMAT_VERSION {
        errors.push(issue("unsupported_version", format!("Expected profile version {FORMAT_VERSION}")));
    }
    if !matches!(profile.engine.as_str(), "RPG Maker MV" | "RPG Maker MZ") {
        errors.push(issue("unsupported_engine", "Expected RPG Maker MV or MZ"));
    }
    if profile.game_path.trim().is_empty() {
        errors.push(issue("missing_game_path", "Game path must not be empty"));
    }
    let mut used = BTreeMap::<u32, &str>::new();
    for (action, &button) in &profile.bindings {
        if action.trim().is_empty() {
            errors.push(issue("empty_action", "Action names must not be blank"));
        }
        if button > MAX_BUTTON {
            errors.push(issue("invalid_button", format!("{action}: button {button} is outside 0..={MAX_BUTTON}")));
        }
        if let Some(previous) = used.insert(button, action.as_str()) {
            errors.push(issue("duplicate_button", format!("Button {button} is assigned to both {previous} and {action}")));
        }
    }
    for action in ["ok", "cancel", "up", "down", "left", "right"] {
        if !profile.bindings.contains_key(action) {
            warnings.push(issue("missing_essential_action", format!("No binding for {action}")));
        }
    }
    MappingValidation { valid: errors.is_empty(), errors, warnings }
}

/// Transactional in-memory edit: invalid changes leave the original untouched.
pub fn set_binding(profile: &mut MappingProfile, action: String, button: u32) -> Result<(), String> {
    let mut next = profile.clone();
    next.bindings.insert(action, button);
    let validation = validate_mapping(&next);
    if !validation.valid {
        return Err(validation.errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join("; "));
    }
    *profile = next;
    Ok(())
}

pub fn remove_binding(profile: &mut MappingProfile, action: &str) -> bool {
    profile.bindings.remove(action).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> MappingActions {
        MappingActions { engine: "RPG Maker MV".into(), game_path: "/example".into(),
            actions: ["ok", "cancel", "up", "e", "E", "custom"].iter().map(|s| s.to_string()).collect(),
            warnings: vec![], evidence: BTreeMap::new() }
    }

    #[test]
    fn suggestion_does_not_guess_custom_actions() {
        let p = suggest_from_actions(&catalog());
        assert_eq!(p.bindings.len(), 3);
        assert_eq!(p.bindings["ok"], 0);
        assert!(!p.bindings.contains_key("custom"));
        assert!(validate_mapping(&p).valid);
    }

    #[test]
    fn collision_rejected_without_mutation() {
        let mut p = suggest_from_actions(&catalog());
        let original = p.clone();
        assert!(set_binding(&mut p, "e".into(), 0).is_err());
        assert_eq!(p, original);
        assert!(set_binding(&mut p, "e".into(), 7).is_ok());
        assert!(set_binding(&mut p, "E".into(), 6).is_ok());
        assert!(remove_binding(&mut p, "e"));
        assert!(p.bindings.contains_key("E"));
    }

    #[test]
    fn invalid_button_and_version_are_rejected() {
        let mut p = suggest_from_actions(&catalog());
        assert!(set_binding(&mut p, "custom".into(), 17).is_err());
        p.format_version = 2;
        assert!(!validate_mapping(&p).valid);
    }

    #[test]
    fn automatic_preserves_manual_and_prioritizes_configured() {
        let mut c = catalog();
        c.evidence.insert("e".into(), "configured_plugin".into());
        c.evidence.insert("E".into(), "unverified_plugin".into());
        c.evidence.insert("custom".into(), "unresolved".into());
        let mut p = suggest_from_actions(&c);
        set_binding(&mut p, "manual".into(), 7).unwrap();
        let result = fill_suggestions(&c, Some(p), SuggestionMode::Automatic).unwrap();
        assert_eq!(result.profile.bindings["manual"], 7);
        assert_eq!(result.profile.bindings["e"], 6);
        assert!(result.profile.bindings.contains_key("E"));
        assert!(validate_mapping(&result.profile).valid);
        assert!(result.unassigned.is_empty());
    }

    #[test]
    fn profile_json_roundtrip() {
        let p = suggest_from_actions(&catalog());
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<MappingProfile>(&json).unwrap(), p);
    }
}
