use std::path::{Path, PathBuf};

use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog};
use ironhold_core::schema::project::LogicRulesAsset;
use ironhold_core::schema::scene_v2::GameSceneV2;
use ironhold_core::schema::stats::StatCatalog;
use ironhold_core::schema::{Action, ModelFixesAsset, ProjectConfig, StateMachineAsset};

use crate::output::OutputMode;

// ── Data structures ───────────────────────────────────────────────────────────

struct FileResult {
    rel_path: String,
    errors: Vec<String>,
}

impl FileResult {
    fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

struct CrossFileError {
    source_file: String,
    message: String,
    error_type: &'static str,
}

// ── RON parsing ───────────────────────────────────────────────────────────────

fn ron_from_str<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, ron::error::SpannedError> {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(s)
}

fn parse_file<T: serde::de::DeserializeOwned>(
    full_path: &Path,
    rel_path: &str,
    results: &mut Vec<FileResult>,
) -> Option<T> {
    let content = match std::fs::read_to_string(full_path) {
        Ok(c) => c,
        Err(e) => {
            results.push(FileResult {
                rel_path: rel_path.to_string(),
                errors: vec![format!("IO error: {e}")],
            });
            return None;
        }
    };
    match ron_from_str::<T>(&content) {
        Ok(val) => {
            results.push(FileResult { rel_path: rel_path.to_string(), errors: Vec::new() });
            Some(val)
        }
        Err(e) => {
            results.push(FileResult {
                rel_path: rel_path.to_string(),
                errors: vec![format!("line {}, col {}: {}", e.span.start.line, e.span.start.col, e.code)],
            });
            None
        }
    }
}

// Parse a project-relative path only if the file exists on disk.
fn try_parse<T: serde::de::DeserializeOwned>(
    project_dir: &Path,
    rel_path: &str,
    results: &mut Vec<FileResult>,
) -> Option<T> {
    let full = project_dir.join(rel_path);
    if !full.exists() {
        return None;
    }
    parse_file(&full, rel_path, results)
}

// ── File discovery ────────────────────────────────────────────────────────────

fn find_project_ron(project_dir: &Path) -> Option<String> {
    std::fs::read_dir(project_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|name| name.ends_with(".project.ron"))
}

fn glob_dir(project_dir: &Path, subdir: &str, suffix: &str) -> Vec<PathBuf> {
    let dir = project_dir.join(subdir);
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.to_str().map(|s| s.ends_with(suffix)).unwrap_or(false))
        .collect();
    paths.sort();
    paths
}

fn rel(project_dir: &Path, full: &Path) -> String {
    full.strip_prefix(project_dir)
        .unwrap_or(full)
        .to_string_lossy()
        .replace('\\', "/")
}

// ── Action collection ─────────────────────────────────────────────────────────

fn collect_actions(
    rules: Option<(&str, &LogicRulesAsset)>,
    state_machine: Option<(&str, &StateMachineAsset)>,
    behaviors: &[(String, StateMachineAsset)],
) -> Vec<(String, Action)> {
    let mut out = Vec::new();

    if let Some((src, r)) = rules {
        for rule in &r.rules {
            for action in &rule.do_actions {
                out.push((src.to_string(), action.clone()));
            }
        }
    }
    if let Some((src, fsm)) = state_machine {
        for action in fsm_actions(fsm) {
            out.push((src.to_string(), action));
        }
    }
    for (path, behavior) in behaviors {
        for action in fsm_actions(behavior) {
            out.push((path.clone(), action));
        }
    }
    out
}

fn fsm_actions(fsm: &StateMachineAsset) -> Vec<Action> {
    let mut out = Vec::new();
    for state in &fsm.states {
        out.extend(state.entry_actions.iter().cloned());
        out.extend(state.exit_actions.iter().cloned());
        for binding in &state.on {
            out.extend(binding.do_actions.iter().cloned());
        }
    }
    for binding in &fsm.global_on {
        out.extend(binding.do_actions.iter().cloned());
    }
    out
}

