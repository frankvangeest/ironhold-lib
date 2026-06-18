use bevy::prelude::*;
use bevy::gltf::Gltf;
use std::collections::HashMap;
use crate::ProjectRoot;
use crate::schema::*;
use crate::schema::catalog::ColliderShapeKind;
use crate::schema::player::{PlayerConfig, AnimationPolicy, CameraConfig, InputMap};
use crate::runtime::model_spawner::ModelSpawner;
use crate::runtime::material_factory::PendingMaterialOverride;
use crate::capabilities::player::CharacterController;
use crate::capabilities::animation::AnimationController;
use crate::capabilities::camera::OrbitCamera;
use crate::capabilities::animation_resolver::{
    ActiveOverride, AnimationPolicyComponent, AnimationRequests, LocomotionState,
};
use bevy_rapier3d::prelude::*;
use super::{
    LevelEntity, LoadedAssetCatalog, MergedModelFixes, PendingAnimationPolicy,
    PendingPlayerConfig, PendingTonemapping,
    PendingBehavior, BehaviorHandle, EntityFsmState, SpawnId, SpawnRegistry,
    PendingEntitySpawns, tag_spawned_entity,
    resolve_project_path,
    scene_loader::resolve_jump_velocity,
};
use crate::runtime::actions::ActionQueue;
use super::message_interpreter::rewrite_self;
use crate::schema::stats::{LiveStat, StatMap};
use crate::capabilities::npc::{NpcAgent, NpcState};
use crate::capabilities::motion::Motion;
use super::DynamicStatUiQueue;

