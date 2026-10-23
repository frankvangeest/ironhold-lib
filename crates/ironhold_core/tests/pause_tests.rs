//! Real pause (`planning/features/real_pause.md`), phase 1: `SimClock`, the three actions, the
//! `game.paused`/`game.resumed` events, the Replace-mode auto-resume and the input gates.
//!
//! Pause is applied by `apply_sim_clock_system` in the same `Update` that executes the action, but the
//! clock only stops *advancing* on the next frame: tests therefore look at `Time<Virtual>::is_paused()`
//! straight after the action's frame and at `delta()`/fixed-tick counts one frame later.

use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

use ironhold_core::capabilities::action_bar::ActionSlotUi;
use ironhold_core::capabilities::interactable::Interactable;
use ironhold_core::capabilities::player::{BoundGamepad, CharacterController, PlayerTarget};
use ironhold_core::GameVariables;
use ironhold_core::runtime::{ActionQueue, GameEvent, PauseHold, PendingJoinGamepad, ProjectClaimedGamepadBindings, SceneHandleV2, SimClock, SpawnId, UiEvent};
use ironhold_core::schema::player::InputMap;
use ironhold_core::schema::{Action, AppState, GameSceneV2, ProjectConfig, ProjectConfigHandle};

mod support;
use support::{connect_test_gamepad, press_gamepad_button, setup_test_app};

#[derive(Resource, Default)]
struct FixedTicks(u32);

fn count_fixed_ticks(mut n: ResMut<FixedTicks>) {
    n.0 += 1;
}

/// Pins the virtual clock to one 64 Hz fixed tick per `app.update()` so tick counts are exact.
fn pin_one_tick_per_update(app: &mut App) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 64.0)));
}

fn run(app: &mut App, action: Action) {
    app.world_mut().resource_mut::<ActionQueue>().push(action);
    app.update();
}

fn paused(app: &App) -> bool {
    app.world().resource::<Time<Virtual>>().is_paused()
}

/// Every `game.*` event currently buffered (Bevy keeps messages for two frames).
fn game_events(app: &mut App, prefix: &str) -> Vec<String> {
    let prefix = prefix.to_string();
    app.world_mut()
        .run_system_once(move |mut r: MessageReader<GameEvent>| {
            r.read()
                .map(|GameEvent::Trigger(n)| n.clone())
                .filter(|n| n.starts_with(&prefix))
                .collect::<Vec<_>>()
        })
        .unwrap()
}

/// Drive a Replace-mode scene load (same shape as `scene_lifecycle_tests::drive_replace_load`).
fn drive_replace_load(app: &mut App) {
    let config_handle = app.world_mut().resource_mut::<Assets<ProjectConfig>>().add(ProjectConfig {
        schema_version: 1,
        initial_scene: "scenes/t.ron".to_string(),
        ..Default::default()
    });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));
    let scene: GameSceneV2 = ron::de::from_str("(schema_version: 2, entities: [], ui: [])").unwrap();
    let scene_handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LoadingScene);
    app.update();
    app.update();
    app.update();
}

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

#[test]
fn pause_game_holds_the_clock_and_emits_game_paused() {
    let mut app = setup_test_app();
    app.update();
    assert!(!paused(&app));

    run(&mut app, Action::PauseGame);
    assert!(paused(&app), "PauseGame must pause Time<Virtual>");
    assert!(app.world().resource::<SimClock>().is_held(PauseHold::Menu));
    assert!(game_events(&mut app, "game.").contains(&"game.paused".to_string()));
}

#[test]
fn resume_game_releases_and_emits_game_resumed() {
    let mut app = setup_test_app();
    app.update();
    run(&mut app, Action::PauseGame);
    run(&mut app, Action::ResumeGame);
    assert!(!paused(&app));
    assert!(app.world().resource::<SimClock>().is_running());
    assert!(game_events(&mut app, "game.").contains(&"game.resumed".to_string()));
}

#[test]
fn pause_and_resume_are_idempotent_not_counters() {
    let mut app = setup_test_app();
    app.update();

    // ResumeGame when not paused: a no-op, no event.
    run(&mut app, Action::ResumeGame);
    app.update();
    app.update(); // let any earlier buffered message age out
    assert!(!paused(&app));
    assert!(game_events(&mut app, "game.").is_empty(), "ResumeGame while running must emit nothing");

    // Two PauseGame, one ResumeGame: running again (pause -> options-from-pause must not need two resumes).
    run(&mut app, Action::PauseGame);
    run(&mut app, Action::PauseGame);
    assert!(paused(&app));
    run(&mut app, Action::ResumeGame);
    assert!(!paused(&app), "one ResumeGame must undo any number of PauseGame");
}

