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
