//! Regression coverage for the wall-friction velocity-crush bug — see `planning/backlog.md`'s
//! former "Moving into a wall while airborne crushes vertical velocity via Coulomb friction" entry
//! for the full root-cause writeup (verified against vendored `rapier3d` source): a wall contact's
//! friction constraint spans its full tangent plane, which in 3D includes vertical — and since
//! `player_movement_system` re-writes `velocity.linvel.x/z` every tick (an impulse-sized command,
//! not a force), the resulting friction impulse each physics step is proportional to the player's
//! commanded approach speed, independent of mass or frame rate. At shipped defaults this cost
//! ~83% of jump height when a jump was held into a wall, and the same mechanism let a merely
//! *falling* player "hang" against a wall at ~1/5 free-fall rate just by holding movement into it
//! — no jump required.
//!
//! Fixed by making the player's `Friction` coefficient (`PLAYER_IDLE_FRICTION`,
//! `capabilities/player.rs`) conditional: `0.0` while moving or airborne, the real coefficient only
//! while grounded and idle — see that constant's doc comment for the full rationale. Every NPC
//! spawn site already used `0.0` unconditionally and was never affected.
//!
//! Style follows `prop_ground_veto_tests.rs`/`player_slope_jump_tests.rs`: real Rapier physics,
//! `player_movement_system` driven directly via `run_system_once`, one `step()` == one
//! `FixedUpdate` tick. Comparisons are against a same-input open-field control rather than
//! hardcoded numbers, so these tests don't depend on the exact jump-velocity/gravity constants
//! staying at their current values.
use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy_rapier3d::prelude::*;
use ironhold_core::runtime::{InputAction, InputActionMessage};
use ironhold_core::capabilities::player::{CharacterController, SpeedMultiplier, player_movement_system, PLAYER_IDLE_FRICTION};
use ironhold_core::capabilities::animation_resolver::{LocomotionState, AnimationRequests};
use ironhold_core::schema::player::InputMap;

mod support;
use support::setup_test_app;

fn input_map() -> InputMap {
    InputMap {
        forward: "KeyW".to_string(), backward: "KeyS".to_string(), left: "KeyA".to_string(), right: "KeyD".to_string(),
        strafe_left: "KeyQ".to_string(), strafe_right: "KeyE".to_string(), jump: "Space".to_string(), run: "ShiftLeft".to_string(),
        interact: "KeyF".to_string(), strafe_mouse_button: Some("Left".to_string()), target_next: "Tab".to_string(), target_range: 30.0,
        gamepad_index: None, look_left: None, look_right: None, look_up: None, look_down: None,
        gamepad_jump: "South".to_string(), gamepad_run: "East".to_string(), gamepad_interact: "West".to_string(),
        gamepad_target_next: "North".to_string(), gamepad_deadzone: 0.15,
    }
}

struct Case {
    app: App,
    player: Entity,
}

fn character_controller(jump_velocity: f32, is_running: bool) -> CharacterController {
    CharacterController {
        walk_speed: 5.0,
        run_speed: 10.0,
        rot_speed: 3.0,
        inputs: input_map(),
        is_running,
        jump_velocity,
        double_jump_enabled: false,
        double_jump_velocity: jump_velocity,
        jumps_used: 0,
        max_jumps: 1,
        collider_radius: 0.4,
        ground_cast_length: 0.3,
        max_walkable_slope_deg: 45.0,
        coyote_time_secs: 0.1,
        coyote_ticks_remaining: 0,
        idle_drag: 0.8,
        jump_air_grace: 0,
        jump_liftoff_y: None,
    }
}

fn spawn_player(world: &mut World, start: Vec3, controller: CharacterController) -> Entity {
    world.spawn((
        Name::new("Player"),
        Transform::from_translation(start),
        controller,
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
        Friction { coefficient: PLAYER_IDLE_FRICTION, combine_rule: CoefficientCombineRule::Min },
    )).id()
}

