use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use bevy_rapier3d::prelude::{
    Collider, RigidBody, LockedAxes, Damping, Velocity, ExternalImpulse,
    Friction, CoefficientCombineRule, Sensor, ActiveEvents,
};
use crate::schema::*;
use crate::schema::catalog::{PrefabKind, PrimitiveShapeKind};
use crate::schema::scene_v2::GameSceneV2;
use crate::schema::player::PlayerConfig;
use crate::runtime::messages::*;
use crate::runtime::material_factory::MaterialFactory;
use crate::capabilities::player::CharacterController;
use crate::capabilities::camera::OrbitCamera;
use crate::capabilities::animation_resolver::{AnimationRequests, LocomotionState, ActiveOverride};
use super::{
    SceneV2Params, SceneMaterialParams,
    LevelEntity, OverlayEntity, PendingSceneLoadMode,
    LoadedSpawnPoints, SpawnRegistry, MergedModelFixes,
    ProjectKeyBindings, LoadedKeyBindings, SpawnId, PrefabKey, WorldLabel,
    LoadedAudioHandles, LoadedDecalHandles, LoadedAssetCatalog,
    PendingBehavior, resolve_project_path,
};
use crate::capabilities::collectible::Collectable;
use crate::capabilities::motion::Motion;
use crate::capabilities::stat_display::{StatBarFill, StatValueText, StatLabelMarker, WorldStatBarFillMarker, WorldPixelBarFillMarker};
use crate::schema::catalog::WorldStatBarStyle;
use crate::capabilities::stat_radar::{RadarMaterial, RadarUniforms, StatRadarNode};
use crate::schema::scene_v2::BarOrientation;

const TAG_FLYCAM: &str = "flycam";
const TAG_PLAYER: &str = "player";
const TAG_COLLECTABLE: &str = "collectable";
use crate::capabilities::npc::{NpcAgent, NpcState};
use crate::capabilities::trigger_zone::TriggerZone;
use crate::capabilities::interactable::Interactable;
use crate::schema::stats::{LiveStat, StatMap};
use crate::PipelineWarmup;
use super::entity_spawner::{
    spawn_prefab_instance, spawn_player_entity, default_camera_config, default_input_map,
};

