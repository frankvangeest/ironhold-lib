// Regression coverage for the "uphill jump lock" bug fix — see
// `planning/features/uphill_jump_lock.md`. Steps a real Rapier physics world (a sloped static
// collider + a player capsule matching `spawn_player_entity_core`'s construction) across many
// `FixedUpdate` ticks, driving `player_movement_system` directly via `run_system_once` — the
// mechanism is physics-timing-dependent and not reliably reproducible by hand or by the headless
// (no-Rapier-context) tests in `action_tests.rs`/`scene_lifecycle_tests.rs`.
use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy_rapier3d::prelude::*;
use ironhold_core::runtime::{InputAction, InputActionMessage, GameEvent};
use ironhold_core::capabilities::player::{CharacterController, SpeedMultiplier, player_movement_system};
use ironhold_core::capabilities::animation_resolver::{LocomotionState, AnimationRequests};
use ironhold_core::schema::player::InputMap;

mod support;
use support::setup_test_app;

fn input_map() -> InputMap {
    InputMap {
        forward: "KeyW".to_string(),
        backward: "KeyS".to_string(),
        left: "KeyA".to_string(),
        right: "KeyD".to_string(),
        strafe_left: "KeyQ".to_string(),
        strafe_right: "KeyE".to_string(),
        jump: "Space".to_string(),
        run: "ShiftLeft".to_string(),
        interact: "KeyF".to_string(),
        strafe_mouse_button: Some("Left".to_string()),
        target_next: "Tab".to_string(),
        target_range: 30.0,
        gamepad_index: None,
        look_left: None, look_right: None, look_up: None, look_down: None,
        gamepad_jump: "South".to_string(),
        gamepad_run: "East".to_string(),
        gamepad_interact: "West".to_string(),
        gamepad_target_next: "North".to_string(),
        gamepad_deadzone: 0.15,
    }
}

struct Case {
    app: App,
    player: Entity,
    tick: usize,
    jumps_taken: u32,
}

/// Ground collider family a test case is built on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum GroundKind {
    /// A thick solid box. Convenient for most tests, but **not representative of real terrain**:
    /// on a thick convex shape, even a feet-embedded (penetrating) shape-cast happens to resolve
    /// the true surface normal by geometric coincidence. Every test using this ground kind alone
    /// would have missed the real bug found in post-implementation review (see `TriMesh` below).
    Cuboid,
    /// A zero-thickness triangle mesh, matching `capabilities/terrain.rs`'s actual
    /// `ComputedColliderShape::TriMesh(TriMeshFlags::default())` real terrain collider. A
    /// feet-embedded shape-cast against a zero-thickness surface has no "up" to resolve through —
    /// the shortest way out of a buried point is sideways, so the normal comes back ~90° from
    /// vertical regardless of the triangle's true slope, misclassifying *any* terrain (including
    /// dead flat) as unwalkable. This is exactly the bug two independent post-implementation
    /// reviews caught (measured against real `rapier3d`/`parry3d`) that no `Cuboid`-only test
    /// suite could see, since it's invisible on thick convex geometry. Fixed by lifting the
    /// shape-cast's origin above the surface before sweeping down (see `player.rs`) — these tests
    /// exist specifically to keep that fix from regressing.
    TriMesh,
}

/// A single large flat quad (two triangles), tall enough (±400m) that a tilted copy still passes
/// under the player's spawn/settle position — the `TriMesh` equivalent of the `Cuboid` case's
/// `Collider::cuboid(400.0, 0.25, 400.0)`. Uses `Collider::trimesh` directly (no flags), matching
/// `capabilities/terrain.rs`'s real `TriMeshFlags::default()` (empty) exactly. Sized generously
/// (not the original 60m) so an actively-moving test (`run_speed 10` over 200+ ticks) can never
/// reach the slab's edge cap, whose normal differs from the slope's own — see
/// `unwalkable_slope_never_reports_grounded`'s comment for the real bug this margin exists to
/// prevent.
fn trimesh_ground_collider() -> Collider {
    let s = 400.0;
    let vertices = vec![
        Vec3::new(-s, 0.0, -s),
        Vec3::new(s, 0.0, -s),
        Vec3::new(s, 0.0, s),
        Vec3::new(-s, 0.0, s),
    ];
    let indices = vec![[0u32, 1, 2], [0, 2, 3]];
    Collider::trimesh(vertices, indices).expect("valid trimesh")
}

