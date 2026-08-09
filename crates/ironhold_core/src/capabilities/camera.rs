use bevy::prelude::*;
use bevy::camera::Viewport;
use bevy::input::gamepad::{Gamepad, GamepadAxis};
use crate::capabilities::player::{BoundGamepad, CharacterController};
use crate::schema::player::InputMap;
use crate::runtime::scene_manager::LevelEntity;

/// Inserted by `Action::CameraShake` on the active orbit/party camera entity.
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

// ─── Unified camera mode (planning/features/camera_modes.md v1) ───────────────────────────────
//
// Replaces the old `OrbitCamera`/`PartyOrbitCamera`/`FlyCamera` trio with one data-carrying
// component (`ActiveCameraMode`, the runtime-resolved analog of the authored `CameraModeDef` in
// `schema/camera.rs` — NOT the same type, since this one holds `Entity`-independent mutable
// per-frame state like yaw/pitch/radius) plus:
//   - `CameraTargets(Vec<Entity>)` — camera-to-player ownership, present on every camera
//     regardless of mode (length 0 for Fixed/Flycam, 1 for Orbit/Follow/FirstPerson, N for Party).
//   - a zero-sized per-mode marker component (`OrbitCameraMode`, `PartyCameraMode`, etc.) —
//     `ActiveCameraMode` is a single enum component, so Bevy queries can't filter on *which
//     variant* it holds; `dynamic_split_screen_system` and `camera_shake_system` need exactly that
//     (real Bevy query filter, not a runtime branch), so the marker exists purely for that.
// Whichever system spawns/updates `ActiveCameraMode` is responsible for keeping the matching
// marker in sync — there is no v1 code path that changes a camera's mode after spawn (that's
// `SetCameraMode`, v2), so today the marker is simply inserted once alongside the enum and never
// touched again.

/// Camera-to-player ownership, present on every camera entity regardless of mode. Unifies the old
/// `OrbitCamera.target: Entity` / `PartyOrbitCamera.targets: Vec<Entity>` into one shape so every
/// consumer that needs "who owns this camera" queries `&CameraTargets` uniformly instead of
/// matching on `ActiveCameraMode`'s variant. `.first()` is "the/a owning player" for the common
/// single-owner case; empty means no owner (`Fixed`/`Flycam`).
#[derive(Component, Clone, Default)]
pub struct CameraTargets(pub Vec<Entity>);

/// Marks a camera entity as currently running `ActiveCameraMode::Orbit`.
#[derive(Component)]
pub struct OrbitCameraMode;
/// Marks a camera entity as currently running `ActiveCameraMode::Party`.
#[derive(Component)]
pub struct PartyCameraMode;
/// Marks a camera entity as currently running `ActiveCameraMode::Fixed`.
#[derive(Component)]
pub struct FixedCameraMode;
/// Marks a camera entity as currently running `ActiveCameraMode::Follow`.
#[derive(Component)]
pub struct FollowCameraMode;
/// Marks a camera entity as currently running `ActiveCameraMode::FirstPerson`.
#[derive(Component)]
pub struct FirstPersonCameraMode;
/// Marks a camera entity as currently running `ActiveCameraMode::Flycam`.
#[derive(Component)]
pub struct FlycamCameraMode;

/// Present on a camera while it's under an explicit `Action::SetCameraMode` override (**v2**) —
/// i.e. its current mode came from the `camera_modes:` registry, not its own scene-authored
/// starting mode. Inserted when `SetCameraMode(mode: "<preset>")` targets this camera; removed
/// when `SetCameraMode(mode: "default")` restores it. `dynamic_split_screen_system` checks this to
/// suspend its automatic merge/split `is_active` toggling on an overridden camera — see that
/// system's own doc comment.
#[derive(Component)]
pub struct CameraModeOverride;

/// The `CameraModeDef` a camera was spawned with (**v2**) — what `Action::SetCameraMode(mode:
/// "default")` restores a camera to after one or more registry-driven switches. Written once at
/// spawn time (from the same `CameraModeDef` that produced the camera's initial `ActiveCameraMode`)
/// and never mutated afterward — a `SetCameraMode` targeting a *named registry preset* changes only
/// `ActiveCameraMode`/the marker/`CameraBlendState`, never this component. The dynamic-split merged
/// camera (built by `dynamic_split_screen_system`'s owning spawn site, not from any authored
/// `camera_mode:`) gets an explicitly synthesized `Party`-mode value here — it has no authored mode
/// of its own to record.
#[derive(Component, Clone)]
pub struct AuthoredCameraMode(pub crate::schema::camera::CameraModeDef);

/// Runtime-resolved camera state, one variant per `CameraModeDef` case. The single source of
/// truth for a camera's per-frame mutable state (yaw/pitch/radius/etc.); the marker components
/// above exist alongside it purely so other systems can query by kind (see module doc above).
#[derive(Component)]
pub enum ActiveCameraMode {
    Orbit(OrbitState),
    Party(PartyState),
    Fixed(FixedState),
    Follow(FollowState),
    FirstPerson(FirstPersonState),
    Flycam(FlycamState),
}