/// Instantiates a prefab entity: spawns the model, applies material overrides and
/// animation components based on what the PrefabDef declares. Called from both
/// `spawn_scene_v2` (scene entities) and the `Action::Spawn` executor (dynamic spawns)
/// so the two paths are guaranteed to be identical.
///
/// Returns the parent entity so callers can attach extra components (e.g. `SpawnId`).
pub fn spawn_prefab_instance(
    commands: &mut Commands,
    asset_server: &AssetServer,
    model_spawner: &ModelSpawner,
    fixes: &HashMap<String, TransformFix>,
    project_root: &str,
    prefab: &crate::schema::catalog::PrefabDef,
    model_path: String,
    transform: Transform,
    name: &str,
    stat_overrides: &HashMap<String, f32>,
) -> Entity {
    let spawned =
        model_spawner.spawn_instance(commands, asset_server, fixes, model_path.clone(), transform);
    let mut ec = commands.entity(spawned.parent);
    ec.insert(Name::new(name.to_string()));

    if let Some(mat_key) = &prefab.material {
        ec.insert(PendingMaterialOverride(mat_key.clone()));
    }

    if let Some(behavior_path) = &prefab.behavior {
        let resolved = resolve_project_path(project_root, behavior_path);
        let handle: Handle<crate::schema::project::StateMachineAsset> =
            asset_server.load(resolved);
        ec.insert(PendingBehavior(handle));
    }

    if let Some(interactable_def) = &prefab.interactable {
        ec.insert(crate::capabilities::interactable::Interactable {
            radius: interactable_def.radius,
            hint_text: interactable_def.hint_text.clone(),
        });
    }

    if let Some(zone_def) = &prefab.trigger_zone {
        ec.insert((
            crate::capabilities::trigger_zone::TriggerZone,
            bevy_rapier3d::prelude::Collider::ball(zone_def.radius),
            bevy_rapier3d::prelude::Sensor,
            bevy_rapier3d::prelude::ActiveEvents::COLLISION_EVENTS,
        ));
    }

    if let Some(policy_path) = &prefab.animation_policy {
        let resolved = resolve_project_path(project_root, policy_path);
        let policy_handle: Handle<AnimationPolicy> = asset_server.load(resolved);
        let gltf_path = model_path.split('#').next().unwrap_or("").to_string();
        let gltf_handle = asset_server.load(gltf_path.clone());
        ec.insert((
            PendingAnimationPolicy(policy_handle),
            AnimationController {
                current: String::new(),
                last_played: String::new(),
                gltf_path,
                gltf_handle,
                source_handles: Vec::new(),
                node_indices: HashMap::new(),
                graph_initialized: false,
                transition_ms: 0,
                should_loop: true,
                last_player_entity: None,
            },
            LocomotionState::default(),
            AnimationRequests::default(),
            ActiveOverride::default(),
        ));
    }

    // Targeting markers (click_selectable/targetable) and the standard metadata
    // (SpawnId/PrefabKey/LevelEntity/registry) are attached by the caller via
    // `tag_spawned_entity`, so every spawn path stays consistent.

    if !prefab.colliders.is_empty() {
        let shapes: Vec<(Vec3, Quat, Collider)> = prefab.colliders.iter().filter_map(|cdef| {
            let shape = match cdef.shape {
                ColliderShapeKind::Cuboid => {
                    let (x, y, z) = cdef.size.unwrap_or((1.0, 1.0, 1.0));
                    Collider::cuboid(x / 2.0, y / 2.0, z / 2.0)
                }
                ColliderShapeKind::Sphere => Collider::ball(cdef.radius.unwrap_or(0.5)),
                ColliderShapeKind::Cylinder => Collider::cylinder(
                    cdef.height.unwrap_or(1.0) / 2.0,
                    cdef.radius.unwrap_or(0.5),
                ),
            };
            let (rx, ry, rz) = cdef.rotation_euler_deg;
            let rot = Quat::from_euler(EulerRot::XYZ, rx.to_radians(), ry.to_radians(), rz.to_radians());
            Some((Vec3::from(cdef.offset), rot, shape))
        }).collect();
        if !shapes.is_empty() {
            commands.entity(spawned.parent).insert((RigidBody::Fixed, Collider::compound(shapes)));
        }
    }

    if !prefab.stat_templates.is_empty() {
        for key in stat_overrides.keys() {
            if !prefab.stat_templates.iter().any(|t| &t.key == key) {
                warn!("stat_overrides: entity '{}' has unknown stat key '{}' (not in prefab)", name, key);
            }
        }
        let mut stat_map = StatMap::default();
        for tpl in &prefab.stat_templates {
            let base = stat_overrides.get(&tpl.key).copied().unwrap_or(tpl.base);
            if base > tpl.max {
                warn!("stat_overrides: entity '{}' stat '{}' override {} exceeds template max {}; value will exceed max", name, tpl.key, base, tpl.max);
            }
            let def = crate::schema::stats::StatDef {
                base,
                min: tpl.min,
                max: tpl.max,
                soft_max: None,
                regen_rate: tpl.regen_rate,
                regen_delay: tpl.regen_delay,
                thresholds: tpl.thresholds.iter().map(|t| crate::schema::stats::StatThreshold {
                    when: t.when.clone(),
                    emit: t.emit.replace("{self}", name),
                }).collect(),
            };
            stat_map.0.insert(tpl.key.clone(), LiveStat::new(def));
        }
        commands.entity(spawned.parent).insert(stat_map);
    }

    // Motion: continuous rotation and/or vertical bob. Inserting here (rather than at each
    // call site) ensures dynamic Action::Spawn entities get the same behaviour as scene-placed ones.
    if let Some(motion_def) = &prefab.motion {
        let rotate = motion_def.rotate
            .map(|(x, y, z)| Vec3::new(x, y, z))
            .unwrap_or(Vec3::ZERO);
        commands.entity(spawned.parent).insert(Motion {
            rotate,
            bob: motion_def.bob,
            bob_origin_y: Some(transform.translation.y),
        });
    }

    // NPC agent: GLB Actor/Prop prefabs can declare `components.npc` to gain NPC AI and
    // movement. A capsule physics body is added here (sized conservatively); designers tune
    // behaviour radius and approach via the NpcDef fields.
    if let Some(npc_def) = &prefab.components.npc {
        let waypoints: Vec<Vec3> = npc_def.patrol_waypoints.iter()
            .map(|(x, y, z)| transform.translation + Vec3::new(*x, *y, *z))
            .collect();

        let fov_cos = npc_def.fov_degrees
            .map(|deg| (deg.to_radians() / 2.0).cos())
            .unwrap_or(-1.0);

        let initial_state = if waypoints.is_empty() { NpcState::Idle } else { NpcState::Patrol };

        let cap_radius = npc_def.collider_radius.unwrap_or(0.35_f32);
        let cap_height = npc_def.collider_height.unwrap_or(1.6_f32);
        let cap_half   = (cap_height / 2.0 - cap_radius).max(0.0);
        let body_y     = cap_half + cap_radius;

        commands.entity(spawned.parent).insert((
            NpcAgent {
                npc_id:            name.to_string(),
                faction:           npc_def.faction.clone(),
                on_player_near:    npc_def.on_player_near.clone(),
                detection_radius:  npc_def.detection_radius,
                chase_radius:      npc_def.chase_radius,
                fov_cos,
                requires_los:      npc_def.requires_los,
                approach_distance: npc_def.approach_distance,
                patrol_speed:      npc_def.patrol_speed,
                chase_speed:       npc_def.chase_speed,
                waypoints,
                current_waypoint:  0,
                state:             initial_state,
                target:            None,
                state_timer:       0.0,
                origin:            transform.translation,
                eye_height:        npc_def.eye_height,
                alerted_duration:  npc_def.alerted_duration,
                drag:              npc_def.drag,
                waypoint_reach_radius:    npc_def.waypoint_reach_radius,
                interact_leave_factor:    npc_def.interact_leave_factor,
                home_arrival_radius:      npc_def.home_arrival_radius,
            },
            RigidBody::Dynamic,
            Collider::compound(vec![(
                Vec3::new(0.0, body_y, 0.0),
                Quat::IDENTITY,
                Collider::capsule_y(cap_half, cap_radius),
            )]),
            LockedAxes::ROTATION_LOCKED,
            Damping {
                linear_damping:  npc_def.linear_damping,
                angular_damping: npc_def.angular_damping,
            },
            Velocity::default(),
            Friction { coefficient: 0.0, combine_rule: CoefficientCombineRule::Min },
        ));
    }

    spawned.parent
}