/// Spawns a real Rapier world: a slope of `angle_deg` (rising toward +X) with a player capsule
/// identical to `spawn_player_entity_core`'s (collider/damping/friction/controller values), then
/// settles it onto the slope. `angle_deg: 0.0` is flat ground. One `step()` call always equals
/// one `FixedUpdate` tick (so `jump_air_grace_ticks()`'s tick-counting is exercised normally),
/// but `physics_dt` controls how much *real physics time* Rapier advances per tick — passing
/// something other than `1.0/64.0` deliberately decouples the two clocks, simulating the
/// tick-vs-Rapier-timestep mismatch a low real framerate (or a `Time<Virtual>::max_delta`-clamped
/// hitch) can cause in production, where `player_movement_system`'s `FixedUpdate` ticks are
/// counted independently of Rapier's own `TimestepMode::Variable` stepping in `PostUpdate`.
fn setup_case_full(angle_deg: f32, jump_velocity: f32, double_jump_enabled: bool, max_jumps: u8, physics_dt: f32, max_walkable_slope_deg: f32, ground_kind: GroundKind) -> Case {
    let mut app = setup_test_app();
    app.insert_resource(TimestepMode::Fixed { dt: physics_dt, substeps: 1 });
    // Pin the virtual clock so `GamePlugin`'s own `FixedUpdate`-registered `player_movement_system`
    // never self-triggers off real wall-clock time during `app.update()` — this harness drives it
    // exclusively via explicit `run_system_once` calls in `step()`, and letting `FixedUpdate`'s
    // real-time accumulator also fire it (a variable number of extra times depending on how long
    // each test iteration actually takes to run) would make jump counts nondeterministic.
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::ZERO));
    app.update();

    let theta = angle_deg.to_radians();
    let ground_transform = Transform::from_translation(Vec3::new(0.25 * theta.sin(), -0.25 * theta.cos(), 0.0))
        .with_rotation(Quat::from_rotation_z(theta));
    match ground_kind {
        GroundKind::Cuboid => {
            // Sized generously (not a tight fit) so an actively-moving test can't reach the
            // slab's edge cap within any test's tick budget — see `unwalkable_slope_never_
            // reports_grounded`'s comment for the real, previously-passing-by-margin bug this
            // prevents (an edge cap's normal differs from the slope face's own).
            app.world_mut().spawn((RigidBody::Fixed, Collider::cuboid(400.0, 0.25, 400.0), ground_transform));
        }
        GroundKind::TriMesh => {
            // The trimesh quad itself sits exactly at y=0 in its own local frame (no thickness to
            // offset for, unlike the cuboid's half-height) — reuse the same rotation, skip the
            // cuboid's `-0.25*cos(theta)` vertical offset.
            app.world_mut().spawn((RigidBody::Fixed, trimesh_ground_collider(), Transform::from_rotation(Quat::from_rotation_z(theta))));
        }
    }

    let player = app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.02, 0.0),
        CharacterController {
            walk_speed: 5.0,
            run_speed: 10.0,
            rot_speed: 3.0,
            inputs: input_map(),
            is_running: true,
            jump_velocity,
            double_jump_enabled,
            double_jump_velocity: jump_velocity,
            jumps_used: 0,
            max_jumps,
            collider_radius: 0.4,
            ground_cast_length: 0.3,
            max_walkable_slope_deg,
            coyote_time_secs: 0.1,
            coyote_ticks_remaining: 0,
            idle_drag: 0.8,
            jump_air_grace: 0, jump_liftoff_y: None,
        },
        LocomotionState::default(),
        AnimationRequests::default(),
        SpeedMultiplier(1.0),
        RigidBody::Dynamic,
        Collider::compound(vec![(
            Vec3::new(0.0, 0.9, 0.0),
            Quat::IDENTITY,
            Collider::capsule_y(0.5, 0.4),
        )]),
        LockedAxes::ROTATION_LOCKED,
        Damping { linear_damping: 0.5, angular_damping: 0.5 },
        Velocity::zero(),
        ExternalImpulse::default(),
        Friction { coefficient: 0.15, combine_rule: CoefficientCombineRule::Min },
    )).id();

    // Settle onto the slope.
    for _ in 0..40 { app.update(); }

    Case { app, player, tick: 0, jumps_taken: 0 }
}

/// `setup_case_with_dt_and_slope_limit` with `physics_dt = 1.0/64.0` (real physics time advances
/// at exactly the rate `jump_air_grace_ticks()`'s tick-counting assumes — only the
/// framerate-independence test below deliberately picks a different `physics_dt`) and
/// `max_walkable_slope_deg = 45.0` (the shipped default — only the slope-limit tests below
/// deliberately pick a different value).
fn setup_case(angle_deg: f32, jump_velocity: f32, double_jump_enabled: bool, max_jumps: u8) -> Case {
    setup_case_full(angle_deg, jump_velocity, double_jump_enabled, max_jumps, 1.0 / 64.0, 45.0, GroundKind::Cuboid)
}

/// `setup_case_full` with the shipped-default `physics_dt` and `Cuboid` ground, letting a test
/// pick a specific `max_walkable_slope_deg`.
fn setup_case_with_slope_limit(angle_deg: f32, jump_velocity: f32, double_jump_enabled: bool, max_jumps: u8, max_walkable_slope_deg: f32) -> Case {
    setup_case_full(angle_deg, jump_velocity, double_jump_enabled, max_jumps, 1.0 / 64.0, max_walkable_slope_deg, GroundKind::Cuboid)
}

