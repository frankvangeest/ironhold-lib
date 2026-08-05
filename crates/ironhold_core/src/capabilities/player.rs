use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::schema::player::InputMap;
use crate::schema::stats::{LoadedStats, LoadedModifiers}; // used by update_player_speed_system
use crate::runtime::messages::*;
use crate::runtime::scene_manager::ActiveViewBox;
use std::collections::HashMap;

use crate::capabilities::animation_resolver::{LocomotionState, AnimationRequests};

/// Multiplier applied to walk/run speed from the `player_speed` stat.
/// Updated in `Update` by `update_player_speed_system`; consumed in `FixedUpdate`
/// by `player_movement_system` — keeps stat reads out of the physics hot path.
#[derive(Component)]
pub struct SpeedMultiplier(pub f32);

/// Marker for a player-controlled entity, as opposed to NPCs, props, or other prefabs.
/// Inserted unconditionally wherever a player entity is spawned — GLB (`spawn_player_entity`)
/// or primitive (inline in `scene_loader.rs`), scene-placed or dynamic character-select.
/// Distinct from `CharacterController`: a future networked remote player may carry `Player`
/// without local input handling.
#[derive(Component)]
pub struct Player;

/// Whether a `Player` entity is controlled by this client (`Local`) or mirrored from another
/// client (`Remote`). Always `Local` today — there is no multiplayer code yet. Reserved as a
/// forward-compat hook for Beta 0.6 (LAN co-op) so nameplate/UI/camera systems can distinguish
/// "me" from "other players" without another schema pass once real players exist.
#[derive(Component, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayerOwnership {
    #[default]
    Local,
    Remote,
}

/// Forwarded from `PrefabDef.player_index`. Inserted on every player entity (local co-op or
/// single-player, where it's always `0`). Drives the split-screen HUD corner label's "P{n}" text
/// and `PLAYER_LABEL_COLORS` palette index (`capabilities/camera.rs`) — its first real consumer.
/// Local co-op still identifies "the first player" for camera/party switches by scene `entities`
/// order, not by this value; only the HUD label reads it.
#[derive(Component, Clone, Copy, Default)]
pub struct PlayerIndex(pub u32);

/// This player's currently selected target (spawn ID), independent of every other player's.
/// Inserted alongside `PlayerIndex`/`CharacterController` at both player-construction sites
/// (GLB: `spawn_player_entity_core` in `entity_spawner.rs`; primitive: inline in
/// `scene_loader.rs`) — always present on any player entity, defaulting to `None`.
///
/// The player with no `PlayerIndex` or `PlayerIndex(0)` is "primary" — `capabilities/targeting.rs`
/// mirrors the primary player's `PlayerTarget` into the global `CurrentTarget` resource, so
/// `{target}` substitution (`rules.ron`/`state_machine.ron`/behaviors) and the action bar's
/// `{target}`-gated cost check keep resolving against the primary player exactly as before this
/// component existed — see `planning/features/per_player_split_screen_targeting.md`. A
/// non-primary player's `PlayerTarget` only drives their own visual feedback (target indicator
/// ring, per-viewport HUD readout); it has no gameplay effect through the shared action pipeline.
#[derive(Component, Default)]
pub struct PlayerTarget(pub Option<String>);

/// This player's resolved physical gamepad, if any. Inserted alongside `PlayerIndex`/
/// `PlayerTarget` at every player-construction site (`gamepad_player_binding_hardening.md`).
///
/// `None` ("pending") means either no `InputMap.gamepad_index` was authored, or the authored
/// index hasn't resolved to a live connected gamepad **that has been stable for
/// `GAMEPAD_STABLE_CONNECT_SECS`** yet — `gamepad_bind_system` (`runtime/input.rs`) retries the
/// resolution every tick while pending, using `gamepad_index` purely as a **one-time seed**, not a
/// live positional lookup. The stability requirement exists because of a real hardware finding: a
/// single physical controller can register as two separate gamepad entries for a brief moment,
/// and without a debounce a player could permanently lock onto the spurious one in the window
/// before it disappears.
///
/// `Some(entity)` ("bound") locks this player to that specific gamepad `Entity` for the rest of
/// their lifetime (until a future hot-leave/rejoin) — every gamepad-consuming system reads this
/// directly instead of re-deriving a sorted position every frame, so a disconnect/reconnect of
/// any *other* pad can never silently re-route this player's input. A disconnected bound pad
/// simply stops matching `Query<&Gamepad>` (Bevy never despawns the entity, only removes the
/// `Gamepad` component) — this player's gamepad input silently pauses; their keyboard bindings,
/// if any, are unaffected, since gamepad input is always additive in this engine. On reconnect of
/// the *same* device, Bevy re-inserts `Gamepad` onto the *same* `Entity`, so this player's input
/// resumes automatically with no extra code.
#[derive(Component, Default)]
pub struct BoundGamepad(pub Option<Entity>);

#[derive(Component)]
pub struct CharacterController {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub rot_speed: f32,
    pub inputs: InputMap,
    pub is_running: bool,
    /// Pre-computed initial Y velocity for a normal jump.
    pub jump_velocity: f32,
    /// Whether a second jump in mid-air is permitted.
    pub double_jump_enabled: bool,
    /// Pre-computed initial Y velocity for the second jump.
    pub double_jump_velocity: f32,
    /// Number of jumps already used in the current airborne period (reset on landing).
    pub jumps_used: u8,
    /// Maximum jumps per airborne period: 1 = normal, 2 = double jump.
    pub max_jumps: u8,
    /// Radius of the ground-detection sphere cast (= capsule radius).
    pub collider_radius: f32,
    /// How far below the feet the ground-detection sphere is swept each frame.
    pub ground_cast_length: f32,
    /// Velocity decay multiplier each physics tick when there is no input. Default: 0.8.
    pub idle_drag: f32,
}