/// Flat solid ground whose top face is exactly y = 0 (a `Collider::cuboid`, not real terrain).
fn spawn_flat_ground(world: &mut World) {
    world.spawn((Name::new("ground"), RigidBody::Fixed, Collider::cuboid(200.0, 0.25, 200.0), Transform::from_xyz(0.0, -0.25, 0.0)));
}

/// The `TriMesh` counterpart of `spawn_flat_ground` — required by `tests/CLAUDE.md`'s "TriMesh vs
/// Cuboid ground testing" rule for any test exercising `player_movement_system`'s ground-detection
/// shape-cast, even though this file's bug is a solver-friction issue unrelated to that cast.
fn spawn_flat_trimesh_ground(world: &mut World) {
    let s = 400.0;
    let vertices = vec![
        Vec3::new(-s, 0.0, -s), Vec3::new(s, 0.0, -s), Vec3::new(s, 0.0, s), Vec3::new(-s, 0.0, s),
    ];
    let indices = vec![[0u32, 1, 2], [0, 2, 3]];
    world.spawn((Name::new("ground"), RigidBody::Fixed, Collider::trimesh(vertices, indices).expect("valid trimesh"), Transform::IDENTITY));
}

/// A wall tall enough (spans well beyond any apex/fall-distance this file's tests reach) that
/// contact with its *top edge* — the separately-tracked "mid-air spurious grounding" bug — can
/// never confound these tests, which are purely about the friction mechanism at a wall's *side*.
fn spawn_tall_wall(world: &mut World, x: f32) {
    world.spawn((Name::new("wall"), RigidBody::Fixed, Collider::cuboid(0.6, 10.0, 0.6), Transform::from_xyz(x, 0.0, 0.0)));
}

/// `ground`, optionally a tall wall at `wall_x`, and a player spawned at `start` — touching the
/// wall's near face when `wall_x` and `start.x` are chosen accordingly (every test below uses
/// `start.x = 1.0`, `wall_x = 2.0`: wall half-extent 0.6 → near face at 1.4; player radius 0.4 →
/// flush contact at 1.4 - 0.4 = 1.0).
fn setup(ground: fn(&mut World), wall_x: Option<f32>, start: Vec3, jump_velocity: f32, is_running: bool) -> Case {
    let mut app = setup_test_app();
    app.insert_resource(TimestepMode::Fixed { dt: 1.0 / 64.0, substeps: 1 });
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::ZERO));
    app.update();

    ground(app.world_mut());
    if let Some(x) = wall_x {
        spawn_tall_wall(app.world_mut(), x);
    }
    let player = spawn_player(app.world_mut(), start, character_controller(jump_velocity, is_running));

    for _ in 0..40 { app.update(); }
    Case { app, player }
}

/// One `FixedUpdate` tick. Sends `Move(forward)` (toward +X) unless `moving` is false, and `Jump`
/// iff `jumping` (pass `tick == 0` from the caller's loop for a single press). Returns
/// `(x, y, velocity.y)`.
fn step(case: &mut Case, moving: bool, jumping: bool) -> (f32, f32, f32) {
    {
        let player = case.player;
        let mut msgs = case.app.world_mut().resource_mut::<Messages<InputActionMessage>>();
        msgs.clear();
        if moving {
            msgs.write(InputActionMessage { entity: player, action: InputAction::Move(Vec2::new(1.0, 0.0)) });
        }
        if jumping {
            msgs.write(InputActionMessage { entity: player, action: InputAction::Jump(true) });
        }
    }
    case.app.world_mut().run_system_once(player_movement_system).unwrap();
    case.app.update();
    let t = case.app.world().entity(case.player).get::<Transform>().unwrap();
    let v = case.app.world().entity(case.player).get::<Velocity>().unwrap();
    (t.translation.x, t.translation.y, v.linvel.y)
}