/// `setup_case_full` with the shipped defaults, on `TriMesh` ground — the real terrain
/// collider's geometry family, not the `Cuboid` every other test in this file uses.
fn setup_case_trimesh(angle_deg: f32, jump_velocity: f32) -> Case {
    setup_case_full(angle_deg, jump_velocity, false, 1, 1.0 / 64.0, 45.0, GroundKind::TriMesh)
}

/// Advances one tick, sending `Move(forward)` (unless `moving` is false) and `Jump` (iff
/// `jumping`). Returns this tick's `is_grounded` and `jumps_used` for assertions.
fn step(case: &mut Case, moving: bool, jumping: bool) -> (bool, u8) {
    {
        let mut msgs = case.app.world_mut().resource_mut::<Messages<InputActionMessage>>();
        msgs.clear();
        if moving {
            msgs.write(InputActionMessage { entity: case.player, action: InputAction::Move(Vec2::new(1.0, 0.0)) });
        }
        if jumping {
            msgs.write(InputActionMessage { entity: case.player, action: InputAction::Jump(true) });
        }
    }
    case.app.world_mut().run_system_once(player_movement_system).unwrap();
    // Count this tick's firings, then explicitly `.update()` (rotate) the buffer ourselves —
    // `setup_test_app()` never registers `message_update_system` for `GameEvent` (nothing in this
    // harness needs it otherwise), so without this the buffer would just keep accumulating every
    // write for the test's whole lifetime, making a plain read-and-sum silently correct only by
    // accident of registration order. Doing the rotation explicitly here makes `+=` unconditionally
    // correct regardless of what `setup_test_app()` does or doesn't register elsewhere.
    {
        let mut messages = case.app.world_mut().resource_mut::<Messages<GameEvent>>();
        let fired_this_tick = messages
            .iter_current_update_messages()
            .filter(|e| matches!(e, GameEvent::Trigger(n) if n == "player.jumped"))
            .count() as u32;
        messages.update();
        case.jumps_taken += fired_this_tick;
    }
    case.app.update();
    case.tick += 1;

    let controller = case.app.world().entity(case.player).get::<CharacterController>().unwrap();
    let loco = case.app.world().entity(case.player).get::<LocomotionState>().unwrap();
    (loco.is_grounded, controller.jumps_used)
}

/// Drains and returns this tick's queued `AnimationRequests` (as pushed by
/// `player_movement_system`), so a test can assert on `"jump_exit"`/`"jump_enter"` without
/// caring about `jumps_used`/velocity.
fn drain_animation_requests(case: &mut Case) -> Vec<String> {
    let mut entity = case.app.world_mut().entity_mut(case.player);
    let mut requests = entity.get_mut::<AnimationRequests>().unwrap();
    requests.queue.drain(..).map(|r| r.clip_or_id).collect()
}

/// Runs `ticks` ticks holding Move + spamming Jump every tick (as in the original bug report:
/// "run toward any hill and spam jump while ascending"). Returns the final `jumps_taken` count.
fn spam_jump(case: &mut Case, ticks: usize) -> u32 {
    for _ in 0..ticks {
        step(case, true, true);
    }
    case.jumps_taken
}

#[test]
fn unwalkable_slope_never_reports_grounded() {
    // Regression guard for a real bug found during real-hardware playtest: a slope steeper than
    // `max_walkable_slope_deg` must never register as "grounded" at all — otherwise (via the same
    // mechanism the rest of this file fixes) continuous ground-sensor contact while sliding down
    // it lets `jumps_used` reset every tick once grace expires, since `velocity.linvel.y` is
    // trivially negative for the entire descent. A 60° slope, with the shipped default 45°
    // walkable limit, must never classify as grounded — matching how Unity's
    // `CharacterController.slopeLimit`, Unreal's `WalkableFloorAngle`, and Godot's
    // `floor_max_angle` all define "floor" by contact-normal angle, not raw proximity.
    //
    // `moving: true` actively runs the player up the slope at `run_speed` for the full 200 ticks
    // (~3.1s @ 64Hz, ~31m of travel) — this previously came within ~7% of reaching the ground
    // slab's edge cap, whose normal (~30° from vertical) is legitimately walkable at the 45°
    // default and would have made this test fail for a reason unrelated to its own intent
    // ("a 60° slope must never report grounded" is not what an edge-cap false-positive means).
    // Fixed by generously sizing the slab (`Collider::cuboid`/`trimesh_ground_collider`, both
    // 400m) rather than disabling movement, since active movement while sliding down an
    // unwalkable slope is exactly the reported real-world scenario.
    let mut case = setup_case(60.0, 5.94, false, 1);
    for _ in 0..200 {
        let (grounded, _) = step(&mut case, true, false);
        assert!(!grounded, "a 60° slope (default 45° walkable limit) must never report grounded");
    }
}

