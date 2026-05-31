use clap::{Parser, Subcommand};
use std::process;

mod commands;
mod output;

use output::OutputMode;

#[derive(Parser)]
#[command(name = "ironhold", about = "Ironhold CLI — inspect, validate, and query game project assets")]
struct Cli {
    #[arg(long, global = true, help = "Output machine-readable JSON")]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Inspect asset files (glb, texture, audio)")]
    Inspect {
        #[command(subcommand)]
        subcommand: commands::inspect::InspectCommand,
    },
    #[command(about = "Parse and validate all RON files in a project directory")]
    Validate {
        /// Path to the project directory (e.g. assets/projects/particles_demo)
        project_dir: std::path::PathBuf,
        /// Also report keys defined in assets.ron / prefabs.ron that are never referenced anywhere
        #[arg(long)]
        strict: bool,
    },
    #[command(about = "List and filter data from a project (prefabs, effects, scenes, rules)")]
    Query {
        #[command(subcommand)]
        subcommand: commands::query::QueryCommand,
    },
    #[command(about = "Watch a project directory and re-validate on every .ron file change")]
    Watch {
        /// Path to the project directory (e.g. assets/projects/particles_demo)
        project_dir: std::path::PathBuf,
    },
    #[command(about = "Print a compact summary of a project (scenes, prefabs, effects, rules, size)")]
    Stats {
        /// Path to the project directory (e.g. assets/projects/particles_demo)
        project_dir: std::path::PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    let mode = OutputMode { json: cli.json };

    let result = match cli.command {
        Command::Inspect { subcommand } => commands::inspect::run(subcommand, &mode),
        Command::Validate { project_dir, strict } => {
            commands::validate::run(&project_dir, &mode, strict)
        }
        Command::Query { subcommand } => commands::query::run(subcommand, &mode),
        Command::Watch { project_dir } => commands::watch::run(&project_dir),
        Command::Stats { project_dir } => commands::stats::run(&project_dir, &mode),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(2);
    }
}
