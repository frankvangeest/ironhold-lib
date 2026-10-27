//! `ironhold backend config`: generates the server-side config for the Nakama module from projects' `backend.ron` files
//! (`planning/features/backend_abstraction.md`, v2). A server-admin step: adding, renaming or removing a leaderboard needs
//! a regenerated config and a redeploy.

use std::path::{Path, PathBuf};

use clap::Subcommand;
use ironhold_core::schema::backend::BackendConfig;
use ironhold_core::schema::ProjectConfig;
use ironhold_nakama::server_config::{generate_games_lua, GameServerConfig, GAMES_FILE, MODULE_LUA};

use super::utils::{find_project_ron, ron_from_str};
use crate::output::OutputMode;

#[derive(Subcommand)]
pub enum BackendCommand {
    #[command(
        about = "Generate the Nakama server module config for one or more projects",
        after_help = "The projects named together become one server: each game_id may appear once.\n\nExamples:\n  ironhold backend config assets/projects/backend_demo --out deploy/nakama-modules\n  ironhold backend config assets/projects/game_a assets/projects/game_b --out deploy/nakama-modules\n\nWithout --out the generated ironhold_games.lua is printed. With --out it writes ironhold_games.lua and ironhold.lua\n(the generic module) into that folder; copy both into Nakama's modules directory and restart Nakama."
    )]
    Config {
        /// Project directories to include (each with a `backend_path` in its `.project.ron`)
        #[arg(required = true)]
        project_dirs: Vec<PathBuf>,
        /// Folder to write `ironhold_games.lua` and `ironhold.lua` into
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

pub fn run(cmd: BackendCommand, mode: &OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        BackendCommand::Config { project_dirs, out } => Ok(config(&project_dirs, out.as_deref(), mode)?),
    }
}

struct Loaded {
    game_id: String,
    config: BackendConfig,
}

fn load(dir: &Path) -> Result<Loaded, String> {
    let ron_name = find_project_ron(dir).ok_or_else(|| format!("{}: no *.project.ron found", dir.display()))?;
    let text = std::fs::read_to_string(dir.join(&ron_name)).map_err(|e| format!("{}: {e}", dir.display()))?;
    let project: ProjectConfig = ron_from_str(&text).map_err(|e| format!("{ron_name}: {e}"))?;
    let backend_path = project.backend_path.as_deref().ok_or_else(|| format!("{}: the project has no backend_path", dir.display()))?;
    let btext = std::fs::read_to_string(dir.join(backend_path)).map_err(|e| format!("{backend_path}: {e}"))?;
    let config: BackendConfig = ron_from_str(&btext).map_err(|e| format!("{backend_path}: {e}"))?;
    let problems = config.problems();
    if !problems.is_empty() {
        return Err(format!("{backend_path} is invalid: {}", problems.join("; ")));
    }
    let game_id = config
        .effective_game_id(project.project_id.as_deref())
        .ok_or_else(|| format!("{}: neither backend.ron game_id nor project_id is set", dir.display()))?;
    Ok(Loaded { game_id, config })
}

fn config(dirs: &[PathBuf], out: Option<&Path>, mode: &OutputMode) -> Result<(), String> {
    let loaded: Vec<Loaded> = dirs.iter().map(|d| load(d)).collect::<Result<_, _>>()?;
    let games: Vec<GameServerConfig> =
        loaded.iter().map(|l| GameServerConfig { game_id: &l.game_id, leaderboards: &l.config.leaderboards }).collect();
    let lua = generate_games_lua(&games)?;
    match out {
        None => {
            if mode.json {
                println!("{}", serde_json::json!({ "file": GAMES_FILE, "content": lua }));
            } else {
                print!("{lua}");
            }
        }
        Some(folder) => {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
            std::fs::write(folder.join(GAMES_FILE), &lua).map_err(|e| format!("{GAMES_FILE}: {e}"))?;
            std::fs::write(folder.join("ironhold.lua"), MODULE_LUA).map_err(|e| format!("ironhold.lua: {e}"))?;
            let ids: Vec<&str> = loaded.iter().map(|l| l.game_id.as_str()).collect();
            if mode.json {
                println!("{}", serde_json::json!({ "written": [GAMES_FILE, "ironhold.lua"], "games": ids }));
            } else {
                println!("Wrote {GAMES_FILE} and ironhold.lua to {} for: {}", folder.display(), ids.join(", "));
                println!("Copy both into Nakama's modules directory and restart Nakama. Leaderboard changes need this redeploy.");
            }
        }
    }
    Ok(())
}
