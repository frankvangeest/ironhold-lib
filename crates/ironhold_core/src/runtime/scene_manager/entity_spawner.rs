use bevy::prelude::*;
use bevy::gltf::Gltf;
use std::collections::{HashMap, HashSet};
use crate::ProjectRoot;
use crate::schema::*;
use crate::schema::catalog::ColliderShapeKind;
use crate::schema::player::{PlayerConfig, PlayerModelSource, AnimationPolicy, CameraConfig, InputMap};
use crate::runtime::model_spawner::ModelSpawner;
use crate::runtime::material_factory::PendingMaterialOverride;
use crate::capabilities::player::CharacterController;
use crate::capabilities::animation::AnimationController;
use crate::capabilities::camera::{ActiveCameraMode, OrbitCameraMode, OrbitState, CameraTargets};
use crate::schema::camera::CameraModeDef;
use crate::capabilities::animation_resolver::{
    ActiveOverride, AnimationPolicyComponent, AnimationRequests, LocomotionState,
};
use bevy_rapier3d::prelude::*;
use super::{
    LevelEntity, LoadedAssetCatalog, MergedModelFixes, PendingAnimationPolicy,
    PendingPlayerConfig, PendingTonemapping,
    PendingBehavior, BehaviorHandle, EntityFsmState, SpawnId, SpawnRegistry,
    PendingEntitySpawns, tag_spawned_entity, should_insert_nameplate,
    resolve_project_path,
    scene_loader::{resolve_jump_velocity, ChildSpawnCtx, build_primitive_mesh, primitive_material, spawn_primitive_children},
};
use crate::runtime::actions::ActionQueue;
use super::message_interpreter::rewrite_self;
use crate::schema::stats::{LiveStat, StatMap, StatTemplateDef};
use crate::capabilities::npc::{NpcAgent, NpcState};
use crate::capabilities::motion::Motion;
use super::DynamicStatUiQueue;

/// Attaches capability features declared on a `PrefabDef` to any spawned entity.
/// Single source of truth for: behavior, interactable, dialogue, inventory,
/// stat_templates, and trigger_zone. Called from `spawn_prefab_instance` (GLB path)
/// and from both Primitive branches in scene_loader.rs so all three spawn paths
/// stay in sync. Adding a new PrefabDef capability field here propagates everywhere.
///
/// The trigger_zone block spawns a sensor child and must come last to avoid a
/// Commands borrow conflict with the entity-level inserts above.
pub(super) fn attach_prefab_features(
    commands: &mut Commands,
    entity: Entity,
    prefab: &crate::schema::catalog::PrefabDef,
    project_root: &str,
    asset_server: &AssetServer,
    entity_id: &str,
    stat_overrides: &HashMap<String, f32>,
    prefab_key: &str,
) {
    if let Some(behavior_path) = &prefab.behavior {
        let resolved = resolve_project_path(project_root, behavior_path);
        let handle: Handle<crate::schema::project::StateMachineAsset> =
            asset_server.load(resolved);
        commands.entity(entity).insert(PendingBehavior(handle));
    }

    if let Some(interactable_def) = &prefab.interactable {
        commands.entity(entity).insert(crate::capabilities::interactable::Interactable {
            radius: interactable_def.radius,
            hint_text: interactable_def.hint_text.clone(),
        });
    }

    if let Some(dialogue_path) = &prefab.dialogue {
        commands.entity(entity).insert(
            crate::capabilities::dialogue::DialoguePath(dialogue_path.clone())
        );
    }

    if let Some(inv_def) = &prefab.inventory {
        let slots = inv_def.max_slots.max(4);
        let mut inv = crate::capabilities::inventory::Inventory::new(slots);
        for entry in &inv_def.initial_items {
            crate::capabilities::inventory::add_to_slots(
                &mut inv.slots, inv.max_slots, &entry.item_key, entry.count, None,
            );
        }
        commands.entity(entity).insert(inv);
    }

    if let Some(stat_map) = build_stat_map_from_templates(
        &prefab.stat_templates, stat_overrides, entity_id, prefab_key,
    ) {
        commands.entity(entity).insert(stat_map);
    }

    // Trigger zone must come last: spawning the sensor child requires a fresh Commands
    // borrow after all entity-level inserts above have been committed.
    if let Some(zone_def) = &prefab.trigger_zone {
        let sensor = commands.spawn((
            Name::new(format!("{}/trigger_zone", entity_id)),
            crate::capabilities::trigger_zone::TriggerZone,
            crate::capabilities::trigger_zone::TriggerZoneId(entity_id.to_string()),
            Collider::ball(zone_def.radius),
            Sensor,
            ActiveEvents::COLLISION_EVENTS,
            Transform::default(),
        )).id();
        commands.entity(entity).add_child(sensor);
    }
}

