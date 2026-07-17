use std::collections::HashSet;
use std::path::Path;

use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabKind};
use ironhold_core::schema::project::LogicRulesAsset;
use ironhold_core::schema::scene_v2::GameSceneV2;
use ironhold_core::schema::stats::StatCatalog;
use ironhold_core::schema::{Action, ModelFixesAsset, ProjectConfig, StateMachineAsset};

use super::utils::{glob_dir, rel, ron_from_str};
use crate::output::OutputMode;

// ── Internal data structures ──────────────────────────────────────────────────

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

struct StrictWarning {
    source_file: String,
    message: String,
    kind: &'static str,
}

struct ValidationRun {
    project_name: String,
    file_results: Vec<FileResult>,
    cross_errors: Vec<CrossFileError>,
    strict_warnings: Vec<StrictWarning>,
    all_valid: bool,
}

// ── Public result type (used by watch) ───────────────────────────────────────

pub struct ValidateResult {
    pub all_valid: bool,
    pub file_count: usize,
    /// Flat list of error strings in "path: message" format.
    pub errors: Vec<String>,
}

// ── RON parsing ───────────────────────────────────────────────────────────────

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
            Action::PreloadGlb(key) => {
                if let Some(c) = asset_catalog {
                    if !c.models.contains_key(key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("model key {:?} not found in assets.ron", key),
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

    for (scene_path, scene) in scenes {
        // Scene-wide (not per-bar) so a slot key shared across two different `ActionBar`s is
        // also caught here, not just within one bar's own slots — per-player action bars
        // (`owner_player`, see `planning/features/per_player_split_screen_targeting.md` Phase 2)
        // are the first feature to author 2+ `ActionBar`s in one scene, and a cross-bar collision
        // is worse than "the wrong slot fires": `CooldownMap`/`PendingIntentActions`/
        // `HandledIntentSlots` are keyed by the literal slot_key string alone, scene-wide, so a
        // `rules.ron` rule handling one bar's intent on a colliding key silently suppresses the
        // other bar's pending slot too.
        // `_` lets the compiler infer bevy's `KeyCode` from `InputMap::parse_key`'s return
        // type without this file needing its own `use`/import to name it (this crate already
        // links bevy transitively via ironhold_core — this only avoids one import line).
        //
        // Keyed by positional ui-node index, not `bar.id` — `id` is documented "Unique
        // identifier" but nothing actually enforces that, and comparing by `id` would
        // misclassify (or silently miss) a real cross-bar collision if two bars happened to
        // share an id (system-architect finding, per_player_split_screen_targeting.md Phase 2).
        let mut seen: std::collections::HashMap<_, (usize, &str, &str)> = std::collections::HashMap::new();
        for (node_index, node) in scene.ui.iter().enumerate() {
            let ironhold_core::schema::scene_v2::UiNodeDef::ActionBar(bar) = node else { continue };
            for slot in &bar.slots {
                match ironhold_core::schema::player::InputMap::parse_key(&slot.key) {
                    None => errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "ActionBar {:?}: slot {:?} has an unrecognised key {:?} — it will never fire",
                            bar.id, slot.key, slot.key
                        ),
                        error_type: "invalid_key",
                    }),
                    Some(kc) => {
                        if let Some((prev_node_index, prev_bar, prev_key)) = seen.insert(kc, (node_index, &bar.id, &slot.key)) {
                            if prev_node_index == node_index {
                                errors.push(CrossFileError {
                                    source_file: scene_path.clone(),
                                    message: format!(
                                        "ActionBar {:?}: slots {:?} and {:?} both resolve to {:?} — only {:?} will fire on press",
                                        bar.id, prev_key, slot.key, kc, prev_key
                                    ),
                                    error_type: "duplicate_key",
                                });
                            } else {
                                errors.push(CrossFileError {
                                    source_file: scene_path.clone(),
                                    message: format!(
                                        "ActionBar {:?} slot {:?} and ActionBar {:?} slot {:?} both resolve to {:?} — \
                                         the intent/cooldown pipeline is keyed by slot_key alone, scene-wide, so a \
                                         rules.ron rule handling one bar's intent on this key will also silently \
                                         suppress the other bar's pending slot",
                                        prev_bar, prev_key, bar.id, slot.key, kc
                                    ),
                                    error_type: "cross_bar_duplicate_key",
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Per-player action-bar cost slots whose stat isn't declared on the owning player's own
    // `stat_templates` — the player clearly opted into a per-player pool (declares
    // `stat_templates` at all), so a slot costing an undeclared key would silently fall back to
    // the shared global `LoadedStats` pool for just that one stat. Deliberately does not error
    // when the owning player declares no `stat_templates` at all — that's the ordinary, unchanged
    // shared-pool fallback. `owner_player.unwrap_or(0)`, not an early-continue on `None` — mirrors
    // `owns_slot`'s runtime "None/Some(0) both mean the primary player" resolution, so a default
    // (owner_player omitted) bar gets the same coverage as an explicit `owner_player: 0` one
    // (debug-detective finding). See `planning/features/per_player_stat_pools.md`.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            for node in &scene.ui {
                let ironhold_core::schema::scene_v2::UiNodeDef::ActionBar(bar) = node else { continue };
                let owner_player = bar.owner_player.unwrap_or(0);
                let player_prefab = scene.entities.iter()
                    .filter_map(|e| catalog.prefabs.get(&e.prefab))
                    .find(|p| p.player_index == owner_player && p.components.tags.iter().any(|t| t == "player"));
                let Some(prefab) = player_prefab else { continue };
                if prefab.stat_templates.is_empty() { continue; }
                for slot in &bar.slots {
                    let Some(cost) = &slot.cost else { continue };
                    if !prefab.stat_templates.iter().any(|t| t.key == cost.stat) {
                        errors.push(CrossFileError {
                            source_file: scene_path.clone(),
                            message: format!(
                                "ActionBar {:?} slot {:?} costs stat {:?}, but player_index {}'s \
                                 prefab declares stat_templates without that key — this slot's \
                                 cost will silently fall back to the shared global LoadedStats \
                                 pool instead of this player's own pool",
                                bar.id, slot.key, cost.stat, owner_player
                            ),
                            error_type: "missing_player_stat_template",
                        });
                    }
                }
            }
        }
    }

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

            if def.kind == PrefabKind::Foliage {
                if let Some(foliage) = &def.foliage {
                    if let Some(ac) = asset_catalog {
                        let tex_key = &foliage.material.leaf_texture;
                        if !tex_key.is_empty() && !ac.textures.contains_key(tex_key) {
                            errors.push(CrossFileError {
                                source_file: "prefabs/prefabs.ron".to_string(),
                                message: format!(
                                    "prefab {:?}: foliage leaf_texture key {:?} not found in assets.ron textures",
                                    key, tex_key
                                ),
                                error_type: "missing_catalog_key",
                            });
                        }
                    }
                }
            }

            // stat_label/world_stat_bar authored with an entity-local ("{self}.<stat>") key
            // require a matching entry in this SAME prefab's stat_templates, or the widget
            // silently renders empty forever with no runtime feedback. Generic across every
            // prefab kind (players included, since a player prefab is just a prefab with
            // `tags: ["player"]`) — NPCs/props have had this exact silent-failure mode all
            // along; `player_stat_widgets` just makes it far more likely a designer hits it on
            // a player prefab for the first time (carrying over a `{self}.mana` habit onto a
            // player prefab with no matching `stat_templates` entry).
            // See `planning/features/player_stat_widgets.md` Part C.
            for (widget_kind, stat_key) in [
                ("stat_label", def.stat_label.as_ref().map(|sl| &sl.stat_key)),
                ("world_stat_bar", def.world_stat_bar.as_ref().map(|wb| &wb.stat_key)),
            ] {
                let Some(stat_key) = stat_key else { continue };
                let Some(local_stat) = stat_key.strip_prefix("{self}.") else { continue };
                if !def.stat_templates.iter().any(|t| t.key == local_stat) {
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: {} keyed {:?}, but this prefab's stat_templates has no \
                             entry for {:?} — the widget will render empty with no further warning",
                            key, widget_kind, stat_key, local_stat
                        ),
                        error_type: "missing_stat_widget_template",
                    });
                }
            }
        }
    }

    errors
}