/// Shared body for the "jump next to a wall must reach near the unobstructed apex" family —
/// compares a wall-pressed jump against an identical-input open-field control, so the assertion
/// doesn't depend on the exact `jump_velocity`/gravity constants.
fn assert_jump_reaches_near_full_height(ground: fn(&mut World), is_running: bool, label: &str) {
    let jump_velocity = 6.0;
    let start = Vec3::new(1.0, 0.02, 0.0);

    let mut control = setup(ground, None, start, jump_velocity, is_running);
    let mut control_apex = start.y;
    for tick in 0..150 {
        let (_, y, _) = step(&mut control, true, tick == 0);
        control_apex = control_apex.max(y);
    }
    assert!(control_apex > 1.0, "[{label}] sanity: unobstructed jump must reach a real height, got {control_apex:.3}");

    let mut walled = setup(ground, Some(2.0), start, jump_velocity, is_running);
    let mut walled_apex = start.y;
    for tick in 0..150 {
        let (_, y, _) = step(&mut walled, true, tick == 0);
        walled_apex = walled_apex.max(y);
    }

    assert!(
        walled_apex >= control_apex * 0.9,
        "[{label}] jump next to a wall (held Move into it) must reach close to the unobstructed \
         apex: control={control_apex:.3} walled={walled_apex:.3} (ratio {:.2}) — a ratio well \
         below 1.0 means wall friction is crushing vertical velocity again",
        walled_apex / control_apex,
    );
}

#[test]
fn jump_reaches_near_full_height_when_moving_into_a_wall_at_walk_speed() {
    assert_jump_reaches_near_full_height(spawn_flat_ground, false, "walk_speed");
}

#[test]
fn jump_reaches_near_full_height_when_moving_into_a_wall_at_run_speed() {
    // The failure scales with commanded speed (`Δv_y per step ∝ μ × approach_speed`) — a
    // walk-speed-only test would understate it roughly 2x versus this project's `run_speed`.
    assert_jump_reaches_near_full_height(spawn_flat_ground, true, "run_speed");
}

/// TriMesh-ground sibling of the walk-speed test above, per `tests/CLAUDE.md`'s ground-testing
/// rule — the fix touches `player_movement_system`, which also exercises the ground shape-cast,
/// even though this specific bug is unrelated to ground-cast normal detection.
#[test]
fn jump_reaches_near_full_height_when_moving_into_a_wall_on_trimesh_terrain() {
    assert_jump_reaches_near_full_height(spawn_flat_trimesh_ground, false, "trimesh");
}

/// The more alarming, no-jump-required shape of the same bug: a falling player holding movement
/// into a wall must still descend at essentially the same rate as an identical open-field control
/// — before this fix, wall friction alone (no jump, no ground contact at all) slowed the fall to
/// ~1/5 of free-fall rate.
#[test]
fn falling_against_a_wall_with_move_held_still_descends_at_free_fall_rate() {
    let start = Vec3::new(1.0, 5.0, 0.0);
    let jump_velocity = 6.0; // unused — no Jump input is ever sent in this test

    let mut control = setup(spawn_flat_ground, None, start, jump_velocity, false);
    let mut walled = setup(spawn_flat_ground, Some(2.0), start, jump_velocity, false);

    for tick in 0..80 {
        let (_, control_y, control_vy) = step(&mut control, true, false);
        let (_, walled_y, walled_vy) = step(&mut walled, true, false);
        assert!(
            (walled_vy - control_vy).abs() < 0.5,
            "falling against a wall with Move held must descend at ~free-fall rate, matching the \
             open-field control: tick={tick} control_vy={control_vy:.3} walled_vy={walled_vy:.3} \
             (control_y={control_y:.3} walled_y={walled_y:.3})"
        );
    }
}

/// A 20° slope (well within the 45° default `max_walkable_slope_deg`) with a player settled onto
/// it, for the idle-creep test below.
fn setup_slope_case() -> Case {
    let mut app = setup_test_app();
    app.insert_resource(TimestepMode::Fixed { dt: 1.0 / 64.0, substeps: 1 });
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::ZERO));
    app.update();

    let theta = 20f32.to_radians();
    let ground_transform = Transform::from_translation(Vec3::new(0.25 * theta.sin(), -0.25 * theta.cos(), 0.0))
        .with_rotation(Quat::from_rotation_z(theta));
    app.world_mut().spawn((RigidBody::Fixed, Collider::cuboid(400.0, 0.25, 400.0), ground_transform));

    let player = spawn_player(app.world_mut(), Vec3::new(0.0, 0.02, 0.0), character_controller(6.0, false));
    for _ in 0..40 { app.update(); }
    Case { app, player }
}

