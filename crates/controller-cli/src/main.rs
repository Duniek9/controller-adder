use clap::{Parser, Subcommand};
use controller_core::{
    detect_engine,
    scanner::{
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

#[derive(Debug, Subcommand)]
enum Commands {
    /// Detect the game engine.
    Detect {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Scan engine input defaults.
    Inputs {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Scan plugin registry entries.
    Plugins {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Scan enabled plugin JavaScript for input-related code.
    Javascript {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Combine scanner evidence into a report and action catalog.
    Resolve {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

fn print_json<T: Serialize>(value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|err| format!("Cannot serialize JSON: {err}"))?;
    println!("{json}");
    Ok(())
}

fn execute(command: Commands) -> Result<(), String> {
    match command {
        Commands::Detect { path, json } => {
            let engine = detect_engine(&path)?;
            if json {
                print_json(&serde_json::json!({
                    "path": path,
                    "engine": engine,
                }))?;
            } else {
                println!("Engine: {engine}");
            }
        }
        Commands::Inputs { path, json } => {
            let actions = scan_inputs(path.to_string_lossy().into_owned())?;
            if !json {
                println!("Detected {} core input actions", actions.len());
            }
            print_json(&actions)?;
        }
        Commands::Plugins { path, json } => {
            let plugins = scan_plugins(path.to_string_lossy().into_owned())?;
            if !json {
                println!("Plugin scan result:");
            }
            print_json(&plugins)?;
        }
        Commands::Javascript { path, json } => {
            let report = scan_javascript(&path)?;
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
        Commands::Resolve { path, json } => {
            let report = resolve_inputs(&path)?;
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
