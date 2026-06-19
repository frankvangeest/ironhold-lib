use bevy::prelude::*;
use crate::capabilities::player::CharacterController;
use crate::schema::player::InputMap;

/// Inserted by `Action::CameraShake` on the active orbit camera entity.
/// Removed automatically when `remaining` reaches zero.
#[derive(Component)]
pub struct CameraShakeState {
    /// Seconds of shake remaining.
    pub remaining: f32,
    /// Initial duration — used to compute the sqrt decay envelope.
    pub duration: f32,
    /// Peak displacement in world-space metres.
    pub intensity: f32,
}

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
    /// Minimum pitch in radians. Sourced from `CameraConfig.min_pitch`.
    pub min_pitch: f32,
    /// Maximum pitch in radians. Sourced from `CameraConfig.max_pitch`.
    pub max_pitch: f32,
    /// Which mouse buttons activate orbit. Parsed from `CameraConfig.orbit_button`.
    /// `None` = no mouse orbit; `Some(button)` = that specific button; see `orbit_rmb`.
    pub orbit_lmb: bool,
    pub orbit_rmb: bool,
    /// Whether RMB also rotates the character. Sourced from `CameraConfig.character_rotate_button`.
    pub character_rotate_rmb: bool,
    pub character_rotate_lmb: bool,
}

pub fn camera_orbit_system(
    time: Res<Time>,
    mut mouse_motion_events: MessageReader<bevy::input::mouse::MouseMotion>,
    mut mouse_wheel_events: MessageReader<bevy::input::mouse::MouseWheel>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut camera_query: Query<(&mut Transform, &mut OrbitCamera), Without<CharacterController>>,
    mut character_query: Query<&mut Transform, (With<CharacterController>, Without<OrbitCamera>)>,
    #[cfg(feature = "inspector")]
    inspector_enabled: Option<Res<crate::inspector::InspectorEnabled>>,
) {
    #[cfg(feature = "inspector")]
    if let Some(enabled) = inspector_enabled {
        if enabled.0 {
            return;
        }
    }

    let mut mouse_delta = Vec2::ZERO;
    for event in mouse_motion_events.read() {
        mouse_delta += event.delta;
    }

    let zoom_delta: f32 = mouse_wheel_events.read().map(|e| e.y).sum();

    for (mut cam_transform, mut orbit) in &mut camera_query {
        if zoom_delta != 0.0 {
            orbit.radius -= zoom_delta * orbit.zoom_speed * time.delta_secs();
            orbit.radius = orbit.radius.clamp(orbit.min_radius, orbit.max_radius);
        }

        let lmb = mouse_button_input.pressed(MouseButton::Left);
        let rmb = mouse_button_input.pressed(MouseButton::Right);
        let orbit_active = (orbit.orbit_lmb && lmb) || (orbit.orbit_rmb && rmb);

        if orbit_active {
            orbit.yaw -= mouse_delta.x * orbit.orbit_speed * time.delta_secs();
            orbit.pitch -= mouse_delta.y * orbit.orbit_speed * time.delta_secs();
            orbit.pitch = orbit.pitch.clamp(orbit.min_pitch, orbit.max_pitch);
        }

        let char_rotate = (orbit.character_rotate_rmb && rmb)
            || (orbit.character_rotate_lmb && lmb);
        if char_rotate {
            if let Ok(mut char_transform) = character_query.get_mut(orbit.target) {
                char_transform.rotate_y(-mouse_delta.x * orbit.orbit_speed * time.delta_secs());
            }
        }

        if let Ok(char_transform) = character_query.get(orbit.target) {
            let target_pos = char_transform.translation + orbit.look_at_offset;
            let rot = Quat::from_axis_angle(Vec3::Y, orbit.yaw)
                * Quat::from_axis_angle(Vec3::X, -orbit.pitch);
            let offset = rot * Vec3::Z * orbit.radius;
            cam_transform.translation = target_pos + offset;
            cam_transform.look_at(target_pos, Vec3::Y);
        }
    }
}

/// Parse a `CameraConfig.orbit_button` / `character_rotate_button` string into
/// `(activate_on_lmb, activate_on_rmb)`.
pub fn parse_orbit_button(s: &str) -> (bool, bool) {
    match s {
        "Left"   => (true,  false),
        "Right"  => (false, true),
        "Either" => (true,  true),
        _        => {
            warn!("Unknown orbit button value {:?} — defaulting to Either", s);
            (true, true)
        }
    }
}

/// Parse `InputMap.strafe_mouse_button` to a `MouseButton`.
pub fn parse_strafe_button(s: &str) -> Option<MouseButton> {
    InputMap::parse_mouse_button(s)
}

/// Applies and decays a procedural camera shake after `camera_orbit_system` has set the
/// orbital position. The shake is a deterministic sine-wave offset (no RNG — WASM safe).
/// Removes `CameraShakeState` when the remaining time reaches zero.
pub fn camera_shake_system(
    time: Res<Time>,
    mut commands: Commands,
    mut camera_query: Query<(Entity, &mut Transform, &mut CameraShakeState), With<OrbitCamera>>,
) {
    for (entity, mut cam_transform, mut shake) in &mut camera_query {
        shake.remaining -= time.delta_secs();
        if shake.remaining <= 0.0 {
            commands.entity(entity).remove::<CameraShakeState>();
            continue;
        }
        let t = time.elapsed_secs();
        // sqrt decay: fast initial burst that tapers — feels snappier than linear.
        let decay = (shake.remaining / shake.duration).sqrt();
        let x = (t * 37.0).sin() * shake.intensity * decay;
        let y = (t * 53.0).sin() * shake.intensity * decay * 0.5;
        cam_transform.translation += Vec3::new(x, y, 0.0);
    }
}