#[test]
fn unwalkable_slope_does_not_allow_endless_rejump_while_sliding() {
    // The actual reported symptom: holding jump while continuously falling/sliding along a steep,
    // unwalkable decline must not re-arm jump indefinitely. With single-jump-only
    // (`double_jump_enabled: false`), `can_jump`'s airborne branch is always false, so a jump can
    // only ever fire on a tick where `is_grounded` reads true — bounded to at most a small,
    // fixed number of incidental grounded readings while first settling onto/off the slope, never
    // growing with how long the slide continues. `<= 2`, not an exact count, deliberately: the
    // point being proven is "bounded, not endless", not the precise incidental settling behavior.
    let mut case = setup_case(60.0, 5.94, false, 1);
    let jumps_at_300 = spam_jump(&mut case, 300);
    assert!(jumps_at_300 <= 2, "an unwalkable descent must not let jump re-arm mid-slide; got {jumps_at_300} jumps in 300 ticks");
    let jumps_at_600 = spam_jump(&mut case, 300);
    assert_eq!(
        jumps_at_600, jumps_at_300,
        "continuing to hold jump while still sliding must not add any further jumps beyond \
         whatever incidental settling allowed early on — got {jumps_at_300} at 300 ticks, \
         {jumps_at_600} at 600"
    );
}

#[test]
fn walkable_slope_pogo_cadence_is_unaffected_by_the_slope_limit_check() {
    // The slope-limit check must not regress the already-accepted "bounded pogo" behavior on a
    // genuinely walkable incline (20°, well under the 45° default) — the original uphill-jump-lock
    // fix (grace + liftoff-height) is still what handles this case; slope-limit only matters for
    // terrain steeper than what a player should be able to walk/climb at all.
    let mut case = setup_case(20.0, 5.94, false, 1);
    let jumps = spam_jump(&mut case, 200);
    assert!(jumps > 1, "a walkable 20° slope must still allow repeated jumps, not lock; got {jumps}");
}

#[test]
fn custom_walkable_slope_limit_is_respected() {
    // `max_walkable_slope_deg` is designer-authorable per `MovementConfig` — a project that sets
    // it lower than the 45° default must have that respected: a 30° slope becomes unwalkable
    // (never grounded) once the limit is set to 20°.
    let mut case = setup_case_with_slope_limit(30.0, 5.94, false, 1, 20.0);
    for _ in 0..100 {
        let (grounded, _) = step(&mut case, true, false);
        assert!(!grounded, "a 30° slope must not be grounded when max_walkable_slope_deg is 20°");
    }

    // Positive control: the SAME 30° geometry, at a limit that includes it, must be grounded —
    // without this, the test above can't distinguish "the limit correctly rejected 30°" from
    // "this player is never grounded on this geometry for some unrelated reason" (e.g. it slid
    // off the slab entirely, or the cast is broken).
    let mut walkable_case = setup_case_with_slope_limit(30.0, 5.94, false, 1, 45.0);
    let (grounded, _) = step(&mut walkable_case, false, false);
    assert!(grounded, "the same 30° geometry must be grounded when max_walkable_slope_deg is 45°");
}

#[test]
fn walkable_slope_steeper_than_the_ground_cast_underfoot_tolerance_is_still_grounded() {
    // Regression guard for a real bug caught in review (`planning/claude_suggestions.md`, Physics
    // / Movement section) and never before covered by this file: `ground_cast()`
    // (`capabilities/player.rs`) also gates a hit's floor-candidacy on whether its contact point
    // reads as "underfoot" (`witness1.y <= feet_pos.y + collider_radius * 0.5`), a check needed to
    // stop a solid prop/wall from vetoing a legitimate floor beneath it. That underfoot tolerance
    // alone imposes a hidden `acos(1 - 0.5) = 60°` ceiling on which slopes can ever read as
    // "underfoot" — independent of `max_walkable_slope_deg` — since both the tolerance and a
    // slope's own contact-height offset scale with `collider_radius`. `ground_cast` closes this by
    // also accepting any hit whose *normal* is walkable regardless of underfoot status, but nothing
    // before this test proved that: every other slope-limit test in this file tops out at 30°, well
    // under the ceiling. A 65° slope with the limit explicitly raised to 70° must still be grounded
    // — if this regresses back to "not grounded", the underfoot-only version of the fix (which
    // silently reintroduces this exact bug class on any project authoring a walkable slope steeper
    // than 60°) has been reintroduced.
    let mut case = setup_case_with_slope_limit(65.0, 5.94, false, 1, 70.0);
    for tick in 0..60 {
        let (grounded, _) = step(&mut case, true, false);
        assert!(grounded, "a 65° slope must stay grounded when max_walkable_slope_deg is 70° \
                 (tick {tick}) — the ground cast's underfoot tolerance must not impose its own \
                 hidden slope ceiling");
    }
}

#[test]
fn standing_still_is_grounded_on_flat_ground() {
    // Positive control missing from every other test in this file (all of them exercise jump
    // cadence, which only proves the *negative* case — never that a resting player is correctly
    // grounded at all). Trivial on `Cuboid` ground; see the `TriMesh` sibling below for the
    // geometry family this actually needed proving against.
    let mut case = setup_case(0.0, 5.94, false, 1);
    let (grounded, _) = step(&mut case, false, false);
    assert!(grounded, "a player standing still on flat ground must be grounded");
}