fn friction_coefficient(case: &Case) -> f32 {
    case.app.world().entity(case.player).get::<Friction>().unwrap().coefficient
}

/// A player spawned elevated (y = 5) over flat ground it can't reach (`ground_sensor_reach` ≈
/// 0.7m, far short of a 5m drop) — `raw_grounded` is `false` from the very first tick, no settling
/// needed. Used by the state-table test below for the airborne rows.
fn setup_airborne_case() -> Case {
    setup(spawn_flat_ground, None, Vec3::new(0.0, 5.0, 0.0), 6.0, false)
}

/// Direct contract test for `PLAYER_IDLE_FRICTION`'s gating condition, covering all four
/// `(raw_grounded, loco.moving)` combinations by directly reading `Friction.coefficient` after one
/// `step()` — not an indirect physical-drift comparison. A prior draft of this test compared
/// downhill drift against a "broken" counterfactual that also skipped `player_movement_system`
/// entirely, which confounded the friction coefficient with `idle_drag` (`velocity.linvel.xz *=
/// idle_drag` also lives in the branch that skip removed) — `idle_drag` dominates slope creep far
/// more than `μ = 0.15` does, so that comparison would not actually have caught the fix being
/// deleted (verified analytically: `drift_with_friction / drift_without = 1 - μ/tan(θ)` is
/// independent of `idle_drag`/damping, giving ~0.59 at 20°, comfortably under any threshold that
/// comparison used). This version tests the actual contract instead, and specifically exercises
/// the `raw_grounded` half of the gate (grounded+idle vs. airborne+idle), which the other four
/// tests in this file never distinguish — every one of them either holds Move the whole time
/// (`moving == true` forces `0.0` regardless of `raw_grounded`) or stays grounded throughout, so a
/// future "simplification" to just `!loco.moving` would pass all of them silently.
#[test]
fn friction_coefficient_matches_the_grounded_and_idle_state_table() {
    // Sanity: without this, the grounded-and-idle assertion below is self-referential — if
    // `PLAYER_IDLE_FRICTION` were ever accidentally set to `0.0` (e.g. "simplifying" the gate by
    // just always zeroing it), `friction_coefficient(&grounded_idle) == PLAYER_IDLE_FRICTION`
    // would still pass (`0.0 == 0.0`), silently reintroducing the original downhill-creep bug
    // this coefficient exists to prevent, with nothing in this file to catch it (debug-detective
    // review finding).
    assert!(PLAYER_IDLE_FRICTION > 0.0, "PLAYER_IDLE_FRICTION must be meaningfully nonzero");

    let mut grounded_idle = setup_slope_case();
    step(&mut grounded_idle, false, false);
    assert_eq!(
        friction_coefficient(&grounded_idle), PLAYER_IDLE_FRICTION,
        "grounded and idle must carry the real coefficient"
    );

    let mut grounded_moving = setup_slope_case();
    step(&mut grounded_moving, true, false);
    assert_eq!(
        friction_coefficient(&grounded_moving), 0.0,
        "grounded but moving must be frictionless (a jump could fire from here next to a wall)"
    );

    let mut airborne_idle = setup_airborne_case();
    step(&mut airborne_idle, false, false);
    assert_eq!(
        friction_coefficient(&airborne_idle), 0.0,
        "airborne and idle must be frictionless (the falling-against-a-wall case needs no Move \
         input to reach a wall the player already drifted into)"
    );

    let mut airborne_moving = setup_airborne_case();
    step(&mut airborne_moving, true, false);
    assert_eq!(
        friction_coefficient(&airborne_moving), 0.0,
        "airborne and moving must be frictionless — this is the direct jump-into-a-wall repro"
    );
}