// ── Strict (orphan) checks ────────────────────────────────────────────────────

fn strict_checks(
    asset_catalog: Option<&AssetCatalog>,
    prefab_catalog: Option<&PrefabCatalog>,
    scenes: &[(String, GameSceneV2)],
    actions: &[(String, Action)],
) -> Vec<StrictWarning> {
    let mut warnings: Vec<StrictWarning> = Vec::new();

    // Collect every key that appears on the "usage" side.
    let mut used_prefabs: HashSet<&str> = HashSet::new();
    let mut used_effects: HashSet<&str> = HashSet::new();
    let mut used_audio: HashSet<&str> = HashSet::new();
    let mut used_decals: HashSet<&str> = HashSet::new();

    for (_, scene) in scenes {
        for entity in &scene.entities {
            used_prefabs.insert(&entity.prefab);
        }
    }
    for (_, action) in actions {
        match action {
            Action::Spawn { prefab, .. } => { used_prefabs.insert(prefab); }
            Action::PreloadPrefab(key) => { used_prefabs.insert(key); }
            Action::SpawnEffect { key, .. } => { used_effects.insert(key); }
            Action::PlaySound { key, .. } | Action::PlayMusicLoop { key, .. } => {
                used_audio.insert(key);
            }
            Action::ProjectDecal { key, .. } => { used_decals.insert(key); }
            _ => {}
        }
    }

    // Report defined-but-never-used keys.
    if let Some(catalog) = prefab_catalog {
        let mut keys: Vec<&String> = catalog.prefabs.keys().collect();
        keys.sort();
        for key in keys {
            if !used_prefabs.contains(key.as_str()) {
                warnings.push(StrictWarning {
                    source_file: "prefabs/prefabs.ron".to_string(),
                    message: format!(
                        "prefab {:?} is defined but never referenced in any scene or action",
                        key
                    ),
                    kind: "unused_prefab",
                });
            }
        }
    }
    if let Some(catalog) = asset_catalog {
        let mut effect_keys: Vec<&String> = catalog.effects.keys().collect();
        effect_keys.sort();
        for key in effect_keys {
            if !used_effects.contains(key.as_str()) {
                warnings.push(StrictWarning {
                    source_file: "assets.ron".to_string(),
                    message: format!(
                        "effect {:?} is defined but never used in any SpawnEffect action",
                        key
                    ),
                    kind: "unused_effect",
                });
            }
        }
        let mut audio_keys: Vec<&String> = catalog.audio.keys().collect();
        audio_keys.sort();
        for key in audio_keys {
            if !used_audio.contains(key.as_str()) {
                warnings.push(StrictWarning {
                    source_file: "assets.ron".to_string(),
                    message: format!(
                        "audio {:?} is defined but never used in any PlaySound or PlayMusicLoop action",
                        key
                    ),
                    kind: "unused_audio",
                });
            }
        }
        let mut decal_keys: Vec<&String> = catalog.decals.keys().collect();
        decal_keys.sort();
        for key in decal_keys {
            if !used_decals.contains(key.as_str()) {
                warnings.push(StrictWarning {
                    source_file: "assets.ron".to_string(),
                    message: format!(
                        "decal {:?} is defined but never used in any ProjectDecal action",
                        key
                    ),
                    kind: "unused_decal",
                });
            }
        }
    }

    warnings
}