/// Reads the `player_speed` stat once per rendered frame (Update) and writes the resulting
/// multiplier onto `SpeedMultiplier` — keeping all stat access out of FixedUpdate.
pub fn update_player_speed_system(
    loaded_stats: Option<Res<LoadedStats>>,
    loaded_modifiers: Option<Res<LoadedModifiers>>,
    mut query: Query<&mut SpeedMultiplier>,
) {
    let multiplier = loaded_stats.as_ref()
        .and_then(|ls| ls.0.get("player_speed"))
        .map(|s| {
            let effective = loaded_modifiers.as_ref()
                .map(|m| s.compute_effective(&m.0))
                .unwrap_or(s.current);
            effective / s.def.base.max(0.001)
        })
        .unwrap_or(1.0);

    for mut sm in &mut query {
        if (sm.0 - multiplier).abs() > 0.001 {
            sm.0 = multiplier;
        }
    }
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
        &SpeedMultiplier,
    )>,
    rapier_context: Option<ReadRapierContext>,
    mut game_events: MessageWriter<GameEvent>,
) {
    let mut actions = HashMap::new();
    for event in input_events.read() {
        actions.entry(event.entity).or_insert_with(Vec::new).push(event.action.clone());
    }

    // Try to get the rapier context, but don't panic if it's missing (e.g. in headless tests)
    let rapier_context = rapier_context.as_ref().and_then(|rc| rc.single().ok());

    for (entity, mut transform, global_transform, mut controller, mut loco, mut velocity, mut requests, speed_mul) in &mut query {
        let mut move_vec = Vec3::ZERO;
        let mut rotation = 0.0;
        let mut jumping = false;

        // Perform raycast for ground detection every frame for animation state
        let was_grounded = loco.is_grounded;
        
        if let Some(ref context) = rapier_context {
            // Sphere cast from the entity origin (feet) downward.
            // Using a ball equal to the capsule radius rather than a point ray means
            // the detection covers the full bottom-hemisphere footprint, so sloped and
            // rough terrain is handled correctly — matching how Unity's CharacterController
            // and Godot's CharacterBody3D sweep the capsule shape for floor detection.
            let feet_pos = global_transform.translation();
            // Build a sphere matching the capsule's bottom hemisphere and sweep it
            // downward. Passing the raw parry shape is required; bevy_rapier3d's
            // Collider wrapper does not implement the parry Shape trait directly.
            let ground_ball = Collider::ball(controller.collider_radius);
            loco.is_grounded = context.cast_shape(
                feet_pos,
                Quat::IDENTITY,
                Vec3::NEG_Y,
                ground_ball.raw.as_ref(),
                ShapeCastOptions {
                    max_time_of_impact: controller.ground_cast_length,
                    ..default()
                },
                QueryFilter::new().exclude_rigid_body(entity),
            ).is_some();
        } else {
            // Default to grounded if no physics (for basic testing)
            loco.is_grounded = true;
        }

        // Detect landing
        if !was_grounded && loco.is_grounded {
            requests.queue.push_back("jump_exit".to_string());
            controller.jumps_used = 0;
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
            } * speed_mul.0;

            velocity.linvel.x = move_vec.x * speed;
            velocity.linvel.z = move_vec.z * speed;

            loco.moving = true;
            loco.running = controller.is_running;
        } else {
            velocity.linvel.x *= controller.idle_drag;
            velocity.linvel.z *= controller.idle_drag;
            loco.moving = false;
            loco.running = false;
        }

        // Jump logic — grounded first jump, or double jump in air.
        // `jumps_used == 0` ensures we can't re-trigger while still in the
        // grounded-ray window immediately after jumping.
        let can_jump = if loco.is_grounded {
            controller.jumps_used == 0
        } else {
            controller.double_jump_enabled && controller.jumps_used < controller.max_jumps
        };
        if jumping && can_jump {
            let vel = if controller.jumps_used > 0 {
                controller.double_jump_velocity
            } else {
                controller.jump_velocity
            };
            debug!("Jump triggered (jumps_used={}) velocity_y={:.2}", controller.jumps_used, vel);
            velocity.linvel.y = vel;
            controller.jumps_used += 1;
            requests.queue.push_back("jump_enter".to_string());
            game_events.write(GameEvent::Trigger("player.jumped".to_string()));
        }
    }
}

/// Clamps every `CharacterController` entity's XZ position into `ActiveViewBox` (Y/jump is
/// untouched). Also zeroes the clamped axis's `Velocity.linvel` component — without that,
/// Rapier keeps re-integrating the outward velocity every tick and the player visibly
/// jitters against the edge instead of stopping cleanly. Runs after `player_movement_system`
/// so it clamps this tick's movement, not last tick's.
pub fn player_view_box_clamp_system(
    view_box: Res<ActiveViewBox>,
    mut query: Query<(&mut Transform, &mut Velocity), With<CharacterController>>,
) {
    let Some((min_x, min_z, max_x, max_z)) = view_box.0 else { return };

    for (mut transform, mut velocity) in &mut query {
        let clamped_x = transform.translation.x.clamp(min_x, max_x);
        let clamped_z = transform.translation.z.clamp(min_z, max_z);

        if clamped_x != transform.translation.x {
            velocity.linvel.x = 0.0;
            transform.translation.x = clamped_x;
        }
        if clamped_z != transform.translation.z {
            velocity.linvel.z = 0.0;
            transform.translation.z = clamped_z;
        }
    }
}