pub fn spawn_scene_v2(
    mut commands: Commands,
    params: SceneV2Params,
    asset_server: Res<AssetServer>,
    mut events: MessageReader<AssetEvent<GameSceneV2>>,
    mut next_state: ResMut<NextState<AppState>>,
    state: Res<State<AppState>>,
    level_entities: Query<Entity, With<LevelEntity>>,
    overlay_entities: Query<Entity, With<OverlayEntity>>,
    mut scene_events: MessageWriter<SceneEvent>,
    model_spawner: Res<crate::runtime::model_spawner::ModelSpawner>,
    merged_fixes: Res<MergedModelFixes>,
    mut mats: SceneMaterialParams,
    mut spawn_registry: ResMut<SpawnRegistry>,
    mut load_mode: ResMut<PendingSceneLoadMode>,
    project_key_bindings: Res<ProjectKeyBindings>,
    mut loaded_key_bindings: ResMut<LoadedKeyBindings>,
) {
    let Some(scene_handle) = params.scene_handle.as_ref() else { return; };
    let Some(project) = params.configs.get(&params.config_handle.0) else { return; };
    let is_overlay = *load_mode == PendingSceneLoadMode::Overlay;

    let mut ready_to_spawn = false;
    for event in events.read() {
        if event.is_loaded_with_dependencies(&scene_handle.0) {
            ready_to_spawn = true;
        }
    }
    let can_spawn_state = *state.get() == AppState::LoadingScene
        || *state.get() == AppState::LoadingProject
        || (is_overlay && *state.get() == AppState::InGame);
    if can_spawn_state && params.scenes.get(&scene_handle.0).is_some() {
        ready_to_spawn = true;
    }

    if !ready_to_spawn { return; }
    if *state.get() == AppState::InGame && !is_overlay { return; }

    let Some(scene) = params.scenes.get(&scene_handle.0) else { return; };

    scene_events.write(SceneEvent::Loaded(
        asset_server
            .get_path(&scene_handle.0)
            .map(|p| p.path().to_string_lossy().into_owned())
            .unwrap_or_default(),
    ));

    info!(
        "Scene V2 Loaded! name={}, {} entities, {} ui",
        scene.name,
        scene.entities.len(),
        scene.ui.len()
    );

    // Always remove any existing overlay (loading a new scene or new overlay replaces it).
    for entity in overlay_entities.iter() {
        commands.entity(entity).despawn();
    }

    if is_overlay {
        // Overlay mode: keep the game world, only spawn the UI section.
        // Reset load_mode immediately so subsequent loads default to Replace.
        *load_mode = PendingSceneLoadMode::Replace;
    } else {
        // Replace mode: tear down the existing world.
        commands.insert_resource(LoadedSpawnPoints(scene.spawn_points.clone()));
        spawn_registry.entities.clear();
        spawn_registry.counter = 0;

        let mut load_errors: Vec<String> = Vec::new();

        // Rebuild effective key bindings: project base + scene-level overrides.
        // This ensures bindings from a previous scene never bleed into the next one.
        {
            let mut effective = project_key_bindings.0.clone();
            for (key_name, trigger) in &scene.scene_key_bindings {
                if InputMap::parse_key(key_name).is_none() {
                    load_errors.push(format!(
                        "scene_key_bindings: unrecognised key name {:?} — binding will have no effect",
                        key_name
                    ));
                }
                effective.insert(key_name.clone(), trigger.clone());
            }
            *loaded_key_bindings = LoadedKeyBindings(effective);
        }

        for entity in level_entities.iter() {
            commands.entity(entity).despawn();
        }

        // Build material handles from the asset catalog for this scene.
        mats.built.0.clear();
        for (name, mat_def) in &params.asset_catalog.0.materials {
            let handle = MaterialFactory::build(
                &asset_server,
                &mut mats.standard,
                &mut mats.terrain,
                &mut mats.custom,
                name,
                mat_def,
            );
            mats.built.0.insert(name.clone(), handle);
        }
        if !params.asset_catalog.0.materials.is_empty() {
            info!("Built {} material(s) from asset catalog", params.asset_catalog.0.materials.len());
        }

        // Capture these refs before the primitive_player destructure shadows `params`.
        let prefab_catalog = &params.prefab_catalog.0;
        let asset_catalog  = &params.asset_catalog.0;
        let project_root   = params.project_root.0.as_str();

        // Spawn entities from prefabs
        let mut pending_labels: Vec<(Entity, crate::schema::scene_v2::EntityLabelDef)> = Vec::new();
        let mut pending_stat_labels: Vec<(Entity, String, crate::schema::catalog::StatLabelDef)> = Vec::new();
        let mut pending_world_bars: Vec<(Entity, String, crate::schema::catalog::WorldStatBarDef)> = Vec::new();
        let mut player_config: Option<PlayerConfig> = None;
        // A primitive prefab with tags: ["player"]: shape + params + spawn position + components.
        let mut primitive_player: Option<(String, PrimitiveShapeKind, crate::schema::catalog::PrimitiveParams, Vec3, crate::schema::catalog::PrefabComponents, Vec<crate::schema::catalog::ChildPrimitiveDef>)> = None;
        let mut flycam_start: Option<(Transform, crate::schema::catalog::FlyCamDef)> = None;
        for entity_def in &scene.entities {
            let Some(prefab) = params.prefab_catalog.0.prefabs.get(&entity_def.prefab) else {
                load_errors.push(format!(
                    "entity '{}': prefab '{}' not found in catalog, entity skipped",
                    entity_def.id, entity_def.prefab
                ));
                continue;
            };

            let is_flycam = prefab.components.tags.contains(&TAG_FLYCAM.to_string());
            let is_player = prefab.components.tags.contains(&TAG_PLAYER.to_string());

            // Build transform early — needed before model lookup so flycam can early-out.
            let t = &entity_def.transform;
            let translation = Vec3::new(t.translation.0, t.translation.1, t.translation.2);
            let rotation = Quat::from_euler(
                EulerRot::XYZ,
                t.rotation_euler_deg.0.to_radians(),
                t.rotation_euler_deg.1.to_radians(),
                t.rotation_euler_deg.2.to_radians(),
            );
            let scale = Vec3::new(t.scale.0, t.scale.1, t.scale.2);
            let transform = Transform { translation, rotation, scale };

            if is_flycam {
                // No model needed — just record the spawn transform and any tuning.
                let fc_def = prefab.components.flycam.clone().unwrap_or_default();
                flycam_start = Some((transform, fc_def));
                continue;
            }

            if prefab.kind == PrefabKind::Foliage {
                let foliage_def = prefab.foliage.clone().unwrap_or_default();
                let root = commands.spawn((
                    Name::new(entity_def.id.clone()),
                    transform,
                    Visibility::default(),
                    LevelEntity,
                    SpawnId(entity_def.id.clone()),
                    crate::capabilities::foliage::PendingFoliage(foliage_def.clone()),
                )).id();
                spawn_registry.entities.insert(entity_def.id.clone(), root);

                // Spawn trunk GLB as a child entity if defined.
                if let Some(trunk_key) = &foliage_def.trunk {
                    if let Some(catalog_entry) = params.asset_catalog.0.models.get(trunk_key) {
                        let trunk = spawn_prefab_instance(
                            &mut commands,
                            &asset_server,
                            &model_spawner,
                            &merged_fixes.0,
                            &params.project_root.0,
                            prefab,
                            catalog_entry.path.clone(),
                            Transform::IDENTITY,
                            trunk_key,
                        );
                        commands.entity(root).add_child(trunk);
                    } else {
                        load_errors.push(format!(
                            "entity '{}': foliage trunk key '{}' not found in asset catalog, trunk skipped",
                            entity_def.id, trunk_key
                        ));
                    }
                }
                continue;
            }

            if prefab.kind == PrefabKind::Primitive {
                // ── Primitive player: collect and defer; camera spawned after entity loop ──
                if is_player {
                    let p = prefab.primitive.as_ref().cloned().unwrap_or_default();
                    primitive_player = Some((entity_def.id.clone(), prefab.shape.as_ref().cloned().unwrap_or(PrimitiveShapeKind::Capsule3d), p, translation, prefab.components.clone(), prefab.children.clone()));
                    continue;
                }

                // ── Composite prefab: non-empty `children` list ───────────────────────────
                if !prefab.children.is_empty() {
                    let parent = commands.spawn((
                        Name::new(entity_def.id.clone()),
                        transform,
                        Visibility::default(),
                        LevelEntity,
                    )).id();
                    {
                        let mut ctx = ChildSpawnCtx {
                            meshes:    &mut mats.meshes,
                            standard:  &mut mats.standard,
                            built_mats: &mats.built.0,
                            custom_mats: &mats.custom,
                            primitive_default_color: project.primitive_default_color,
                            asset_server:  &asset_server,
                            model_spawner: &model_spawner,
                            fixes: &merged_fixes.0,
                            asset_catalog,
                            project_root,
                        };
                        spawn_primitive_children(
                            &mut commands, parent, &prefab.children,
                            prefab_catalog, &mut ctx,
                            &mut load_errors, &entity_def.id, 0, &mut HashSet::new(),
                        );
                    }

                    // Register composite entities in the spawn registry so that
                    // Action::Despawn can locate them by id — same as single-mesh entities.
                    commands.entity(parent).insert((
                        SpawnId(entity_def.id.clone()),
                        PrefabKey(entity_def.prefab.clone()),
                    ));
                    spawn_registry.entities.insert(entity_def.id.clone(), parent);

                    // ── NPC agent ────────────────────────────────────────────────────────
                    // Composite prefabs with an `npc` config get a dynamic physics body
                    // and an NpcAgent component so the behaviour system can drive them.
                    if let Some(npc_def) = &prefab.components.npc {
                        let p = prefab.primitive.as_ref().cloned().unwrap_or_default();
                        let cap_radius = p.radius.unwrap_or(0.4);
                        let npc_height = p.height.unwrap_or(1.8);
                        let cap_half = (npc_height / 2.0 - cap_radius).max(0.0);
                        let body_y = cap_half + cap_radius;

                        let waypoints: Vec<Vec3> = npc_def.patrol_waypoints.iter()
                            .map(|(x, y, z)| translation + Vec3::new(*x, *y, *z))
                            .collect();

                        let fov_cos = npc_def.fov_degrees
                            .map(|deg| (deg.to_radians() / 2.0).cos())
                            .unwrap_or(-1.0);

                        let initial_state = if waypoints.is_empty() {
                            NpcState::Idle
                        } else {
                            NpcState::Patrol
                        };

                        commands.entity(parent).insert((
                            NpcAgent {
                                npc_id: entity_def.id.clone(),
                                faction: npc_def.faction.clone(),
                                on_player_near: npc_def.on_player_near.clone(),
                                detection_radius: npc_def.detection_radius,
                                chase_radius: npc_def.chase_radius,
                                fov_cos,
                                requires_los: npc_def.requires_los,
                                approach_distance: npc_def.approach_distance,
                                patrol_speed: npc_def.patrol_speed,
                                chase_speed: npc_def.chase_speed,
                                waypoints,
                                current_waypoint: 0,
                                state: initial_state,
                                target: None,
                                state_timer: 0.0,
                                origin: translation,
                                eye_height: npc_def.eye_height,
                                alerted_duration: npc_def.alerted_duration,
                                drag: npc_def.drag,
                                waypoint_reach_radius: npc_def.waypoint_reach_radius,
                                interact_leave_factor: npc_def.interact_leave_factor,
                                home_arrival_radius: npc_def.home_arrival_radius,
                            },
                            RigidBody::Dynamic,
                            Collider::compound(vec![(
                                Vec3::new(0.0, body_y, 0.0),
                                Quat::IDENTITY,
                                Collider::capsule_y(cap_half, cap_radius),
                            )]),
                            LockedAxes::ROTATION_LOCKED,
                            Damping { linear_damping: npc_def.linear_damping, angular_damping: npc_def.angular_damping },
                            Velocity::default(),
                            Friction { coefficient: 0.0, combine_rule: CoefficientCombineRule::Min },
                        ));
                    }

                    // Entity FSM behavior for composite primitives.
                    if let Some(behavior_path) = &prefab.behavior {
                        let project_root = params.project_root.0.as_str();
                        let resolved = resolve_project_path(project_root, behavior_path);
                        let handle: Handle<crate::schema::project::StateMachineAsset> =
                            asset_server.load(resolved);
                        commands.entity(parent).insert(PendingBehavior(handle));
                    }

                    // Interactable: proximity + F key → entity.interacted:{id}
                    if let Some(interactable_def) = &prefab.interactable {
                        commands.entity(parent).insert(Interactable {
                            radius: interactable_def.radius,
                            hint_text: interactable_def.hint_text.clone(),
                        });
                    }

                    // TriggerZone: Rapier sensor → entity.entered/exited:{id}
                    if let Some(zone_def) = &prefab.trigger_zone {
                        commands.entity(parent).insert((
                            TriggerZone,
                            bevy_rapier3d::prelude::Collider::ball(zone_def.radius),
                            Sensor,
                            ActiveEvents::COLLISION_EVENTS,
                        ));
                    }

                    // Targeting markers (composite path)
                    if prefab.click_selectable {
                        commands.entity(parent).insert(crate::capabilities::targeting::ClickSelectable);
                    }
                    if prefab.targetable {
                        commands.entity(parent).insert(crate::capabilities::targeting::Targetable);
                    }

                    if !prefab.stat_templates.is_empty() {
                        let spawn_id = &entity_def.id;
                        let mut stat_map = StatMap::default();
                        for tpl in &prefab.stat_templates {
                            let def = crate::schema::stats::StatDef {
                                base: tpl.base, min: tpl.min, max: tpl.max,
                                soft_max: None,
                                regen_rate: tpl.regen_rate, regen_delay: tpl.regen_delay,
                                thresholds: tpl.thresholds.iter().map(|t| crate::schema::stats::StatThreshold {
                                    when: t.when.clone(),
                                    emit: t.emit.replace("{self}", spawn_id),
                                }).collect(),
                            };
                            stat_map.0.insert(tpl.key.clone(), LiveStat::new(def));
                        }
                        commands.entity(parent).insert(stat_map);
                    }

                    // Motion: continuous rotation and/or vertical bob on the root entity.
                    // Children inherit the transform via Bevy's hierarchy, so the whole
                    // composite moves together.
                    if let Some(motion_def) = &prefab.motion {
                        let rotate = motion_def.rotate
                            .map(|(x, y, z)| Vec3::new(x, y, z))
                            .unwrap_or(Vec3::ZERO);
                        commands.entity(parent).insert(Motion {
                            rotate,
                            bob: motion_def.bob,
                            bob_origin_y: Some(translation.y),
                        });
                    }

                    if let Some(label_def) = &entity_def.label {
                        pending_labels.push((parent, label_def.clone()));
                    }

                    if let Some(sl) = &prefab.stat_label {
                        let resolved_key = sl.stat_key.replace("{self}", &entity_def.id);
                        pending_stat_labels.push((parent, resolved_key, sl.clone()));
                    }

                    if let Some(wb) = &prefab.world_stat_bar {
                        let resolved_key = wb.stat_key.replace("{self}", &entity_def.id);
                        pending_world_bars.push((parent, resolved_key, wb.clone()));
                    }
                    continue;
                }

                // ── Single primitive mesh ─────────────────────────────────────────────────
                let p = prefab.primitive.as_ref().cloned().unwrap_or_default();
                let prim_shape = prefab.shape.as_ref().unwrap();
                let mesh_handle = mats.meshes.add(build_primitive_mesh(prim_shape, &p));
                let built_mat = prefab.material.as_ref()
                    .and_then(|key| mats.built.0.get(key));

                let spawned = match built_mat {
                            Some(crate::runtime::material_factory::BuiltMaterialHandle::Standard(h)) => {
                                commands.spawn((
                                    Name::new(entity_def.id.clone()),
                                    Mesh3d(mesh_handle),
                                    MeshMaterial3d(h.clone()),
                                    transform,
                                    LevelEntity,
                                )).id()
                            }
                            Some(crate::runtime::material_factory::BuiltMaterialHandle::Terrain(h)) => {
                                commands.spawn((
                                    Name::new(entity_def.id.clone()),
                                    Mesh3d(mesh_handle),
                                    MeshMaterial3d(h.clone()),
                                    transform,
                                    LevelEntity,
                                )).id()
                            }
                            Some(crate::runtime::material_factory::BuiltMaterialHandle::Custom(h)) => {
                                commands.spawn((
                                    Name::new(entity_def.id.clone()),
                                    Mesh3d(mesh_handle),
                                    MeshMaterial3d(h.clone()),
                                    transform,
                                    LevelEntity,
                                )).id()
                            }
                            None => {
                                let mat_handle = mats.standard.add(
                                    primitive_material(&p, project.primitive_default_color)
                                );
                                commands.spawn((
                                    Name::new(entity_def.id.clone()),
                                    Mesh3d(mesh_handle),
                                    MeshMaterial3d(mat_handle),
                                    transform,
                                    LevelEntity,
                                )).id()
                            }
                        };

                        // Unlit custom materials are outside the lighting system — they must
                        // not cast shadows either, since shadow maps are a lighting concept.
                        if let Some(crate::runtime::material_factory::BuiltMaterialHandle::Custom(h)) = built_mat {
                            if mats.custom.get(h).map(|m| m.unlit).unwrap_or(false) {
                                commands.entity(spawned).insert(bevy::light::NotShadowCaster);
                            }
                        }

                        // Sensor takes precedence; otherwise check for static physics collider.
                        if p.sensor {
                            if let Some(collider) = build_primitive_collider(prim_shape, &p) {
                                commands.entity(spawned).insert((Sensor, collider, ActiveEvents::COLLISION_EVENTS));
                            } else {
                                load_errors.push(format!(
                                    "entity '{}': sensor: true on shape '{:?}' — no collider builder, sensor skipped",
                                    entity_def.id, prim_shape
                                ));
                            }
                        } else if p.physics {
                            if let Some(collider) = build_primitive_collider(prim_shape, &p) {
                                commands.entity(spawned).insert((RigidBody::Fixed, collider));
                            } else {
                                load_errors.push(format!(
                                    "entity '{}': physics: true on shape '{:?}' — no collider builder, physics skipped",
                                    entity_def.id, prim_shape
                                ));
                            }
                        }

                        // Give every single-primitive scene entity a stable SpawnId so that
                        // Action::Despawn can locate it by the scene entity id.
                        commands.entity(spawned).insert((
                            SpawnId(entity_def.id.clone()),
                            PrefabKey(entity_def.prefab.clone()),
                        ));
                        spawn_registry.entities.insert(entity_def.id.clone(), spawned);

                        // Collectable marker: collision triggers GameEvent into the rules pipeline.
                        // What happens on collection (Despawn, PlaySound, IncrementVariable, etc.)
                        // is defined in state_machine.ron — not hardcoded here.
                        if prefab.components.tags.contains(&TAG_COLLECTABLE.to_string()) {
                            commands.entity(spawned).insert(Collectable);
                        }
                        // Motion: continuous world-space rotation and/or vertical bob.
                        if let Some(motion_def) = &prefab.motion {
                            let rotate = motion_def.rotate
                                .map(|(x, y, z)| Vec3::new(x, y, z))
                                .unwrap_or(Vec3::ZERO);
                            commands.entity(spawned).insert(Motion {
                                rotate,
                                bob: motion_def.bob,
                                bob_origin_y: Some(translation.y),
                            });
                        }

                        // ── NPC agent (single-mesh variant) ──────────────────────────────────
                        // Applies to single-mesh primitives that carry an `npc` config.
                        // (Composite NPC prefabs — those with `children` — are handled above.)
                        if let Some(npc_def) = &prefab.components.npc {
                            let cap_radius = p.radius.unwrap_or(0.4);
                            let npc_height = p.height.unwrap_or(1.8);
                            let cap_half = (npc_height / 2.0 - cap_radius).max(0.0);
                            let body_y = cap_half + cap_radius;

                            let waypoints: Vec<Vec3> = npc_def.patrol_waypoints.iter()
                                .map(|(x, y, z)| translation + Vec3::new(*x, *y, *z))
                                .collect();

                            let fov_cos = npc_def.fov_degrees
                                .map(|deg| (deg.to_radians() / 2.0).cos())
                                .unwrap_or(-1.0);

                            let initial_state = if waypoints.is_empty() {
                                NpcState::Idle
                            } else {
                                NpcState::Patrol
                            };

                            commands.entity(spawned).insert((
                                NpcAgent {
                                    npc_id: entity_def.id.clone(),
                                    faction: npc_def.faction.clone(),
                                    on_player_near: npc_def.on_player_near.clone(),
                                    detection_radius: npc_def.detection_radius,
                                    chase_radius: npc_def.chase_radius,
                                    fov_cos,
                                    requires_los: npc_def.requires_los,
                                    approach_distance: npc_def.approach_distance,
                                    patrol_speed: npc_def.patrol_speed,
                                    chase_speed: npc_def.chase_speed,
                                    waypoints,
                                    current_waypoint: 0,
                                    state: initial_state,
                                    target: None,
                                    state_timer: 0.0,
                                    origin: translation,
                                    eye_height: npc_def.eye_height,
                                    alerted_duration: npc_def.alerted_duration,
                                    drag: npc_def.drag,
                                    waypoint_reach_radius: npc_def.waypoint_reach_radius,
                                    interact_leave_factor: npc_def.interact_leave_factor,
                                    home_arrival_radius: npc_def.home_arrival_radius,
                                },
                                RigidBody::Dynamic,
                                Collider::compound(vec![(
                                    Vec3::new(0.0, body_y, 0.0),
                                    Quat::IDENTITY,
                                    Collider::capsule_y(cap_half, cap_radius),
                                )]),
                                LockedAxes::ROTATION_LOCKED,
                                Damping { linear_damping: npc_def.linear_damping, angular_damping: npc_def.angular_damping },
                                Velocity::default(),
                                Friction { coefficient: 0.0, combine_rule: CoefficientCombineRule::Min },
                            ));
                        }

                        // Entity FSM behavior.
                        if let Some(behavior_path) = &prefab.behavior {
                            let project_root = params.project_root.0.as_str();
                            let resolved = resolve_project_path(project_root, behavior_path);
                            let handle: Handle<crate::schema::project::StateMachineAsset> =
                                asset_server.load(resolved);
                            commands.entity(spawned).insert(PendingBehavior(handle));
                        }

                        // Interactable: proximity + F key → entity.interacted:{id}
                        if let Some(interactable_def) = &prefab.interactable {
                            commands.entity(spawned).insert(Interactable {
                                radius: interactable_def.radius,
                                hint_text: interactable_def.hint_text.clone(),
                            });
                        }

                        // TriggerZone: Rapier sensor → entity.entered/exited:{id}
                        if let Some(zone_def) = &prefab.trigger_zone {
                            commands.entity(spawned).insert((
                                TriggerZone,
                                bevy_rapier3d::prelude::Collider::ball(zone_def.radius),
                                Sensor,
                                ActiveEvents::COLLISION_EVENTS,
                            ));
                        }

                        // Targeting markers (single-mesh primitive path)
                        if prefab.click_selectable {
                            commands.entity(spawned).insert(crate::capabilities::targeting::ClickSelectable);
                        }
                        if prefab.targetable {
                            commands.entity(spawned).insert(crate::capabilities::targeting::Targetable);
                        }

                        if !prefab.stat_templates.is_empty() {
                            let spawn_id = &entity_def.id;
                            let mut stat_map = StatMap::default();
                            for tpl in &prefab.stat_templates {
                                let def = crate::schema::stats::StatDef {
                                    base: tpl.base, min: tpl.min, max: tpl.max,
                                    soft_max: None,
                                    regen_rate: tpl.regen_rate, regen_delay: tpl.regen_delay,
                                    thresholds: tpl.thresholds.iter().map(|t| crate::schema::stats::StatThreshold {
                                        when: t.when.clone(),
                                        emit: t.emit.replace("{self}", spawn_id),
                                    }).collect(),
                                };
                                stat_map.0.insert(tpl.key.clone(), LiveStat::new(def));
                            }
                            commands.entity(spawned).insert(stat_map);
                        }

                        if let Some(label_def) = &entity_def.label {
                            pending_labels.push((spawned, label_def.clone()));
                        }

                        if let Some(sl) = &prefab.stat_label {
                            let resolved_key = sl.stat_key.replace("{self}", &entity_def.id);
                            pending_stat_labels.push((spawned, resolved_key, sl.clone()));
                        }

                        if let Some(wb) = &prefab.world_stat_bar {
                            let resolved_key = wb.stat_key.replace("{self}", &entity_def.id);
                            pending_world_bars.push((spawned, resolved_key, wb.clone()));
                        }
                continue;
            }

            let model_path = if let Some(catalog_entry) =
                params.asset_catalog.0.models.get(&prefab.model)
            {
                catalog_entry.path.clone()
            } else {
                load_errors.push(format!(
                    "entity '{}': model key '{}' not found in asset catalog, entity skipped",
                    entity_def.id, prefab.model
                ));
                continue;
            };

            if is_player {
                if prefab.animation_policy.is_none() {
                    warn!(
                        "Player prefab '{}' has no animation_policy — no animations will play. \
                         Set animation_policy in prefabs.ron to enable locomotion animation.",
                        entity_def.prefab
                    );
                }
                player_config = Some(PlayerConfig {
                    model_path,
                    initial_position: (translation.x, translation.y, translation.z),
                    camera: prefab.components.camera.clone().unwrap_or_else(default_camera_config),
                    inputs: prefab.components.inputs.clone().unwrap_or_else(default_input_map),
                    animation_policy: prefab.animation_policy.clone(),
                    movement: prefab.components.movement.clone(),
                });
            } else {
                let parent = spawn_prefab_instance(
                    &mut commands,
                    &asset_server,
                    &model_spawner,
                    &merged_fixes.0,
                    &params.project_root.0,
                    prefab,
                    model_path,
                    transform,
                    &entity_def.id,
                );
                // GLB actor/prop scene entities need a SpawnId (and registry entry) just like
                // the primitive/composite paths — otherwise id-targeted actions (Despawn,
                // ProjectDecal) and the targeting systems (which query `&SpawnId`) can't find
                // them. This branch historically omitted it.
                commands.entity(parent).insert((
                    LevelEntity,
                    SpawnId(entity_def.id.clone()),
                    PrefabKey(entity_def.prefab.clone()),
                ));
                spawn_registry.entities.insert(entity_def.id.clone(), parent);
                if let Some(label_def) = &entity_def.label {
                    pending_labels.push((parent, label_def.clone()));
                }
            }
        }

        if !load_errors.is_empty() {
            error!(
                "Scene '{}' — {} problem(s) found during load:\n  - {}",
                scene.name,
                load_errors.len(),
                load_errors.join("\n  - ")
            );
        }

        let tonemapping = scene.tonemapping.to_bevy();

        // ── Primitive player ─────────────────────────────────────────────────────────
        if let Some((entity_id, shape, params, position, components, player_children)) = primitive_player {
            let cap_radius = params.radius.unwrap_or(0.4);
            // `height` always means total visual height (cylindrical body + two hemispheres).
            let player_height = params.height.unwrap_or(1.8);
            let cap_half = (player_height / 2.0 - cap_radius).max(0.0);

            let mv = &components.movement;
            let walk_speed = mv.walk_speed;
            let run_speed  = mv.run_speed;
            let double_jump_enabled = mv.double_jump;
            let max_jumps: u8 = if double_jump_enabled { 2 } else { 1 };
            let jump_velocity = resolve_jump_velocity(mv.jump.as_ref(), player_height);
            let double_jump_velocity = if double_jump_enabled {
                resolve_jump_velocity(mv.double_jump_height.as_ref(), player_height)
            } else {
                jump_velocity
            };

            let mesh = build_primitive_mesh(&shape, &params);
            let mesh_handle = mats.meshes.add(mesh);
            let mat_handle  = mats.standard.add(primitive_material(&params, project.primitive_default_color));

            // `body_y` is the offset from the entity origin (feet) to the capsule centre.
            // Both the visual mesh and the physics collider are children at this offset,
            // so the capsule bottom sits exactly at the entity origin (ground-contact point).
            let body_y = cap_half + cap_radius;

            let player_entity = commands.spawn((
                (
                    Name::new("Player"),
                    SpawnId(entity_id.clone()),
                    Transform::from_translation(position),
                    Visibility::default(),
                    LevelEntity,
                ),
                (
                    CharacterController {
                        walk_speed,
                        run_speed,
                        rot_speed: mv.rot_speed.unwrap_or(3.0),
                        inputs: components.inputs.clone().unwrap_or_else(default_input_map),
                        is_running: false,
                        jump_velocity,
                        double_jump_enabled,
                        double_jump_velocity,
                        jumps_used: 0,
                        max_jumps,
                        collider_radius: cap_radius,
                        ground_cast_length: mv.ground_cast_length,
                        idle_drag: mv.idle_drag,
                    },
                    crate::capabilities::player::SpeedMultiplier(1.0),
                    LocomotionState::default(),
                    AnimationRequests::default(),
                    ActiveOverride::default(),
                ),
                (
                    // Compound collider: capsule centre offset up by body_y so its bottom
                    // coincides with entity origin (feet). Collider stays on the main entity
                    // so CollisionEvent reports the entity that has CharacterController.
                    RigidBody::Dynamic,
                    Collider::compound(vec![(
                        Vec3::new(0.0, body_y, 0.0),
                        Quat::IDENTITY,
                        Collider::capsule_y(cap_half, cap_radius),
                    )]),
                    LockedAxes::ROTATION_LOCKED,
                    Damping { linear_damping: 0.5, angular_damping: 0.5 },
                    Velocity::default(),
                    ExternalImpulse::default(),
                    // Zero friction prevents the capsule from catching on cube edges.
                    Friction { coefficient: 0.0, combine_rule: CoefficientCombineRule::Min },
                ),
            )).id();

            // Register the player in the spawn registry so SetEntityVisible and other
            // entity-ID–targeted actions can reach it — same pattern as every other entity.
            spawn_registry.entities.insert(entity_id.clone(), player_entity);

            // Visual body child — mesh centred at body_y above the feet so it aligns
            // with the compound collider above.
            let mesh_child = commands.spawn((
                Name::new("Player Body"),
                Mesh3d(mesh_handle),
                MeshMaterial3d(mat_handle),
                Transform::from_xyz(0.0, body_y, 0.0),
                Visibility::default(),
            )).id();
            commands.entity(player_entity).add_child(mesh_child);

            // Spawn cosmetic children (cap, eyes, nose, etc.) defined in the prefab.
            // Offsets are relative to the entity origin (feet), matching all other prefabs.
            {
                let mut ctx = ChildSpawnCtx {
                    meshes:    &mut mats.meshes,
                    standard:  &mut mats.standard,
                    built_mats: &mats.built.0,
                    custom_mats: &mats.custom,
                    primitive_default_color: project.primitive_default_color,
                    asset_server:  &asset_server,
                    model_spawner: &model_spawner,
                    fixes: &merged_fixes.0,
                    asset_catalog,
                    project_root,
                };
                spawn_primitive_children(
                    &mut commands, player_entity, &player_children,
                    prefab_catalog, &mut ctx,
                    &mut load_errors, "player", 0, &mut HashSet::new(),
                );
            }

            let cam = components.camera.clone().unwrap_or_else(default_camera_config);
            let cam_offset = Vec3::from(cam.offset);
            commands.spawn((
                Name::new("Orbit Camera"),
                Camera3d::default(),
                tonemapping,
                Transform::from_translation(position + cam_offset)
                    .looking_at(position + Vec3::from(cam.look_at_offset), Vec3::Y),
                LevelEntity,
                {
                    use crate::capabilities::camera::parse_orbit_button;
                    let (orbit_lmb, orbit_rmb) = parse_orbit_button(&cam.orbit_button);
                    let (char_rot_lmb, char_rot_rmb) = cam.character_rotate_button
                        .as_deref()
                        .map(parse_orbit_button)
                        .unwrap_or((false, false));
                    OrbitCamera {
                        target:                 player_entity,
                        radius:                 cam_offset.length(),
                        offset:                 cam_offset,
                        zoom_speed:             cam.zoom_speed,
                        orbit_speed:            cam.orbit_speed,
                        min_radius:             cam.min_radius,
                        max_radius:             cam.max_radius,
                        pitch:                  cam.initial_pitch,
                        yaw:                    cam.initial_yaw,
                        look_at_offset:         Vec3::from(cam.look_at_offset),
                        min_pitch:              cam.min_pitch,
                        max_pitch:              cam.max_pitch,
                        orbit_lmb,
                        orbit_rmb,
                        character_rotate_lmb:   char_rot_lmb,
                        character_rotate_rmb:   char_rot_rmb,
                    }
                },
            ));
        }
        // Spawn player (delayed if terrain present), flycam, or fallback camera.
        else if let Some(pc) = player_config {
            if scene.terrain.is_some() {
                info!("Terrain detected. Delaying player spawn...");
                commands.spawn((
                    crate::runtime::scene_manager::PendingPlayerConfig(pc),
                    crate::runtime::scene_manager::PendingTonemapping(tonemapping),
                    LevelEntity,
                ));
            } else {
                spawn_player_entity(
                    &mut commands,
                    &asset_server,
                    &merged_fixes.0,
                    &model_spawner,
                    &pc,
                    &params.project_root.0,
                    tonemapping,
                );
            }
        } else if let Some((fc_transform, fc_def)) = flycam_start {
            // Extract initial yaw/pitch from the spawn transform so the camera
            // starts oriented correctly and the first mouse move causes no jump.
            let (yaw, pitch, _) = fc_transform.rotation.to_euler(EulerRot::YXZ);
            info!(
                "Spawning FlyCamera at ({:.1}, {:.1}, {:.1})",
                fc_transform.translation.x,
                fc_transform.translation.y,
                fc_transform.translation.z,
            );
            commands.spawn((
                Name::new("FlyCamera"),
                Camera3d::default(),
                tonemapping,
                fc_transform,
                LevelEntity,
                {
                    use crate::schema::player::InputMap;
                    use crate::capabilities::flycam::parse_flycam_look_button;
                    let (look_lmb, look_rmb) = parse_flycam_look_button(&fc_def.look_button);
                    crate::capabilities::flycam::FlyCamera {
                        speed: fc_def.speed,
                        fast_speed: fc_def.fast_speed,
                        sensitivity: fc_def.sensitivity,
                        pitch,
                        yaw,
                        key_forward:  InputMap::parse_key(&fc_def.forward).unwrap_or(KeyCode::KeyW),
                        key_backward: InputMap::parse_key(&fc_def.backward).unwrap_or(KeyCode::KeyS),
                        key_left:     InputMap::parse_key(&fc_def.left).unwrap_or(KeyCode::KeyA),
                        key_right:    InputMap::parse_key(&fc_def.right).unwrap_or(KeyCode::KeyD),
                        key_up:       InputMap::parse_key(&fc_def.up).unwrap_or(KeyCode::Space),
                        key_down:     InputMap::parse_key(&fc_def.down).unwrap_or(KeyCode::KeyQ),
                        look_lmb,
                        look_rmb,
                    }
                },
            ));
        } else {
            info!("No player entity in v2 scene, spawning default camera...");
            commands.spawn((
                Name::new("Default Camera"),
                Camera3d::default(),
                tonemapping,
                Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
                LevelEntity,
            ));
        }

        // Spawn Terrain
        if let Some(terrain_v2) = &scene.terrain {
            info!("Spawning V2 Terrain...");
            commands.spawn((Name::new("Terrain"), LevelEntity, terrain_v2.clone()));
        }

        // Spawn world-label annotations (fixed positions).
        for label in &scene.world_labels {
            let (r, g, b, a) = label.color;
            commands.spawn((
                Name::new(format!("WorldLabel: {}", label.id)),
                Text2d::new(label.text.clone()),
                TextFont { font_size: label.font_size, ..default() },
                TextColor(Color::srgba(r, g, b, a)),
                Transform::from_xyz(0.0, 0.0, 1.0),
                WorldLabel {
                    world_pos: Vec3::from(label.translation),
                    tracked_entity: None,
                    offset: Vec3::ZERO,
                    base_font_size: label.font_size,
                    depth_scale: resolve_label_depth_scale(
                        scene.label_depth_scale.as_ref(),
                        label.depth_scale,
                    ),
                },
                LevelEntity,
            ));
        }

        // Spawn per-entity labels collected during the entity loop above.
        for (tracked, label_def) in pending_labels {
            let (r, g, b, a) = label_def.color;
            commands.spawn((
                Name::new(format!("EntityLabel: {}", label_def.text)),
                Text2d::new(label_def.text.clone()),
                TextFont { font_size: label_def.font_size, ..default() },
                TextColor(Color::srgba(r, g, b, a)),
                Transform::from_xyz(0.0, 0.0, 1.0),
                WorldLabel {
                    world_pos: Vec3::ZERO,
                    tracked_entity: Some(tracked),
                    offset: Vec3::from(label_def.offset),
                    base_font_size: label_def.font_size,
                    depth_scale: resolve_label_depth_scale(
                        scene.label_depth_scale.as_ref(),
                        label_def.depth_scale,
                    ),
                },
                LevelEntity,
            ));
        }

        // Spawn floating stat labels from PrefabDef.stat_label.
        // Uses the same WorldLabel + Text2d infrastructure as entity labels, but with
        // a StatLabelMarker component so stat_label_update_system drives the text.
        for (tracked, stat_key, sl) in pending_stat_labels {
            let (r, g, b, a) = sl.color;
            commands.spawn((
                Name::new(format!("StatLabel: {}", stat_key)),
                Text2d::new(String::new()),
                TextFont { font_size: sl.font_size, ..default() },
                TextColor(Color::srgba(r, g, b, a)),
                Transform::from_xyz(0.0, 0.0, 1.0),
                WorldLabel {
                    world_pos: Vec3::ZERO,
                    tracked_entity: Some(tracked),
                    offset: Vec3::from(sl.offset),
                    base_font_size: sl.font_size,
                    depth_scale: resolve_label_depth_scale(scene.label_depth_scale.as_ref(), None),
                },
                StatLabelMarker { stat_key, show_max: sl.show_max },
                LevelEntity,
            ));
        }

        // Spawn world-space stat bars from PrefabDef.world_stat_bar.
        // Dispatches on wb.style: Ascii → two Text2d entities; Pixel → anchor + 3 Mesh2d children.
        for (tracked, stat_key, wb) in pending_world_bars {
            let offset_v3 = Vec3::from(wb.offset);
            let fill_color = wb.fill_color;
            let (fr, fg, fb, fa) = fill_color;
            let (bgr, bgg, bgb, bga) = wb.bg_color;
            let color_bands = wb.color_bands;
            let depth_scale = resolve_label_depth_scale(scene.label_depth_scale.as_ref(), None);

            match wb.style {
                WorldStatBarStyle::Ascii { cells, font_size } => {
                    let cells_clamped = cells.max(1) as usize;
                    let bg_chars = " ".repeat(cells_clamped);

                    // Background track — static, never updated.
                    commands.spawn((
                        Name::new(format!("StatBarBg: {}", stat_key)),
                        Text2d::new(bg_chars),
                        TextFont { font_size, ..default() },
                        TextColor(Color::srgba(bgr, bgg, bgb, bga)),
                        Transform::from_xyz(0.0, 0.0, 1.0),
                        WorldLabel {
                            world_pos: Vec3::ZERO,
                            tracked_entity: Some(tracked),
                            offset: offset_v3,
                            base_font_size: font_size,
                            depth_scale,
                        },
                        LevelEntity,
                    ));

                    // Fill entity — text and colour updated each frame by world_stat_bar_update_system.
                    commands.spawn((
                        Name::new(format!("StatBarFill: {}", stat_key)),
                        Text2d::new(String::new()),
                        TextFont { font_size, ..default() },
                        TextColor(Color::srgba(fr, fg, fb, fa)),
                        Transform::from_xyz(0.0, 0.0, 2.0),
                        WorldLabel {
                            world_pos: Vec3::ZERO,
                            tracked_entity: Some(tracked),
                            offset: offset_v3,
                            base_font_size: font_size,
                            depth_scale,
                        },
                        WorldStatBarFillMarker { stat_key, cells, fill_color, color_bands },
                        LevelEntity,
                    ));
                }
                WorldStatBarStyle::Pixel { size, border, border_color } => {
                    let w = size.0.max(1.0);
                    let h = size.1.max(1.0);
                    let b = border.clamp(0.0, h / 2.0);
                    let (bdr, bdg, bdb, bda) = border_color;
                    let color_mats = mats.color_materials.as_mut()
                        .expect("ColorMaterial assets must be available to spawn pixel stat bars");

                    // Invisible anchor — WorldLabel tracks the entity; children follow via hierarchy.
                    let anchor = commands.spawn((
                        Name::new(format!("PixelBarAnchor: {}", stat_key)),
                        Transform::default(),
                        Visibility::default(),
                        WorldLabel {
                            world_pos: Vec3::ZERO,
                            tracked_entity: Some(tracked),
                            offset: offset_v3,
                            base_font_size: 1.0,
                            depth_scale: None,
                        },
                        LevelEntity,
                    )).id();

                    // Border quad (skip when border <= 0).
                    if b > 0.0 {
                        let border_child = commands.spawn((
                            Name::new(format!("PixelBarBorder: {}", stat_key)),
                            Mesh2d(mats.meshes.add(Rectangle::new(w + 2.0 * b, h + 2.0 * b))),
                            MeshMaterial2d(color_mats.add(ColorMaterial::from(Color::srgba(bdr, bdg, bdb, bda)))),
                            Transform::from_xyz(0.0, 0.0, 1.0),
                            LevelEntity,
                        )).id();
                        commands.entity(anchor).add_child(border_child);
                    }

                    // Background quad — full bar size, static.
                    let bg_child = commands.spawn((
                        Name::new(format!("PixelBarBg: {}", stat_key)),
                        Mesh2d(mats.meshes.add(Rectangle::new(w, h))),
                        MeshMaterial2d(color_mats.add(ColorMaterial::from(Color::srgba(bgr, bgg, bgb, bga)))),
                        Transform::from_xyz(0.0, 0.0, 2.0),
                        LevelEntity,
                    )).id();
                    commands.entity(anchor).add_child(bg_child);

                    // Fill quad — width=1 mesh scaled per frame; left-aligned via transform.
                    // scale.x = ratio * w; translation.x = -w/2 + (ratio*w)/2.
                    let fill_child = commands.spawn((
                        Name::new(format!("PixelBarFill: {}", stat_key)),
                        Mesh2d(mats.meshes.add(Rectangle::new(1.0, h))),
                        MeshMaterial2d(color_mats.add(ColorMaterial::from(Color::srgba(fr, fg, fb, fa)))),
                        Transform::from_xyz(-w / 2.0, 0.0, 3.0)
                            .with_scale(Vec3::new(0.0, 1.0, 1.0)),
                        WorldPixelBarFillMarker { stat_key, full_width: w, fill_color, color_bands },
                        LevelEntity,
                    )).id();
                    commands.entity(anchor).add_child(fill_child);
                }
            }
        }

        // Apply lighting
        apply_lighting_v2(&mut commands, scene, project, &asset_server, &mut mats.images);

        // Force all mesh pipelines to compile before the player interacts.
        // pipeline_warmup_system adds NoFrustumCulling for the first N frames so every
        // material pipeline compiles while meshes are "always visible", preventing
        // per-entity stalls as the camera moves and new objects enter the frustum.
        commands.insert_resource(PipelineWarmup(4));

        commands.insert_resource(crate::capabilities::particle_budget::ParticleBudget {
            max_count: scene.particle_budget.unwrap_or(2000),
        });

        next_state.set(AppState::InGame);
    } // end if !is_overlay

    // Spawn UI — always runs for both Replace and Overlay mode.
    // Pre-create RadarMaterial handles for any StatRadar elements so we can pass owned
    // handles into the with_children closures without borrowing `mats` inside them.
    let radar_handles: HashMap<String, Handle<RadarMaterial>> = scene.ui.iter()
        .filter_map(|el| {
            if let crate::schema::scene_v2::UiNodeDef::StatRadar(d) = el {
                let (fr, fg, fb, fa) = d.fill_color;
                let (or, og, ob, oa) = d.outline_color;
                let (gr, gg, gb, ga) = d.grid_color;
                let (br, bg_c, bb, ba) = d.background_color;
                let mat = RadarMaterial {
                    uniforms: RadarUniforms {
                        ratios_0: Vec4::ZERO,
                        ratios_1: Vec4::ZERO,
                        ratios_2: Vec4::ZERO,
                        // outline_width is authored in pixels; convert to UV fraction.
                        config: Vec4::new(d.stats.len().min(12) as f32, d.grid_steps as f32, d.outline_width / d.size.0.min(d.size.1).max(1.0), 0.0),
                        fill_color: Vec4::new(fr, fg, fb, fa),
                        outline_color: Vec4::new(or, og, ob, oa),
                        grid_color: Vec4::new(gr, gg, gb, ga),
                        background_color: Vec4::new(br, bg_c, bb, ba),
                    },
                };
                Some((d.id.clone(), mats.radar.add(mat)))
            } else {
                None
            }
        })
        .collect();

    if !scene.ui.is_empty() {
        if let Some(panel_def) = &scene.ui_panel {
            // Panel mode: full-screen flex root → centered panel box → column of elements.
            let mut root_cmd = commands.spawn((
                Name::new("UI Root"),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
            if is_overlay {
                root_cmd.insert(OverlayEntity);
            } else {
                root_cmd.insert(LevelEntity);
            }
            let (pr, pg, pb, pa) = panel_def.background_color;
            let padding = panel_def.padding;
            let gap = panel_def.gap;
            let panel_width = panel_def.width.map(Val::Px).unwrap_or(Val::Auto);
            let panel_height = panel_def.height.map(Val::Px).unwrap_or(Val::Auto);
            let ui_elements = scene.ui.clone();
            root_cmd.with_children(|parent| {
                parent
                    .spawn((
                        Name::new("Panel"),
                        Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            padding: UiRect::all(Val::Px(padding)),
                            row_gap: Val::Px(gap),
                            width: panel_width,
                            height: panel_height,
                            overflow: Overflow::clip(),
                            position_type: PositionType::Relative,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(pr, pg, pb, pa)),
                    ))
                    .with_children(|parent| {
                        for el in &ui_elements {
                            let h_justify = ui_justify(el.align());
                            let node = if el.absolute() {
                                Node {
                                    width: Val::Px(el.size().0),
                                    height: Val::Px(el.size().1),
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(el.position().0),
                                    top: Val::Px(el.position().1),
                                    justify_content: h_justify,
                                    align_items: AlignItems::Center,
                                    ..default()
                                }
                            } else {
                                Node {
                                    width: Val::Px(el.size().0),
                                    height: Val::Px(el.size().1),
                                    justify_content: h_justify,
                                    align_items: AlignItems::Center,
                                    ..default()
                                }
                            };
                            spawn_ui_element_node(parent, el, node, &radar_handles);
                        }
                    });
            });
        } else {
            // Absolute positioning mode.
            let mut root_cmd = commands.spawn((
                Name::new("UI Root"),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
            if is_overlay {
                root_cmd.insert(OverlayEntity);
            } else {
                root_cmd.insert(LevelEntity);
            }
            root_cmd.with_children(|parent| {
                for el in &scene.ui {
                    let node = Node {
                        width: Val::Px(el.size().0),
                        height: Val::Px(el.size().1),
                        justify_content: ui_justify(el.align()),
                        align_items: AlignItems::Center,
                        position_type: PositionType::Absolute,
                        left: Val::Px(el.position().0),
                        top: Val::Px(el.position().1),
                        ..default()
                    };
                    spawn_ui_element_node(parent, el, node, &radar_handles);
                }
            });
        }
    }

    // Only emit Ready for full scene loads. Overlays must not overwrite debug.scene
    // (which tracks the active main scene) — otherwise the scene.unloading:<name>
    // event fires with the overlay's name instead of the main scene's name, breaking
    // rules like `scene.unloading:main → StopMusic`.
    if !is_overlay {
        scene_events.write(SceneEvent::Ready(
            asset_server
                .get_path(&scene_handle.0)
                .map(|p| p.path().to_string_lossy().into_owned())
                .unwrap_or_default(),
        ));
    }
}

fn spawn_ui_element_node(
    parent: &mut ChildSpawnerCommands,
    el: &crate::schema::scene_v2::UiNodeDef,
    node: Node,
    radar_handles: &HashMap<String, Handle<RadarMaterial>>,
) {
    use crate::schema::scene_v2::UiNodeDef;
    match el {
        UiNodeDef::Rect(rect) => {
            let (r, g, b, a) = rect.color;
            parent.spawn((
                Name::new(format!("Rect: {}", rect.id)),
                node,
                BackgroundColor(Color::srgba(r, g, b, a)),
            ));
        }
        UiNodeDef::Label(label) => {
            let label_id = label.id.clone();
            parent
                .spawn((
                    Name::new(format!("Label: {}", label.text)),
                    node,
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                ))
                .with_children(|parent| {
                    let mut text_cmd = parent.spawn((
                        Name::new(format!("Text: {}", label.text)),
                        Text::new(label.text.clone()),
                        TextFont { font_size: 22.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                    if label_id == "flycam_position" {
                        text_cmd.insert(crate::capabilities::flycam::FlyCamPositionLabel);
                    }
                    if let Some(key) = &label.bind {
                        text_cmd.insert(crate::DynamicLabel {
                            key: key.clone(),
                            format: label.format.clone(),
                        });
                    }
                });
        }
        UiNodeDef::Button(btn) => {
            let (r, g, b, a) = btn.color;
            let bg_color = Color::srgba(r, g, b, a);
            let trigger = btn.action.strip_prefix("ui.").unwrap_or(&btn.action).to_string();
            let mut btn_node = node;
            btn_node.border = UiRect::all(Val::Px(5.0));
            parent
                .spawn((
                    Name::new(format!("Button: {}", btn.text)),
                    Button,
                    btn_node,
                    BorderColor::from(Color::BLACK),
                    BackgroundColor(bg_color),
                    UiAction::Trigger(trigger),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Name::new(format!("Text: {}", btn.text)),
                        Text::new(btn.text.clone()),
                        TextFont { font_size: 26.0, ..default() },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    ));
                });
        }
        UiNodeDef::StatBar(bar) => {
            let (br, bg_c, bb, ba) = bar.background_color;
            let (fr, fg, fb, fa) = bar.fill_color;
            let orientation = bar.orientation;
            let color_bands: Vec<(f32, (f32, f32, f32, f32))> = bar
                .color_bands.iter()
                .map(|cb| (cb.above_percent, cb.color))
                .collect();
            let mut bar_node = node;
            bar_node.overflow = Overflow::clip();
            match orientation {
                BarOrientation::Horizontal => {
                    bar_node.flex_direction = FlexDirection::Row;
                    bar_node.align_items = AlignItems::Stretch;
                }
                BarOrientation::Vertical => {
                    // ColumnReverse stacks children from the bottom, so the fill rect
                    // grows upward as its height percentage increases.
                    bar_node.flex_direction = FlexDirection::ColumnReverse;
                    bar_node.align_items = AlignItems::Stretch;
                }
            }
            let fill_node = match orientation {
                BarOrientation::Horizontal => Node {
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BarOrientation::Vertical => Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(0.0),
                    ..default()
                },
            };
            parent
                .spawn((
                    Name::new(format!("StatBar: {}", bar.id)),
                    bar_node,
                    BackgroundColor(Color::srgba(br, bg_c, bb, ba)),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Name::new("Fill"),
                        fill_node,
                        BackgroundColor(Color::srgba(fr, fg, fb, fa)),
                        StatBarFill {
                            stat_key: bar.stat_key.clone(),
                            orientation,
                            fill_color: (fr, fg, fb, fa),
                            color_bands,
                        },
                    ));
                    if bar.show_value {
                        parent.spawn((
                            Name::new("ValueText"),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                right: Val::Px(0.0),
                                bottom: Val::Px(0.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            Text::new(""),
                            TextFont { font_size: 13.0, ..default() },
                            TextColor(Color::WHITE),
                            StatValueText { stat_key: bar.stat_key.clone() },
                        ));
                    }
                });
        }
        UiNodeDef::StatSpread(spread) => {
            let (bfr, bfg, bfb, bfa) = spread.bar_fill_color;
            let (bbr, bbg, bbb, bba) = spread.bar_background_color;
            let (lr, lg, lb, la) = spread.label_color;
            let font_size_label = (spread.row_height * 0.70).max(10.0);
            let font_size_value = (spread.row_height * 0.65).max(10.0);
            let bar_width = spread.bar_width;
            let label_width = spread.label_width;
            let row_height = spread.row_height;
            let row_gap = spread.row_gap;
            let show_values = spread.show_values;
            let stats = spread.stats.clone();
            let mut spread_node = node;
            spread_node.flex_direction = FlexDirection::Column;
            spread_node.row_gap = Val::Px(row_gap);
            spread_node.width = Val::Auto;
            spread_node.height = Val::Auto;
            parent
                .spawn((
                    Name::new(format!("StatSpread: {}", spread.id)),
                    spread_node,
                ))
                .with_children(|parent| {
                    for stat_key in &stats {
                        parent
                            .spawn((
                                Name::new(format!("Row: {}", stat_key)),
                                Node {
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(4.0),
                                    height: Val::Px(row_height),
                                    ..default()
                                },
                            ))
                            .with_children(|parent| {
                                // Stat name label
                                parent.spawn((
                                    Name::new("Label"),
                                    Node {
                                        width: Val::Px(label_width),
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    Text::new(stat_key.clone()),
                                    TextFont { font_size: font_size_label, ..default() },
                                    TextColor(Color::srgba(lr, lg, lb, la)),
                                ));
                                // Minibar background + fill
                                parent
                                    .spawn((
                                        Name::new("Bar"),
                                        Node {
                                            width: Val::Px(bar_width),
                                            height: Val::Percent(100.0),
                                            overflow: Overflow::clip(),
                                            flex_direction: FlexDirection::Row,
                                            align_items: AlignItems::Stretch,
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(bbr, bbg, bbb, bba)),
                                    ))
                                    .with_children(|parent| {
                                        parent.spawn((
                                            Name::new("Fill"),
                                            Node {
                                                width: Val::Percent(0.0),
                                                height: Val::Percent(100.0),
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgba(bfr, bfg, bfb, bfa)),
                                            StatBarFill {
                                                stat_key: stat_key.clone(),
                                                orientation: BarOrientation::Horizontal,
                                                fill_color: (bfr, bfg, bfb, bfa),
                                                color_bands: vec![],
                                            },
                                        ));
                                    });
                                // Optional value text
                                if show_values {
                                    parent.spawn((
                                        Name::new("Value"),
                                        Node {
                                            min_width: Val::Px(50.0),
                                            align_items: AlignItems::Center,
                                            ..default()
                                        },
                                        Text::new(""),
                                        TextFont { font_size: font_size_value, ..default() },
                                        TextColor(Color::srgba(lr, lg, lb, la)),
                                        StatValueText { stat_key: stat_key.clone() },
                                    ));
                                }
                            });
                    }
                });
        }
        UiNodeDef::StatRadar(radar) => {
            if let Some(handle) = radar_handles.get(&radar.id) {
                parent.spawn((
                    Name::new(format!("StatRadar: {}", radar.id)),
                    node,
                    MaterialNode(handle.clone()),
                    StatRadarNode { stat_keys: radar.stats.clone() },
                ));
            } else {
                warn!("StatRadar {:?}: no pre-created material handle — skipping spawn", radar.id);
            }
        }
        UiNodeDef::ActionBar(bar) => {
            use crate::capabilities::action_bar::{ActionSlotUi, CooldownOverlay};
            let (br, bg_c, bb, ba) = bar.background_color;
            let slot_size = bar.slot_size;
            let slot_gap = bar.slot_gap;
            let mut bar_node = node;
            bar_node.flex_direction = FlexDirection::Row;
            bar_node.column_gap = Val::Px(slot_gap);
            bar_node.padding = UiRect::all(Val::Px(4.0));
            bar_node.align_items = AlignItems::Center;
            let slots = bar.slots.clone();
            parent
                .spawn((
                    Name::new(format!("ActionBar: {}", bar.id)),
                    bar_node,
                    BackgroundColor(Color::srgba(br, bg_c, bb, ba)),
                ))
                .with_children(|parent| {
                    for slot in &slots {
                        let key = slot.key.clone();
                        parent
                            .spawn((
                                Name::new(format!("Slot:{}", key)),
                                Button,
                                Node {
                                    width: Val::Px(slot_size),
                                    height: Val::Px(slot_size),
                                    position_type: PositionType::Relative,
                                    overflow: Overflow::clip(),
                                    justify_content: JustifyContent::FlexEnd,
                                    align_items: AlignItems::FlexEnd,
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.18, 0.18, 0.22, 0.95)),
                                BorderColor::from(Color::srgba(0.45, 0.45, 0.55, 0.8)),
                                ActionSlotUi {
                                    slot_key: key.clone(),
                                    do_actions: slot.do_actions.clone(),
                                    cooldown_secs: slot.cooldown_secs,
                                    cost: slot.cost.clone(),
                                },
                            ))
                            .with_children(|parent| {
                                // Full-slot overlay — alpha-fade only; no Node height writes
                                // so Bevy's UI layout is never invalidated by the visual system.
                                parent.spawn((
                                    Name::new("CooldownOverlay"),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        top: Val::Px(0.0),
                                        left: Val::Px(0.0),
                                        right: Val::Px(0.0),
                                        bottom: Val::Px(0.0),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                                    CooldownOverlay { slot_key: key.clone() },
                                ));
                                // Keybind label — bottom-right corner.
                                parent.spawn((
                                    Name::new(format!("Key:{}", key)),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        bottom: Val::Px(3.0),
                                        right: Val::Px(5.0),
                                        ..default()
                                    },
                                    Text::new(key.clone()),
                                    TextFont { font_size: 13.0, ..default() },
                                    TextColor(Color::srgba(0.85, 0.85, 0.85, 0.75)),
                                ));
                            });
                    }
                });
        }
    }
}

fn apply_lighting_v2(
    commands: &mut Commands,
    scene: &crate::schema::scene_v2::GameSceneV2,
    project: &ProjectConfig,
    asset_server: &AssetServer,
    images: &mut Assets<Image>,
) {
    if let Some(lighting) = &scene.lighting {
        if let Some(size) = lighting.shadow_map_size {
            commands.insert_resource(bevy::light::DirectionalLightShadowMap { size: size as usize });
        }
        if let Some(size) = lighting.point_shadow_map_size {
            commands.insert_resource(bevy::light::PointLightShadowMap { size: size as usize });
        }

        if let Some((r, g, b)) = lighting.ambient {
            commands.spawn((
                Name::new("Ambient Light"),
                AmbientLight {
                    color: Color::srgba(r, g, b, 1.0),
                    brightness: lighting.ambient_brightness.unwrap_or(150.0),
                    ..default()
                },
                LevelEntity,
            ));
        }

        for (i, pl) in lighting.point_lights.iter().enumerate() {
            commands.spawn((
                Name::new(format!("Point Light {}", i)),
                PointLight {
                    color: Color::srgb(pl.color.0, pl.color.1, pl.color.2),
                    intensity: pl.intensity,
                    radius: pl.radius,
                    range: pl.range,
                    shadows_enabled: pl.shadows_enabled,
                    ..default()
                },
                Transform::from_translation(Vec3::new(pl.position.0, pl.position.1, pl.position.2)),
                LevelEntity,
            ));
        }

        if let Some(dl) = &lighting.directional {
            let rot = Quat::from_euler(
                EulerRot::XYZ,
                dl.rotation_euler_deg.0.to_radians(),
                dl.rotation_euler_deg.1.to_radians(),
                dl.rotation_euler_deg.2.to_radians(),
            );
            let mut dir_light = commands.spawn((
                Name::new("Directional Light"),
                DirectionalLight {
                    color: Color::srgba(dl.color.0, dl.color.1, dl.color.2, 1.0),
                    illuminance: dl.intensity,
                    shadows_enabled: dl.shadows_enabled,
                    ..default()
                },
                Transform::from_rotation(rot),
                LevelEntity,
            ));
            if dl.shadow_distance.is_some() || dl.cascade_overlap.is_some() || dl.num_cascades.is_some() {
                let mut builder = bevy::light::CascadeShadowConfigBuilder::default();
                if let Some(dist) = dl.shadow_distance {
                    builder.maximum_distance = dist;
                }
                if let Some(overlap) = dl.cascade_overlap {
                    builder.overlap_proportion = overlap;
                }
                if let Some(n) = dl.num_cascades {
                    builder.num_cascades = n as usize;
                }
                dir_light.insert(builder.build());
            }
        }
    }

    // Environment map from project global_environment
    if let Some(env) = &project.global_environment {
        let (d_handle, s_handle) = if env.diffuse_path.is_none() && env.specular_path.is_none() {
            if let Some(fallback) = &env.fallback {
                let img = super::generate_cubemap(fallback);
                let handle = images.add(img);
                (handle.clone(), handle)
            } else {
                (Handle::default(), Handle::default())
            }
        } else {
            let d = env.diffuse_path.as_ref().map(|p| asset_server.load(p)).unwrap_or_default();
            let s = env.specular_path.as_ref().map(|p| asset_server.load(p)).unwrap_or_default();
            (d, s)
        };

        if d_handle != Handle::default() || s_handle != Handle::default() {
            commands.spawn((
                Name::new("Environment Map Light"),
                EnvironmentMapLight {
                    diffuse_map: d_handle,
                    specular_map: s_handle,
                    intensity: env.intensity,
                    ..default()
                },
                LevelEntity,
            ));
        }
    }
}

// ─── Label depth scale helper ─────────────────────────────────────────────────

/// Resolves the effective depth-scale config for a single label.
/// Returns `Some((reference_distance, min_scale_floor))` when scaling is active,
/// or `None` when the label should always render at its authored font size.
///
/// Resolution order:
///   - `per_label = Some(false)` → always disabled, regardless of scene config.
///   - `per_label = Some(true)`  → always enabled; uses scene params or fallback defaults.
///   - `per_label = None`        → inherits scene config (enabled iff scene has a block).
fn resolve_label_depth_scale(
    scene: Option<&crate::schema::scene_v2::LabelDepthScaleDef>,
    per_label: Option<bool>,
) -> Option<(f32, f32)> {
    let enabled = match per_label {
        Some(b) => b,
        None => scene.is_some(),
    };
    if !enabled {
        return None;
    }
    let (ref_dist, min_floor) = match scene {
        Some(cfg) => (cfg.reference_distance, cfg.min_scale.unwrap_or(0.0)),
        None => (50.0, 0.0),
    };
    Some((ref_dist, min_floor))
}

// ─── Jump velocity helper ─────────────────────────────────────────────────────

/// Standard gravitational acceleration (m/s²), matching Rapier's default.
const GRAVITY: f32 = 9.81;

fn ui_justify(align: UiTextAlign) -> JustifyContent {
    match align {
        UiTextAlign::Left   => JustifyContent::FlexStart,
        UiTextAlign::Center => JustifyContent::Center,
        UiTextAlign::Right  => JustifyContent::FlexEnd,
    }
}

/// Convert a `JumpConfig` (or `None` → jump own height) to an initial Y velocity.
/// Uses kinematic relation: v = √(2 · g · h).
pub(super) fn resolve_jump_velocity(config: Option<&crate::schema::catalog::JumpConfig>, player_height: f32) -> f32 {
    use crate::schema::catalog::JumpConfig;
    let h = match config {
        None => player_height,
        Some(JumpConfig::Fixed { height }) => *height,
        Some(JumpConfig::RelativeToHeight { percent }) => player_height * percent / 100.0,
    };
    (2.0 * GRAVITY * h).sqrt()
}

// ─── Nested-prefab child spawner ──────────────────────────────────────────────

/// Holds the asset references needed by `spawn_primitive_children`.
/// Splits out the mutable and read-only slices of `SceneMaterialParams` so we can
/// pass them into a recursive free function without fighting the borrow checker.
struct ChildSpawnCtx<'a> {
    meshes:    &'a mut Assets<Mesh>,
    standard:  &'a mut Assets<StandardMaterial>,
    built_mats: &'a std::collections::HashMap<String, crate::runtime::material_factory::BuiltMaterialHandle>,
    custom_mats: &'a Assets<crate::capabilities::custom_material::CustomMaterial>,
    primitive_default_color: Option<(f32, f32, f32)>,
    asset_server:  &'a AssetServer,
    model_spawner: &'a crate::runtime::model_spawner::ModelSpawner,
    fixes: &'a std::collections::HashMap<String, crate::schema::project::TransformFix>,
    asset_catalog: &'a crate::schema::catalog::AssetCatalog,
    project_root:  &'a str,
}

/// Spawns the `children` list of a composite prefab under `parent`, recursing into
/// nested prefab references.  `visiting` tracks the keys currently on the call stack
/// for cycle detection; `depth` enforces a hard nesting limit.
fn spawn_primitive_children(
    commands: &mut Commands,
    parent: Entity,
    children: &[crate::schema::catalog::ChildPrimitiveDef],
    prefab_catalog: &crate::schema::catalog::PrefabCatalog,
    ctx: &mut ChildSpawnCtx<'_>,
    load_errors: &mut Vec<String>,
    entity_id: &str,
    depth: u8,
    visiting: &mut HashSet<String>,
) {
    const MAX_DEPTH: u8 = 8;

    for child_def in children {
        let child_tf = Transform {
            translation: Vec3::from(child_def.offset),
            rotation: Quat::from_euler(
                EulerRot::XYZ,
                child_def.rotation_euler_deg.0.to_radians(),
                child_def.rotation_euler_deg.1.to_radians(),
                child_def.rotation_euler_deg.2.to_radians(),
            ),
            scale: Vec3::from(child_def.scale),
        };

        // ── Nested prefab reference ───────────────────────────────────────────
        if let Some(nested_key) = &child_def.prefab {
            if depth >= MAX_DEPTH {
                load_errors.push(format!(
                    "entity '{}': nested prefab '{}' exceeds max nesting depth ({}), skipped",
                    entity_id, nested_key, MAX_DEPTH
                ));
                continue;
            }
            if visiting.contains(nested_key.as_str()) {
                load_errors.push(format!(
                    "entity '{}': circular prefab reference detected at '{}', skipped",
                    entity_id, nested_key
                ));
                continue;
            }
            let Some(nested_prefab) = prefab_catalog.prefabs.get(nested_key.as_str()) else {
                load_errors.push(format!(
                    "entity '{}': nested prefab '{}' not found, skipped",
                    entity_id, nested_key
                ));
                continue;
            };

            match nested_prefab.kind {
                PrefabKind::Foliage => {
                    // Foliage nested inside a composite is not supported — skip silently.
                    load_errors.push(format!(
                        "entity '{}': nested prefab '{}' has kind Foliage; foliage cannot be nested inside composite prefabs, skipped",
                        entity_id, nested_key
                    ));
                    continue;
                }
                PrefabKind::Actor | PrefabKind::Prop => {
                    // GLB branch: resolve model path and spawn via the shared instance spawner.
                    let Some(catalog_entry) = ctx.asset_catalog.models.get(&nested_prefab.model) else {
                        load_errors.push(format!(
                            "entity '{}': nested prefab '{}' model key '{}' not found in asset catalog, skipped",
                            entity_id, nested_key, nested_prefab.model
                        ));
                        continue;
                    };
                    let model_path = catalog_entry.path.clone();
                    visiting.insert(nested_key.clone());
                    let model_entity = spawn_prefab_instance(
                        commands,
                        ctx.asset_server,
                        ctx.model_spawner,
                        ctx.fixes,
                        ctx.project_root,
                        nested_prefab,
                        model_path,
                        child_tf,
                        nested_key,
                    );
                    commands.entity(parent).add_child(model_entity);
                    visiting.remove(nested_key.as_str());
                }
                PrefabKind::Primitive => {
                    // Primitive branch: anchor + children list or single shape.
                    let anchor = commands.spawn((
                        Name::new(nested_key.clone()),
                        child_tf,
                        Visibility::default(),
                    )).id();
                    commands.entity(parent).add_child(anchor);

                    if !nested_prefab.children.is_empty() {
                        // Composite primitive: recurse into children.
                        visiting.insert(nested_key.clone());
                        spawn_primitive_children(
                            commands, anchor, &nested_prefab.children,
                            prefab_catalog, ctx, load_errors, entity_id, depth + 1, visiting,
                        );
                        visiting.remove(nested_key.as_str());
                    } else if let Some(nested_shape) = nested_prefab.shape.as_ref() {
                        // Single-shape primitive: build one mesh child under the anchor.
                        let pparams = nested_prefab.primitive.as_ref().cloned().unwrap_or_default();
                        let mesh_h = ctx.meshes.add(build_primitive_mesh(nested_shape, &pparams));
                        let shape_name = format!("{:?}", nested_shape);
                        let built_mat = nested_prefab.material.as_ref()
                            .and_then(|k| ctx.built_mats.get(k));
                        let mesh_entity = match built_mat {
                            Some(crate::runtime::material_factory::BuiltMaterialHandle::Standard(h)) => {
                                commands.spawn((
                                    Name::new(shape_name),
                                    Mesh3d(mesh_h),
                                    MeshMaterial3d(h.clone()),
                                    Transform::IDENTITY,
                                )).id()
                            }
                            Some(crate::runtime::material_factory::BuiltMaterialHandle::Custom(h)) => {
                                let e = commands.spawn((
                                    Name::new(shape_name),
                                    Mesh3d(mesh_h),
                                    MeshMaterial3d(h.clone()),
                                    Transform::IDENTITY,
                                )).id();
                                if ctx.custom_mats.get(h).map(|m| m.unlit).unwrap_or(false) {
                                    commands.entity(e).insert(bevy::light::NotShadowCaster);
                                }
                                e
                            }
                            Some(crate::runtime::material_factory::BuiltMaterialHandle::Terrain(h)) => {
                                commands.spawn((
                                    Name::new(shape_name),
                                    Mesh3d(mesh_h),
                                    MeshMaterial3d(h.clone()),
                                    Transform::IDENTITY,
                                )).id()
                            }
                            None => {
                                let mat_h = ctx.standard.add(
                                    primitive_material(&pparams, ctx.primitive_default_color)
                                );
                                commands.spawn((
                                    Name::new(shape_name),
                                    Mesh3d(mesh_h),
                                    MeshMaterial3d(mat_h),
                                    Transform::IDENTITY,
                                )).id()
                            }
                        };
                        commands.entity(anchor).add_child(mesh_entity);
                    }
                    // else: primitive with no shape and no children — valid empty anchor.
                }
            }
            continue;
        }

        // ── Inline primitive shape ────────────────────────────────────────────
        let child_shape = child_def.shape.as_ref().unwrap();
        let child_mesh_h = ctx.meshes.add(build_primitive_mesh(child_shape, &child_def.primitive));
        let built_mat = child_def.material.as_ref().and_then(|key| ctx.built_mats.get(key));

        let child_entity = match built_mat {
            Some(crate::runtime::material_factory::BuiltMaterialHandle::Standard(h)) => {
                commands.spawn((
                    Name::new(format!("{:?}", child_shape)),
                    Mesh3d(child_mesh_h),
                    MeshMaterial3d(h.clone()),
                    child_tf,
                )).id()
            }
            Some(crate::runtime::material_factory::BuiltMaterialHandle::Custom(h)) => {
                let entity = commands.spawn((
                    Name::new(format!("{:?}", child_shape)),
                    Mesh3d(child_mesh_h),
                    MeshMaterial3d(h.clone()),
                    child_tf,
                )).id();
                if ctx.custom_mats.get(h).map(|m| m.unlit).unwrap_or(false) {
                    commands.entity(entity).insert(bevy::light::NotShadowCaster);
                }
                entity
            }
            Some(crate::runtime::material_factory::BuiltMaterialHandle::Terrain(h)) => {
                commands.spawn((
                    Name::new(format!("{:?}", child_shape)),
                    Mesh3d(child_mesh_h),
                    MeshMaterial3d(h.clone()),
                    child_tf,
                )).id()
            }
            None => {
                let mat_h = ctx.standard.add(
                    primitive_material(&child_def.primitive, ctx.primitive_default_color)
                );
                commands.spawn((
                    Name::new(format!("{:?}", child_shape)),
                    Mesh3d(child_mesh_h),
                    MeshMaterial3d(mat_h),
                    child_tf,
                )).id()
            }
        };
        commands.entity(parent).add_child(child_entity);

        // Static physics collider: child entity gets the collider shape; parent gets
        // RigidBody::Fixed so Rapier treats the children as a compound static body.
        if child_def.primitive.physics {
            if let Some(collider) = build_primitive_collider(child_shape, &child_def.primitive) {
                commands.entity(child_entity).insert(collider);
                commands.entity(parent).insert(RigidBody::Fixed);
            }
        }
    }
}

// ─── Primitive shape helpers ───────────────────────────────────────────────────

fn build_primitive_mesh(shape: &PrimitiveShapeKind, p: &crate::schema::catalog::PrimitiveParams) -> Mesh {
    use bevy::math::primitives as bmp;
    match shape {
        PrimitiveShapeKind::Cuboid => {
            let (x, y, z) = p.size.unwrap_or((3.0, 3.0, 3.0));
            bmp::Cuboid::new(x, y, z).mesh().build()
        }
        PrimitiveShapeKind::Sphere => bmp::Sphere::new(p.radius.unwrap_or(2.0)).mesh().build(),
        PrimitiveShapeKind::Cylinder => bmp::Cylinder::new(
            p.radius.unwrap_or(1.5),
            p.height.unwrap_or(4.0),
        ).mesh().build(),
        PrimitiveShapeKind::Capsule3d => {
            let radius = p.radius.unwrap_or(1.5);
            let total_height = p.height.unwrap_or(4.0);
            let half_length = (total_height / 2.0 - radius).max(0.0);
            bmp::Capsule3d { radius, half_length }.mesh().build()
        }
        PrimitiveShapeKind::Cone => bmp::Cone {
            radius: p.radius.unwrap_or(2.0),
            height: p.height.unwrap_or(4.0),
        }.mesh().build(),
        PrimitiveShapeKind::Torus => bmp::Torus::new(
            p.radius_top.unwrap_or(0.5),  // inner radius
            p.radius.unwrap_or(2.0),      // outer radius
        ).mesh().build(),
        PrimitiveShapeKind::ConicalFrustum => bmp::ConicalFrustum {
            radius_top:    p.radius_top.unwrap_or(1.0),
            radius_bottom: p.radius.unwrap_or(2.0),
            height:        p.height.unwrap_or(4.0),
        }.mesh().build(),
        PrimitiveShapeKind::Plane => {
            let (x, _, z) = p.size.unwrap_or((2.0, 0.0, 2.0));
            bmp::Plane3d::default().mesh().size(x, z).build()
        }
    }
}

/// Returns a Rapier3D static collider matching the given shape, or `None` for shapes
/// without a direct Rapier equivalent (Cone, Torus, ConicalFrustum, Plane).
fn build_primitive_collider(shape: &PrimitiveShapeKind, p: &crate::schema::catalog::PrimitiveParams) -> Option<Collider> {
    match shape {
        PrimitiveShapeKind::Cuboid => {
            let (x, y, z) = p.size.unwrap_or((3.0, 3.0, 3.0));
            Some(Collider::cuboid(x / 2.0, y / 2.0, z / 2.0))
        }
        PrimitiveShapeKind::Sphere => Some(Collider::ball(p.radius.unwrap_or(2.0))),
        PrimitiveShapeKind::Cylinder => Some(Collider::cylinder(
            p.height.unwrap_or(4.0) / 2.0,
            p.radius.unwrap_or(1.5),
        )),
        PrimitiveShapeKind::Capsule3d => {
            let radius = p.radius.unwrap_or(1.5);
            let total_height = p.height.unwrap_or(4.0);
            let half_length = (total_height / 2.0 - radius).max(0.0);
            Some(Collider::capsule_y(half_length, radius))
        }
        _ => None,
    }
}

/// Builds a `StandardMaterial` for a primitive shape.
///
/// Color priority (highest wins):
/// 1. `p.color` — set per-prefab in the prefab catalog RON
/// 2. `project_default` — set via `primitive_default_color` in the project RON
/// 3. Neutral grey `(0.7, 0.7, 0.7)` — engine fallback
pub(crate) fn primitive_material(
    p: &crate::schema::catalog::PrimitiveParams,
    project_default: Option<(f32, f32, f32)>,
) -> StandardMaterial {
    let (r, g, b) = p.color
        .or(project_default)
        .unwrap_or((0.7, 0.7, 0.7));
    StandardMaterial {
        base_color: Color::srgb(r, g, b),
        perceptual_roughness: p.roughness.unwrap_or(0.5),
        metallic: p.metallic.unwrap_or(0.0),
        ..default()
    }
}

/// Warms the asset server cache for all audio files in the current project's catalog.
/// Runs once per scene load (triggered by `SceneEvent::Ready`) so audio handles are live
/// before the player can interact, eliminating the first-play I/O delay.
pub fn preload_audio_system(
    mut events: MessageReader<SceneEvent>,
    asset_catalog: Res<LoadedAssetCatalog>,
    asset_server: Res<AssetServer>,
    mut audio_handles: ResMut<LoadedAudioHandles>,
) {
    for event in events.read() {
        if matches!(event, SceneEvent::Ready(_)) {
            audio_handles.0.clear();
            for entry in asset_catalog.0.audio.values() {
                let handle: Handle<bevy::audio::AudioSource> = asset_server.load(entry.path.clone());
                audio_handles.0.push(handle);
            }
            if !audio_handles.0.is_empty() {
                info!("Audio preload: {} file(s) warmed up", audio_handles.0.len());
            }
        }
    }
}

pub fn preload_decals_system(
    mut events: MessageReader<SceneEvent>,
    asset_catalog: Res<LoadedAssetCatalog>,
    asset_server: Res<AssetServer>,
    mut decal_handles: ResMut<LoadedDecalHandles>,
) {
    for event in events.read() {
        if matches!(event, SceneEvent::Ready(_)) {
            decal_handles.0.clear();
            for path in asset_catalog.0.decals.values() {
                let handle: Handle<Image> = asset_server.load(path.clone());
                decal_handles.0.push(handle);
            }
            if !decal_handles.0.is_empty() {
                info!("Decal preload: {} texture(s) warmed up", decal_handles.0.len());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::catalog::PrimitiveParams;

    fn color_of(mat: &StandardMaterial) -> (f32, f32, f32) {
        let c = mat.base_color.to_srgba();
        (
            (c.red   * 1000.0).round() / 1000.0,
            (c.green * 1000.0).round() / 1000.0,
            (c.blue  * 1000.0).round() / 1000.0,
        )
    }

    #[test]
    fn primitive_material_prefab_color_wins_over_all() {
        let p = PrimitiveParams {
            color: Some((1.0, 0.0, 0.0)),
            ..Default::default()
        };
        let mat = primitive_material(&p, Some((0.0, 1.0, 0.0)));
        assert_eq!(color_of(&mat), (1.0, 0.0, 0.0), "prefab color must take priority");
    }

    #[test]
    fn primitive_material_project_default_wins_over_grey() {
        let p = PrimitiveParams::default(); // no color set
        let mat = primitive_material(&p, Some((0.2, 0.4, 0.8)));
        assert_eq!(color_of(&mat), (0.2, 0.4, 0.8), "project default must win over grey fallback");
    }

    #[test]
    fn primitive_material_falls_back_to_grey_when_no_defaults() {
        let p = PrimitiveParams::default();
        let mat = primitive_material(&p, None);
        assert_eq!(color_of(&mat), (0.7, 0.7, 0.7), "should fall back to neutral grey");
    }

    #[test]
    fn primitive_material_roughness_and_metallic_defaults() {
        let p = PrimitiveParams::default();
        let mat = primitive_material(&p, None);
        assert!((mat.perceptual_roughness - 0.5).abs() < 1e-6);
        assert!((mat.metallic - 0.0).abs() < 1e-6);
    }

    #[test]
    fn primitive_material_roughness_and_metallic_overrides() {
        let p = PrimitiveParams {
            roughness: Some(0.1),
            metallic: Some(0.9),
            ..Default::default()
        };
        let mat = primitive_material(&p, None);
        assert!((mat.perceptual_roughness - 0.1).abs() < 1e-6);
        assert!((mat.metallic - 0.9).abs() < 1e-6);
    }
}
