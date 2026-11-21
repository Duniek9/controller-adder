mod mapping_command;

use clap::{Args, Parser, Subcommand};
use controller_core::output::{default_path, write_json, write_new, OutputKind};
use controller_core::{
    detect_engine,
    scanner::{
        effective::analyze_effective_inputs,
        input::scan_inputs,
        javascript::scan_javascript,
        plugins::scan_plugins,
        resolver::resolve_inputs,
    },
};
use serde::Serialize;
use std::{path::PathBuf, process::ExitCode};

#[derive(Debug, Parser)]
#[command(name = "controller-cli", about = "Inspect game engines and input controls")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Save in the default output directory.
    #[arg(long, conflicts_with = "output")]
    pub save: bool,
    /// Write to a custom file without overwriting.
    #[arg(long)]
    pub output: Option<PathBuf>,
}

impl ExportArgs {
    pub fn destination(&self, kind: OutputKind, game: &std::path::Path, label: &str) -> Result<Option<PathBuf>, String> {
        if let Some(path) = &self.output { return Ok(Some(path.clone())); }
        if self.save { return default_path(kind, game, label).map(Some); }
        Ok(None)
    }

    pub fn report<T: Serialize>(&self, game: &std::path::Path, label: &str, value: &T) -> Result<(), String> {
        if let Some(path) = self.destination(OutputKind::Scan, game, label)? {
            write_json(&path, value)?;
            eprintln!("Saved {}", path.display());
        }
        Ok(())
    }
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Generate or install controller plugins.
    Patch {
        #[command(subcommand)]
        command: PatchCommands,
    },
    /// Discover, suggest, edit and validate controller profiles.
    Mapping {
        #[command(subcommand)]
        command: mapping_command::MappingCommands,
    },
    /// Detect the game engine.
    Detect {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        export: ExportArgs,
    },
    /// Scan engine input defaults.
    Inputs {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        export: ExportArgs,
    },
    /// Scan plugin registry entries.
    Plugins {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        export: ExportArgs,
    },
    /// Scan enabled plugin JavaScript for input-related code.
    Javascript {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        export: ExportArgs,
    },
    /// Build a conservative keyboard/gamepad input inventory for the GUI.
    Effective {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        export: ExportArgs,
    },
    /// Combine scanner evidence into a report and action catalog.
    Resolve {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        export: ExportArgs,
    },
}

