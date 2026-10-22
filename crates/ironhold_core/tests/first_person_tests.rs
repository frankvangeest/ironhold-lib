//! `planning/features/done/first_person_starter.md` v1: the `controls: FirstPerson` scheme as seen by
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
        aim_target: None,
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

/// `first_person_camera_system` only drives a camera's first target, so the translator must only hand the
/// stick to that one: a second target of a shared camera keeps turning on the stick.
#[test]
fn translator_only_skips_the_stick_turn_for_the_first_target_of_a_first_person_camera() {
    let mut app = setup_test_app();
    pin_one_tick_per_update(&mut app);
    app.update();
    let gp = connect_test_gamepad(&mut app);
    app.update();
    app.update();
    let spawn_player = |app: &mut App| {
        app.world_mut()
            .spawn((
                Transform::default(),
                controller(inputs_from_ron("(gamepad_index: 0)")),
                BoundGamepad(Some(gp)),
            ))
            .id()
    };
    let first = spawn_player(&mut app);
    let second = spawn_player(&mut app);
    app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::FirstPerson(fp_state(false)),
        CameraTargets(vec![first, second]),
        FirstPersonCameraMode,
    ));
    set_gamepad_axis(&mut app, gp, GamepadAxis::RightStickX, 1.0);
    app.update();
    app.world_mut().run_system_once(input_translator_system).unwrap();
    let turns: Vec<Entity> = app
        .world()
        .resource::<Messages<InputActionMessage>>()
        .iter_current_update_messages()
        .filter(|m| matches!(m.action, InputAction::Turn(_)))
        .map(|m| m.entity)
        .collect();
    assert!(!turns.contains(&first), "the camera's first target is owned by the camera, no Turn");
    assert!(turns.contains(&second), "a later target of a shared camera is not driven by the camera, so it still turns");
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

// ── hide_own_body ────────────────────────────────────────────────────────────────────────

mod hide_own_body {
    use super::*;
    use ironhold_core::capabilities::camera::OwnBodyHidden;
    use ironhold_core::capabilities::nameplate::NameplateTag;
    use ironhold_core::runtime::{LoadedAssetCatalog, LoadedPrefabCatalog, SceneHandleV2};
    use ironhold_core::schema::camera::CameraModeDef;
    use ironhold_core::schema::catalog::{
        AssetCatalog, ModelCatalogEntry, PrefabCatalog, PrefabComponents, PrefabDef, PrefabKind,
    };
    use ironhold_core::schema::{AppState, GameSceneV2, ProjectConfig, ProjectConfigHandle};

    fn ron_value<T: serde::de::DeserializeOwned>(s: &str) -> T {
        ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(s)
            .expect("must parse")
    }

