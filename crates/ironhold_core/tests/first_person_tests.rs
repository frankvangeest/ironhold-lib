//! `planning/features/first_person_starter.md` v1: the `controls: FirstPerson` scheme as seen by
//! `input_translator_system`, the run-key hold mode, and stick + keyboard look in
//! `first_person_camera_system` (including who owns the right stick, and the pause gate).
//!
//! The raw-RON resolution of `controls:` (authored > scheme > engine default) is tested in
//! `ron_validation.rs`; this file tests what the systems do with the resolved `InputMap`.

use bevy::ecs::system::RunSystemOnce;
use bevy::input::gamepad::GamepadAxis;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

use ironhold_core::capabilities::camera::{
    ActiveCameraMode, CameraTargets, FirstPersonCameraMode, FirstPersonState,
};
use ironhold_core::capabilities::player::{BoundGamepad, CharacterController};
use ironhold_core::runtime::{
    input_translator_system, ActionQueue, InputAction, InputActionMessage,
};
use ironhold_core::schema::player::InputMap;
use ironhold_core::schema::Action;

mod support;
use support::{connect_test_gamepad, set_gamepad_axis, setup_test_app};

/// Parses an `inputs:` block exactly as the loader does (`implicit_some`).
fn inputs_from_ron(ron_str: &str) -> InputMap {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(ron_str)
        .expect("inputs block must parse")
}

fn controller(inputs: InputMap) -> CharacterController {
    CharacterController {
        walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
        inputs,
        is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
        double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
        collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0,
        coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8,
        jump_air_grace: 0, jump_liftoff_y: None,
    }
}

/// One 64 Hz tick of virtual time per `app.update()`, so look-rate maths is exact.
fn pin_one_tick_per_update(app: &mut App) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 64.0)));
}

fn fp_state(invert_y: bool) -> FirstPersonState {
    FirstPersonState {
        eye_offset: Vec3::new(0.0, 1.6, 0.0),
        sensitivity: 0.01,
        pitch: 0.0,
        min_pitch: -1.2,
        max_pitch: 1.2,
        look_speed: 2.5,
        invert_y,
        capture_cursor: false,
        hide_own_body: false,
    }
}

/// A player plus a `FirstPerson` camera targeting it. Returns `(player, camera)`.
fn first_person_app(inputs: InputMap, invert_y: bool, gamepad: Option<Entity>) -> (App, Entity, Entity) {
    let mut app = setup_test_app();
    pin_one_tick_per_update(&mut app);
    app.update();
    let mut player = app.world_mut().spawn((Transform::default(), controller(inputs)));
    if let Some(gp) = gamepad {
        player.insert(BoundGamepad(Some(gp)));
    }
    let player = player.id();
    let camera = app
        .world_mut()
        .spawn((
            Transform::default(),
            ActiveCameraMode::FirstPerson(fp_state(invert_y)),
            CameraTargets(vec![player]),
            FirstPersonCameraMode,
        ))
        .id();
    app.update();
    (app, player, camera)
}

fn yaw(app: &App, player: Entity) -> f32 {
    app.world().get::<Transform>(player).unwrap().rotation.to_euler(EulerRot::YXZ).0
}

fn pitch(app: &App, camera: Entity) -> f32 {
    match app.world().get::<ActiveCameraMode>(camera).unwrap() {
        ActiveCameraMode::FirstPerson(fp) => fp.pitch,
        _ => panic!("camera is not in FirstPerson mode"),
    }
}

fn hold(app: &mut App, key: KeyCode, frames: usize) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    for _ in 0..frames {
        app.update();
    }
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key);
}

fn look_keys() -> InputMap {
    inputs_from_ron(
        r#"(controls: FirstPerson, look_left: "KeyZ", look_right: "KeyX", look_up: "KeyC", look_down: "KeyV")"#,
    )
}

// ── keyboard look ─────────────────────────────────────────────────────────────────────────

#[test]
fn keyboard_look_left_turns_the_player_left_and_right_turns_it_back() {
    let (mut app, player, _camera) = first_person_app(look_keys(), false, None);
    hold(&mut app, KeyCode::KeyZ, 8);
    let left = yaw(&app, player);
    assert!(left > 0.0, "look_left must turn the player left (positive yaw), got {left}");
    hold(&mut app, KeyCode::KeyX, 8);
    assert!(yaw(&app, player) < left, "look_right must turn the player back to the right");
}

