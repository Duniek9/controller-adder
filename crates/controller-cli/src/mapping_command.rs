use clap::{Subcommand, ValueEnum};
use super::ExportArgs;
use controller_core::output::{OutputKind, write_json};
use controller_core::mapping::{
    scan_actions, fill_suggestions, validate_mapping, edit_binding, remove_binding, MappingProfile, SuggestionMode, PhysicalInput, Direction,
};
use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Mode {
    Conservative,
    Automatic,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AxisDirection { Negative, Positive }

#[derive(Debug, Subcommand)]
pub enum MappingCommands {
    /// List discovered actions, including unresolved custom actions.
    Scan {
        path: PathBuf,
        #[command(flatten)]
        export: ExportArgs,
    },
    /// Generate conservative standard bindings; custom actions remain unassigned.
    Suggest {
        path: PathBuf,
        #[command(flatten)]
        export: ExportArgs,
        #[arg(long, value_enum, default_value = "automatic")]
        mode: Mode,
        /// Fill a previously exported profile without overwriting its bindings.
        #[arg(long)]
        existing: Option<PathBuf>,
    },
    /// Edit a profile, rejecting conflicting or invalid assignments.
    Set {
        profile: PathBuf,
        #[arg(long)]
        action: String,
        #[arg(long, conflicts_with = "axis", required_unless_present = "axis")]
        button: Option<u32>,
        #[arg(long, requires = "direction", conflicts_with = "button")]
        axis: Option<u32>,
        #[arg(long, value_enum, requires = "axis")]
        direction: Option<AxisDirection>,
        #[arg(long, requires = "axis")]
        threshold: Option<f64>,
        /// Add another input instead of replacing the action's inputs.
        #[arg(long)]
        append: bool,
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
        MappingCommands::Scan { path, export } => {
            let catalog = scan_actions(&path)?;
            export.report(&path, "mapping", &catalog)?;
            super::print_json(&catalog)
        },
        MappingCommands::Suggest { path, export, mode, existing } => {
            let catalog = scan_actions(&path)?;
            let existing = existing.as_deref().map(load).transpose()?;
            let mode = match mode {
                Mode::Conservative => SuggestionMode::Conservative,
                Mode::Automatic => SuggestionMode::Automatic,
            };
            let result = fill_suggestions(&catalog, existing, mode)?;
            if let Some(output) = export.destination(OutputKind::Profile, &path, "profile")? {
                write_json(&output, &result.profile)?;
                eprintln!("Saved {}", output.display());
            }
            super::print_json(&result)
        }
        MappingCommands::Set { profile, action, button, axis, direction, threshold, append } => {
            let mut data = load(&profile)?;
            let input = match (button, axis, direction) {
                (Some(index), None, None) => PhysicalInput::Button { index },
                (None, Some(index), Some(direction)) => PhysicalInput::Axis {
                    index, direction: match direction {
                        AxisDirection::Negative => Direction::Negative,
                        AxisDirection::Positive => Direction::Positive,
                    }, threshold: threshold.unwrap_or(0.5),
                },
                _ => return Err("Specify a button or an axis and direction".into()),
            };
            edit_binding(&mut data, action, input, append)?;
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
