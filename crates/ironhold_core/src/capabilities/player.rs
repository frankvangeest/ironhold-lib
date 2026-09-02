use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::schema::player::InputMap;
use crate::schema::stats::{LoadedStats, LoadedModifiers}; // used by update_player_speed_system
use crate::runtime::messages::*;
use crate::runtime::scene_manager::ActiveViewBox;
use crate::runtime::scene_manager::scene_loader::GRAVITY;
use std::collections::HashMap;

/// `FixedUpdate`'s tick rate — Bevy's engine default (`Time<Fixed>`'s period), unmodified
/// anywhere in this crate. Used only to convert the physically-derived jump air-grace duration
/// (seconds) into a tick count once, at jump-fire time — see `CharacterController::jump_air_grace`.
const FIXED_TICK_RATE: f32 = 64.0;

/// Safety multiplier applied to the analytically-estimated ground-sensor detach time when
/// deriving `jump_air_grace`. `t_detach` (see `jump_air_grace_ticks`) is a simplified ballistic
/// estimate — actual Rapier contact/integration timing can lag it slightly — so this leaves
/// comfortable headroom without approaching the real detach time (measured ~9 ticks / ~0.14s at
/// shipped defaults; 2x gives ~0.26s, still small against any shipped project's real jump airtime
/// of ~1.2s). See `planning/features/uphill_jump_lock.md`.
const JUMP_AIR_GRACE_SAFETY: f32 = 2.0;

/// Ground-sensor combined reach: how far below the entity's feet origin `player_movement_system`'s
/// downward shape-cast can detect a surface (see the cast in `player_movement_system` below).
fn ground_sensor_reach(controller: &CharacterController) -> f32 {
    controller.collider_radius + controller.ground_cast_length
}

/// Derives the jump air-grace window (in `FixedUpdate` ticks) for a jump fired at `velocity` — the
/// minimum time to force a `jumps_used` reset to wait for, so that a steep-enough slope (whose
/// ground-check never truthfully reports "ungrounded", see `planning/features/
/// uphill_jump_lock.md`) can't permanently starve the reset. Derived from the same physical
/// quantities `player_movement_system`'s ground-check and `resolve_jump_velocity` already use —
/// not a separate tuned constant — so it can never desync from a project's authored
/// `collider_radius`/`ground_cast_length`/`jump` values.
///
/// `t_detach` solves the ballistic height equation `h = v·t − ½g·t²` for the first time the jump
/// clears the sensor's combined reach `h`. When the jump's own apex (`v²/2g`) can't reach `h` at
/// all (`vel² <= 2·g·h` — a project whose `jump` height can't clear its own `ground_cast_length`,
/// a pre-existing, non-slope-specific instance of this same bug class — see the design-time
/// `warn!`/`ironhold_cli validate` check below), the `.max(0.0)` clamp degrades `t_detach` to
/// `vel / GRAVITY` (time to apex) — the grace window caps at a full jump's airborne duration
/// instead of growing unbounded.
fn jump_air_grace_ticks(vel: f32, controller: &CharacterController) -> u16 {
    let h = ground_sensor_reach(controller);
    // `f32::max` returns the non-NaN operand when either side is NaN (IEEE 754 semantics), so
    // this also guards against a negative/NaN `vel` from a misconfigured `jump`/
    // `double_jump_height` (negative height, negative `RelativeToHeight` percent) — the
    // design-time `warn!`/`ironhold_cli validate` check should catch that authoring mistake
    // before it ships, but this keeps the runtime from degrading straight back into the
    // permanent lock this fix exists to close if it doesn't.
    let vel = vel.max(0.0);
    let discriminant = (vel * vel - 2.0 * GRAVITY * h).max(0.0);
    let t_detach = (vel - discriminant.sqrt()) / GRAVITY;
    let grace_secs = JUMP_AIR_GRACE_SAFETY * t_detach;
    // `.max(1)`: a near-zero `vel` would otherwise round to 0 ticks of grace, letting the reset
    // fire the very next tick and re-arm every other tick (an audible `player.jumped` event
    // storm on a bound sound) instead of the intended bounded fallback.
    ((grace_secs * FIXED_TICK_RATE).ceil() as u16).max(1)
}