#[test]
fn toggle_pause_flips_the_menu_hold() {
    let mut app = setup_test_app();
    app.update();
    run(&mut app, Action::TogglePause);
    assert!(paused(&app));
    run(&mut app, Action::TogglePause);
    assert!(!paused(&app));
}

#[test]
fn fixed_ticks_stop_while_paused_and_resume_without_a_burst() {
    let mut app = setup_test_app();
    app.init_resource::<FixedTicks>();
    app.add_systems(FixedUpdate, count_fixed_ticks);
    pin_one_tick_per_update(&mut app);
    for _ in 0..4 {
        app.update();
    }
    assert!(app.world().resource::<FixedTicks>().0 > 0, "precondition: fixed ticks run while running");

    run(&mut app, Action::PauseGame); // pause lands this frame; the clock stops advancing from the next
    let at_pause = app.world().resource::<FixedTicks>().0;
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(app.world().resource::<FixedTicks>().0, at_pause, "no fixed tick may run while paused");
    assert_eq!(app.world().resource::<Time<Virtual>>().delta(), Duration::ZERO, "virtual delta is 0 while paused");

    run(&mut app, Action::ResumeGame);
    for _ in 0..2 {
        app.update();
    }
    let after = app.world().resource::<FixedTicks>().0;
    assert!(after > at_pause, "ticks run again after resume");
    assert!(after - at_pause <= 3, "resume must not replay the paused time as a burst of ticks (got {})", after - at_pause);
}

#[test]
fn replace_load_releases_the_menu_pause_and_emits_game_resumed() {
    let mut app = setup_test_app();
    app.update();
    run(&mut app, Action::PauseGame);
    assert!(paused(&app));

    drive_replace_load(&mut app);
    assert!(app.world().resource::<SimClock>().is_running(), "a Replace-mode load must auto-resume");
    assert!(!paused(&app));
    assert!(game_events(&mut app, "game.").contains(&"game.resumed".to_string()));
}

#[test]
fn only_the_menu_hold_is_released_by_resume_and_by_a_replace_load() {
    let mut app = setup_test_app();
    app.update();
    // A static-scene-mode style hold must survive both the menu's ResumeGame and an auto-resume.
    app.world_mut().resource_mut::<SimClock>().hold(PauseHold::Static);
    app.update();
    assert!(paused(&app));

    run(&mut app, Action::PauseGame);
    run(&mut app, Action::ResumeGame);
    assert!(paused(&app), "ResumeGame must not release a Static hold");

    run(&mut app, Action::PauseGame);
    drive_replace_load(&mut app);
    assert!(paused(&app), "an auto-resume must not release a Static hold");
    assert!(app.world().resource::<SimClock>().is_held(PauseHold::Static));
    assert!(!app.world().resource::<SimClock>().is_held(PauseHold::Menu));
}

/// `game.paused`/`game.resumed` mean "the player's pause took effect", not "the clock stopped": the browser-test
/// `Static` hold (`?static=1`) holds the clock from boot without ever announcing, so a project that opens its pause
/// menu from a `game.paused` rule never opens it in a static screenshot (`static_scene_mode.md`).
#[test]
fn a_static_hold_from_boot_never_emits_game_paused() {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(SimClock::with_hold(PauseHold::Static));
    app.update();
    app.update();
    assert!(paused(&app), "the Static hold must pause Time<Virtual>");
    assert!(game_events(&mut app, "game.").is_empty(), "Static must not announce a pause");
    assert_eq!(app.world().resource::<Time<Virtual>>().elapsed(), Duration::ZERO, "no virtual time may pass");
}

#[test]
fn pause_game_under_a_static_hold_still_announces_and_resume_keeps_the_clock_held() {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(SimClock::with_hold(PauseHold::Static));
    app.update();

    run(&mut app, Action::PauseGame);
    assert_eq!(game_events(&mut app, "game."), vec!["game.paused".to_string()], "a PauseGame must announce exactly once");
    assert!(paused(&app));

    run(&mut app, Action::ResumeGame);
    assert_eq!(
        game_events(&mut app, "game."),
        vec!["game.paused".to_string(), "game.resumed".to_string()],
        "a ResumeGame must announce exactly once (the earlier pause is still buffered)"
    );
    assert!(paused(&app), "the clock stays held by Static");
    assert!(app.world().resource::<SimClock>().is_held(PauseHold::Static));
}