    fn player_prefab(camera_mode: &str, index: u32) -> PrefabDef {
        PrefabDef {
            kind: PrefabKind::Actor,
            model: "char_a".to_string(),
            player_index: index,
            components: PrefabComponents {
                tags: vec!["player".to_string()],
                camera_mode: Some(ron_value::<CameraModeDef>(camera_mode)),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Loads a single-player scene (one player, so the loader gives it its own `FirstPerson`
    /// camera: with two players and no split/party block it would fall back to one shared Orbit
    /// camera) using prefab `fp_hidden` (`hide_own_body: true`) or `fp_visible`, with a `far`
    /// Fixed camera preset in the registry. Returns `(app, player)`.
    fn loaded_app(prefab: &str) -> (App, Entity) {
        let mut app = setup_test_app();
        pin_one_tick_per_update(&mut app);
        app.update();
        app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
            models: std::collections::BTreeMap::from([(
                "char_a".to_string(),
                ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() },
            )]),
            ..Default::default()
        }));
        app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
            prefabs: std::collections::BTreeMap::from([
                (
                    "fp_hidden".to_string(),
                    player_prefab("FirstPerson((eye_offset: (0.0, 1.6, 0.0), hide_own_body: true))", 0),
                ),
                (
                    "fp_visible".to_string(),
                    player_prefab("FirstPerson((eye_offset: (0.0, 1.6, 0.0)))", 0),
                ),
            ]),
            ..Default::default()
        }));
        let config = app.world_mut().resource_mut::<Assets<ProjectConfig>>().add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
        app.world_mut().insert_resource(ProjectConfigHandle(config));
        let scene: GameSceneV2 = ron_value(&format!(
            r#"(
                schema_version: 2,
                nameplate_options: (show_player_nameplate: true),
                camera_modes: {{ "far": Fixed((position: (0.0, 8.0, 8.0), look_at: (0.0, 0.0, 0.0))) }},
                entities: [
                    (id: "p", prefab: "{prefab}", transform: (translation: (0.0, 0.5, 0.0))),
                ],
                ui: [],
            )"#
        ));
        let handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
        app.world_mut().insert_resource(SceneHandleV2(handle));
        app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LoadingScene);
        for _ in 0..4 {
            app.update();
        }
        let player = app
            .world_mut()
            .query::<(Entity, &ironhold_core::runtime::SpawnId)>()
            .iter(app.world())
            .find(|(_, s)| s.0 == "p")
            .map(|(e, _)| e)
            .expect("player p spawned");
        (app, player)
    }

    /// Two players whose prefab asks for `hide_own_body`, with no split/party block: the loader falls back
    /// to one shared Orbit camera, so neither body may be hidden (no first-person camera exists).
    #[test]
    fn two_hidden_body_prefabs_without_a_split_keep_their_bodies_when_the_camera_falls_back() {
        let mut app = setup_test_app();
        pin_one_tick_per_update(&mut app);
        app.update();
        app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
            models: std::collections::BTreeMap::from([(
                "char_a".to_string(),
                ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() },
            )]),
            ..Default::default()
        }));
        app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
            prefabs: std::collections::BTreeMap::from([
                ("fp_a".to_string(), player_prefab("FirstPerson((eye_offset: (0.0, 1.6, 0.0), hide_own_body: true))", 0)),
                ("fp_b".to_string(), player_prefab("FirstPerson((eye_offset: (0.0, 1.6, 0.0), hide_own_body: true))", 1)),
            ]),
            ..Default::default()
        }));
        let config = app.world_mut().resource_mut::<Assets<ProjectConfig>>().add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
        app.world_mut().insert_resource(ProjectConfigHandle(config));
        let scene: GameSceneV2 = ron_value(
            r#"(
                schema_version: 2,
                entities: [
                    (id: "a", prefab: "fp_a", transform: (translation: (-2.0, 0.5, 0.0))),
                    (id: "b", prefab: "fp_b", transform: (translation: (2.0, 0.5, 0.0))),
                ],
                ui: [],
            )"#,
        );
        let handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
        app.world_mut().insert_resource(SceneHandleV2(handle));
        app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LoadingScene);
        for _ in 0..4 {
            app.update();
        }
        let mut players = app.world_mut().query_filtered::<(Entity, Option<&Visibility>), With<CharacterController>>();
        let rows: Vec<_> = players.iter(app.world()).map(|(e, v)| (e, v.copied())).collect();
        assert_eq!(rows.len(), 2, "both players spawned");
        for (e, v) in rows {
            assert_ne!(v, Some(Visibility::Hidden), "player {e:?}: no first-person camera spawned, so its body must stay visible");
            assert!(app.world().get::<OwnBodyHidden>(e).is_none());
        }
        let first_person_cameras = app
            .world_mut()
            .query_filtered::<(), With<FirstPersonCameraMode>>()
            .iter(app.world())
            .count();
        assert_eq!(first_person_cameras, 0, "precondition: the loader really fell back to a shared non-first-person camera");
    }

    /// `SetEntityVisible` is the designer's explicit word: it clears the engine's own hide marker, so a later
    /// camera switch does not undo it.
    #[test]
    fn set_entity_visible_clears_the_hide_own_body_marker() {
        let (mut app, hidden) = loaded_app("fp_hidden");
        assert!(app.world().get::<OwnBodyHidden>(hidden).is_some(), "precondition");
        app.world_mut().resource_mut::<ActionQueue>().push(Action::SetEntityVisible {
            entity: "p".to_string(),
            visible: false,
        });
        for _ in 0..3 {
            app.update();
        }
        assert!(app.world().get::<OwnBodyHidden>(hidden).is_none(), "the explicit visibility takes over from the marker");
        switch_camera(&mut app, "far");
        assert_eq!(
            app.world().get::<Visibility>(hidden),
            Some(&Visibility::Hidden),
            "leaving first person must not un-hide a body the designer hid explicitly"
        );
    }

    fn switch_camera(app: &mut App, mode: &str) {
        app.world_mut().resource_mut::<ActionQueue>().push(Action::SetCameraMode {
            mode: mode.to_string(),
            owner_player: Some(0),
        });
        for _ in 0..3 {
            app.update();
        }
    }

    #[test]
    fn the_body_is_hidden_from_the_first_frame_and_carries_no_floating_widgets() {
        let (app, hidden) = loaded_app("fp_hidden");
        assert_eq!(app.world().get::<Visibility>(hidden), Some(&Visibility::Hidden), "hide_own_body hides the player root");
        assert!(app.world().get::<OwnBodyHidden>(hidden).is_some());
        assert!(app.world().get::<NameplateTag>(hidden).is_none(), "a hidden body gets no nameplate");
    }

    #[test]
    fn an_ordinary_first_person_player_is_untouched() {
        let (app, visible) = loaded_app("fp_visible");
        assert_ne!(app.world().get::<Visibility>(visible), Some(&Visibility::Hidden));
        assert!(app.world().get::<OwnBodyHidden>(visible).is_none());
        assert!(
            app.world().get::<NameplateTag>(visible).is_some(),
            "control: the same scene gives a player without hide_own_body its nameplate"
        );
    }

    #[test]
    fn leaving_first_person_unhides_the_body_and_returning_hides_it_again() {
        let (mut app, hidden) = loaded_app("fp_hidden");
        switch_camera(&mut app, "far");
        assert_ne!(app.world().get::<Visibility>(hidden), Some(&Visibility::Hidden), "a non-hiding camera shows the body");
        assert!(app.world().get::<OwnBodyHidden>(hidden).is_none());

        switch_camera(&mut app, "default");
        assert_eq!(app.world().get::<Visibility>(hidden), Some(&Visibility::Hidden), "back in first person the body is hidden again");
        assert!(app.world().get::<OwnBodyHidden>(hidden).is_some());
    }

    #[test]
    fn a_body_the_designer_hid_is_not_unhidden_by_a_camera_switch() {
        let (mut app, player) = loaded_app("fp_visible");
        // The camera does not hide the body; the designer hid it with SetEntityVisible.
        app.world_mut().entity_mut(player).insert(Visibility::Hidden);
        switch_camera(&mut app, "far");
        assert_eq!(
            app.world().get::<Visibility>(player),
            Some(&Visibility::Hidden),
            "no OwnBodyHidden marker, so the engine must not un-hide it"
        );
    }
}
