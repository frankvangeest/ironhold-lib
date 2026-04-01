use bevy::prelude::*;
use crate::ProjectRoot;
use crate::schema::*;
use crate::schema::scene_v2::GameSceneV2;
use crate::schema::catalog::{AssetCatalog, PrefabCatalog};
use crate::schema::player::InputMap;
use crate::runtime::messages::*;
use super::{
    MergedModelFixes, LoadedRules, LoadedKeyBindings, LoadedAssetCatalog, LoadedPrefabCatalog,
    PendingProjectLoads, SceneHandleV2, resolve_project_path,
};

pub fn check_project_loaded(
    mut commands: Commands,
    config_handle: Res<ProjectConfigHandle>,
    configs: Res<Assets<ProjectConfig>>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
    mut scene_events: MessageWriter<SceneEvent>,
    project_root: Res<ProjectRoot>,
    pending: Option<Res<PendingProjectLoads>>,
    model_fixes_assets: Res<Assets<ModelFixesAsset>>,
    rules_assets: Res<Assets<LogicRulesAsset>>,
    asset_catalog_assets: Res<Assets<AssetCatalog>>,
    prefab_catalog_assets: Res<Assets<PrefabCatalog>>,
) {
    let Some(config) = configs.get(&config_handle.0) else { return; };

    if let Err(e) = config.validate() {
        panic!("Invalid ProjectConfig: {}", e);
    }

    // Phase 1: kick off all external file loads on the first frame the project config is ready.
    if pending.is_none() {
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

        let any_pending = model_fixes_handle.is_some()
            || rules_handle.is_some()
            || asset_catalog_handle.is_some()
            || prefab_catalog_handle.is_some();
        commands.insert_resource(PendingProjectLoads {
            model_fixes: model_fixes_handle,
            rules: rules_handle,
            asset_catalog: asset_catalog_handle,
            prefab_catalog: prefab_catalog_handle,
        });

        if any_pending {
            return; // Wait for next frame.
        }

        // No external files — store inline data and proceed.
        commands.insert_resource(MergedModelFixes(config.model_fixes.clone()));
        commands.insert_resource(LoadedRules(config.rules.clone()));
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
            commands.insert_resource(LoadedKeyBindings(key_bindings));
        }
        commands.insert_resource(LoadedAssetCatalog(AssetCatalog::default()));
        commands.insert_resource(LoadedPrefabCatalog(PrefabCatalog::default()));
    } else {
        // Phase 2: wait for all pending loads to complete.
        let pending = pending.unwrap();

        if let Some(h) = &pending.model_fixes {
            match asset_server.load_state(h) {
                bevy::asset::LoadState::Loaded => {}
                bevy::asset::LoadState::Failed(_) => {
                    warn!("model_fixes failed to load — proceeding without it");
                }
                _ => { return; }
            }
        }
        if let Some(h) = &pending.rules {
            match asset_server.load_state(h) {
                bevy::asset::LoadState::Loaded => {}
                bevy::asset::LoadState::Failed(_) => {
                    warn!("rules failed to load — proceeding without it");
                }
                _ => { return; }
            }
        }
        if let Some(h) = &pending.asset_catalog {
            match asset_server.load_state(h) {
                bevy::asset::LoadState::Loaded => {}
                bevy::asset::LoadState::Failed(_) => {
                    warn!("asset catalog failed to load — models/materials will be missing");
                }
                _ => { return; }
            }
        }
        if let Some(h) = &pending.prefab_catalog {
            match asset_server.load_state(h) {
                bevy::asset::LoadState::Loaded => {}
                bevy::asset::LoadState::Failed(_) => {
                    warn!("prefab catalog failed to load — entities will not spawn");
                }
                _ => { return; }
            }
        }

        // Phase 3: merge and store results.
        let mut merged_fixes = config.model_fixes.clone();
        if let Some(h) = &pending.model_fixes {
            if let Some(fixes_asset) = model_fixes_assets.get(h) {
                merged_fixes.extend(
                    fixes_asset.model_fixes.iter().map(|(k, v)| (k.clone(), v.clone())),
                );
            }
        }
        commands.insert_resource(MergedModelFixes(merged_fixes));

        let rules = if let Some(h) = &pending.rules {
            rules_assets.get(h).map(|a| a.rules.clone()).unwrap_or_default()
        } else {
            config.rules.clone()
        };
        commands.insert_resource(LoadedRules(rules));

        let key_bindings = config.global_key_bindings.clone();
        for key_name in key_bindings.keys() {
            if InputMap::parse_key(key_name).is_none() {
                warn!(
                    "global_key_bindings: unrecognised key name {:?} — binding will have no effect",
                    key_name
                );
            }
        }
        commands.insert_resource(LoadedKeyBindings(key_bindings));

        let asset_catalog = if let Some(h) = &pending.asset_catalog {
            asset_catalog_assets.get(h).cloned().unwrap_or_default()
        } else {
            AssetCatalog::default()
        };
        commands.insert_resource(LoadedAssetCatalog(asset_catalog));

        let prefab_catalog = if let Some(h) = &pending.prefab_catalog {
            prefab_catalog_assets.get(h).cloned().unwrap_or_default()
        } else {
            PrefabCatalog::default()
        };
        commands.insert_resource(LoadedPrefabCatalog(prefab_catalog));
    }

    let scene_path = resolve_project_path(&project_root.0, &config.initial_scene);
    info!(
        "Project Config Loaded (schema v{}). Initial Scene: {}",
        config.schema_version, scene_path
    );

    if scene_path.ends_with(".scene.ron") {
        let scene_handle: Handle<GameSceneV2> = asset_server.load(scene_path.clone());
        commands.insert_resource(SceneHandleV2(scene_handle));
    } else {
        let scene_handle = asset_server.load(scene_path.clone());
        commands.insert_resource(LevelHandle(scene_handle));
    }
    scene_events.write(SceneEvent::Requested(scene_path));
    next_state.set(AppState::LoadingScene);
}