// ── Cross-file checks ─────────────────────────────────────────────────────────

fn cross_file_checks(
    project_dir: &Path,
    asset_catalog: Option<&AssetCatalog>,
    prefab_catalog: Option<&PrefabCatalog>,
    stat_catalog: Option<&StatCatalog>,
    scenes: &[(String, GameSceneV2)],
    actions: &[(String, Action)],
) -> Vec<CrossFileError> {
    let mut errors = Vec::new();

    for (source, action) in actions {
        match action {
            Action::SpawnEffect { key, .. } => {
                if let Some(c) = asset_catalog {
                    if !c.effects.contains_key(key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("effect key {:?} not found in assets.ron", key),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::ProjectDecal { key, .. } => {
                if let Some(c) = asset_catalog {
                    if !c.decals.contains_key(key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("decal key {:?} not found in assets.ron", key),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::PlaySound { key, .. } | Action::PlayMusicLoop { key, .. } => {
                if let Some(c) = asset_catalog {
                    if !c.audio.contains_key(key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("audio key {:?} not found in assets.ron", key),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::Spawn { prefab, .. } => {
                if let Some(c) = prefab_catalog {
                    if !c.prefabs.contains_key(prefab) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("prefab key {:?} not found in prefabs.ron", prefab),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::PreloadPrefab(key) => {
                if let Some(c) = prefab_catalog {
                    if !c.prefabs.contains_key(key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("prefab key {:?} not found in prefabs.ron", key),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::ApplyModifier { modifier_key } | Action::RemoveModifier { modifier_key } => {
                if let Some(c) = stat_catalog {
                    if !c.modifiers.contains_key(modifier_key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!(
                                "modifier key {:?} not found in stats.ron",
                                modifier_key
                            ),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // Prefab keys in scene entity defs
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            for entity in &scene.entities {
                if !catalog.prefabs.contains_key(&entity.prefab) {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "entity {:?}: prefab {:?} not found in prefabs.ron",
                            entity.id, entity.prefab
                        ),
                        error_type: "missing_reference",
                    });
                }
            }
        }
    }

    // Behavior file paths on PrefabDef
    if let Some(catalog) = prefab_catalog {
        for (key, def) in &catalog.prefabs {
            if let Some(behavior_path) = &def.behavior {
                if !project_dir.join(behavior_path).exists() {
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: behavior {:?} not found on disk",
                            key, behavior_path
                        ),
                        error_type: "missing_file",
                    });
                }
            }
        }
    }

    errors
}

// ── Output ────────────────────────────────────────────────────────────────────

fn print_human(
    project_dir: &Path,
    file_results: &[FileResult],
    cross_errors: &[CrossFileError],
    all_valid: bool,
) {
    println!("Validating: {}", project_dir.display());
    println!();

    const CROSS_LABEL: &str = "Cross-file checks";
    let col_width = file_results
        .iter()
        .map(|r| r.rel_path.len())
        .chain(std::iter::once(CROSS_LABEL.len()))
        .max()
        .unwrap_or(24)
        + 4;

    for result in file_results {
        let status = if result.is_ok() { "OK" } else { "ERROR" };
        println!("  {:<width$} {}", result.rel_path, status, width = col_width);
        if !result.is_ok() {
            for err in &result.errors {
                println!("    {err}");
            }
            println!();
        }
    }

    println!();

    let cross_status = match cross_errors.len() {
        0 => "OK".to_string(),
        1 => "1 error".to_string(),
        n => format!("{n} errors"),
    };
    println!("  {:<width$} {}", CROSS_LABEL, cross_status, width = col_width);
    for err in cross_errors {
        println!("    {}: {}", err.source_file, err.message);
    }

    println!();

    let file_error_count = file_results.iter().filter(|r| !r.is_ok()).count();
    let total = file_results.len();

    if all_valid {
        println!("{total} files checked — all valid.");
    } else {
        let mut parts = Vec::new();
        if file_error_count > 0 {
            parts.push(format!(
                "{} file error{}",
                file_error_count,
                if file_error_count == 1 { "" } else { "s" }
            ));
        }
        if !cross_errors.is_empty() {
            parts.push(format!(
                "{} cross-file error{}",
                cross_errors.len(),
                if cross_errors.len() == 1 { "" } else { "s" }
            ));
        }
        println!("{total} files checked — {}.", parts.join(", "));
    }
}

fn print_json(
    project_name: &str,
    file_results: &[FileResult],
    cross_errors: &[CrossFileError],
    all_valid: bool,
) {
    let val = serde_json::json!({
        "valid": all_valid,
        "project": project_name,
        "files": file_results.iter().map(|r| serde_json::json!({
            "path": r.rel_path,
            "valid": r.is_ok(),
            "errors": r.errors.iter().map(|e| serde_json::json!({
                "type": "parse_error",
                "message": e,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "cross_file_errors": cross_errors.iter().map(|e| serde_json::json!({
            "type": e.error_type,
            "source": e.source_file,
            "message": e.message,
        })).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&val).unwrap());
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(project_dir: &Path, mode: &OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    if !project_dir.is_dir() {
        return Err(format!("{}: not a directory", project_dir.display()).into());
    }

    let project_name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut file_results: Vec<FileResult> = Vec::new();

    // ── Parse per-file ────────────────────────────────────────────────────────

    let _project_config: Option<ProjectConfig> = find_project_ron(project_dir)
        .and_then(|name| try_parse(project_dir, &name, &mut file_results));

    let asset_catalog: Option<AssetCatalog> =
        try_parse(project_dir, "assets.ron", &mut file_results);

    let prefab_catalog: Option<PrefabCatalog> =
        try_parse(project_dir, "prefabs/prefabs.ron", &mut file_results);

    let stat_catalog: Option<StatCatalog> =
        try_parse(project_dir, "stats/stats.ron", &mut file_results);

    let mut scenes: Vec<(String, GameSceneV2)> = Vec::new();
    for path in glob_dir(project_dir, "scenes", ".scene.ron") {
        let r = rel(project_dir, &path);
        if let Some(scene) = parse_file::<GameSceneV2>(&path, &r, &mut file_results) {
            scenes.push((r, scene));
        }
    }

    let rules: Option<LogicRulesAsset> =
        try_parse(project_dir, "logic/rules.ron", &mut file_results);

    let state_machine: Option<StateMachineAsset> =
        try_parse(project_dir, "logic/state_machine.ron", &mut file_results);

    let mut behaviors: Vec<(String, StateMachineAsset)> = Vec::new();
    for path in glob_dir(project_dir, "behaviors", ".behavior.ron") {
        let r = rel(project_dir, &path);
        if let Some(b) = parse_file::<StateMachineAsset>(&path, &r, &mut file_results) {
            behaviors.push((r, b));
        }
    }

    let _model_fixes: Option<ModelFixesAsset> =
        try_parse(project_dir, "overrides/model_fixes.ron", &mut file_results);

    // ── Cross-file checks ─────────────────────────────────────────────────────

    let all_actions = collect_actions(
        rules.as_ref().map(|r| ("logic/rules.ron", r)),
        state_machine.as_ref().map(|s| ("logic/state_machine.ron", s)),
        &behaviors,
    );

    let cross_errors = cross_file_checks(
        project_dir,
        asset_catalog.as_ref(),
        prefab_catalog.as_ref(),
        stat_catalog.as_ref(),
        &scenes,
        &all_actions,
    );

    // ── Output ────────────────────────────────────────────────────────────────

    let all_valid = file_results.iter().all(|r| r.is_ok()) && cross_errors.is_empty();

    if mode.json {
        print_json(&project_name, &file_results, &cross_errors, all_valid);
    } else {
        print_human(project_dir, &file_results, &cross_errors, all_valid);
    }

    if !all_valid {
        std::process::exit(1);
    }

    Ok(())
}