/// Builds a `StatMap` from a `PrefabDef.stat_templates` list, applying `stat_overrides` and
/// `{self}` substitution in threshold `emit` strings. Returns `None` for an empty template list
/// (the caller should simply not insert a `StatMap` in that case) — this is the single source of
/// truth for the stat-template-to-`StatMap` conversion, shared by `attach_prefab_features` (every
/// NPC/prop/composite prefab) and `spawn_player_entity_core` (players that declare their own
/// `stat_templates`, see `planning/features/per_player_stat_pools.md`).
pub(super) fn build_stat_map_from_templates(
    templates: &[StatTemplateDef],
    stat_overrides: &HashMap<String, f32>,
    entity_id: &str,
    prefab_key: &str,
) -> Option<StatMap> {
    if templates.is_empty() {
        return None;
    }
    for key in stat_overrides.keys() {
        if !templates.iter().any(|t| &t.key == key) {
            warn!(
                "stat_overrides: entity '{}' has unknown stat key '{}' (not in prefab '{}')",
                entity_id, key, prefab_key
            );
        }
    }
    let mut stat_map = StatMap::default();
    for tpl in templates {
        let base = stat_overrides.get(&tpl.key).copied().unwrap_or(tpl.base);
        if base > tpl.max {
            warn!(
                "stat_overrides: entity '{}' stat '{}' override {} exceeds template max {}; value will exceed max",
                entity_id, tpl.key, base, tpl.max
            );
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
                emit: t.emit.replace("{self}", entity_id),
            }).collect(),
        };
        stat_map.0.insert(tpl.key.clone(), LiveStat::new(def));
    }
    Some(stat_map)
}

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
                waypoint_reach_radius:      npc_def.waypoint_reach_radius,
                waypoint_wait_secs:         npc_def.waypoint_wait_secs,
                waypoint_wait_timer:        0.0,
                interact_leave_factor:      npc_def.interact_leave_factor,
                home_arrival_radius:        npc_def.home_arrival_radius,
                investigate_timeout_secs:   npc_def.investigate_timeout_secs,
                last_known_attacker_pos:    None,
                investigate_timer:          0.0,
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

    // Capability features: single call covers behavior, interactable, dialogue, inventory,
    // stat_templates, and trigger_zone — all three spawn paths now route through this helper.
    attach_prefab_features(commands, spawned.parent, prefab, project_root, asset_server, name, stat_overrides, name);

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
    nameplate_config: Res<crate::capabilities::nameplate::NameplateSceneConfig>,
    mut active_split_slot_count: ResMut<super::ActiveSplitSlotCount>,
    ring_visibility: Res<super::TargetRingVisibilityMode>,
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

        // Player-tagged prefabs spawn a full character. Hot-join (`Action::JoinPlayer`) players
        // get a camera-less body plus one incremental split camera, growing the existing Grid
        // layout live; every other player-tagged spawn keeps its own dedicated Orbit-mode camera
        // via `spawn_player_entity`, unchanged.
        if let Some(player_config) = queued.player_config {
            if queued.is_hot_join {
                let player_entity = spawn_player_entity_core(
                    &mut commands,
                    &asset_server,
                    &fixes.0,
                    &model_spawner,
                    &player_config,
                    &queued.project_root,
                    &mut registry,
                    &mut stat_ui_queue,
                    None,
                );
                let slot = player_config.player_index;
                spawn_split_camera_for_player(
                    &mut commands, active_tonemapping.0, &player_config, player_entity, slot,
                    *ring_visibility == super::TargetRingVisibilityMode::OwnViewportOnly,
                );
                active_split_slot_count.0 = Some(active_split_slot_count.0.unwrap_or(0) + 1);
                info!(
                    "Action::JoinPlayer: spawned player at slot {} (live split slots now {})",
                    slot, active_split_slot_count.0.unwrap()
                );
                continue;
            }
            // This path spawns its own dedicated full-window Orbit-mode camera (see
            // `spawn_player_entity`/`spawn_orbit_camera_for_player`), never a split-tagged one, so
            // it never carries a ring-visibility `RenderLayers`. In an `own_viewport_only` scene
            // that camera's implicit layer 0 doesn't intersect ANY ring's reserved layer — this
            // player would see zero rings, not even their own. Already an odd combination (a
            // dynamically-spawned full-window camera alongside an existing split layout), so warn
            // rather than silently plumbing a new per-player layer into a camera that doesn't
            // participate in the split layout at all.
            if *ring_visibility == super::TargetRingVisibilityMode::OwnViewportOnly {
                warn!(
                    "Action::Spawn spawned a `tags: [\"player\"]` prefab into a scene with \
                     `split.own_viewport_only: true`, but this spawn path gets its own \
                     full-window Orbit-mode camera with no ring-visibility layer — this player will see \
                     NO target rings at all (not even their own) until the scene reloads. Use \
                     hot-join (`Action::JoinPlayer`) to add players into an own_viewport_only \
                     split scene instead."
                );
            }
            spawn_player_entity(
                &mut commands,
                &asset_server,
                &fixes.0,
                &model_spawner,
                &player_config,
                &queued.project_root,
                active_tonemapping.0,
                &mut registry,
                &mut stat_ui_queue,
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

        if should_insert_nameplate(queued.prefab_def.nameplate, nameplate_config.enabled) {
            let display_name = queued.prefab_def.display_name.clone().unwrap_or_else(|| queued.prefab_key.clone());
            commands.entity(parent).insert(crate::capabilities::nameplate::NameplateTag {
                display_name,
                prefab_override: queued.prefab_def.nameplate,
            });
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
    mut stat_ui_queue: ResMut<DynamicStatUiQueue>,
) {
    if terrain_query.is_empty() {
        return;
    }

    for (pending_entity, pending, pending_tm) in &pending_query {
        info!("Terrain is ready. Spawning player(s)...");
        let tonemapping = pending_tm
            .map(|pt| pt.0)
            .unwrap_or(bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted);
        // `None`: terrain-deferred primitive-player spawn is v3-deferred (see the feature
        // plan) — `scene_loader.rs` already warns and skips a primitive player prefab combined
        // with `scene.terrain: Some(...)` before it ever reaches `PendingPlayerConfig`, so every
        // config here is guaranteed `PlayerModelSource::Glb`.
        spawn_players_and_camera(
            &mut commands,
            &asset_server,
            &merged_fixes.0,
            &model_spawner,
            &pending.0,
            &project_root.0,
            tonemapping,
            &mut registry,
            &mut stat_ui_queue,
            None,
        );
        commands.entity(pending_entity).despawn();
    }
}

/// Spawns a player's character (model, physics, controller, metadata) plus its own dedicated
/// camera, dispatched on the player's resolved `CameraModeDef` (`resolve_camera_mode`) — this is
/// the single-player path: dynamic `Action::Spawn`/`Action::JoinPlayer` (character-select, hot
/// join's non-split fallback) use it unchanged. The terrain-delayed spawn and normal scene loads
/// go through `spawn_players_and_camera` instead, which reaches the same mode-generic dispatch
/// via its own single-player branch (`entities.len() < 2`) and calls `spawn_player_entity_core`
/// directly for the 2+-player case, spawning one shared `Party`-mode camera rather than one
/// per-player camera.
pub(crate) fn spawn_player_entity(
    commands: &mut Commands,
    asset_server: &AssetServer,
    fixes: &HashMap<String, TransformFix>,
    model_spawner: &ModelSpawner,
    player_config: &PlayerConfig,
    project_root: &str,
    tonemapping: bevy::core_pipeline::tonemapping::Tonemapping,
    registry: &mut SpawnRegistry,
    stat_ui_queue: &mut DynamicStatUiQueue,
) {
    // `None`: this function's callers (dynamic `Action::Spawn`/character-select,
    // terrain-delayed spawn) never build a `Primitive` `PlayerModelSource` in v1 — that path's
    // resources aren't threaded this far yet (v3-deferred, see the feature plan). Character-
    // select already actively rejects primitive player prefabs before reaching this function.
    let player_entity = spawn_player_entity_core(
        commands, asset_server, fixes, model_spawner, player_config, project_root, registry,
        stat_ui_queue, None,
    );
    spawn_active_camera_for_player(commands, tonemapping, player_config, player_entity);
}

/// Spawns one or more players from the same scene, sharing a single camera when there are
/// 2+ of them (local co-op). A single player gets its own Orbit-mode camera, matching
/// `spawn_player_entity`'s single-player behavior exactly.
///
/// When 2+ players are present, the first player's `CameraConfig.party` block is the sole,
/// explicit switch for the shared Party-mode camera — an absent `party` block is treated as
/// a designer oversight, not "use single-player mode": rather than silently spawning
/// competing per-player cameras, this logs a warning and falls back to a single Orbit-mode camera
/// targeting only the first player.
pub(crate) fn spawn_players_and_camera(
    commands: &mut Commands,
    asset_server: &AssetServer,
    fixes: &HashMap<String, TransformFix>,
    model_spawner: &ModelSpawner,
    player_configs: &[PlayerConfig],
    project_root: &str,
    tonemapping: bevy::core_pipeline::tonemapping::Tonemapping,
    registry: &mut SpawnRegistry,
    stat_ui_queue: &mut DynamicStatUiQueue,
    mut primitive_ctx: Option<&mut PrimitivePlayerCtx<'_>>,
) {
    // Per-player targeting (capabilities/targeting.rs) treats `player_index: 0` (the default —
    // `#[serde(default)]` on `PrefabDef.player_index`) as "the primary player": the one whose
    // target mirrors into the shared `CurrentTarget` resource and fires global `target.changed`/
    // `target.cleared` events. Two players sharing that status (both explicit `0`, or both
    // omitting the field) fight over `CurrentTarget` and each emit their own global events the
    // same frame, in query-iteration-order-dependent ways — no crash, but nondeterministic
    // `{target}`-driven rule behavior. Same class of authoring mistake the split-screen HUD
    // corner label already warns designers to avoid via unique `player_index` values; this is
    // the runtime-visible half of that same requirement.
    if player_configs.iter().filter(|pc| pc.player_index == 0).count() > 1 {
        warn!(
            "Scene has 2+ players with `player_index: 0` (or `player_index` omitted, which \
             defaults to 0) — per-player targeting treats player_index 0 as the sole \"primary\" \
             player. With 2+ players sharing that status, they will fight over the shared \
             CurrentTarget resource and each emit their own global target.changed/target.cleared \
             events. Give each player a unique player_index (0, 1, 2, ...) to fix."
        );
    }

    let mut entities: Vec<Entity> = Vec::with_capacity(player_configs.len());
    for pc in player_configs {
        // Reborrow `primitive_ctx` fresh each iteration — `ChildSpawnCtx`'s `&mut Assets<...>`
        // fields can't be moved/cloned, only reborrowed for the duration of one player's spawn.
        entities.push(spawn_player_entity_core(
            commands, asset_server, fixes, model_spawner, pc, project_root, registry, stat_ui_queue,
            primitive_ctx.as_mut().map(|ctx| &mut **ctx),
        ));
    }

    let Some(first) = player_configs.first() else { return };
    if entities.len() < 2 {
        commands.insert_resource(crate::runtime::scene_manager::ActiveSplitScreen(None));
        commands.insert_resource(crate::runtime::scene_manager::DynamicSplitConfig(None));
        commands.insert_resource(crate::runtime::scene_manager::ActiveSplitSlotCount(None));
        commands.insert_resource(crate::runtime::scene_manager::TargetRingVisibilityMode::AllViewports);
        spawn_active_camera_for_player(commands, tonemapping, first, entities[0]);
        return;
    }

    let split = first.split.as_ref();
    let party = first.party.as_ref();
    if split.is_some() && party.is_some() {
        warn!(
            "Scene has both `split` and `party` set on the first player's `camera` config — \
             these are mutually exclusive. Using `split` (the more specific setting) and \
             ignoring `party`; remove one of the two blocks to silence this warning."
        );
    }

    if let Some(s) = split {
        if s.orientation == crate::schema::player::SplitOrientation::Grid && s.dynamic.is_some() {
            warn!(
                "Scene has `split.orientation: Grid` and `split.dynamic` both set on the first \
                 player's `camera` config — dynamic split only supports Vertical/Horizontal, so \
                 `dynamic` silently wins here and the scene gets a 2-way merge/split, not an \
                 N-way grid. Remove `dynamic` to get a static Grid split, or remove `orientation: \
                 Grid` (dynamic picks its own axis) to silence this warning."
            );
        }

        // `own_viewport_only` keys each player's reserved ring/camera layer on
        // `player_index % MAX_SPLIT_PLAYERS` (see `capabilities::camera::ring_layer_for_player`).
        // Two players whose `player_index` collides under that modulo (an out-of-range index like
        // 4, or a plain duplicate like two players both authoring `player_index: 1`) silently
        // defeats the whole feature — both players end up on the same reserved layer, so each
        // sees the other's ring exactly as if `own_viewport_only` were `false`. Unlike
        // `PLAYER_LABEL_COLORS`' own harmless modulo-collision precedent (a cosmetic duplicate
        // tint), this collision breaks a stated visibility guarantee with no cosmetic cue at all.
        if s.own_viewport_only {
            let mut seen_layers: HashMap<usize, u32> = HashMap::new();
            for pc in player_configs {
                let layer = crate::capabilities::camera::ring_layer_for_player(pc.player_index);
                if let Some(&other_index) = seen_layers.get(&layer) {
                    warn!(
                        "Scene has `split.own_viewport_only: true`, but players with player_index \
                         {} and {} both resolve to reserved layer {} (player_index % {} \
                         collides) — their rings/cameras will be indistinguishable from \
                         `own_viewport_only: false` for this pair; each will still see the \
                         other's ring. Give every player a unique player_index in \
                         0..{} to fix.",
                        other_index, pc.player_index, layer,
                        crate::capabilities::camera::MAX_SPLIT_PLAYERS,
                        crate::capabilities::camera::MAX_SPLIT_PLAYERS,
                    );
                }
                seen_layers.insert(layer, pc.player_index);
            }
        }
    }

    if let Some(dynamic) = split.and_then(|s| s.dynamic.as_ref()) {
        // Dynamic split (Stage 5): all 3 cameras (party + 2 split) are spawned up front and live
        // for the scene's lifetime; only `Camera.is_active` + `Camera.viewport` change at runtime
        // (see `dynamic_split_screen_system`). This works with zero new camera-following logic
        // because neither `camera_orbit_system` nor `party_camera_follow_system` gate on
        // `is_active` — an inactive camera's Transform stays correctly updated the whole time, so
        // there's no pop/snap when it reactivates.
        let (split_distance, merge_distance) = if dynamic.merge_distance < dynamic.split_distance {
            (dynamic.split_distance, dynamic.merge_distance)
        } else {
            warn!(
                "Scene's `split.dynamic.merge_distance` ({}) is not less than `split_distance` \
                 ({}) — without a gap, the merge/split state would flicker at the boundary. \
                 Clamping merge_distance just below split_distance.",
                dynamic.merge_distance, dynamic.split_distance
            );
            (dynamic.split_distance, dynamic.split_distance - 0.01)
        };
        let own_viewport_only = split.is_some_and(|s| s.own_viewport_only);
        let first_cam = resolve_orbit_config_for_multiplayer(first);
        let party_cam = crate::capabilities::camera::spawn_party_orbit_camera(
            commands, tonemapping, &first_cam,
            &crate::schema::player::PartyZoomDef {
                zoom_margin: dynamic.merged_zoom_margin,
                allow_manual_zoom: dynamic.merged_allow_manual_zoom,
            },
            &entities,
            own_viewport_only,
        );
        let p0 = Vec3::from(player_configs[0].initial_position);
        let p1 = Vec3::from(player_configs[1].initial_position);
        let dx = p1.x - p0.x;
        let dz = p1.z - p0.z;
        let starts_split = p0.distance(p1) > split_distance;
        let initial_orientation = if dx.abs() >= dz.abs() {
            crate::schema::player::SplitOrientation::Vertical
        } else {
            crate::schema::player::SplitOrientation::Horizontal
        };
        commands.entity(party_cam).insert(Camera {
            is_active: !starts_split,
            order: 2,
            ..default()
        });
        commands.insert_resource(crate::runtime::scene_manager::ActiveSplitScreen(
            if starts_split { Some(initial_orientation) } else { None },
        ));
        commands.insert_resource(crate::runtime::scene_manager::DynamicSplitConfig(Some(
            crate::schema::player::DynamicSplitDef {
                split_distance,
                merge_distance,
                merged_zoom_margin: dynamic.merged_zoom_margin,
                merged_allow_manual_zoom: dynamic.merged_allow_manual_zoom,
            },
        )));
        // Dynamic split is always a 2-way merge/split (Grid's N-way layout is static-only —
        // see the feature plan's "not in scope" list), so the slot count is never Grid-driven.
        commands.insert_resource(crate::runtime::scene_manager::ActiveSplitSlotCount(None));
        commands.insert_resource(if own_viewport_only {
            crate::runtime::scene_manager::TargetRingVisibilityMode::OwnViewportOnly
        } else {
            crate::runtime::scene_manager::TargetRingVisibilityMode::AllViewports
        });
        for (i, (config, entity)) in player_configs.iter().zip(entities.iter()).enumerate().take(2) {
            let camera_entity = spawn_orbit_camera_for_player(commands, tonemapping, config, *entity);
            commands.entity(camera_entity).insert((
                crate::capabilities::camera::SplitViewportSlot(i as u32),
                Camera {
                    is_active: starts_split,
                    order: i as isize,
                    ..default()
                },
            ));
            if own_viewport_only {
                let layer = crate::capabilities::camera::ring_layer_for_player(config.player_index);
                commands.entity(camera_entity).insert(
                    bevy::camera::visibility::RenderLayers::layer(0).with(layer),
                );
            }
        }
    } else if let Some(split) = split {
        commands.insert_resource(
            crate::runtime::scene_manager::ActiveSplitScreen(Some(split.orientation)),
        );
        commands.insert_resource(crate::runtime::scene_manager::DynamicSplitConfig(None));
        commands.insert_resource(if split.own_viewport_only {
            crate::runtime::scene_manager::TargetRingVisibilityMode::OwnViewportOnly
        } else {
            crate::runtime::scene_manager::TargetRingVisibilityMode::AllViewports
        });
        // `Vertical`/`Horizontal` stay strictly 2-way (Stages 3-5's original behavior, unchanged);
        // only `Grid` (Stage 6) unlocks N-way, capped at `MAX_SPLIT_PLAYERS` to bound render-pass
        // count and avoid degenerate slivers on a misconfigured scene.
        let slot_count: u32 = if split.orientation == crate::schema::player::SplitOrientation::Grid {
            let max = crate::capabilities::camera::MAX_SPLIT_PLAYERS;
            if entities.len() as u32 > max {
                warn!(
                    "Scene has {} players with `split.orientation: Grid`, but MAX_SPLIT_PLAYERS \
                     is {} — the extra player(s) spawn without a camera (cameraless, but still \
                     playable). Reduce the player count or raise MAX_SPLIT_PLAYERS to silence \
                     this warning.",
                    entities.len(), max
                );
            }
            (entities.len() as u32).min(max)
        } else {
            2
        };
        commands.insert_resource(crate::runtime::scene_manager::ActiveSplitSlotCount(
            if split.orientation == crate::schema::player::SplitOrientation::Grid {
                Some(slot_count)
            } else {
                None
            },
        ));
        for (i, (config, entity)) in player_configs.iter().zip(entities.iter())
            .enumerate().take(slot_count as usize)
        {
            spawn_split_camera_for_player(
                commands, tonemapping, config, *entity, i as u32, split.own_viewport_only,
            );
        }
    } else if let Some(party) = party {
        commands.insert_resource(crate::runtime::scene_manager::ActiveSplitScreen(None));
        commands.insert_resource(crate::runtime::scene_manager::DynamicSplitConfig(None));
        commands.insert_resource(crate::runtime::scene_manager::ActiveSplitSlotCount(None));
        commands.insert_resource(crate::runtime::scene_manager::TargetRingVisibilityMode::AllViewports);
        let first_cam = resolve_orbit_config_for_multiplayer(first);
        crate::capabilities::camera::spawn_party_orbit_camera(
            commands, tonemapping, &first_cam, party, &entities, false,
        );
    } else {
        commands.insert_resource(crate::runtime::scene_manager::ActiveSplitScreen(None));
        commands.insert_resource(crate::runtime::scene_manager::DynamicSplitConfig(None));
        commands.insert_resource(crate::runtime::scene_manager::ActiveSplitSlotCount(None));
        commands.insert_resource(crate::runtime::scene_manager::TargetRingVisibilityMode::AllViewports);
        warn!(
            "Scene has {} players but no `party` or `split` camera block on the first player's \
             `camera` config — falling back to a single Orbit-mode camera targeting only the first \
             player. Add a `party: (zoom_margin: ...)` or `split: (orientation: Vertical)` block \
             to the first player's camera config to get a shared or split local co-op camera.",
            entities.len()
        );
        spawn_orbit_camera_for_player(commands, tonemapping, first, entities[0]);
    }
}

/// Bundles everything a `PlayerModelSource::Primitive` body needs that a `Glb` one doesn't —
/// mesh/material asset access, the built-materials memo, cosmetic-children resolution, and
/// error collection. `None` at every caller except the immediate (non-terrain) scene-load path,
/// which is the only one with these resources in scope — see the resource-threading note in
/// `planning/features/player_model_source_unification.md`. If `spawn_player_entity_core`
/// ever receives a `Primitive` `model_source` with no ctx, that's a v1-scope violation (only the
/// scene-load path builds `Primitive` configs at all) and it panics rather than silently
/// constructing a broken player.
pub(crate) struct PrimitivePlayerCtx<'a> {
    pub(crate) child_ctx: ChildSpawnCtx<'a>,
    pub(crate) prefab_catalog: &'a crate::schema::catalog::PrefabCatalog,
    pub(crate) load_errors: &'a mut Vec<String>,
}

fn spawn_player_entity_core(
    commands: &mut Commands,
    asset_server: &AssetServer,
    fixes: &HashMap<String, TransformFix>,
    model_spawner: &ModelSpawner,
    player_config: &PlayerConfig,
    project_root: &str,
    registry: &mut SpawnRegistry,
    stat_ui_queue: &mut DynamicStatUiQueue,
    primitive_ctx: Option<&mut PrimitivePlayerCtx<'_>>,
) -> Entity {
    // Body construction dispatches on `model_source`; everything after this match is shared
    // unconditionally by both variants (PlayerIndex, StatMap, material override, nameplate, stat
    // widgets) — that sharing is the whole point of this unification.
    let (player_entity, cap_radius, player_height, glb_anim): (Entity, f32, f32, Option<(String, Handle<Gltf>)>) = match &player_config.model_source {
        PlayerModelSource::Glb(model_path) => {
            let gltf_path = model_path.split('#').next().unwrap_or("").to_string();
            let gltf_handle = asset_server.load(gltf_path.clone());
            let spawned = model_spawner.spawn_instance(
                commands, asset_server, fixes, model_path.clone(),
                Transform::from_translation(Vec3::from(player_config.initial_position)),
            );
            let mv = &player_config.movement;
            let cap_radius = mv.collider_radius.unwrap_or(0.4);
            let player_height = mv.collider_height.unwrap_or(1.8);
            (spawned.parent, cap_radius, player_height, Some((gltf_path, gltf_handle)))
        }
        PlayerModelSource::Primitive { shape, params, children } => {
            let ctx = primitive_ctx.expect(
                "PlayerModelSource::Primitive requires a PrimitivePlayerCtx — only the immediate \
                 scene-load path builds Primitive player configs in v1, and it always supplies one",
            );
            let cap_radius = params.radius.unwrap_or(0.4);
            let player_height = params.height.unwrap_or(1.8);
            let cap_half = (player_height / 2.0 - cap_radius).max(0.0);
            let body_y = cap_half + cap_radius;

            let mesh = build_primitive_mesh(shape, params);
            let mesh_handle = ctx.child_ctx.meshes.add(mesh);
            let mat_handle = ctx.child_ctx.standard.add(
                primitive_material(params, ctx.child_ctx.primitive_default_color),
            );

            // `Name::new("Player")` is applied once, unconditionally, in the shared section below
            // (same as the GLB arm) — not duplicated here.
            let player_entity = commands.spawn((
                Transform::from_translation(Vec3::from(player_config.initial_position)),
                Visibility::default(),
            )).id();

            // Visual body child — mesh centred at body_y above the feet so it aligns with the
            // compound collider inserted in the shared section below.
            let mesh_child = commands.spawn((
                Name::new("Player Body"),
                Mesh3d(mesh_handle),
                MeshMaterial3d(mat_handle),
                Transform::from_xyz(0.0, body_y, 0.0),
                Visibility::default(),
            )).id();
            commands.entity(player_entity).add_child(mesh_child);

            // Cosmetic children (cap, eyes, nose, etc.) defined in the prefab. Offsets are
            // relative to the entity origin (feet), matching every other primitive prefab.
            //
            // `physics`/`sensor` children are rejected here, not passed through: on a non-player
            // composite prefab those flags attach a *second* Rapier collider to the shared parent
            // and (for `physics`) push `RigidBody::Fixed` onto it — fine for a static prop, but on
            // a player that Fixed insert would race the shared section's `RigidBody::Dynamic`
            // insert below and can silently freeze the player solid depending on command order.
            // Found during `player_model_source_unification.md` v2 review — `children:` on a
            // player prefab is new as of this feature, so nothing has copied a physics child onto
            // one yet, but room10's prefab is the reference example designers will copy from.
            let safe_children: Vec<crate::schema::catalog::ChildPrimitiveDef> = children.iter()
                .filter(|c| {
                    if c.primitive.physics || c.primitive.sensor {
                        ctx.load_errors.push(format!(
                            "player '{}': a `children:` entry has `physics`/`sensor: true` — not \
                             supported on a player prefab (it would conflict with the player's own \
                             RigidBody). Skipping this child; remove `physics`/`sensor` from it.",
                            player_config.spawn_id
                        ));
                        false
                    } else {
                        true
                    }
                })
                .cloned()
                .collect();
            spawn_primitive_children(
                commands, player_entity, &safe_children, ctx.prefab_catalog, &mut ctx.child_ctx,
                ctx.load_errors, &player_config.spawn_id, 0, &mut HashSet::new(),
                Transform::IDENTITY,
            );

            (player_entity, cap_radius, player_height, None)
        }
    };

    if let Some(mat_key) = &player_config.material {
        commands.entity(player_entity).insert(PendingMaterialOverride(mat_key.clone()));
    }
    let mv = &player_config.movement;
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
        // Required by player_movement_system's query. Without it the player is silently
        // filtered out of that query and never moves.
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
        // Low, non-zero friction (not the primitive-only path's old 0.0) — `Min` still keeps a
        // capsule from catching hard on cube/step edges, but 0.0 let a real hillside playtest
        // (`player_model_source_unification.md` v2, `quick_scene`) show a visible, permanent
        // downhill creep for an idle player: movement writes `velocity.linvel` directly each
        // tick, so friction was never doing much *while moving*, but an idle body on a slope has
        // nothing but `idle_drag` (`MovementConfig`) opposing gravity's tangential component, and
        // `idle_drag` only bounds that creep asymptotically — it can't zero it, and pushing it low
        // enough to matter also cancels horizontal air momentum right after releasing input
        // mid-jump (same multiply runs in `capabilities/player.rs` with no grounded gate). `0.15`
        // is a real, if modest, static-friction coefficient the physics solver applies against
        // gravity directly, confirmed via playtest to hold a slope without noticeably reintroducing
        // edge-catching (`Min` still discounts to whichever surface's coefficient is lower).
        Friction { coefficient: 0.15, combine_rule: CoefficientCombineRule::Min },
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
    commands.entity(player_entity).insert((
        crate::capabilities::player::Player,
        crate::capabilities::player::PlayerOwnership::Local,
        crate::capabilities::player::PlayerIndex(player_config.player_index),
        crate::capabilities::player::PlayerTarget::default(),
        // `None` for every scene-load-time player (resolved by `gamepad_bind_system`'s pending
        // retry from `InputMap.gamepad_index`); `Some(entity)` only for a hot-joined player whose
        // triggering gamepad was already captured — see `PlayerConfig.bound_gamepad`'s doc comment.
        crate::capabilities::player::BoundGamepad(player_config.bound_gamepad),
    ));

    // Gives this player their own stat pool (e.g. a per-player action-bar mana cost) when their
    // prefab declares `stat_templates` — empty by default, so most players get no `StatMap` and
    // `SlotCost` keeps reading the global `LoadedStats` exactly as before this field existed.
    // See `planning/features/per_player_stat_pools.md`.
    if let Some(stat_map) = build_stat_map_from_templates(
        &player_config.stat_templates,
        &HashMap::new(),
        &player_config.spawn_id,
        &player_config.prefab_key,
    ) {
        commands.entity(player_entity).insert(stat_map);
    }

    // Gives this player a floating stat_label/world_stat_bar widget when their prefab declares
    // one — routed through the same DynamicStatUiQueue/drain_dynamic_stat_ui_system mechanism
    // NPC/prop Action::Spawn entities use (mirrors drain_spawn_queue_system's own push below),
    // rather than a player-specific spawn path. `{self}` is resolved against this player's own
    // spawn_id, exactly like every other entity kind. See
    // `planning/features/player_stat_widgets.md`.
    let stat_label = player_config.stat_label.as_ref().map(|sl| {
        let key = sl.stat_key.replace("{self}", &player_config.spawn_id);
        (key, sl.clone())
    });
    let world_stat_bar = player_config.world_stat_bar.as_ref().map(|wb| {
        let key = wb.stat_key.replace("{self}", &player_config.spawn_id);
        (key, wb.clone())
    });
    if stat_label.is_some() || world_stat_bar.is_some() {
        stat_ui_queue.0.push(super::DynamicStatUiEntry {
            entity: player_entity, stat_label, world_stat_bar,
        });
    }

    if let Some(display_name) = &player_config.nameplate_display_name {
        commands.entity(player_entity).insert(crate::capabilities::nameplate::NameplateTag {
            display_name: display_name.clone(),
            prefab_override: player_config.nameplate_override,
        });
    }

    // Animation policy only applies to GLB players — a primitive body has no skeleton/animation
    // graph to drive. `assemble_player_config` sets `animation_policy` from the prefab
    // unconditionally regardless of model source, so this gate (not just `animation_policy.
    // is_some()`) is what actually prevents a copy-pasted `animation_policy` field on a
    // primitive player prefab from trying to load a policy against a nonexistent GLTF.
    if let Some((gltf_path, gltf_handle)) = glb_anim {
        if let Some(rel) = player_config.animation_policy.as_deref() {
            let path = resolve_project_path(project_root, rel);
            info!("Loading AnimationPolicy from: {}", path);
            let policy_handle: Handle<AnimationPolicy> = asset_server.load(path);
            commands.entity(player_entity).insert((
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
            ));
        }
    }

    player_entity
}

/// Spawns one Orbit-mode camera for `player_config`/`player_entity` tagged for split-screen grid
/// slot `slot` (`SplitViewportSlot(slot)` + `Camera { order: slot, .. }`). Shared by the
/// static-`Grid`-branch loop in `spawn_players_and_camera` (initial scene load) and by
/// `drain_spawn_queue_system`'s `is_hot_join` branch (`Action::JoinPlayer`) — the only piece of
/// the multi-player spawn logic this feature factors out; nothing else about
/// `spawn_players_and_camera`'s collection/dispatch changes. See
/// `planning/features/local_coop_hot_join_leave.md`.
///
/// `own_viewport_only` (resolved by each caller from `TargetRingVisibilityMode` — the hot-join
/// caller reads the resource instead of an in-scope `SplitScreenDef` since `drain_spawn_queue_system`
/// has no scene config in scope at all, not because of any frame-ordering concern; the resource
/// write and the hot-joined player's own spawn both land in the same command buffer regardless)
/// inserts a `RenderLayers` restricting this camera to layer 0 (ordinary scene geometry) plus this
/// player's own reserved ring layer, keyed on `player_config.player_index` (not `slot`, which can
/// diverge) — see `planning/features/per_viewport_target_ring_visibility.md`.
pub(crate) fn spawn_split_camera_for_player(
    commands: &mut Commands,
    tonemapping: bevy::core_pipeline::tonemapping::Tonemapping,
    player_config: &PlayerConfig,
    player_entity: Entity,
    slot: u32,
    own_viewport_only: bool,
) -> Entity {
    let camera_entity = spawn_orbit_camera_for_player(commands, tonemapping, player_config, player_entity);
    // All split cameras render to the same window at Camera's default order (0), which Bevy's
    // ambiguity detection flags every frame even though non-overlapping viewports make the
    // render order harmless. Giving each slot a distinct order silences it and makes the
    // (harmless-but-real) N-passes-per-frame render order explicit/deterministic.
    commands.entity(camera_entity).insert((
        crate::capabilities::camera::SplitViewportSlot(slot),
        Camera {
            order: slot as isize,
            ..default()
        },
    ));
    if own_viewport_only {
        let layer = crate::capabilities::camera::ring_layer_for_player(player_config.player_index);
        commands.entity(camera_entity).insert(
            bevy::camera::visibility::RenderLayers::layer(0).with(layer),
        );
    }
    camera_entity
}

/// Spawns a single-target `Orbit`-mode camera following `player_entity`, per
/// `player_config.camera`. Factored out of `spawn_player_entity` so the local-coop split/party
/// dispatch in `spawn_players_and_camera` can reuse it without duplicating the field mapping —
/// unconditionally Orbit, since split-screen per-player cameras and the fallback-to-Orbit warning
/// path are not (in v1) mode-generic; only the single-player fallback branch is (see
/// `spawn_active_camera_for_player`).
/// Returns the spawned camera entity so callers needing to attach extra components (e.g.
/// `SplitViewportSlot` for local co-op split-screen) can do so without this function needing to
/// know about them.
fn spawn_orbit_camera_for_player(
    commands: &mut Commands,
    tonemapping: bevy::core_pipeline::tonemapping::Tonemapping,
    player_config: &PlayerConfig,
    player_entity: Entity,
) -> Entity {
    let cam = resolve_orbit_config_for_multiplayer(player_config);
    let entity = spawn_orbit_camera_from_config(commands, tonemapping, &cam, &player_config.inputs, player_config.initial_position, player_entity);
    commands.entity(entity).insert(crate::capabilities::camera::AuthoredCameraMode(CameraModeDef::Orbit(cam)));
    entity
}

/// Resolves the effective `CameraConfig` for the local-coop split/party/dynamic dispatch, which
/// (unlike the single-player fallback's `spawn_active_camera_for_player`) is not mode-generic in
/// v1 — only `Orbit` is supported there. Bug fix (found by 4 independent post-implementation
/// reviews): the split/party paths originally read `player_config.camera` directly, silently
/// ignoring an authored `camera_mode: Orbit(...)` and falling back to `default_camera_config()`
/// whenever a migrated prefab dropped its legacy `camera:` block — exactly what
/// `local_coop_demo`'s `player_p1_split_h`/`player_p2_split_h` (room4) did, regressing their
/// `orbit_button: "None"`/`zoom_speed: 0.0` split-screen mouse-decoupling.
fn resolve_orbit_config_for_multiplayer(pc: &PlayerConfig) -> CameraConfig {
    match &pc.camera_mode {
        Some(CameraModeDef::Orbit(cfg)) => cfg.clone(),
        Some(other) => {
            warn!(
                "player '{}': camera_mode {:?} is not supported for a local-coop split/party \
                 player in v1 — falling back to the legacy `camera:` tuning (or engine defaults if \
                 that's also absent). Only `camera_mode: Orbit(...)` is supported for split-screen/ \
                 party players; per-player mode diversity in co-op is v2 scope.",
                pc.spawn_id, other
            );
            pc.camera.clone()
        }
        None => pc.camera.clone(),
    }
}

/// `inputs: None` (**v2**, `apply_camera_mode`'s switch-time path for a camera with no owning
/// player — `CameraTargets` empty) falls back to no keyboard-look bindings and a neutral gamepad
/// deadzone, rather than requiring a full `InputMap` to exist for a camera that has none to draw
/// from. Every spawn-time caller always has a real player's `InputMap` and passes `Some(inputs)`.
fn orbit_state_from_config(cam: &CameraConfig, inputs: Option<&InputMap>) -> OrbitState {
    let (orbit_lmb, orbit_rmb) = crate::capabilities::camera::parse_orbit_button(&cam.orbit_button);
    let (char_rot_lmb, char_rot_rmb) = cam.character_rotate_button
        .as_deref()
        .map(crate::capabilities::camera::parse_orbit_button)
        .unwrap_or((false, false));
    OrbitState {
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
        look_left_key: inputs.and_then(|i| i.look_left.as_deref().and_then(InputMap::parse_key)),
        look_right_key: inputs.and_then(|i| i.look_right.as_deref().and_then(InputMap::parse_key)),
        look_up_key: inputs.and_then(|i| i.look_up.as_deref().and_then(InputMap::parse_key)),
        look_down_key: inputs.and_then(|i| i.look_down.as_deref().and_then(InputMap::parse_key)),
        look_speed: cam.look_speed,
        gamepad_deadzone: inputs.map_or(0.15, |i| i.gamepad_deadzone),
    }
}

fn spawn_orbit_camera_from_config(
    commands: &mut Commands,
    tonemapping: bevy::core_pipeline::tonemapping::Tonemapping,
    cam: &CameraConfig,
    inputs: &InputMap,
    target_initial_position: (f32, f32, f32),
    player_entity: Entity,
) -> Entity {
    let start_pos = Vec3::from(target_initial_position) + Vec3::from(cam.offset);
    let entity = commands.spawn((
        Name::new("Orbit Camera"),
        Camera3d::default(),
        tonemapping,
        Transform::from_translation(start_pos)
            .looking_at(Vec3::from(target_initial_position), Vec3::Y),
        LevelEntity,
        ActiveCameraMode::Orbit(orbit_state_from_config(cam, Some(inputs))),
        OrbitCameraMode,
        CameraTargets(vec![player_entity]),
    )).id();
    insert_fov(commands, entity, cam.fov);
    entity
}

/// Inserts a `Projection::Perspective` with the given field-of-view (degrees), overriding Bevy's
/// default FOV. Static in v1 — see `planning/features/camera_modes.md`'s `fov:` scope note.
fn insert_fov(commands: &mut Commands, entity: Entity, fov_degrees: f32) {
    commands.entity(entity).insert(Projection::Perspective(PerspectiveProjection {
        fov: fov_degrees.to_radians(),
        ..default()
    }));
}

/// Builds and applies the runtime `ActiveCameraMode` + matching marker component for `mode` onto
/// an existing camera `entity` — the switch-time analog of `spawn_active_camera_for_player`'s
/// per-mode match arms (**v2**, `Action::SetCameraMode`). Instantly applies the resolved FOV too
/// (`insert_fov`) — for an instant cut that's the whole story; for a blended transition, the
/// caller additionally inserts `CameraBlendState`, which overwrites both `Transform` and this FOV
/// value every frame until the blend completes (see `capabilities::camera::camera_blend_system`).
///
/// Deliberately does NOT touch `CameraTargets` (ownership is fixed at spawn time and never changes
/// on a mode switch) or otherwise duplicate `Transform` — the newly-active mode's own per-frame
/// system computes that the very same frame, once the marker swap below (via `Commands`) takes
/// effect. `Party(...)` is rejected (returns `None`, no-op): it's excluded from the
/// `camera_modes:` registry and from the `"default"` restore path (whose `AuthoredCameraMode` is
/// never literally `Party` on a per-player camera — see that component's own doc), so this should
/// be unreachable; kept defensive rather than panicking if it ever is.
///
/// Some duplication with `spawn_active_camera_for_player`'s per-mode field-mapping exists here
/// deliberately — unifying "construct runtime state from a `CameraModeDef`" across spawn-time and
/// switch-time would touch the already-shipped v1 spawn path for a v2-only feature, which is a
/// real DRY win but adds regression risk; logged as a `claude_suggestions.md` candidate rather
/// than attempted in this pass.
pub(crate) fn apply_camera_mode(
    commands: &mut Commands,
    entity: Entity,
    mode: &CameraModeDef,
    inputs: Option<&InputMap>,
) -> Option<f32> {
    if matches!(mode, CameraModeDef::Party(_)) {
        warn!(
            "Action::SetCameraMode: target mode is Party(...), which cannot be switched to via \
             SetCameraMode — camera mode left unchanged"
        );
        return None;
    }
    {
        let mut ec = commands.entity(entity);
        ec.remove::<OrbitCameraMode>();
        ec.remove::<crate::capabilities::camera::PartyCameraMode>();
        ec.remove::<crate::capabilities::camera::FixedCameraMode>();
        ec.remove::<crate::capabilities::camera::FollowCameraMode>();
        ec.remove::<crate::capabilities::camera::FirstPersonCameraMode>();
        ec.remove::<crate::capabilities::camera::FlycamCameraMode>();
    }
    let fov = match mode {
        CameraModeDef::Orbit(cam) => {
            commands.entity(entity).insert((
                ActiveCameraMode::Orbit(orbit_state_from_config(cam, inputs)),
                OrbitCameraMode,
            ));
            cam.fov
        }
        CameraModeDef::Follow(f) => {
            commands.entity(entity).insert((
                ActiveCameraMode::Follow(crate::capabilities::camera::FollowState {
                    offset: Vec3::from(f.offset),
                    look_at_offset: Vec3::from(f.look_at_offset),
                    smoothing: f.smoothing,
                    rotation_smoothing: f.rotation_smoothing,
                }),
                crate::capabilities::camera::FollowCameraMode,
            ));
            f.fov
        }
        CameraModeDef::FirstPerson(fp) => {
            commands.entity(entity).insert((
                ActiveCameraMode::FirstPerson(crate::capabilities::camera::FirstPersonState {
                    eye_offset: Vec3::from(fp.eye_offset),
                    sensitivity: fp.sensitivity,
                    pitch: 0.0,
                    min_pitch: fp.min_pitch,
                    max_pitch: fp.max_pitch,
                }),
                crate::capabilities::camera::FirstPersonCameraMode,
            ));
            fp.fov
        }
        CameraModeDef::Fixed(fx) => {
            commands.entity(entity).insert((
                ActiveCameraMode::Fixed(crate::capabilities::camera::FixedState {
                    look_at: fx.look_at.map(Vec3::from),
                    look_at_entity: fx.look_at_entity.clone(),
                }),
                crate::capabilities::camera::FixedCameraMode,
            ));
            fx.fov
        }
        CameraModeDef::Flycam(fc) => {
            use crate::capabilities::flycam::parse_flycam_look_button;
            let (look_lmb, look_rmb) = parse_flycam_look_button(&fc.look_button);
            commands.entity(entity).insert((
                ActiveCameraMode::Flycam(crate::capabilities::camera::FlycamState {
                    speed: fc.speed,
                    fast_speed: fc.fast_speed,
                    sensitivity: fc.sensitivity,
                    pitch: 0.0,
                    yaw: 0.0,
                    key_forward:  InputMap::parse_key(&fc.forward).unwrap_or(KeyCode::KeyW),
                    key_backward: InputMap::parse_key(&fc.backward).unwrap_or(KeyCode::KeyS),
                    key_left:     InputMap::parse_key(&fc.left).unwrap_or(KeyCode::KeyA),
                    key_right:    InputMap::parse_key(&fc.right).unwrap_or(KeyCode::KeyD),
                    key_up:       InputMap::parse_key(&fc.up).unwrap_or(KeyCode::Space),
                    key_down:     InputMap::parse_key(&fc.down).unwrap_or(KeyCode::KeyQ),
                    look_lmb,
                    look_rmb,
                }),
                crate::capabilities::camera::FlycamCameraMode,
            ));
            // FlyCamDef has no `fov:` field (v1 never gave Flycam one — spawn-time never called
            // `insert_fov` for it either, leaving Bevy's real default). 45.0 reproduces that
            // exact default explicitly here, since `apply_camera_mode` always calls `insert_fov`
            // (unlike the spawn-time match arms, which only did for the modes that have the field).
            45.0
        }
        CameraModeDef::Party(_) => unreachable!("rejected above"),
    };
    insert_fov(commands, entity, fov);
    Some(fov)
}

/// Resolves a player's effective `CameraModeDef` — the explicit `camera_mode` override if set,
/// else `Orbit` built from the always-defaulted legacy `camera` field. No field-presence
/// detection needed here: both `PlayerConfig.camera` and `PlayerConfig.camera_mode` are already
/// fully resolved (defaulted where absent) by `assemble_player_config`, so this is a pure
/// override-or-fallback, not a tag re-check.
pub(crate) fn resolve_camera_mode(pc: &PlayerConfig) -> CameraModeDef {
    pc.camera_mode.clone().unwrap_or_else(|| CameraModeDef::Orbit(pc.camera.clone()))
}

/// Mode-generic single-player camera spawn — dispatches on `resolve_camera_mode`, covering
/// `Orbit`/`Follow`/`FirstPerson`/`Fixed`/`Flycam` (`Party` has no single-target meaning; it warns
/// and falls back to `Orbit`). Used only by `spawn_players_and_camera`'s single-player fallback
/// branch — the local-coop split/party dispatch stays Orbit-only via `spawn_orbit_camera_for_player`
/// (not a v1 requirement to generalize; see `planning/features/camera_modes.md`).
fn spawn_active_camera_for_player(
    commands: &mut Commands,
    tonemapping: bevy::core_pipeline::tonemapping::Tonemapping,
    player_config: &PlayerConfig,
    player_entity: Entity,
) -> Entity {
    let initial_pos = Vec3::from(player_config.initial_position);
    match resolve_camera_mode(player_config) {
        CameraModeDef::Orbit(cam) => {
            let entity = spawn_orbit_camera_from_config(commands, tonemapping, &cam, &player_config.inputs, player_config.initial_position, player_entity);
            commands.entity(entity).insert(crate::capabilities::camera::AuthoredCameraMode(CameraModeDef::Orbit(cam)));
            entity
        }
        CameraModeDef::Flycam(fc) => {
            use crate::capabilities::flycam::parse_flycam_look_button;
            let (look_lmb, look_rmb) = parse_flycam_look_button(&fc.look_button);
            let entity = commands.spawn((
                Name::new("Flycam"),
                Camera3d::default(),
                tonemapping,
                Transform::from_translation(initial_pos),
                LevelEntity,
                ActiveCameraMode::Flycam(crate::capabilities::camera::FlycamState {
                    speed: fc.speed,
                    fast_speed: fc.fast_speed,
                    sensitivity: fc.sensitivity,
                    pitch: 0.0,
                    yaw: 0.0,
                    key_forward:  InputMap::parse_key(&fc.forward).unwrap_or(KeyCode::KeyW),
                    key_backward: InputMap::parse_key(&fc.backward).unwrap_or(KeyCode::KeyS),
                    key_left:     InputMap::parse_key(&fc.left).unwrap_or(KeyCode::KeyA),
                    key_right:    InputMap::parse_key(&fc.right).unwrap_or(KeyCode::KeyD),
                    key_up:       InputMap::parse_key(&fc.up).unwrap_or(KeyCode::Space),
                    key_down:     InputMap::parse_key(&fc.down).unwrap_or(KeyCode::KeyQ),
                    look_lmb,
                    look_rmb,
                }),
                crate::capabilities::camera::FlycamCameraMode,
                CameraTargets::default(),
            )).id();
            commands.entity(entity).insert(crate::capabilities::camera::AuthoredCameraMode(CameraModeDef::Flycam(fc)));
            entity
        }
        CameraModeDef::Follow(f) => {
            let start_pos = initial_pos + Vec3::from(f.offset);
            let entity = commands.spawn((
                Name::new("Follow Camera"),
                Camera3d::default(),
                tonemapping,
                Transform::from_translation(start_pos)
                    .looking_at(initial_pos + Vec3::from(f.look_at_offset), Vec3::Y),
                LevelEntity,
                ActiveCameraMode::Follow(crate::capabilities::camera::FollowState {
                    offset: Vec3::from(f.offset),
                    look_at_offset: Vec3::from(f.look_at_offset),
                    smoothing: f.smoothing,
                    rotation_smoothing: f.rotation_smoothing,
                }),
                crate::capabilities::camera::FollowCameraMode,
                CameraTargets(vec![player_entity]),
            )).id();
            insert_fov(commands, entity, f.fov);
            commands.entity(entity).insert(crate::capabilities::camera::AuthoredCameraMode(CameraModeDef::Follow(f)));
            entity
        }
        CameraModeDef::FirstPerson(fp) => {
            let entity = commands.spawn((
                Name::new("First Person Camera"),
                Camera3d::default(),
                tonemapping,
                Transform::from_translation(initial_pos + Vec3::from(fp.eye_offset)),
                LevelEntity,
                ActiveCameraMode::FirstPerson(crate::capabilities::camera::FirstPersonState {
                    eye_offset: Vec3::from(fp.eye_offset),
                    sensitivity: fp.sensitivity,
                    pitch: 0.0,
                    min_pitch: fp.min_pitch,
                    max_pitch: fp.max_pitch,
                }),
                crate::capabilities::camera::FirstPersonCameraMode,
                CameraTargets(vec![player_entity]),
            )).id();
            insert_fov(commands, entity, fp.fov);
            commands.entity(entity).insert(crate::capabilities::camera::AuthoredCameraMode(CameraModeDef::FirstPerson(fp)));
            entity
        }
        CameraModeDef::Fixed(fx) => {
            if fx.look_at.is_some() && fx.look_at_entity.is_some() {
                warn!(
                    "player '{}': `camera_mode: Fixed(...)` has both `look_at` and \
                     `look_at_entity` set — `look_at_entity` wins (re-resolved every frame); \
                     remove one to silence this warning.",
                    player_config.spawn_id
                );
            } else if fx.look_at.is_none() && fx.look_at_entity.is_none() {
                warn!(
                    "player '{}': `camera_mode: Fixed(...)` has neither `look_at` nor \
                     `look_at_entity` set — the camera will sit at `position` with no rotation \
                     applied (facing -Z) and never turn to look at anything.",
                    player_config.spawn_id
                );
            }
            let entity = commands.spawn((
                Name::new("Fixed Camera"),
                Camera3d::default(),
                tonemapping,
                Transform::from_translation(Vec3::from(fx.position)),
                LevelEntity,
                ActiveCameraMode::Fixed(crate::capabilities::camera::FixedState {
                    look_at: fx.look_at.map(Vec3::from),
                    look_at_entity: fx.look_at_entity.clone(),
                }),
                crate::capabilities::camera::FixedCameraMode,
                CameraTargets::default(),
            )).id();
            insert_fov(commands, entity, fx.fov);
            commands.entity(entity).insert(crate::capabilities::camera::AuthoredCameraMode(CameraModeDef::Fixed(fx)));
            entity
        }
        CameraModeDef::Party(_) => {
            warn!(
                "player '{}': `camera_mode: Party(...)` has no meaning for a single player — \
                 falling back to Orbit. Party mode is derived automatically from the local-coop \
                 `party:`/`split:` sibling fields instead of being authored directly.",
                player_config.spawn_id
            );
            // `spawn_orbit_camera_for_player` -> `resolve_orbit_config_for_multiplayer` matches
            // this same `Some(Party(_))` shape and falls back to `player_config.camera.clone()`
            // (its `Some(other)` arm) — record that exact value here directly rather than calling
            // `resolve_orbit_config_for_multiplayer` a second time, which would re-warn with a
            // split/party-scoped message that doesn't apply to this single-player fallback path.
            let entity = spawn_orbit_camera_for_player(commands, tonemapping, player_config, player_entity);
            commands.entity(entity).insert(crate::capabilities::camera::AuthoredCameraMode(CameraModeDef::Orbit(player_config.camera.clone())));
            entity
        }
    }
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
        party: None,
        split: None,
        look_speed: 2.0,
        fov: crate::schema::player::default_fov(),
        transition: None,
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
        gamepad_index: None,
        look_left: None,
        look_right: None,
        look_up: None,
        look_down: None,
        gamepad_jump: "South".to_string(),
        gamepad_run: "East".to_string(),
        gamepad_interact: "West".to_string(),
        gamepad_target_next: "North".to_string(),
        gamepad_deadzone: 0.15,
    }
}