#[test]
fn a_replace_load_under_a_static_hold_emits_game_resumed_but_keeps_the_clock_held() {
    let mut app = setup_test_app();
    app.world_mut().insert_resource(SimClock::with_hold(PauseHold::Static));
    app.update();
    run(&mut app, Action::PauseGame);
    drive_replace_load(&mut app);
    assert!(game_events(&mut app, "game.").contains(&"game.resumed".to_string()));
    assert!(paused(&app), "an auto-resume must not release a Static hold");
}

#[test]
fn overlay_actions_never_resume() {
    let mut app = setup_test_app();
    app.update();
    run(&mut app, Action::PauseGame);

    run(&mut app, Action::LoadSceneOverlay("scenes/pause.scene.ron".to_string()));
    run(&mut app, Action::ToggleOverlay("scenes/pause.scene.ron".to_string()));
    run(&mut app, Action::UnloadOverlay);
    assert!(paused(&app), "overlay open/close must never touch the pause");
    assert!(app.world().resource::<SimClock>().is_held(PauseHold::Menu));
}

#[test]
fn pause_game_after_a_replace_load_completes_pauses_the_new_scene() {
    // To start a scene paused, run PauseGame once it is loaded (e.g. from its `scene.ready:<scene>` rule).
    let mut app = setup_test_app();
    app.update();
    run(&mut app, Action::PauseGame);
    drive_replace_load(&mut app);
    assert!(!paused(&app), "the Replace load released the pause");
    run(&mut app, Action::PauseGame);
    assert!(paused(&app));
}

#[test]
fn load_scene_and_pause_game_queued_together_end_up_running() {
    // FIFO: both actions run in the same executor drain, so the pause is taken first; the Replace load then releases it
    // when the scene spawns a few frames later. A Replace load ALWAYS starts the new scene running.
    let mut app = setup_test_app();
    app.update();
    app.world_mut().resource_mut::<ActionQueue>().push(Action::LoadScene("scenes/t.ron".to_string()));
    app.world_mut().resource_mut::<ActionQueue>().push(Action::PauseGame);
    app.update();
    assert!(paused(&app), "the pause is taken in the same drain as the LoadScene");
    drive_replace_load(&mut app);
    assert!(!paused(&app), "the Replace load releases it: the new scene runs");
}

#[test]
fn action_bar_slot_does_not_fire_while_paused_and_fires_after_resume() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        resolved_mouse_button: None,
        do_actions: vec![Action::SetVariable("slot_fired".to_string(), "yes".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });
    app.world_mut().spawn((SpawnId("player_01".to_string()), controller(), PlayerTarget::default()));
    app.update();

    run(&mut app, Action::PauseGame);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().reset(KeyCode::Digit1); // no InputPlugin here: clear the latch like a real frame boundary
    app.update();
    assert_eq!(app.world().resource::<GameVariables>().0.get("slot_fired"), None, "a key press while paused must not fire a slot");

    run(&mut app, Action::ResumeGame);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("slot_fired").map(String::as_str),
        Some("yes"),
        "the slot fires again once resumed"
    );
}

#[test]
fn interact_is_suppressed_while_paused() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().spawn((
        SpawnId("chest".to_string()),
        Transform::from_xyz(0.5, 0.0, 0.0),
        Interactable { radius: 3.0, hint_text: None, requires_item: None },
    ));
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        Transform::default(),
        controller(),
        PlayerTarget::default(),
    ));
    app.update();

    run(&mut app, Action::PauseGame);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyF);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().reset(KeyCode::KeyF);
    app.update();
    app.update();
    assert!(
        !game_events(&mut app, "entity.interact").iter().any(|n| n == "entity.interacted:chest"),
        "F must not interact while paused"
    );

    run(&mut app, Action::ResumeGame);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyF);
    app.update();
    assert!(
        game_events(&mut app, "entity.interact").iter().any(|n| n == "entity.interacted:chest"),
        "F interacts again once resumed"
    );
}

// ---- global_claimed_gamepad_bindings (phase 2) ----

