use bevy::prelude::*;
use crate::schema::player::InputMap;
use crate::capabilities::animation::AnimationController;
use crate::runtime::messages::*;
use std::collections::HashMap;

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
    mut query: Query<(Entity, &mut Transform, &mut CharacterController, &mut AnimationController)>,
) {
    let mut actions = HashMap::new();
    for event in input_events.read() {
        actions.entry(event.entity).or_insert_with(Vec::new).push(event.action.clone());
    }

    for (entity, mut transform, mut controller, mut anim_ctrl) in &mut query {
        let mut velocity = Vec3::ZERO;
        let mut rotation = 0.0;
        
        if let Some(entity_actions) = actions.get(&entity) {
            for action in entity_actions {
                match action {
                    InputAction::Move(dir) => {
                        let forward = transform.forward();
                        let right = transform.right();
                        velocity += *forward * dir.y;
                        velocity += *right * dir.x;
                    }
                    InputAction::Turn(val) => {
                        rotation += val;
                    }
                    InputAction::Run(true) => {
                        controller.is_running = !controller.is_running;
                    }
                    _ => {}
                }
            }
        }

        // Apply Rotation
        if rotation != 0.0 {
            transform.rotate_y(rotation * controller.rot_speed * time.delta_secs());
        }

        // Apply Movement and set animation
        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize();
            let speed = if controller.is_running { controller.run_speed } else { controller.walk_speed };
            transform.translation += velocity * speed * time.delta_secs();
            
            // Set animation based on running state
            let target_anim = if controller.is_running {
                anim_ctrl.animations.run.clone()
            } else {
                anim_ctrl.animations.walk.clone()
            };
            if anim_ctrl.current != target_anim {
                anim_ctrl.current = target_anim;
            }
        } else {
            // Idle animation
            if anim_ctrl.current != anim_ctrl.animations.idle {
                anim_ctrl.current = anim_ctrl.animations.idle.clone();
            }
        }
    }
}
