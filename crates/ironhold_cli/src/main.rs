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
}

fn main() {
    let cli = Cli::parse();
    let mode = OutputMode { json: cli.json };

    let result = match cli.command {
        Command::Inspect { subcommand } => commands::inspect::run(subcommand, &mode),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(2);
    }
}
