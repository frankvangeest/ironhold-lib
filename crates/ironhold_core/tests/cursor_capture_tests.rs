//! `FirstPersonCameraDef.capture_cursor`: `cursor_capture_system` and the mouse-look gate in
//! `first_person_camera_system` (`planning/features/done/first_person_starter.md`, Spike A).
//!
//! **Every test takes `GUARD` first**: the runner -> core report channel
//! (`report_pointer_lock` / `runner_reports_pointer_lock`) is a process-wide atomic, because the
//! web runner's DOM listeners have no handle to the Bevy `World`, and this file's tests would
//! otherwise see each other's reports when the test harness runs them on parallel threads.

use bevy::input::mouse::{MouseButton, MouseMotion};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use std::sync::Mutex;

use ironhold_core::capabilities::camera::{
    ActiveCameraMode, CameraTargets, FirstPersonCameraMode, FirstPersonState,
};
use ironhold_core::capabilities::cursor_capture::{
    report_pointer_lock, reset_pointer_lock_report, runner_reports_pointer_lock, CursorLockState,
};
use ironhold_core::capabilities::action_bar::ActionSlotUi;
use ironhold_core::capabilities::player::{CharacterController, PlayerTarget};
use ironhold_core::runtime::SpawnId;
use ironhold_core::runtime::{ActionQueue, GameEvent};
use ironhold_core::schema::player::InputMap;
use ironhold_core::schema::Action;

mod support;
use support::setup_test_app;

static GUARD: Mutex<()> = Mutex::new(());

fn controller() -> CharacterController {
    CharacterController {
        walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
        inputs: InputMap::default(),
        is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
        double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
        collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0,
        coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8,
        jump_air_grace: 0, jump_liftoff_y: None,
    }
}

fn fp_state(capture_cursor: bool) -> FirstPersonState {
    FirstPersonState {
        eye_offset: Vec3::new(0.0, 1.6, 0.0),
        sensitivity: 0.01,
        pitch: 0.0,
        min_pitch: -1.2,
        max_pitch: 1.2,
        look_speed: 2.5,
        invert_y: false,
        capture_cursor,
        hide_own_body: false,
        aim_target: None,
    }
}

/// A primary window, a player, and a `FirstPerson` camera. Returns `(app, window, player)`.
fn capture_app(capture_cursor: bool) -> (App, Entity, Entity) {
    let mut app = setup_test_app();
    app.update();
    let window = app.world_mut().spawn((PrimaryWindow, CursorOptions::default())).id();
    let player = app.world_mut().spawn((Transform::default(), controller())).id();
    app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::FirstPerson(fp_state(capture_cursor)),
        CameraTargets(vec![player]),
        FirstPersonCameraMode,
    ));
    app.update();
    (app, window, player)
}

/// Counts frames in which `CursorOptions` was written (marked changed).
#[derive(Resource, Default)]
struct OptionsWrites(usize);

fn count_options_writes(changed: Query<(), Changed<CursorOptions>>, mut n: ResMut<OptionsWrites>) {
    if !changed.is_empty() {
        n.0 += 1;
    }
}

fn click(app: &mut App) {
    let mut m = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    m.press(MouseButton::Left);
    app.update();
    let mut m = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    m.release(MouseButton::Left);
    m.clear();
}

fn cursor(app: &App, window: Entity) -> (CursorGrabMode, bool) {
    let c = app.world().get::<CursorOptions>(window).unwrap();
    (c.grab_mode, c.visible)
}

fn locked(app: &App) -> bool {
    app.world().resource::<CursorLockState>().locked
}

fn lock_lost_events(app: &mut App) -> usize {
    app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .filter(|e| matches!(e, GameEvent::Trigger(name) if name == "cursor.lock_lost"))
        .count()
}

fn yaw(app: &App, player: Entity) -> f32 {
    app.world().get::<Transform>(player).unwrap().rotation.to_euler(EulerRot::YXZ).0
}

fn mouse_look(app: &mut App) {
    app.world_mut()
        .resource_mut::<Messages<MouseMotion>>()
        .write(MouseMotion { delta: Vec2::new(100.0, 0.0) });
    app.update();
}

// ── native: the engine's own grab is the lock state ──────────────────────────────────────

#[test]
fn a_click_grabs_and_hides_the_cursor_only_for_a_capture_cursor_camera() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    let (mut app, window, _p) = capture_app(true);
    assert_eq!(cursor(&app, window), (CursorGrabMode::None, true), "nothing grabbed before the user clicks");
    click(&mut app);
    assert_eq!(cursor(&app, window), (CursorGrabMode::Locked, false), "the first click grabs and hides the cursor");
    assert!(locked(&app), "native: the engine's own grab is the lock state");
}

