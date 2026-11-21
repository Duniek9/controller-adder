//! Shared, read-only controller profile discovery and validation.
use crate::scanner::effective::{analyze_effective_inputs, EffectiveInputReport};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, path::Path};

pub const FORMAT_VERSION: u32 = 1;
// Standard browser Gamepad API button indices (not keyboard codes).
pub const MAX_BUTTON: u32 = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MappingProfile {
    pub format_version: u32,
    pub engine: String,
    pub game_path: String,
    pub bindings: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MappingActions {
    pub engine: String,
    pub game_path: String,
    pub actions: Vec<String>,
    pub warnings: Vec<String>,
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
    actions.retain(|action| !action.trim().is_empty());
    actions.into_iter().collect()
}

pub fn scan_actions(path: &Path) -> Result<MappingActions, String> {
    let report = analyze_effective_inputs(path)?;
    let actions = actions_from_report(&report);
    let canonical = path.canonicalize().map_err(|e| format!("Cannot resolve game directory: {e}"))?;
    let game_path = canonical.to_str().ok_or("Game path is not valid UTF-8")?.to_owned();
    Ok(MappingActions { engine: report.engine, game_path, actions, warnings: report.warnings })
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
            warnings: vec![] }
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
    fn profile_json_roundtrip() {
        let p = suggest_from_actions(&catalog());
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<MappingProfile>(&json).unwrap(), p);
    }
}
