//! Desktop bridge to the shared patch-management backend.
use controller_core::{
    installer::{install_plugin, patch_status, uninstall_plugin, update_plugin,
        InstallReport, StatusReport, UninstallReport, UpdateReport},
    mapping::MappingProfile,
};
use std::{path::Path, sync::Mutex};

static PATCH_LOCK: Mutex<()> = Mutex::new(());

#[tauri::command]
pub async fn controller_patch_status(game_path: String) -> Result<StatusReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = PATCH_LOCK.lock().map_err(|_| "Patch operation lock poisoned")?;
        patch_status(Path::new(&game_path))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn install_controller_patch(profile: MappingProfile) -> Result<InstallReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = PATCH_LOCK.lock().map_err(|_| "Patch operation lock poisoned")?;
        install_plugin(&profile)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn update_controller_patch(profile: MappingProfile) -> Result<UpdateReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = PATCH_LOCK.lock().map_err(|_| "Patch operation lock poisoned")?;
        update_plugin(&profile)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn uninstall_controller_patch(game_path: String) -> Result<UninstallReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = PATCH_LOCK.lock().map_err(|_| "Patch operation lock poisoned")?;
        uninstall_plugin(Path::new(&game_path))
    }).await.map_err(|e| e.to_string())?
}
