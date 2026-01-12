use bevy::prelude::*;
use crate::schema::player::InputMap;
use crate::capabilities::animation::AnimationController;

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
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &mut CharacterController, &mut AnimationController)>,
) {
    for (mut transform, mut controller, mut anim_ctrl) in &mut query {
        let mut velocity = Vec3::ZERO;
        let mut rotation = 0.0;
        
        let forward = transform.forward();
        let right = transform.right();

        // Toggle run with shift
        if let Some(key) = controller.inputs.key("run") {
            if keyboard_input.just_pressed(key) {
                controller.is_running = !controller.is_running;
            }
        }

        if let Some(key) = controller.inputs.key("forward") {
            if keyboard_input.pressed(key) { velocity += *forward; }
        }
        if let Some(key) = controller.inputs.key("backward") {
            if keyboard_input.pressed(key) { velocity -= *forward; }
        }
        if let Some(key) = controller.inputs.key("strafe_right") {
            if keyboard_input.pressed(key) { velocity += *right; }
        }
        if let Some(key) = controller.inputs.key("strafe_left") {
            if keyboard_input.pressed(key) { velocity -= *right; }
        }
        
        // Turning
        if let Some(key) = controller.inputs.key("left") {
            if keyboard_input.pressed(key) { rotation += 1.0; }
        }
        if let Some(key) = controller.inputs.key("right") {
            if keyboard_input.pressed(key) { rotation -= 1.0; }
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