#[test]
fn a_camera_without_capture_cursor_never_touches_the_cursor() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    let (mut app, window, _p) = capture_app(false);
    click(&mut app);
    assert_eq!(cursor(&app, window), (CursorGrabMode::None, true));
    assert!(!locked(&app));
}

#[test]
fn pausing_releases_the_cursor_and_a_click_after_resume_grabs_it_again() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    let (mut app, window, _p) = capture_app(true);
    click(&mut app);
    assert!(locked(&app));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::PauseGame);
    app.update();
    app.update();
    assert_eq!(cursor(&app, window), (CursorGrabMode::None, true), "paused: the cursor is free so the menu is clickable");
    assert!(!locked(&app));
    click(&mut app);
    assert_eq!(cursor(&app, window), (CursorGrabMode::None, true), "a click while paused must not capture");

    app.world_mut().resource_mut::<ActionQueue>().push(Action::ResumeGame);
    app.update();
    app.update();
    assert_eq!(cursor(&app, window), (CursorGrabMode::None, true), "resuming alone does not re-grab (needs a user gesture)");
    click(&mut app);
    assert_eq!(cursor(&app, window), (CursorGrabMode::Locked, false), "the next click after resume captures again");
}

#[test]
fn leaving_first_person_releases_the_cursor() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    let (mut app, window, _p) = capture_app(true);
    click(&mut app);
    assert!(locked(&app));
    // `SetCameraMode` away removes the `FirstPersonCameraMode` marker.
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<FirstPersonCameraMode>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(camera).remove::<FirstPersonCameraMode>();
    app.update();
    assert_eq!(cursor(&app, window), (CursorGrabMode::None, true));
    assert!(!locked(&app));
}

// ── mouse look is gated on the cursor really being captured ──────────────────────────────

#[test]
fn mouse_look_needs_the_cursor_captured_when_the_camera_asks_for_capture() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    let (mut app, _window, player) = capture_app(true);
    mouse_look(&mut app);
    assert_eq!(yaw(&app, player), 0.0, "uncaptured: moving the mouse over the page must not turn the player");
    click(&mut app);
    mouse_look(&mut app);
    assert!(yaw(&app, player) != 0.0, "captured: mouse look turns the player");
}

#[test]
fn mouse_look_is_never_gated_for_a_camera_that_does_not_capture() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    let (mut app, _window, player) = capture_app(false);
    mouse_look(&mut app);
    assert!(yaw(&app, player) != 0.0, "without capture_cursor the mouse always looks (existing behaviour)");
}

// ── web: the runner reports the real state ───────────────────────────────────────────────

#[test]
fn with_a_reporting_runner_the_engine_waits_for_the_real_lock() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    runner_reports_pointer_lock();
    let (mut app, window, player) = capture_app(true);
    click(&mut app);
    assert_eq!(cursor(&app, window), (CursorGrabMode::Locked, true), "the request is made on the click, the cursor stays visible");
    assert!(!locked(&app), "but a request is not a lock: the engine must not assume it succeeded");
    mouse_look(&mut app);
    assert_eq!(yaw(&app, player), 0.0, "no mouse look until the browser reports the lock");

    report_pointer_lock(true);
    app.update();
    assert!(locked(&app));
    assert_eq!(cursor(&app, window), (CursorGrabMode::Locked, false), "the cursor is hidden once the lock is real");
    mouse_look(&mut app);
    assert!(yaw(&app, player) != 0.0, "mouse look works once the lock is real");
    reset_pointer_lock_report();
}

#[test]
fn losing_a_granted_lock_emits_cursor_lock_lost_once() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    runner_reports_pointer_lock();
    let (mut app, window, _p) = capture_app(true);
    click(&mut app);
    report_pointer_lock(true);
    app.update();
    assert!(locked(&app));
    assert_eq!(cursor(&app, window), (CursorGrabMode::Locked, false));
    assert_eq!(lock_lost_events(&mut app), 0);

    // The user presses Escape: the browser drops the lock without the engine asking.
    report_pointer_lock(false);
    app.update();
    assert_eq!(lock_lost_events(&mut app), 1, "losing the lock must raise `cursor.lock_lost`");
    assert_eq!(
        cursor(&app, window),
        (CursorGrabMode::None, true),
        "a lost lock releases fully: the cursor must be visible again even in a project that does not pause on the event"
    );
    app.update();
    // The message buffer is not flipped by a plain `Update` in a test app, so a second emission
    // would show up as a count of 2.
    assert_eq!(lock_lost_events(&mut app), 1, "and not emitted again on the next frame");
    reset_pointer_lock_report();
}

