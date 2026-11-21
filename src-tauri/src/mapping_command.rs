use std::path::Path;
use controller_core::mapping::{
    scan_actions, fill_suggestions, validate_mapping, MappingActions, MappingProfile,
    MappingValidation, SuggestionMode, SuggestionReport,
};

#[tauri::command]
pub async fn scan_mapping_actions(game_path: String) -> Result<MappingActions, String> {
    tauri::async_runtime::spawn_blocking(move || scan_actions(Path::new(&game_path)))
        .await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn suggest_controller_mapping(
    catalog: MappingActions, profile: MappingProfile, mode: SuggestionMode,
) -> Result<SuggestionReport, String> {
    fill_suggestions(&catalog, Some(profile), mode)
}

#[tauri::command]
pub fn validate_controller_mapping(profile: MappingProfile) -> MappingValidation {
    validate_mapping(&profile)
}

#[tauri::command]
pub fn export_mapping_profile(profile: MappingProfile) -> Result<String, String> {
    use controller_core::output::{default_path, write_json, OutputKind};
    let result = validate_mapping(&profile);
    if !result.valid {
        return Err(result.errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join("; "));
    }
    let path = default_path(OutputKind::Profile, Path::new(&profile.game_path), "profile")?;
    write_json(&path, &profile)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn import_mapping_profile(file_path: String, game_path: String, engine: String) -> Result<MappingProfile, String> {
    use std::{fs, io::Read};
    let mut bytes = Vec::new();
    let file = fs::File::open(&file_path).map_err(|e| format!("Cannot open profile: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() { return Err("Select a regular JSON file".into()); }
    file.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|e| format!("Cannot read profile: {e}"))?;
    if bytes.len() > 2 * 1024 * 1024 { return Err("Profile exceeds 2 MiB".into()); }
    let mut profile: MappingProfile = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid profile JSON: {e}"))?;
    let selected = Path::new(&game_path).canonicalize().map_err(|e| format!("Invalid selected game path: {e}"))?;
    let imported = Path::new(&profile.game_path).canonicalize().map_err(|e| format!("Invalid game path in profile: {e}"))?;
    if imported != selected || profile.engine != engine {
        return Err("Profile belongs to a different game or engine".into());
    }
    profile.game_path = selected.to_str().ok_or("Game path is not valid UTF-8")?.to_owned();
    let checked = validate_mapping(&profile);
    if !checked.valid {
        return Err(checked.errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join("; "));
    }
    Ok(profile)
}

#[tauri::command]
pub fn profile_import_directory() -> Result<Option<String>, String> {
    let directory = controller_core::output::output_root()?.join("profiles");
    Ok(directory.is_dir().then(|| directory.to_string_lossy().into_owned()))
}

#[tauri::command]
pub fn export_mapping_profile_as(profile: MappingProfile, file_path: String) -> Result<String, String> {
    let result = validate_mapping(&profile);
    if !result.valid {
        return Err(result.errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join("; "));
    }
    let path = Path::new(&file_path);
    if path.extension().and_then(|e| e.to_str()).map_or(true, |e| !e.eq_ignore_ascii_case("json")) {
        return Err("Choose a .json filename".into());
    }
    controller_core::output::write_json(path, &profile)?;
    Ok(file_path)
}
