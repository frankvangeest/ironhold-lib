// TEMPORARY investigation harness for the "uphill jump lock" backlog bug.
// Not intended to be committed as-is.
use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy_rapier3d::prelude::*;
use ironhold_core::runtime::{InputAction, InputActionMessage};
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

struct Report {
    ever_ungrounded_after_jump: bool,
    landing_resets: u32,
    jumps_taken: u32,
    jumps_used_final: u8,
    max_gap: f32,
    final_x: f32,
}

/// Steps a real Rapier world: a slope of `angle_deg` (rising toward +X) with a player
/// capsule identical to `spawn_player_entity_core`'s, running uphill and spamming jump.
fn run_case(angle_deg: f32, running: bool, jump_ticks: usize, verbose: bool) -> Report {
    run_case_ex(angle_deg, running, jump_ticks, verbose, 0.3, None)
}

fn run_case_ex(
    angle_deg: f32,
    running: bool,
    jump_ticks: usize,
    verbose: bool,
    ground_cast_length: f32,
    stop_moving_after: Option<usize>,
) -> Report {
    let mut app = setup_test_app();
    // Deterministic physics: one 1/64s step per app.update().
    app.insert_resource(TimestepMode::Fixed { dt: 1.0 / 64.0, substeps: 1 });
    // Stop FixedUpdate from also running player_movement_system on its own (real-time
    // accumulator would make the tick count nondeterministic); we drive it manually.
    app.insert_resource(Time::<Fixed>::from_seconds(1000.0));
    app.update();

    let theta = angle_deg.to_radians();
    // Cuboid whose top face is the plane y = x*tan(theta) through the world origin.
    app.world_mut().spawn((
        RigidBody::Fixed,
        Collider::cuboid(60.0, 0.25, 60.0),
        Transform::from_translation(Vec3::new(0.25 * theta.sin(), -0.25 * theta.cos(), 0.0))
            .with_rotation(Quat::from_rotation_z(theta)),
    ));

    // Player: same collider/damping/friction/controller values as spawn_player_entity_core
    // with MovementConfig defaults (height 1.8, radius 0.4 -> cap_half 0.5, offset 0.9).
    let player = app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.02, 0.0),
        CharacterController {
            walk_speed: 5.0,
            run_speed: 10.0,
            rot_speed: 3.0,
            inputs: input_map(),
            is_running: running,
            jump_velocity: 5.94,
            double_jump_enabled: false,
            double_jump_velocity: 5.94,
            jumps_used: 0,
            max_jumps: 1,
            collider_radius: 0.4,
            ground_cast_length,
            idle_drag: 0.8,
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

    let mut rep = Report {
        ever_ungrounded_after_jump: false,
        landing_resets: 0,
        jumps_taken: 0,
        jumps_used_final: 0,
        max_gap: 0.0,
        final_x: 0.0,
    };
    let mut prev_used: u8 = 0;
    let mut jumped_yet = false;

    // Run uphill (+X = transform.right()). Send Jump on exactly ONE tick (t=20), then never
    // again: `jumps_used` can then only ever go 1 -> 0, which is unambiguously the landing-edge
    // reset. A second single Jump is sent at the very end to see if jumping is possible again.
    let last_tick = 20 + jump_ticks;
    for tick in 0..=last_tick {
        let jumping = tick >= 20; // spam jump, as in the bug report
        {
            let moving = stop_moving_after.map_or(true, |stop| tick < stop);
            let mut msgs = app.world_mut().resource_mut::<Messages<InputActionMessage>>();
            // Nothing clears this buffer (init_resource, not add_message), so clear it by hand
            // to get exact per-tick input control.
            msgs.clear();
            if moving {
                msgs.write(InputActionMessage { entity: player, action: InputAction::Move(Vec2::new(1.0, 0.0)) });
            }
            if jumping {
                msgs.write(InputActionMessage { entity: player, action: InputAction::Jump(true) });
            }
        }
        app.world_mut().run_system_once(player_movement_system).unwrap();
        // Count actual jump firings: a landing reset + re-jump can both happen inside one
        // system run, so diffing `jumps_used` across ticks under-counts.
        let fired = app.world()
            .resource::<Messages<ironhold_core::runtime::GameEvent>>()
            .iter_current_update_messages()
            .filter(|e| matches!(e, ironhold_core::runtime::GameEvent::Trigger(n) if n == "player.jumped"))
            .count() as u32;
        app.update();

        let used = app.world().entity(player).get::<CharacterController>().unwrap().jumps_used;
        let grounded = app.world().entity(player).get::<LocomotionState>().unwrap().is_grounded;
        let t = app.world().entity(player).get::<Transform>().unwrap().translation;
        let gap = t.y - t.x * theta.tan();

        // `fired` is cumulative (setup_test_app registers Messages<T> via init_resource, so no
        // message_update_system ever clears the buffer) — take the latest value, don't sum.
        rep.jumps_taken = fired;
        if used > prev_used { jumped_yet = true; }
        if used < prev_used { rep.landing_resets += 1; }
        prev_used = used;
        if jumped_yet {
            if !grounded { rep.ever_ungrounded_after_jump = true; }
            if gap > rep.max_gap { rep.max_gap = gap; }
        }
        if verbose && tick >= 18 {
            println!(
                "  t{:>3} grounded={:<5} used={} fired={} x={:>7.3} y={:>7.3} gap={:>6.3} vy={:>7.3}",
                tick, grounded, used, fired, t.x, t.y, gap,
                app.world().entity(player).get::<Velocity>().unwrap().linvel.y,
            );
        }
        rep.final_x = t.x;
    }
    rep.jumps_used_final = prev_used;
    rep
}

