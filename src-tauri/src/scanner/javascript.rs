use controller_core::scanner::javascript::{
    JavaScriptScanReport, scan_javascript as core_scan_javascript,
};

use std::path::Path;

#[tauri::command]
pub fn scan_javascript(path: String) -> Result<JavaScriptScanReport, String> {
    core_scan_javascript(Path::new(&path))
}
