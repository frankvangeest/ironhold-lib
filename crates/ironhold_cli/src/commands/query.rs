use std::collections::HashSet;
use std::path::{Path, PathBuf};

use clap::Subcommand;

use ironhold_core::schema::catalog::{AssetCatalog, EffectDef, EffectPriority, PrefabCatalog, PrefabDef};
use ironhold_core::schema::project::{LogicRulesAsset, StateMachineAsset};
use ironhold_core::schema::scene_v2::GameSceneV2;

use super::utils::{glob_dir, rel, ron_from_str, silent_parse};
use crate::output::OutputMode;

// ── CLI surface ───────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum QueryCommand {
    #[command(about = "List prefab definitions in a project")]
    Prefabs {
        /// Path to the project directory (e.g. assets/projects/particles_demo)
        project_dir: PathBuf,
        #[arg(long, help = "Print only prefab keys, one per line")]
        keys_only: bool,
        #[arg(long, value_name = "key=value",
              help = "Filter: kind=actor, kind=prop, kind=primitive, tag=player, behavior=true")]
        filter: Option<String>,
    },
    #[command(about = "List particle effect definitions in a project")]
    Effects {
        project_dir: PathBuf,
        #[arg(long, help = "Print only effect keys, one per line")]
        keys_only: bool,
        #[arg(long, value_name = "key=value",
              help = "Filter: additive=true, priority=Player, priority=Ambient, layers=true")]
        filter: Option<String>,
    },
    #[command(about = "List scene files in a project")]
    Scenes {
        project_dir: PathBuf,
    },
    #[command(about = "List logic rules and state machines in a project")]
    Rules {
        project_dir: PathBuf,
    },
}

pub fn run(cmd: QueryCommand, mode: &OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        QueryCommand::Prefabs { project_dir, keys_only, filter } => {
            query_prefabs(&project_dir, keys_only, filter.as_deref(), mode)
        }
        QueryCommand::Effects { project_dir, keys_only, filter } => {
            query_effects(&project_dir, keys_only, filter.as_deref(), mode)
        }
        QueryCommand::Scenes { project_dir } => query_scenes(&project_dir, mode),
        QueryCommand::Rules { project_dir } => query_rules(&project_dir, mode),
    }
}

// ── Filter helper ─────────────────────────────────────────────────────────────

fn parse_filter(
    filter: Option<&str>,
) -> Result<(Option<String>, Option<String>), Box<dyn std::error::Error>> {
    match filter {
        None => Ok((None, None)),
        Some(f) => {
            let parts: Vec<&str> = f.splitn(2, '=').collect();
            if parts.len() != 2 {
                return Err(format!("Invalid filter {:?}: expected key=value", f).into());
            }
            Ok((Some(parts[0].to_string()), Some(parts[1].to_string())))
        }
    }
}

// ── query prefabs ─────────────────────────────────────────────────────────────

