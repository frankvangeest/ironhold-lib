use bevy::prelude::*;
use crate::ProjectRoot;
use crate::schema::*;
use crate::schema::scene_v2::GameSceneV2;
use crate::runtime::messages::*;
use crate::runtime::actions::ActionQueue;
use crate::capabilities::animation_resolver::AnimationRequests;
use super::{
    BackgroundMusic, LoadedAssetCatalog, OverlayEntity, PendingSceneLoadMode,
    SceneHandleV2, SceneStateParams, SpawnParams, SpawnId, resolve_project_path,
};

pub fn action_executor_system(
    mut commands: Commands,
    mut action_queue: ResMut<ActionQueue>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
    mut scene_events: MessageWriter<SceneEvent>,
    mut animation_requests: Query<(&mut AnimationRequests, Option<&SpawnId>)>,
    project_root: Res<ProjectRoot>,
    asset_catalog: Res<LoadedAssetCatalog>,
    mut spawn_params: SpawnParams,
    mut debug: ResMut<crate::DebugState>,
    mut global_volume: Option<ResMut<bevy::audio::GlobalVolume>>,
    bg_music_query: Query<Entity, With<BackgroundMusic>>,
    overlay_entities: Query<Entity, With<OverlayEntity>>,
    mut scene_state: SceneStateParams,
    mut game_events: MessageWriter<GameEvent>,
) {
    while let Some(action) = action_queue.pop() {
        debug.last_action = format!("{:?}", action);
        match action {
            Action::LoadScene(path) => {
                // Notify rules that the current scene is about to be replaced.
                if !debug.scene.is_empty() {
                    scene_events.write(SceneEvent::Unloading(debug.scene.clone()));
                }
                scene_state.preloaded.0.clear();
                scene_state.preloaded_glbs.0.clear();
                spawn_params.pending_spawns.0.clear();
                *scene_state.load_mode = PendingSceneLoadMode::Replace;
                let resolved = resolve_project_path(&project_root.0, &path);
                info!("Executing Action::LoadScene: {}", resolved);
                let handle: Handle<GameSceneV2> = asset_server.load(resolved.clone());
                commands.insert_resource(SceneHandleV2(handle));
                scene_events.write(SceneEvent::Requested(resolved));
                next_state.set(AppState::LoadingScene);
            }
            Action::LoadSceneOverlay(path) => {
                *scene_state.load_mode = PendingSceneLoadMode::Overlay;
                let resolved = resolve_project_path(&project_root.0, &path);
                info!("Executing Action::LoadSceneOverlay: {}", resolved);
                let handle: Handle<GameSceneV2> = asset_server.load(resolved.clone());
                commands.insert_resource(SceneHandleV2(handle));
                scene_events.write(SceneEvent::Requested(resolved));
            }
            Action::UnloadOverlay => {
                info!("Executing Action::UnloadOverlay");
                for entity in overlay_entities.iter() {
                    commands.entity(entity).despawn();
                }
            }
            Action::ToggleOverlay(path) => {
                if overlay_entities.is_empty() {
                    // No overlay active — load it.
                    *scene_state.load_mode = PendingSceneLoadMode::Overlay;
                    let resolved = resolve_project_path(&project_root.0, &path);
                    info!("Action::ToggleOverlay: opening overlay {}", resolved);
                    let handle: Handle<GameSceneV2> = asset_server.load(resolved.clone());
                    commands.insert_resource(SceneHandleV2(handle));
                    scene_events.write(SceneEvent::Requested(resolved));
                } else {
                    // Overlay is active — dismiss it.
                    info!("Action::ToggleOverlay: closing overlay");
                    for entity in overlay_entities.iter() {
                        commands.entity(entity).despawn();
                    }
                }
            }
            Action::Quit => {
                info!("Executing Action::Quit");
                exit.write(AppExit::Success);
            }
            Action::Log(msg) => {
                info!("Action::Log: {}", msg);
            }
            Action::Spawn { prefab, id, position, spawn_point, yaw_deg } => {
                let Some(prefab_def) = spawn_params.prefab_catalog.0.prefabs.get(&prefab) else {
                    warn!("Action::Spawn: prefab {:?} not found in catalog", prefab);
                    continue;
                };
                let Some(model_entry) = asset_catalog.0.models.get(&prefab_def.model) else {
                    warn!("Action::Spawn: model key {:?} not found in asset catalog", prefab_def.model);
                    continue;
                };
                let model_path = model_entry.path.clone();
                let prefab_def = prefab_def.clone();

                let spawn_id = id.unwrap_or_else(|| {
                    spawn_params.registry.counter += 1;
                    format!("{}_{}", prefab, spawn_params.registry.counter)
                });

                let (sx, sy, sz) = if let Some(pos) = position {
                    pos
                } else if let Some(ref name) = spawn_point {
                    match spawn_params.spawn_points.0.get(name.as_str()) {
                        Some(&pt) => pt,
                        None => {
                            warn!("Action::Spawn: spawn_point {:?} not found in scene, using origin", name);
                            (0.0, 0.0, 0.0)
                        }
                    }
                } else {
                    (0.0, 0.0, 0.0)
                };
                let yaw_rad = yaw_deg.unwrap_or(0.0).to_radians();
                let transform = Transform::from_xyz(sx, sy, sz)
                    .with_rotation(Quat::from_rotation_y(yaw_rad));

                info!(
                    "Action::Spawn: queued '{}' (prefab: {}) at ({:.1}, {:.1}, {:.1})",
                    spawn_id, prefab, sx, sy, sz
                );
                spawn_params.pending_spawns.0.push_back(super::QueuedSpawn {
                    prefab_def,
                    model_path,
                    transform,
                    spawn_id,
                    project_root: project_root.0.clone(),
                });
            }
            Action::Despawn(target_id) => {
                let found = spawn_params
                    .spawned
                    .iter()
                    .find(|(_, sid)| sid.0 == target_id)
                    .map(|(e, _)| e);
                if let Some(entity) = found {
                    info!("Action::Despawn: removing '{}' (entity {:?})", target_id, entity);
                    commands.entity(entity).despawn();
                    spawn_params.registry.entities.remove(&target_id);
                } else {
                    warn!("Action::Despawn: no entity with spawn id {:?}", target_id);
                }
            }
            Action::PlayAnimation(anim) => {
                info!("Executing Action::PlayAnimation: {}", anim);
                for (mut req, _) in &mut animation_requests {
                    req.queue.push_back(anim.clone());
                }
            }
            Action::PlayAnimationOn { target, clip } => {
                info!("Executing Action::PlayAnimationOn: target={} clip={}", target, clip);
                let mut found = false;
                for (mut req, sid) in &mut animation_requests {
                    if sid.map_or(false, |s| s.0 == target) {
                        req.queue.push_back(clip.clone());
                        found = true;
                    }
                }
                if !found {
                    warn!("Action::PlayAnimationOn: no entity with spawn id {:?}", target);
                }
            }
            Action::EmitEvent(event) => {
                info!("Executing Action::EmitEvent: {}", event);
                game_events.write(GameEvent::Trigger(event));
            }
            Action::StopMusic => {
                info!("Executing Action::StopMusic");
                for entity in bg_music_query.iter() {
                    commands.entity(entity).despawn();
                }
            }
            Action::PlayMusicLoop(key) => {
                // Stop any currently playing background music.
                for entity in bg_music_query.iter() {
                    commands.entity(entity).despawn();
                }
                if let Some(path) = asset_catalog.0.audio.get(&key) {
                    const SUPPORTED: &[&str] = &["wav", "ogg", "mp3"];
                    let ext = std::path::Path::new(path.as_str())
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if !SUPPORTED.contains(&ext.as_str()) {
                        warn!(
                            "Action::PlayMusicLoop: unsupported format '.{}' for key {:?}",
                            ext, key
                        );
                    } else {
                        info!("Action::PlayMusicLoop: {} -> {}", key, path);
                        let handle: Handle<bevy::audio::AudioSource> =
                            asset_server.load(path.clone());
                        commands.spawn((
                            BackgroundMusic,
                            bevy::audio::AudioPlayer::new(handle),
                            bevy::audio::PlaybackSettings::LOOP,
                        ));
                    }
                } else {
                    warn!("Action::PlayMusicLoop: key {:?} not found in audio catalog", key);
                }
            }
            Action::SetVolume(pct) => {
                let linear = (pct.min(100) as f32) / 100.0;
                info!("Action::SetVolume: {}% (linear {:.2})", pct, linear);
                if let Some(ref mut gv) = global_volume {
                    gv.volume = bevy::audio::Volume::Linear(linear);
                } else {
                    warn!("Action::SetVolume: GlobalVolume resource not available");
                }
            }
            Action::Preload(path) => {
                let resolved = resolve_project_path(&project_root.0, &path);
                info!("Action::Preload: warming cache for {}", resolved);
                if resolved.ends_with(".scene.ron") {
                    let handle: Handle<GameSceneV2> = asset_server.load(resolved);
                    scene_state.preloaded.0.push(handle);
                } else {
                    warn!(
                        "Action::Preload: only .scene.ron paths are supported (got {})",
                        resolved
                    );
                }
            }
            Action::PreloadPrefab(prefab_key) => {
                let Some(prefab_def) = spawn_params.prefab_catalog.0.prefabs.get(&prefab_key) else {
                    warn!("Action::PreloadPrefab: prefab {:?} not found in catalog", prefab_key);
                    continue;
                };
                let Some(model_entry) = asset_catalog.0.models.get(&prefab_def.model) else {
                    warn!("Action::PreloadPrefab: model key {:?} not found in asset catalog", prefab_def.model);
                    continue;
                };
                let model_path = model_entry.path.clone();
                info!("Action::PreloadPrefab: warming GLB cache for '{}' -> {}", prefab_key, model_path);
                let handle: Handle<bevy::scene::Scene> = asset_server.load(model_path);
                scene_state.preloaded_glbs.0.push(handle);
            }
            Action::PlaySound(key) => {
                if let Some(path) = asset_catalog.0.audio.get(&key) {
                    const SUPPORTED: &[&str] = &["wav", "ogg", "mp3"];
                    let ext = std::path::Path::new(path.as_str())
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if !SUPPORTED.contains(&ext.as_str()) {
                        warn!(
                            "Action::PlaySound: unsupported audio format '.{}' for key {:?} \
                             (path: {}). Supported formats: {:?}",
                            ext, key, path, SUPPORTED
                        );
                    } else {
                        info!("Executing Action::PlaySound: {} -> {}", key, path);
                        let handle: Handle<bevy::audio::AudioSource> =
                            asset_server.load(path.clone());
                        commands.spawn((
                            bevy::audio::AudioPlayer::new(handle),
                            bevy::audio::PlaybackSettings::DESPAWN,
                        ));
                    }
                } else {
                    warn!("Action::PlaySound: key {:?} not found in audio catalog", key);
                }
            }
            Action::EnterState(state) => {
                info!("Action::EnterState: \"{}\" -> \"{}\"", scene_state.logic_state.0, state);
                scene_state.logic_state.0 = state;
            }
            Action::SetVariable(key, value) => {
                info!("Action::SetVariable: \"{}\" = \"{}\"", key, value);
                scene_state.game_vars.0.insert(key, value);
            }
            Action::IncrementVariable(key, delta) => {
                let raw = scene_state.game_vars.0.get(&key).map(String::as_str).unwrap_or("0");
                let current: i32 = match raw.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        warn!(
                            "Action::IncrementVariable: variable \"{}\" has non-numeric value \"{}\", treating as 0",
                            key, raw
                        );
                        0
                    }
                };
                let next = current + delta;
                info!("Action::IncrementVariable: \"{}\" {} -> {}", key, delta, next);
                scene_state.game_vars.0.insert(key, next.to_string());
            }
        }
    }
}