#[test]
fn standing_still_is_grounded_on_flat_trimesh_terrain() {
    // The critical regression test two independent post-implementation reviews demanded: this
    // project's real terrain collider is a zero-thickness `TriMesh`
    // (`capabilities/terrain.rs`'s `ComputedColliderShape::TriMesh(TriMeshFlags::default())`),
    // not the solid `Collider::cuboid` every other test in this file uses. A feet-embedded
    // shape-cast against a zero-thickness surface resolves an arbitrary near-horizontal normal
    // regardless of the triangle's true slope — misclassifying flat terrain itself as unwalkable,
    // which would make jumping impossible on every terrain-based project (`quick_scene`,
    // `primitive_world`, `local_coop_demo`). Fixed by lifting the shape-cast's origin above the
    // surface before sweeping down (see `player_movement_system`'s `GROUND_CAST_SKIN`). This test
    // exists specifically to keep that fix from regressing — it would have failed against the
    // version of this fix that cast from the bare feet position.
    let mut case = setup_case_trimesh(0.0, 5.94);
    let (grounded, _) = step(&mut case, false, false);
    assert!(grounded, "a player standing still on flat TriMesh terrain must be grounded");
}

#[test]
fn walkable_trimesh_slope_is_grounded_and_repeated_jumps_work() {
    // Same geometry family as the flat-terrain test above, but sloped — a walkable (20°, under
    // the 45° default) TriMesh incline must behave identically to the equivalent `Cuboid` case
    // (`walkable_slope_pogo_cadence_is_unaffected_by_the_slope_limit_check`): grounded while
    // resting/climbing, and repeated jumps still possible.
    let mut case = setup_case_trimesh(20.0, 5.94);
    let (grounded, _) = step(&mut case, true, false);
    assert!(grounded, "a walkable 20° TriMesh slope must be grounded");
    let jumps = spam_jump(&mut case, 200);
    assert!(jumps > 1, "a walkable 20° TriMesh slope must still allow repeated jumps; got {jumps}");
}

#[test]
fn unwalkable_trimesh_slope_never_reports_grounded() {
    // The `TriMesh` counterpart of `unwalkable_slope_never_reports_grounded` — real terrain can
    // absolutely include cliff faces/unwalkably steep sections, and those must be correctly
    // classified as unwalkable on the engine's actual terrain collider type, not just on a solid
    // test box.
    let mut case = setup_case_trimesh(60.0, 5.94);
    for _ in 0..100 {
        let (grounded, _) = step(&mut case, true, false);
        assert!(!grounded, "a 60° TriMesh slope (default 45° walkable limit) must never report grounded");
    }
}

#[test]
fn flat_ground_repeated_jumps_still_work() {
    // Control: this must have worked before the fix and must still work identically after —
    // the fix must not change flat-ground jump cadence.
    let mut case = setup_case(0.0, 5.94, false, 1);
    let jumps = spam_jump(&mut case, 200);
    assert!(jumps >= 2, "expected multiple jumps on flat ground over 200 ticks (~3.1s), got {jumps}");
    // Real flight time at these defaults is ~1.2s (~77 ticks) — jumps should be gated by actually
    // landing, not by the ~0.26s grace window alone (that would be the "hover exploit" both
    // reviews flagged). Assert we're nowhere near grace-window cadence (~17 ticks/jump ≈ 11 jumps
    // in 200 ticks).
    assert!(jumps <= 4, "flat-ground cadence should track real flight time, not the grace window; got {jumps} jumps in 200 ticks");
}

#[test]
fn steep_slope_jump_no_longer_locks_permanently() {
    // This is the reported bug: previously, `ever_ungrounded` never became true past ~12° at
    // shipped defaults, so `jumps_used` stuck at 1 forever. 20° is well past that threshold.
    let mut case = setup_case(20.0, 5.94, false, 1);
    let jumps = spam_jump(&mut case, 200);
    assert!(jumps > 1, "jump must be usable again after landing on a steep slope, not just once; got {jumps}");
}

#[test]
fn steep_slope_rejump_cadence_is_bounded_not_a_hover_exploit() {
    // On a slope steep enough that the ground sensor never truthfully reports "ungrounded", the
    // reset is gated purely by the grace window expiring — a real, bounded "pogo" cadence, not a
    // permanent lock (the bug) and not an unbounded/faster-than-physically-plausible hover.
    let mut case = setup_case(20.0, 5.94, false, 1);
    let ticks = 320;
    let jumps = spam_jump(&mut case, ticks);
    assert!(jumps > 1, "must not be locked (got {jumps})");
    // Grace at these defaults is ~17 ticks (~0.26s) — cadence can't be faster than one jump per
    // grace window. Allow generous headroom (one jump per 10 ticks) rather than asserting the
    // exact derived constant, so this doesn't become a change-detector for tuning JUMP_AIR_GRACE_SAFETY.
    let max_plausible_jumps = ticks / 10;
    assert!(
        jumps as usize <= max_plausible_jumps,
        "re-jump cadence while holding jump on a slope must be bounded, not runaway; got {jumps} jumps in {ticks} ticks (max plausible {max_plausible_jumps})"
    );
}

