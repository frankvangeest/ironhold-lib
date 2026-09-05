use std::collections::HashSet;
use std::path::Path;

use ironhold_core::schema::camera::CameraModeDef;
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, PrefabKind};
use ironhold_core::schema::items::ItemCatalog;
use ironhold_core::schema::project::LogicRulesAsset;
use ironhold_core::schema::scene_v2::{GameSceneV2, UiNodeDef};
use ironhold_core::schema::player::InputMap;
use ironhold_core::schema::stats::StatCatalog;
use ironhold_core::schema::dialogue::DialogueDef;
use ironhold_core::schema::{Action, ModelFixesAsset, ProjectConfig, StateMachineAsset};
use ironhold_core::runtime::scene_manager::entity_spawner::default_camera_config;

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

/// Resolves a project catalog whose path comes from an optional `ProjectConfig` field --
/// `asset_catalog`, `prefab_catalog`, `stats_path`, `items_path` are all treated identically by
/// the runtime's `project_loader.rs` (each just `.map()`'d into an asset load; no convention-path
/// fallback if unset -- an unset field means the runtime loads nothing at all for that catalog).
///
/// `validate` deliberately does NOT mirror that "nothing at all" runtime behavior when the field
/// is unset: it falls back to `convention_path` instead, via the same tolerant `try_parse` every
/// other convention-path file in this module already uses (silently `None` if that file doesn't
/// exist either -- e.g. every project without a stat system). This keeps every project-config-less
/// or field-less fixture/project validating exactly as before this catalog became configurable
/// (confirmed: no shipped project or existing test fixture has a stray, undeclared catalog file
/// sitting at a convention path it doesn't use, so the fallback is inert for existing content)
/// while still closing the actual reported gap: an explicitly *configured* path is honored exactly,
/// and a configured-but-missing path is a hard error (unlike a merely-absent convention-path file),
/// since the runtime unconditionally tries to load whatever's configured. `try_parse` alone can't
/// express that last distinction -- it silently returns `None` for a missing file with no
/// `FileResult` pushed at all, since it's designed for "this convention path might not apply to
/// this project," not "this configured path should exist."
///
/// A `--strict` warning (`unset_catalog_path_with_convention_file`, in `strict_checks`) reports the
/// one case this fallback deliberately leaves otherwise-silent: a real project with a convention-path
/// file on disk but no matching field set, which validates clean here while the runtime loads
/// nothing for it at all.
fn load_configured_catalog<T: serde::de::DeserializeOwned>(
    project_dir: &Path,
    field: Option<&str>,
    convention_path: &str,
    field_name: &str,
    results: &mut Vec<FileResult>,
) -> Option<T> {
    let Some(path) = field else {
        return try_parse(project_dir, convention_path, results);
    };
    if !project_dir.join(path).is_file() {
        results.push(FileResult {
            rel_path: path.to_string(),
            errors: vec![format!("{field_name} in .project.ron does not exist on disk")],
        });
        return None;
    }
    try_parse(project_dir, path, results)
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
    dialogues: &[(String, DialogueDef)],
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
    for (path, dialogue) in dialogues {
        for node in &dialogue.nodes {
            for choice in &node.choices {
                for action in &choice.do_actions {
                    out.push((path.clone(), action.clone()));
                }
            }
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
    project_config: Option<&ProjectConfig>,
    asset_catalog: Option<&AssetCatalog>,
    prefab_catalog: Option<&PrefabCatalog>,
    stat_catalog: Option<&StatCatalog>,
    item_catalog: Option<&ItemCatalog>,
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
            // See also: Action::Spawn's `spawn_point` reference check further below, in its own
            // loop over `actions` (grouped with the other scene-scoped "union across all scenes"
            // checks rather than here, since it needs `scenes`, not a catalog).
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
            Action::LoadScene(path)
            | Action::LoadSceneOverlay(path)
            | Action::PreloadScene(path)
            | Action::ToggleOverlay(path) => {
                if !project_dir.join(path).exists() {
                    errors.push(CrossFileError {
                        source_file: source.clone(),
                        message: format!(
                            "scene path {:?} not found on disk (paths are relative to the \
                             project folder, e.g. \"scenes/main.scene.ron\")",
                            path
                        ),
                        error_type: "missing_file",
                    });
                }
            }
            Action::StartDialogue { dialogue_path, .. } => {
                if !project_dir.join(dialogue_path).exists() {
                    errors.push(CrossFileError {
                        source_file: source.clone(),
                        message: format!(
                            "dialogue path {:?} not found on disk (paths are relative to the \
                             project folder, e.g. \"dialogues/npc_intro.dialogue.ron\")",
                            dialogue_path
                        ),
                        error_type: "missing_file",
                    });
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
            Action::AddItem { item_key, .. }
            | Action::RemoveItem { item_key, .. }
            | Action::TransferItem { item_key, .. }
            | Action::BuyItem(item_key) => {
                if let Some(c) = item_catalog {
                    if !c.items.contains_key(item_key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("item_key {:?} not found in items.ron", item_key),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::PlayAnimationOn { start_at_fraction: Some(fraction), .. } => {
                if !(0.0..=1.0).contains(fraction) {
                    errors.push(CrossFileError {
                        source_file: source.clone(),
                        message: format!(
                            "PlayAnimationOn: start_at_fraction {:?} is outside the valid \
                             [0.0, 1.0] range — it's a fraction of the clip's duration, not seconds",
                            fraction
                        ),
                        error_type: "animation_start_at_fraction_out_of_range",
                    });
                }
            }
            _ => {}
        }

        // `{new_id}` only resolves inside Action::Spawn's `id` field (action_executor.rs) --
        // anywhere else (a typo, or a misunderstanding of the token) it silently bakes a literal
        // "{new_id}" substring into a live runtime string instead of resolving, which then fails
        // to match whatever it was meant to reference.
        let misplaced_new_id = match action {
            Action::Spawn { .. } => false,
            other => format!("{:?}", other).contains("{new_id}"),
        };
        if misplaced_new_id {
            errors.push(CrossFileError {
                source_file: source.clone(),
                message: "{new_id} only resolves inside Action::Spawn's `id` field -- it will not \
                    be substituted here and will appear as a literal string at runtime"
                    .to_string(),
                error_type: "misplaced_new_id_token",
            });
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

    // A merchant's currency_stat/item_key are only ever read at the moment a player opens the
    // shop (Action::OpenShop) or attempts a purchase (Action::BuyItem) — a typo in either
    // otherwise only surfaces as a runtime no-op the first time someone actually tries to trade.
    // Catches it here instead, mirroring every other key-lookup check in this file.
    if let Some(catalog) = prefab_catalog {
        for (prefab_key, prefab) in &catalog.prefabs {
            let Some(merchant) = &prefab.merchant else { continue };
            if let Some(stats) = stat_catalog {
                if !stats.stats.contains_key(&merchant.currency_stat) {
                    // currency_stat defaults to "gold" when omitted entirely -- if that's the
                    // value that's missing, the designer may not have authored it at all, so say
                    // so rather than implying they typed a bad stat key.
                    let default_note = if merchant.currency_stat == "gold" {
                        " (this is the schema default used when currency_stat is omitted -- \
                          either define a \"gold\" stat or set currency_stat explicitly)"
                    } else {
                        ""
                    };
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: merchant currency_stat {:?} not found in stats.ron{}",
                            prefab_key, merchant.currency_stat, default_note
                        ),
                        error_type: "missing_reference",
                    });
                }
            }
            if let Some(items) = item_catalog {
                for entry in &merchant.stock {
                    if !items.items.contains_key(&entry.item_key) {
                        errors.push(CrossFileError {
                            source_file: "prefabs/prefabs.ron".to_string(),
                            message: format!(
                                "prefab {:?}: merchant stock item_key {:?} not found in items.ron",
                                prefab_key, entry.item_key
                            ),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
        }
    }

    // A prefab's `inventory.initial_items[].item_key` is only read at spawn time -- a typo there
    // doesn't drop the item, it silently creates a stack with no catalog entry (entity_spawner.rs
    // passes None for the catalog, so add_to_slots falls back to max_stack: 99 and the panel
    // renders it at icon_index 0 of the default sheet): a phantom, wrong-icon slot instead of a
    // design-time error. Same failure shape as the merchant stock check above.
    if let Some(catalog) = prefab_catalog {
        if let Some(items) = item_catalog {
            for (prefab_key, prefab) in &catalog.prefabs {
                let Some(inventory) = &prefab.inventory else { continue };
                for entry in &inventory.initial_items {
                    if !items.items.contains_key(&entry.item_key) {
                        errors.push(CrossFileError {
                            source_file: "prefabs/prefabs.ron".to_string(),
                            message: format!(
                                "prefab {:?}: inventory initial_items item_key {:?} not found in \
                                 items.ron",
                                prefab_key, entry.item_key
                            ),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
        }
    }

    // `ItemDef.currency_stat` (set when looting an item should add to a global stat, e.g. a coin
    // pickup, instead of occupying an inventory slot) is only read the moment that item is
    // looted -- a typo doesn't stop the item from being consumed, it just loses the currency gain
    // (action_executor.rs does warn! at runtime, but only there, and only after the item is
    // already gone). Same failure shape as the merchant currency_stat check above, catching it at
    // design time instead.
    if let Some(items) = item_catalog {
        if let Some(stats) = stat_catalog {
            // items.ron's path is configurable via ProjectConfig.items_path (unlike stats.ron's
            // fixed convention path) -- pointing this diagnostic at a literal "items.ron" would
            // send the designer to a path that doesn't exist in every shipped project (they all
            // use "items/items.ron"). Fall back to the literal only in the unreachable case where
            // an item_catalog exists without items_path having been set.
            let items_source = project_config
                .and_then(|c| c.items_path.clone())
                .unwrap_or_else(|| "items.ron".to_string());
            for (item_key, item) in &items.items {
                let Some(currency_stat) = &item.currency_stat else { continue };
                if !stats.stats.contains_key(currency_stat) {
                    errors.push(CrossFileError {
                        source_file: items_source.clone(),
                        message: format!(
                            "item {:?}: currency_stat {:?} not found in stats.ron",
                            item_key, currency_stat
                        ),
                        error_type: "missing_reference",
                    });
                }
            }
        }
    }

    // `join_prefab_keys` (local_coop_hot_join_leave.md) entries are read by Action::JoinPlayer
    // only at the moment a player actually presses the join key — a typo'd or missing entry
    // otherwise only surfaces as a runtime warn!+no-op, never at authoring time. Catch it here,
    // and mirror the same two Action::JoinPlayer executor guards (player-tagged, GLB-only) so a
    // scene author sees the mistake before ever running the project rather than discovering a
    // silent no-op (or, for the primitive case, a spawn-time panic) during a playtest.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            for (slot, entry) in scene.join_prefab_keys.iter().enumerate() {
                let Some(prefab_key) = entry else { continue };
                let Some(prefab) = catalog.prefabs.get(prefab_key) else {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "join_prefab_keys[{}]: prefab {:?} not found in prefabs.ron",
                            slot, prefab_key
                        ),
                        error_type: "missing_reference",
                    });
                    continue;
                };
                if !prefab.components.tags.iter().any(|t| t == "player") {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "join_prefab_keys[{}]: prefab {:?} has no `tags: [\"player\"]` — \
                             Action::JoinPlayer will refuse to hot-join it at runtime",
                            slot, prefab_key
                        ),
                        error_type: "unsupported_join_prefab",
                    });
                } else if prefab.kind == PrefabKind::Primitive {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "join_prefab_keys[{}]: prefab {:?} is primitive-shaped (kind: \
                             Primitive) — hot-join only supports GLB (Actor-kind) players in v1",
                            slot, prefab_key
                        ),
                        error_type: "unsupported_join_prefab",
                    });
                }
            }
        }
    }

    // The project's own boot scene -- the highest-consequence scene path of all, since a typo
    // here means the project never gets past a blank/loading screen. Same on-disk-existence
    // check as the LoadScene/LoadSceneOverlay/PreloadScene/ToggleOverlay action arm above,
    // mirroring how the runtime resolves it (project_loader.rs's resolve_project_path).
    if let Some(config) = project_config {
        if !project_dir.join(&config.initial_scene).exists() {
            errors.push(CrossFileError {
                source_file: find_project_ron(project_dir).unwrap_or_default(),
                message: format!(
                    "initial_scene {:?} not found on disk (paths are relative to the project \
                     folder, e.g. \"scenes/main.scene.ron\")",
                    config.initial_scene
                ),
                error_type: "missing_file",
            });
        }
    }

    // `global_unclaimed_gamepad_bindings`/`scene_unclaimed_gamepad_bindings` (gamepad_hot_join.md)
    // button names are only checked at runtime (a `warn!` in
    // project_loader.rs/scene_loader.rs) — same design-time gap `join_prefab_keys` above closes
    // for its own field. Catch it here too, so a typo'd button name surfaces at validate time
    // instead of only as a silent no-op the first time someone presses it.
    if let Some(config) = project_config {
        for button_name in config.global_unclaimed_gamepad_bindings.keys() {
            if InputMap::parse_gamepad_button(button_name).is_none() {
                errors.push(CrossFileError {
                    source_file: find_project_ron(project_dir).unwrap_or_default(),
                    message: format!(
                        "global_unclaimed_gamepad_bindings: unrecognised button name {:?} — binding will have no effect",
                        button_name
                    ),
                    error_type: "invalid_binding",
                });
            }
        }
    }
    for (scene_path, scene) in scenes {
        for button_name in scene.scene_unclaimed_gamepad_bindings.keys() {
            if InputMap::parse_gamepad_button(button_name).is_none() {
                errors.push(CrossFileError {
                    source_file: scene_path.clone(),
                    message: format!(
                        "scene_unclaimed_gamepad_bindings: unrecognised button name {:?} — binding will have no effect",
                        button_name
                    ),
                    error_type: "invalid_binding",
                });
            }
        }
    }

    // A primitive-shaped (`kind: Primitive`) player prefab combined with `scene.terrain:
    // Some(...)` isn't supported yet — v3 of `player_model_source_unification.md`. Mirrors the
    // scene-load-time `warn!` in `scene_loader.rs`; this is the design-time counterpart so a
    // scene author sees it before ever running the project.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            if scene.terrain.is_none() { continue; }
            for entity in &scene.entities {
                let Some(prefab) = catalog.prefabs.get(&entity.prefab) else { continue };
                if prefab.kind == PrefabKind::Primitive
                    && prefab.components.tags.iter().any(|t| t == "player")
                {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "entity {:?}: primitive-shaped player prefab {:?} combined with \
                             scene.terrain — primitive players aren't supported on terrain-\
                             deferred spawn yet (v3 of player_model_source_unification.md); use a \
                             GLB (Actor-kind) player prefab for terrain scenes, or remove terrain \
                             from this scene",
                            entity.id, entity.prefab
                        ),
                        error_type: "unsupported_primitive_player_on_terrain",
                    });
                }
            }
        }
    }

    // A scene authoring 2+ `tags: ["flycam"]` entities silently keeps only the last one in
    // `entities:` order at runtime (`scene_loader.rs`) — this is the design-time counterpart so a
    // scene author sees it before ever running the project. See
    // `planning/features/flycam_scene_conflicts.md`.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            let mut flycam_ids: Vec<&str> = Vec::new();
            for entity in &scene.entities {
                let Some(prefab) = catalog.prefabs.get(&entity.prefab) else { continue };
                if prefab.is_flycam() {
                    flycam_ids.push(&entity.id);
                }
            }
            if flycam_ids.len() > 1 {
                errors.push(CrossFileError {
                    source_file: scene_path.clone(),
                    message: format!(
                        "scene has {} `tags: [\"flycam\"]` entities ({}) — only the last one in \
                         `entities:` order is used at runtime, the rest are silently discarded. \
                         Remove all but one flycam-tagged entity from this scene.",
                        flycam_ids.len(),
                        flycam_ids.join(", ")
                    ),
                    error_type: "duplicate_flycam_entity",
                });
            }
        }
    }

    // A `tags: ["flycam"]` prefab's `model`/`shape`/`primitive`/`children` are silently discarded
    // at scene load (`scene_loader.rs`'s `is_flycam` branch `continue`s before any of them are
    // ever consulted), and a prefab tagged both `"player"` and `"flycam"` never spawns its player
    // components at all — this is the design-time counterpart to both scene-load `warn!`s.
    // Prefab-catalog-scoped (not per-scene, unlike `duplicate_flycam_entity` above): the condition
    // is entirely prefab-local, so one bad prefab would otherwise report once per scene that
    // instantiates it. Both scoped to scene-`entities:`-placed flycams specifically — a flycam
    // prefab dynamically `Action::Spawn`ed at runtime doesn't go through this branch and isn't
    // covered (logged in `planning/claude_suggestions.md`).
    // See `planning/features/flycam_model_never_renders_warning.md`.
    if let Some(catalog) = prefab_catalog {
        let mut prefab_keys: Vec<&String> = catalog.prefabs.keys().collect();
        prefab_keys.sort();
        for prefab_key in prefab_keys {
            let prefab = &catalog.prefabs[prefab_key];
            if !prefab.is_flycam() {
                continue;
            }
            if prefab.is_player() {
                errors.push(CrossFileError {
                    source_file: "prefabs/prefabs.ron".to_string(),
                    message: format!(
                        "prefab '{}' has both \"player\" and \"flycam\" tags — the flycam tag \
                         makes it spawn as a camera-only entity and its player components never \
                         spawn at all. Use camera_mode: Flycam(...) on a \"player\"-only prefab \
                         instead if you want a flying player character, or remove the \"player\" \
                         tag if you wanted a plain camera-only flycam.",
                        prefab_key
                    ),
                    error_type: "flycam_player_tag_conflict",
                });
                continue;
            }
            let ignored_fields = prefab.flycam_ignored_fields();
            if !ignored_fields.is_empty() {
                let remedy = PrefabDef::flycam_ignored_fields_remedy(&ignored_fields);
                errors.push(CrossFileError {
                    source_file: "prefabs/prefabs.ron".to_string(),
                    message: format!(
                        "flycam prefab '{}' sets {} — a flycam is camera-only and never renders \
                         a body, so that body will never appear. To silence this, {}. To give a \
                         flying camera a visible body, use camera_mode: Flycam(...) on a \
                         \"player\" prefab instead, or spawn the body as a separate non-flycam \
                         entity at the same position.",
                        prefab_key, ignored_fields.join(", "), remedy
                    ),
                    error_type: "flycam_model_never_renders",
                });
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

        // `Label`/`Button.font_size <= 0.0` — Bevy's text pipeline doesn't panic on this (guarded
        // before cosmic-text's own `assert_ne!(font_size, 0.0)`), it just silently renders
        // nothing, and the one `warn!` it does log fires via `once!` — a per-process flag, so
        // only the very first offending entity in the whole session is ever reported; a second
        // mis-authored label anywhere, or the same one on a scene reload, produces no diagnostic
        // at all. That "looks fine but is subtly wrong" failure mode is exactly what this CLI
        // check exists to catch at design time instead.
        for node in &scene.ui {
            let (kind, id, font_size) = match node {
                ironhold_core::schema::scene_v2::UiNodeDef::Label(l) => ("Label", &l.id, l.font_size),
                ironhold_core::schema::scene_v2::UiNodeDef::Button(b) => ("Button", &b.id, b.font_size),
                _ => continue,
            };
            if font_size <= 0.0 {
                errors.push(CrossFileError {
                    source_file: scene_path.clone(),
                    message: format!(
                        "{kind} {id:?}: font_size {font_size} must be > 0.0 — the text will silently \
                         render nothing (Bevy only warns once, ever, for the whole process)"
                    ),
                    error_type: "invalid_font_size",
                });
            }
        }

        // Same-player gamepad-slot collision — a different failure mode than the keyboard check
        // above, so a separate pass: the intent/cooldown pipeline is never keyed by `gamepad_key`,
        // so there's no cross-bar pipeline entanglement risk here. The risk is a same-player
        // double-fire (one physical button press activating 2 slots for the same player). Keyed
        // by `(owner_player.unwrap_or(0), GamepadButton)` — matching the runtime's
        // `owns_slot`/`warn_missing_player_stat_templates` "None/Some(0) both mean the primary
        // player" normalization — so two *different* players' bars sharing a button name (each has
        // their own physical pad) is correctly not flagged. See
        // `planning/features/gamepad_action_bar_slots.md`.
        let mut seen_gamepad: std::collections::HashMap<(u32, _), (&str, &str)> = std::collections::HashMap::new();
        for node in &scene.ui {
            let ironhold_core::schema::scene_v2::UiNodeDef::ActionBar(bar) = node else { continue };
            let owner_player = bar.owner_player.unwrap_or(0);
            for slot in &bar.slots {
                let Some(gk) = &slot.gamepad_key else { continue };
                match ironhold_core::schema::player::InputMap::parse_gamepad_button(gk) {
                    None => errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "ActionBar {:?}: slot {:?} has an unrecognised gamepad_key {:?} — it will never fire from gamepad",
                            bar.id, slot.key, gk
                        ),
                        error_type: "invalid_gamepad_key",
                    }),
                    Some(btn) => {
                        if let Some((prev_bar, prev_key)) = seen_gamepad.insert((owner_player, btn), (&bar.id, &slot.key)) {
                            errors.push(CrossFileError {
                                source_file: scene_path.clone(),
                                message: format!(
                                    "Player {} has 2+ ActionBar slots bound to gamepad button {:?}: ActionBar {:?} \
                                     slot {:?} and ActionBar {:?} slot {:?} — one press of this button would \
                                     activate both slots for this player",
                                    owner_player, btn, prev_bar, prev_key, bar.id, slot.key
                                ),
                                error_type: "same_player_gamepad_duplicate_key",
                            });
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

    // A slot's `gamepad_key` resolves against its owning player's own `BoundGamepad` (seeded once
    // from `InputMap.gamepad_index`; `gamepad_bind_system` never falls back to any connected
    // pad), so a slot that declares `gamepad_key` for a player whose prefab sets no
    // `gamepad_index` at all is silently inert: no crash, no runtime signal, the slot simply
    // never fires from gamepad. Mirrors the `missing_player_stat_template` check above exactly,
    // including the `unwrap_or(0)` normalization. See `planning/features/gamepad_action_bar_slots.md`.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            for node in &scene.ui {
                let ironhold_core::schema::scene_v2::UiNodeDef::ActionBar(bar) = node else { continue };
                let owner_player = bar.owner_player.unwrap_or(0);
                let player_prefab = scene.entities.iter()
                    .filter_map(|e| catalog.prefabs.get(&e.prefab))
                    .find(|p| p.player_index == owner_player && p.components.tags.iter().any(|t| t == "player"));
                let Some(prefab) = player_prefab else { continue };
                let has_gamepad_index = prefab.components.inputs.as_ref()
                    .is_some_and(|i| i.gamepad_index.is_some());
                if has_gamepad_index { continue; }
                for slot in &bar.slots {
                    let Some(gamepad_key) = &slot.gamepad_key else { continue };
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "ActionBar {:?} slot {:?} declares gamepad_key {:?}, but player_index \
                             {}'s prefab sets no inputs.gamepad_index — this binding will never \
                             fire from gamepad (the slot's keyboard key, if any, still works)",
                            bar.id, slot.key, gamepad_key, owner_player
                        ),
                        error_type: "gamepad_key_without_gamepad_index",
                    });
                }
            }
        }
    }

    // Two or more player-tagged prefabs **instantiated in the same scene's `entities:` list, or
    // reachable via that scene's `join_prefab_keys` hot-join slots** authoring the same non-`None`
    // `gamepad_index` — one physical controller would drive both characters at once, whether both
    // are scene-placed, both are hot-join slots, or one of each (a hot-joined player's
    // `gamepad_index` seed is read from its prefab exactly like a scene-placed player's, unless a
    // gamepad-triggered join instead captures the triggering pad directly — see "Gamepad-triggered
    // hot join" in `crates/ironhold_core/src/CLAUDE.md` — so a keyboard-triggered join can still
    // collide with an already-bound scene player via this same seed). Deliberately scoped to each
    // scene's instantiated/reachable players, not the raw prefab catalog: `local_coop_demo`'s
    // catalog legitimately reuses `gamepad_index` values across different rooms' player variants
    // (never co-instantiated), which a catalog-wide check would false-positive on. Mirrors the
    // runtime `warn!` in `scene_loader.rs`'s `warn_duplicate_gamepad_index` only for the
    // `entities:` half — that warning only scans players already instantiated at scene-load time,
    // so it cannot see a `join_prefab_keys` collision at all; this check is the only design-time
    // (or any-time) signal for that case until the join spawn path itself. See
    // `planning/features/gamepad_player_binding_hardening.md`.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            let mut seen: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
            let mut check_seed = |id: String, prefab_key: &str, errors: &mut Vec<CrossFileError>| {
                let Some(prefab) = catalog.prefabs.get(prefab_key) else { return };
                if !prefab.components.tags.iter().any(|t| t == "player") { return }
                let Some(seed) = prefab.components.inputs.as_ref().and_then(|i| i.gamepad_index)
                else { return };
                if let Some(other_id) = seen.insert(seed, id.clone()) {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "players {:?} and {:?} both use gamepad_index: {} — one physical \
                             controller would drive both characters at once. Give each player a \
                             different gamepad_index. Deliberately sharing one controller between \
                             two characters is not supported",
                            other_id, id, seed
                        ),
                        error_type: "duplicate_gamepad_index",
                    });
                }
            };
            for entity_def in &scene.entities {
                check_seed(entity_def.id.clone(), &entity_def.prefab, &mut errors);
            }
            for (slot, entry) in scene.join_prefab_keys.iter().enumerate() {
                let Some(prefab_key) = entry else { continue };
                check_seed(
                    format!("join_prefab_keys[{slot}] (prefab {prefab_key:?})"),
                    prefab_key,
                    &mut errors,
                );
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

            if let Some(dialogue_path) = &def.dialogue {
                if !project_dir.join(dialogue_path).exists() {
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: dialogue {:?} not found on disk",
                            key, dialogue_path
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

    // `camera_modes:` registry authoring mistakes (planning/features/camera_modes.md v2, "Named
    // mode registry" resolution) — mirrors the load-time `warn!`s in `warn_camera_modes_registry`
    // (scene_loader.rs), as the design-time counterpart.
    for (scene_path, scene) in scenes {
        for (key, mode) in &scene.camera_modes {
            if key == "default" {
                errors.push(CrossFileError {
                    source_file: scene_path.clone(),
                    message: format!(
                        "camera_modes: preset {:?} uses the reserved key \"default\" — \
                         SetCameraMode(mode: \"default\") always restores a camera's own \
                         scene-authored starting mode, never a designer-defined preset; rename \
                         this entry",
                        key
                    ),
                    error_type: "reserved_camera_mode_key",
                });
            }
            if matches!(mode, CameraModeDef::Party(_)) {
                errors.push(CrossFileError {
                    source_file: scene_path.clone(),
                    message: format!(
                        "camera_modes: preset {:?} is Party(...), which cannot be reached via \
                         SetCameraMode (no per-camera meaning when targeting a single camera) — \
                         remove it or replace it with a different mode",
                        key
                    ),
                    error_type: "unsupported_registry_camera_mode",
                });
            }
            if let CameraModeDef::Fixed(fx) = mode {
                if let Some(target_id) = &fx.look_at_entity {
                    if !scene.entities.iter().any(|e| &e.id == target_id) {
                        errors.push(CrossFileError {
                            source_file: scene_path.clone(),
                            message: format!(
                                "camera_modes: preset {:?}: Fixed's look_at_entity {:?} does not \
                                 match any entity id in this scene's entities — the camera will \
                                 silently fail to find it and hold its last known rotation \
                                 (or none, if it never resolved) every frame",
                                key, target_id
                            ),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
        }
    }

    // `Action::SetCameraMode(mode:)` must be either the reserved "default" or a key present in
    // SOME scene's camera_modes registry. Weaker than the project-scoped catalog checks above
    // (rules.ron/state_machine.ron are project-scoped, but camera_modes is scene-scoped, so
    // "defined in scene A, fired only while scene B is active" isn't caught) — still catches the
    // dominant failure, a typo'd key, which today only surfaces as a silent runtime warn!+no-op.
    for (source, action) in actions {
        let Action::SetCameraMode { mode, .. } = action else { continue };
        if mode == "default" {
            continue;
        }
        let found_in_any_scene = scenes.iter().any(|(_, scene)| scene.camera_modes.contains_key(mode));
        if !found_in_any_scene {
            errors.push(CrossFileError {
                source_file: source.clone(),
                message: format!(
                    "SetCameraMode: mode {:?} is not \"default\" and not found in any scene's \
                     camera_modes registry — this will silently warn+no-op at runtime",
                    mode
                ),
                error_type: "missing_reference",
            });
        }
    }

    // `Action::Spawn`'s `spawn_point` must match a key in SOME scene's `spawn_points` map. Same
    // weaker-than-project-scoped caveat as the `SetCameraMode` check above: rules.ron/
    // state_machine.ron actions are project-scoped but `spawn_points` is scene-scoped, so "defined
    // in scene A, fired only while scene B is active" isn't caught — still catches the dominant
    // failure, a typo'd spawn_point name, which today only warns and falls back to the world
    // origin at runtime (`action_executor.rs`). `at_entity` is deliberately not checked here — it
    // needs harder reachability reasoning about which entities exist when the action fires (and,
    // like `spawn_point` itself, is `{self}`/`{target}`-substituted, so a literal check on it
    // would have the same false-positive problem this check guards against below) and stays
    // deferred; see `planning/backlog.md`'s "CLI validate: no reference check for `Action::Spawn`'s
    // `spawn_point`".
    for (source, action) in actions {
        let Action::Spawn { spawn_point: Some(spawn_point), .. } = action else { continue };
        // `spawn_point` is substituted at interpret time for `{self}`/`{target}` tokens
        // (message_interpreter.rs, dialogue.rs) — see the supported-fields list in
        // `crates/ironhold_core/src/CLAUDE.md`. The authored string here is pre-substitution, so a
        // templated value (e.g. `"{self}_spawn"`, used to share one behavior rule across several
        // named spawn points) is not the literal key that will be looked up at runtime; skip it
        // rather than false-positive on a legal pattern.
        if spawn_point.contains('{') {
            continue;
        }
        let found_in_any_scene =
            scenes.iter().any(|(_, scene)| scene.spawn_points.contains_key(spawn_point));
        if !found_in_any_scene {
            errors.push(CrossFileError {
                source_file: source.clone(),
                message: format!(
                    "Spawn: spawn_point {:?} not found in any scene's spawn_points map — this \
                     will silently warn and fall back to the world origin at runtime",
                    spawn_point
                ),
                error_type: "missing_reference",
            });
        }
    }

    // `label_depth_scale.min_scale` outside the documented [0.0, 1.0] range — design-time
    // counterpart of the scene-load clamp+warn! in `scene_loader.rs::
    // warn_label_depth_scale_min_scale_out_of_range`. Without this fix, a value > 1.0 would pin
    // every depth-scaled widget in this scene at that factor forever, regardless of camera
    // distance (the engine clamps it to 1.0 instead); a negative value is inert (never binds
    // against an already-non-negative ratio) rather than doing anything useful. See
    // `planning/features/label_depth_scale_validation.md`.
    for (scene_path, scene) in scenes {
        let Some(cfg) = &scene.label_depth_scale else { continue };
        let Some(min_scale) = cfg.min_scale else { continue };
        if min_scale.is_finite() && (0.0..=1.0).contains(&min_scale) {
            continue;
        }
        let message = if !min_scale.is_finite() {
            format!(
                "label_depth_scale.min_scale is {} (not a finite number) — must be in [0.0, 1.0]",
                min_scale
            )
        } else if min_scale > 1.0 {
            format!(
                "label_depth_scale.min_scale is {} — outside [0.0, 1.0]. Without this fix, every \
                 nameplate/stat label/bar in this scene would pin at {:.0}% size forever, \
                 regardless of camera distance — the engine now clamps it to 1.0 (100%) instead",
                min_scale, min_scale * 100.0
            )
        } else {
            format!(
                "label_depth_scale.min_scale is {} — outside [0.0, 1.0]. Negative values are \
                 silently inert (no effect on scaling) rather than doing anything useful",
                min_scale
            )
        };
        errors.push(CrossFileError {
            source_file: scene_path.clone(),
            message,
            error_type: "label_depth_scale_min_scale_out_of_range",
        });
    }

    errors
}

// ── UI trigger reachability ───────────────────────────────────────────────────

/// Every event string any rule/transition/binding in the project's logic matches against —
/// `rules.ron`'s `on:`, `state_machine.ron`'s in-state `on:`/`transitions[].on`/`global_on:`,
/// and the same three fields in every behavior file. Takes the same already-parsed
/// `rules`/`state_machine`/`behaviors` `do_validate` builds for `collect_actions` above, rather
/// than re-reading the files from disk — a malformed logic file then degrades exactly like every
/// other check in this module (an incomplete `handled` set alongside the file's own already-
/// reported parse error), instead of `check_ui_trigger_reachability` silently swallowing that
/// same parse error a second time and fabricating an `unreachable_trigger` report against every
/// button in the project on top of the real error.
fn collect_handled_events(
    rules: Option<&LogicRulesAsset>,
    state_machine: Option<&StateMachineAsset>,
    behaviors: &[(String, StateMachineAsset)],
) -> HashSet<String> {
    let mut events = HashSet::new();

    if let Some(rules) = rules {
        for rule in &rules.rules {
            events.insert(rule.on.clone());
        }
    }
    if let Some(fsm) = state_machine {
        collect_fsm_events(fsm, &mut events);
    }
    for (_, fsm) in behaviors {
        collect_fsm_events(fsm, &mut events);
    }

    events
}

fn collect_fsm_events(fsm: &StateMachineAsset, events: &mut HashSet<String>) {
    for state in &fsm.states {
        for binding in &state.on {
            events.insert(binding.event.clone());
        }
    }
    for t in &fsm.transitions {
        events.insert(t.on.clone());
    }
    for binding in &fsm.global_on {
        events.insert(binding.event.clone());
    }
}

/// For every scene `Button`/`IconButton`, every `global_key_bindings`/`scene_key_bindings` entry,
/// and every `global_unclaimed_gamepad_bindings`/`scene_unclaimed_gamepad_bindings` entry, derive
/// the `ui.button_pressed:{trigger}` event it fires at runtime
/// (`scene_manager/scene_loader.rs`'s `strip_prefix("ui.")` derivation for buttons; every binding
/// map's value is used as the trigger directly, no `ui.` stripping — see `ProjectConfig.
/// global_key_bindings`'s doc comment) and confirm at least one rule/transition/binding anywhere
/// in the project's logic actually matches it. A mismatch means the button/binding is live and
/// fires a `UiEvent` that is matched against zero rules and silently dropped — "I clicked the
/// button and nothing happened" (or "I pressed the key/gamepad button and nothing happened"),
/// with no other symptom. Not gated behind `--strict`: this is "referenced but never resolves,"
/// the same severity class as every other missing-key check above, not an orphan-detection
/// question.
///
/// Deliberately not extended to dialogue choice buttons (`dialogue_choice:{n}`) — those are
/// spawned dynamically by `dialogue.rs` from `DialogueChoiceDef`, never appear as a `UiNodeDef`
/// in scene RON, and are matched directly by `dialogue_tick_system`, not through
/// `rules.ron`/`state_machine.ron`. Nothing here walks `scene.ui` for them, so there is no
/// false-positive risk from that surface.
///
/// **Known latent gap, no shipped project hits it today:** an entity `.behavior.ron`'s event
/// pattern can contain a `{self}` token, substituted against the owning entity's spawn id at
/// match time (`message_interpreter.rs`) — `collect_handled_events` stores the raw,
/// pre-substitution literal, so a behavior authored as `on: "ui.button_pressed:{self}_open"`
/// would never string-match a button's already-concrete derived event and would be wrongly
/// reported as unreachable. No shipped behavior file currently handles a `ui.button_pressed:*`
/// event, so this is theoretical; if one ever does, this check will need `{self}`-aware matching.
///
/// This function's own site enumeration is mirrored by `collect_reachable_ui_triggers` below (the
/// reverse-direction `orphan_rule` check's data source) — keep both in sync if a new UI trigger
/// site type is ever added here. The `{self}`/`dialogue_choice:` false-positive exclusions above
/// are also handled there, in `check_orphan_event`.
fn check_ui_trigger_reachability(
    project_dir: &Path,
    project_config: Option<&ProjectConfig>,
    scenes: &[(String, GameSceneV2)],
    rules: Option<&LogicRulesAsset>,
    state_machine: Option<&StateMachineAsset>,
    behaviors: &[(String, StateMachineAsset)],
    logic_files_parsed_cleanly: bool,
) -> Vec<CrossFileError> {
    // A malformed rules.ron/state_machine.ron/behavior file already reports its own parse error
    // in the per-file results above. Treating that file's "nothing parsed" state as "this project
    // handles nothing" would flood every button/binding in the project with a derived
    // `unreachable_trigger` report piled on top of the one real root cause. Skip entirely until
    // the parse error is fixed rather than fabricate a wave of secondary noise.
    if !logic_files_parsed_cleanly {
        return Vec::new();
    }

    let mut errors = Vec::new();
    let handled = collect_handled_events(rules, state_machine, behaviors);

    // `verb`/`consequence` phrase the message correctly for each trigger source — a key/gamepad
    // binding is never "clicked," and saying so is actively misleading (a designer debugging a
    // broken Escape binding would go looking for a nonexistent button).
    let check = |errors: &mut Vec<CrossFileError>,
                 source: &str,
                 describe: String,
                 trigger: &str,
                 verb: &str,
                 consequence: &str| {
        if trigger.is_empty() {
            errors.push(CrossFileError {
                source_file: source.to_string(),
                message: format!("{describe} has no action configured — {consequence}"),
                error_type: "unreachable_trigger",
            });
            return;
        }
        let event = format!("ui.button_pressed:{trigger}");
        if !handled.contains(&event) {
            errors.push(CrossFileError {
                source_file: source.to_string(),
                message: format!(
                    "{describe} fires {event:?} {verb}, but no rule/transition/binding in \
                     rules.ron, state_machine.ron, or a behavior file handles it — {consequence}"
                ),
                error_type: "unreachable_trigger",
            });
        }
    };

    if let Some(config) = project_config {
        let source = find_project_ron(project_dir).unwrap_or_default();
        for (key, trigger) in &config.global_key_bindings {
            check(
                &mut errors, &source, format!("global_key_bindings[{key:?}]"), trigger,
                "when the key is pressed", "pressing this key will do nothing",
            );
        }
        for (button, trigger) in &config.global_unclaimed_gamepad_bindings {
            check(
                &mut errors, &source, format!("global_unclaimed_gamepad_bindings[{button:?}]"), trigger,
                "when the gamepad button is pressed", "pressing it will do nothing",
            );
        }
    }

    for (scene_path, scene) in scenes {
        for (key, trigger) in &scene.scene_key_bindings {
            check(
                &mut errors, scene_path, format!("scene_key_bindings[{key:?}]"), trigger,
                "when the key is pressed", "pressing this key will do nothing",
            );
        }
        for (button, trigger) in &scene.scene_unclaimed_gamepad_bindings {
            check(
                &mut errors, scene_path, format!("scene_unclaimed_gamepad_bindings[{button:?}]"), trigger,
                "when the gamepad button is pressed", "pressing it will do nothing",
            );
        }
        for node in &scene.ui {
            match node {
                UiNodeDef::Button(btn) => {
                    let trigger = btn.action.strip_prefix("ui.").unwrap_or(&btn.action);
                    check(
                        &mut errors, scene_path, format!("Button {:?}", btn.id), trigger,
                        "when clicked", "clicking it will do nothing",
                    );
                }
                UiNodeDef::IconButton(btn) => {
                    let trigger = btn.action.strip_prefix("ui.").unwrap_or(&btn.action);
                    check(
                        &mut errors, scene_path, format!("IconButton {:?}", btn.id), trigger,
                        "when clicked", "clicking it will do nothing",
                    );
                }
                _ => {}
            }
        }
    }

    errors
}

/// Every `ui.button_pressed:{trigger}` event derivable from this project's buttons/key/gamepad
/// bindings, unioned across all scenes — mirrors `check_ui_trigger_reachability`'s own site
/// enumeration exactly (global/scene key bindings, global/scene unclaimed gamepad bindings,
/// scene `Button`/`IconButton` nodes); keep both in sync if a new UI trigger site type is ever
/// added. Feeds `check_orphan_ui_rules` below — the same "same two data sets" backing both
/// directions of the reachability question (forward: does a button/binding's fire resolve to a
/// handled rule; reverse: does a rule's `on:` resolve to some button/binding that can fire it).
///
/// Also includes the five engine-hardcoded panel triggers (`close_inventory`/`close_shop`/
/// `close_container`/`take_all_from_container`/`buy_item:{item_key}`, `scene_loader.rs`'s
/// panel-spawn sites and `action_executor.rs`'s per-`MerchantDef.stock[]` entry) — a designer
/// never authors these as a `Button.action` string, they're emitted internally whenever a
/// panel's own built-in close/buy button is clicked, so they'd otherwise false-positive as
/// orphaned every time `check_orphan_ui_rules` sees the (correct, live) state-machine rule that
/// handles one — confirmed against `3rd_person_game_demo`'s real `ShopPanel`/`InventoryPanel`/
/// `ContainerPanel` usage before this was added. The *forward* direction (`unreachable_trigger`)
/// still doesn't cover these five — see `planning/backlog.md`'s "doesn't cover the five
/// engine-hardcoded panel triggers" entry, deliberately left open, out of scope here.
fn collect_reachable_ui_triggers(
    project_config: Option<&ProjectConfig>,
    scenes: &[(String, GameSceneV2)],
    prefab_catalog: Option<&PrefabCatalog>,
) -> HashSet<String> {
    let mut reachable = HashSet::new();
    let mut insert = |trigger: &str| {
        if !trigger.is_empty() {
            reachable.insert(format!("ui.button_pressed:{trigger}"));
        }
    };

    if let Some(config) = project_config {
        for trigger in config.global_key_bindings.values() {
            insert(trigger);
        }
        for trigger in config.global_unclaimed_gamepad_bindings.values() {
            insert(trigger);
        }
    }
    for (_, scene) in scenes {
        for trigger in scene.scene_key_bindings.values() {
            insert(trigger);
        }
        for trigger in scene.scene_unclaimed_gamepad_bindings.values() {
            insert(trigger);
        }
        for node in &scene.ui {
            match node {
                UiNodeDef::Button(btn) => insert(btn.action.strip_prefix("ui.").unwrap_or(&btn.action)),
                UiNodeDef::IconButton(btn) => insert(btn.action.strip_prefix("ui.").unwrap_or(&btn.action)),
                UiNodeDef::InventoryPanel(_) => insert("close_inventory"),
                UiNodeDef::ShopPanel(_) => {
                    insert("close_shop");
                    if let Some(catalog) = prefab_catalog {
                        for prefab in catalog.prefabs.values() {
                            if let Some(merchant) = &prefab.merchant {
                                for entry in &merchant.stock {
                                    insert(&format!("buy_item:{}", entry.item_key));
                                }
                            }
                        }
                    }
                }
                UiNodeDef::ContainerPanel(_) => {
                    insert("close_container");
                    insert("take_all_from_container");
                }
                _ => {}
            }
        }
    }
    reachable
}

/// `--strict` reverse of `check_ui_trigger_reachability` above: a rule/transition/binding whose
/// `on:`/`event:` matches the `ui.button_pressed:{trigger}` shape but no button/key/gamepad
/// binding anywhere in the project can ever produce that exact trigger — dead code left over from
/// a scene rewrite (a renamed/removed button, a rule nobody wired up). Only ever inspects
/// `ui.button_pressed:*`-shaped strings; every other event shape (`scene.ready:*`,
/// `entity.entered:*`, a custom `EmitEvent` name, dialogue events, etc.) has no button/binding
/// origin at all and is out of scope for this check, not merely unreachable-by-this-analysis.
/// Same "union across all scenes, not project-scoped" approximation as `collect_reachable_ui_triggers`,
/// and the same scene-parse-failure blind spot already accepted for `SetCameraMode`/`spawn_point`
/// elsewhere in this file (a scene that fails to parse silently drops its buttons from the
/// reachable set, which could make an otherwise-live rule look orphaned) — not fixed here, see
/// `planning/claude_suggestions.md`.
fn check_orphan_ui_rules(
    reachable: &HashSet<String>,
    rules: Option<(&str, &LogicRulesAsset)>,
    state_machine: Option<(&str, &StateMachineAsset)>,
    behaviors: &[(String, StateMachineAsset)],
) -> Vec<StrictWarning> {
    let mut warnings = Vec::new();
    if let Some((src, r)) = rules {
        for rule in &r.rules {
            check_orphan_event(&mut warnings, reachable, src, format!("rule handling {:?}", rule.on), &rule.on);
        }
    }
    if let Some((src, fsm)) = state_machine {
        check_fsm_orphans(&mut warnings, reachable, src, fsm);
    }
    for (path, fsm) in behaviors {
        check_fsm_orphans(&mut warnings, reachable, path, fsm);
    }
    warnings
}

fn check_fsm_orphans(
    warnings: &mut Vec<StrictWarning>,
    reachable: &HashSet<String>,
    source: &str,
    fsm: &StateMachineAsset,
) {
    for state in &fsm.states {
        for binding in &state.on {
            check_orphan_event(
                warnings, reachable, source,
                format!("state {:?}'s on: binding for {:?}", state.name, binding.event),
                &binding.event,
            );
        }
    }
    for t in &fsm.transitions {
        check_orphan_event(
            warnings, reachable, source,
            format!("transition to {:?} on {:?}", t.to, t.on),
            &t.on,
        );
    }
    for binding in &fsm.global_on {
        check_orphan_event(
            warnings, reachable, source,
            format!("global_on binding for {:?}", binding.event),
            &binding.event,
        );
    }
}

fn check_orphan_event(
    warnings: &mut Vec<StrictWarning>,
    reachable: &HashSet<String>,
    source: &str,
    describe: String,
    event: &str,
) {
    if !event.starts_with("ui.button_pressed:") {
        return;
    }
    // `dialogue_choice:{n}` is a sixth engine-emitted UiAction::Trigger source (dialogue.rs's
    // choice buttons), spawned dynamically and never appearing as a UiNodeDef in scene RON --
    // `collect_reachable_ui_triggers` has no way to enumerate it (there's no fixed set of choice
    // indices; a dialogue can have any number of nodes/choices), so a rule correctly handling one
    // would otherwise always look orphaned. Skip rather than false-positive on a working project.
    if event.starts_with("ui.button_pressed:dialogue_choice:") {
        return;
    }
    // Same false-positive class every other string-key check in this file already guards against
    // (e.g. stat_label's "{self}." skip): a behavior file's on:/event: pattern can contain a
    // `{self}` token, substituted against the owning entity's spawn id at match time
    // (message_interpreter.rs) -- the raw, pre-substitution literal stored here would never
    // string-match the button's already-concrete derived event.
    if event.contains('{') {
        return;
    }
    if reachable.contains(event) {
        return;
    }
    warnings.push(StrictWarning {
        source_file: source.to_string(),
        message: format!(
            "{describe} — no button/key/gamepad binding anywhere in the project can ever fire \
             {event:?}. Dead code, or a stale event name left over from a rename/removal."
        ),
        kind: "orphan_rule",
    });
}

// ── Strict (orphan) checks ────────────────────────────────────────────────────

fn strict_checks(
    project_dir: &Path,
    project_config: Option<&ProjectConfig>,
    asset_catalog: Option<&AssetCatalog>,
    prefab_catalog: Option<&PrefabCatalog>,
    scenes: &[(String, GameSceneV2)],
    actions: &[(String, Action)],
    rules: Option<(&str, &LogicRulesAsset)>,
    state_machine: Option<(&str, &StateMachineAsset)>,
    behaviors: &[(String, StateMachineAsset)],
    orphan_rule_prereqs_clean: bool,
) -> Vec<StrictWarning> {
    let mut warnings: Vec<StrictWarning> = Vec::new();

    // `load_configured_catalog` falls back to checking a catalog's convention-path file whenever
    // its ProjectConfig field is unset (see that function's doc comment for why) -- deliberately
    // diverging from the runtime, which loads nothing at all for an unset field. That divergence
    // is silent by design at the always-on error level (it exists specifically so a project with
    // no config, or a fixture, still gets checked), but it's a real, reportable authoring mistake
    // when it happens in a REAL project: a convention-path file left on disk without its matching
    // field set validates clean here while the runtime silently loads an empty/absent catalog.
    // Only fires when a .project.ron actually exists — a config-less project (every check above
    // this comment already treats that as the normal, expected fixture/bootstrap shape, not a
    // mistake) would otherwise light this up on every one of its convention-path files.
    if let Some(config) = project_config {
        for (field, convention_path, field_name) in [
            (config.asset_catalog.as_deref(), "assets.ron", "asset_catalog"),
            (config.prefab_catalog.as_deref(), "prefabs/prefabs.ron", "prefab_catalog"),
            (config.stats_path.as_deref(), "stats/stats.ron", "stats_path"),
            (config.items_path.as_deref(), "items/items.ron", "items_path"),
        ] {
            if field.is_none() && project_dir.join(convention_path).is_file() {
                warnings.push(StrictWarning {
                    source_file: find_project_ron(project_dir).unwrap_or_default(),
                    message: format!(
                        "{convention_path} exists but {field_name} is not set in .project.ron — \
                         the runtime will not load it, even though this validate run just checked \
                         it via the convention-path fallback"
                    ),
                    kind: "unset_catalog_path_with_convention_file",
                });
            }
        }
    }

    // Collect every key that appears on the "usage" side.
    let mut used_prefabs: HashSet<&str> = HashSet::new();
    let mut used_effects: HashSet<&str> = HashSet::new();
    let mut used_audio: HashSet<&str> = HashSet::new();
    let mut used_decals: HashSet<&str> = HashSet::new();

    for (_, scene) in scenes {
        for entity in &scene.entities {
            used_prefabs.insert(&entity.prefab);
        }
        for entry in scene.join_prefab_keys.iter().flatten() {
            used_prefabs.insert(entry);
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

        // A player prefab's jump can never ballistically clear its own ground-detection
        // sensor's combined reach (collider_radius + ground_cast_length) — the design-time
        // counterpart of the scene-load `warn!` in `scene_loader.rs::
        // warn_jump_cannot_clear_ground_sensor`. Not slope-specific: even on flat ground the
        // ground-check can then never truthfully report "ungrounded", so the jump only re-arms
        // via the bounded jump_air_grace fallback rather than a real landing — a real, working
        // fallback, not a broken feature, which is why this is a `--strict`-only warning rather
        // than a hard error (matching the runtime side, which is also a `warn!`, not a panic or
        // rejected spawn). See `planning/features/uphill_jump_lock.md`. Resolves the jump-height
        // target directly (mirroring `resolve_jump_velocity`'s height resolution) rather than
        // round-tripping through velocity — `apex == height` by construction when the runtime
        // derives velocity from a target height via `v = sqrt(2*g*h)`.
        let mut keys: Vec<&String> = catalog.prefabs.keys().collect();
        keys.sort();
        for key in keys {
            let def = &catalog.prefabs[key];
            if !def.components.tags.iter().any(|t| t == "player") { continue }
            let player_height = if def.kind == PrefabKind::Primitive {
                def.primitive.as_ref().and_then(|p| p.height).unwrap_or(1.8)
            } else {
                def.components.movement.collider_height.unwrap_or(1.8)
            };
            let collider_radius = if def.kind == PrefabKind::Primitive {
                def.primitive.as_ref().and_then(|p| p.radius).unwrap_or(0.4)
            } else {
                def.components.movement.collider_radius.unwrap_or(0.4)
            };
            let reach = collider_radius + def.components.movement.ground_cast_length;
            let resolve_height = |config: Option<&ironhold_core::schema::catalog::JumpConfig>| -> f32 {
                use ironhold_core::schema::catalog::JumpConfig;
                match config {
                    None => player_height,
                    Some(JumpConfig::Fixed { height }) => *height,
                    Some(JumpConfig::RelativeToHeight { percent }) => player_height * percent / 100.0,
                }
            };
            let mut checks = vec![("jump", resolve_height(def.components.movement.jump.as_ref()))];
            if def.components.movement.double_jump {
                checks.push(("double_jump_height", resolve_height(def.components.movement.double_jump_height.as_ref())));
            }
            for (field_name, apex) in checks {
                // `!(apex > reach)`, not `apex <= reach`: a negative/zero authored height (or a
                // negative `RelativeToHeight` percent) makes the resolved velocity NaN at
                // runtime, and `NaN <= reach` is false — silently missing the exact
                // misconfiguration this check exists to catch.
                if !(apex > reach) {
                    warnings.push(StrictWarning {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: `{}` gives a jump apex of {:.2}m, which does not \
                             clear this player's ground-check reach of {:.2}m (collider_radius \
                             {:.2}m + ground_cast_length {:.2}m) — the ground sensor may never \
                             report \"ungrounded\" even on flat ground. Raise `{}` (or \
                             `double_jump_height`) or lower `ground_cast_length`",
                            key, field_name, apex, reach, collider_radius,
                            def.components.movement.ground_cast_length, field_name
                        ),
                        kind: "jump_cannot_clear_ground_sensor",
                    });
                }
            }

            // `max_walkable_slope_deg` outside a sane range silently breaks grounding entirely —
            // a value at or below 0 means no surface is ever walkable, and a player can then only
            // jump if `double_jump` is enabled (the grounded branch of `can_jump` never applies).
            // A value above 90 makes every surface (however overhanging) count as walkable, which
            // is likely not intended either. `90.0` itself is meaningful and valid — it's the
            // "disable this check, fall back to proximity-only grounding" escape hatch, matching
            // this project's pre-fix behavior — so the valid range is `(0.0, 90.0]`, not `(0.0,
            // 90.0)`. See `MovementConfig.max_walkable_slope_deg`'s doc comment.
            let slope_limit = def.components.movement.max_walkable_slope_deg;
            if !(slope_limit > 0.0 && slope_limit <= 90.0) {
                warnings.push(StrictWarning {
                    source_file: "prefabs/prefabs.ron".to_string(),
                    message: format!(
                        "prefab {:?}: `max_walkable_slope_deg` is {:.2}, outside the valid \
                         (0, 90] range — a value at or below 0 means no surface is ever walkable \
                         (jump only works via double_jump, if enabled at all); above 90 makes \
                         every surface, however overhanging, count as walkable",
                        key, slope_limit
                    ),
                    kind: "invalid_walkable_slope_limit",
                });
            }

            // Unlike `max_walkable_slope_deg`, `coyote_time_secs` has no invalid range that breaks
            // grounding outright — any non-negative value just makes the debounce buffer bigger or
            // smaller (it does have a practical, jump-height-dependent upper bound where a large
            // enough value can mask an entire jump's animation, but that's not checked here — see
            // `planning/claude_suggestions.md`). A negative value is the one case worth flagging
            // unconditionally: it silently launders to a zero-tick buffer (same as `0.0`) rather
            // than doing anything with the negative value, which is far more likely a sign-flip
            // typo than an intentional way to spell "disabled".
            let coyote_time_secs = def.components.movement.coyote_time_secs;
            if coyote_time_secs < 0.0 {
                warnings.push(StrictWarning {
                    source_file: "prefabs/prefabs.ron".to_string(),
                    message: format!(
                        "prefab {:?}: `coyote_time_secs` is {:.3}, which is negative — this \
                         silently disables the coyote-time buffer entirely (same as `0.0`) rather \
                         than doing anything with the negative value. If you meant to disable it, \
                         use `0.0` instead",
                        key, coyote_time_secs
                    ),
                    kind: "negative_coyote_time_secs",
                });
            }
        }
    }

    // `label_depth_scale.reference_distance` far outside the scene's reachable player-camera
    // radius range — design-time counterpart of the scene-load `warn!` in `scene_loader.rs::
    // warn_label_depth_scale_reference_distance`. `--strict`-only (not a hard error, unlike
    // `min_scale` above): this is a heuristic band, not a provable misconfiguration — the CLI
    // can't prove scaling never engages, only that it's unlikely to at any camera distance this
    // scene's cameras can actually reach. See `planning/features/label_depth_scale_validation.md`.
    for (scene_path, scene) in scenes {
        let Some(cfg) = &scene.label_depth_scale else { continue };

        let mut overall_min = f32::INFINITY;
        let mut overall_max = f32::NEG_INFINITY;
        let mut widen = |range: Option<(f32, f32)>| {
            if let Some((min_r, max_r)) = range {
                overall_min = overall_min.min(min_r);
                overall_max = overall_max.max(max_r);
            }
        };

        // A `tags: ["flycam"]` entity suppresses every player camera entirely
        // (`SuppressPlayerCameras`, spectator mode) — a scene combining a flycam with
        // `label_depth_scale` (e.g. `custom_materials`) has no player camera to compare
        // `reference_distance` against, so player/join_prefab_keys collection is skipped when one
        // is present. Mirrors the runtime's identical `has_flycam` guard in `scene_loader.rs`.
        let has_flycam = prefab_catalog.is_some_and(|catalog| {
            scene.entities.iter().any(|e| {
                catalog.prefabs.get(&e.prefab).is_some_and(|p| p.is_flycam())
            })
        });
        if let (Some(catalog), false) = (prefab_catalog, has_flycam) {
            let mut widen_prefab_if_player = |prefab_key: &str| {
                let Some(prefab) = catalog.prefabs.get(prefab_key) else { return };
                if !prefab.components.tags.iter().any(|t| t == "player") { return }
                match &prefab.components.camera_mode {
                    Some(mode) => widen(mode.radius_range()),
                    None => {
                        let c = prefab.components.camera.clone().unwrap_or_else(default_camera_config);
                        widen(Some((c.min_radius, c.max_radius)));
                    }
                }
            };
            for entity_def in &scene.entities {
                widen_prefab_if_player(&entity_def.prefab);
            }
            // Local-coop character-select variants (`join_prefab_keys`) are player-tagged
            // prefabs too, reachable independently of `scene.entities` — omitting them would
            // narrow the band and risk a false positive, the opposite direction of every other
            // tradeoff this check makes.
            for key in scene.join_prefab_keys.iter().flatten() {
                widen_prefab_if_player(key);
            }
            // A player is frequently *not* scene-placed — `3rd_person_game_demo`'s own player is
            // spawned entirely via `Action::Spawn` in `state_machine.ron`'s entry_actions, never
            // appearing in `scene.entities` at all. Without this, the flagship scene this feature
            // was written to protect would silently never trigger the check. Project-wide, not
            // scene-scoped (an action isn't reliably attributable to one scene file) — same
            // documented tradeoff as the `SetCameraMode` registry check above: weaker than a
            // scene-scoped check, but catches the dominant case, and erring toward more reachable
            // cameras only ever widens the acceptable band (fewer false positives), never narrows it.
            for (_, action) in actions {
                if let Action::Spawn { prefab, .. } = action {
                    widen_prefab_if_player(prefab);
                }
            }
        }
        for mode in scene.camera_modes.values() {
            widen(mode.radius_range());
        }

        if !overall_min.is_finite() || !overall_max.is_finite() {
            // No radius-bearing camera reachable from this scene (every camera is
            // Fixed/FirstPerson/Flycam, there's a flycam suppressing player cameras, or there
            // are no player prefabs at all) — no meaningful range to compare against; a false
            // warning here would be worse than no check.
            continue;
        }
        let rd = cfg.reference_distance;
        if !rd.is_finite() {
            warnings.push(StrictWarning {
                source_file: scene_path.clone(),
                message: format!(
                    "label_depth_scale.reference_distance is {} (not a finite number) — depth \
                     scaling will never engage",
                    rd
                ),
                kind: "label_depth_scale_reference_distance_outside_camera_range",
            });
        } else if rd < overall_min * 0.5 || rd > overall_max * 2.0 {
            let suggested = (overall_min + overall_max) / 2.0;
            warnings.push(StrictWarning {
                source_file: scene_path.clone(),
                message: format!(
                    "label_depth_scale.reference_distance is {:.1}, outside this scene's typical \
                     camera zoom range ({:.1}-{:.1}) — depth scaling may never visibly engage, or \
                     may engage immediately at max zoom-out. Try ~{:.1} (the range midpoint), \
                     then confirm in-browser",
                    rd, overall_min, overall_max, suggested
                ),
                kind: "label_depth_scale_reference_distance_outside_camera_range",
            });
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

    // Parse-failure protection, both directions: a malformed rules.ron/state_machine.ron/behavior
    // file already reports its own parse error, and would flood this check with secondary noise
    // once its rules/transitions/bindings are entirely absent from the two data sets below (same
    // reasoning as `check_ui_trigger_reachability`'s own gate). But unlike that forward check, an
    // unparseable SCENE is an equally real risk here: `do_validate` silently drops any scene that
    // fails to parse from the `scenes` list, shrinking `collect_reachable_ui_triggers`'s reachable
    // set -- so a live rule handling that scene's own (now-invisible) button could be wrongly
    // flagged orphaned. `orphan_rule_prereqs_clean` is true only when BOTH logic files and every
    // scene parsed cleanly.
    if orphan_rule_prereqs_clean {
        let reachable = collect_reachable_ui_triggers(project_config, scenes, prefab_catalog);
        warnings.extend(check_orphan_ui_rules(&reachable, rules, state_machine, behaviors));
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

    let project_config: Option<ProjectConfig> = find_project_ron(project_dir)
        .and_then(|name| try_parse(project_dir, &name, &mut file_results));

    // asset_catalog/prefab_catalog/stats_path/items_path are all ProjectConfig-configured paths
    // (see load_configured_catalog's doc comment) -- every shipped project happens to set the
    // first two to the "assets.ron"/"prefabs/prefabs.ron" convention path, which is why hardcoding
    // those two literals here never surfaced as a bug: a project that relocates either (or omits
    // it, same as a project legitimately omitting stats_path/items_path today) would previously
    // have silently lost every check depending on that catalog, exactly the items_path gap this
    // helper was originally written to close.
    let asset_catalog: Option<AssetCatalog> = load_configured_catalog(
        project_dir,
        project_config.as_ref().and_then(|c| c.asset_catalog.as_deref()),
        "assets.ron",
        "asset_catalog",
        &mut file_results,
    );

    let prefab_catalog: Option<PrefabCatalog> = load_configured_catalog(
        project_dir,
        project_config.as_ref().and_then(|c| c.prefab_catalog.as_deref()),
        "prefabs/prefabs.ron",
        "prefab_catalog",
        &mut file_results,
    );

    let stat_catalog: Option<StatCatalog> = load_configured_catalog(
        project_dir,
        project_config.as_ref().and_then(|c| c.stats_path.as_deref()),
        "stats/stats.ron",
        "stats_path",
        &mut file_results,
    );

    let item_catalog: Option<ItemCatalog> = load_configured_catalog(
        project_dir,
        project_config.as_ref().and_then(|c| c.items_path.as_deref()),
        "items/items.ron",
        "items_path",
        &mut file_results,
    );

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

    let mut dialogues: Vec<(String, DialogueDef)> = Vec::new();
    for path in glob_dir(project_dir, "dialogues", ".dialogue.ron") {
        let r = rel(project_dir, &path);
        if let Some(d) = parse_file::<DialogueDef>(&path, &r, &mut file_results) {
            dialogues.push((r, d));
        }
    }

    let _model_fixes: Option<ModelFixesAsset> =
        try_parse(project_dir, "overrides/model_fixes.ron", &mut file_results);

    let all_actions = collect_actions(
        rules.as_ref().map(|r| ("logic/rules.ron", r)),
        state_machine.as_ref().map(|s| ("logic/state_machine.ron", s)),
        &behaviors,
        &dialogues,
    );

    let mut cross_errors = cross_file_checks(
        project_dir,
        project_config.as_ref(),
        asset_catalog.as_ref(),
        prefab_catalog.as_ref(),
        stat_catalog.as_ref(),
        item_catalog.as_ref(),
        &scenes,
        &all_actions,
    );
    let logic_files_parsed_cleanly = file_results
        .iter()
        .filter(|r| {
            r.rel_path == "logic/rules.ron"
                || r.rel_path == "logic/state_machine.ron"
                || r.rel_path.starts_with("behaviors/")
        })
        .all(|r| r.is_ok());
    let scenes_parsed_cleanly = file_results
        .iter()
        .filter(|r| r.rel_path.starts_with("scenes/"))
        .all(|r| r.is_ok());
    cross_errors.extend(check_ui_trigger_reachability(
        project_dir,
        project_config.as_ref(),
        &scenes,
        rules.as_ref(),
        state_machine.as_ref(),
        &behaviors,
        logic_files_parsed_cleanly,
    ));

    let strict_warnings = if strict {
        strict_checks(
            project_dir,
            project_config.as_ref(),
            asset_catalog.as_ref(),
            prefab_catalog.as_ref(),
            &scenes,
            &all_actions,
            rules.as_ref().map(|r| ("logic/rules.ron", r)),
            state_machine.as_ref().map(|s| ("logic/state_machine.ron", s)),
            &behaviors,
            logic_files_parsed_cleanly && scenes_parsed_cleanly,
        )
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