/// Converts `MovementConfig::coyote_time_secs` (or a negative/NaN misauthoring of it) into a
/// `FixedUpdate` tick count. `f32::max(0.0, secs)` launders a negative/NaN value to `0.0` (no
/// buffer — degrades to the pre-coyote raw-sensor behavior, never a panic or a stuck-forever
/// buffer) the same way `jump_air_grace_ticks` guards its own input.
fn coyote_ticks(secs: f32) -> u16 {
    (secs.max(0.0) * FIXED_TICK_RATE).round() as u16
}

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
    /// `FixedUpdate` ticks remaining before a grounded-and-`jumps_used > 0` reading is even
    /// considered as a possible landing (as opposed to residual ground-sensor contact from the
    /// jump that just fired) — a cheap minimum floor, not the sole correctness mechanism (see
    /// `jump_liftoff_y` below for why). Set via `jump_air_grace_ticks()` on every jump fire;
    /// decremented in `player_movement_system`. See `planning/features/uphill_jump_lock.md` —
    /// this field never affects `LocomotionState.is_grounded` or which branch of `can_jump`
    /// runs; it only gates the `jumps_used` reset.
    pub jump_air_grace: u16,
    /// World-space Y position at the moment of the most recent jump fire, or `None` if no jump
    /// is currently pending a reset. Once `jump_air_grace` reaches 0, the reset additionally
    /// requires either `velocity.linvel.y <= 0.0` (ballistic ascent has genuinely ended) OR the
    /// entity having risen at least `collider_radius + ground_cast_length` above this position
    /// (proof the ground-sensor's overlap with the liftoff pose can no longer explain a grounded
    /// reading) — both are physical quantities, not clock-derived, so correctness can't desync
    /// from `jump_air_grace`'s tick count regardless of framerate (unlike a tick-only grace,
    /// which assumes `FixedUpdate` ticks and Rapier's own — separately clocked — physics
    /// stepping advance in lockstep). See `planning/features/uphill_jump_lock.md`.
    pub jump_liftoff_y: Option<f32>,
    /// Radius of the ground-detection sphere cast (= capsule radius).
    pub collider_radius: f32,
    /// How far below the feet the ground-detection sphere is swept each frame.
    pub ground_cast_length: f32,
    /// Maximum surface angle (degrees from horizontal) the ground sensor treats as walkable —
    /// a hit surface steeper than this is never counted as grounded. See
    /// `MovementConfig::max_walkable_slope_deg`'s doc comment for why this matters (it's what
    /// stops a genuinely too-steep incline from letting jump silently re-arm every tick while
    /// sliding, uphill or downhill).
    pub max_walkable_slope_deg: f32,
    /// Seconds the ground sensor keeps reporting grounded after it stops finding a walkable
    /// surface — see `MovementConfig::coyote_time_secs`'s doc comment.
    pub coyote_time_secs: f32,
    /// `FixedUpdate` ticks remaining before `LocomotionState.is_grounded` actually switches to
    /// `false`, once the raw ground-sensor+slope check first stops finding a walkable surface.
    /// Refreshed to `coyote_ticks(coyote_time_secs)` every tick the raw check *does* find one
    /// (so it only ever delays leaving the ground, never delays returning to it — landing stays
    /// exactly as responsive as before this field existed). Set/decremented entirely within
    /// `player_movement_system`; see `planning/features/uphill_jump_lock.md`.
    pub coyote_ticks_remaining: u16,
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