/// Maximum number of queued spawns processed per frame.
/// Limits how many WebGPU pipeline compiles can be triggered in a single frame when
/// many prefabs are spawned at once (e.g. a wave spawn). A value of 2 caps the per-frame
/// stall to ~2 pipeline compiles while keeping batch spawns fast (5 enemies in 3 frames).
const SPAWNS_PER_FRAME: usize = 2;

/// Drains up to `SPAWNS_PER_FRAME` entries from `PendingEntitySpawns` each frame.
/// `Action::Spawn` enqueues into that resource instead of calling `spawn_prefab_instance`
/// directly, so wave spawns spread their pipeline compilation cost across multiple frames
/// rather than hitting in one frame.
pub fn drain_spawn_queue_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut pending: ResMut<PendingEntitySpawns>,
    mut registry: ResMut<SpawnRegistry>,
    model_spawner: Res<ModelSpawner>,
    fixes: Res<MergedModelFixes>,
    mut stat_ui_queue: ResMut<DynamicStatUiQueue>,
    active_tonemapping: Res<super::ActiveTonemapping>,
) {
    for _ in 0..SPAWNS_PER_FRAME {
        let Some(queued) = pending.0.pop_front() else { break };
        info!(
            "Spawning '{}' at ({:.1}, {:.1}, {:.1})",
            queued.spawn_id,
            queued.transform.translation.x,
            queued.transform.translation.y,
            queued.transform.translation.z,
        );

        // Player-tagged prefabs spawn a full character: capsule physics + orbit camera.
        if let Some(player_config) = queued.player_config {
            spawn_player_entity(
                &mut commands,
                &asset_server,
                &fixes.0,
                &model_spawner,
                &player_config,
                &queued.project_root,
                active_tonemapping.0,
                &mut registry,
            );
            continue;
        }

        let parent = spawn_prefab_instance(
            &mut commands,
            &asset_server,
            &model_spawner,
            &fixes.0,
            &queued.project_root,
            &queued.prefab_def,
            queued.model_path,
            queued.transform,
            &queued.spawn_id,
            &Default::default(),
        );
        tag_spawned_entity(
            &mut commands.entity(parent),
            &mut registry,
            &queued.spawn_id,
            &queued.prefab_key,
            queued.prefab_def.click_selectable,
            queued.prefab_def.targetable,
            queued.prefab_def.select_aim_height,
        );

        let stat_label = queued.prefab_def.stat_label.as_ref().map(|sl| {
            let key = sl.stat_key.replace("{self}", &queued.spawn_id);
            (key, sl.clone())
        });
        let world_stat_bar = queued.prefab_def.world_stat_bar.as_ref().map(|wb| {
            let key = wb.stat_key.replace("{self}", &queued.spawn_id);
            (key, wb.clone())
        });
        if stat_label.is_some() || world_stat_bar.is_some() {
            stat_ui_queue.0.push(super::DynamicStatUiEntry { entity: parent, stat_label, world_stat_bar });
        }
    }
}