#[derive(Debug, Subcommand)]
enum PatchCommands {
    /// Install and register the generated plugin for the game in a profile.
    /// Close the game and its editor before running this command.
    Apply {
        profile: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Generate JavaScript to stdout or a new output file.
    Generate {
        profile: PathBuf,
        #[command(flatten)]
        export: ExportArgs,
    },
}

fn execute_patch(command: PatchCommands) -> Result<(), String> {
    use std::fs;
    match command {
        PatchCommands::Apply { profile, json } => {
            let bytes = fs::read(&profile).map_err(|e| format!("Cannot read profile: {e}"))?;
            let profile: controller_core::mapping::MappingProfile =
                serde_json::from_slice(&bytes).map_err(|e| format!("Invalid profile: {e}"))?;
            let report = controller_core::installer::install_plugin(&profile)?;
            if json {
                print_json(&report)?;
            } else {
                println!("Installed for {}", report.engine);
                println!("Plugin: {}", report.plugin.display());
                println!("Registry: {}", report.registry.display());
                println!("Original registry backup: {}", report.backup.display());
            }
            Ok(())
        }
        PatchCommands::Generate { profile, export } => {
            let bytes = fs::read(&profile).map_err(|e| format!("Cannot read profile: {e}"))?;
            let profile: controller_core::mapping::MappingProfile =
                serde_json::from_slice(&bytes).map_err(|e| format!("Invalid profile: {e}"))?;
            let plugin = controller_core::patch::generate_plugin(&profile)?;
            if let Some(path) = export.destination(OutputKind::Patch, std::path::Path::new(&profile.game_path), "patch")? {
                write_new(&path, plugin.as_bytes())?;
                eprintln!("Generated {}", path.display());
            } else {
                print!("{plugin}");
            }
            Ok(())
        }
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|err| format!("Cannot serialize JSON: {err}"))?;
    println!("{json}");
    Ok(())
}

fn execute(command: Commands) -> Result<(), String> {
    match command {
        Commands::Patch { command } => execute_patch(command)?,
        Commands::Mapping { command } => mapping_command::execute(command)?,
        Commands::Detect { path, json, export } => {
            let engine = detect_engine(&path)?;
            export.report(&path, "detect", &serde_json::json!({"path": path, "engine": engine}))?;
            if json {
                print_json(&serde_json::json!({
                    "path": path,
                    "engine": engine,
                }))?;
            } else {
                println!("Engine: {engine}");
            }
        }
        Commands::Inputs { path, json, export } => {
            let actions = scan_inputs(path.to_string_lossy().into_owned())?;
            export.report(&path, "inputs", &actions)?;
            if !json {
                println!("Detected {} core input actions", actions.len());
            }
            print_json(&actions)?;
        }
        Commands::Plugins { path, json, export } => {
            let plugins = scan_plugins(path.to_string_lossy().into_owned())?;
            export.report(&path, "plugins", &plugins)?;
            if !json {
                println!("Plugin scan result:");
            }
            print_json(&plugins)?;
        }
        Commands::Javascript { path, json, export } => {
            let report = scan_javascript(&path)?;
            export.report(&path, "javascript", &report)?;
            if !json {
                println!(
                    "Scanned {} plugins, found {} JavaScript references ({} skipped)",
                    report.plugins_scanned,
                    report.findings.len(),
                    report.skipped.len()
                );
            }
            print_json(&report)?;
        }
        Commands::Effective { path, json, export } => {
            let report = analyze_effective_inputs(&path)?;
            export.report(&path, "effective", &report)?;
            if json {
                print_json(&report)?;
            } else {
                println!("Engine: {}", report.engine);
                println!("Input slots: {}", report.inputs.len());
                println!("Unresolved actions: {}", report.unresolved_actions.len());
                println!("Potential conflicts: {}", report.potential_conflict_count);
                for slot in &report.inputs {
                    let baseline = slot.core_default_actions.join(", ");
                    println!(
                        "  {}[{}]  {}  baseline: {}  plugin candidates: {}",
                        slot.input_kind,
                        slot.input_index,
                        slot.status,
                        if baseline.is_empty() { "none" } else { &baseline },
                        slot.plugin_candidates.len()
                    );
                }
                for warning in &report.warnings {
                    println!("Warning: {warning}");
                }
            }
        }
        Commands::Resolve { path, json, export } => {
            let report = resolve_inputs(&path)?;
            export.report(&path, "resolve", &report)?;
            if json {
                // The report already contains all fields, including
                // action_catalog and any new mapper evidence.
                print_json(&report)?;
            } else {
                println!("Engine: {}", report.engine);
                println!("Schema version: {}", report.schema_version);
                println!("Core actions: {}", report.core_actions.len());
                println!("Plugin input candidates: {}", report.plugin_candidates.len());
                println!("Plugins scanned: {}", report.plugins_scanned);
                println!("\nAction catalog ({} entries):", report.action_catalog.len());

                for action in &report.action_catalog {
                    let keys = action.keyboard_keys
                        .iter()
                        .map(|key| key.key_name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let keys = if keys.is_empty() { "unresolved" } else { &keys };
                    println!(
                        "  {:<16} {:<26} keys: {:<22} references: {}",
                        action.action,
                        action.classification,
                        keys,
                        action.references.len()
                    );
                }

                let assignments: Vec<_> = report.plugin_candidates
                    .iter()
                    .flat_map(|plugin| &plugin.javascript_findings)
                    .filter(|finding| {
                        finding.category == "keyboard_mapping_assignment"
                            || finding.category == "gamepad_mapping_assignment"
                    })
                    .collect();
                println!("\nLiteral mapper assignments found: {}", assignments.len());
                for assignment in assignments {
                    println!(
                        "  {}[{}] = {:?}  ({}:{})",
                        assignment.pattern,
                        assignment.mapper_index.unwrap_or_default(),
                        assignment.action,
                        assignment.source_file,
                        assignment.line
                    );
                }
                if !report.warnings.is_empty() {
                    println!("\nWarnings:");
                    for warning in &report.warnings {
                        println!("  - {warning}");
                    }
                }
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match execute(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        }
    }
}