#[test]
fn keyboard_look_up_raises_pitch_and_look_down_lowers_it() {
    let (mut app, _player, camera) = first_person_app(look_keys(), false, None);
    hold(&mut app, KeyCode::KeyC, 8);
    let up = pitch(&app, camera);
    assert!(up > 0.0, "look_up must raise pitch, got {up}");
    hold(&mut app, KeyCode::KeyV, 8);
    assert!(pitch(&app, camera) < up, "look_down must lower pitch");
}

#[test]
fn invert_y_flips_keyboard_pitch() {
    let (mut app, _player, camera) = first_person_app(look_keys(), true, None);
    hold(&mut app, KeyCode::KeyC, 8);
    assert!(pitch(&app, camera) < 0.0, "with invert_y, look_up must lower pitch");
}

#[test]
fn keyboard_pitch_is_clamped_to_the_authored_limits() {
    let (mut app, _player, camera) = first_person_app(look_keys(), false, None);
    hold(&mut app, KeyCode::KeyC, 400);
    assert!(pitch(&app, camera) <= 1.2 + 1e-4, "pitch must stop at max_pitch");
}

#[test]
fn no_look_keys_bound_leaves_the_player_unturned() {
    let (mut app, player, camera) = first_person_app(InputMap::default(), false, None);
    hold(&mut app, KeyCode::KeyZ, 8);
    assert_eq!(yaw(&app, player), 0.0);
    assert_eq!(pitch(&app, camera), 0.0);
}

// ── stick look ────────────────────────────────────────────────────────────────────────────

fn first_person_app_with_pad() -> (App, Entity, Entity, Entity) {
    let mut app = setup_test_app();
    pin_one_tick_per_update(&mut app);
    app.update();
    let gp = connect_test_gamepad(&mut app);
    app.update();
    app.update();
    let player = app
        .world_mut()
        .spawn((
            Transform::default(),
            controller(inputs_from_ron("(controls: FirstPerson, gamepad_index: 0)")),
            BoundGamepad(Some(gp)),
        ))
        .id();
    let camera = app
        .world_mut()
        .spawn((
            Transform::default(),
            ActiveCameraMode::FirstPerson(fp_state(false)),
            CameraTargets(vec![player]),
            FirstPersonCameraMode,
        ))
        .id();
    app.update();
    (app, player, camera, gp)
}

#[test]
fn right_stick_right_turns_the_player_right() {
    let (mut app, player, _camera, gp) = first_person_app_with_pad();
    set_gamepad_axis(&mut app, gp, GamepadAxis::RightStickX, 1.0);
    for _ in 0..8 {
        app.update();
    }
    assert!(yaw(&app, player) < 0.0, "stick right must turn the player right (negative yaw)");
}

#[test]
fn right_stick_up_looks_up() {
    let (mut app, _player, camera, gp) = first_person_app_with_pad();
    set_gamepad_axis(&mut app, gp, GamepadAxis::RightStickY, 1.0);
    for _ in 0..8 {
        app.update();
    }
    assert!(pitch(&app, camera) > 0.0, "stick up must raise pitch (not inverted)");
}

#[test]
fn right_stick_inside_the_deadzone_does_nothing() {
    let (mut app, player, camera, gp) = first_person_app_with_pad();
    set_gamepad_axis(&mut app, gp, GamepadAxis::RightStickX, 0.10);
    set_gamepad_axis(&mut app, gp, GamepadAxis::RightStickY, 0.10);
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(yaw(&app, player), 0.0, "drift below the deadzone must not turn the player");
    assert_eq!(pitch(&app, camera), 0.0, "drift below the deadzone must not pitch the camera");
}

#[test]
fn stick_look_does_not_apply_while_paused() {
    let (mut app, player, _camera, gp) = first_person_app_with_pad();
    app.world_mut().resource_mut::<ActionQueue>().push(Action::PauseGame);
    app.update();
    let before = yaw(&app, player);
    set_gamepad_axis(&mut app, gp, GamepadAxis::RightStickX, 1.0);
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(yaw(&app, player), before, "stick look must not turn the player while paused");
    app.world_mut().resource_mut::<ActionQueue>().push(Action::ResumeGame);
    for _ in 0..8 {
        app.update();
    }
    assert!(yaw(&app, player) < before, "stick look works again after resume");
}

