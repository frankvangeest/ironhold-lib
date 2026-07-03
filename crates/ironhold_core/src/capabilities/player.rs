use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::schema::player::InputMap;
use crate::schema::stats::{LoadedStats, LoadedModifiers}; // used by update_player_speed_system
use crate::runtime::messages::*;
use std::collections::HashMap;

use crate::capabilities::animation_resolver::{LocomotionState, AnimationRequests};

/// Multiplier applied to walk/run speed from the `player_speed` stat.
/// Updated in `Update` by `update_player_speed_system`; consumed in `FixedUpdate`
/// by `player_movement_system` — keeps stat reads out of the physics hot path.
#[derive(Component)]
pub struct SpeedMultiplier(pub f32);

/// Marker for a player-controlled entity, as opposed to NPCs, props, or other prefabs.
/// Inserted unconditionally wherever a player entity is spawned — GLB (`spawn_player_entity`)
/// or primitive (inline in `scene_loader.rs`), scene-placed or dynamic character-select.
/// Distinct from `CharacterController`: a future networked remote player may carry `Player`
/// without local input handling.
#[derive(Component)]
pub struct Player;

/// Whether a `Player` entity is controlled by this client (`Local`) or mirrored from another
/// client (`Remote`). Always `Local` today — there is no multiplayer code yet. Reserved as a
/// forward-compat hook for Beta 0.6 (LAN co-op) so nameplate/UI/camera systems can distinguish
/// "me" from "other players" without another schema pass once real players exist.
#[derive(Component, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayerOwnership {
    #[default]
    Local,
    Remote,
}

#[derive(Component)]
pub struct CharacterController {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub rot_speed: f32,
    pub inputs: InputMap,
    pub is_running: bool,
    /// Pre-computed initial Y velocity for a normal jump.
    pub jump_velocity: f32,
    /// Whether a second jump in mid-air is permitted.
    pub double_jump_enabled: bool,
    /// Pre-computed initial Y velocity for the second jump.
    pub double_jump_velocity: f32,
    /// Number of jumps already used in the current airborne period (reset on landing).
    pub jumps_used: u8,
    /// Maximum jumps per airborne period: 1 = normal, 2 = double jump.
    pub max_jumps: u8,
    /// Radius of the ground-detection sphere cast (= capsule radius).
    pub collider_radius: f32,
    /// How far below the feet the ground-detection sphere is swept each frame.
    pub ground_cast_length: f32,
    /// Velocity decay multiplier each physics tick when there is no input. Default: 0.8.
    pub idle_drag: f32,
}

/// Reads the `player_speed` stat once per rendered frame (Update) and writes the resulting
/// multiplier onto `SpeedMultiplier` — keeping all stat access out of FixedUpdate.
pub fn update_player_speed_system(
    loaded_stats: Option<Res<LoadedStats>>,
    loaded_modifiers: Option<Res<LoadedModifiers>>,
    mut query: Query<&mut SpeedMultiplier>,
) {
    let multiplier = loaded_stats.as_ref()
        .and_then(|ls| ls.0.get("player_speed"))
        .map(|s| {
            let effective = loaded_modifiers.as_ref()
                .map(|m| s.compute_effective(&m.0))
                .unwrap_or(s.current);
            effective / s.def.base.max(0.001)
        })
        .unwrap_or(1.0);

    for mut sm in &mut query {
        if (sm.0 - multiplier).abs() > 0.001 {
            sm.0 = multiplier;
        }
    }
}