pub fn animation_policy_loader_system(
    mut commands: Commands,
    mut pending: Query<(Entity, &PendingAnimationPolicy, &mut AnimationController)>,
    policies: Res<Assets<AnimationPolicy>>,
    asset_catalog: Res<LoadedAssetCatalog>,
    asset_server: Res<AssetServer>,
) {
    for (entity, pending_policy, mut controller) in &mut pending {
        if let Some(policy) = policies.get(&pending_policy.0) {
            controller.current = policy.base.idle.clone();

            // Load animation-source GLBs declared in the policy.
            let mut source_handles: Vec<Handle<Gltf>> = Vec::new();
            for key in &policy.animation_sources {
                if let Some(entry) = asset_catalog.0.models.get(key.as_str()) {
                    let gltf_path = entry.path.split('#').next().unwrap_or("").to_string();
                    source_handles.push(asset_server.load(gltf_path));
                } else {
                    warn!("animation_sources: catalog key '{}' not found", key);
                }
            }
            controller.source_handles = source_handles;

            commands
                .entity(entity)
                .insert(AnimationPolicyComponent(policy.clone()))
                .remove::<PendingAnimationPolicy>();
            info!(
                "AnimationPolicy loaded — initial: '{}', {} animation source(s)",
                policy.base.idle,
                policy.animation_sources.len()
            );
        }
    }
}

/// Polls `PendingBehavior` handles; once the `StateMachineAsset` loads, replaces the
/// pending component with `BehaviorHandle` + `EntityFsmState` seeded to `initial_state`,
/// and fires the initial state's `entry_actions` so self-sustaining loops (like campfire
/// particle emitters) start without requiring an explicit transition into the first state.
pub fn resolve_pending_behaviors_system(
    mut commands: Commands,
    pending: Query<(Entity, &PendingBehavior, &SpawnId)>,
    state_machines: Res<Assets<crate::schema::project::StateMachineAsset>>,
    mut action_queue: ResMut<ActionQueue>,
) {
    for (entity, pending_behavior, spawn_id) in &pending {
        if let Some(fsm) = state_machines.get(&pending_behavior.0) {
            let initial = fsm.initial_state.clone();
            commands
                .entity(entity)
                .insert((
                    BehaviorHandle(pending_behavior.0.clone()),
                    EntityFsmState { current: initial.clone() },
                ))
                .remove::<PendingBehavior>();
            info!("Behavior loaded — initial state: \"{}\"", initial);

            // Fire initial state entry_actions so the behavior starts immediately.
            if let Some(state_def) = fsm.states.iter().find(|s| s.name == initial) {
                for action in &state_def.entry_actions {
                    action_queue.push(rewrite_self(action.clone(), &spawn_id.0));
                }
            }
        }
    }
}