// ── translator: right-stick ownership ────────────────────────────────────────────────────

fn translated(app: &mut App) -> Vec<InputAction> {
    app.world_mut().run_system_once(input_translator_system).unwrap();
    app.world()
        .resource::<Messages<InputActionMessage>>()
        .iter_current_update_messages()
        .map(|m| m.action.clone())
        .collect()
}

#[test]
fn translator_skips_right_stick_turn_for_a_player_seen_through_a_first_person_camera() {
    let (mut app, _player, _camera, gp) = first_person_app_with_pad();
    set_gamepad_axis(&mut app, gp, GamepadAxis::RightStickX, 1.0);
    app.update();
    let actions = translated(&mut app);
    assert!(
        !actions.iter().any(|a| matches!(a, InputAction::Turn(_))),
        "the FirstPerson camera owns right-stick look, so the translator must not also Turn, got {actions:?}"
    );
}

#[test]
fn translator_still_turns_on_the_right_stick_without_a_first_person_camera() {
    let mut app = setup_test_app();
    pin_one_tick_per_update(&mut app);
    app.update();
    let gp = connect_test_gamepad(&mut app);
    app.update();
    app.update();
    app.world_mut().spawn((
        Transform::default(),
        controller(inputs_from_ron("(gamepad_index: 0)")),
        BoundGamepad(Some(gp)),
    ));
    set_gamepad_axis(&mut app, gp, GamepadAxis::RightStickX, 1.0);
    app.update();
    let actions = translated(&mut app);
    assert!(
        actions.iter().any(|a| matches!(a, InputAction::Turn(v) if *v < 0.0)),
        "outside first person the right stick still turns the character, got {actions:?}"
    );
}

// ── translator: the FirstPerson scheme's keys and run mode ───────────────────────────────

fn translate_keys(inputs: InputMap, keys: &[KeyCode]) -> Vec<InputAction> {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().spawn(controller(inputs));
    for k in keys {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(*k);
    }
    translated(&mut app)
}

#[test]
fn first_person_scheme_a_and_d_strafe_and_never_turn() {
    let inputs = inputs_from_ron("(controls: FirstPerson)");
    let left = translate_keys(inputs.clone(), &[KeyCode::KeyA]);
    assert!(left.iter().any(|a| matches!(a, InputAction::Move(v) if v.x < 0.0)), "A strafes left, got {left:?}");
    assert!(!left.iter().any(|a| matches!(a, InputAction::Turn(_))), "A must not turn, got {left:?}");
    let right = translate_keys(inputs, &[KeyCode::KeyD]);
    assert!(right.iter().any(|a| matches!(a, InputAction::Move(v) if v.x > 0.0)), "D strafes right, got {right:?}");
    assert!(!right.iter().any(|a| matches!(a, InputAction::Turn(_))), "D must not turn, got {right:?}");
}

#[test]
fn third_person_scheme_a_and_d_still_turn() {
    let actions = translate_keys(InputMap::default(), &[KeyCode::KeyA]);
    assert!(actions.iter().any(|a| matches!(a, InputAction::Turn(v) if *v > 0.0)), "A turns left, got {actions:?}");
    assert!(!actions.iter().any(|a| matches!(a, InputAction::Move(v) if v.x != 0.0)), "A must not strafe, got {actions:?}");
}

#[test]
fn hold_run_mode_reports_the_held_state_every_tick() {
    let inputs = inputs_from_ron("(controls: FirstPerson)");
    let held = translate_keys(inputs.clone(), &[KeyCode::ShiftLeft]);
    assert!(held.iter().any(|a| matches!(a, InputAction::Run(true))), "held Shift reports Run(true), got {held:?}");
    let released = translate_keys(inputs, &[]);
    assert!(released.iter().any(|a| matches!(a, InputAction::Run(false))), "released Shift reports Run(false), got {released:?}");
}

#[test]
fn toggle_run_mode_only_reports_the_press() {
    let none = translate_keys(InputMap::default(), &[]);
    assert!(!none.iter().any(|a| matches!(a, InputAction::Run(_))), "no press, no Run, got {none:?}");
    let pressed = translate_keys(InputMap::default(), &[KeyCode::ShiftLeft]);
    assert!(pressed.iter().any(|a| matches!(a, InputAction::Run(true))), "a press reports Run(true), got {pressed:?}");
}
