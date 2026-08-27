use bevy::prelude::*;
use bevy::audio::AudioSinkPlayback;
use crate::ProjectRoot;
use crate::schema::*;
use crate::schema::scene_v2::GameSceneV2;
use crate::schema::stats::{ActiveModifier, StackRule};
use crate::runtime::messages::*;
use crate::runtime::actions::ActionQueue;
use crate::capabilities::animation_resolver::{AnimationRequests, AnimationRequest};
use crate::capabilities::camera::{MAX_SPLIT_PLAYERS, CameraBlendState};
use crate::schema::camera::CameraModeDef;
use crate::capabilities::damage_popup::DamagePopup;
use crate::capabilities::particle::QueuedParticleEffect;
use super::{
    BackgroundMusic, LevelEntity, LoadedAssetCatalog, OverlayEntity, PendingSceneLoadMode,
    SceneHandleV2, SceneStateParams, SpawnParams, SpawnId, WorldLabel, WorldLabelRank,
    resolve_project_path,
};
use super::entity_spawner::{assemble_player_config, apply_camera_mode};

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
    mut bg_music_query: Query<(Entity, Option<&mut bevy::audio::AudioSink>, Option<&bevy::audio::PlaybackSettings>), With<BackgroundMusic>>,
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
                scene_state.current_target.0 = None;
                crate::capabilities::targeting::clear_target_vars(&mut scene_state.game_vars);
                scene_state.preloaded.0.clear();
                scene_state.preloaded_glbs.0.clear();
                scene_state.delayed_events.0.clear();
                spawn_params.pending_spawns.0.clear();
                scene_state.active_dialogue.clear();
                scene_state.inventory_ui.panels_open = 0;
                commands.insert_resource(crate::runtime::scene_manager::LoadedTargetIndicator(None));
                commands.insert_resource(crate::runtime::scene_manager::LoadedTargetHud(None));
                commands.insert_resource(crate::runtime::scene_manager::ActiveViewBox(None));
                commands.insert_resource(crate::runtime::scene_manager::ActiveSplitScreen(None));
                commands.insert_resource(crate::runtime::scene_manager::DynamicSplitConfig(None));
                commands.insert_resource(crate::runtime::scene_manager::ActiveSplitSlotCount(None));
                commands.insert_resource(crate::runtime::scene_manager::TargetRingVisibilityMode::AllViewports);
                commands.insert_resource(crate::runtime::scene_manager::SuppressPlayerCameras(false));
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
                // try_despawn: only overlay roots are OverlayEntity-tagged (descendants aren't,
                // so recursion is fine) — but this shares its query snapshot with
                // ToggleOverlay's close branch below and scene_loader.rs's own overlay sweep;
                // two of those queued in the same frame would double-despawn the same root.
                for entity in overlay_entities.iter() {
                    commands.entity(entity).try_despawn();
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
                        commands.entity(entity).try_despawn();
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
            Action::Spawn { prefab, id, position, spawn_point, yaw_deg, at_entity } => {
                let Some(prefab_def) = spawn_params.prefab_catalog.0.prefabs.get(&prefab) else {
                    warn!("Action::Spawn: prefab {:?} not found in catalog", prefab);
                    continue;
                };
                // Reject a primitive-shaped player prefab unconditionally, BEFORE the model
                // lookup below — not just when that lookup happens to fail. `model: ""` is only
                // a convention for `kind: Primitive` prefabs (schema/catalog.rs), not enforced;
                // a primitive player prefab that also sets a resolvable `model` key would
                // otherwise sail past the lookup, get assembled with
                // `PlayerModelSource::Primitive`, and panic at spawn time (`spawn_player_entity`
                // always passes `None` for `PrimitivePlayerCtx` on this path — debug-detective
                // finding, player_model_source_unification.md). Dynamic (character-select) spawn
                // of a primitive-bodied player is v3-deferred, regardless of what `model` is set to.
                if prefab_def.kind == crate::schema::catalog::PrefabKind::Primitive
                    && prefab_def.components.tags.iter().any(|t| t == "player")
                {
                    warn!(
                        "Action::Spawn: primitive-shaped player prefab '{}' can't be spawned \
                         dynamically (via character-select/Action::Spawn) yet — only the \
                         immediate scene-load path supports primitive players in v1. Use a \
                         GLB (Actor-kind) player prefab for character-select, or place this \
                         player directly in the scene's `entities:` list instead.",
                        prefab
                    );
                    continue;
                }
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

                if position.is_some() && spawn_point.is_some() {
                    warn!(
                        "Action::Spawn '{}': both position and spawn_point are set; position wins",
                        spawn_id
                    );
                }
                if at_entity.is_some() && (position.is_some() || spawn_point.is_some()) {
                    // debug!, not warn! — authoring a position/spawn_point fallback alongside
                    // at_entity is the documented, encouraged safe pattern (it's what makes the
                    // "unresolvable at_entity" case degrade to a known position instead of
                    // skipping the spawn entirely), not a misconfiguration. Warning on the
                    // correct usage every time would be spurious noise (system-architect finding).
                    debug!(
                        "Action::Spawn '{}': both at_entity and position/spawn_point are set; at_entity wins",
                        spawn_id
                    );
                }
                let fallback_pos = if let Some(pos) = position {
                    Some(pos)
                } else if let Some(ref name) = spawn_point {
                    match spawn_params.spawn_points.0.get(name.as_str()) {
                        Some(&pt) => Some(pt),
                        None => {
                            warn!("Action::Spawn: spawn_point {:?} not found in scene, using origin", name);
                            Some((0.0, 0.0, 0.0))
                        }
                    }
                } else {
                    None
                };
                // `at_entity` resolves both position and facing from a live entity's
                // GlobalTransform (same SpawnRegistry lookup SpawnEffect's `entity` field uses) —
                // it takes precedence over position/spawn_point/yaw_deg entirely, since the caller
                // doesn't know its own current yaw to also write a matching `yaw_deg`. Unlike a
                // missing spawn_point (which falls back to the origin), an unresolvable
                // `at_entity` with no other position given skips the spawn outright — placing a
                // dynamically-important entity (e.g. a lootable corpse) at the origin would be
                // worse than not spawning it at all.
                let at_entity_transform = at_entity.as_ref().and_then(|target_id| {
                    spawn_params.registry.entities.get(target_id)
                        .and_then(|e| scene_state.global_transforms.get(*e).ok())
                        .map(|gt| gt.compute_transform())
                });
                let transform = if let Some(tf) = at_entity_transform {
                    tf
                } else if at_entity.is_some() {
                    let Some((sx, sy, sz)) = fallback_pos else {
                        warn!(
                            "Action::Spawn: at_entity {:?} not found and no position/spawn_point \
                             fallback given; skipping spawn",
                            at_entity
                        );
                        continue;
                    };
                    warn!(
                        "Action::Spawn: at_entity {:?} not found in registry; falling back to \
                         position/spawn_point",
                        at_entity
                    );
                    Transform::from_xyz(sx, sy, sz)
                        .with_rotation(Quat::from_rotation_y(yaw_deg.unwrap_or(0.0).to_radians()))
                } else {
                    let (sx, sy, sz) = fallback_pos.unwrap_or((0.0, 0.0, 0.0));
                    Transform::from_xyz(sx, sy, sz)
                        .with_rotation(Quat::from_rotation_y(yaw_deg.unwrap_or(0.0).to_radians()))
                };

                info!(
                    "Action::Spawn: queued '{}' (prefab: {}) at ({:.1}, {:.1}, {:.1})",
                    spawn_id, prefab, transform.translation.x, transform.translation.y, transform.translation.z
                );

                // Detect player-tagged prefabs and assemble a PlayerConfig so the drain
                // system calls spawn_player_entity (camera + controller) instead of the
                // normal spawn_prefab_instance path.
                let player_config = if prefab_def.components.tags.contains(&"player".to_string()) {
                    Some(assemble_player_config(
                        &prefab_def,
                        &prefab,
                        &spawn_id,
                        Some(model_path.clone()),
                        (transform.translation.x, transform.translation.y, transform.translation.z),
                        spawn_params.nameplate_config.player_enabled,
                    ))
                } else {
                    None
                };

                if player_config.is_some() {
                    scene_state.player_inventory.player_spawn_id = Some(spawn_id.clone());
                }
                spawn_params.pending_spawns.0.push_back(super::QueuedSpawn {
                    prefab_def,
                    model_path,
                    transform,
                    spawn_id,
                    prefab_key: prefab.clone(),
                    project_root: project_root.0.clone(),
                    player_config,
                    is_hot_join: false,
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
                    // try_despawn: two Action::Despawn(same_id) in one executor run both read
                    // the same `spawned` query snapshot, so the second still finds the entity
                    // (its removal is deferred, not yet applied) and would double-despawn it.
                    commands.entity(entity).try_despawn();
                    spawn_params.registry.entities.remove(&target_id);

                    // If the despawned entity was the currently-open container (e.g. a lootable
                    // corpse decaying, or the id-reuse guard's own Despawn firing while its panel
                    // is still open), tear the panel down the same way CloseContainer does.
                    // Otherwise this leaves a ghost panel bound to a gone entity and
                    // `panels_open` stuck above 0, permanently blocking interact/pickup/
                    // tab-targeting (debug-detective finding, monster_corpse_loot.md v2).
                    if scene_state.container_ui.active_container == Some(entity) {
                        for (_, mut vis) in scene_state.container_panel_q.iter_mut() {
                            if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
                        }
                        scene_state.inventory_ui.set_panel_open(false);
                        game_events.write(GameEvent::Trigger("ui.panel_closed".to_string()));
                        scene_state.container_ui.active_container = None;
                        game_events.write(GameEvent::Trigger("container.closed".to_string()));
                    }
                } else {
                    warn!("Action::Despawn: no entity with spawn id {:?}", target_id);
                }
            }
            Action::SetDespawnTimer { entity: target_id, delay_secs } => {
                let found = spawn_params
                    .spawned
                    .iter()
                    .find(|(_, sid)| sid.0 == target_id)
                    .map(|(e, _)| e);
                if let Some(entity) = found {
                    info!(
                        "Action::SetDespawnTimer: '{}' (entity {:?}) despawns in {:.1}s",
                        target_id, entity, delay_secs
                    );
                    commands
                        .entity(entity)
                        .insert(crate::capabilities::DespawnTimer { remaining_secs: delay_secs });
                } else {
                    warn!("Action::SetDespawnTimer: no entity with spawn id {:?}", target_id);
                }
            }
            Action::PlayAnimation(anim) => {
                info!("Executing Action::PlayAnimation: {}", anim);
                for (mut req, _) in &mut animation_requests {
                    req.queue.push_back(anim.clone().into());
                }
            }
            Action::PlayAnimationOn { target, clip, start_at_fraction, freeze } => {
                info!(
                    "Executing Action::PlayAnimationOn: target={} clip={} start_at_fraction={:?} freeze={}",
                    target, clip, start_at_fraction, freeze
                );
                let mut found = false;
                for (mut req, sid) in &mut animation_requests {
                    if sid.map_or(false, |s| s.0 == target) {
                        req.queue.push_back(AnimationRequest {
                            clip_or_id: clip.clone(),
                            start_at_fraction,
                            freeze,
                        });
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
                // try_despawn: StopMusic + PlayMusicLoop (or two StopMusic) in the same
                // executor run both iterate this same bg_music_query snapshot.
                for (entity, _, _) in bg_music_query.iter_mut() {
                    commands.entity(entity).try_despawn();
                }
            }
            Action::PlayMusicLoop { key, volume: action_volume } => {
                // Stop any currently playing background music. try_despawn: see StopMusic above.
                for (entity, _, _) in bg_music_query.iter_mut() {
                    commands.entity(entity).try_despawn();
                }
                if let Some(entry) = asset_catalog.0.audio.get(&key) {
                    const SUPPORTED: &[&str] = &["wav", "ogg", "mp3"];
                    let ext = std::path::Path::new(entry.path.as_str())
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
                        let combined = (entry.volume * action_volume).clamp(0.0, 1.0);
                        info!("Action::PlayMusicLoop: {} -> {} (volume {:.2})", key, entry.path, combined);
                        let handle: Handle<bevy::audio::AudioSource> =
                            asset_server.load(entry.path.clone());
                        commands.spawn((
                            BackgroundMusic,
                            bevy::audio::AudioPlayer::new(handle),
                            bevy::audio::PlaybackSettings {
                                volume: bevy::audio::Volume::Linear(combined),
                                ..bevy::audio::PlaybackSettings::LOOP
                            },
                        ));
                    }
                } else {
                    warn!("Action::PlayMusicLoop: key {:?} not found in audio catalog", key);
                }
            }
            Action::SetVolume(pct) => {
                let fraction = (pct.min(100) as f32) / 100.0;
                scene_state.audio_state.active_fraction = fraction;
                let effective = scene_state.audio_state.effective_volume();
                info!("Action::SetVolume: {}% (effective {:.2}, max {:.2})", pct, effective, scene_state.audio_state.max_volume);
                if let Some(ref mut gv) = global_volume {
                    gv.volume = bevy::audio::Volume::Linear(effective);
                } else {
                    warn!("Action::SetVolume: GlobalVolume resource not available");
                }
                // Update already-playing sinks: GlobalVolume only applies at sink creation time.
                for (_, sink_opt, settings_opt) in bg_music_query.iter_mut() {
                    if let (Some(mut sink), Some(settings)) = (sink_opt, settings_opt) {
                        sink.set_volume(bevy::audio::Volume::Linear(settings.volume.to_linear() * effective));
                    }
                }
                game_events.write(GameEvent::Trigger("audio.volume_changed".to_string()));
            }
            Action::ToggleMute => {
                scene_state.audio_state.muted = !scene_state.audio_state.muted;
                let effective = scene_state.audio_state.effective_volume();
                let event_name = if scene_state.audio_state.muted { "audio.muted" } else { "audio.unmuted" };
                info!("Action::ToggleMute: muted={} (effective {:.2})", scene_state.audio_state.muted, effective);
                if let Some(ref mut gv) = global_volume {
                    gv.volume = bevy::audio::Volume::Linear(effective);
                } else {
                    warn!("Action::ToggleMute: GlobalVolume resource not available");
                }
                // Update already-playing sinks: GlobalVolume only applies at sink creation time.
                for (_, sink_opt, settings_opt) in bg_music_query.iter_mut() {
                    if let (Some(mut sink), Some(settings)) = (sink_opt, settings_opt) {
                        sink.set_volume(bevy::audio::Volume::Linear(settings.volume.to_linear() * effective));
                    }
                }
                game_events.write(GameEvent::Trigger(event_name.to_string()));
            }
            Action::SyncAudioState => {
                let event_name = if scene_state.audio_state.muted { "audio.muted" } else { "audio.unmuted" };
                info!("Action::SyncAudioState: emitting {}", event_name);
                game_events.write(GameEvent::Trigger(event_name.to_string()));
            }
            Action::ToggleOwnNameplate => {
                scene_state.nameplate_pref.0 = !scene_state.nameplate_pref.0;
                let event_name = if scene_state.nameplate_pref.0 { "nameplate.own_shown" } else { "nameplate.own_hidden" };
                info!("Action::ToggleOwnNameplate: shown={} (emitting {})", scene_state.nameplate_pref.0, event_name);
                game_events.write(GameEvent::Trigger(event_name.to_string()));
            }
            Action::PreloadScene(path) => {
                let resolved = resolve_project_path(&project_root.0, &path);
                info!("Action::PreloadScene: warming cache for {}", resolved);
                if resolved.ends_with(".scene.ron") {
                    let handle: Handle<GameSceneV2> = asset_server.load(resolved);
                    scene_state.preloaded.0.push(handle);
                } else {
                    warn!(
                        "Action::PreloadScene: only .scene.ron paths are supported (got {})",
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
            Action::PreloadGlb(model_key) => {
                let Some(model_entry) = asset_catalog.0.models.get(&model_key) else {
                    warn!("Action::PreloadGlb: model key {:?} not found in asset catalog", model_key);
                    continue;
                };
                let model_path = model_entry.path.clone();
                info!("Action::PreloadGlb: warming GLB cache for model '{}' -> {}", model_key, model_path);
                let handle: Handle<bevy::scene::Scene> = asset_server.load(model_path);
                scene_state.preloaded_glbs.0.push(handle);
            }
            Action::PlaySound { key, volume: action_volume } => {
                if let Some(entry) = asset_catalog.0.audio.get(&key) {
                    const SUPPORTED: &[&str] = &["wav", "ogg", "mp3"];
                    let ext = std::path::Path::new(entry.path.as_str())
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if !SUPPORTED.contains(&ext.as_str()) {
                        warn!(
                            "Action::PlaySound: unsupported audio format '.{}' for key {:?} \
                             (path: {}). Supported formats: {:?}",
                            ext, key, entry.path, SUPPORTED
                        );
                    } else {
                        let combined = (entry.volume * action_volume).clamp(0.0, 1.0);
                        info!("Executing Action::PlaySound: {} -> {} (volume {:.2})", key, entry.path, combined);
                        let handle: Handle<bevy::audio::AudioSource> =
                            asset_server.load(entry.path.clone());
                        commands.spawn((
                            bevy::audio::AudioPlayer::new(handle),
                            bevy::audio::PlaybackSettings {
                                volume: bevy::audio::Volume::Linear(combined),
                                ..bevy::audio::PlaybackSettings::DESPAWN
                            },
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
            Action::ModifyStat { key, delta } => {
                if let Some((entity_id, stat_name)) = key.split_once('.') {
                    // Instance stat — find entity by spawn ID, mutate its StatMap component.
                    let entity = spawn_params.registry.entities.get(entity_id).copied();
                    if let Some(e) = entity {
                        if let Ok((_, mut stat_map)) = scene_state.stat_map_query.get_mut(e) {
                            if let Some(stat) = stat_map.0.get_mut(stat_name) {
                                let new_val = stat.apply_delta(delta);
                                info!("Action::ModifyStat: \"{}.{}\" {:+.2} -> {:.2}", entity_id, stat_name, delta, new_val);
                            } else {
                                warn!("Action::ModifyStat: stat {:?} not found in StatMap of entity {:?}", stat_name, entity_id);
                            }
                        } else {
                            warn!("Action::ModifyStat: entity {:?} has no StatMap component", entity_id);
                        }
                    } else {
                        warn!("Action::ModifyStat: entity {:?} not found in spawn registry", entity_id);
                    }
                } else {
                    // Global stat — LoadedStats resource.
                    if let Some(stat) = scene_state.loaded_stats.0.get_mut(&key) {
                        let new_val = stat.apply_delta(delta);
                        info!("Action::ModifyStat: \"{}\" {:+.2} -> {:.2}", key, delta, new_val);
                    } else {
                        warn!("Action::ModifyStat: stat {:?} not found in stats catalog", key);
                    }
                }
            }
            Action::SetStat { key, value } => {
                if let Some((entity_id, stat_name)) = key.split_once('.') {
                    // Instance stat — find entity by spawn ID, set its StatMap component.
                    let entity = spawn_params.registry.entities.get(entity_id).copied();
                    if let Some(e) = entity {
                        if let Ok((_, mut stat_map)) = scene_state.stat_map_query.get_mut(e) {
                            if let Some(stat) = stat_map.0.get_mut(stat_name) {
                                let new_val = stat.set_value(value);
                                info!("Action::SetStat: \"{}.{}\" = {:.2}", entity_id, stat_name, new_val);
                            } else {
                                warn!("Action::SetStat: stat {:?} not found in StatMap of entity {:?}", stat_name, entity_id);
                            }
                        } else {
                            warn!("Action::SetStat: entity {:?} has no StatMap component", entity_id);
                        }
                    } else {
                        warn!("Action::SetStat: entity {:?} not found in spawn registry", entity_id);
                    }
                } else {
                    // Global stat — LoadedStats resource.
                    if let Some(stat) = scene_state.loaded_stats.0.get_mut(&key) {
                        let new_val = stat.set_value(value);
                        info!("Action::SetStat: \"{}\" = {:.2}", key, new_val);
                    } else {
                        warn!("Action::SetStat: stat {:?} not found in stats catalog", key);
                    }
                }
            }
            Action::ApplyModifier { modifier_key } => {
                let Some(def) = scene_state.loaded_modifiers.0.get(&modifier_key) else {
                    warn!("Action::ApplyModifier: modifier {:?} not defined in stats catalog", modifier_key);
                    continue;
                };
                let stat_key = def.stat.clone();
                let duration = def.duration_secs;
                let stack_rule = def.stack_rule.clone();

                let modifier = ActiveModifier {
                    key: modifier_key.clone(),
                    remaining_secs: duration,
                };

                if let Some(stat) = scene_state.loaded_stats.0.get_mut(&stat_key) {
                    match stack_rule {
                        StackRule::Replace => {
                            stat.active_modifiers.retain(|am| am.key != modifier_key);
                            stat.active_modifiers.push(modifier);
                        }
                        _ => stat.active_modifiers.push(modifier),
                    }
                    info!("Action::ApplyModifier: \"{}\" applied to stat \"{}\"", modifier_key, stat_key);
                } else {
                    warn!("Action::ApplyModifier: stat {:?} not found for modifier {:?}", stat_key, modifier_key);
                }
            }
            Action::RemoveModifier { modifier_key } => {
                let stat_key = scene_state.loaded_modifiers.0.get(&modifier_key)
                    .map(|d| d.stat.clone());

                if let Some(stat_key) = stat_key {
                    if let Some(stat) = scene_state.loaded_stats.0.get_mut(&stat_key) {
                        let before = stat.active_modifiers.len();
                        stat.active_modifiers.retain(|am| am.key != modifier_key);
                        if stat.active_modifiers.len() < before {
                            let event = format!("stat.modifier.removed:{}", modifier_key);
                            info!("Action::RemoveModifier: \"{}\" removed from stat \"{}\" -> emitting \"{}\"", modifier_key, stat_key, event);
                            game_events.write(GameEvent::Trigger(event));
                        } else {
                            info!("Action::RemoveModifier: \"{}\" was not active (no-op)", modifier_key);
                        }
                    } else {
                        warn!("Action::RemoveModifier: stat {:?} not found for modifier {:?}", stat_key, modifier_key);
                    }
                } else {
                    warn!("Action::RemoveModifier: modifier {:?} not defined in stats catalog", modifier_key);
                }
            }
            Action::ShowDamagePopup { entity: entity_id, amount } => {
                let entity = spawn_params.registry.entities.get(&entity_id).copied();
                if let Some(e) = entity {
                    if let Ok(gtf) = scene_state.global_transforms.get(e) {
                        let default_style = crate::schema::project::DamagePopupStyle::default();
                        let style = scene_state.project_config
                            .as_ref()
                            .and_then(|pc| pc.damage_popup_style.as_ref())
                            .unwrap_or(&default_style);
                        let text = if amount >= 0.0 {
                            format!("+{:.0}", amount)
                        } else {
                            format!("{:.0}", amount)
                        };
                        let (r, g, b, a) = if amount >= 0.0 { style.heal_color } else { style.damage_color };
                        info!(
                            "Action::ShowDamagePopup: '{}' ({}) at {:?}",
                            entity_id, text, gtf.translation()
                        );
                        let (ox, oy, oz) = style.spawn_offset;
                        let popup_offset = Vec3::new(ox, oy, oz);
                        // Split-screen scenes duplicate one rank-0 primary plus rank
                        // 1..MAX_SPLIT_PLAYERS siblings — same gate drain_dynamic_stat_ui_system
                        // uses — so the popup renders in whichever viewport(s) the target is
                        // actually visible in, not just the single highest-priority active camera
                        // regardless of which player's action triggered it. See
                        // planning/features/per_player_split_screen_targeting.md Phase 2.
                        let is_split_screen = scene_state.active_split.0.is_some() || scene_state.dynamic_split.0.is_some();
                        let popup_ranks = if is_split_screen { MAX_SPLIT_PLAYERS } else { 1 };
                        for rank in 0..popup_ranks {
                            let mut shadow_entity = commands.spawn((
                                Text2d::new(text.clone()),
                                TextFont { font_size: style.font_size, ..default() },
                                TextColor(Color::srgba(0.0, 0.0, 0.0, a)),
                                Transform::from_xyz(0.0, 0.0, 9.0),
                                WorldLabel {
                                    world_pos: Vec3::ZERO,
                                    tracked_entity: Some(e),
                                    offset: popup_offset,
                                    base_font_size: style.font_size,
                                    depth_scale: None,
                                    screen_offset: Vec2::new(1.0, -1.0),
                                },
                                DamagePopup { elapsed: 0.0, duration: style.duration_secs, rise_speed: style.rise_speed },
                                LevelEntity,
                            ));
                            if rank > 0 {
                                shadow_entity.insert((WorldLabelRank(rank as u8), Visibility::Hidden));
                            }
                            let mut main_entity = commands.spawn((
                                Text2d::new(text.clone()),
                                TextFont { font_size: style.font_size, ..default() },
                                TextColor(Color::srgba(r, g, b, a)),
                                Transform::from_xyz(0.0, 0.0, 10.0),
                                WorldLabel {
                                    world_pos: Vec3::ZERO,
                                    tracked_entity: Some(e),
                                    offset: popup_offset,
                                    base_font_size: style.font_size,
                                    depth_scale: None,
                                    screen_offset: Vec2::ZERO,
                                },
                                DamagePopup { elapsed: 0.0, duration: style.duration_secs, rise_speed: style.rise_speed },
                                LevelEntity,
                            ));
                            if rank > 0 {
                                main_entity.insert((WorldLabelRank(rank as u8), Visibility::Hidden));
                            }
                        }
                    } else {
                        warn!("Action::ShowDamagePopup: entity '{}' has no GlobalTransform", entity_id);
                    }
                } else {
                    warn!("Action::ShowDamagePopup: entity '{}' not found in spawn registry", entity_id);
                }
            }
            Action::ShowFloatingText { entity: entity_id, text, offset: offset_override } => {
                let entity = spawn_params.registry.entities.get(&entity_id).copied();
                if let Some(e) = entity {
                    if let Ok(gtf) = scene_state.global_transforms.get(e) {
                        let default_style = crate::schema::project::DamagePopupStyle::default();
                        let style = scene_state.project_config
                            .as_ref()
                            .and_then(|pc| pc.damage_popup_style.as_ref())
                            .unwrap_or(&default_style);
                        let (ox, oy, oz) = offset_override.unwrap_or(style.spawn_offset);
                        let popup_offset = Vec3::new(ox, oy, oz);
                        info!("Action::ShowFloatingText: '{}' \"{}\" at {:?}", entity_id, text, gtf.translation());
                        // Same split-screen rank-duplication gate as Action::ShowDamagePopup above
                        // — see planning/features/per_player_split_screen_targeting.md Phase 2.
                        let is_split_screen = scene_state.active_split.0.is_some() || scene_state.dynamic_split.0.is_some();
                        let popup_ranks = if is_split_screen { MAX_SPLIT_PLAYERS } else { 1 };
                        for rank in 0..popup_ranks {
                            let mut shadow_entity = commands.spawn((
                                Text2d::new(text.clone()),
                                TextFont { font_size: style.font_size, ..default() },
                                TextColor(Color::srgba(0.0, 0.0, 0.0, 1.0)),
                                Transform::from_xyz(0.0, 0.0, 9.0),
                                WorldLabel {
                                    world_pos: Vec3::ZERO,
                                    tracked_entity: Some(e),
                                    offset: popup_offset,
                                    base_font_size: style.font_size,
                                    depth_scale: None,
                                    screen_offset: Vec2::new(1.0, -1.0),
                                },
                                DamagePopup { elapsed: 0.0, duration: style.duration_secs, rise_speed: style.rise_speed },
                                LevelEntity,
                            ));
                            if rank > 0 {
                                shadow_entity.insert((WorldLabelRank(rank as u8), Visibility::Hidden));
                            }
                            let mut main_entity = commands.spawn((
                                Text2d::new(text.clone()),
                                TextFont { font_size: style.font_size, ..default() },
                                TextColor(Color::srgba(1.0, 0.92, 0.3, 1.0)),
                                Transform::from_xyz(0.0, 0.0, 10.0),
                                WorldLabel {
                                    world_pos: Vec3::ZERO,
                                    tracked_entity: Some(e),
                                    offset: popup_offset,
                                    base_font_size: style.font_size,
                                    depth_scale: None,
                                    screen_offset: Vec2::ZERO,
                                },
                                DamagePopup { elapsed: 0.0, duration: style.duration_secs, rise_speed: style.rise_speed },
                                LevelEntity,
                            ));
                            if rank > 0 {
                                main_entity.insert((WorldLabelRank(rank as u8), Visibility::Hidden));
                            }
                        }
                    } else {
                        warn!("Action::ShowFloatingText: entity '{}' has no GlobalTransform", entity_id);
                    }
                } else {
                    warn!("Action::ShowFloatingText: entity '{}' not found in spawn registry", entity_id);
                }
            }
            Action::SetEntityVisible { entity: entity_id, visible } => {
                let entity = spawn_params.registry.entities.get(&entity_id).copied();
                if let Some(e) = entity {
                    let vis = if visible { Visibility::Visible } else { Visibility::Hidden };
                    commands.entity(e).insert(vis);
                    info!("Action::SetEntityVisible: '{}' -> {:?}", entity_id, vis);
                } else {
                    warn!("Action::SetEntityVisible: entity '{}' not found in spawn registry", entity_id);
                }
            }
            Action::EmitEventAfterDelay { event, delay_secs } => {
                info!("Action::EmitEventAfterDelay: '{}' in {:.1}s", event, delay_secs);
                scene_state.delayed_events.0.push((delay_secs, event));
            }
            Action::SpawnEffect { key, position, entity } => {
                let Some(def) = asset_catalog.0.effects.get(&key).cloned() else {
                    warn!("Action::SpawnEffect: unknown effect key {:?}", key);
                    continue;
                };
                let offset = Vec3::new(def.offset.0, def.offset.1, def.offset.2);

                let world_pos: Option<Vec3> = if let Some(ref entity_id) = entity {
                    if position.is_some() {
                        warn!("Action::SpawnEffect {:?}: both entity and position given; entity wins", key);
                    }
                    spawn_params.registry.entities.get(entity_id)
                        .and_then(|e| scene_state.global_transforms.get(*e).ok())
                        .map(|gt| gt.translation() + offset)
                } else {
                    position.map(|(x, y, z)| Vec3::new(x, y, z) + offset)
                };

                let Some(origin) = world_pos else {
                    warn!("Action::SpawnEffect {:?}: no entity or position resolved; skipping", key);
                    continue;
                };

                info!(
                    "Action::SpawnEffect: queued {:?} at ({:.1}, {:.1}, {:.1}) ({} particles)",
                    key, origin.x, origin.y, origin.z, def.particle_count
                );
                spawn_params.pending_particles.0.push(QueuedParticleEffect { origin, def });
            }
            Action::ProjectDecal { key, entity, position, radius, duration_secs, color, pulse_speed } => {
                let Some(texture_path) = asset_catalog.0.decals.get(&key).cloned() else {
                    warn!("Action::ProjectDecal: unknown decal key {:?}", key);
                    continue;
                };

                let world_pos: Option<Vec3> = if let Some(ref entity_id) = entity {
                    if position.is_some() {
                        warn!("Action::ProjectDecal {:?}: both entity and position given; entity wins", key);
                    }
                    spawn_params.registry.entities.get(entity_id)
                        .and_then(|e| scene_state.global_transforms.get(*e).ok())
                        .map(|gt| gt.translation())
                } else {
                    position.map(|(x, y, z)| Vec3::new(x, y, z))
                };

                let Some(origin) = world_pos else {
                    warn!("Action::ProjectDecal {:?}: no entity or position resolved; skipping", key);
                    continue;
                };

                let track_entity = entity.as_ref()
                    .and_then(|id| spawn_params.registry.entities.get(id))
                    .copied();

                info!(
                    "Action::ProjectDecal: queued {:?} at ({:.1}, {:.1}, {:.1}) r={:.1} dur={:.1}s",
                    key, origin.x, origin.y, origin.z, radius, duration_secs
                );
                spawn_params.pending_decals.0.push(crate::capabilities::decal::QueuedDecal {
                    texture_path,
                    world_pos: origin,
                    radius,
                    duration_secs,
                    color,
                    pulse_speed,
                    track_entity,
                });
            }
            Action::SetParticleQuality(level) => {
                info!("Action::SetParticleQuality: {:?}", level);
                spawn_params.particle_quality.level = level;
            }
            Action::ResetToSpawn(entity_id) => {
                let Some(&entity) = spawn_params.registry.entities.get(&entity_id) else {
                    warn!("Action::ResetToSpawn: entity '{}' not found in spawn registry", entity_id);
                    continue;
                };
                let Ok(npc) = scene_state.npc_agents.get(entity) else {
                    warn!("Action::ResetToSpawn: entity '{}' has no NpcAgent — ResetToSpawn only works on NPC entities", entity_id);
                    continue;
                };
                let origin = npc.origin;
                if let Ok(mut tf) = scene_state.transforms.get_mut(entity) {
                    tf.translation = origin;
                    info!("Action::ResetToSpawn: '{}' teleported to ({:.1}, {:.1}, {:.1})", entity_id, origin.x, origin.y, origin.z);
                }
                if let Ok(mut vel) = scene_state.npc_velocities.get_mut(entity) {
                    vel.linvel = Vec3::ZERO;
                }
            }
            Action::SetTarget(id) => {
                info!("Action::SetTarget: {:?}", id);
                // Update the target UI variables so a rule-driven SetTarget updates a bound
                // label identically to click/Tab selection (resolve the prefab key via the
                // spawn registry, falling back to id-only display if it isn't found).
                let prefab = spawn_params.registry.entities.get(&id)
                    .and_then(|e| scene_state.prefab_keys.get(*e).ok())
                    .map(|p| p.0.clone());
                let is_multiplayer = scene_state.player_targets.iter().count() >= 2;
                let primary = scene_state.player_targets.iter()
                    .find(|(_, _, idx)| crate::capabilities::targeting::is_primary_player(*idx))
                    .map(|(e, _, _)| e);
                // Mirror into the primary player's PlayerTarget (not just CurrentTarget) —
                // otherwise the ring (target_indicator_system) and target_auto_clear_system,
                // both now driven by PlayerTarget, would never react to a rule-driven SetTarget.
                if let Some(entity) = primary {
                    if let Ok((_, mut player_target, _)) = scene_state.player_targets.get_mut(entity) {
                        crate::capabilities::targeting::apply_player_target(
                            &id, prefab.as_deref(), true, is_multiplayer,
                            &mut player_target, &mut scene_state.current_target,
                            &mut scene_state.game_vars, &mut game_events,
                        );
                    }
                } else {
                    // No player entities in this scene at all (e.g. a menu) — fall back to the
                    // resource/vars-only behavior that predates per-player targeting.
                    scene_state.current_target.0 = Some(id.clone());
                    crate::capabilities::targeting::write_target_vars(
                        &mut scene_state.game_vars, prefab.as_deref(), &id,
                    );
                    game_events.write(GameEvent::Trigger(format!("target.changed:{}", id)));
                    game_events.write(GameEvent::Trigger("target.changed".to_string()));
                }
            }
            Action::ClearTarget => {
                info!("Action::ClearTarget");
                let primary = scene_state.player_targets.iter()
                    .find(|(_, _, idx)| crate::capabilities::targeting::is_primary_player(*idx))
                    .map(|(e, _, _)| e);
                if let Some(entity) = primary {
                    if let Ok((_, mut player_target, _)) = scene_state.player_targets.get_mut(entity) {
                        crate::capabilities::targeting::clear_player_target(
                            true, &mut player_target, &mut scene_state.current_target,
                            &mut scene_state.game_vars, &mut game_events,
                        );
                    }
                } else {
                    scene_state.current_target.0 = None;
                    crate::capabilities::targeting::clear_target_vars(&mut scene_state.game_vars);
                    game_events.write(GameEvent::Trigger("target.cleared".to_string()));
                }
            }
            Action::CameraShake { duration_secs, intensity, owner_player } => {
                info!(
                    "Action::CameraShake: duration={:.2}s intensity={:.3} owner_player={:?}",
                    duration_secs, intensity, owner_player
                );
                let target_player = owner_player.and_then(|n| {
                    let resolved = resolve_player_entity_by_index(n, &scene_state.player_targets);
                    if resolved.is_none() {
                        warn!(
                            "Action::CameraShake: owner_player {} has no live player entity \
                             (not yet joined, or out of range) — shake ignored",
                            n
                        );
                    }
                    resolved
                });
                if owner_player.is_some() && target_player.is_none() {
                    continue; // warned above; nothing to shake
                }
                let mut found = false;
                for (camera_entity, targets) in scene_state.orbit_cameras.iter() {
                    let applies = match target_player {
                        None => true, // owner_player omitted — every active orbit/party camera
                        Some(player) => targets.0.contains(&player),
                    };
                    if !applies {
                        continue;
                    }
                    commands.entity(camera_entity).insert(
                        crate::capabilities::camera::CameraShakeState {
                            remaining: duration_secs,
                            duration: duration_secs,
                            intensity,
                        },
                    );
                    found = true;
                }
                if !found && target_player.is_none() {
                    warn!("Action::CameraShake: no orbit camera in scene — shake ignored");
                } else if !found {
                    warn!(
                        "Action::CameraShake: owner_player {:?} owns no orbit/party camera in \
                         scene — shake ignored",
                        owner_player
                    );
                }
            }
            Action::SetCameraMode { mode, owner_player } => {
                info!("Action::SetCameraMode: mode='{}' owner_player={:?}", mode, owner_player);

                // Resolve `mode` against the scene's camera_modes registry — except "default",
                // which resolves per-camera below (each target restores its OWN authored mode,
                // not one shared value).
                let registry_mode: Option<CameraModeDef> = if mode == "default" {
                    None
                } else {
                    match scene_state.loaded_camera_modes.0.get(&mode) {
                        Some(m) => Some(m.clone()),
                        None => {
                            warn!(
                                "Action::SetCameraMode: no camera mode named '{}' in this scene's \
                                 camera_modes registry (and it isn't \"default\") — ignoring",
                                mode
                            );
                            continue;
                        }
                    }
                };

                // Resolve owner_player -> target camera entities. Omitted = every active camera
                // EXCEPT a shared Party camera (see below); Some(n) = only player n's own
                // single-owner camera(s), warn+no-op for every other case in the targeting table
                // (unjoined seat, out-of-range index, or player n owns only a shared Party camera
                // — no single per-player camera to retarget there).
                let targets: Vec<Entity> = match owner_player {
                    // A Party-authored camera can never round-trip through apply_camera_mode
                    // (which rejects Party as an unreachable target — see its own doc), so a
                    // "default" restore on one would resolve back to Party and fail forever,
                    // permanently stuck on whatever preset it was switched to and permanently
                    // breaking dynamic_split_screen_system's is_active toggling (its party_camera
                    // query requires PartyCameraMode, which the failed switch already removed).
                    // Excluding it here — rather than trying to recover after the fact — is what
                    // keeps every camera this action ever touches switchable back to "default".
                    None => scene_state.all_cameras.iter()
                        .filter(|(_, _, authored, ..)| !matches!(authored.0, CameraModeDef::Party(_)))
                        .map(|(e, ..)| e)
                        .collect(),
                    Some(n) => {
                        let Some(player_entity) = resolve_player_entity_by_index(n, &scene_state.player_targets) else {
                            warn!(
                                "Action::SetCameraMode: owner_player {} has no live player entity \
                                 (not yet joined, or out of range) — ignoring",
                                n
                            );
                            continue;
                        };
                        // Filtered to single-owner cameras only — a split.dynamic scene's player
                        // owns BOTH their own split camera (CameraTargets = [self]) and the
                        // shared merged party camera (CameraTargets = [p0, p1, ...]); rejecting
                        // the whole action just because ONE owned camera happens to be shared
                        // would make owner_player targeting unreachable in every split.dynamic
                        // scene, the exact scenario CameraModeOverride was built for.
                        let matching: Vec<Entity> = scene_state.all_cameras.iter()
                            .filter(|(_, targets, ..)| targets.0.len() == 1 && targets.0.contains(&player_entity))
                            .map(|(e, ..)| e)
                            .collect();
                        if matching.is_empty() {
                            warn!(
                                "Action::SetCameraMode: owner_player {} owns no single-owner \
                                 camera in this scene (either no camera at all, or only a shared \
                                 Party camera with no single per-player camera to retarget) — \
                                 ignoring",
                                n
                            );
                            continue;
                        }
                        matching
                    }
                };
                if targets.is_empty() && owner_player.is_none() {
                    // Every camera in scope was excluded (no camera at all, or every camera is a
                    // shared Party camera) — without this, a scene with no AuthoredCameraMode-
                    // bearing camera in it (e.g. a standalone flycam-tagged spawn missing the
                    // component) would silently do nothing with zero diagnostic.
                    warn!("Action::SetCameraMode: no switchable camera found in this scene — ignoring");
                    continue;
                }

                for camera_entity in targets {
                    let Ok((_, camera_targets, authored, transform, projection)) =
                        scene_state.all_cameras.get(camera_entity) else { continue };
                    let resolved_mode = match &registry_mode {
                        Some(m) => m.clone(),
                        None => authored.0.clone(), // "default"
                    };
                    let from_translation = transform.translation;
                    let from_rotation = transform.rotation;
                    let from_fov = match projection {
                        Some(Projection::Perspective(persp)) => persp.fov.to_degrees(),
                        _ => 45.0, // matches PerspectiveProjection::default() / this codebase's default_fov()
                    };
                    let transition = resolved_mode.transition().cloned();
                    let owner_inputs = camera_targets.0.first()
                        .and_then(|owner| scene_state.character_controllers.get(*owner).ok())
                        .map(|cc| &cc.inputs);
                    let Some(new_fov) = apply_camera_mode(&mut commands, camera_entity, from_rotation, &resolved_mode, owner_inputs) else {
                        continue; // Party(...) rejected inside apply_camera_mode; already warned there
                    };
                    // A registry preset switch marks this camera "overridden" so
                    // dynamic_split_screen_system suspends its automatic merge/split is_active
                    // toggling on it; restoring "default" clears the marker again.
                    if registry_mode.is_some() {
                        commands.entity(camera_entity).insert(crate::capabilities::camera::CameraModeOverride);
                    } else {
                        commands.entity(camera_entity).remove::<crate::capabilities::camera::CameraModeOverride>();
                    }
                    match transition {
                        Some(t) => {
                            commands.entity(camera_entity).insert(CameraBlendState {
                                remaining: t.duration_secs,
                                duration: t.duration_secs,
                                ease: t.ease,
                                from_translation,
                                from_rotation,
                                from_fov,
                                to_fov: new_fov,
                            });
                        }
                        None => {
                            // Instant cut — cancel any transition already in progress on this
                            // camera rather than letting it keep blending toward a now-stale target.
                            commands.entity(camera_entity).remove::<CameraBlendState>();
                        }
                    }
                }
            }
            Action::StartDialogue { npc_id, dialogue_path } => {
                info!("Action::StartDialogue: npc='{}' path='{}'", npc_id, dialogue_path);
                let resolved = resolve_project_path(&project_root.0, &dialogue_path);
                let handle: Handle<crate::schema::dialogue::DialogueDef> = asset_server.load(resolved);
                scene_state.active_dialogue.npc_id = npc_id.clone();
                scene_state.active_dialogue.dialogue_path = dialogue_path.clone();
                scene_state.active_dialogue.current_node_index = 0;
                scene_state.active_dialogue.last_rendered_node = None;
                scene_state.active_dialogue.auto_advance_timer = None;
                scene_state.active_dialogue.handle = Some(handle);
                game_events.write(GameEvent::Trigger(format!("dialogue.started:{}", npc_id)));
            }
            Action::AdvanceDialogue => {
                if scene_state.active_dialogue.is_active() {
                    info!("Action::AdvanceDialogue");
                    scene_state.active_dialogue.current_node_index += 1;
                    scene_state.active_dialogue.last_rendered_node = None;
                    scene_state.active_dialogue.auto_advance_timer = None;
                } else {
                    warn!("Action::AdvanceDialogue: no dialogue active — ignored");
                }
            }
            Action::EndDialogue => {
                if scene_state.active_dialogue.is_active() {
                    let path = scene_state.active_dialogue.dialogue_path.clone();
                    info!("Action::EndDialogue: closing '{}'", path);
                    scene_state.active_dialogue.clear();
                    game_events.write(GameEvent::Trigger(format!("dialogue.ended:{}", path)));
                }
            }
            Action::AddItem { entity: entity_id, item_key, count } => {
                use crate::capabilities::inventory::add_to_slots;
                let catalog_ref = scene_state.loaded_item_catalog.0.as_ref();
                if entity_id == "player" {
                    let inv = &mut *scene_state.player_inventory;
                    if inv.max_slots == 0 { inv.resize(20); }
                    let (added, full) = add_to_slots(&mut inv.slots, inv.max_slots, &item_key, count, catalog_ref);
                    if added > 0 {
                        game_events.write(GameEvent::Trigger(
                            format!("inventory.added:player:{}:{}", item_key, added)));
                    }
                    if full {
                        game_events.write(GameEvent::Trigger("inventory.full:player".to_string()));
                    }
                } else {
                    let mut found = false;
                    for (sid, mut inv) in scene_state.container_inventories.iter_mut() {
                        if sid.0 == entity_id {
                            found = true;
                            let max = inv.max_slots;
                            let (added, full) = add_to_slots(&mut inv.slots, max, &item_key, count, catalog_ref);
                            if added > 0 {
                                game_events.write(GameEvent::Trigger(
                                    format!("inventory.added:{}:{}:{}", entity_id, item_key, added)));
                            }
                            if full {
                                game_events.write(GameEvent::Trigger(
                                    format!("inventory.full:{}", entity_id)));
                            }
                            break;
                        }
                    }
                    if !found {
                        warn!("Action::AddItem: entity '{}' not found or has no Inventory", entity_id);
                    }
                }
            }
            Action::RemoveItem { entity: entity_id, item_key, count } => {
                use crate::capabilities::inventory::remove_from_slots;
                if entity_id == "player" {
                    let inv = &mut *scene_state.player_inventory;
                    let removed = remove_from_slots(&mut inv.slots, &item_key, count);
                    if removed > 0 {
                        game_events.write(GameEvent::Trigger(
                            format!("inventory.removed:player:{}:{}", item_key, removed)));
                    }
                } else {
                    let mut found = false;
                    for (sid, mut inv) in scene_state.container_inventories.iter_mut() {
                        if sid.0 == entity_id {
                            found = true;
                            let removed = remove_from_slots(&mut inv.slots, &item_key, count);
                            if removed > 0 {
                                game_events.write(GameEvent::Trigger(
                                    format!("inventory.removed:{}:{}:{}", entity_id, item_key, removed)));
                            }
                            break;
                        }
                    }
                    if !found {
                        warn!("Action::RemoveItem: entity '{}' not found or has no Inventory", entity_id);
                    }
                }
            }
            Action::TransferItem { from, to, item_key, count } => {
                use crate::capabilities::inventory::{add_to_slots, remove_from_slots};
                let catalog_ref = scene_state.loaded_item_catalog.0.as_ref();

                // Step 1: remove from source.
                let removed = if from == "player" {
                    let inv = &mut *scene_state.player_inventory;
                    remove_from_slots(&mut inv.slots, &item_key, count)
                } else {
                    let mut r = 0u32;
                    for (sid, mut inv) in scene_state.container_inventories.iter_mut() {
                        if sid.0 == from {
                            r = remove_from_slots(&mut inv.slots, &item_key, count);
                            break;
                        }
                    }
                    r
                };

                if removed == 0 {
                    warn!("Action::TransferItem: no '{}' found in '{}'", item_key, from);
                } else {
                    // Step 2: add to destination.
                    if to == "player" {
                        let inv = &mut *scene_state.player_inventory;
                        if inv.max_slots == 0 { inv.resize(20); }
                        let max = inv.max_slots;
                        add_to_slots(&mut inv.slots, max, &item_key, removed, catalog_ref);
                    } else {
                        for (sid, mut inv) in scene_state.container_inventories.iter_mut() {
                            if sid.0 == to {
                                let max = inv.max_slots;
                                add_to_slots(&mut inv.slots, max, &item_key, removed, catalog_ref);
                                break;
                            }
                        }
                    }
                    game_events.write(GameEvent::Trigger(
                        format!("inventory.transferred:{}:{}:{}", from, to, item_key)));
                }
            }
            Action::OpenInventory => {
                for (_, mut vis) in scene_state.inventory_panel_q.iter_mut() {
                    if *vis != Visibility::Visible { *vis = Visibility::Visible; }
                }
                scene_state.inventory_ui.set_panel_open(true);
                game_events.write(GameEvent::Trigger("ui.panel_opened".to_string()));
            }
            Action::CloseInventory => {
                for (_, mut vis) in scene_state.inventory_panel_q.iter_mut() {
                    if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
                }
                scene_state.inventory_ui.set_panel_open(false);
                game_events.write(GameEvent::Trigger("ui.panel_closed".to_string()));
            }
            Action::ToggleInventory => {
                let is_visible = {
                    scene_state.inventory_panel_q.iter()
                        .next()
                        .map(|(_, vis)| *vis == Visibility::Visible)
                        .unwrap_or(false)
                };
                let target = if is_visible { Visibility::Hidden } else { Visibility::Visible };
                for (_, mut vis) in scene_state.inventory_panel_q.iter_mut() {
                    if *vis != target { *vis = target; }
                }
                scene_state.inventory_ui.set_panel_open(!is_visible);
                game_events.write(GameEvent::Trigger(if is_visible {
                    "ui.panel_closed".to_string()
                } else {
                    "ui.panel_opened".to_string()
                }));
            }
            Action::OpenShop(merchant_id) => {
                // Resolve merchant's PrefabKey from the SpawnRegistry.
                let entity_opt = spawn_params.registry.entities.get(&merchant_id).copied();
                let prefab_key_opt = entity_opt.and_then(|e| {
                    scene_state.prefab_keys.get(e).ok().map(|k| k.0.clone())
                });
                let merchant_def = prefab_key_opt.and_then(|key| {
                    spawn_params.prefab_catalog.0.prefabs.get(&key)
                        .and_then(|p| p.merchant.clone())
                });

                let Some(merchant_def) = merchant_def else {
                    warn!("Action::OpenShop: entity '{}' not found or has no MerchantDef", merchant_id);
                    continue;
                };

                // Find the ShopPanel entity, make it visible, read font_size.
                let shop_panel_data = scene_state.shop_panel_q.iter_mut().next()
                    .map(|(e, mut vis, marker)| {
                        if *vis != Visibility::Visible { *vis = Visibility::Visible; }
                        (e, marker.font_size)
                    });

                let Some((shop_entity, font_size)) = shop_panel_data else {
                    warn!("Action::OpenShop: no ShopPanel in scene — add a ShopPanel UI node");
                    continue;
                };
                // Same guard as Action::OpenContainer (see its comment): the single ShopPanel can
                // only ever show one merchant at a time, so a second OpenShop while one is already
                // active (e.g. two interactable merchants both in range of one interact press)
                // must not double-count panels_open — otherwise the one matching CloseShop only
                // brings it back to 1, permanently suppressing interact/collectible-pickup/
                // tab-targeting until the next LoadScene.
                if scene_state.inventory_ui.active_merchant_id.is_none() {
                    scene_state.inventory_ui.set_panel_open(true);
                    game_events.write(GameEvent::Trigger("ui.panel_opened".to_string()));
                }

                // Track active merchant so BuyItem knows where to look up prices.
                scene_state.inventory_ui.active_merchant_id = Some(merchant_id.clone());

                // Populate only the entries container — the header + close button live above it.
                let entries_entity = scene_state.shop_entries_q.iter()
                    .find(|(_, child_of)| child_of.parent() == shop_entity)
                    .map(|(e, _)| e);
                let Some(entries_entity) = entries_entity else {
                    warn!("Action::OpenShop: ShopPanel has no entries container — scene may need a rebuild");
                    continue;
                };

                let catalog = &scene_state.loaded_item_catalog.0;
                let panel_icon_sheet = scene_state.inventory_ui.panel_icon_sheet.clone();
                commands.entity(entries_entity).despawn_children();
                for entry in &merchant_def.stock {
                    let display_name = catalog
                        .as_ref()
                        .and_then(|c| c.items.get(&entry.item_key))
                        .map(|d| d.display_name.clone())
                        .unwrap_or_else(|| entry.item_key.clone());
                    let stock_label = entry.stock_count
                        .map(|n| format!(" [{}]", n))
                        .unwrap_or_default();

                    // Resolve icon data for this entry (smaller than inventory slots).
                    let item_def = catalog.as_ref().and_then(|c| c.items.get(&entry.item_key));
                    let icon_index = item_def.map(|d| d.icon_index as usize).unwrap_or(0);
                    let icon_color = item_def
                        .and_then(|d| d.icon_color)
                        .map(|(r, g, b, a)| Color::srgba(r, g, b, a))
                        .unwrap_or(Color::WHITE);
                    let sheet_key: Option<String> = item_def
                        .and_then(|d| d.icon_sheet.clone())
                        .or_else(|| panel_icon_sheet.clone());
                    // Clone handles so the borrow on inventory_ui ends before with_children.
                    let atlas_pair: Option<(Handle<Image>, Handle<TextureAtlasLayout>)> = sheet_key
                        .as_deref()
                        .and_then(|k| scene_state.inventory_ui.icon_atlases.get(k))
                        .map(|(t, l)| (t.clone(), l.clone()));

                    let item_key = entry.item_key.clone();
                    let buy_price = entry.buy_price;
                    commands.entity(entries_entity).with_children(|p| {
                        p.spawn((
                            Name::new("ShopEntryRow"),
                            Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
                                ..default()
                            },
                        )).with_children(|row| {
                            // Item icon (28×28, smaller than inventory slots).
                            if let Some((tex, layout)) = atlas_pair {
                                row.spawn((
                                    Name::new("ShopEntryIcon"),
                                    Node {
                                        width: Val::Px(28.0),
                                        height: Val::Px(28.0),
                                        flex_shrink: 0.0,
                                        margin: UiRect::right(Val::Px(6.0)),
                                        ..default()
                                    },
                                    ImageNode {
                                        image: tex,
                                        texture_atlas: Some(TextureAtlas { layout, index: icon_index }),
                                        color: icon_color,
                                        ..default()
                                    },
                                ));
                            }
                            // Item name + optional stock count.
                            row.spawn((
                                Name::new("ShopEntryName"),
                                Node { flex_grow: 1.0, ..default() },
                                Text::new(format!("{}{}", display_name, stock_label)),
                                TextFont { font_size, ..default() },
                                TextColor(Color::srgba(0.90, 0.88, 0.78, 1.0)),
                            ));
                            // Buy price (gold tint).
                            row.spawn((
                                Name::new("ShopEntryPrice"),
                                Node { margin: UiRect::axes(Val::Px(8.0), Val::Px(0.0)), ..default() },
                                Text::new(format!("{} g", buy_price)),
                                TextFont { font_size, ..default() },
                                TextColor(Color::srgba(0.95, 0.85, 0.40, 1.0)),
                            ));
                            // Buy button (bigger padding, full font_size).
                            row.spawn((
                                Name::new("ShopBuyBtn"),
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BorderColor::from(Color::srgba(0.2, 0.5, 0.2, 0.8)),
                                BackgroundColor(Color::srgba(0.08, 0.20, 0.08, 0.85)),
                                UiAction::Trigger(format!("buy_item:{}", item_key)),
                            )).with_children(|b| {
                                b.spawn((
                                    Name::new("BuyBtnText"),
                                    Text::new("Buy"),
                                    TextFont { font_size, ..default() },
                                    TextColor(Color::srgba(0.60, 0.90, 0.60, 1.0)),
                                ));
                            });
                        });
                    });
                }
            }
            Action::CloseShop => {
                scene_state.inventory_ui.active_merchant_id = None;
                for (_, mut vis, _) in scene_state.shop_panel_q.iter_mut() {
                    if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
                }
                scene_state.inventory_ui.set_panel_open(false);
                game_events.write(GameEvent::Trigger("ui.panel_closed".to_string()));
            }
            Action::BuyItem(item_key) => {
                use crate::capabilities::inventory::add_to_slots;

                // Find active merchant.
                let merchant_id = match scene_state.inventory_ui.active_merchant_id.clone() {
                    Some(id) => id,
                    None => { warn!("Action::BuyItem: no shop is currently open"); continue; }
                };

                // Look up merchant def.
                let entity_opt = spawn_params.registry.entities.get(&merchant_id).copied();
                let prefab_key_opt = entity_opt.and_then(|e| {
                    scene_state.prefab_keys.get(e).ok().map(|k| k.0.clone())
                });
                let merchant_def = prefab_key_opt.and_then(|key| {
                    spawn_params.prefab_catalog.0.prefabs.get(&key)
                        .and_then(|p| p.merchant.clone())
                });
                let Some(merchant_def) = merchant_def else {
                    warn!("Action::BuyItem: active merchant '{}' has no MerchantDef", merchant_id);
                    continue;
                };

                // Find item in stock.
                let Some(stock_entry) = merchant_def.stock.iter().find(|e| e.item_key == item_key) else {
                    warn!("Action::BuyItem: item '{}' not in merchant '{}' stock", item_key, merchant_id);
                    continue;
                };

                // Check stock (stock_count: Some(0) means sold out).
                if stock_entry.stock_count == Some(0) {
                    info!("Action::BuyItem: item '{}' is sold out", item_key);
                    continue;
                }

                // Resolve display name for floating text.
                let display_name = scene_state.loaded_item_catalog.0.as_ref()
                    .and_then(|c| c.items.get(&item_key))
                    .map(|d| d.display_name.clone())
                    .unwrap_or_else(|| item_key.clone());

                let player_id = scene_state.player_inventory.player_spawn_id.clone();

                // Check and deduct currency (global stat).
                let currency = &merchant_def.currency_stat;
                let price = stock_entry.buy_price as f32;
                let current = scene_state.loaded_stats.0.get(currency.as_str())
                    .map(|s| s.current)
                    .unwrap_or(0.0);
                if current < price {
                    info!("Action::BuyItem: not enough {} ({:.0} < {:.0})", currency, current, price);
                    game_events.write(GameEvent::Trigger("shop.insufficient_funds".to_string()));
                    if let Some(ref pid) = player_id {
                        action_queue.push(Action::ShowFloatingText {
                            entity: pid.clone(),
                            text: "Not enough gold!".to_string(),
                            offset: Some((0.0, 2.2, 0.0)),
                        });
                    }
                    continue;
                }
                if let Some(stat) = scene_state.loaded_stats.0.get_mut(currency.as_str()) {
                    stat.apply_delta(-price);
                }

                // Add item to player inventory.
                let catalog_ref = scene_state.loaded_item_catalog.0.as_ref();
                let inv = &mut *scene_state.player_inventory;
                if inv.max_slots == 0 { inv.resize(20); }
                let max = inv.max_slots;
                add_to_slots(&mut inv.slots, max, &item_key, 1, catalog_ref);

                info!("Action::BuyItem: bought '{}' for {} {}", item_key, price, currency);
                game_events.write(GameEvent::Trigger(format!("item.bought:{}", item_key)));
                if let Some(ref pid) = player_id {
                    action_queue.push(Action::ShowFloatingText {
                        entity: pid.clone(),
                        text: format!("Bought {}!", display_name),
                        offset: Some((0.0, 2.2, 0.0)),
                    });
                }
            }
            Action::OpenContainer(entity_id) => {
                // Verify a ContainerPanel exists in the scene.
                if scene_state.container_panel_q.iter().next().is_none() {
                    warn!("Action::OpenContainer: no ContainerPanel in scene — add a ContainerPanel UI node");
                    continue;
                }

                // Resolve spawn ID → ECS entity.
                let entity_opt = spawn_params.registry.entities.get(&entity_id).copied();
                let Some(container_entity) = entity_opt else {
                    warn!("Action::OpenContainer: entity '{}' not found", entity_id);
                    continue;
                };

                // Verify it has an Inventory component.
                if scene_state.container_inventories.get(container_entity).is_err() {
                    warn!("Action::OpenContainer: entity '{}' has no Inventory component", entity_id);
                    continue;
                }

                // Show the container panel.
                for (_, mut vis) in scene_state.container_panel_q.iter_mut() {
                    if *vis != Visibility::Visible { *vis = Visibility::Visible; }
                }

                // Only count this as a *new* panel opening if none was already open. The
                // ContainerPanel is a single UI node with one `active_container` slot — a second
                // OpenContainer while one is already showing just re-targets that same slot, it
                // doesn't add a second visible panel. Without this guard, two interactable entities
                // both in range of one interact press (each queuing its own OpenContainer in the
                // same frame — e.g. two nearby lootable corpses) would increment `panels_open`
                // twice for one visual panel, and the single matching CloseContainer that follows
                // only brings it back to 1 — permanently suppressing interact/collectible
                // pickup/tab-targeting (all gated on `panels_open == 0`) until the next LoadScene.
                if scene_state.container_ui.active_container.is_none() {
                    scene_state.inventory_ui.set_panel_open(true);
                    game_events.write(GameEvent::Trigger("ui.panel_opened".to_string()));
                }
                scene_state.container_ui.active_container = Some(container_entity);
                game_events.write(GameEvent::Trigger(format!("container.opened:{}", entity_id)));
            }
            Action::CloseContainer => {
                for (_, mut vis) in scene_state.container_panel_q.iter_mut() {
                    if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
                }
                scene_state.inventory_ui.set_panel_open(false);
                game_events.write(GameEvent::Trigger("ui.panel_closed".to_string()));
                scene_state.container_ui.active_container = None;
                game_events.write(GameEvent::Trigger("container.closed".to_string()));
            }
            Action::TakeAllFromContainer => {
                use crate::capabilities::inventory::{add_to_slots, remove_from_slots};
                let catalog_ref = scene_state.loaded_item_catalog.0.as_ref();
                let Some(container_entity) = scene_state.container_ui.active_container else {
                    warn!("Action::TakeAllFromContainer: no container is open");
                    continue;
                };

                // Collect all items from the container.
                let items_to_transfer: Vec<(String, u32)> = {
                    if let Ok((_, inv)) = scene_state.container_inventories.get(container_entity) {
                        inv.slots.iter()
                            .filter_map(|s| s.as_ref().map(|stack| (stack.item_key.clone(), stack.count)))
                            .collect()
                    } else {
                        warn!("Action::TakeAllFromContainer: container entity has no Inventory");
                        continue;
                    }
                };

                if items_to_transfer.is_empty() { continue; }

                // Remove from container; currency items go to their stat, others to inventory.
                let mut any_transferred = false;
                for (item_key, count) in &items_to_transfer {
                    if let Ok((sid, mut inv)) = scene_state.container_inventories.get_mut(container_entity) {
                        let removed = remove_from_slots(&mut inv.slots, item_key, *count);
                        if removed > 0 {
                            let currency_stat = catalog_ref
                                .and_then(|c| c.items.get(item_key.as_str()))
                                .and_then(|def| def.currency_stat.clone());
                            if let Some(ref stat_key) = currency_stat {
                                if let Some(stat) = scene_state.loaded_stats.0.get_mut(stat_key) {
                                    let new_val = stat.apply_delta(removed as f32);
                                    info!("TakeAllFromContainer: {} x{} → stat \"{}\" now {:.0}", item_key, removed, stat_key, new_val);
                                } else {
                                    warn!("TakeAllFromContainer: currency_stat {:?} not found in stats", stat_key);
                                }
                            } else {
                                let player = &mut *scene_state.player_inventory;
                                if player.max_slots == 0 { player.resize(20); }
                                let max = player.max_slots;
                                add_to_slots(&mut player.slots, max, item_key, removed, catalog_ref);
                            }
                            game_events.write(GameEvent::Trigger(
                                format!("inventory.added:player:{}:{}", item_key, removed)));
                            any_transferred = true;
                            let _ = sid; // suppress unused warning
                        }
                    }
                }

                if any_transferred {
                    // Resolve entity ID from registry for the event.
                    let entity_id = spawn_params.registry.entities.iter()
                        .find_map(|(id, &e)| if e == container_entity { Some(id.clone()) } else { None })
                        .unwrap_or_default();
                    game_events.write(GameEvent::Trigger(format!("container.looted:{}", entity_id)));
                }
            }
            Action::JoinPlayer => {
                // Slot-assignment correctness here depends on three invariants that live in
                // different files — noted together so a future change to any one of them doesn't
                // silently corrupt slot assignment: (1) `action_executor_system` always runs
                // before `drain_spawn_queue_system` within a frame (the `.chain()` in `lib.rs`),
                // so `active_split_slot_count` reflects only *already-drained* joins, never one
                // still sitting in `pending_spawns`; (2) `pending_spawns` is cleared synchronously
                // (not deferred) on `Action::LoadScene` above, so a stale `is_hot_join` count can't
                // survive a scene transition; (3) `queued_hot_joins` below counts *only*
                // `is_hot_join` entries, so a concurrently-queued `Action::Spawn` (NPC, etc.)
                // never perturbs the next-slot computation.
                //
                // `ActiveSplitSlotCount` is `Some` only while the scene is currently
                // `Grid`-split (see its own doc comment) — every other split mode
                // (party/dynamic/Vertical/Horizontal) and single-camera scenes read `None` here
                // and are correctly rejected without needing their own separate checks.
                let Some(slot_count) = spawn_params.active_split_slot_count.0 else {
                    warn!(
                        "Action::JoinPlayer: current scene is not Grid-split — party, dynamic, \
                         Vertical/Horizontal split, and single-camera scenes don't support \
                         hot-join in v1. Ignoring."
                    );
                    continue;
                };
                // Same-frame double-join safety: count already-queued-but-not-yet-drained
                // is_hot_join entries too, not just the live (already-flushed) slot count —
                // two JoinPlayer actions processed in this same executor pass must not both
                // compute the same next slot.
                let queued_hot_joins = spawn_params.pending_spawns.0.iter()
                    .filter(|q| q.is_hot_join)
                    .count() as u32;
                let next_slot = slot_count + queued_hot_joins;
                if next_slot >= MAX_SPLIT_PLAYERS {
                    warn!(
                        "Action::JoinPlayer: scene is already at MAX_SPLIT_PLAYERS ({}) — ignoring join.",
                        MAX_SPLIT_PLAYERS
                    );
                    continue;
                }
                let scene = spawn_params.scene_handle.as_ref()
                    .and_then(|h| spawn_params.scenes.get(&h.0));
                let Some(prefab_key) = scene.and_then(|s| s.join_prefab_keys.get(next_slot as usize))
                    .cloned()
                    .flatten()
                else {
                    warn!(
                        "Action::JoinPlayer: scene has no join_prefab_keys entry for slot {} — \
                         ignoring join. Add a join_prefab_keys entry for this slot to enable \
                         joining.",
                        next_slot
                    );
                    continue;
                };
                let Some(prefab_def) = spawn_params.prefab_catalog.0.prefabs.get(&prefab_key) else {
                    warn!("Action::JoinPlayer: join prefab '{}' not found in catalog", prefab_key);
                    continue;
                };
                // Same two guards as Action::Spawn above, for the same two reasons: (1) a
                // join_prefab_keys entry pointing at a non-`tags: ["player"]` prefab would
                // otherwise be silently assembled into a PlayerConfig and spawned as a player
                // (camera, controller, split slot consumed) — clearly not what a designer meant.
                // (2) a primitive-shaped player prefab with a resolvable `model` key would sail
                // past the asset_catalog lookup below, get assembled with
                // `PlayerModelSource::Primitive`, and panic in `spawn_player_entity_core` — the
                // hot-join drain branch always passes `None` for `PrimitivePlayerCtx` (GLB-only
                // in v1, debug-detective finding, see local_coop_hot_join_leave.md).
                if !prefab_def.components.tags.iter().any(|t| t == "player") {
                    warn!(
                        "Action::JoinPlayer: join prefab '{}' has no `tags: [\"player\"]` — \
                         refusing to hot-join a non-player prefab. Fix join_prefab_keys to \
                         point at a player prefab.",
                        prefab_key
                    );
                    continue;
                }
                if prefab_def.kind == crate::schema::catalog::PrefabKind::Primitive {
                    warn!(
                        "Action::JoinPlayer: join prefab '{}' is primitive-shaped (kind: \
                         Primitive) — hot-join only supports GLB (Actor-kind) players in v1. \
                         Use a GLB player prefab in join_prefab_keys instead.",
                        prefab_key
                    );
                    continue;
                }
                let Some(model_entry) = asset_catalog.0.models.get(&prefab_def.model) else {
                    warn!(
                        "Action::JoinPlayer: model key {:?} not found in asset catalog",
                        prefab_def.model
                    );
                    continue;
                };
                let model_path = model_entry.path.clone();
                let prefab_def = prefab_def.clone();

                spawn_params.registry.counter += 1;
                let spawn_id = format!("{}_{}", prefab_key, spawn_params.registry.counter);

                // `player_N_start` is 1-based everywhere else in the project (room6/room7/room8
                // and every doc example) — `next_slot` is the 0-based absolute slot number, so
                // it must be offset by 1 here or a joiner silently lands on an existing player's
                // spawn point instead of their own (alignment-reviewer finding).
                let spawn_point_key = format!("player_{}_start", next_slot + 1);
                let (sx, sy, sz) = match spawn_params.spawn_points.0.get(spawn_point_key.as_str()) {
                    Some(&pt) => pt,
                    None => {
                        let primary_pos = scene_state.player_targets.iter()
                            .find(|(_, _, idx)| crate::capabilities::targeting::is_primary_player(*idx))
                            .and_then(|(e, _, _)| scene_state.global_transforms.get(e).ok())
                            .map(|gt| gt.translation())
                            .unwrap_or(Vec3::ZERO);
                        let nudged = primary_pos + Vec3::new(1.5 * next_slot as f32, 0.0, 0.0);
                        (nudged.x, nudged.y, nudged.z)
                    }
                };

                let mut player_config = assemble_player_config(
                    &prefab_def,
                    &prefab_key,
                    &spawn_id,
                    Some(model_path.clone()),
                    (sx, sy, sz),
                    spawn_params.nameplate_config.player_enabled,
                );
                player_config.player_index = next_slot;

                // Gamepad-triggered join: bind the specific pad that pressed the join button
                // directly to this player's `PlayerConfig.bound_gamepad` — no round-trip through
                // `inputs.gamepad_index` (a sorted-position *seed*, re-resolved by
                // `gamepad_bind_system` a frame later against a possibly-different sorted slice,
                // which could silently rebind to the wrong pad). A keyboard-triggered join (or any
                // frame with no captured pad) leaves this `None`, so the join prefab's own
                // `gamepad_index` seed (if any) still resolves normally through the pending-bind
                // path. `take()` both reads and clears the value, so it can't be reused by a
                // second JoinPlayer processed later in this same executor pass. See
                // `planning/features/gamepad_player_binding_hardening.md`.
                player_config.bound_gamepad = spawn_params.pending_join_gamepad.0.take();

                info!(
                    "Action::JoinPlayer: queued join for slot {} (prefab '{}') at ({:.1}, {:.1}, {:.1})",
                    next_slot, prefab_key, sx, sy, sz
                );

                spawn_params.pending_spawns.0.push_back(super::QueuedSpawn {
                    prefab_def,
                    model_path,
                    transform: Transform::from_xyz(sx, sy, sz),
                    spawn_id,
                    prefab_key: prefab_key.clone(),
                    project_root: project_root.0.clone(),
                    player_config: Some(player_config),
                    is_hot_join: true,
                });

                if next_slot + 1 == MAX_SPLIT_PLAYERS {
                    game_events.write(GameEvent::Trigger("coop.lobby_full".to_string()));
                }
            }
        }
    }
}

/// Resolves `owner_player: Some(n)` (`Action::CameraShake`/`Action::SetCameraMode`, **v2**) to the
/// live player entity carrying `PlayerIndex(n)` — or, for `n == 0`, a player with no `PlayerIndex`
/// component at all, matching `targeting::is_primary_player`'s existing convention for the
/// single-player (no local-coop) case. Returns `None` if no live player currently has that index
/// (seat not yet hot-joined, or `n` is out of range) — the caller's cue to `warn!` + no-op rather
/// than silently acting on the wrong player or on nobody.
fn resolve_player_entity_by_index(
    n: u32,
    player_targets: &Query<
        (Entity, &mut crate::capabilities::player::PlayerTarget, Option<&crate::capabilities::player::PlayerIndex>),
        With<crate::capabilities::player::CharacterController>,
    >,
) -> Option<Entity> {
    player_targets.iter().find_map(|(entity, _, idx)| {
        let matches = idx.map_or(n == 0, |i| i.0 == n);
        matches.then_some(entity)
    })
}
