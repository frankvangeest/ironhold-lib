use bevy::prelude::*;
use bevy::camera::Viewport;
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
) -> Entity {
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
    )).id()
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

/// Hard ceiling on `SplitOrientation::Grid`'s player count — bounds render-pass count (4 cameras
/// is the worst case this engine has validated) and avoids degenerate slivers on a misconfigured
/// scene. Extra players beyond this cap spawn cameraless, matching the existing (pre-Stage-6)
/// behavior when a 3rd player exists in a `Vertical`/`Horizontal` (always-2-way) scene.
pub const MAX_SPLIT_PLAYERS: u32 = 4;

/// Marks a local co-op split-screen camera and which share of the window it owns. Spawned
/// alongside a normal `OrbitCamera` (not a replacement) by `spawn_players_and_camera` when
/// `CameraConfig.split` is set — each split-screen camera independently tracks its own player,
/// exactly like a single-player `OrbitCamera` would, and only needs its `Camera.viewport`
/// constrained to its share of the window. Orientation is deliberately NOT stored here — see
/// `ActiveSplitScreen`.
#[derive(Component)]
pub struct SplitViewportSlot(pub u32);

/// Recomputes every `SplitViewportSlot` camera's `Camera.viewport` from the current primary
/// window size and `ActiveSplitScreen`'s orientation. Runs every frame (cheap: at most
/// `MAX_SPLIT_PLAYERS` cameras, simple arithmetic) rather than hooking window-resize events
/// specifically, so a resize is always correct on the very next frame with no missed-event risk.
///
/// `Viewport.physical_position`/`physical_size` are physical pixels, not logical — reads
/// `Window::physical_size()` (not `width()`/`height()`, which are logical) so this is correct on
/// any HiDPI display without doing the `scale_factor()` multiplication by hand.
///
/// `Grid` reads `ActiveSplitSlotCount` for its player count rather than counting the
/// `SplitViewportSlot` query live — see that resource's doc comment for why (Stage 6 plan: the
/// live-count approach would silently reflow the grid on any mid-transition entity churn). A
/// `Grid` scene with `count == 3` leaves one grid cell empty (no special-cased 3-way layout); a
/// camera whose `slot.0 >= cols*rows` (more players than slots accounted for) is left unpositioned
/// this frame rather than assigned a bogus cell.
pub fn split_screen_viewport_system(
    active_split: Res<crate::runtime::scene_manager::ActiveSplitScreen>,
    slot_count: Res<crate::runtime::scene_manager::ActiveSplitSlotCount>,
    window_q: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<(&mut Camera, &SplitViewportSlot)>,
) {
    let Some(orientation) = active_split.0 else { return };
    let Ok(window) = window_q.single() else { return };

    let physical_size = window.physical_size();
    let physical_width = physical_size.x;
    let physical_height = physical_size.y;

    for (mut camera, slot) in &mut cameras {
        let (position, size) = match orientation {
            crate::schema::player::SplitOrientation::Vertical => {
                let half_width = physical_width / 2;
                if slot.0 == 0 {
                    (UVec2::new(0, 0), UVec2::new(half_width, physical_height))
                } else {
                    (
                        UVec2::new(half_width, 0),
                        // Remainder (not another half_width) absorbs odd-pixel-width rounding
                        // so the two halves always sum to the full window width exactly.
                        UVec2::new(physical_width - half_width, physical_height),
                    )
                }
            }
            crate::schema::player::SplitOrientation::Horizontal => {
                let half_height = physical_height / 2;
                if slot.0 == 0 {
                    // Screen-space Y grows downward, so slot 0 (top half) starts at y=0.
                    (UVec2::new(0, 0), UVec2::new(physical_width, half_height))
                } else {
                    (
                        UVec2::new(0, half_height),
                        // Remainder (not another half_height) absorbs odd-pixel-height rounding
                        // so the two halves always sum to the full window height exactly.
                        UVec2::new(physical_width, physical_height - half_height),
                    )
                }
            }
            crate::schema::player::SplitOrientation::Grid => {
                let Some(count) = slot_count.0 else { continue };
                if count == 0 { continue; }
                let cols = (count as f32).sqrt().ceil() as u32;
                let rows = count.div_ceil(cols);
                if slot.0 >= cols * rows { continue; }
                let row = slot.0 / cols;
                let col = slot.0 % cols;
                let cell_width = physical_width / cols;
                let cell_height = physical_height / rows;
                let x = col * cell_width;
                let y = row * cell_height;
                // Last column/row absorbs the remainder — same odd-pixel-dimension handling as
                // `Vertical`/`Horizontal` above, generalized to N cells per axis.
                let w = if col == cols - 1 { physical_width - x } else { cell_width };
                let h = if row == rows - 1 { physical_height - y } else { cell_height };
                (UVec2::new(x, y), UVec2::new(w, h))
            }
        };
        camera.viewport = Some(Viewport {
            physical_position: position,
            physical_size: size.max(UVec2::new(1, 1)),
            ..default()
        });
    }
}