// ── Core validation (shared by `run` and `validate_project`) ─────────────────

fn do_validate(project_dir: &Path, strict: bool) -> ValidationRun {
    let project_name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut file_results: Vec<FileResult> = Vec::new();

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

    let strict_warnings = if strict {
        strict_checks(asset_catalog.as_ref(), prefab_catalog.as_ref(), &scenes, &all_actions)
    } else {
        Vec::new()
    };

    let all_valid = file_results.iter().all(|r| r.is_ok())
        && cross_errors.is_empty()
        && strict_warnings.is_empty();

    ValidationRun { project_name, file_results, cross_errors, strict_warnings, all_valid }
}

// ── Public: used by `watch` ───────────────────────────────────────────────────

pub fn validate_project(project_dir: &Path) -> ValidateResult {
    let vr = do_validate(project_dir, false);

    let mut errors = Vec::new();
    for fr in &vr.file_results {
        for e in &fr.errors {
            errors.push(format!("{}: {}", fr.rel_path, e));
        }
    }
    for ce in &vr.cross_errors {
        errors.push(format!("{}: {}", ce.source_file, ce.message));
    }

    ValidateResult {
        all_valid: vr.all_valid,
        file_count: vr.file_results.len(),
        errors,
    }
}

// ── Output ────────────────────────────────────────────────────────────────────

fn print_human(
    project_dir: &Path,
    file_results: &[FileResult],
    cross_errors: &[CrossFileError],
    strict_warnings: &[StrictWarning],
    all_valid: bool,
) {
    println!("Validating: {}", project_dir.display());
    println!();

    const CROSS_LABEL: &str = "Cross-file checks";
    const STRICT_LABEL: &str = "Strict checks";
    let col_width = file_results
        .iter()
        .map(|r| r.rel_path.len())
        .chain(std::iter::once(CROSS_LABEL.len()))
        .chain(std::iter::once(STRICT_LABEL.len()))
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

    if !strict_warnings.is_empty() {
        println!();
        let strict_status = match strict_warnings.len() {
            1 => "1 warning".to_string(),
            n => format!("{n} warnings"),
        };
        println!("  {:<width$} {}", STRICT_LABEL, strict_status, width = col_width);
        for w in strict_warnings {
            println!("    {}: {}", w.source_file, w.message);
        }
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
        if !strict_warnings.is_empty() {
            parts.push(format!(
                "{} unused definition{}",
                strict_warnings.len(),
                if strict_warnings.len() == 1 { "" } else { "s" }
            ));
        }
        println!("{total} files checked — {}.", parts.join(", "));
    }
}

fn print_json(
    project_name: &str,
    file_results: &[FileResult],
    cross_errors: &[CrossFileError],
    strict_warnings: &[StrictWarning],
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
        "strict_warnings": strict_warnings.iter().map(|w| serde_json::json!({
            "type": w.kind,
            "source": w.source_file,
            "message": w.message,
        })).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&val).unwrap());
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(
    project_dir: &Path,
    mode: &OutputMode,
    strict: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !project_dir.is_dir() {
        return Err(format!("{}: not a directory", project_dir.display()).into());
    }

    let vr = do_validate(project_dir, strict);

    if mode.json {
        print_json(
            &vr.project_name,
            &vr.file_results,
            &vr.cross_errors,
            &vr.strict_warnings,
            vr.all_valid,
        );
    } else {
        print_human(
            project_dir,
            &vr.file_results,
            &vr.cross_errors,
            &vr.strict_warnings,
            vr.all_valid,
        );
    }

    if !vr.all_valid {
        std::process::exit(1);
    }

    Ok(())
}
