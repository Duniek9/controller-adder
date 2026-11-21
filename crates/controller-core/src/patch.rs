//! Generate a standalone RPG Maker plugin without modifying the game.
use crate::mapping::{validate_mapping, MappingProfile};

pub fn generate_plugin(profile: &MappingProfile) -> Result<String, String> {
    let validation = validate_mapping(profile);
    if !validation.valid {
        return Err(validation.errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join("; "));
    }
    if profile.bindings.is_empty() {
        return Err("Cannot generate a plugin with no bindings".into());
    }
    // RPG Maker's state uses normal JS objects. Avoid prototype-sensitive names.
    for action in profile.bindings.keys() {
        if matches!(action.as_str(), "__proto__" | "prototype" | "constructor" |
            "toString" | "valueOf" | "hasOwnProperty" | "isPrototypeOf" |
            "propertyIsEnumerable" | "toLocaleString" | "__defineGetter__" |
            "__defineSetter__" | "__lookupGetter__" | "__lookupSetter__") {
            return Err(format!("Unsupported action identifier: {action}"));
        }
    }
    // Encode as JSON text inside a JS string, not as executable source.
    let data = serde_json::to_string(&profile.bindings).map_err(|e| e.to_string())?;
    let literal = serde_json::to_string(&data).map_err(|e| e.to_string())?
        .replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029");
    Ok(include_str!("controller_adder.js").replace("__CONTROLLER_ADDER_BINDINGS__", &literal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::{PhysicalInput, Direction, FORMAT_VERSION};
    use std::collections::BTreeMap;

    fn profile() -> MappingProfile {
        MappingProfile {
            format_version: FORMAT_VERSION,
            engine: "RPG Maker MZ".into(),
            game_path: "/private/example".into(),
            bindings: BTreeMap::from([
                ("e".into(), vec![PhysicalInput::Button { index: 6 }]),
                ("up".into(), vec![PhysicalInput::Axis { index: 1,
                    direction: Direction::Negative, threshold: 0.5 }]),
            ]),
        }
    }

    #[test]
    fn generates_without_private_path_or_placeholder() {
        let js = generate_plugin(&profile()).unwrap();
        assert!(js.contains("Input._pollGamepads"));
        assert!(js.contains("negative"));
        assert!(!js.contains("/private/example"));
        assert!(!js.contains("__CONTROLLER_ADDER_BINDINGS__"));
    }

    #[test]
    fn rejects_invalid_and_empty_profiles() {
        let mut p = profile();
        p.bindings.insert("duplicate".into(), vec![PhysicalInput::Button { index: 6 }]);
        assert!(generate_plugin(&p).is_err());
        p.bindings.clear();
        assert!(generate_plugin(&p).is_err());
    }

    #[test]
    fn rejects_prototype_sensitive_action() {
        let mut p = profile();
        p.bindings.insert("__proto__".into(), vec![PhysicalInput::Button { index: 7 }]);
        assert!(generate_plugin(&p).is_err());
    }
}