pub fn player_movement_system(
    time: Res<Time>,
    mut input_events: MessageReader<InputActionMessage>,
    mut query: Query<(
        Entity,
        &mut Transform,
        &GlobalTransform,
        &mut CharacterController,
        &mut LocomotionState,
        &mut Velocity,
        &mut AnimationRequests,
        &SpeedMultiplier,
    )>,
    rapier_context: Option<ReadRapierContext>,
    mut game_events: MessageWriter<GameEvent>,
) {
    let mut actions = HashMap::new();
    for event in input_events.read() {
        actions.entry(event.entity).or_insert_with(Vec::new).push(event.action.clone());
    }

    // Try to get the rapier context, but don't panic if it's missing (e.g. in headless tests)
    let rapier_context = rapier_context.as_ref().and_then(|rc| rc.single().ok());

    for (entity, mut transform, global_transform, mut controller, mut loco, mut velocity, mut requests, speed_mul) in &mut query {
        let mut move_vec = Vec3::ZERO;
        let mut rotation = 0.0;
        let mut jumping = false;

        // Perform raycast for ground detection every frame for animation state
        let was_grounded = loco.is_grounded;
        
        if let Some(ref context) = rapier_context {
            // Sphere cast from the entity origin (feet) downward.
            // Using a ball equal to the capsule radius rather than a point ray means
            // the detection covers the full bottom-hemisphere footprint, so sloped and
            // rough terrain is handled correctly — matching how Unity's CharacterController
            // and Godot's CharacterBody3D sweep the capsule shape for floor detection.
            let feet_pos = global_transform.translation();
            // Build a sphere matching the capsule's bottom hemisphere and sweep it
            // downward. Passing the raw parry shape is required; bevy_rapier3d's
            // Collider wrapper does not implement the parry Shape trait directly.
            let ground_ball = Collider::ball(controller.collider_radius);
            loco.is_grounded = context.cast_shape(
                feet_pos,
                Quat::IDENTITY,
                Vec3::NEG_Y,
                ground_ball.raw.as_ref(),
                ShapeCastOptions {
                    max_time_of_impact: controller.ground_cast_length,
                    ..default()
                },
                QueryFilter::new().exclude_rigid_body(entity),
            ).is_some();
        } else {
            // Default to grounded if no physics (for basic testing)
            loco.is_grounded = true;
        }

        // Detect landing
        if !was_grounded && loco.is_grounded {
            requests.queue.push_back("jump_exit".to_string());
            controller.jumps_used = 0;
        }

        if let Some(entity_actions) = actions.get(&entity) {
            for action in entity_actions {
                match action {
                    InputAction::Move(dir) => {
                        let forward = transform.forward();
                        let right = transform.right();
                        move_vec += *forward * dir.y;
                        move_vec += *right * dir.x;
                    }
                    InputAction::Turn(val) => {
                        rotation += val;
                    }
                    InputAction::Run(_) => {
                        controller.is_running = !controller.is_running;
                    }
                    InputAction::Jump(true) => {
                        jumping = true;
                    }
                    _ => {}
                }
            }
        }

        // Apply Rotation
        if rotation != 0.0 {
            transform.rotate_y(rotation * controller.rot_speed * time.delta_secs());
        }

        // Apply Movement via Linear Velocity (XZ only to allow gravity to work on Y)
        if move_vec.length_squared() > 0.1 {
            move_vec = move_vec.normalize();
            let speed = if controller.is_running {
                controller.run_speed
            } else {
                controller.walk_speed
            } * speed_mul.0;

            velocity.linvel.x = move_vec.x * speed;
            velocity.linvel.z = move_vec.z * speed;

            loco.moving = true;
            loco.running = controller.is_running;
        } else {
            velocity.linvel.x *= controller.idle_drag;
            velocity.linvel.z *= controller.idle_drag;
            loco.moving = false;
            loco.running = false;
        }

        // Jump logic — grounded first jump, or double jump in air.
        // `jumps_used == 0` ensures we can't re-trigger while still in the
        // grounded-ray window immediately after jumping.
        let can_jump = if loco.is_grounded {
            controller.jumps_used == 0
        } else {
            controller.double_jump_enabled && controller.jumps_used < controller.max_jumps
        };
        if jumping && can_jump {
            let vel = if controller.jumps_used > 0 {
                controller.double_jump_velocity
            } else {
                controller.jump_velocity
            };
            debug!("Jump triggered (jumps_used={}) velocity_y={:.2}", controller.jumps_used, vel);
            velocity.linvel.y = vel;
            controller.jumps_used += 1;
            requests.queue.push_back("jump_enter".to_string());
            game_events.write(GameEvent::Trigger("player.jumped".to_string()));
        }
    }
}