#[test]
fn grace_expiry_does_not_reset_early_when_real_physics_time_lags_ticks() {
    // Regression guard for a real issue caught in post-implementation review: `jump_air_grace`
    // is counted in `FixedUpdate` ticks, but in production Rapier's own physics stepping runs on
    // a *separate*, framerate-coupled clock (`TimestepMode::Variable` in `PostUpdate`) — the two
    // aren't guaranteed to advance in lockstep. A low real framerate (or one clamped
    // `Time<Virtual>::max_delta` hitch) can mean real *physics* time has advanced far less than
    // the tick count assumes when grace expires, which — if tick-counting were the *only* gate —
    // would let a flat-ground jump's `jumps_used` reset while the body is still genuinely rising.
    //
    // physics_dt = 1/256s (4x slower than the 1/64s `jump_air_grace_ticks()` assumes) simulates
    // exactly that mismatch: grace still expires after the same *tick* count, but only 1/4 as much
    // real physics time has actually elapsed. The velocity/liftoff-height backstops must still
    // block a premature reset.
    let mut case = setup_case_full(0.0, 5.94, false, 1, 1.0 / 256.0, 45.0, GroundKind::Cuboid);
    // Fire exactly one jump, then stop pressing it — isolates one clean ballistic arc rather than
    // a continuous spam pattern.
    for _ in 0..20 { step(&mut case, true, false); }
    let (_, used_after_jump) = step(&mut case, true, true);
    assert_eq!(used_after_jump, 1, "jump should fire");

    // At the real 1/64s rate this exact tick count is well past both grace expiry and real
    // detachment; at 1/256s realtime, only a quarter as much physics has actually happened, so
    // the body must still be genuinely ascending (linvel.y > 0) — assert the reset hasn't
    // incorrectly fired while that's true.
    for _ in 0..40 {
        let (_, jumps_used) = step(&mut case, false, false);
        let vy = case.app.world().entity(case.player).get::<Velocity>().unwrap().linvel.y;
        if vy > 0.5 {
            assert_eq!(
                jumps_used, 1,
                "jumps_used must not reset while the body is still genuinely ascending (linvel.y={vy:.2}), \
                 regardless of how many FixedUpdate ticks have elapsed"
            );
        }
    }
}

#[test]
fn flat_ground_low_jump_height_also_locks_and_is_also_fixed() {
    // This bug class isn't slope-specific: any jump whose ballistic apex (v^2 / 2g) can't clear
    // `collider_radius + ground_cast_length` (0.7m at defaults) hits the identical permanent lock
    // on perfectly flat ground. jump_velocity=3.0 -> apex ~0.46m, well under 0.7m.
    let mut case = setup_case(0.0, 3.0, false, 1);
    let jumps = spam_jump(&mut case, 150);
    assert!(jumps > 1, "a low jump height must not permanently lock on flat ground either; got {jumps}");
}

#[test]
fn coyote_time_lets_a_jump_fire_briefly_after_leaving_the_ground() {
    // The debounce's other intended purpose (besides absorbing single-tick sensor noise from
    // uneven terrain — see `capabilities/player.rs`'s `coyote_ticks`/`MovementConfig::
    // coyote_time_secs` doc comments): a jump pressed shortly after leaving solid ground (walking
    // off a ledge) should still fire, the same forgiveness most action/platformer character
    // controllers implement deliberately. Teleport away from the ground (simulating having just
    // left it) and confirm a jump a few ticks later — comfortably inside the default ~6-tick
    // (0.1s @ 64Hz) coyote window — still succeeds.
    let mut case = setup_case(0.0, 5.94, false, 1);
    // Confirm genuinely grounded first — this also seeds `coyote_ticks_remaining` to full
    // (it starts at 0 from construction and is only ever set by a real ground-check tick, which
    // the settle loop inside `setup_case` never runs).
    let (grounded_before, _) = step(&mut case, false, false);
    assert!(grounded_before, "sanity: should start grounded");
    // Pin the seeded value itself, not just that grounding was reported — `coyote_time_secs: 0.1`
    // at the harness's 64Hz tick rate must seed exactly 6 ticks (`(0.1 * 64.0).round()`), not some
    // other value that would happen to still pass the loose "still grounded a few ticks later"
    // checks below.
    let seeded = case.app.world().entity(case.player).get::<CharacterController>().unwrap().coyote_ticks_remaining;
    assert_eq!(seeded, 6, "coyote_time_secs=0.1 @ 64Hz should seed a 6-tick buffer");
    {
        let mut entity = case.app.world_mut().entity_mut(case.player);
        entity.get_mut::<Transform>().unwrap().translation.y = 1.0; // clearly beyond the ~0.71m sensor reach
        entity.get_mut::<Velocity>().unwrap().linvel = Vec3::ZERO;
    }
    case.app.update(); // sync GlobalTransform + Rapier before the first real step() (see the sibling test below for why)

    let (grounded_after_leaving, _) = step(&mut case, false, false);
    assert!(grounded_after_leaving, "coyote buffer should still report grounded immediately after leaving the ground");
    for _ in 0..2 { step(&mut case, false, false); } // still comfortably inside the coyote window
    let (_, jumps_used) = step(&mut case, false, true);
    assert_eq!(jumps_used, 1, "a jump pressed shortly after leaving the ground should still fire via the coyote buffer");
}