/// Local co-op dynamic split (Stage 5): decides every frame whether the scene should be merged
/// (one shared `PartyOrbitCamera`) or split (two per-player `OrbitCamera`s), and flips
/// `Camera.is_active` accordingly. Runs after `party_camera_follow_system` (so the distance read
/// below uses that frame's fresh transforms) and before `split_screen_viewport_system` (so an
/// `is_active` flip takes effect the same frame, with no one-frame-stale viewport) — see the
/// `.chain()` order in `lib.rs`.
///
/// No-ops when `DynamicSplitConfig` is `None` (fixed-orientation or no split at all). Never
/// spawns/despawns cameras — all three (party + 2 split) already exist for the scene's lifetime
/// (see `spawn_players_and_camera`'s dynamic branch); only `is_active` toggles. This works with
/// zero new camera-following logic because neither `camera_orbit_system` nor
/// `party_camera_follow_system` gate on `is_active` — an inactive camera's `Transform` stays
/// correctly updated the whole time, so there's no pop/snap on reactivation.
///
/// Applies hysteresis against the *current* `ActiveSplitScreen` value (merged → split only past
/// `split_distance`; split → merged only below `merge_distance`) to avoid flickering right at a
/// single boundary. The split orientation is decided (`abs(dx)` vs `abs(dz)` between the two
/// players — a world-space approximation of "which way they're spread apart on screen", not an
/// exact projection) only at the merged→split transition instant and then held fixed for the
/// rest of that split period, so it can't visibly flip if the players' relative dx/dz ordering
/// changes sign while they remain apart.
pub fn dynamic_split_screen_system(
    dynamic_config: Res<crate::runtime::scene_manager::DynamicSplitConfig>,
    mut active_split: ResMut<crate::runtime::scene_manager::ActiveSplitScreen>,
    mut split_cameras: Query<(&mut Camera, &OrbitCamera), (With<SplitViewportSlot>, Without<PartyOrbitCamera>)>,
    mut party_camera: Query<&mut Camera, (With<PartyOrbitCamera>, Without<SplitViewportSlot>)>,
    transforms: Query<&Transform>,
) {
    let Some(dynamic) = dynamic_config.0.as_ref() else { return };

    let mut targets = split_cameras.iter().map(|(_, orbit)| orbit.target);
    let Some(t0) = targets.next() else { return };
    let Some(t1) = targets.next() else { return };
    let Ok(p0) = transforms.get(t0) else { return };
    let Ok(p1) = transforms.get(t1) else { return };
    let p0 = p0.translation;
    let p1 = p1.translation;
    let distance = p0.distance(p1);

    let currently_split = active_split.0.is_some();
    let should_split = if currently_split {
        distance >= dynamic.merge_distance
    } else {
        distance > dynamic.split_distance
    };
    if should_split == currently_split {
        return;
    }

    active_split.0 = if should_split {
        let dx = p1.x - p0.x;
        let dz = p1.z - p0.z;
        Some(if dx.abs() >= dz.abs() {
            crate::schema::player::SplitOrientation::Vertical
        } else {
            crate::schema::player::SplitOrientation::Horizontal
        })
    } else {
        None
    };

    for (mut camera, _) in &mut split_cameras {
        camera.is_active = should_split;
    }
    if let Ok(mut party_camera) = party_camera.single_mut() {
        party_camera.is_active = !should_split;
    }
}

/// Parse a `CameraConfig.orbit_button` / `character_rotate_button` string into
/// `(activate_on_lmb, activate_on_rmb)`.
pub fn parse_orbit_button(s: &str) -> (bool, bool) {
    match s {
        "Left"   => (true,  false),
        "Right"  => (false, true),
        "Either" => (true,  true),
        // Explicit opt-out — no warning, this is a real designer choice (e.g. local co-op
        // split-screen player cameras, where a shared mouse would otherwise orbit/zoom every
        // player's camera identically).
        "None"   => (false, false),
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
///
/// Queries `With<OrbitCamera>`, so in a `split` scene this fires on both per-player
/// `OrbitCamera`s (unlike `PartyOrbitCamera`, which it silently skips — see the limitation
/// noted above `PartyOrbitCamera`). In a `dynamic` split scene it fires on both split cameras
/// even when one is currently inactive (`Camera.is_active: false`) — harmless, since nothing
/// renders for the inactive one, but the shake state keeps accumulating so it's already correct
/// if that camera reactivates mid-shake. Intentional: each split-screen camera is a real independent
/// `OrbitCamera`, so a shake action shakes whichever camera(s) match its target, exactly like
/// single-player. See `SplitViewportSlot`.
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