pub fn spawn_player_when_terrain_ready(
    mut commands: Commands,
    terrain_query: Query<Entity, Added<crate::capabilities::terrain::TerrainReady>>,
    pending_query: Query<(Entity, &PendingPlayerConfig, Option<&PendingTonemapping>)>,
    asset_server: Res<AssetServer>,
    model_spawner: Res<ModelSpawner>,
    merged_fixes: Res<MergedModelFixes>,
    project_root: Res<ProjectRoot>,
    mut registry: ResMut<SpawnRegistry>,
) {
    if terrain_query.is_empty() {
        return;
    }

    for (pending_entity, pending, pending_tm) in &pending_query {
        info!("Terrain is ready. Spawning player...");
        let tonemapping = pending_tm
            .map(|pt| pt.0)
            .unwrap_or(bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted);
        spawn_player_entity(
            &mut commands,
            &asset_server,
            &merged_fixes.0,
            &model_spawner,
            &pending.0,
            &project_root.0,
            tonemapping,
            &mut registry,
        );
        commands.entity(pending_entity).despawn();
    }
}

pub(crate) fn spawn_player_entity(
    commands: &mut Commands,
    asset_server: &AssetServer,
    fixes: &HashMap<String, TransformFix>,
    model_spawner: &ModelSpawner,
    player_config: &PlayerConfig,
    project_root: &str,
    tonemapping: bevy::core_pipeline::tonemapping::Tonemapping,
    registry: &mut SpawnRegistry,
) {
    let gltf_path = player_config.model_path.split('#').next().unwrap_or("").to_string();
    let gltf_handle = asset_server.load(gltf_path.clone());

    let policy_handle_opt: Option<Handle<AnimationPolicy>> =
        player_config.animation_policy.as_deref().map(|rel| {
            let path = resolve_project_path(project_root, rel);
            info!("Loading AnimationPolicy from: {}", path);
            asset_server.load(path)
        });

    let spawned = model_spawner.spawn_instance(
        commands,
        asset_server,
        fixes,
        player_config.model_path.clone(),
        Transform::from_translation(Vec3::from(player_config.initial_position)),
    );

    let player_entity = spawned.parent;
    let mv = &player_config.movement;
    let cap_radius = mv.collider_radius.unwrap_or(0.4);
    let player_height = mv.collider_height.unwrap_or(1.8);
    let cap_half = (player_height / 2.0 - cap_radius).max(0.0);
    let double_jump_enabled = mv.double_jump;
    let max_jumps: u8 = if double_jump_enabled { 2 } else { 1 };
    let jump_velocity = resolve_jump_velocity(mv.jump.as_ref(), player_height);
    let double_jump_velocity = if double_jump_enabled {
        resolve_jump_velocity(mv.double_jump_height.as_ref(), player_height)
    } else {
        jump_velocity
    };
    commands.entity(player_entity).insert((
        Name::new("Player"),
        // LevelEntity attached by tag_spawned_entity below (sole owner).
        CharacterController {
            walk_speed: mv.walk_speed,
            run_speed: mv.run_speed,
            rot_speed: mv.rot_speed.unwrap_or(3.0),
            inputs: player_config.inputs.clone(),
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
        LocomotionState::default(),
        AnimationRequests::default(),
        ActiveOverride::default(),
        // Required by player_movement_system's query. Without it the GLB player is
        // silently filtered out of that query and never moves (the primitive player
        // path inserts this at scene_loader.rs; the GLB path historically did not).
        crate::capabilities::player::SpeedMultiplier(1.0),
        RigidBody::Dynamic,
        Collider::compound(vec![(
            Vec3::new(0.0, cap_half + cap_radius, 0.0),
            Quat::IDENTITY,
            Collider::capsule_y(cap_half, cap_radius),
        )]),
        LockedAxes::ROTATION_LOCKED,
        Damping { linear_damping: mv.linear_damping, angular_damping: mv.angular_damping },
        Velocity::default(),
        ExternalImpulse::default(),
    ));

    // Standard metadata (SpawnId/PrefabKey/LevelEntity/registry) via the shared helper, so
    // the GLB player is addressable by id like every other entity. Players are never
    // click/Tab targets, so markers are off. Player-specific components are inserted above.
    tag_spawned_entity(
        &mut commands.entity(player_entity),
        registry,
        &player_config.spawn_id,
        &player_config.prefab_key,
        false,
        false,
        1.0,
    );

    if let Some(policy_handle) = policy_handle_opt {
        commands.entity(player_entity).insert((
            PendingAnimationPolicy(policy_handle.clone()),
            AnimationController {
                current: String::new(),
                last_played: String::new(),
                gltf_path,
                gltf_handle,
                source_handles: Vec::new(),
                node_indices: HashMap::new(),
                graph_initialized: false,
                transition_ms: 0,
                should_loop: true,
                last_player_entity: None,
            },
        ));
    }

    // Spawn Orbit Camera
    let cam = &player_config.camera;
    let (orbit_lmb, orbit_rmb) = crate::capabilities::camera::parse_orbit_button(&cam.orbit_button);
    let (char_rot_lmb, char_rot_rmb) = cam.character_rotate_button
        .as_deref()
        .map(crate::capabilities::camera::parse_orbit_button)
        .unwrap_or((false, false));
    let start_pos =
        Vec3::from(player_config.initial_position) + Vec3::from(cam.offset);
    commands.spawn((
        Name::new("Orbit Camera"),
        Camera3d::default(),
        tonemapping,
        Transform::from_translation(start_pos)
            .looking_at(Vec3::from(player_config.initial_position), Vec3::Y),
        LevelEntity,
        OrbitCamera {
            target: player_entity,
            radius: Vec3::from(cam.offset).length(),
            offset: Vec3::from(cam.offset),
            zoom_speed: cam.zoom_speed,
            orbit_speed: cam.orbit_speed,
            min_radius: cam.min_radius,
            max_radius: cam.max_radius,
            pitch: cam.initial_pitch,
            yaw: cam.initial_yaw,
            look_at_offset: Vec3::from(cam.look_at_offset),
            min_pitch: cam.min_pitch,
            max_pitch: cam.max_pitch,
            orbit_lmb,
            orbit_rmb,
            character_rotate_lmb: char_rot_lmb,
            character_rotate_rmb: char_rot_rmb,
        },
    ));
}

pub(crate) fn default_camera_config() -> CameraConfig {
    CameraConfig {
        offset: (0.0, 5.0, 10.0),
        look_at_offset: (0.0, 2.0, 0.0),
        zoom_speed: 10.0,
        orbit_speed: 0.5,
        min_radius: 2.0,
        max_radius: 20.0,
        min_pitch: 0.1,
        max_pitch: 0.9,
        orbit_button: "Either".to_string(),
        character_rotate_button: Some("Right".to_string()),
        initial_pitch: 0.5,
        initial_yaw: 0.0,
    }
}

pub(crate) fn default_input_map() -> InputMap {
    InputMap {
        forward: "KeyW".to_string(),
        backward: "KeyS".to_string(),
        left: "KeyA".to_string(),
        right: "KeyD".to_string(),
        strafe_left: "KeyQ".to_string(),
        strafe_right: "KeyE".to_string(),
        jump: "Space".to_string(),
        run: "ShiftLeft".to_string(),
        interact: "KeyF".to_string(),
        strafe_mouse_button: Some("Left".to_string()),
        target_next: "Tab".to_string(),
        target_range: 30.0,
    }
}
