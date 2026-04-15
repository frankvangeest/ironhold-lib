use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::schema::player::InputMap;
use crate::schema::Action;
use crate::runtime::messages::*;
use crate::runtime::actions::ActionQueue;
use std::collections::HashMap;

use crate::capabilities::animation_resolver::{LocomotionState, AnimationRequests};

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
    /// Distance from the player's center to the bottom of the capsule plus a small
    /// tolerance (~0.2 m). Used as the ground-detection ray length so that
    /// `is_grounded` goes false within 2–3 frames of a jump instead of persisting
    /// for ~8 frames with a hardcoded 1.5 m value.
    pub ground_cast_length: f32,
    /// Asset-catalog audio key to play when the player jumps (e.g. `"jump"`).
    /// `None` = silent. Resolved through `Action::PlaySound` → catalog lookup.
    pub jump_sound: Option<String>,
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
    )>,
    rapier_context: Option<ReadRapierContext>,
    mut action_queue: ResMut<ActionQueue>,
) {
    let mut actions = HashMap::new();
    for event in input_events.read() {
        actions.entry(event.entity).or_insert_with(Vec::new).push(event.action.clone());
    }

    // Try to get the rapier context, but don't panic if it's missing (e.g. in headless tests)
    let rapier_context = rapier_context.as_ref().and_then(|rc| rc.single().ok());

    for (entity, mut transform, global_transform, mut controller, mut loco, mut velocity, mut requests) in &mut query {
        let mut move_vec = Vec3::ZERO;
        let mut rotation = 0.0;
        let mut jumping = false;

        // Perform raycast for ground detection every frame for animation state
        let was_grounded = loco.is_grounded;
        
        if let Some(ref context) = rapier_context {
            let ray_pos = global_transform.translation();
            let ray_dir = -Vec3::Y;
            loco.is_grounded = context.cast_ray(
                ray_pos,
                ray_dir,
                controller.ground_cast_length,
                true,
                QueryFilter::new().exclude_rigid_body(entity)
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
            };
            
            velocity.linvel.x = move_vec.x * speed;
            velocity.linvel.z = move_vec.z * speed;

            loco.moving = true;
            loco.running = controller.is_running;
        } else {
            // Apply drag/friction to stop sliding
            velocity.linvel.x *= 0.8;
            velocity.linvel.z *= 0.8;
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
            info!("Jump triggered (jumps_used={})! velocity_y={:.2}", controller.jumps_used, vel);
            velocity.linvel.y = vel;
            controller.jumps_used += 1;
            requests.queue.push_back("jump_enter".to_string());
            if let Some(key) = &controller.jump_sound {
                action_queue.push(Action::PlaySound(key.clone()));
            }
        }
    }
}
