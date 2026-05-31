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
    },
    #[command(about = "List and filter data from a project (prefabs, effects, scenes, rules)")]
    Query {
        #[command(subcommand)]
        subcommand: commands::query::QueryCommand,
    },
}

fn main() {
    let cli = Cli::parse();
    let mode = OutputMode { json: cli.json };

    let result = match cli.command {
        Command::Inspect { subcommand } => commands::inspect::run(subcommand, &mode),
        Command::Validate { project_dir } => commands::validate::run(&project_dir, &mode),
        Command::Query { subcommand } => commands::query::run(subcommand, &mode),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(2);
    }
}