/// Runtime state for `ActiveCameraMode::Orbit` — identical fields to the old `OrbitCamera`
/// component, minus `target` (now `CameraTargets`, shared across every mode — Blocker 3).
pub struct OrbitState {
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
    /// Keyboard camera-look bindings, pre-resolved once at spawn from `InputMap.look_left`/
    /// `look_right`/`look_up`/`look_down` (mirrors how `orbit_lmb`/`orbit_rmb` are pre-resolved
    /// from RON strings rather than re-parsed every frame). `None` = that axis unbound.
    pub look_left_key: Option<KeyCode>,
    pub look_right_key: Option<KeyCode>,
    pub look_up_key: Option<KeyCode>,
    pub look_down_key: Option<KeyCode>,
    /// Angular rate (rad/sec) for keyboard-held camera look. Sourced from
    /// `CameraConfig.look_speed`.
    pub look_speed: f32,
    /// Analog stick deadzone for camera pitch, pre-resolved at spawn from the player's own
    /// `InputMap.gamepad_deadzone`.
    pub gamepad_deadzone: f32,
}

pub fn camera_orbit_system(
    time: Res<Time>,
    mut mouse_motion_events: MessageReader<bevy::input::mouse::MouseMotion>,
    mut mouse_wheel_events: MessageReader<bevy::input::mouse::MouseWheel>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    gamepad_query: Query<&Gamepad>,
    bound_q: Query<&BoundGamepad>,
    mut camera_query: Query<(&mut Transform, &mut ActiveCameraMode, &CameraTargets), (With<OrbitCameraMode>, Without<CharacterController>)>,
    mut character_query: Query<&mut Transform, (With<CharacterController>, Without<OrbitCameraMode>)>,
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

    for (mut cam_transform, mut mode, targets) in &mut camera_query {
        let Some(target) = targets.0.first().copied() else { continue };
        let ActiveCameraMode::Orbit(orbit) = &mut *mode else { continue };

        let lmb = mouse_button_input.pressed(MouseButton::Left);
        let rmb = mouse_button_input.pressed(MouseButton::Right);
        let orbit_active = (orbit.orbit_lmb && lmb) || (orbit.orbit_rmb && rmb);

        if zoom_delta != 0.0 {
            orbit.radius -= zoom_delta * orbit.zoom_speed * time.delta_secs();
            orbit.radius = orbit.radius.clamp(orbit.min_radius, orbit.max_radius);
        }

        if orbit_active {
            orbit.yaw -= mouse_delta.x * orbit.orbit_speed * time.delta_secs();
            orbit.pitch -= mouse_delta.y * orbit.orbit_speed * time.delta_secs();
            orbit.pitch = orbit.pitch.clamp(orbit.min_pitch, orbit.max_pitch);
        }

        // Keyboard camera look — independent of the mouse-orbit gate above (split-screen scenes
        // disable mouse-orbit per camera via `orbit_button: "None"`, since one shared mouse can't
        // drive 2+ independently-active cameras; this is the per-player alternative). Pitch
        // direction mirrors the mouse convention above (`look_up` increases `pitch` toward
        // `max_pitch`, matching "mouse down" in this codebase's pitch convention, not a literal
        // "up = sky" reading) — see `planning/features/done/per_player_camera_look_controls.md`.
        let dt = time.delta_secs();
        if orbit.look_left_key.is_some_and(|k| keyboard_input.pressed(k)) {
            orbit.yaw += orbit.look_speed * dt;
        }
        if orbit.look_right_key.is_some_and(|k| keyboard_input.pressed(k)) {
            orbit.yaw -= orbit.look_speed * dt;
        }
        if orbit.look_up_key.is_some_and(|k| keyboard_input.pressed(k)) {
            orbit.pitch = (orbit.pitch + orbit.look_speed * dt).min(orbit.max_pitch);
        }
        if orbit.look_down_key.is_some_and(|k| keyboard_input.pressed(k)) {
            orbit.pitch = (orbit.pitch - orbit.look_speed * dt).max(orbit.min_pitch);
        }

        // Gamepad camera pitch via right-stick-Y — independent of the mouse-orbit gate above,
        // same rationale as keyboard look above (split-screen disables mouse-orbit per camera).
        // Right-stick-Y is otherwise unused (only right-stick-X drives InputAction::Turn), so
        // this is a net-new axis with no conflict. Sign is NOT inverted: pushing the stick up
        // (positive GamepadAxis::RightStickY, confirmed by this codebase's existing LeftStickY
        // usage in input_translator_system, where a forward/up push produces move_vec.y > 0
        // matching the keyboard "forward" key) increases pitch toward max_pitch, exactly like
        // the keyboard look_up key above — direction pinned by a regression test, not asserted
        // from hardware feel.
        // Resolved live via the owning player's `BoundGamepad` (through `CameraTargets`) instead
        // of a value pre-resolved onto this component at spawn time — a spawn-frozen copy would
        // otherwise silently diverge from `BoundGamepad` the moment the player's binding resolves
        // or their pad reconnects to a different slot. See `planning/features/
        // gamepad_player_binding_hardening.md`.
        let gamepad = bound_q.get(target).ok()
            .and_then(|bound| bound.0)
            .and_then(|e| gamepad_query.get(e).ok());
        if let Some(gp) = gamepad {
            let stick_y = gp.get(GamepadAxis::RightStickY).unwrap_or(0.0);
            if stick_y.abs() > orbit.gamepad_deadzone {
                orbit.pitch = (orbit.pitch + stick_y * orbit.look_speed * dt)
                    .clamp(orbit.min_pitch, orbit.max_pitch);
            }
        }

        let char_rotate = (orbit.character_rotate_rmb && rmb)
            || (orbit.character_rotate_lmb && lmb);
        if char_rotate {
            if let Ok(mut char_transform) = character_query.get_mut(target) {
                char_transform.rotate_y(-mouse_delta.x * orbit.orbit_speed * time.delta_secs());
            }
        }

        if let Ok(char_transform) = character_query.get(target) {
            let target_pos = char_transform.translation + orbit.look_at_offset;
            let rot = Quat::from_axis_angle(Vec3::Y, orbit.yaw)
                * Quat::from_axis_angle(Vec3::X, -orbit.pitch);
            let offset = rot * Vec3::Z * orbit.radius;
            cam_transform.translation = target_pos + offset;
            cam_transform.look_at(target_pos, Vec3::Y);
        }
    }
}

