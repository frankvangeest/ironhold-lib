use bevy::prelude::*;
use crate::ProjectRoot;
use crate::schema::*;
use crate::schema::scene_v2::GameSceneV2;
use crate::schema::stats::{ActiveModifier, StackRule};
use crate::runtime::messages::*;
use crate::runtime::actions::ActionQueue;
use crate::capabilities::animation_resolver::AnimationRequests;
use crate::capabilities::damage_popup::DamagePopup;
use crate::capabilities::particle::QueuedParticleEffect;
use super::{
    BackgroundMusic, LevelEntity, LoadedAssetCatalog, OverlayEntity, PendingSceneLoadMode,
    SceneHandleV2, SceneStateParams, SpawnParams, SpawnId, WorldLabel, resolve_project_path,
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
                scene_state.delayed_events.0.clear();
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

                if position.is_some() && spawn_point.is_some() {
                    warn!(
                        "Action::Spawn '{}': both position and spawn_point are set; position wins",
                        spawn_id
                    );
                }
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
            Action::PlayMusicLoop { key, volume: action_volume } => {
                // Stop any currently playing background music.
                for entity in bg_music_query.iter() {
                    commands.entity(entity).despawn();
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
                let linear = (pct.min(100) as f32) / 100.0;
                info!("Action::SetVolume: {}% (linear {:.2})", pct, linear);
                if let Some(ref mut gv) = global_volume {
                    gv.volume = bevy::audio::Volume::Linear(linear);
                } else {
                    warn!("Action::SetVolume: GlobalVolume resource not available");
                }
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
                        commands.spawn((
                            Text2d::new(text),
                            TextFont { font_size: style.font_size, ..default() },
                            TextColor(Color::srgba(r, g, b, a)),
                            Transform::from_xyz(0.0, 0.0, 10.0),
                            WorldLabel {
                                world_pos: Vec3::ZERO,
                                tracked_entity: Some(e),
                                offset: Vec3::new(ox, oy, oz),
                                base_font_size: style.font_size,
                                depth_scale: None,
                            },
                            DamagePopup { elapsed: 0.0, duration: style.duration_secs, rise_speed: style.rise_speed },
                            LevelEntity,
                        ));
                    } else {
                        warn!("Action::ShowDamagePopup: entity '{}' has no GlobalTransform", entity_id);
                    }
                } else {
                    warn!("Action::ShowDamagePopup: entity '{}' not found in spawn registry", entity_id);
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
        }
    }
}
