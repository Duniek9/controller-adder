use clap::Subcommand;
use controller_core::mapping::{
    scan_actions, suggest_mapping, validate_mapping, set_binding, remove_binding, MappingProfile,
};
use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Subcommand)]
pub enum MappingCommands {
    /// List discovered actions, including unresolved custom actions.
    Scan { path: PathBuf },
    /// Generate conservative standard bindings; custom actions remain unassigned.
    Suggest {
        path: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Edit a profile, rejecting conflicting or invalid assignments.
    Set {
        profile: PathBuf,
        #[arg(long)]
        action: String,
        #[arg(long)]
        button: u32,
    },
    /// Remove an exact, case-sensitive action binding.
    Remove {
        profile: PathBuf,
        #[arg(long)]
        action: String,
    },
    /// Print the profile as JSON.
    Show { profile: PathBuf },
    /// Validate profile structure and button assignments (not runtime support).
    Validate {
        profile: PathBuf,
        /// JSON output is always enabled; accepted for scripting consistency.
        #[arg(long)]
        json: bool,
    },
}

fn load(path: &Path) -> Result<MappingProfile, String> {
    let bytes = fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid profile JSON: {e}"))
}

// Write beside the destination and rename only after a complete write.
// create_new prevents overwriting a pre-existing temporary file.
fn save(path: &Path, profile: &MappingProfile, overwrite: bool) -> Result<(), String> {
    use std::io::Write;
    let json = serde_json::to_vec_pretty(profile).map_err(|e| e.to_string())?;
    let name = path.file_name().ok_or("Expected a profile filename")?;
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let temp = parent.join(format!(".{}.{}.tmp", name.to_string_lossy(), std::process::id()));
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temp)
        .map_err(|e| format!("Cannot create temporary profile: {e}"))?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(&json)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        if overwrite {
            fs::rename(&temp, path)?;
        } else {
            // No clobber, including if another process creates the destination.
            fs::hard_link(&temp, path)?;
            fs::remove_file(&temp)?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|e| format!("Cannot save {}: {e}", path.display()))
}

pub fn execute(command: MappingCommands) -> Result<(), String> {
    match command {
        MappingCommands::Scan { path } => super::print_json(&scan_actions(&path)?),
        MappingCommands::Suggest { path, output } => {
            let profile = suggest_mapping(&path)?;
            if let Some(output) = output { save(&output, &profile, false)?; }
            super::print_json(&profile)
        }
        MappingCommands::Set { profile, action, button } => {
            let mut data = load(&profile)?;
            set_binding(&mut data, action, button)?;
            save(&profile, &data, true)?;
            super::print_json(&data)
        }
        MappingCommands::Remove { profile, action } => {
            let mut data = load(&profile)?;
            if !remove_binding(&mut data, &action) {
                return Err(format!("No binding for {action}"));
            }
            save(&profile, &data, true)?;
            super::print_json(&data)
        }
        MappingCommands::Show { profile } => super::print_json(&load(&profile)?),
        MappingCommands::Validate { profile, json: _ } => {
            let report = validate_mapping(&load(&profile)?);
            super::print_json(&report)?;
            if report.valid { Ok(()) } else { Err("Mapping validation failed".into()) }
        }
    }
}
