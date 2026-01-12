use bevy::prelude::*;
use crate::capabilities::player::CharacterController;

#[derive(Component)]
pub struct OrbitCamera {
    pub target: Entity,
    pub radius: f32,
    pub offset: Vec3,
    pub zoom_speed: f32,
    pub orbit_speed: f32,
    pub min_radius: f32,
    pub max_radius: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub look_at_offset: Vec3,
}

pub fn camera_orbit_system(
    time: Res<Time>,
    mut mouse_motion_events: MessageReader<bevy::input::mouse::MouseMotion>,
    mut mouse_wheel_events: MessageReader<bevy::input::mouse::MouseWheel>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut camera_query: Query<(&mut Transform, &mut OrbitCamera), Without<CharacterController>>,
    mut character_query: Query<&mut Transform, (With<CharacterController>, Without<OrbitCamera>)>,
) {
    // Collect mouse motion
    let mut mouse_delta = Vec2::ZERO;
    for event in mouse_motion_events.read() {
        mouse_delta += event.delta;
    }

    let zoom_delta: f32 = mouse_wheel_events.read().map(|e| e.y).sum();

    for (mut cam_transform, mut orbit) in &mut camera_query {
        // Zoom
        if zoom_delta != 0.0 {
            orbit.radius -= zoom_delta * orbit.zoom_speed * time.delta_secs();
            orbit.radius = orbit.radius.clamp(orbit.min_radius, orbit.max_radius);
        }

        // Orbit Logic
        let lmb_pressed = mouse_button_input.pressed(MouseButton::Left);
        let rmb_pressed = mouse_button_input.pressed(MouseButton::Right);

        if lmb_pressed || rmb_pressed {
            // Yaw (Left/Right)
            orbit.yaw -= mouse_delta.x * orbit.orbit_speed * time.delta_secs();
            
            // Pitch (Up/Down)
            orbit.pitch -= mouse_delta.y * orbit.orbit_speed * time.delta_secs();
            // Clamp pitch to avoid flipping
            orbit.pitch = orbit.pitch.clamp(0.1, 1.5); 
        }
        
        // If RMB pressed, also rotate character if possible
        if rmb_pressed {
             if let Ok(mut char_transform) = character_query.get_mut(orbit.target) {
                 char_transform.rotate_y(-mouse_delta.x * orbit.orbit_speed * time.delta_secs());
             }
        }

        // Update Camera Position
        if let Ok(char_transform) = character_query.get(orbit.target) {
            let target_pos = char_transform.translation + orbit.look_at_offset;
            
            // Calculate offset based on yaw/pitch
            let rot = Quat::from_axis_angle(Vec3::Y, orbit.yaw) * Quat::from_axis_angle(Vec3::X, -orbit.pitch);
            let offset = rot * Vec3::Z * orbit.radius;
            
            cam_transform.translation = target_pos + offset;
            cam_transform.look_at(target_pos, Vec3::Y);
        }
    }
}