fn query_prefabs(
    project_dir: &Path,
    keys_only: bool,
    filter: Option<&str>,
    mode: &OutputMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let catalog: PrefabCatalog =
        silent_parse(project_dir, "prefabs/prefabs.ron").ok_or_else(|| {
            format!("prefabs/prefabs.ron not found or could not be parsed in {}", project_dir.display())
        })?;

    let (filter_key, filter_val) = parse_filter(filter)?;

    let mut prefabs: Vec<(&String, &PrefabDef)> = catalog.prefabs.iter().collect();
    prefabs.sort_by_key(|(k, _)| k.as_str());

    if let (Some(k), Some(v)) = (&filter_key, &filter_val) {
        prefabs.retain(|(_, def)| prefab_matches(def, k, v));
    }

    if mode.json {
        let items: Vec<_> = prefabs
            .iter()
            .map(|(key, def)| {
                if keys_only {
                    serde_json::json!(key)
                } else {
                    serde_json::json!({
                        "key": key,
                        "kind": def.kind,
                        "model": def.model,
                        "tags": def.components.tags,
                        "behavior": def.behavior,
                        "has_npc": def.components.npc.is_some(),
                        "has_trigger_zone": def.trigger_zone.is_some(),
                        "has_interactable": def.interactable.is_some(),
                    })
                }
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!(items)).unwrap());
        return Ok(());
    }

    if keys_only {
        for (key, _) in &prefabs {
            println!("{key}");
        }
        return Ok(());
    }

    let suffix = filter
        .map(|f| format!(" — filtered by {f}"))
        .unwrap_or_default();
    println!("Prefabs: {} ({} prefabs{})", project_dir.display(), prefabs.len(), suffix);
    println!();

    let col_width = prefabs.iter().map(|(k, _)| k.len()).max().unwrap_or(8) + 4;

    for (key, def) in &prefabs {
        let mut parts = vec![format!("kind:{}", def.kind)];
        if !def.model.is_empty() {
            parts.push(format!("model:{}", def.model));
        }
        if !def.components.tags.is_empty() {
            parts.push(format!("tags:{}", def.components.tags.join(",")));
        }
        if def.components.npc.is_some() {
            parts.push("npc".to_string());
        }
        if def.trigger_zone.is_some() {
            parts.push("trigger_zone".to_string());
        }
        if def.interactable.is_some() {
            parts.push("interactable".to_string());
        }
        if let Some(b) = &def.behavior {
            parts.push(format!("behavior:{b}"));
        }
        println!("  {:<width$} {}", key, parts.join("  "), width = col_width);
    }

    Ok(())
}

fn prefab_matches(def: &PrefabDef, key: &str, val: &str) -> bool {
    match key {
        "kind" => def.kind == val,
        "model" => def.model.contains(val),
        "tag" | "tags" => def.components.tags.iter().any(|t| t == val),
        "behavior" => match val {
            "true" | "yes" => def.behavior.is_some(),
            "false" | "no" | "none" => def.behavior.is_none(),
            v => def.behavior.as_deref() == Some(v),
        },
        "npc" => match val {
            "true" | "yes" => def.components.npc.is_some(),
            _ => def.components.npc.is_none(),
        },
        _ => false,
    }
}

// ── query effects ─────────────────────────────────────────────────────────────

fn query_effects(
    project_dir: &Path,
    keys_only: bool,
    filter: Option<&str>,
    mode: &OutputMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let catalog: AssetCatalog =
        silent_parse(project_dir, "assets.ron").ok_or_else(|| {
            format!("assets.ron not found or could not be parsed in {}", project_dir.display())
        })?;

    let (filter_key, filter_val) = parse_filter(filter)?;

    let mut effects: Vec<(&String, &EffectDef)> = catalog.effects.iter().collect();
    effects.sort_by_key(|(k, _)| k.as_str());

    if let (Some(k), Some(v)) = (&filter_key, &filter_val) {
        effects.retain(|(_, def)| effect_matches(def, k, v));
    }

    if mode.json {
        let items: Vec<_> = effects
            .iter()
            .map(|(key, def)| {
                if keys_only {
                    serde_json::json!(key)
                } else {
                    serde_json::json!({
                        "key": key,
                        "particle_count": def.particle_count,
                        "lifetime_secs": def.lifetime_secs,
                        "layers": def.layers.len(),
                        "additive": def.additive,
                        "priority": format!("{:?}", def.priority),
                        "has_sprite": def.sprite.is_some() || !def.sprites.is_empty(),
                        "has_light": def.light.is_some(),
                    })
                }
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!(items)).unwrap());
        return Ok(());
    }

    if keys_only {
        for (key, _) in &effects {
            println!("{key}");
        }
        return Ok(());
    }

    let suffix = filter
        .map(|f| format!(" — filtered by {f}"))
        .unwrap_or_default();
    println!("Effects: {} ({} effects{})", project_dir.display(), effects.len(), suffix);
    println!();

    let col_width = effects.iter().map(|(k, _)| k.len()).max().unwrap_or(8) + 4;

    for (key, def) in &effects {
        let mut parts = Vec::new();
        if def.layers.is_empty() {
            parts.push(format!("count:{}", def.particle_count));
            parts.push(format!("lifetime:{:.1}s", def.lifetime_secs));
        } else {
            parts.push(format!("layers:{}", def.layers.len()));
        }
        if def.additive {
            parts.push("additive".to_string());
        }
        if def.sprite.is_some() || !def.sprites.is_empty() {
            parts.push("sprite".to_string());
        }
        if def.light.is_some() {
            parts.push("light".to_string());
        }
        match def.priority {
            EffectPriority::Player => parts.push("priority:Player".to_string()),
            EffectPriority::Ambient => parts.push("priority:Ambient".to_string()),
            EffectPriority::Npc => {}
        }
        println!("  {:<width$} {}", key, parts.join("  "), width = col_width);
    }

    Ok(())
}

