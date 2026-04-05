use bevy::prelude::*;
use crate::schema::AppState;

/// A free-flying camera that responds to WASD, QE/Space, Shift, and mouse look.
/// No physics — the transform is updated directly each frame.
///
/// Spawned by `scene_loader` for any prefab whose `components.tags` contains `"flycam"`.
/// Initial pitch and yaw are extracted from the entity's spawn transform so the first
/// mouse movement never causes a jump.
///
/// Controls (fixed, not configurable via RON):
/// - **W/S** — forward / back
/// - **A/D** — strafe left / right
/// - **E / Space** — ascend
/// - **Q / LCtrl** — descend
/// - **LShift / RShift** — fast mode (`fast_speed`)
/// - **LMB / RMB + Mouse** — look (hold either mouse button to rotate)
#[derive(Component)]
pub struct FlyCamera {
    /// Normal movement speed in units/second.
    pub speed: f32,
    /// Movement speed when a Shift key is held, in units/second.
    pub fast_speed: f32,
    /// Mouse look sensitivity in radians per pixel.
    pub sensitivity: f32,
    /// Current pitch in radians, clamped to ±89°. Initialised from the spawn transform.
    pub pitch: f32,
    /// Current yaw in radians. Initialised from the spawn transform.
    pub yaw: f32,
}

impl Default for FlyCamera {
    fn default() -> Self {
        Self {
            speed: 8.0,
            fast_speed: 24.0,
            sensitivity: 0.002,
            pitch: 0.0,
            yaw: 0.0,
        }
    }
}

/// Marker component placed on the `Text` entity of any UI label with
/// `id: "flycam_position"` in the scene RON. `update_flycam_position_label`
/// writes the active FlyCamera's world position into it every frame.
#[derive(Component)]
pub struct FlyCamPositionLabel;

/// Updates every [`FlyCamPositionLabel`] text entity with the current world
/// position of the first active [`FlyCamera`]. No-ops when no FlyCamera exists
/// (e.g., regular player scenes).
pub fn update_flycam_position_label(
    flycam_query: Query<&Transform, With<FlyCamera>>,
    mut label_query: Query<&mut Text, With<FlyCamPositionLabel>>,
) {
    let Ok(transform) = flycam_query.single() else { return };
    let pos = transform.translation;
    for mut text in &mut label_query {
        *text = Text::new(format!(
            "X: {:.1}  Y: {:.1}  Z: {:.1}",
            pos.x, pos.y, pos.z,
        ));
    }
}

/// Moves and rotates every [`FlyCamera`] entity each frame.
///
/// Reads keyboard directly (WASD + QE/Space/Ctrl + Shift) and mouse motion events
/// for look. Only runs in [`AppState::InGame`]. Suppressed when the inspector overlay
/// is active.
pub fn fly_camera_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    state: Res<State<AppState>>,
    mut query: Query<(&mut Transform, &mut FlyCamera)>,
    time: Res<Time>,
    #[cfg(feature = "inspector")]
    inspector_enabled: Option<Res<crate::inspector::InspectorEnabled>>,
) {
    #[cfg(feature = "inspector")]
    if let Some(enabled) = inspector_enabled {
        if enabled.0 {
            return;
        }
    }

    if *state.get() != AppState::InGame {
        return;
    }

    // Only rotate the camera while LMB or RMB is held, so the mouse is free
    // for UI interaction when no button is pressed.
    let mouse_look_active = mouse_buttons.pressed(MouseButton::Left)
        || mouse_buttons.pressed(MouseButton::Right);

    let mut mouse_delta = Vec2::ZERO;
    for ev in mouse_motion.read() {
        if mouse_look_active {
            mouse_delta += ev.delta;
        }
    }

    for (mut transform, mut cam) in &mut query {
        // --- Look (applied every frame even without mouse motion) ---
        cam.yaw -= mouse_delta.x * cam.sensitivity;
        cam.pitch -= mouse_delta.y * cam.sensitivity;
        cam.pitch = cam.pitch.clamp(-1.5532, 1.5532); // ±89°

        transform.rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);

        // --- Move ---
        let is_fast = keyboard_input.pressed(KeyCode::ShiftLeft)
            || keyboard_input.pressed(KeyCode::ShiftRight);
        let speed = if is_fast { cam.fast_speed } else { cam.speed };
        let dt = time.delta_secs();

        let forward = *transform.forward();
        let right = *transform.right();

        let mut vel = Vec3::ZERO;
        if keyboard_input.pressed(KeyCode::KeyW) { vel += forward; }
        if keyboard_input.pressed(KeyCode::KeyS) { vel -= forward; }
        if keyboard_input.pressed(KeyCode::KeyD) { vel += right; }
        if keyboard_input.pressed(KeyCode::KeyA) { vel -= right; }
        if keyboard_input.pressed(KeyCode::Space) || keyboard_input.pressed(KeyCode::KeyE) {
            vel += Vec3::Y;
        }
        if keyboard_input.pressed(KeyCode::ControlLeft) || keyboard_input.pressed(KeyCode::KeyQ) {
            vel -= Vec3::Y;
        }

        if vel.length_squared() > 0.0001 {
            transform.translation += vel.normalize() * speed * dt;
        }
    }
}
