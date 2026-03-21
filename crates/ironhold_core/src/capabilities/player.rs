use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::schema::player::InputMap;
use crate::runtime::messages::*;
use std::collections::HashMap;

use crate::capabilities::animation_resolver::{LocomotionState, AnimationRequests};

#[derive(Component)]
pub struct CharacterController {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub rot_speed: f32,
    pub inputs: InputMap,
    pub is_running: bool,
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
            let max_toi = 1.5;
            
            loco.is_grounded = context.cast_ray(
                ray_pos, 
                ray_dir, 
                max_toi, 
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

        // Simple Jump logic
        if jumping && loco.is_grounded {
            info!("Jump triggered! Set velocity and push jump_enter");
            velocity.linvel.y = 5.0; 
            requests.queue.push_back("jump_enter".to_string());
        }
    }
}