fn effect_matches(def: &EffectDef, key: &str, val: &str) -> bool {
    match key {
        "additive" => match val {
            "true" | "yes" => def.additive,
            _ => !def.additive,
        },
        "priority" => format!("{:?}", def.priority) == val,
        "layers" => match val {
            "true" | "yes" | "multi" => !def.layers.is_empty(),
            _ => def.layers.is_empty(),
        },
        "sprite" => match val {
            "true" | "yes" => def.sprite.is_some() || !def.sprites.is_empty(),
            _ => def.sprite.is_none() && def.sprites.is_empty(),
        },
        _ => false,
    }
}

// ── query scenes ──────────────────────────────────────────────────────────────

fn query_scenes(project_dir: &Path, mode: &OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    let scene_paths = glob_dir(project_dir, "scenes", ".scene.ron");

    if scene_paths.is_empty() {
        if mode.json {
            println!("[]");
        } else {
            println!("No scenes found in {}", project_dir.display());
        }
        return Ok(());
    }

    let prefab_catalog: Option<PrefabCatalog> = silent_parse(project_dir, "prefabs/prefabs.ron");
    let player_prefab_keys: HashSet<String> = prefab_catalog
        .as_ref()
        .map(|c| {
            c.prefabs
                .iter()
                .filter(|(_, def)| def.components.tags.contains(&"player".to_string()))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default();

    struct SceneInfo {
        rel_path: String,
        name: String,
        entity_count: usize,
        ui_count: usize,
        has_player: bool,
        overlay: bool,
    }

    let mut scenes: Vec<SceneInfo> = Vec::new();
    for path in &scene_paths {
        let r = rel(project_dir, path);
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Warning: could not read {r}: {e}");
                continue;
            }
        };
        let scene: GameSceneV2 = match ron_from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Warning: could not parse {r}: {e}");
                continue;
            }
        };
        let has_player = scene.entities.iter().any(|e| player_prefab_keys.contains(&e.prefab));
        // Overlay: no world entities and no terrain — only UI
        let overlay = scene.entities.is_empty() && scene.terrain.is_none();
        scenes.push(SceneInfo {
            rel_path: r,
            name: scene.name.clone(),
            entity_count: scene.entities.len(),
            ui_count: scene.ui.len(),
            has_player,
            overlay,
        });
    }

    if mode.json {
        let arr: Vec<_> = scenes
            .iter()
            .map(|s| {
                serde_json::json!({
                    "path": s.rel_path,
                    "name": s.name,
                    "entities": s.entity_count,
                    "ui_elements": s.ui_count,
                    "has_player": s.has_player,
                    "overlay": s.overlay,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!(arr)).unwrap());
        return Ok(());
    }

    println!("Scenes: {} ({} scenes)", project_dir.display(), scenes.len());
    println!();

    let col_width = scenes.iter().map(|s| s.rel_path.len()).max().unwrap_or(20) + 4;

    for s in &scenes {
        let mut parts = Vec::new();
        if !s.name.is_empty() {
            parts.push(format!("name:{}", s.name));
        }
        if s.entity_count > 0 {
            parts.push(format!("entities:{}", s.entity_count));
        }
        if s.ui_count > 0 {
            parts.push(format!("ui:{}", s.ui_count));
        }
        if s.has_player {
            parts.push("player:true".to_string());
        }
        if s.overlay {
            parts.push("overlay".to_string());
        }
        println!("  {:<width$} {}", s.rel_path, parts.join("  "), width = col_width);
    }

    Ok(())
}

// ── query rules ───────────────────────────────────────────────────────────────

fn query_rules(project_dir: &Path, mode: &OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    let rules: Option<LogicRulesAsset> = silent_parse(project_dir, "logic/rules.ron");
    let fsm: Option<StateMachineAsset> = silent_parse(project_dir, "logic/state_machine.ron");

    if rules.is_none() && fsm.is_none() {
        if mode.json {
            println!("[]");
        } else {
            println!("No logic files found in {}", project_dir.display());
        }
        return Ok(());
    }

    if mode.json {
        let mut arr = Vec::new();
        if let Some(r) = &rules {
            let rules_json: Vec<_> = r
                .rules
                .iter()
                .map(|rule| {
                    serde_json::json!({
                        "on": rule.on,
                        "when": rule.when,
                        "actions": rule.do_actions.len(),
                    })
                })
                .collect();
            arr.push(serde_json::json!({
                "type": "rules",
                "path": "logic/rules.ron",
                "count": r.rules.len(),
                "rules": rules_json,
            }));
        }
        if let Some(s) = &fsm {
            let states_json: Vec<_> = s
                .states
                .iter()
                .map(|state| {
                    serde_json::json!({
                        "name": state.name,
                        "entry_actions": state.entry_actions.len(),
                        "exit_actions": state.exit_actions.len(),
                        "on_bindings": state.on.len(),
                    })
                })
                .collect();
            arr.push(serde_json::json!({
                "type": "state_machine",
                "path": "logic/state_machine.ron",
                "initial_state": s.initial_state,
                "states": states_json,
                "transitions": s.transitions.len(),
                "global_on": s.global_on.len(),
            }));
        }
        println!("{}", serde_json::to_string_pretty(&serde_json::json!(arr)).unwrap());
        return Ok(());
    }

    println!("Rules: {}", project_dir.display());
    println!();

    if let Some(r) = &rules {
        println!("  logic/rules.ron  ({} rules)", r.rules.len());
        for rule in &r.rules {
            let guard = rule
                .when
                .as_deref()
                .map(|w| format!("[when:{w}] "))
                .unwrap_or_default();
            println!(
                "    {guard}on:{:<40}  → {} action{}",
                rule.on,
                rule.do_actions.len(),
                if rule.do_actions.len() == 1 { "" } else { "s" }
            );
        }
        println!();
    }

    if let Some(s) = &fsm {
        let global_note = if s.global_on.is_empty() {
            String::new()
        } else {
            format!(", {} global binding{}", s.global_on.len(), if s.global_on.len() == 1 { "" } else { "s" })
        };
        println!(
            "  logic/state_machine.ron  initial:{}  ({} states, {} transitions{})",
            s.initial_state,
            s.states.len(),
            s.transitions.len(),
            global_note
        );
        for state in &s.states {
            let to_states: Vec<&str> = s
                .transitions
                .iter()
                .filter(|t| t.from.as_deref() == Some(state.name.as_str()))
                .map(|t| t.to.as_str())
                .collect();
            let mut info = format!(
                "entry:{}  exit:{}  on:{}",
                state.entry_actions.len(),
                state.exit_actions.len(),
                state.on.len()
            );
            if !to_states.is_empty() {
                info.push_str(&format!("  → {}", to_states.join(", ")));
            }
            println!("    {}  {}", state.name, info);
        }
        println!();
    }

    Ok(())
}