/// Whether a ground-cast hit's contact normal is within the walkable slope limit — the single
/// source of truth for "is this actually floor", shared by `ground_cast`'s underfoot-candidate
/// loop below (see its doc comment) and `player_movement_system`'s own final `raw_grounded` check.
/// A hit with no computable normal (`details: None`, a `Failed`-status cast) is treated as
/// unwalkable rather than assumed walkable, UNLESS `max_walkable_slope_deg >= 90.0` (the
/// documented "disable this check" escape hatch — see `MovementConfig.max_walkable_slope_deg`'s
/// doc comment), in which case any hit at all counts as walkable regardless of its normal
/// (including a detail-less hit, which would otherwise incorrectly stay ungrounded even at the
/// maximum limit) — this is what makes `90.0` restore this project's original pre-fix
/// proximity-only grounding exactly, even after `ground_cast`'s underfoot filter (below) was
/// added: at `90.0`, the very first hit `ground_cast` finds is always walkable, so it's accepted
/// immediately with zero exclusion iterations, matching the old single-cast behavior.
fn is_walkable_contact(controller: &CharacterController, details: Option<ShapeCastHitDetails>) -> bool {
    controller.max_walkable_slope_deg >= 90.0 || details.is_some_and(|d| {
        // `normalize_or_zero()`, not a bare `.dot()`: on a *penetrating* hit (`time_of_
        // impact == 0`) parry's EPA normal is not unit length (measured ~0.52 in
        // practice, not 1.0) — an un-normalized `.dot(Vec3::Y).acos()` computes
        // `acos(|n| * cos(theta))`, not the real angle theta, silently biasing every
        // penetrating-hit angle toward 90°. Harmless when the surface really is steep
        // (still correctly unwalkable either way), but would report a false "too steep"
        // for a genuinely walkable penetrating contact. A zero-length result (fully
        // degenerate normal) dots to 0, giving a 90° angle — unwalkable, matching the
        // existing "no computable normal" treatment above.
        let normal = d.normal1.normalize_or_zero();
        let angle_from_up_deg = normal.dot(Vec3::Y).clamp(-1.0, 1.0).acos().to_degrees();
        angle_from_up_deg <= controller.max_walkable_slope_deg
    })
}