#[test]
fn coyote_time_does_not_mask_an_extended_fall_forever() {
    // The debounce must only smooth over brief sensor noise / give a short jump-forgiveness
    // window — it must not permanently hide a real, extended fall, and the window's *length*
    // must actually track `coyote_time_secs` rather than just "eventually gives up somehow": a
    // loose "did it ever go false within N generous ticks" check would still pass if a regression
    // tripled the window (e.g. misreading `coyote_time_secs` as 0.3s instead of 0.1s), silently
    // widening how long a real fall's animation/jump-availability stays masked. Pin the exact
    // 6-tick boundary instead (see the sibling test's `coyote_ticks_remaining` assertion for why
    // 6 is the right number at these defaults).
    let mut case = setup_case(0.0, 5.94, false, 1);
    let (grounded_before, _) = step(&mut case, false, false); // seed coyote_ticks_remaining, see sibling test
    assert!(grounded_before, "sanity: should start grounded");
    {
        let mut entity = case.app.world_mut().entity_mut(case.player);
        entity.get_mut::<Transform>().unwrap().translation.y = 1.0;
        entity.get_mut::<Velocity>().unwrap().linvel = Vec3::ZERO;
    }
    case.app.update();

    for tick in 1..=6 {
        let (grounded, _) = step(&mut case, false, false);
        assert!(grounded, "tick {tick} of 6: coyote buffer should still be masking the fall");
    }
    let (grounded, _) = step(&mut case, false, false);
    assert!(!grounded, "tick 7: the coyote window must have expired by now — a real fall must not stay masked forever");
}

#[test]
fn falling_off_a_ledge_still_plays_landing_animation_without_ever_jumping() {
    // Regression guard for a real issue caught in alignment review: the first version of this
    // fix gated the "jump_exit" animation request behind `jumps_used > 0`, so a plain fall (never
    // having pressed jump at all — jumps_used stays 0 throughout) silently stopped playing the
    // landing clip. The animation request must fire on any genuine airborne->grounded edge,
    // independent of the jumps_used/grace bookkeeping.
    let mut case = setup_case(0.0, 5.94, false, 1);
    // Teleport well above the ground with zero velocity — a fall, not a jump.
    {
        let mut entity = case.app.world_mut().entity_mut(case.player);
        entity.get_mut::<Transform>().unwrap().translation.y = 3.0;
        entity.get_mut::<Velocity>().unwrap().linvel = Vec3::ZERO;
        // `LocomotionState::default()` starts `is_grounded: true` — make the post-teleport intent
        // explicit rather than relying on whatever it happened to hold from settling.
        entity.get_mut::<LocomotionState>().unwrap().is_grounded = false;
    }
    // A direct `Transform` mutation doesn't propagate to `GlobalTransform` (nor sync into
    // Rapier's own body position) until a real `app.update()` runs that pass — `player_movement_
    // system` reads `GlobalTransform`, not `Transform`, for `feet_pos`. Without this, the very
    // first `step()` below would read the *pre-teleport* (still resting on the ground) position,
    // immediately reporting grounded and ending the test before the fall ever happens. `step()`
    // itself can't be used here since it also runs `player_movement_system`, which is exactly
    // what must NOT see the stale position.
    case.app.update();
    drain_animation_requests(&mut case); // clear anything queued by the teleport-adjacent tick

    let mut saw_jump_exit = false;
    let mut landed = false;
    for _ in 0..200 {
        let (grounded, jumps_used) = step(&mut case, false, false);
        assert_eq!(jumps_used, 0, "never pressed jump; jumps_used must stay 0 throughout the fall");
        if drain_animation_requests(&mut case).iter().any(|r| r == "jump_exit") {
            saw_jump_exit = true;
        }
        if grounded { landed = true; break; }
    }
    assert!(landed, "sanity: player must eventually land");
    assert!(saw_jump_exit, "landing from a plain fall (jumps_used == 0 throughout) must still fire the jump_exit animation request");
}

