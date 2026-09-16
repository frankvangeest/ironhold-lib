use bevy::prelude::*;
use crate::ProjectRoot;
use crate::schema::*;
use crate::schema::project::StateMachineAsset;
use crate::schema::scene_v2::GameSceneV2;
use crate::schema::catalog::{AssetCatalog, PrefabCatalog};
use crate::schema::stats::{StatCatalog, LiveStat, LoadedStats, LoadedModifiers};
use crate::schema::items::ItemCatalog;
use crate::capabilities::inventory::LoadedItemCatalog;
use crate::schema::player::InputMap;
use crate::runtime::messages::*;
use super::{
    MergedModelFixes, LoadedRules, LoadedStateMachine, LoadedKeyBindings, ProjectKeyBindings,
    LoadedGamepadBindings, ProjectGamepadBindings,
    LoadedAssetCatalog, LoadedPrefabCatalog, PendingProjectLoads, SceneHandleV2,
    LogicState, AudioState, resolve_project_path,
};

/// Bundled `SystemParam` for the pending-catalog-asset `LoadState` checks in
/// `check_project_loaded` — grouped here (rather than 7 bare params) to stay within Bevy's
/// 16-param limit once `Time<Virtual>` was added for `ProjectConfig.max_frame_delta_secs`. Same
/// pattern as `SpawnParams`/`SceneV2Params` in `runtime/scene_manager/mod.rs`.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PendingCatalogAssets<'w> {
    pub model_fixes: Res<'w, Assets<ModelFixesAsset>>,
    pub rules: Res<'w, Assets<LogicRulesAsset>>,
    pub state_machine: Res<'w, Assets<StateMachineAsset>>,
    pub asset_catalog: Res<'w, Assets<AssetCatalog>>,
    pub prefab_catalog: Res<'w, Assets<PrefabCatalog>>,
    pub stat_catalog: Res<'w, Assets<StatCatalog>>,
    pub item_catalog: Res<'w, Assets<ItemCatalog>>,
}