#[test]
fn a_refused_request_is_retried_on_the_next_click_without_a_lost_event() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    runner_reports_pointer_lock();
    let (mut app, window, _p) = capture_app(true);
    app.init_resource::<OptionsWrites>().add_systems(Update, count_options_writes);
    app.update();
    let baseline = app.world().resource::<OptionsWrites>().0;

    click(&mut app); // browser refuses (e.g. Chrome's re-lock delay): no report arrives
    app.update();
    assert!(!locked(&app));
    assert_eq!(lock_lost_events(&mut app), 0, "a request that was never granted is not a *lost* lock");
    assert_eq!(cursor(&app, window), (CursorGrabMode::Locked, true), "a refused request leaves the cursor visible");
    let after_first = app.world().resource::<OptionsWrites>().0;
    assert_eq!(after_first, baseline + 1, "the first click writes the grab once");

    // No click: nothing writes again (a per-frame write would re-request the lock every frame).
    app.update();
    app.update();
    assert_eq!(app.world().resource::<OptionsWrites>().0, after_first, "idle frames must not write CursorOptions");

    // The cursor is still `Locked` in the options (as in real use), yet the next click must write
    // again: that write is what makes bevy_winit call `request_pointer_lock` once more.
    click(&mut app);
    app.update(); // the counter system is unordered against the capture system: give it a frame to see the write
    assert_eq!(app.world().resource::<OptionsWrites>().0, after_first + 1, "the next click asks again");
    reset_pointer_lock_report();
}

// ── a mouse-button action-bar slot fires only while the cursor is captured ────────────────

/// Like `capture_app` plus an action-bar slot `"1"` (also on the `1` key) with `mouse_button: Left` whose action
/// bumps the `shots` variable, and a player the slot can resolve (`SpawnId` + `PlayerTarget`). Returns `(app, window)`.
fn slot_app(capture_cursor: bool, do_actions: Vec<Action>) -> (App, Entity) {
    let (mut app, window, player) = capture_app(capture_cursor);
    app.world_mut().entity_mut(player).insert((SpawnId("player_01".to_string()), PlayerTarget::default()));
    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        resolved_mouse_button: Some(MouseButton::Left),
        do_actions,
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });
    app.update();
    (app, window)
}

fn shots(app: &App) -> i32 {
    app.world()
        .resource::<ironhold_core::GameVariables>()
        .0
        .get("shots")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn count_shots() -> Vec<Action> {
    vec![Action::IncrementVariable("shots".to_string(), 1)]
}

#[test]
fn the_click_that_captures_the_cursor_does_not_fire_a_mouse_slot_but_the_next_one_does() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    let (mut app, _window) = slot_app(true, count_shots());
    click(&mut app);
    app.update();
    assert!(locked(&app), "precondition: the first click captured the cursor");
    assert_eq!(shots(&app), 0, "the capturing click is swallowed: it must not also shoot");
    click(&mut app);
    app.update();
    assert_eq!(shots(&app), 1, "with the cursor captured a click fires the slot");
}

#[test]
fn a_mouse_slot_never_fires_without_capture() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    let (mut app, _window) = slot_app(false, count_shots());
    click(&mut app);
    click(&mut app);
    app.update();
    assert_eq!(shots(&app), 0, "no capture_cursor camera: the cursor is never captured, so a click never shoots");
}

#[test]
fn a_mouse_slot_does_not_fire_while_paused_or_after_the_cursor_is_released() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    let (mut app, _window) = slot_app(true, count_shots());
    click(&mut app);
    app.update();
    click(&mut app);
    app.update();
    assert_eq!(shots(&app), 1, "precondition: it fires while captured");
    app.world_mut().resource_mut::<ActionQueue>().push(Action::PauseGame);
    app.update();
    app.update();
    click(&mut app);
    app.update();
    assert_eq!(shots(&app), 1, "paused: the cursor is released and the slot is gated");
}

#[test]
fn a_key_press_and_a_mouse_click_on_one_frame_activate_the_slot_once() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    let (mut app, _window) = slot_app(true, count_shots());
    click(&mut app);
    app.update(); // captured
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();
    // The test app has no input plugin, so `just_pressed` would stay true on later frames: clear it by hand.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    app.update();
    app.update();
    assert_eq!(shots(&app), 1, "key and mouse collapse into one activation");
}

#[test]
fn a_mouse_slot_with_no_target_emits_no_target_and_does_not_run_its_actions() {
    let _g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    reset_pointer_lock_report();
    let needs_target = vec![
        Action::ModifyStat { key: "{target}.health".to_string(), delta: -25.0 },
        Action::IncrementVariable("shots".to_string(), 1),
    ];
    let (mut app, _window) = slot_app(true, needs_target);
    click(&mut app);
    app.update();
    click(&mut app);
    let no_target = app
        .world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .filter(|e| matches!(e, GameEvent::Trigger(name) if name == "action_bar.no_target:1"))
        .count();
    assert_eq!(no_target, 1, "firing with nothing under the crosshair raises action_bar.no_target:{{key}} (a project plays a miss sound from it)");
    app.update();
    assert_eq!(shots(&app), 0, "the slot's actions did not run without a target");
}

