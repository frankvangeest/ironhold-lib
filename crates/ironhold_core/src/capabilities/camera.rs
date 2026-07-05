use bevy::prelude::*;
use crate::capabilities::player::CharacterController;
use crate::schema::player::InputMap;
use crate::runtime::scene_manager::LevelEntity;

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

/// Local co-op shared camera: frames the midpoint of `targets` and derives its distance from
/// how far apart they are, instead of orbiting one fixed target like `OrbitCamera`. Spawned by
/// `spawn_party_orbit_camera` when 2+ players exist in a scene and the first player's
/// `CameraConfig.party` block is set — see `entity_spawner::spawn_players_and_camera`.
///
/// Known limitation: `Action::CameraShake` only queries `With<OrbitCamera>`
/// (`scene_manager/mod.rs`'s `SceneStateParams::orbit_cameras`), so it silently no-ops on a
/// scene using `PartyOrbitCamera` instead. Not needed for Stage 1's acceptance criteria.
#[derive(Component)]
pub struct PartyOrbitCamera {
    pub targets: Vec<Entity>,
    /// Extra distance added beyond the raw max pairwise distance between targets.
    pub zoom_margin: f32,
    /// Whether manual scroll-zoom still nudges the derived radius. See `PartyZoomDef`.
    pub allow_manual_zoom: bool,
    /// Accumulated manual scroll input (only used when `allow_manual_zoom` is true).
    pub manual_zoom_offset: f32,
    pub zoom_speed: f32,
    pub orbit_speed: f32,
    pub min_radius: f32,
    pub max_radius: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub look_at_offset: Vec3,
    pub min_pitch: f32,
    pub max_pitch: f32,
    pub orbit_lmb: bool,
    pub orbit_rmb: bool,
}

/// Spawns a single shared camera framing every entity in `targets`, reusing `base_camera`
/// (the first player's `CameraConfig`) for tuning fields and `party` for the co-op-specific
/// zoom behavior. Called once per scene load from `spawn_players_and_camera` — never per
/// player, since all players share this one camera.
pub fn spawn_party_orbit_camera(
    commands: &mut Commands,
    tonemapping: bevy::core_pipeline::tonemapping::Tonemapping,
    base_camera: &crate::schema::player::CameraConfig,
    party: &crate::schema::player::PartyZoomDef,
    targets: &[Entity],
) {
    let (orbit_lmb, orbit_rmb) = parse_orbit_button(&base_camera.orbit_button);
    commands.spawn((
        Name::new("Party Orbit Camera"),
        Camera3d::default(),
        tonemapping,
        // Real position/look-at is set on the first `party_camera_follow_system` tick, once
        // target transforms are available — this initial transform is just a safe placeholder.
        Transform::from_translation(Vec3::from(base_camera.offset))
            .looking_at(Vec3::ZERO, Vec3::Y),
        LevelEntity,
        PartyOrbitCamera {
            targets: targets.to_vec(),
            zoom_margin: party.zoom_margin,
            allow_manual_zoom: party.allow_manual_zoom,
            manual_zoom_offset: 0.0,
            zoom_speed: base_camera.zoom_speed,
            orbit_speed: base_camera.orbit_speed,
            min_radius: base_camera.min_radius,
            max_radius: base_camera.max_radius,
            pitch: base_camera.initial_pitch,
            yaw: base_camera.initial_yaw,
            look_at_offset: Vec3::from(base_camera.look_at_offset),
            min_pitch: base_camera.min_pitch,
            max_pitch: base_camera.max_pitch,
            orbit_lmb,
            orbit_rmb,
        },
    ));
}

/// Frames the midpoint of a `PartyOrbitCamera`'s `targets` each frame and derives the orbit
/// radius from their maximum pairwise separation, clamped to `[min_radius, max_radius]`.
/// Mirrors `camera_orbit_system`'s mouse-orbit handling but has no single character to rotate.
pub fn party_camera_follow_system(
    time: Res<Time>,
    mut mouse_motion_events: MessageReader<bevy::input::mouse::MouseMotion>,
    mut mouse_wheel_events: MessageReader<bevy::input::mouse::MouseWheel>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut camera_query: Query<(&mut Transform, &mut PartyOrbitCamera)>,
    target_query: Query<&Transform, (With<CharacterController>, Without<PartyOrbitCamera>)>,
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

    for (mut cam_transform, mut party) in &mut camera_query {
        let positions: Vec<Vec3> = party.targets.iter()
            .filter_map(|e| target_query.get(*e).ok())
            .map(|t| t.translation)
            .collect();
        // No resolvable targets this frame (e.g. mid scene-transition) — hold the last position
        // rather than snapping the camera to the origin.
        if positions.is_empty() {
            continue;
        }

        let midpoint = positions.iter().copied().sum::<Vec3>() / positions.len() as f32;
        let mut max_dist = 0.0_f32;
        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                max_dist = max_dist.max(positions[i].distance(positions[j]));
            }
        }

        if party.allow_manual_zoom && zoom_delta != 0.0 {
            party.manual_zoom_offset -= zoom_delta * party.zoom_speed * time.delta_secs();
        }
        let radius = (max_dist + party.zoom_margin + party.manual_zoom_offset)
            .clamp(party.min_radius, party.max_radius);

        let lmb = mouse_button_input.pressed(MouseButton::Left);
        let rmb = mouse_button_input.pressed(MouseButton::Right);
        let orbit_active = (party.orbit_lmb && lmb) || (party.orbit_rmb && rmb);
        if orbit_active {
            party.yaw -= mouse_delta.x * party.orbit_speed * time.delta_secs();
            party.pitch -= mouse_delta.y * party.orbit_speed * time.delta_secs();
            party.pitch = party.pitch.clamp(party.min_pitch, party.max_pitch);
        }

        let target_pos = midpoint + party.look_at_offset;
        let rot = Quat::from_axis_angle(Vec3::Y, party.yaw)
            * Quat::from_axis_angle(Vec3::X, -party.pitch);
        let offset = rot * Vec3::Z * radius;
        cam_transform.translation = target_pos + offset;
        cam_transform.look_at(target_pos, Vec3::Y);
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