pub fn check_project_loaded(
    mut commands: Commands,
    config_handle: Res<ProjectConfigHandle>,
    configs: Res<Assets<ProjectConfig>>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
    mut scene_events: MessageWriter<SceneEvent>,
    project_root: Res<ProjectRoot>,
    pending: Option<Res<PendingProjectLoads>>,
    pending_assets: PendingCatalogAssets,
    scene_override: Option<Res<crate::InitialSceneOverride>>,
    mut time_virtual: ResMut<Time<Virtual>>,
) {
    let Some(config) = configs.get(&config_handle.0) else { return; };

    // Phase 1: kick off all external file loads on the first frame the project config is ready.
    if pending.is_none() {
        if let Err(e) = config.validate() {
            error!("Invalid ProjectConfig: {} — scene loading may behave incorrectly", e);
        }
        let model_fixes_handle = config.model_fixes_path.as_ref().map(|p| {
            let resolved = resolve_project_path(&project_root.0, p);
            info!("Loading external model fixes from: {}", resolved);
            asset_server.load::<ModelFixesAsset>(resolved)
        });
        let rules_handle = config.rules_path.as_ref().map(|p| {
            let resolved = resolve_project_path(&project_root.0, p);
            info!("Loading external rules from: {}", resolved);
            asset_server.load::<LogicRulesAsset>(resolved)
        });
        let state_machine_handle = config.state_machine_path.as_ref().map(|p| {
            let resolved = resolve_project_path(&project_root.0, p);
            info!("Loading state machine from: {}", resolved);
            asset_server.load::<StateMachineAsset>(resolved)
        });
        if config.rules_path.is_some() && config.state_machine_path.is_some() {
            warn!(
                "Project has both rules_path and state_machine_path set; \
                 rules.ron is NOT loaded when state_machine_path is present — remove rules_path to silence this"
            );
        }
        let asset_catalog_handle = config.asset_catalog.as_ref().map(|p| {
            let resolved = resolve_project_path(&project_root.0, p);
            info!("Loading asset catalog from: {}", resolved);
            asset_server.load::<AssetCatalog>(resolved)
        });
        let prefab_catalog_handle = config.prefab_catalog.as_ref().map(|p| {
            let resolved = resolve_project_path(&project_root.0, p);
            info!("Loading prefab catalog from: {}", resolved);
            asset_server.load::<PrefabCatalog>(resolved)
        });

        let stats_handle = config.stats_path.as_ref().map(|p| {
            let resolved = resolve_project_path(&project_root.0, p);
            info!("Loading stats catalog from: {}", resolved);
            asset_server.load::<StatCatalog>(resolved)
        });

        let items_handle = config.items_path.as_ref().map(|p| {
            let resolved = resolve_project_path(&project_root.0, p);
            info!("Loading item catalog from: {}", resolved);
            asset_server.load::<ItemCatalog>(resolved)
        });

        let any_pending = model_fixes_handle.is_some()
            || rules_handle.is_some()
            || state_machine_handle.is_some()
            || asset_catalog_handle.is_some()
            || prefab_catalog_handle.is_some()
            || stats_handle.is_some()
            || items_handle.is_some();
        commands.insert_resource(PendingProjectLoads {
            model_fixes: model_fixes_handle,
            rules: rules_handle,
            state_machine: state_machine_handle,
            asset_catalog: asset_catalog_handle,
            prefab_catalog: prefab_catalog_handle,
            stats: stats_handle,
            items: items_handle,
        });

        if any_pending {
            return; // Wait for next frame.
        }

        // No external files — store inline data and proceed.
        commands.insert_resource(MergedModelFixes(config.model_fixes.clone()));
        commands.insert_resource(LoadedRules(config.rules.clone()));
        commands.insert_resource(LoadedStateMachine(None));
        commands.insert_resource(LoadedItemCatalog(None));
        commands.insert_resource(LoadedStats::default());
        commands.insert_resource(LoadedModifiers::default());
        {
            let key_bindings = config.global_key_bindings.clone();
            for key_name in key_bindings.keys() {
                if InputMap::parse_key(key_name).is_none() {
                    warn!(
                        "global_key_bindings: unrecognised key name {:?} — binding will have no effect",
                        key_name
                    );
                }
            }
            commands.insert_resource(ProjectKeyBindings(key_bindings.clone()));
            commands.insert_resource(LoadedKeyBindings(key_bindings));
        }
        {
            let gamepad_bindings = config.global_unclaimed_gamepad_bindings.clone();
            for button_name in gamepad_bindings.keys() {
                if InputMap::parse_gamepad_button(button_name).is_none() {
                    warn!(
                        "global_unclaimed_gamepad_bindings: unrecognised button name {:?} — binding will have no effect",
                        button_name
                    );
                }
            }
            commands.insert_resource(ProjectGamepadBindings(gamepad_bindings.clone()));
            commands.insert_resource(LoadedGamepadBindings(gamepad_bindings));
        }
        commands.insert_resource(LoadedAssetCatalog(AssetCatalog::default()));
        commands.insert_resource(LoadedPrefabCatalog(PrefabCatalog::default()));
        commands.insert_resource(AudioState {
            max_volume: config.audio.max_volume,
            active_fraction: 1.0,
            muted: config.audio.mute_on_start,
        });
    } else {
        // Phase 2: wait for all pending loads to complete.
        let Some(pending) = pending else { return; };

        if let Some(h) = &pending.model_fixes {
            match asset_server.load_state(h) {
                bevy::asset::LoadState::Loaded => {}
                bevy::asset::LoadState::Failed(e) => {
                    let path = asset_server.get_path(h)
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    error!("model_fixes failed to load: {} — {} — proceeding without it", path, e);
                }
                _ => { return; }
            }
        }
        if let Some(h) = &pending.rules {
            match asset_server.load_state(h) {
                bevy::asset::LoadState::Loaded => {}
                bevy::asset::LoadState::Failed(e) => {
                    let path = asset_server.get_path(h)
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    error!("rules failed to load: {} — {} — proceeding without it (every rule in this file is now inactive)", path, e);
                }
                _ => { return; }
            }
        }
        if let Some(h) = &pending.state_machine {
            match asset_server.load_state(h) {
                bevy::asset::LoadState::Loaded => {}
                bevy::asset::LoadState::Failed(e) => {
                    let path = asset_server.get_path(h)
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    error!("state machine failed to load: {} — {} — proceeding without it (every state transition in this file is now inactive)", path, e);
                }
                _ => { return; }
            }
        }
        if let Some(h) = &pending.asset_catalog {
            match asset_server.load_state(h) {
                bevy::asset::LoadState::Loaded => {}
                bevy::asset::LoadState::Failed(e) => {
                    let path = asset_server.get_path(h)
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    error!("Asset catalog failed to load: {} — {} — proceeding with empty catalog", path, e);
                }
                _ => { return; }
            }
        }
        if let Some(h) = &pending.prefab_catalog {
            match asset_server.load_state(h) {
                bevy::asset::LoadState::Loaded => {}
                bevy::asset::LoadState::Failed(e) => {
                    let path = asset_server.get_path(h)
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    error!("Prefab catalog failed to load: {} — {} — proceeding with empty catalog", path, e);
                }
                _ => { return; }
            }
        }
        if let Some(h) = &pending.stats {
            match asset_server.load_state(h) {
                bevy::asset::LoadState::Loaded => {}
                bevy::asset::LoadState::Failed(e) => {
                    let path = asset_server.get_path(h)
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    error!("Stats catalog failed to load: {} — {} — proceeding with no stats", path, e);
                }
                _ => { return; }
            }
        }
        if let Some(h) = &pending.items {
            match asset_server.load_state(h) {
                bevy::asset::LoadState::Loaded => {}
                bevy::asset::LoadState::Failed(e) => {
                    let path = asset_server.get_path(h)
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    error!("Item catalog failed to load: {} — {} — proceeding with no items", path, e);
                }
                _ => { return; }
            }
        }

        // Phase 3: merge and store results.
        let mut merged_fixes = config.model_fixes.clone();
        if let Some(h) = &pending.model_fixes {
            if let Some(fixes_asset) = pending_assets.model_fixes.get(h) {
                merged_fixes.extend(
                    fixes_asset.model_fixes.iter().map(|(k, v)| (k.clone(), v.clone())),
                );
            }
        }
        commands.insert_resource(MergedModelFixes(merged_fixes));

        let rules = if let Some(h) = &pending.rules {
            pending_assets.rules.get(h).map(|a| a.rules.clone()).unwrap_or_default()
        } else {
            config.rules.clone()
        };
        commands.insert_resource(LoadedRules(rules));

        let fsm = pending.state_machine.as_ref()
            .and_then(|h| pending_assets.state_machine.get(h))
            .cloned();
        if let Some(ref machine) = fsm {
            if let Err(e) = machine.validate() {
                error!("Invalid StateMachineAsset: {} — state machine transitions may be unreliable", e);
            }
            info!(
                "State machine loaded: initial_state=\"{}\", {} states, {} transitions",
                machine.initial_state,
                machine.states.len(),
                machine.transitions.len(),
            );
            commands.insert_resource(LogicState(machine.initial_state.clone()));
        }
        commands.insert_resource(LoadedStateMachine(fsm));

        let key_bindings = config.global_key_bindings.clone();
        for key_name in key_bindings.keys() {
            if InputMap::parse_key(key_name).is_none() {
                warn!(
                    "global_key_bindings: unrecognised key name {:?} — binding will have no effect",
                    key_name
                );
            }
        }
        commands.insert_resource(ProjectKeyBindings(key_bindings.clone()));
        commands.insert_resource(LoadedKeyBindings(key_bindings));

        let gamepad_bindings = config.global_unclaimed_gamepad_bindings.clone();
        for button_name in gamepad_bindings.keys() {
            if InputMap::parse_gamepad_button(button_name).is_none() {
                warn!(
                    "global_unclaimed_gamepad_bindings: unrecognised button name {:?} — binding will have no effect",
                    button_name
                );
            }
        }
        commands.insert_resource(ProjectGamepadBindings(gamepad_bindings.clone()));
        commands.insert_resource(LoadedGamepadBindings(gamepad_bindings));

        let asset_catalog = if let Some(h) = &pending.asset_catalog {
            pending_assets.asset_catalog.get(h).cloned().unwrap_or_default()
        } else {
            AssetCatalog::default()
        };
        if let Err(e) = asset_catalog.validate() {
            error!("Invalid AssetCatalog: {} — catalog entries may not resolve correctly", e);
        }
        commands.insert_resource(LoadedAssetCatalog(asset_catalog));

        let prefab_catalog = if let Some(h) = &pending.prefab_catalog {
            pending_assets.prefab_catalog.get(h).cloned().unwrap_or_default()
        } else {
            PrefabCatalog::default()
        };
        if let Err(e) = prefab_catalog.validate() {
            error!("Invalid PrefabCatalog: {} — prefab spawning may fail", e);
        }
        commands.insert_resource(LoadedPrefabCatalog(prefab_catalog));

        let (loaded_stats, loaded_modifiers) = if let Some(h) = &pending.stats {
            if let Some(catalog) = pending_assets.stat_catalog.get(h) {
                if let Err(e) = catalog.validate() {
                    error!("Invalid StatCatalog: {} — stat system may not behave correctly", e);
                }
                let map = catalog.stats.iter()
                    .map(|(key, def)| (key.clone(), LiveStat::new(def.clone())))
                    .collect();
                info!(
                    "Stats loaded: {} stat(s), {} modifier(s) defined",
                    catalog.stats.len(),
                    catalog.modifiers.len(),
                );
                (LoadedStats(map), LoadedModifiers(catalog.modifiers.clone()))
            } else {
                (LoadedStats::default(), LoadedModifiers::default())
            }
        } else {
            (LoadedStats::default(), LoadedModifiers::default())
        };
        commands.insert_resource(loaded_stats);
        commands.insert_resource(loaded_modifiers);

        let loaded_item_catalog = if let Some(h) = &pending.items {
            if let Some(catalog) = pending_assets.item_catalog.get(h) {
                if let Err(e) = catalog.validate() {
                    error!("Invalid ItemCatalog: {} — item system may not behave correctly", e);
                }
                info!("Item catalog loaded: {} item(s) defined", catalog.items.len());
                LoadedItemCatalog(Some(catalog.clone()))
            } else {
                LoadedItemCatalog(None)
            }
        } else {
            LoadedItemCatalog(None)
        };
        commands.insert_resource(loaded_item_catalog);

        commands.insert_resource(AudioState {
            max_volume: config.audio.max_volume,
            active_fraction: 1.0,
            muted: config.audio.mute_on_start,
        });
    }

    // Runs once per project load (this is the shared tail both phase-1 and phase-2
    // config-application paths above fall through to, and exactly one ProjectConfig loads per
    // app lifetime today), so this can't double-apply or race a later scene load.
    if let Some(secs) = config.max_frame_delta_secs {
        apply_max_frame_delta(&mut time_virtual, secs);
    }

    let initial = scene_override
        .as_deref()
        .map(|r| r.0.as_str())
        .unwrap_or(&config.initial_scene);
    let scene_path = resolve_project_path(&project_root.0, initial);
    info!(
        "Project Config Loaded (schema v{}). Initial Scene: {}",
        config.schema_version, scene_path
    );

    let scene_handle: Handle<GameSceneV2> = asset_server.load(scene_path.clone());
    commands.insert_resource(SceneHandleV2(scene_handle));
    scene_events.write(SceneEvent::Requested(scene_path));
    next_state.set(AppState::LoadingScene);
}