fn report(label: &str, r: &Report) {
    println!(
        "{label}: jumps_taken={} landing_resets={} ever_ungrounded={} max_gap={:.3} final_x={:.2} jumps_used_final={}",
        r.jumps_taken, r.landing_resets, r.ever_ungrounded_after_jump, r.max_gap, r.final_x, r.jumps_used_final
    );
}

#[test]
fn slope_jump_matrix() {
    // Control: flat ground must produce repeated jumps (grounded -> false -> landing reset).
    let flat = run_case(0.0, true, 160, false);
    report("flat  0deg run ", &flat);

    for angle in [5.0f32, 10.0, 12.0, 15.0, 18.0, 20.0, 25.0, 30.0] {
        let r = run_case(angle, true, 160, false);
        report(&format!("slope {angle:>2}deg run "), &r);
    }
    for angle in [15.0f32, 20.0, 25.0, 30.0] {
        let r = run_case(angle, false, 160, false);
        report(&format!("slope {angle:>2}deg walk"), &r);
    }

    println!("--- can `ground_cast_length` tune the lock away? (probe depth = radius 0.4 + gcl) ---");
    for gcl in [0.0f32, 0.05, 0.3] {
        for angle in [12.0f32, 15.0, 20.0, 25.0] {
            let r = run_case_ex(angle, true, 160, false, gcl, None);
            report(&format!("slope {angle:>2}deg run  gcl={gcl:.2}"), &r);
        }
    }

    println!("--- does the lock persist after the player stops climbing at t=60? ---");
    for angle in [15.0f32, 20.0, 25.0] {
        let r = run_case_ex(angle, true, 300, false, 0.3, Some(60));
        report(&format!("slope {angle:>2}deg run, stop@60"), &r);
    }

    assert!(flat.jumps_taken > 1, "control case must re-jump on flat ground");
}

#[test]
fn slope_jump_trace_20deg() {
    println!("--- 20deg running uphill, per-tick trace ---");
    let r = run_case(20.0, true, 160, true);
    report("slope 20deg run ", &r);
}

#[test]
fn probe_depth_vs_cast_length() {
    // At what gap above flat ground does is_grounded flip, for each ground_cast_length?
    // radius = 0.4 in all cases.
    for gcl in [0.05f32, 0.1, 0.3, 0.6] {
        println!("--- gcl={gcl} (radius 0.4) ---");
        let r = run_case_ex(0.0, true, 12, true, gcl, None);
        report(&format!("flat gcl={gcl:.2}"), &r);
    }
}

#[test]
fn slope_jump_trace_flat() {
    println!("--- flat control, per-tick trace ---");
    let r = run_case(0.0, true, 160, true);
    report("flat 0deg run ", &r);
}