/// Runtime state for `ActiveCameraMode::Party` — identical fields to the old `PartyOrbitCamera`
/// component, minus `targets` (now `CameraTargets`).
pub struct PartyState {
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
///
/// `own_viewport_only` is only ever `true` from the `dynamic`-split merged-state caller (a
/// pure `party:`-block scene has no split cameras/rings to restrict, so it's always `false`
/// there). When `true`, this camera gets `all_ring_layers()` — layer 0 (ordinary scene geometry)
/// *plus every reserved ring layer* — so the merged view still shows every player's ring even
/// though each ring only carries its own single layer. Leaving this camera componentless
/// (implicit layer 0 only) would make it render zero rings once any ring restricts itself to a
/// non-zero layer — see the plan's plan-review note.
pub fn spawn_party_orbit_camera(
    commands: &mut Commands,
    tonemapping: bevy::core_pipeline::tonemapping::Tonemapping,
    base_camera: &crate::schema::player::CameraConfig,
    party: &crate::schema::player::PartyZoomDef,
    targets: &[Entity],
    own_viewport_only: bool,
) -> Entity {
    let (orbit_lmb, orbit_rmb) = parse_orbit_button(&base_camera.orbit_button);
    let entity = commands.spawn((
        Name::new("Party Orbit Camera"),
        Camera3d::default(),
        tonemapping,
        // Real position/look-at is set on the first `party_camera_follow_system` tick, once
        // target transforms are available — this initial transform is just a safe placeholder.
        Transform::from_translation(Vec3::from(base_camera.offset))
            .looking_at(Vec3::ZERO, Vec3::Y),
        LevelEntity,
        ActiveCameraMode::Party(PartyState {
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
        }),
        PartyCameraMode,
        CameraTargets(targets.to_vec()),
        // Matches the FOV every split/orbit camera gets (`entity_spawner.rs`'s `insert_fov`) —
        // without this, a `split.dynamic` scene's merge/split transition visibly pops the field of
        // view, since the party camera would otherwise sit at Bevy's `Projection::default()` (45°)
        // while its sibling split cameras use `base_camera.fov`.
        Projection::Perspective(PerspectiveProjection {
            fov: base_camera.fov.to_radians(),
            ..default()
        }),
    )).id();
    if own_viewport_only {
        commands.entity(entity).insert(all_ring_layers());
    }
    // Synthesized `AuthoredCameraMode` (**v2**) — this camera is engine-constructed (the shared
    // `party:` field, or the merged camera in a `split.dynamic` scene), not resolved from a
    // directly-authored `components.camera_mode: Party(...)`, so it has no authored mode of its
    // own to record. Build an equivalent `PartyCameraDef` from the same fields `PartyState` above
    // was built from, so `SetCameraMode(mode: "default")` on this camera has something to restore.
    commands.entity(entity).insert(AuthoredCameraMode(crate::schema::camera::CameraModeDef::Party(
        crate::schema::camera::PartyCameraDef {
            look_at_offset: base_camera.look_at_offset,
            zoom_margin: party.zoom_margin,
            min_radius: base_camera.min_radius,
            max_radius: base_camera.max_radius,
            orbit_speed: base_camera.orbit_speed,
            zoom_speed: base_camera.zoom_speed,
            orbit_button: base_camera.orbit_button.clone(),
            allow_manual_zoom: party.allow_manual_zoom,
            transition: None,
        },
    )));
    entity
}

/// Frames the midpoint of a `Party`-mode camera's `CameraTargets` each frame and derives the
/// orbit radius from their maximum pairwise separation, clamped to `[min_radius, max_radius]`.
/// Mirrors `camera_orbit_system`'s mouse-orbit handling but has no single character to rotate.
pub fn party_camera_follow_system(
    time: Res<Time>,
    mut mouse_motion_events: MessageReader<bevy::input::mouse::MouseMotion>,
    mut mouse_wheel_events: MessageReader<bevy::input::mouse::MouseWheel>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut camera_query: Query<(&mut Transform, &mut ActiveCameraMode, &CameraTargets), With<PartyCameraMode>>,
    target_query: Query<&Transform, (With<CharacterController>, Without<PartyCameraMode>)>,
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

    for (mut cam_transform, mut mode, targets) in &mut camera_query {
        let ActiveCameraMode::Party(party) = &mut *mode else { continue };

        let positions: Vec<Vec3> = targets.0.iter()
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

/// Runtime state for `ActiveCameraMode::Fixed` — new in v1, no pre-existing behavior to match.
pub struct FixedState {
    pub position: Vec3,
    pub look_at: Option<Vec3>,
    /// Prefab instance id, re-resolved via `SpawnRegistry` every frame so the camera keeps
    /// pointing at the target as it moves. Takes priority over `look_at` when both resolve.
    pub look_at_entity: Option<String>,
}

/// Keeps a `Fixed`-mode camera at its configured `position`, pointed at its configured
/// `look_at`/`look_at_entity`. Writes `position` unconditionally every frame — not just at spawn
/// — so this system is also the one `Action::SetCameraMode` (v2) relies on to actually relocate
/// the camera on a switch; `apply_camera_mode` deliberately doesn't touch `Transform` itself (see
/// its own doc comment), and a `CameraBlendState` blends *toward* whatever this system writes, so
/// if this system only wrote `position` at spawn, switching to `Fixed` would silently never move
/// the camera (real bug, caught by 3 independent post-implementation reviews before it shipped).
pub fn fixed_camera_system(
    mut camera_query: Query<(&mut Transform, &ActiveCameraMode), With<FixedCameraMode>>,
    registry: Res<crate::runtime::scene_manager::SpawnRegistry>,
    transforms: Query<&GlobalTransform>,
) {
    for (mut transform, mode) in &mut camera_query {
        let ActiveCameraMode::Fixed(fixed) = mode else { continue };
        transform.translation = fixed.position;
        let look_at = fixed.look_at_entity.as_ref()
            .and_then(|id| registry.entities.get(id))
            .and_then(|&e| transforms.get(e).ok())
            .map(|gt| gt.translation())
            .or(fixed.look_at);
        if let Some(target) = look_at {
            transform.look_at(target, Vec3::Y);
        }
    }
}

/// Runtime state for `ActiveCameraMode::Follow` — new in v1, no pre-existing behavior to match.
pub struct FollowState {
    pub offset: Vec3,
    pub look_at_offset: Vec3,
    /// Position lerp rate (higher = snappier, 0 = instant).
    pub smoothing: f32,
    /// Separate lerp rate for look-at rotation.
    pub rotation_smoothing: f32,
}

/// Tracks a `Follow`-mode camera's single `CameraTargets` entity at a fixed offset, with
/// framerate-independent exponential smoothing (`1 - exp(-rate * dt)`) on both position and
/// look-at rotation — no free orbit input, unlike `Orbit`.
pub fn follow_camera_system(
    time: Res<Time>,
    mut camera_query: Query<(&mut Transform, &ActiveCameraMode, &CameraTargets), With<FollowCameraMode>>,
    target_query: Query<&Transform, (With<CharacterController>, Without<FollowCameraMode>)>,
) {
    let dt = time.delta_secs();
    for (mut transform, mode, targets) in &mut camera_query {
        let ActiveCameraMode::Follow(follow) = mode else { continue };
        let Some(target_entity) = targets.0.first().copied() else { continue };
        let Ok(target_transform) = target_query.get(target_entity) else { continue };

        let desired_pos = target_transform.translation + follow.offset;
        transform.translation = if follow.smoothing <= 0.0 {
            desired_pos
        } else {
            let t = 1.0 - (-follow.smoothing * dt).exp();
            transform.translation.lerp(desired_pos, t)
        };

        let look_target = target_transform.translation + follow.look_at_offset;
        let desired_rot = Transform::from_translation(transform.translation)
            .looking_at(look_target, Vec3::Y)
            .rotation;
        transform.rotation = if follow.rotation_smoothing <= 0.0 {
            desired_rot
        } else {
            let t = 1.0 - (-follow.rotation_smoothing * dt).exp();
            transform.rotation.slerp(desired_rot, t)
        };
    }
}

/// Runtime state for `ActiveCameraMode::FirstPerson` — new in v1, no pre-existing behavior to
/// match.
pub struct FirstPersonState {
    pub eye_offset: Vec3,
    pub sensitivity: f32,
    pub pitch: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
}

/// Locks a `FirstPerson`-mode camera to its `CameraTargets` entity's head position. Mouse look
/// yaw rotates the character directly (so the body faces where the camera looks, standard FPS
/// convention); pitch is camera-only.
pub fn first_person_camera_system(
    mut mouse_motion_events: MessageReader<bevy::input::mouse::MouseMotion>,
    mut camera_query: Query<(&mut Transform, &mut ActiveCameraMode, &CameraTargets), With<FirstPersonCameraMode>>,
    mut target_query: Query<&mut Transform, (With<CharacterController>, Without<FirstPersonCameraMode>)>,
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

    for (mut transform, mut mode, targets) in &mut camera_query {
        let Some(target_entity) = targets.0.first().copied() else { continue };
        let ActiveCameraMode::FirstPerson(fp) = &mut *mode else { continue };
        let Ok(mut target_transform) = target_query.get_mut(target_entity) else { continue };

        let yaw_delta = -mouse_delta.x * fp.sensitivity;
        if yaw_delta != 0.0 {
            target_transform.rotate_y(yaw_delta);
        }
        // `.min()`/`.max()`, not a bare `.clamp(min_pitch, max_pitch)`: `f32::clamp` panics if
        // min > max, and unlike Orbit's positive 0.1/0.9 defaults, FirstPerson's negative-min
        // default (-1.4/1.4) makes an authored-backwards pair an easy mistake to make, not just a
        // theoretical one (found by post-implementation review).
        fp.pitch = (fp.pitch - mouse_delta.y * fp.sensitivity)
            .clamp(fp.min_pitch.min(fp.max_pitch), fp.min_pitch.max(fp.max_pitch));

        transform.translation = target_transform.translation + fp.eye_offset;
        transform.rotation = target_transform.rotation * Quat::from_axis_angle(Vec3::X, fp.pitch);
    }
}

/// Runtime state for `ActiveCameraMode::Flycam` — identical fields to the old `FlyCamera`
/// component. See `capabilities::flycam::fly_camera_system`, which owns the movement logic.
pub struct FlycamState {
    pub speed: f32,
    pub fast_speed: f32,
    pub sensitivity: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub key_forward: KeyCode,
    pub key_backward: KeyCode,
    pub key_left: KeyCode,
    pub key_right: KeyCode,
    pub key_up: KeyCode,
    pub key_down: KeyCode,
    pub look_lmb: bool,
    pub look_rmb: bool,
}

/// Hard ceiling on `SplitOrientation::Grid`'s player count — bounds render-pass count (4 cameras
/// is the worst case this engine has validated) and avoids degenerate slivers on a misconfigured
/// scene. Extra players beyond this cap spawn cameraless, matching the existing (pre-Stage-6)
/// behavior when a 3rd player exists in a `Vertical`/`Horizontal` (always-2-way) scene.
pub const MAX_SPLIT_PLAYERS: u32 = 4;

/// The single reserved `RenderLayers` layer index for `player_index`'s target-indicator ring
/// under `SplitScreenDef.own_viewport_only` — layers 1..=`MAX_SPLIT_PLAYERS`, indexed identically
/// to `PLAYER_LABEL_COLORS`'s own scheme (same modulo-collision behavior that palette already
/// has). The sole owner of this arithmetic — every insertion site (both split-camera spawn sites,
/// `target_indicator_system`) calls this rather than re-deriving it, so raising
/// `MAX_SPLIT_PLAYERS` can never desync one site from another.
pub(crate) fn ring_layer_for_player(player_index: u32) -> usize {
    (1 + player_index % MAX_SPLIT_PLAYERS) as usize
}

/// The full union of every reserved ring layer, plus layer 0 (ordinary scene geometry) —
/// `spawn_party_orbit_camera`'s `RenderLayers` when `own_viewport_only` is true, so the merged/
/// party view still sees every player's ring. Derived from `MAX_SPLIT_PLAYERS` rather than a
/// hand-written literal, so raising that constant can never leave this union under-covering the
/// higher reserved layers `ring_layer_for_player` would then produce.
pub(crate) fn all_ring_layers() -> bevy::camera::visibility::RenderLayers {
    (1..=MAX_SPLIT_PLAYERS)
        .fold(bevy::camera::visibility::RenderLayers::layer(0), |layers, i| layers.with(i as usize))
}

/// Marks a local co-op split-screen camera and which share of the window it owns. Spawned
/// alongside a normal `Orbit`-mode camera (not a replacement) by `spawn_players_and_camera` when
/// `CameraConfig.split` is set — each split-screen camera independently tracks its own player,
/// exactly like a single-player orbit camera would, and only needs its `Camera.viewport`
/// constrained to its share of the window. Orientation is deliberately NOT stored here — see
/// `ActiveSplitScreen`.
#[derive(Component)]
pub struct SplitViewportSlot(pub u32);

/// Deterministic split-screen camera priority order: cameras with a `SplitViewportSlot` sort
/// before ones without (single-camera scenes and party cameras sort last), `Entity` breaks
/// ties. Ensures a selection among 2+ simultaneously active `Camera3d` entities is stable across
/// frames instead of depending on query iteration order. Shared by every system that must pick
/// one active camera among possibly several: `world_label_screen_pos_system`,
/// `rebuild_pool_meshes_system`'s billboard basis, `click_select_system`'s viewport-aware
/// click-to-select, and `nameplate_visibility_system`.
pub fn camera_priority_key(entity: Entity, slot: Option<&SplitViewportSlot>) -> (u32, Entity) {
    (slot.map_or(u32::MAX, |s| s.0), entity)
}

/// Fixed engine-side palette for the per-player split-screen HUD corner label, indexed by
/// `PlayerIndex`. Chosen to visually match `local_coop_demo`'s room6 `tint_blue`/`tint_pink`/
/// `tint_dark_green`/`tint_red` material RGB values, but this is its own independent constant —
/// NOT a read of a player's actual `material:` field (only room6 has one; rooms 3/4/5 use plain
/// untinted models). See `split_viewport_player_label_spawn_system`.
pub const PLAYER_LABEL_COLORS: [Color; MAX_SPLIT_PLAYERS as usize] = [
    Color::srgb(0.15, 0.35, 0.95), // P1 — matches tint_blue
    Color::srgb(0.95, 0.35, 0.70), // P2 — matches tint_pink
    Color::srgb(0.10, 0.40, 0.15), // P3 — matches tint_dark_green
    Color::srgb(0.85, 0.15, 0.15), // P4 — matches tint_red
];

/// Marker on a `SplitViewportSlot` camera entity indicating its corner "P{n}" HUD label has
/// already been spawned. Companion to `LinkedPlayerLabel`, which points at the UI entity itself.
/// Lets `split_viewport_player_label_spawn_system` use `Added<SplitViewportSlot>` filtering
/// instead of a per-frame "does a label already exist" scan — mirrors `nameplate_setup_system`'s
/// `Added<NameplateTag>` idiom.
#[derive(Component)]
pub struct SplitScreenPlayerLabel;

/// Points at the UI `Text` entity spawned for this split-screen camera's corner label. Attached
/// to the camera entity alongside `SplitScreenPlayerLabel`. Read every frame by
/// `split_viewport_player_label_update_system` to sync the label's position and visibility to
/// this camera's live `Camera.viewport`/`is_active`.
#[derive(Component)]
pub struct LinkedPlayerLabel(pub Entity);

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

/// Spawns a colored "P{n}" corner HUD label for every newly-added split-screen camera whose
/// `CameraTargets` points at an entity carrying a `PlayerIndex` — the first real consumer of that
/// component (see `capabilities/player.rs`). `Added<SplitViewportSlot>` fires exactly once per
/// camera (the frame it's spawned by `spawn_players_and_camera`), mirroring
/// `nameplate_setup_system`'s `Added<NameplateTag>` idiom, so no per-frame "does a label already
/// exist" scan is needed.
///
/// The label is a standalone (unparented) UI `Text` root — this resolves against the same
/// full-window `Camera2d` every existing RON UI label uses (confirmed by architecture review:
/// `IsDefaultUiCamera` is commented out on that camera, but every RON UI root and room6's
/// per-quadrant hints are all authored in full-window logical coordinates already). If a future
/// refactor of that `Camera2d`/`IsDefaultUiCamera` setup changes this, `split_viewport_
/// player_label_update_system`'s physical-viewport → logical-window conversion below would need
/// revisiting too.
///
/// Color comes from the fixed `PLAYER_LABEL_COLORS` palette, not from the player's `material:`
/// tint (rooms 3/4/5 have no tint at all — see the palette's doc comment). `TextShadow` keeps the
/// label legible against every room's differently-toned ground.
pub fn split_viewport_player_label_spawn_system(
    mut commands: Commands,
    new_cameras: Query<(Entity, &CameraTargets), Added<SplitViewportSlot>>,
    player_index_q: Query<&crate::capabilities::player::PlayerIndex>,
) {
    for (camera_entity, targets) in &new_cameras {
        let Some(target) = targets.0.first().copied() else { continue };
        let Ok(player_index) = player_index_q.get(target) else { continue };
        let color = PLAYER_LABEL_COLORS[player_index.0 as usize % PLAYER_LABEL_COLORS.len()];

        let label = commands.spawn((
            Name::new(format!("SplitScreenPlayerLabel: P{}", player_index.0 + 1)),
            Text::new(format!("P{}", player_index.0 + 1)),
            TextFont { font_size: 22.0, ..default() },
            TextColor(color),
            TextShadow::default(),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Visibility::Visible,
            LevelEntity,
        )).id();

        commands.entity(camera_entity).insert((
            SplitScreenPlayerLabel,
            LinkedPlayerLabel(label),
        ));
    }
}

/// Keeps every spawned corner label positioned in its own camera's viewport and in sync with
/// `Camera.is_active` (hidden while merged during a `split.dynamic` scene). `.after(
/// split_screen_viewport_system)` in `lib.rs`'s `.chain()` — on the exact frame a merge/split
/// transition flips `is_active` and recomputes `viewport`, this system reads both already-fresh
/// values in the same frame (per architecture review), so there is no stale position/visibility.
///
/// `Camera.viewport` is in physical pixels; `Node.left`/`top` are in logical pixels — divides by
/// `window.scale_factor()` to convert, the mirror image of `split_screen_viewport_system`'s own
/// physical-pixel care. Anchored to the top-right of each camera's cell (a fixed margin inset)
/// rather than top-left, since every room's `room_hint` title label sits at top-left (UX review).
pub fn split_viewport_player_label_update_system(
    window_q: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Camera, &LinkedPlayerLabel), With<SplitScreenPlayerLabel>>,
    mut labels: Query<(&mut Node, &mut Visibility)>,
) {
    const MARGIN_PX: f32 = 8.0;
    const LABEL_WIDTH_PX: f32 = 48.0;

    let Ok(window) = window_q.single() else { return };
    let scale_factor = window.scale_factor();

    for (camera, linked) in &cameras {
        let Ok((mut node, mut visibility)) = labels.get_mut(linked.0) else { continue };

        let new_visibility = if camera.is_active { Visibility::Visible } else { Visibility::Hidden };
        if *visibility != new_visibility {
            *visibility = new_visibility;
        }
        if !camera.is_active {
            continue;
        }

        let Some(viewport) = &camera.viewport else { continue };
        let right_edge = (viewport.physical_position.x + viewport.physical_size.x) as f32 / scale_factor;
        let top_edge = viewport.physical_position.y as f32 / scale_factor;
        // Guarded writes — Val implements PartialEq. An unconditional write marks Node changed
        // every frame, forcing Bevy's ui_layout_system to redo the taffy layout pass for no
        // reason (see CLAUDE.md's "Change-detection discipline"), even though this only actually
        // changes on window resize.
        let new_left = Val::Px(right_edge - LABEL_WIDTH_PX - MARGIN_PX);
        let new_top = Val::Px(top_edge + MARGIN_PX);
        if node.left != new_left { node.left = new_left; }
        if node.top != new_top { node.top = new_top; }
    }
}

/// Marks a `SplitViewportSlot` camera's per-viewport target-HUD readout as spawned. Companion to
/// `LinkedTargetHud`. Opt-in — only spawns when the scene authors a `target_hud:` block; a scene
/// with none gets no readout entities at all, matching `target_indicator:`'s own opt-in pattern.
/// See `planning/features/per_player_split_screen_targeting.md`.
#[derive(Component)]
pub struct SplitScreenTargetHud;

/// Points at the UI `Text` entity spawned for this split camera's target-HUD readout. Read every
/// frame by `target_hud_update_system` to sync text/position/visibility.
#[derive(Component)]
pub struct LinkedTargetHud(pub Entity);

/// Spawns one per-viewport target-HUD readout `Text` entity for every newly-added split-screen
/// camera, mirroring `split_viewport_player_label_spawn_system`'s `Added<SplitViewportSlot>`
/// idiom — but only when the scene authors a `target_hud:` block at all. Starts empty/hidden;
/// `target_hud_update_system` fills in text and shows it once that camera's owning player
/// actually has a target selected.
pub fn target_hud_spawn_system(
    mut commands: Commands,
    new_cameras: Query<Entity, Added<SplitViewportSlot>>,
    target_hud_cfg: Res<crate::runtime::scene_manager::LoadedTargetHud>,
) {
    let Some(cfg) = &target_hud_cfg.0 else { return };
    for camera_entity in &new_cameras {
        let (r, g, b, a) = cfg.color;
        let hud = commands.spawn((
            Name::new("TargetHud"),
            Text::new(String::new()),
            TextFont { font_size: cfg.font_size, ..default() },
            TextColor(Color::srgba(r, g, b, a)),
            TextShadow::default(),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Visibility::Hidden,
            LevelEntity,
        )).id();

        commands.entity(camera_entity).insert((
            SplitScreenTargetHud,
            LinkedTargetHud(hud),
        ));
    }
}

/// Keeps every spawned target-HUD readout's text synced to its camera's owning player's
/// `PlayerTarget`, and its position/visibility synced to that camera's live `Camera.viewport`/
/// `is_active` — same viewport-tracking approach as `split_viewport_player_label_update_system`,
/// anchored bottom-left (distinct from that system's top-right "P{n}" corner label) so the two
/// never collide. Hidden whenever the owning player has no target selected, not just when the
/// camera itself is inactive.
pub fn target_hud_update_system(
    window_q: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Camera, &CameraTargets, &LinkedTargetHud), With<SplitScreenTargetHud>>,
    player_targets: Query<&crate::capabilities::player::PlayerTarget>,
    prefab_keys: Query<&crate::runtime::scene_manager::PrefabKey>,
    registry: Res<crate::runtime::scene_manager::SpawnRegistry>,
    target_hud_cfg: Res<crate::runtime::scene_manager::LoadedTargetHud>,
    mut huds: Query<(&mut Text, &mut Node, &mut Visibility)>,
) {
    use crate::schema::scene_v2::TargetHudDisplay;

    let Some(cfg) = &target_hud_cfg.0 else { return };
    let Ok(window) = window_q.single() else { return };
    let scale_factor = window.scale_factor();

    const MARGIN_PX: f32 = 8.0;
    const READOUT_HEIGHT_PX: f32 = 24.0;

    for (camera, targets, linked) in &cameras {
        let Ok((mut text, mut node, mut visibility)) = huds.get_mut(linked.0) else { continue };

        if !camera.is_active {
            if *visibility != Visibility::Hidden { *visibility = Visibility::Hidden; }
            continue;
        }

        let Some(target_entity) = targets.0.first().copied() else { continue };
        let Ok(player_target) = player_targets.get(target_entity) else { continue };
        let Some(target_id) = &player_target.0 else {
            if *visibility != Visibility::Hidden { *visibility = Visibility::Hidden; }
            continue;
        };

        let prefab_key = registry.entities.get(target_id)
            .and_then(|&e| prefab_keys.get(e).ok())
            .map(|p| p.0.as_str());
        let new_text = match cfg.show {
            TargetHudDisplay::Full => match prefab_key {
                Some(p) => format!("{} {}", p, target_id),
                None => target_id.clone(),
            },
            TargetHudDisplay::NameOnly => prefab_key.unwrap_or(target_id).to_string(),
            TargetHudDisplay::IdOnly => target_id.clone(),
        };
        if text.0 != new_text { text.0 = new_text; }
        if *visibility != Visibility::Visible { *visibility = Visibility::Visible; }

        let Some(viewport) = &camera.viewport else { continue };
        let left_edge = viewport.physical_position.x as f32 / scale_factor;
        let bottom_edge = (viewport.physical_position.y + viewport.physical_size.y) as f32 / scale_factor;
        let new_left = Val::Px(left_edge + MARGIN_PX);
        let new_top = Val::Px(bottom_edge - READOUT_HEIGHT_PX - MARGIN_PX);
        if node.left != new_left { node.left = new_left; }
        if node.top != new_top { node.top = new_top; }
    }
}

/// Local co-op dynamic split (Stage 5): decides every frame whether the scene should be merged
/// (one shared `Party`-mode camera) or split (two per-player `Orbit`-mode cameras), and flips
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
///
/// Filters on the `OrbitCameraMode`/`PartyCameraMode` marker components (Blocker 5 — a bare
/// `With<ActiveCameraMode>` can't distinguish variants, since it's a single enum component, not
/// one type per mode) rather than `Without<PartyOrbitCamera>`'s old defensive-but-redundant
/// cross-filter.
///
/// **v2:** a camera currently carrying `CameraModeOverride` (an explicit `Action::SetCameraMode`
/// switch to a named preset, not this camera's own scene-authored default) has its automatic
/// `is_active` toggling suspended here — the designer's scripted override wins until an explicit
/// `SetCameraMode(mode: "default")` removes the marker, or the scene reloads. Each of the two split
/// cameras and the party camera is checked independently, so one overridden camera doesn't freeze
/// the others' normal merge/split dance.
pub fn dynamic_split_screen_system(
    dynamic_config: Res<crate::runtime::scene_manager::DynamicSplitConfig>,
    mut active_split: ResMut<crate::runtime::scene_manager::ActiveSplitScreen>,
    mut split_cameras: Query<(&mut Camera, &CameraTargets, Has<CameraModeOverride>), (With<OrbitCameraMode>, With<SplitViewportSlot>)>,
    mut party_camera: Query<(&mut Camera, Has<CameraModeOverride>), (With<PartyCameraMode>, Without<SplitViewportSlot>)>,
    transforms: Query<&Transform>,
) {
    let Some(dynamic) = dynamic_config.0.as_ref() else { return };

    let mut targets = split_cameras.iter().filter_map(|(_, t, _)| t.0.first().copied());
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

    for (mut camera, _, overridden) in &mut split_cameras {
        if !overridden {
            camera.is_active = should_split;
        }
    }
    if let Ok((mut party_camera, overridden)) = party_camera.single_mut() {
        if !overridden {
            party_camera.is_active = !should_split;
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

/// Applies and decays a procedural camera shake after `camera_orbit_system`/
/// `party_camera_follow_system` has set the orbital position. The shake is a deterministic
/// sine-wave offset (no RNG — WASM safe). Removes `CameraShakeState` when the remaining time
/// reaches zero.
///
/// Filters on `Or<(With<OrbitCameraMode>, With<PartyCameraMode>)>` — closes the old documented
/// gap where `Action::CameraShake` silently no-op'd on a `party:` scene (see
/// `SceneStateParams::orbit_cameras`, which now inserts `CameraShakeState` on both kinds).
/// Deliberately excludes `Fixed`/`FirstPerson`/`Flycam`'s markers: `fly_camera_system` runs after
/// this system in `lib.rs`'s `.chain()` and unconditionally overwrites `Transform::rotation`
/// every frame, so a flycam scene must keep getting `Action::CameraShake`'s explicit
/// `warn!("no orbit camera in scene — shake ignored")` instead of silently having its shake
/// applied then instantly overwritten.
pub fn camera_shake_system(
    time: Res<Time>,
    mut commands: Commands,
    mut camera_query: Query<(Entity, &mut Transform, &mut CameraShakeState), Or<(With<OrbitCameraMode>, With<PartyCameraMode>)>>,
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

/// In-progress `Action::SetCameraMode` transition (**v2**). Inserted alongside the marker/
/// `ActiveCameraMode` swap when the *target* mode authors a `transition:`; absent means an
/// instant cut. `from_translation`/`from_rotation`/`from_fov` are snapshotted once, at the moment
/// of the switch, from the camera's pose *before* the switch — see `camera_blend_system`'s doc for
/// how they're used.
#[derive(Component)]
pub struct CameraBlendState {
    pub remaining: f32,
    pub duration: f32,
    pub ease: crate::schema::camera::EaseKind,
    pub from_translation: Vec3,
    pub from_rotation: Quat,
    pub from_fov: f32,
    pub to_fov: f32,
}

/// Blends a camera's rendered `Transform`/FOV from a frozen pre-switch snapshot toward whichever
/// live pose the *newly active* mode's own per-frame system computes this frame — **must run
/// after every per-mode system** in `lib.rs`'s `.chain()` (`camera_orbit_system`,
/// `party_camera_follow_system`, `follow_camera_system`, `first_person_camera_system`,
/// `fixed_camera_system`, `fly_camera_system`), which is why it's the last entry there. Each
/// frame, the new mode's system already overwrote `Transform` to its own live target (as if no
/// blend were happening); this system then overwrites it again with
/// `from.lerp(that_live_target, eased_t)` — so `eased_t → 1.0` converges exactly onto the new
/// mode's real behavior, and player input to the new mode (mouse-orbit, etc.) is NOT suppressed
/// during the blend (a deliberate v2 simplification — blends are recommended ≤0.4s, per
/// `planning/features/camera_modes.md`, short enough that this is imperceptible; logged as a
/// possible future refinement rather than blocking the feature on it).
pub fn camera_blend_system(
    time: Res<Time>,
    mut commands: Commands,
    mut camera_query: Query<(Entity, &mut Transform, Option<&mut Projection>, &mut CameraBlendState)>,
) {
    for (entity, mut transform, projection, mut blend) in &mut camera_query {
        blend.remaining = (blend.remaining - time.delta_secs()).max(0.0);
        let t_raw = 1.0 - (blend.remaining / blend.duration.max(f32::EPSILON));
        let t = blend.ease.apply(t_raw.clamp(0.0, 1.0));
        transform.translation = blend.from_translation.lerp(transform.translation, t);
        transform.rotation = blend.from_rotation.slerp(transform.rotation, t);
        if let Some(mut projection) = projection {
            if let Projection::Perspective(ref mut persp) = *projection {
                let from_rad = blend.from_fov.to_radians();
                let to_rad = blend.to_fov.to_radians();
                persp.fov = from_rad + (to_rad - from_rad) * t;
            }
        }
        if blend.remaining <= 0.0 {
            commands.entity(entity).remove::<CameraBlendState>();
        }
    }
}
