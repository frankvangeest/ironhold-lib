use std::path::Path;

use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog};
use ironhold_core::schema::project::LogicRulesAsset;
use ironhold_core::schema::StateMachineAsset;

use super::utils::{glob_dir, silent_parse};
use crate::output::OutputMode;

// ── Data ──────────────────────────────────────────────────────────────────────

struct ProjectStats {
    project_name: String,
    scene_count: usize,
    prefab_count: usize,
    effect_count: usize,
    rule_count: usize,
    state_count: usize,
    behavior_count: usize,
    catalog_model_count: usize,
    catalog_texture_count: usize,
    catalog_audio_count: usize,
    catalog_decal_count: usize,
    ron_file_count: usize,
    total_bytes: u64,
}

impl ProjectStats {
    fn total_catalog_count(&self) -> usize {
        self.catalog_model_count
            + self.catalog_texture_count
            + self.catalog_audio_count
            + self.effect_count
            + self.catalog_decal_count
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn dir_stats(project_dir: &Path) -> (usize, u64) {
    let mut ron_count = 0usize;
    let mut total_bytes = 0u64;
    walk_dir(project_dir, &mut ron_count, &mut total_bytes);
    (ron_count, total_bytes)
}

fn walk_dir(dir: &Path, ron_count: &mut usize, total_bytes: &mut u64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, ron_count, total_bytes);
        } else if path.is_file() {
            if path.extension().and_then(|e| e.to_str()) == Some("ron") {
                *ron_count += 1;
            }
            if let Ok(meta) = std::fs::metadata(&path) {
                *total_bytes += meta.len();
            }
        }
    }
}

fn fmt_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ── Collection ────────────────────────────────────────────────────────────────

fn collect(project_dir: &Path) -> ProjectStats {
    let project_name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let scene_count = glob_dir(project_dir, "scenes", ".scene.ron").len();

    let prefab_count = silent_parse::<PrefabCatalog>(project_dir, "prefabs/prefabs.ron")
        .map(|c| c.prefabs.len())
        .unwrap_or(0);

    let asset_catalog: Option<AssetCatalog> = silent_parse(project_dir, "assets.ron");
    let effect_count = asset_catalog.as_ref().map(|c| c.effects.len()).unwrap_or(0);
    let catalog_model_count = asset_catalog.as_ref().map(|c| c.models.len()).unwrap_or(0);
    let catalog_texture_count = asset_catalog.as_ref().map(|c| c.textures.len()).unwrap_or(0);
    let catalog_audio_count = asset_catalog.as_ref().map(|c| c.audio.len()).unwrap_or(0);
    let catalog_decal_count = asset_catalog.as_ref().map(|c| c.decals.len()).unwrap_or(0);

    let rule_count = silent_parse::<LogicRulesAsset>(project_dir, "logic/rules.ron")
        .map(|r| r.rules.len())
        .unwrap_or(0);

    let state_count = silent_parse::<StateMachineAsset>(project_dir, "logic/state_machine.ron")
        .map(|s| s.states.len())
        .unwrap_or(0);

    let behavior_count = glob_dir(project_dir, "behaviors", ".behavior.ron").len();

    let (ron_file_count, total_bytes) = dir_stats(project_dir);

    ProjectStats {
        project_name,
        scene_count,
        prefab_count,
        effect_count,
        rule_count,
        state_count,
        behavior_count,
        catalog_model_count,
        catalog_texture_count,
        catalog_audio_count,
        catalog_decal_count,
        ron_file_count,
        total_bytes,
    }
}

// ── Output ────────────────────────────────────────────────────────────────────

fn print_human(s: &ProjectStats) {
    println!("{}", s.project_name);
    println!("  Scenes:    {}", s.scene_count);
    println!("  Prefabs:   {}", s.prefab_count);
    println!("  Effects:   {}", s.effect_count);

    let logic_parts: Vec<String> = [
        (s.rule_count, "rule"),
        (s.state_count, "state"),
        (s.behavior_count, "behavior"),
    ]
    .iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, label)| format!("{n} {}{}", label, if *n == 1 { "" } else { "s" }))
    .collect();
    let logic_str = if logic_parts.is_empty() { "none".to_string() } else { logic_parts.join("  ") };
    println!("  Logic:     {logic_str}");

    println!(
        "  Catalog:   {} entries  (models:{}  textures:{}  audio:{}  effects:{}  decals:{})",
        s.total_catalog_count(),
        s.catalog_model_count,
        s.catalog_texture_count,
        s.catalog_audio_count,
        s.effect_count,
        s.catalog_decal_count,
    );

    println!(
        "  Project:   {} RON files, {} on disk",
        s.ron_file_count,
        fmt_bytes(s.total_bytes),
    );
}

fn print_json(s: &ProjectStats) {
    let val = serde_json::json!({
        "project": s.project_name,
        "scenes": s.scene_count,
        "prefabs": s.prefab_count,
        "effects": s.effect_count,
        "logic": {
            "rules": s.rule_count,
            "states": s.state_count,
            "behaviors": s.behavior_count,
        },
        "catalog": {
            "total": s.total_catalog_count(),
            "models": s.catalog_model_count,
            "textures": s.catalog_texture_count,
            "audio": s.catalog_audio_count,
            "effects": s.effect_count,
            "decals": s.catalog_decal_count,
        },
        "project_files": {
            "ron_count": s.ron_file_count,
            "total_bytes": s.total_bytes,
            "total_size": fmt_bytes(s.total_bytes),
        },
    });
    println!("{}", serde_json::to_string_pretty(&val).unwrap());
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(project_dir: &Path, mode: &OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    if !project_dir.is_dir() {
        return Err(format!("{}: not a directory", project_dir.display()).into());
    }

    let stats = collect(project_dir);

    if mode.json {
        print_json(&stats);
    } else {
        print_human(&stats);
    }

    Ok(())
}
