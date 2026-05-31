use clap::{Parser, Subcommand};
use std::process;

mod commands;
mod output;

use output::OutputMode;

#[derive(Parser)]
#[command(
    name = "ironhold",
    about = "Ironhold CLI — inspect, validate, and query game project assets",
    after_help = "NOTE: --json is a global flag and must be placed before the subcommand:\n  ironhold --json validate assets/projects/quick_scene/\n  ironhold --json query prefabs assets/projects/particles_demo/\n  ironhold --json stats assets/projects/particles_demo/"
)]
struct Cli {
    #[arg(long, global = true, help = "Output machine-readable JSON")]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "Inspect asset files (glb, texture, audio)",
        after_help = "Examples:\n  ironhold inspect glb assets/shared/models/creatures/orc-enemy.glb\n  ironhold inspect texture assets/shared/textures/decals/circle_filled.png\n  ironhold inspect audio assets/shared/audio/boulder/boulder-push1.wav\n  ironhold --json inspect glb assets/shared/models/creatures/dragon.glb"
    )]
    Inspect {
        #[command(subcommand)]
        subcommand: commands::inspect::InspectCommand,
    },
    #[command(
        about = "Parse and validate all RON files in a project directory",
        after_help = "Exit codes: 0 = all valid, 1 = errors or strict warnings found, 2 = tool/IO error\n\nExamples:\n  ironhold validate assets/projects/quick_scene/\n  ironhold validate --strict assets/projects/particles_demo/\n  ironhold --json validate assets/projects/quick_scene/"
    )]
    Validate {
        /// Path to the project directory (e.g. assets/projects/particles_demo)
        project_dir: std::path::PathBuf,
        /// Also report keys defined in assets.ron / prefabs.ron that are never referenced anywhere (exit code 1 if any found)
        #[arg(long)]
        strict: bool,
    },
    #[command(
        about = "List and filter data from a project (prefabs, effects, scenes, rules)",
        after_help = "Examples:\n  ironhold query prefabs assets/projects/particles_demo/\n  ironhold query prefabs assets/projects/particles_demo/ --filter kind=actor\n  ironhold query effects assets/projects/particles_demo/ --filter additive=true\n  ironhold query scenes   assets/projects/3rd_person_game_demo/\n  ironhold query rules    assets/projects/3rd_person_game_demo/\n  ironhold query actions  assets/projects/3rd_person_game_demo/\n  ironhold query events   assets/projects/particles_demo/\n  ironhold --json query prefabs assets/projects/particles_demo/ --keys-only"
    )]
    Query {
        #[command(subcommand)]
        subcommand: commands::query::QueryCommand,
    },
    #[command(
        about = "Watch a project directory and re-validate on every .ron file change",
        after_help = "Note: --json has no effect on watch — output is always human-readable.\n\nExamples:\n  ironhold watch assets/projects/quick_scene/\n  ironhold watch assets/projects/particles_demo/"
    )]
    Watch {
        /// Path to the project directory (e.g. assets/projects/particles_demo)
        project_dir: std::path::PathBuf,
    },
    #[command(
        about = "Print a compact summary of a project (scenes, prefabs, effects, rules, size)",
        after_help = "Examples:\n  ironhold stats assets/projects/particles_demo/\n  ironhold stats assets/projects/3rd_person_game_demo/\n  ironhold --json stats assets/projects/quick_scene/"
    )]
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