fn ui_triggers(app: &mut App, trigger: &str) -> usize {
    let trigger = trigger.to_string();
    app.world_mut()
        .run_system_once(move |mut r: MessageReader<UiEvent>| {
            r.read().filter(|UiEvent::ButtonPressed(t)| *t == trigger).count()
        })
        .unwrap()
}

/// An `InGame` app with `"Start" -> toggle_pause` claimed bindings and two connected pads, each owned by a live player.
fn claimed_pad_app() -> (App, Entity, Entity) {
    let mut app = setup_test_app();
    app.update();
    drive_replace_load(&mut app); // leaves the app InGame, which the pad system requires
    app.insert_resource(ProjectClaimedGamepadBindings(std::collections::BTreeMap::from([(
        "Start".to_string(),
        "toggle_pause".to_string(),
    )])));
    let gp_a = connect_test_gamepad(&mut app);
    let gp_b = connect_test_gamepad(&mut app);
    app.update();
    for (i, gp) in [gp_a, gp_b].into_iter().enumerate() {
        app.world_mut().spawn((
            SpawnId(format!("player_{:02}", i + 1)),
            controller(),
            PlayerTarget::default(),
            BoundGamepad(Some(gp)),
        ));
    }
    app.update();
    app.update(); // a quiet frame so earlier messages age out
    (app, gp_a, gp_b)
}

#[test]
fn claimed_pad_binding_fires_for_a_joined_players_pad() {
    let (mut app, gp_a, _gp_b) = claimed_pad_app();
    press_gamepad_button(&mut app, gp_a, GamepadButton::Start);
    app.update();
    assert_eq!(ui_triggers(&mut app, "toggle_pause"), 1, "a claimed pad's Start must fire the bound trigger");
    assert_eq!(app.world().resource::<PendingJoinGamepad>().0, None, "a claimed-pad binding must never take part in hot join");
}

#[test]
fn claimed_pad_binding_does_not_fire_for_an_unclaimed_pad() {
    let (mut app, _gp_a, _gp_b) = claimed_pad_app();
    let free = connect_test_gamepad(&mut app);
    app.update();
    app.update();
    press_gamepad_button(&mut app, free, GamepadButton::Start);
    app.update();
    assert_eq!(ui_triggers(&mut app, "toggle_pause"), 0, "an unclaimed pad must not fire a claimed-pad binding");
}

#[test]
fn two_players_pressing_the_same_claimed_button_in_one_frame_emit_one_trigger() {
    // Otherwise `toggle_pause` fires twice and the FSM toggles straight back to unpaused.
    let (mut app, gp_a, gp_b) = claimed_pad_app();
    press_gamepad_button(&mut app, gp_a, GamepadButton::Start);
    press_gamepad_button(&mut app, gp_b, GamepadButton::Start);
    app.update();
    assert_eq!(ui_triggers(&mut app, "toggle_pause"), 1, "one trigger per frame across all pads");
}

#[test]
fn claimed_pad_binding_still_fires_while_paused() {
    // The button that pauses must also resume: the claimed-pad path is not gated.
    let (mut app, gp_a, _gp_b) = claimed_pad_app();
    run(&mut app, Action::PauseGame);
    assert!(paused(&app));
    app.update();
    press_gamepad_button(&mut app, gp_a, GamepadButton::Start);
    app.update();
    assert_eq!(ui_triggers(&mut app, "toggle_pause"), 1, "must still fire while the sim is held");
}

#[test]
fn first_person_mouse_look_does_not_rotate_the_player_while_paused() {
    // `first_person_camera_system` applies raw (not frame-time scaled) mouse yaw to the PLAYER's Transform, so the
    // clock freeze alone does not stop it: it needs its own gate.
    use bevy::input::mouse::MouseMotion;
    use ironhold_core::capabilities::camera::{ActiveCameraMode, CameraTargets, FirstPersonCameraMode, FirstPersonState};

    let mut app = setup_test_app();
    app.update();
    let player = app.world_mut().spawn((SpawnId("player_01".to_string()), Transform::default(), controller())).id();
    app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::FirstPerson(FirstPersonState {
            eye_offset: Vec3::new(0.0, 1.6, 0.0),
            sensitivity: 0.01,
            pitch: 0.0,
            min_pitch: -1.0,
            max_pitch: 1.0,
            look_speed: 2.5,
            invert_y: false,
            capture_cursor: false,
            hide_own_body: false,
            aim_target: None,
        }),
        CameraTargets(vec![player]),
        FirstPersonCameraMode,
    ));
    app.update();

    let look = |app: &mut App| {
        app.world_mut().resource_mut::<Messages<MouseMotion>>().write(MouseMotion { delta: Vec2::new(100.0, 0.0) });
        app.update();
    };
    let rot = |app: &App| app.world().get::<Transform>(player).unwrap().rotation;

    let before = rot(&app);
    look(&mut app);
    assert_ne!(rot(&app), before, "precondition: mouse look turns the player while running");

    run(&mut app, Action::PauseGame);
    let frozen = rot(&app);
    look(&mut app);
    look(&mut app);
    assert_eq!(rot(&app), frozen, "mouse look must not turn the player while paused");

    run(&mut app, Action::ResumeGame);
    let resumed = rot(&app);
    look(&mut app);
    assert_ne!(rot(&app), resumed, "mouse look works again after resume");
}