#[test]
fn jumps_used_resets_promptly_after_a_genuine_flat_ground_landing() {
    // Pins the core design property directly: the reset is a level check re-evaluated every
    // tick (`loco.is_grounded && jumps_used > 0 && (...)`), not a `!was_grounded && is_grounded`
    // edge — so once a real landing happens, `jumps_used` must go back to 0 within a tick or two
    // of touchdown, not linger. Every other test in this file only asserts "more than one jump
    // eventually happened", which is also true of a much slower/buggier reset; this test checks
    // the promptness that property depends on.
    let mut case = setup_case(0.0, 5.94, false, 1);
    for _ in 0..20 { step(&mut case, true, false); }
    let (_, used) = step(&mut case, true, true);
    assert_eq!(used, 1, "jump should fire");

    // Wait for a genuine liftoff first (grounded must actually go false at least once) — right
    // after firing, the sensor still reads grounded=true for several ticks (real detach takes
    // ~9 ticks at these defaults), and that residual reading must not be mistaken for a landing.
    let mut left_ground = false;
    let mut landed_tick = None;
    for i in 0..200 {
        let (grounded, _) = step(&mut case, true, false);
        if !grounded { left_ground = true; }
        if left_ground && grounded { landed_tick = Some(i); break; }
    }
    assert!(left_ground, "sanity: player must genuinely leave the ground before landing again");
    let landed_tick = landed_tick.expect("sanity: player must eventually land");

    // The tick landing was observed already ran `player_movement_system` for that tick (see
    // `step()`), so the reset — being a level check, not an edge — must have already applied
    // within that same call once `is_grounded` first read true.
    let used_at_landing = case.app.world().entity(case.player).get::<CharacterController>().unwrap().jumps_used;
    assert_eq!(
        used_at_landing, 0,
        "jumps_used must reset in the same tick real grounded contact is (re-)detected (landed at tick {landed_tick}), not lag behind it"
    );
}

#[test]
fn double_jump_still_requires_genuine_airborne_height_on_flat_ground() {
    // Regression guard for a real issue caught in plan review: the rejected v1 approach (forcing
    // `is_grounded = false` for a fixed window) would have let a fast double-tap consume the
    // second jump at ground level. The jump-lock fix itself (grace/velocity/liftoff-height) never
    // touches `is_grounded` or `can_jump`'s branch selection at all. Coyote-time (added later)
    // does touch both, but only for the `jumps_used == 0` branch — the `jumps_used > 0` (double
    // jump) branch this test exercises still reads `raw_grounded` alone, unaffected by the coyote
    // buffer (see `can_jump`'s own comment in `capabilities/player.rs`) — so behavior here must
    // still be identical to before either fix existed.
    let mut case = setup_case(0.0, 5.94, true, 2);

    // First jump at tick 20.
    for _ in 0..20 { step(&mut case, true, false); }
    let (_, used_after_first) = step(&mut case, true, true);
    assert_eq!(used_after_first, 1, "first jump should fire");

    // Immediate double-tap the very next tick, while still within real ground-detach time
    // (~9 ticks at these defaults) — the real shape-cast still reports grounded here.
    let (grounded_next_tick, used_after_immediate_retap) = step(&mut case, true, true);
    assert!(grounded_next_tick, "sanity: one tick after takeoff the real ground-check still reports grounded at these defaults");
    assert_eq!(
        used_after_immediate_retap, 1,
        "double jump must not be consumable at ground level immediately after the first jump — \
         only once genuinely airborne, exactly as before this fix"
    );

    // Advance until genuinely airborne (real detach), then confirm double jump does work.
    let mut became_airborne = false;
    for _ in 0..40 {
        let (grounded, _) = step(&mut case, true, false);
        if !grounded { became_airborne = true; break; }
    }
    assert!(became_airborne, "sanity: player should genuinely leave the ground on flat terrain");
    let (_, used_after_real_double_jump) = step(&mut case, true, true);
    assert_eq!(used_after_real_double_jump, 2, "double jump should fire once genuinely airborne");
}

#[test]
fn double_jump_fires_even_while_the_coyote_buffer_still_reports_grounded() {
    // Regression guard for a real bug found independently by all three post-implementation
    // reviews of the coyote-time addition: the first version gated `can_jump`'s grounded branch
    // on the fully coyote-buffered `is_grounded`, with no `jumps_used == 0` qualifier. Since the
    // two branches are mutually exclusive, that made *neither* branch reachable for the entire
    // coyote window after a real ground jump (`jumps_used == 1` there — the grounded branch's own
    // `jumps_used == 0` check blocks it, and the airborne branch required `!is_grounded`, which
    // the buffer was still refusing to report) — silently swallowing a double-jump press for the
    // whole buffer duration. Fixed by qualifying the grounded branch's coyote-forgiveness on
    // `jumps_used == 0`, so for `jumps_used > 0` the branch choice depends purely on real
    // detachment (`raw_grounded`), never on the buffer. See `capabilities/player.rs`'s `can_jump`.
    let mut case = setup_case(0.0, 5.94, true, 2);

    for _ in 0..20 { step(&mut case, true, false); }
    let (_, used_after_first) = step(&mut case, true, true);
    assert_eq!(used_after_first, 1, "first jump should fire");

    // Spam jump every tick until the second jump fires, capturing whether the buffered
    // `is_grounded` was still reporting true on the exact tick it did.
    let mut fired = None;
    for _ in 1..=20 {
        let (grounded, jumps_used) = step(&mut case, true, true);
        if jumps_used == 2 {
            fired = Some(grounded);
            break;
        }
    }
    assert_eq!(
        fired,
        Some(true),
        "the second jump should fire on the very first tick of real detachment, while the coyote \
         buffer is still reporting grounded true (masking single-tick sensor noise for animation \
         purposes) — proving double-jump availability is governed by real detachment, not by \
         waiting for the buffer to expire"
    );
}