/// Builds a `PlayerConfig` from a `tags: ["player"]` prefab. Single source of truth for the
/// sites that assemble one by hand: the scene-load GLB/primitive collector (`scene_loader.rs`)
/// and the dynamic `Action::Spawn` character-select path (`action_executor.rs`, GLB only — see
/// `planning/features/player_model_source_unification.md`'s v1 scope). Adding a new
/// `PlayerConfig` field means editing this function once instead of every call site.
///
/// `model_path` is only meaningful for a GLB (`PrefabKind::Actor`/etc.) prefab — callers resolve
/// it from the asset catalog themselves (that lookup can fail) and pass `None` for a
/// `PrefabKind::Primitive` prefab, whose body comes from `prefab.shape`/`primitive`/`children`
/// instead. Dispatches on `prefab.kind`, not `shape`/`children` presence — a valid primitive
/// prefab may have `shape: None` (defaults to `Capsule3d`) and empty `children`, which a
/// presence-based check would misclassify as GLB.
pub(crate) fn assemble_player_config(
    prefab: &crate::schema::catalog::PrefabDef,
    prefab_key: &str,
    spawn_id: &str,
    model_path: Option<String>,
    initial_position: (f32, f32, f32),
    player_nameplate_enabled: bool,
) -> PlayerConfig {
    let model_source = if prefab.kind == crate::schema::catalog::PrefabKind::Primitive {
        PlayerModelSource::Primitive {
            shape: prefab.shape.clone().unwrap_or(crate::schema::catalog::PrimitiveShapeKind::Capsule3d),
            params: prefab.primitive.clone().unwrap_or_default(),
            children: prefab.children.clone(),
        }
    } else {
        if prefab.animation_policy.is_none() {
            warn!(
                "Player prefab '{}' has no animation_policy — no animations will play. \
                 Set animation_policy in prefabs.ron to enable locomotion animation.",
                prefab_key
            );
        }
        PlayerModelSource::Glb(model_path.unwrap_or_default())
    };
    let inputs = prefab.components.inputs.clone().unwrap_or_else(default_input_map);
    for (field, name) in [
        ("gamepad_jump", &inputs.gamepad_jump),
        ("gamepad_run", &inputs.gamepad_run),
        ("gamepad_interact", &inputs.gamepad_interact),
        ("gamepad_target_next", &inputs.gamepad_target_next),
    ] {
        if crate::schema::player::InputMap::parse_gamepad_button(name).is_none() {
            warn!(
                "Player prefab '{}': inputs.{} has an unrecognised gamepad button name {:?} — \
                 that action will never fire from a gamepad",
                prefab_key, field, name
            );
        }
    }
    // `split`/`party` nested INSIDE a `camera_mode: Orbit(...)` payload parse fine (CameraConfig
    // still has those fields, no deny_unknown_fields) but are never read — split:/party: must be
    // siblings of camera_mode, not nested in its payload (Blocker 4). Warn rather than silently
    // dropping a designer's likely-intended config (found by post-implementation review).
    if let Some(crate::schema::camera::CameraModeDef::Orbit(cfg)) = &prefab.components.camera_mode {
        if cfg.split.is_some() || cfg.party.is_some() {
            warn!(
                "player prefab '{}': `split`/`party` authored INSIDE `camera_mode: Orbit(...)` are \
                 never read and have no effect — they must be siblings of `camera_mode`, e.g. \
                 `components: (camera_mode: Orbit(...), split: (...))`. See \
                 planning/features/camera_modes.md's \"Unified camera modes\" section.",
                prefab_key
            );
        }
    }
    // `split`/`party` resolve the new sibling `components:` field first, falling back to the
    // legacy nested `camera.split`/`camera.party` — both already-optional either way, so there's
    // no field-presence-vs-tag ambiguity to get wrong here (see `PlayerConfig::split`'s doc).
    let split = prefab.components.split.clone()
        .or_else(|| prefab.components.camera.as_ref().and_then(|c| c.split.clone()));
    let party = prefab.components.party.clone()
        .or_else(|| prefab.components.camera.as_ref().and_then(|c| c.party.clone()));
    PlayerConfig {
        model_source,
        initial_position,
        camera: prefab.components.camera.clone().unwrap_or_else(default_camera_config),
        camera_mode: prefab.components.camera_mode.clone(),
        split,
        party,
        inputs,
        animation_policy: prefab.animation_policy.clone(),
        movement: prefab.components.movement.clone(),
        spawn_id: spawn_id.to_string(),
        prefab_key: prefab_key.to_string(),
        player_index: prefab.player_index,
        bound_gamepad: None,
        nameplate_display_name: if should_insert_nameplate(prefab.nameplate, player_nameplate_enabled) {
            Some(prefab.display_name.clone().unwrap_or_else(|| prefab_key.to_string()))
        } else {
            None
        },
        nameplate_override: prefab.nameplate,
        material: prefab.material.clone(),
        stat_templates: prefab.stat_templates.clone(),
        stat_label: prefab.stat_label.clone(),
        world_stat_bar: prefab.world_stat_bar.clone(),
    }
}
