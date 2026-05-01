use bevy::prelude::*;
use std::collections::HashMap;
use crate::ProjectRoot;
use crate::schema::*;
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
    LevelEntity, MergedModelFixes, PendingAnimationPolicy, PendingPlayerConfig, PendingTonemapping,
    PendingBehavior, BehaviorHandle, EntityFsmState,
    resolve_project_path,
    scene_loader::resolve_jump_velocity,
};

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
                node_indices: HashMap::new(),
                graph_initialized: false,
                transition_ms: 0,
                should_loop: true,
            },
            LocomotionState::default(),
            AnimationRequests::default(),
            ActiveOverride::default(),
        ));
    }

    if !prefab.colliders.is_empty() {
        let shapes: Vec<(Vec3, Quat, Collider)> = prefab.colliders.iter().filter_map(|cdef| {
            let shape = match cdef.shape.as_str() {
                "Cuboid" => {
                    let (x, y, z) = cdef.size.unwrap_or((1.0, 1.0, 1.0));
                    Some(Collider::cuboid(x / 2.0, y / 2.0, z / 2.0))
                }
                "Sphere" => Some(Collider::ball(cdef.radius.unwrap_or(0.5))),
                "Cylinder" => Some(Collider::cylinder(
                    cdef.height.unwrap_or(1.0) / 2.0,
                    cdef.radius.unwrap_or(0.5),
                )),
                _ => None,
            }?;
            Some((Vec3::from(cdef.offset), Quat::IDENTITY, shape))
        }).collect();
        if !shapes.is_empty() {
            commands.entity(spawned.parent).insert((RigidBody::Fixed, Collider::compound(shapes)));
        }
    }

    spawned.parent
}

pub fn animation_policy_loader_system(
    mut commands: Commands,
    mut pending: Query<(Entity, &PendingAnimationPolicy, &mut AnimationController)>,
    policies: Res<Assets<AnimationPolicy>>,
) {
    for (entity, pending_policy, mut controller) in &mut pending {
        if let Some(policy) = policies.get(&pending_policy.0) {
            controller.current = policy.base.idle.clone();
            commands
                .entity(entity)
                .insert(AnimationPolicyComponent(policy.clone()))
                .remove::<PendingAnimationPolicy>();
            info!("AnimationPolicy loaded — initial animation: {}", policy.base.idle);
        }
    }
}

/// Polls `PendingBehavior` handles; once the `StateMachineAsset` loads, replaces the
/// pending component with `BehaviorHandle` + `EntityFsmState` seeded to `initial_state`.
pub fn resolve_pending_behaviors_system(
    mut commands: Commands,
    pending: Query<(Entity, &PendingBehavior)>,
    state_machines: Res<Assets<crate::schema::project::StateMachineAsset>>,
) {
    for (entity, pending_behavior) in &pending {
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
) {
    let gltf_path = player_config.model_path.split('#').next().unwrap_or("").to_string();
    let gltf_handle = asset_server.load(gltf_path.clone());

    let policy_path = resolve_project_path(project_root, &player_config.animation_policy);
    let policy_handle: Handle<AnimationPolicy> = asset_server.load(policy_path.clone());
    info!("Loading AnimationPolicy from: {}", policy_path);

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
        LevelEntity,
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
            // Entity origin is the feet / ground-contact point. The sphere cast starts
            // at the feet and only needs a short distance to detect the ground below.
            collider_radius: cap_radius,
            ground_cast_length: 0.3,
        },
        LocomotionState::default(),
        AnimationRequests::default(),
        ActiveOverride::default(),
        PendingAnimationPolicy(policy_handle.clone()),
        AnimationController {
            current: String::new(), // will be set once policy loads
            last_played: String::new(),
            gltf_path,
            gltf_handle,
            node_indices: HashMap::new(),
            graph_initialized: false,
            transition_ms: 0,
            should_loop: true,
        },
        // Compound collider: the capsule centre is offset up by body_y so its bottom
        // coincides with the entity origin (feet). Keeping the collider on the main
        // entity means CollisionEvent always reports the entity with CharacterController.
        RigidBody::Dynamic,
        Collider::compound(vec![(
            Vec3::new(0.0, cap_half + cap_radius, 0.0),
            Quat::IDENTITY,
            Collider::capsule_y(cap_half, cap_radius),
        )]),
        LockedAxes::ROTATION_LOCKED,
        Damping { linear_damping: 0.5, angular_damping: 0.5 },
        Velocity::default(),
        ExternalImpulse::default(),
    ));

    // Spawn Orbit Camera
    let start_pos =
        Vec3::from(player_config.initial_position) + Vec3::from(player_config.camera.offset);
    commands.spawn((
        Name::new("Orbit Camera"),
        Camera3d::default(),
        tonemapping,
        Transform::from_translation(start_pos)
            .looking_at(Vec3::from(player_config.initial_position), Vec3::Y),
        LevelEntity,
        OrbitCamera {
            target: player_entity,
            radius: Vec3::from(player_config.camera.offset).length(),
            offset: Vec3::from(player_config.camera.offset),
            zoom_speed: player_config.camera.zoom_speed,
            orbit_speed: player_config.camera.orbit_speed,
            min_radius: player_config.camera.min_radius,
            max_radius: player_config.camera.max_radius,
            pitch: 0.5,
            yaw: 0.0,
            look_at_offset: Vec3::from(player_config.camera.look_at_offset),
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
    }
}