/// The ground-detection shape-cast, including the underfoot-candidate re-query loop — shared by
/// `player_movement_system` and its test probe (`prop_ground_veto_tests.rs`) so the two can never
/// silently diverge (see `planning/backlog.md`'s former "solid prop disables jump" entry for the
/// bug this loop fixes: a solid prop/wall pressed against the player has a penetrating,
/// `time_of_impact == 0` contact that always beats the real floor's non-zero toi, so
/// `RapierContext::cast_shape` — which only ever returns the single nearest hit — reported the
/// wall instead of the floor).
///
/// Sphere cast from *above* the entity origin (feet), not from the feet themselves. Using a ball
/// equal to the capsule radius rather than a point ray means the detection covers the full
/// bottom-hemisphere footprint, so sloped and rough terrain is handled correctly — matching how
/// Unity's CharacterController and Godot's CharacterBody3D sweep the capsule shape for floor
/// detection.
///
/// Casting from the feet position itself (this project's original approach) starts the ball
/// already embedded in whatever's below — a resting character's feet sit exactly on the surface.
/// On a solid convex shape (e.g. a thick box) EPA still happens to resolve the minimum-translation
/// vector straight up, matching the true surface normal by geometric coincidence. On this
/// project's real terrain collider — a zero-thickness `TriMesh` (`capabilities/terrain.rs`) —
/// there is no "up" to resolve through; the shortest way out of a buried point on a flat plane is
/// sideways, so the returned normal comes back ~90° from vertical regardless of the triangle's
/// actual slope, misclassifying it as unwalkable *at every angle, including dead flat*. Confirmed
/// independently by two post-implementation reviews (measured against real `rapier3d`/`parry3d`)
/// before this was caught — this bug never reproduced in `player_slope_jump_tests.rs` because
/// every test there uses a solid `Collider::cuboid` slope, the one geometry family where the bug
/// is invisible.
///
/// Lifting the cast's start point above the surface by the ball's own radius (plus a small skin
/// margin so it doesn't re-embed from floating-point slop) guarantees the cast begins genuinely
/// separated, so EPA/GJK always resolves the real contact normal — verified against both solid
/// and `TriMesh` geometry. `max_time_of_impact` is extended by the same lift so the total reach
/// below the feet stays exactly `collider_radius + ground_cast_length` (every formula derived from
/// that combined reach — `ground_sensor_reach()`, `jump_air_grace_ticks()`, the design-time
/// `warn!`/`ironhold_cli validate` check — needs no change).
///
/// The underfoot-candidate loop below re-queries (bounded at `MAX_GROUND_CAST_CANDIDATES`),
/// excluding any hit whose contact point isn't actually underfoot AND isn't itself walkable —
/// both conditions, not either alone, so a legitimate floor contact is never wrongly excluded:
/// (1) a solid wall's contact is both above-tolerance and unwalkable (near-horizontal normal), so
/// it's correctly excluded; (2) a walkable slope steeper than `acos(1 - 0.5) = 60°` (where the
/// contact point would otherwise read as "not underfoot" purely from geometry) is still accepted,
/// since its normal passes `is_walkable_contact` regardless of contact height; (3) a contact point
/// that reads as deeply "not underfoot" only because the player is momentarily penetrating the
/// floor by more than the tolerance (e.g. right after a spawn/teleport/`at_entity` placement) is
/// also still accepted for the same reason. This makes the filter monotone — the underfoot check
/// can only ever *rescue* a hit `is_walkable_contact` alone would have rejected (the wall case,
/// the actual bug), never *reject* one `is_walkable_contact` alone would have accepted — so this
/// fix cannot itself turn a previously-grounded tick ungrounded.
pub fn ground_cast(
    context: &RapierContext,
    entity: Entity,
    feet_pos: Vec3,
    controller: &CharacterController,
) -> Option<(Entity, ShapeCastHit)> {
    const GROUND_CAST_SKIN: f32 = 0.01;
    const MAX_GROUND_CAST_CANDIDATES: usize = 4;
    let lift = controller.collider_radius + GROUND_CAST_SKIN;
    let cast_origin = feet_pos + Vec3::Y * lift;
    // Build a sphere matching the capsule's bottom hemisphere and sweep it downward. Passing the
    // raw parry shape is required; bevy_rapier3d's Collider wrapper does not implement the parry
    // Shape trait directly.
    let ground_ball = Collider::ball(controller.collider_radius);
    let cast_options = ShapeCastOptions {
        max_time_of_impact: lift + controller.ground_cast_length,
        // Already `true` in `ShapeCastOptions::default()` (parry3d) — kept explicit as a pin
        // against that default ever changing, since without it a near-zero-clearance resting
        // contact (`PenetratingOrWithinTargetDist` status) omits the hit normal entirely, silently
        // defeating the walkable-slope check.
        compute_impact_geometry_on_penetration: true,
        ..default()
    };
    // Half the cast ball's own radius: comfortably below a wall's contact height (close to a full
    // `collider_radius` above the feet when the ball is embedded against a vertical surface) while
    // generous enough for genuine floor variance — per-tick position/slope drift under continuous
    // physics resolution is a small fraction of the collider radius at any sane walk speed. Note
    // this alone would impose a hidden `acos(1 - 0.5) = 60°` walkable-slope ceiling independent of
    // `max_walkable_slope_deg` — closed by also accepting any `is_walkable_contact` hit regardless
    // of this tolerance (see the loop below).
    let underfoot_tolerance = controller.collider_radius * 0.5;
    let mut excluded_this_tick: Vec<Entity> = Vec::new();
    loop {
        if excluded_this_tick.len() >= MAX_GROUND_CAST_CANDIDATES {
            return None;
        }
        let excluded = &excluded_this_tick;
        let predicate = move |e: Entity| !excluded.contains(&e);
        let mut filter = QueryFilter::new()
            .exclude_rigid_body(entity)
            // `.exclude_sensors()`: a `trigger_zone` prefab field spawns a child entity with
            // `Collider::ball(radius)` + `Sensor` (`entity_spawner.rs`'s `attach_prefab_
            // features`) — a ghost collider that must never count as floor. Without this, the
            // ground cast could sweep straight into a nearby prop's trigger-zone sensor and treat
            // it as a hit — a real playtest bug (`planning/features/uphill_jump_lock.md`). Matches
            // the existing `exclude_sensors()` precedent in `capabilities/npc.rs`'s
            // line-of-sight raycast.
            .exclude_sensors();
        // Only attach the predicate once there's something to exclude — the overwhelmingly common
        // case (open ground, or any solid prop/wall the player hasn't already been pressed against
        // this same tick) never needs it, and skipping it avoids an extra indirect call +
        // collider→entity decode per broad-phase candidate on every single ground cast, not just
        // the rare re-query path.
        if !excluded_this_tick.is_empty() {
            filter = filter.predicate(&predicate);
        }
        let (hit_entity, candidate) = context.cast_shape(
            cast_origin,
            Quat::IDENTITY,
            Vec3::NEG_Y,
            ground_ball.raw.as_ref(),
            cast_options,
            filter,
        )?;
        // `witness1` (`ShapeCastHitDetails`) is world-space here despite parry's own
        // "local-space" doc comment on the type (`parry3d-0.25.3/src/query/shape_cast/
        // shape_cast.rs`) — verified against `bevy_rapier3d-0.33.0/src/plugin/context/mod.rs`'s
        // `RapierContext::cast_shape` doc comment (~line 478), which explicitly states witness/
        // normal 1 refer to the world collider, in world space. (Two nearby lookalike doc
        // comments in the same crate are not this citation: one sits inside a commented-out
        // `cast_shape_nonlinear` wrapper, the other documents `rapier3d`'s own
        // `cast_shape_nonlinear`, not the plain `cast_shape` this code calls.)
        // A hit with no computable contact point (`details: None`, a `Failed` status) is treated
        // as underfoot by default — its fate is decided by `is_walkable_contact` below either way
        // (which itself treats a detail-less hit as unwalkable, unless the `90.0` escape hatch is
        // set), so this default only matters for which branch of the `||` accepts it; it doesn't
        // change the outcome.
        let is_underfoot = candidate.details
            .map_or(true, |d| d.witness1.y <= feet_pos.y + underfoot_tolerance);
        if is_underfoot || is_walkable_contact(controller, candidate.details) {
            return Some((hit_entity, candidate));
        }
        excluded_this_tick.push(hit_entity);
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
        // The un-debounced sensor reading for this tick — see below for why the `jumps_used`
        // reset logic reads this instead of `loco.is_grounded` once coyote-time is applied.
        let raw_grounded;
        if let Some(ref context) = rapier_context {
            // See `ground_cast`'s doc comment for the full ground-detection design (sphere-cast
            // origin lift, TriMesh-vs-cuboid normal caveat, and the underfoot-candidate re-query
            // loop that stops a solid prop/wall from vetoing a legitimate floor beneath it) and
            // `is_walkable_contact`'s for the walkable-slope-angle check.
            let feet_pos = global_transform.translation();
            let hit = ground_cast(context, entity, feet_pos, &controller);
            raw_grounded = hit.is_some_and(|(_, hit)| is_walkable_contact(&controller, hit.details));
            // Coyote-time debounce: only delays *leaving* the ground, never *returning* to it —
            // becoming grounded always refreshes the buffer to full and reports grounded
            // immediately, with no added latency on landing. Without this, a single-tick sensor
            // false-negative from mildly uneven terrain (a small rock, a mesh seam, a decorative
            // prop's edge) flickers the falling animation on and off while just walking, since
            // `animation_resolver.rs` reads `LocomotionState.is_grounded` with no filtering of
            // its own. See `MovementConfig::coyote_time_secs`'s doc comment.
            //
            // This buffered value drives `LocomotionState.is_grounded` — i.e. animation and
            // `can_jump`'s branch selection (deliberately: coyote-forgiveness for jump timing is
            // the same mechanism) — but NOT the `jumps_used` reset logic below, which reads
            // `raw_grounded` directly. Feeding the buffered value into the reset logic instead
            // was tried and reverted: it let the reset's `risen_since_liftoff >= reach` fallback
            // (designed to fire *while still rising*, for the continuously-climbing-slope case)
            // also fire during an ordinary flat-ground jump, the moment real detachment happened
            // to coincide with the coyote window — resetting `jumps_used` well before the jump's
            // apex on a ballistic arc that was never near any slope. The reset logic already has
            // its own carefully-tuned grace/velocity/height mechanism for exactly this class of
            // timing problem; it doesn't need a second, conflicting layer of buffering on top.
            if raw_grounded {
                controller.coyote_ticks_remaining = coyote_ticks(controller.coyote_time_secs);
                loco.is_grounded = true;
            } else if controller.coyote_ticks_remaining > 0 {
                controller.coyote_ticks_remaining -= 1;
                loco.is_grounded = true;
            } else {
                loco.is_grounded = false;
            }
        } else {
            // Default to grounded if no physics (for basic testing)
            raw_grounded = true;
            loco.is_grounded = true;
        }

        // Landing animation: fire on any genuine airborne->grounded edge, exactly as before this
        // fix (a plain fall — e.g. walking off a ledge with `jumps_used` already 0 — must still
        // play the landing clip). This is independent of the jump-count reset below.
        if !was_grounded && loco.is_grounded {
            requests.queue.push_back("jump_exit".into());
        }

        // Reset the jump count once genuinely landed. Gated by `jump_air_grace`, not the
        // `!was_grounded && is_grounded` edge above: on a steep-enough slope the ground-check can
        // report `is_grounded = true` on every single tick, even immediately after a jump impulse
        // (the incline's rising surface closes the vertical gap faster than gravity opens it) —
        // an edge-triggered reset would then never fire again for the rest of the session. See
        // `planning/features/uphill_jump_lock.md`. This never touches `loco.is_grounded` itself
        // or which branch of `can_jump` below runs (double-jump height is unaffected) — it only
        // gates the `jumps_used` reset.
        //
        // `jump_air_grace` alone is not sufficient: it's counted in `FixedUpdate` ticks, but
        // Rapier's own physics stepping runs on `TimestepMode::Variable` in `PostUpdate` (see
        // `capabilities/physics.rs`) — a different, framerate-coupled clock. At a low enough
        // framerate (or one clamped `Time<Virtual>::max_delta` hitch), real elapsed *physics* time
        // can lag behind the tick count, so the grace window alone could expire while the body is
        // still genuinely rising. The two extra checks below are physical, not clock-derived, so
        // they can't desync from however much real physics time has actually elapsed:
        // `velocity.linvel.y <= 0.0` covers a jump whose ballistic ascent has genuinely ended
        // (flat ground, or a jump too short to ever clear the sensor — see
        // `warn_jump_cannot_clear_ground_sensor`); the liftoff-height check covers a *continuously
        // climbing* slope, where the contact solver keeps `linvel.y` pinned positive (matching the
        // climb rate) for as long as the player keeps walking uphill, so `linvel.y <= 0.0` alone
        // would never fire there — but net height risen since the jump still grows the whole time,
        // so it reliably clears `collider_radius + ground_cast_length` well before or around when
        // `jump_air_grace` expires.
        // Reads `raw_grounded`, not `loco.is_grounded`, deliberately: the coyote-time debounce
        // above (see its own comment) only ever *extends* how long a grounded reading persists,
        // which would let this reset's `risen_since_liftoff` fallback (designed to fire while
        // still ascending, for the slope case) also fire during an ordinary flat-ground jump at
        // the exact moment real detachment happens to fall inside the coyote window — resetting
        // `jumps_used` well before the jump's apex, nowhere near any slope.
        if controller.jump_air_grace > 0 {
            controller.jump_air_grace -= 1;
        } else if raw_grounded && controller.jumps_used > 0 {
            let risen_since_liftoff = controller.jump_liftoff_y
                .map(|liftoff_y| global_transform.translation().y - liftoff_y)
                .unwrap_or(f32::INFINITY);
            if velocity.linvel.y <= 0.0 || risen_since_liftoff >= ground_sensor_reach(&controller) {
                controller.jumps_used = 0;
                controller.jump_liftoff_y = None;
            }
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
        //
        // Grounded branch uses `raw_grounded || (coyote-buffered && never yet jumped)`, NOT the
        // fully coyote-buffered `loco.is_grounded` — deliberately asymmetric. Coyote-forgiveness
        // for a *first* jump (pressing jump shortly after walking off a ledge, `jumps_used == 0`)
        // is intentional — see `MovementConfig::coyote_time_secs`'s doc comment — but extending
        // that same buffer to gate the *double-jump* branch was a real bug, found by three
        // independent post-implementation reviews: since the two branches are mutually exclusive,
        // a fully coyote-buffered `is_grounded` kept the double-jump branch unreachable for the
        // entire coyote window after a real ground jump (`jumps_used == 1` there), even once the
        // player had genuinely left the ground — silently swallowing a double-jump press for the
        // whole buffer duration (up to permanently, for a very large `coyote_time_secs`). Gating
        // the grounded branch on `jumps_used == 0` specifically means the coyote buffer can only
        // ever unlock the *first* jump, never mask the second — `raw_grounded` alone always governs
        // double-jump availability, exactly as before coyote-time existed.
        let can_jump = if raw_grounded || (controller.coyote_ticks_remaining > 0 && controller.jumps_used == 0) {
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
            controller.jump_air_grace = jump_air_grace_ticks(vel, &controller);
            controller.jump_liftoff_y = Some(global_transform.translation().y);
            requests.queue.push_back("jump_enter".into());
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
