use bevy::prelude::*;
use crate::schema::AppState;
use crate::capabilities::camera::{ActiveCameraMode, FlycamCameraMode};

/// Resolve a `FlyCamDef` look_button string to `(lmb, rmb)` flags.
pub fn parse_flycam_look_button(s: &str) -> (bool, bool) {
    match s {
        "Left"   => (true,  false),
        "Right"  => (false, true),
        "Either" => (true,  true),
        _ => {
            warn!("Unknown flycam look_button {:?} — defaulting to Either", s);
            (true, true)
        }
    }
}

/// Marker component placed on the `Text` entity of any UI label with
/// `id: "flycam_position"` in the scene RON. `update_flycam_position_label`
/// writes the active flycam's world position into it every frame.
#[derive(Component)]
pub struct FlyCamPositionLabel;

/// Updates every [`FlyCamPositionLabel`] text entity with the current world
/// position of the first active `Flycam`-mode camera. No-ops when none exists
/// (e.g., regular player scenes).
pub fn update_flycam_position_label(
    flycam_query: Query<&Transform, With<FlycamCameraMode>>,
    mut label_query: Query<&mut Text, With<FlyCamPositionLabel>>,
) {
    let Ok(transform) = flycam_query.single() else { return };
    let pos = transform.translation;
    let new_str = format!("X: {:.1}  Y: {:.1}  Z: {:.1}", pos.x, pos.y, pos.z);
    for mut text in &mut label_query {
        if text.0 != new_str {
            *text = Text::new(new_str.clone());
        }
    }
}

/// Moves and rotates every `Flycam`-mode camera each frame. Free-flying, no physics — the
/// transform is updated directly. Spawned for any prefab whose `components.tags` contains
/// `"flycam"`; initial pitch/yaw are extracted from the entity's spawn transform so the first
/// mouse movement never causes a jump.
///
/// Default controls (all configurable via `FlyCamDef` in the prefab RON):
/// - **W/S** — forward / back
/// - **A/D** — strafe left / right
/// - **Space** — ascend   (also `E` on legacy QWERTY layout)
/// - **Q** — descend      (also `LCtrl` on legacy layout)
/// - **LShift / RShift** — fast mode (`fast_speed`)
/// - **LMB / RMB + Mouse** — look (either or both buttons, configurable)
///
/// Reads keyboard/mouse using the bindings stored on `ActiveCameraMode::Flycam`'s `FlycamState`
/// payload. Only runs in [`AppState::InGame`]. Suppressed when the inspector overlay is active.
pub fn fly_camera_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    state: Res<State<AppState>>,
    mut query: Query<(&mut Transform, &mut ActiveCameraMode), With<FlycamCameraMode>>,
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

    let lmb = mouse_buttons.pressed(MouseButton::Left);
    let rmb = mouse_buttons.pressed(MouseButton::Right);

    let mut mouse_delta = Vec2::ZERO;
    for ev in mouse_motion.read() {
        mouse_delta += ev.delta;
    }

    for (mut transform, mut mode) in &mut query {
        let ActiveCameraMode::Flycam(cam) = &mut *mode else { continue };
        let mouse_look_active = (cam.look_lmb && lmb) || (cam.look_rmb && rmb);
        if !mouse_look_active {
            mouse_delta = Vec2::ZERO;
        }

        cam.yaw -= mouse_delta.x * cam.sensitivity;
        cam.pitch -= mouse_delta.y * cam.sensitivity;
        cam.pitch = cam.pitch.clamp(-1.5532, 1.5532); // ±89°

        transform.rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);

        let is_fast = keyboard_input.pressed(KeyCode::ShiftLeft)
            || keyboard_input.pressed(KeyCode::ShiftRight);
        let speed = if is_fast { cam.fast_speed } else { cam.speed };
        let dt = time.delta_secs();

        let forward = *transform.forward();
        let right = *transform.right();

        let mut vel = Vec3::ZERO;
        if keyboard_input.pressed(cam.key_forward)  { vel += forward; }
        if keyboard_input.pressed(cam.key_backward) { vel -= forward; }
        if keyboard_input.pressed(cam.key_right)    { vel += right; }
        if keyboard_input.pressed(cam.key_left)     { vel -= right; }
        if keyboard_input.pressed(cam.key_up)       { vel += Vec3::Y; }
        if keyboard_input.pressed(cam.key_down)     { vel -= Vec3::Y; }

        if vel.length_squared() > 0.0001 {
            transform.translation += vel.normalize() * speed * dt;
        }
    }
}