/// Applies `ProjectConfig.max_frame_delta_secs` to a `Time<Virtual>` — see that field's doc
/// comment (`schema/project.rs`) for the full rationale. Pure logic (no ECS access beyond the
/// passed-in resource reference), so it's directly unit-testable without spinning up an app.
/// Only called when the config has `Some(secs)` — see the call site — so `time_virtual` is never
/// touched (and never marked changed) on the common "field omitted" path.
///
/// `ironhold_cli validate` (`ProjectConfig::validate()`) already rejects a value outside
/// `[MIN_MAX_FRAME_DELTA_SECS, MAX_MAX_FRAME_DELTA_SECS]` at author time; this mirrors the same
/// bound as a runtime-side belt-and-braces guard for a config that skipped validation (e.g.
/// constructed in a test, or a future non-CLI authoring path). Two distinct panics are guarded
/// against here, not just "out of range": `Duration::from_secs_f32` panics above ~1.8e19s, and
/// separately rounds anything below ~5e-10s to `Duration::ZERO`, which Bevy's own
/// `Time::<Virtual>::set_max_delta` refuses via an internal assert. `try_from_secs_f32` plus an
/// explicit zero-check catches both without relying on the caller having validated first.
fn apply_max_frame_delta(time_virtual: &mut Time<Virtual>, secs: f32) {
    match std::time::Duration::try_from_secs_f32(secs) {
        Ok(duration) if !duration.is_zero() => {
            time_virtual.set_max_delta(duration);
            info!("max_frame_delta_secs override applied: {secs}s (Bevy default: 0.25s)");
        }
        _ => {
            warn!(
                "max_frame_delta_secs must be a positive, finite number of seconds that doesn't \
                 round to zero, got {secs} — ignoring, using Bevy's default (0.25s)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::project::{MIN_MAX_FRAME_DELTA_SECS, MAX_MAX_FRAME_DELTA_SECS};

    #[test]
    fn valid_value_sets_max_delta() {
        let mut time = Time::<Virtual>::default();
        apply_max_frame_delta(&mut time, 0.1);
        assert_eq!(time.max_delta(), std::time::Duration::from_secs_f32(0.1));
    }

    #[test]
    fn non_positive_value_is_ignored() {
        let mut time = Time::<Virtual>::default();
        let default_max_delta = time.max_delta();
        apply_max_frame_delta(&mut time, -1.0);
        assert_eq!(time.max_delta(), default_max_delta, "a negative value must not change max_delta");
        apply_max_frame_delta(&mut time, 0.0);
        assert_eq!(time.max_delta(), default_max_delta, "zero must not change max_delta");
    }

    #[test]
    fn non_finite_value_is_ignored() {
        let mut time = Time::<Virtual>::default();
        let default_max_delta = time.max_delta();
        apply_max_frame_delta(&mut time, f32::NAN);
        assert_eq!(time.max_delta(), default_max_delta);
        apply_max_frame_delta(&mut time, f32::INFINITY);
        assert_eq!(time.max_delta(), default_max_delta);
    }

    /// F1 (system-architect/debug-detective): `Duration::from_secs_f32` panics above ~1.8e19s —
    /// must not reach that call unguarded.
    #[test]
    fn overflow_value_does_not_panic_and_is_ignored() {
        let mut time = Time::<Virtual>::default();
        let default_max_delta = time.max_delta();
        apply_max_frame_delta(&mut time, 1e20);
        assert_eq!(time.max_delta(), default_max_delta, "an overflowing value must not change max_delta");
    }

    /// F2 (debug-detective): values below ~5e-10s round to `Duration::ZERO` via
    /// `from_secs_f32`/`try_from_secs_f32`, and Bevy's `set_max_delta` asserts `!= ZERO` —
    /// must not reach that call unguarded.
    #[test]
    fn zero_rounding_value_does_not_panic_and_is_ignored() {
        let mut time = Time::<Virtual>::default();
        let default_max_delta = time.max_delta();
        apply_max_frame_delta(&mut time, 1e-10);
        assert_eq!(time.max_delta(), default_max_delta, "a value that rounds to zero must not change max_delta");
    }

    #[test]
    fn schema_validate_bounds_agree_with_runtime_guard() {
        let mut time = Time::<Virtual>::default();
        apply_max_frame_delta(&mut time, MIN_MAX_FRAME_DELTA_SECS);
        assert_eq!(time.max_delta(), std::time::Duration::from_secs_f32(MIN_MAX_FRAME_DELTA_SECS));

        let mut time = Time::<Virtual>::default();
        apply_max_frame_delta(&mut time, MAX_MAX_FRAME_DELTA_SECS);
        assert_eq!(time.max_delta(), std::time::Duration::from_secs_f32(MAX_MAX_FRAME_DELTA_SECS));
    }
}