#[test]
fn slot_still_pressed_on_the_first_frame_after_resume_does_not_fire() {
    // `action_bar_input_system` is gated in its body (not with `run_if`) so its `Ref<Interaction>` change baseline keeps
    // advancing while paused: a slot clicked during the pause and still `Pressed` when the game resumes must not fire.
    let mut app = setup_test_app();
    app.update();
    let slot = app
        .world_mut()
        .spawn((
            ActionSlotUi {
                slot_key: "1".to_string(),
                resolved_key: None,
                resolved_gamepad_button: None,
                resolved_mouse_button: None,
                do_actions: vec![Action::SetVariable("slot_clicked".to_string(), "yes".to_string())],
                cooldown_secs: None,
                cost: None,
                owner_player: None,
            },
            Interaction::None,
        ))
        .id();
    app.world_mut().spawn((SpawnId("player_01".to_string()), controller(), PlayerTarget::default()));
    app.update();

    run(&mut app, Action::PauseGame);
    app.world_mut().entity_mut(slot).insert(Interaction::Pressed);
    app.update();
    app.update();
    run(&mut app, Action::ResumeGame);
    app.update();
    assert_eq!(app.world().resource::<GameVariables>().0.get("slot_clicked"), None, "a press made while paused must not fire on resume");

    // A fresh press after the resume still fires.
    app.world_mut().entity_mut(slot).insert(Interaction::None);
    app.update();
    app.world_mut().entity_mut(slot).insert(Interaction::Pressed);
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("slot_clicked").map(String::as_str),
        Some("yes"),
        "a new click after resume fires the slot"
    );
}

#[test]
fn delayed_events_freeze_while_paused_and_fire_after_resume() {
    let mut app = setup_test_app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
    app.update();

    let fired = |app: &mut App| game_events(app, "timer.").iter().any(|n| n == "timer.done");

    run(&mut app, Action::EmitEventAfterDelay { event: "timer.done".to_string(), delay_secs: 0.35 });
    run(&mut app, Action::PauseGame);
    let mut fired_while_paused = false;
    for _ in 0..8 {
        app.update();
        fired_while_paused |= fired(&mut app);
    }
    assert!(!fired_while_paused, "a delayed event must not fire while the game is paused");

    run(&mut app, Action::ResumeGame);
    let mut fired_after_resume = false;
    for _ in 0..8 {
        app.update();
        fired_after_resume |= fired(&mut app);
    }
    assert!(fired_after_resume, "the delayed event fires after resume");
}

#[test]
fn message_buffers_do_not_grow_while_paused() {
    // A paused clock runs zero FixedUpdate ticks, which is what normally flips Bevy's message buffers; without
    // `keep_messages_flowing_while_held` every Messages<T> would grow for the whole pause (and WASM memory never shrinks).
    let mut app = setup_test_app();
    pin_one_tick_per_update(&mut app);
    for _ in 0..4 {
        app.update();
    }
    run(&mut app, Action::PauseGame);
    // `MouseMotion` is registered with Bevy's message registry (`add_message`) in the harness; the harness's bare
    // `init_resource::<Messages<GameEvent>>()` is not, so it would never flip regardless of the clock.
    for _ in 0..120 {
        app.world_mut().resource_mut::<Messages<bevy::input::mouse::MouseMotion>>()
            .write(bevy::input::mouse::MouseMotion { delta: Vec2::new(1.0, 0.0) });
        app.update();
    }
    let len = app.world().resource::<Messages<bevy::input::mouse::MouseMotion>>().len();
    assert!(len < 10, "message buffer grew to {len} while paused (expected it to keep flipping)");
}
