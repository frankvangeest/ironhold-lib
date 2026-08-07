//! `gamepad_player_binding_hardening.md`: `BoundGamepad`/`gamepad_bind_system` — entity-resolved
//! gamepad binding, replacing the old live positional `InputMap.gamepad_index` re-resolution.
//!
//! Split out as its own file (not appended to `local_coop_tests.rs`) because this is a
//! feature-sized batch of new tests centered on one dedicated production system
//! (`gamepad_bind_system`, `runtime/input.rs`) and one dedicated component (`BoundGamepad`,
//! `capabilities/player.rs`) — mirrors how `fsm_tests.rs`..`ui_tests.rs` were originally split out
//! by domain (see `tests/CLAUDE.md`'s file-layout table and the `split_integration_tests.md`
//! plan). `local_coop_tests.rs` already has extensive pre-existing gamepad coverage (hot-join,
//! per-player action bar slots, gamepad camera look) that needed fixing in place for this
//! refactor (missing `BoundGamepad` insertions, the deleted `OrbitCamera.gamepad_index` field) —
//! those fixes stay there; this file is for the new binding-lifecycle behavior itself.

use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::input::gamepad::{
    Gamepad, GamepadButton, GamepadConnection, GamepadConnectionEvent,
};
use ironhold_core::runtime::{
    SceneHandleV2, LoadedAssetCatalog, LoadedPrefabCatalog,
    InputAction, InputActionMessage, gamepad_bind_system, input_translator_system,
    unclaimed_gamepad_trigger_system,
};
use ironhold_core::runtime::scene_manager::{
    LoadedGamepadBindings, PendingJoinGamepad, PendingEntitySpawns, QueuedSpawn,
};
use ironhold_core::schema::{AppState, ProjectConfig, ProjectConfigHandle, GameSceneV2};
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, PrefabKind, ModelCatalogEntry, PrefabComponents, MovementConfig};
use ironhold_core::schema::player::{InputMap, CameraConfig, PlayerModelSource, PlayerConfig};
use ironhold_core::capabilities::player::{CharacterController, PlayerIndex, BoundGamepad};

mod support;
use support::{setup_test_app, connect_test_gamepad, press_gamepad_button};

/// Mirrors the production `GAMEPAD_STABLE_CONNECT_SECS` constant in `runtime/input.rs` (private to
/// that module, not exported) — a candidate gamepad must be continuously present for at least
/// this long before `gamepad_bind_system` will commit a binding to it (added after a real hardware
/// finding during this feature's own playtest: a spurious duplicate gamepad entry can otherwise
/// win a binding in the brief window before it disappears). Advances the `Time` resource directly
/// so `gamepad_bind_system`'s next `run_system_once` call sees exactly this much elapsed time,
/// deterministically, with no real sleep.
fn advance_past_stability_window(app: &mut App) {
    app.world_mut().resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs_f32(0.6));
}

fn test_camera_config() -> CameraConfig {
    CameraConfig {
        offset: (0.0, 5.0, 10.0),
        look_at_offset: (0.0, 2.0, 0.0),
        zoom_speed: 10.0,
        orbit_speed: 0.5,
        min_radius: 4.0,
        max_radius: 20.0,
        min_pitch: 0.1,
        max_pitch: 0.9,
        orbit_button: "Either".to_string(),
        character_rotate_button: None,
        initial_pitch: 0.5,
        initial_yaw: 0.0,
        party: None,
        split: None,
        look_speed: 2.0,
        fov: 60.0,
    }
}

/// Builds a minimal but fully-valid `PlayerConfig` for hand-constructing a `QueuedSpawn` — only
/// used to simulate an in-flight `is_hot_join` entry sitting in `PendingEntitySpawns` (production
/// code always builds these via the private `assemble_player_config`). Mirrors
/// `local_coop_tests.rs`'s identically-named helper (each integration test file is its own
/// compilation unit, so it can't be shared directly).
fn minimal_player_config(gamepad_index: Option<usize>, player_index: u32, bound_gamepad: Option<Entity>) -> PlayerConfig {
    PlayerConfig {
        model_source: PlayerModelSource::Glb("char_a".to_string()),
        initial_position: (0.0, 0.5, 0.0),
        camera: test_camera_config(),
        camera_mode: None,
        split: None,
        party: None,
        inputs: InputMap { gamepad_index, ..test_input_map() },
        animation_policy: None,
        movement: MovementConfig::default(),
        spawn_id: "in_flight_test".to_string(),
        prefab_key: "test_player_3".to_string(),
        nameplate_display_name: None,
        nameplate_override: None,
        player_index,
        bound_gamepad,
        material: None,
        stat_templates: vec![],
        stat_label: None,
        world_stat_bar: None,
    }
}

fn test_input_map() -> InputMap {
    InputMap {
        forward: "KeyW".to_string(), backward: "KeyS".to_string(),
        left: "KeyA".to_string(), right: "KeyD".to_string(),
        strafe_left: "KeyQ".to_string(), strafe_right: "KeyE".to_string(),
        jump: "Space".to_string(), run: "ShiftLeft".to_string(),
        interact: "KeyF".to_string(), strafe_mouse_button: None,
        target_next: "Tab".to_string(), target_range: 30.0,
        gamepad_index: None, look_left: None, look_right: None, look_up: None, look_down: None,
        gamepad_jump: "South".to_string(), gamepad_run: "East".to_string(),
        gamepad_interact: "West".to_string(), gamepad_target_next: "North".to_string(),
        gamepad_deadzone: 0.15,
    }
}

fn test_character_controller() -> CharacterController {
    CharacterController {
        walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
        inputs: test_input_map(),
        is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
        double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
        collider_radius: 0.4, ground_cast_length: 0.3, idle_drag: 0.8,
    }
}

// ── Scenario 1: regression — no gamepad_index authored anywhere ────────────────────────────

/// Sets up a minimal 2-player catalog with no `gamepad_index` authored on either player prefab
/// (`PrefabComponents::default()` — no `inputs` block at all) and no `camera` block (the scene
/// loader falls back to a single shared camera, same as `two_player_catalogs(None)` elsewhere).
fn two_player_catalogs_no_gamepad_index(app: &mut App) {
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
            ("char_b".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-female-01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("test_player_1".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "char_a".to_string(),
                player_index: 0,
                components: PrefabComponents {
                    tags: vec!["player".to_string()],
                    ..Default::default()
                },
                ..Default::default()
            }),
            ("test_player_2".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "char_b".to_string(),
                player_index: 1,
                components: PrefabComponents {
                    tags: vec!["player".to_string()],
                    ..Default::default()
                },
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));
}

/// Drives a Replace-mode load of a two-player scene, mirroring `local_coop_tests.rs`'s
/// `load_two_player_scene`/`scene_lifecycle_tests.rs`'s `drive_replace_load` pattern.
fn load_two_player_scene(app: &mut App) {
    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let scene: GameSceneV2 = ron::de::from_str(r#"(
        schema_version: 2,
        entities: [
            (id: "p1", prefab: "test_player_1", transform: (translation: (-4.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
            (id: "p2", prefab: "test_player_2", transform: (translation: (4.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
        ],
        ui: [],
    )"#).unwrap();
    let scene_handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadingScene);
    app.update(); // state transitions to LoadingScene
    app.update(); // spawn_scene_v2 fires
    app.update(); // commands flushed
}

/// Regression: a scene with no `gamepad_index` authored on any player must behave exactly as
/// before this feature existed — both players get a `BoundGamepad` component (inserted
/// unconditionally at spawn) but it never resolves to `Some`, regardless of any gamepad
/// connecting or disconnecting mid-session, and keyboard movement is completely unaffected.
#[test]
fn test_no_gamepad_index_authored_players_stay_unbound_and_keyboard_still_works() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_no_gamepad_index(&mut app);
    load_two_player_scene(&mut app);

    let player_count = app.world_mut().query::<&BoundGamepad>().iter(app.world()).count();
    assert_eq!(player_count, 2, "both players must have a BoundGamepad component (inserted unconditionally at spawn)");

    // An unrelated gamepad connects, then disconnects, mid-session.
    let gamepad = connect_test_gamepad(&mut app);
    app.update();
    app.update();
    app.world_mut()
        .resource_mut::<Messages<GamepadConnectionEvent>>()
        .write(GamepadConnectionEvent::new(gamepad, GamepadConnection::Disconnected));
    app.update();
    app.update();

    let all_still_none = app.world_mut().query::<&BoundGamepad>().iter(app.world()).all(|b| b.0.is_none());
    assert!(
        all_still_none,
        "no player authored a gamepad_index seed — BoundGamepad must never resolve to Some, \
         regardless of any gamepad connecting/disconnecting mid-session"
    );

    // Keyboard movement is completely unaffected. Called directly (not via app.update()'s
    // real FixedUpdate scheduling, whose exact per-call timestep is not deterministic in a
    // headless test loop) — mirrors this file's other direct `run_system_once` calls and
    // `trigger_zone_system`'s established pattern in `local_coop_tests.rs`.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW);
    app.world_mut().run_system_once(input_translator_system).unwrap();

    let move_fired = app.world()
        .resource::<Messages<InputActionMessage>>()
        .iter_current_update_messages()
        .any(|m| matches!(m.action, InputAction::Move(v) if v.y > 0.0));
    assert!(move_fired, "keyboard movement must still fire exactly as before when no gamepad_index is authored anywhere");
}

// ── Scenario 3: already-connected gamepad binds once stable and stays bound ────────────────

/// A player spawned with a gamepad already connected binds once that pad has been stable for
/// `GAMEPAD_STABLE_CONNECT_SECS`, and stays bound to that exact `Entity` even after an unrelated
/// *other* pad disconnects and reconnects mid-session — the core "no silent re-routing" guarantee
/// this hardening exists for.
#[test]
fn test_player_binds_once_stable_and_stays_bound_through_unrelated_pad_churn() {
    let mut app = setup_test_app();
    app.update();

    let gp_a = connect_test_gamepad(&mut app);
    let gp_b = connect_test_gamepad(&mut app);
    app.update();
    let mut sorted = [gp_a, gp_b];
    sorted.sort_by_key(|e| e.index());
    let seed0_pad = sorted[0];
    let other_pad = sorted[1];

    let mut inputs = test_input_map();
    inputs.gamepad_index = Some(0);
    let player = app.world_mut().spawn((
        CharacterController { inputs, ..test_character_controller() },
        PlayerIndex(0),
        BoundGamepad::default(),
    )).id();

    // Not yet stable on the very first pass — the debounce window hasn't elapsed.
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, None,
        "must not bind before the pad has been stable for GAMEPAD_STABLE_CONNECT_SECS"
    );

    advance_past_stability_window(&mut app);
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, Some(seed0_pad),
        "player must bind once the pad occupying sorted position 0 has been stable long enough"
    );

    // The OTHER pad (not the one this player is bound to) disconnects, then reconnects.
    app.world_mut()
        .resource_mut::<Messages<GamepadConnectionEvent>>()
        .write(GamepadConnectionEvent::new(other_pad, GamepadConnection::Disconnected));
    app.update();
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, Some(seed0_pad),
        "an unrelated pad disconnecting must never change this player's existing binding"
    );

    app.world_mut()
        .resource_mut::<Messages<GamepadConnectionEvent>>()
        .write(GamepadConnectionEvent::new(other_pad, GamepadConnection::Connected {
            name: "Test Gamepad".to_string(), vendor_id: None, product_id: None,
        }));
    app.update();
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, Some(seed0_pad),
        "the unrelated pad reconnecting must not re-route this player's binding either"
    );
}

// ── Scenario 4: late-connecting pad — pending retry ─────────────────────────────────────────

/// A player spawned *before* their gamepad connects stays pending (no panic, `BoundGamepad`
/// stays `None`) and successfully binds once the matching pad connects later — the pending-bind
/// retry this hardening adds specifically so a player who spawns before plugging in their
/// controller isn't permanently stuck keyboard-only.
#[test]
fn test_player_spawned_before_gamepad_connects_binds_once_it_does() {
    let mut app = setup_test_app();
    app.update();

    let mut inputs = test_input_map();
    inputs.gamepad_index = Some(0);
    let player = app.world_mut().spawn((
        CharacterController { inputs, ..test_character_controller() },
        PlayerIndex(0),
        BoundGamepad::default(),
    )).id();

    // No gamepad connected yet — must stay pending, no panic.
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, None,
        "must stay pending with no connected gamepad — this is the ordinary silent keyboard-only case"
    );

    // Now the gamepad connects.
    let gamepad = connect_test_gamepad(&mut app);
    app.update();
    advance_past_stability_window(&mut app);
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, Some(gamepad),
        "the pending player must bind once a matching gamepad connects and has been stable long enough"
    );
}

// ── Scenario 5: disconnect/reconnect of the SAME Entity resumes automatically ───────────────

/// A disconnected-then-reconnected *same* physical pad's bound player automatically resumes
/// receiving input with zero extra code. Bevy never despawns a disconnected gamepad's `Entity` —
/// only the `Gamepad` component is removed, and re-inserted on the *same* `Entity` on reconnect
/// (`bevy_input`'s own doc comment: "Entities are left alive... we remove Gamepad components...
/// and re-add them if they ever reconnect") — simulated here by removing/re-inserting `Gamepad`
/// via the real `GamepadConnectionEvent`/`gamepad_connection_system` pipeline on the same
/// `Entity` id, not by despawning/respawning it.
#[test]
fn test_bound_player_resumes_gamepad_input_after_disconnect_reconnect_same_entity() {
    let mut app = setup_test_app();
    app.update();
    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    let mut inputs = test_input_map();
    inputs.gamepad_index = Some(0);
    let player = app.world_mut().spawn((
        CharacterController { inputs, ..test_character_controller() },
        PlayerIndex(0),
        BoundGamepad::default(),
    )).id();

    advance_past_stability_window(&mut app);
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, Some(gamepad),
        "sanity: must bind once stable"
    );

    // Confirm gamepad input actually works before any disconnect (gamepad_jump defaults to
    // "South" — see test_input_map). A real `app.update()` (not `run_system_once`) is required
    // here first: `press_gamepad_button` only queues a raw event — `gamepad_event_processing_system`
    // (PreUpdate) must actually run to translate it into the `Gamepad` component's digital state
    // before anything can read `just_pressed` on it; `run_system_once` bypasses PreUpdate entirely.
    // The subsequent `run_system_once(input_translator_system)` then deterministically exercises
    // the consumer itself, sidestepping FixedUpdate's own per-call timestep uncertainty.
    press_gamepad_button(&mut app, gamepad, GamepadButton::South);
    app.update();
    app.world_mut().run_system_once(input_translator_system).unwrap();
    let jump_before = app.world()
        .resource::<Messages<InputActionMessage>>()
        .iter_current_update_messages()
        .any(|m| m.entity == player && matches!(m.action, InputAction::Jump(true)));
    assert!(jump_before, "sanity: gamepad jump must fire before any disconnect");

    // Disconnect the pad — Bevy only removes the Gamepad component; the Entity itself survives.
    app.world_mut()
        .resource_mut::<Messages<GamepadConnectionEvent>>()
        .write(GamepadConnectionEvent::new(gamepad, GamepadConnection::Disconnected));
    app.update();
    assert!(app.world().get::<Gamepad>(gamepad).is_none(), "sanity: Gamepad component must be removed on disconnect");
    assert!(app.world().get_entity(gamepad).is_ok(), "sanity: the gamepad Entity itself must NOT be despawned");

    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, Some(gamepad),
        "a disconnected pad's bound player keeps their existing binding — locked to that Entity \
         forever, never cleared just because the pad went away"
    );

    // Reconnect the SAME Entity — exactly how Bevy really does it (gamepad_connection_system
    // re-inserts Gamepad onto the same Entity when the same device/GamepadId reconnects).
    app.world_mut()
        .resource_mut::<Messages<GamepadConnectionEvent>>()
        .write(GamepadConnectionEvent::new(gamepad, GamepadConnection::Connected {
            name: "Test Gamepad".to_string(), vendor_id: None, product_id: None,
        }));
    app.update();
    assert!(app.world().get::<Gamepad>(gamepad).is_some(), "sanity: Gamepad component must be re-inserted on reconnect");

    // No rebinding call needed at all — input_translator_system reads `bound.0` straight through
    // to `gamepad_query`; the component simply exists again. Same real-`app.update()`-then-
    // `run_system_once` two-step as the "before" check above, same reason.
    press_gamepad_button(&mut app, gamepad, GamepadButton::South);
    app.update();
    app.world_mut().run_system_once(input_translator_system).unwrap();
    let jump_after = app.world()
        .resource::<Messages<InputActionMessage>>()
        .iter_current_update_messages()
        .any(|m| m.entity == player && matches!(m.action, InputAction::Jump(true)));
    assert!(
        jump_after,
        "the bound player's gamepad input must resume automatically after reconnect to the same \
         Entity, with zero rebinding code"
    );
}

// ── Scenario 6: cross-time double-bind race ─────────────────────────────────────────────────

/// The cross-*time* double-bind race (`gamepad_player_binding_hardening.md`'s B1 fix, the
/// blocker the system-architect review found): pad B connects first and binds to P1 (seed 0);
/// P2 (seed 1) is out of range and stays pending. Later, pad A connects and happens to receive a
/// *lower* `Entity::index()` than B — simulated here via the same free-slot-reuse mechanism that
/// makes this race possible in a real running game (any unrelated entity despawning between B's
/// and A's connection can free a low index slot A's later connection then reuses; Bevy's entity
/// allocator recycles freed indices). The sorted slice becomes `[A, B]`, so P2's seed 1 would
/// naively now resolve to B — already bound to P1. `gamepad_bind_system`'s `claimed` invariant
/// (built fresh from every already-bound player at the start of each pass) must prevent this: P2
/// must stay pending, never steal P1's pad.
#[test]
fn test_cross_time_race_never_double_binds_a_pad_already_held() {
    let mut app = setup_test_app();
    app.update();

    // Reserve a low-index slot to free up later, so a subsequently-connected gamepad can
    // legitimately end up with a LOWER Entity::index() than one already connected.
    let reserved_low_slot = app.world_mut().spawn_empty().id();

    let gp_b = connect_test_gamepad(&mut app);
    app.update();
    assert!(
        gp_b.index() > reserved_low_slot.index(),
        "sanity: B must not already occupy the reserved low slot"
    );

    let mut p1_inputs = test_input_map();
    p1_inputs.gamepad_index = Some(0);
    let p1 = app.world_mut().spawn((
        CharacterController { inputs: p1_inputs, ..test_character_controller() },
        PlayerIndex(0),
        BoundGamepad::default(),
    )).id();

    let mut p2_inputs = test_input_map();
    p2_inputs.gamepad_index = Some(1);
    let p2 = app.world_mut().spawn((
        CharacterController { inputs: p2_inputs, ..test_character_controller() },
        PlayerIndex(1),
        BoundGamepad::default(),
    )).id();

    // Only one pad (B) exists — P1 (seed 0) binds to it once stable; P2 (seed 1) is out of range, stays pending.
    advance_past_stability_window(&mut app);
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(app.world().get::<BoundGamepad>(p1).unwrap().0, Some(gp_b), "P1 must bind to the only connected pad");
    assert_eq!(app.world().get::<BoundGamepad>(p2).unwrap().0, None, "P2 must stay pending — seed 1 is out of range with only one pad connected");

    // Free the reserved low slot, then connect pad A — it reuses that freed slot, ending up with
    // a LOWER Entity::index() than B.
    app.world_mut().despawn(reserved_low_slot);
    let gp_a = connect_test_gamepad(&mut app);
    app.update();
    assert!(
        gp_a.index() < gp_b.index(),
        "test setup requires gamepad A to end up with a lower Entity::index() than B (got A={:?}, \
         B={:?}) — if this fails, Bevy's entity-slot-reuse allocator behavior this test relies on \
         may have changed",
        gp_a, gp_b
    );

    // The sorted slice is now [A, B]. P2's seed 1 now naively resolves to B — already bound to
    // P1. The cross-player invariant must prevent P2 from stealing it.
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(p2).unwrap().0, None,
        "P2 must NOT bind to B just because a newly-connected pad A shifted the sorted slice — \
         B is already held by P1"
    );
    assert_eq!(
        app.world().get::<BoundGamepad>(p1).unwrap().0, Some(gp_b),
        "P1's own binding must be completely unaffected by the new pad connecting"
    );
}

// ── Scenario 9: same-frame duplicate-seed race ──────────────────────────────────────────────

/// Two players authored with the same duplicated `gamepad_index` must never both bind to the
/// same pad in a single `gamepad_bind_system` pass — `claimed` grows within the same pass, so
/// only the first-iterated player wins; the other stays pending (diagnosed by the scene-load
/// `warn!`/`ironhold_cli validate` hard error — see `duplicate_gamepad_index_same_scene_exits_1`
/// in `ironhold_cli`'s `validate_cross_file.rs` — not silently dual-controlled at runtime).
/// Exercises `gamepad_bind_system` directly (not a full scene load) for a fast, deterministic
/// unit check of the underlying invariant the CLI/warn! diagnostics exist to explain.
#[test]
fn test_duplicate_gamepad_index_seed_never_double_binds_within_one_pass() {
    let mut app = setup_test_app();
    app.update();
    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    let mut p1_inputs = test_input_map();
    p1_inputs.gamepad_index = Some(0);
    let p1 = app.world_mut().spawn((
        CharacterController { inputs: p1_inputs, ..test_character_controller() },
        PlayerIndex(0),
        BoundGamepad::default(),
    )).id();

    let mut p2_inputs = test_input_map();
    p2_inputs.gamepad_index = Some(0); // deliberately duplicated
    let p2 = app.world_mut().spawn((
        CharacterController { inputs: p2_inputs, ..test_character_controller() },
        PlayerIndex(1),
        BoundGamepad::default(),
    )).id();

    advance_past_stability_window(&mut app);
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();

    let p1_bound = app.world().get::<BoundGamepad>(p1).unwrap().0;
    let p2_bound = app.world().get::<BoundGamepad>(p2).unwrap().0;

    let bound_count = [p1_bound, p2_bound].iter().filter(|b| **b == Some(gamepad)).count();
    assert_eq!(
        bound_count, 1,
        "exactly one of the two players sharing gamepad_index: 0 must bind to the pad, never \
         both (got p1={:?}, p2={:?})", p1_bound, p2_bound
    );
    assert!(
        p1_bound == Some(gamepad) || p2_bound == Some(gamepad),
        "at least one of them must still bind — a duplicate seed shouldn't leave the pad \
         completely unclaimed by anyone"
    );

    // A second pass must not change the outcome — the loser stays pending forever (there's no
    // different free pad here for it to fall back to anyway; both seeds point at the same one).
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    let p1_bound_2 = app.world().get::<BoundGamepad>(p1).unwrap().0;
    let p2_bound_2 = app.world().get::<BoundGamepad>(p2).unwrap().0;
    assert_eq!(
        (p1_bound_2, p2_bound_2), (p1_bound, p2_bound),
        "a second bind pass must not change either player's outcome"
    );
}

// ── Post-implementation-review findings: hot-join in-flight races ──────────────────────────

/// system-architect / debug-detective finding (post-implementation review):
/// `gamepad_bind_system`'s `claimed` set originally only looked at live players' `BoundGamepad`,
/// unlike `unclaimed_gamepad_trigger_system`'s equivalent set, which also chains in undrained
/// `is_hot_join` `PendingEntitySpawns` entries. A hot-joined player can sit in that queue for one
/// or more frames (`drain_spawn_queue_system` is rate-limited), so without this, a live pending
/// player whose seed resolves to the same pad the in-flight joiner already captured could bind to
/// it in that window — producing two live players on one controller once the joiner actually
/// spawns. Regression: a pending player must stay pending while any undrained hot-join entry has
/// already captured the pad their own seed would otherwise resolve to.
#[test]
fn test_gamepad_bind_system_never_steals_a_pad_claimed_by_an_in_flight_hot_join() {
    let mut app = setup_test_app();
    app.update();
    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    let mut inputs = test_input_map();
    inputs.gamepad_index = Some(0);
    let player = app.world_mut().spawn((
        CharacterController { inputs, ..test_character_controller() },
        PlayerIndex(0),
        BoundGamepad::default(),
    )).id();

    // An undrained is_hot_join spawn already captured this exact pad directly via
    // PlayerConfig.bound_gamepad — no positional round-trip, mirroring the real
    // Action::JoinPlayer hand-off.
    app.world_mut().resource_mut::<PendingEntitySpawns>().0.push_back(QueuedSpawn {
        prefab_def: PrefabDef::default(),
        model_path: String::new(),
        transform: Transform::IDENTITY,
        spawn_id: "in_flight_test".to_string(),
        prefab_key: "test_player_3".to_string(),
        project_root: String::new(),
        player_config: Some(minimal_player_config(None, 2, Some(gamepad))),
        is_hot_join: true,
    });

    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, None,
        "a pad already captured by an undrained is_hot_join PendingEntitySpawns entry must never \
         be bound to a different, still-pending live player — that would leave two live players \
         bound to the same controller once the in-flight join actually spawns"
    );
}

/// debug-detective finding (post-implementation review): `unclaimed_gamepad_trigger_system` runs
/// in `Update` every frame, but `gamepad_bind_system` (the only writer of `BoundGamepad`) runs in
/// `FixedUpdate`, whose accumulator can tick zero times in a given frame. On the exact frame a pad
/// first becomes visible (its first press — also this system's join-trigger frame on the web), a
/// live player whose authored seed resolves to that pad would otherwise look unclaimed for one
/// frame (their `BoundGamepad` is still `None`, since `gamepad_bind_system` hasn't run yet this
/// frame) and lose it to a spurious join. Regression: a press on a pad a live-but-still-pending
/// player's seed resolves to must never trigger a join, even though that player's `BoundGamepad`
/// is still `None` at the moment of the press.
#[test]
fn test_unclaimed_gamepad_trigger_reserves_pad_for_a_still_pending_authored_player() {
    let mut app = setup_test_app();
    app.update();
    // This system early-returns unless AppState::InGame — inserted directly rather than driving
    // a full scene load, since this test only needs the trigger system itself, not the spawn
    // pipeline.
    app.world_mut().insert_resource(State::new(AppState::InGame));
    app.world_mut().insert_resource(LoadedGamepadBindings(std::collections::HashMap::from([
        ("South".to_string(), "join".to_string()),
    ])));

    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    // A live player authored with gamepad_index: 0, still pending — as if this frame's
    // gamepad_bind_system (FixedUpdate) simply hasn't run yet.
    let mut inputs = test_input_map();
    inputs.gamepad_index = Some(0);
    app.world_mut().spawn((
        CharacterController { inputs, ..test_character_controller() },
        PlayerIndex(0),
        BoundGamepad::default(),
    ));

    press_gamepad_button(&mut app, gamepad, GamepadButton::South);
    app.world_mut().run_system_once(unclaimed_gamepad_trigger_system).unwrap();

    assert_eq!(
        app.world().resource::<PendingJoinGamepad>().0, None,
        "a pad a still-pending live player's own gamepad_index seed resolves to must never be \
         treated as unclaimed, even before gamepad_bind_system has actually bound it this frame"
    );
}

// ── Real-hardware playtest finding: transient ghost-pad connect-then-disconnect ────────────

/// Real hardware finding (playtest, 2026-08-05): a single physical Xbox controller can register
/// as **two** separate browser gamepad entries for a brief moment — the spurious entry wins the
/// lower sorted position (it's discovered first) and disconnects on its own shortly after. Before
/// `GAMEPAD_STABLE_CONNECT_SECS` existed, `gamepad_bind_system` would commit to whichever pad
/// occupied a seed's sorted position the very first time it saw one there — permanently locking a
/// player onto the spurious entry the instant it appeared, before it had a chance to vanish. This
/// reproduced on *every* connection attempt with the affected hardware (the spurious entry
/// reliably won position 0 and reliably outlived at least one bind-system tick), not just as an
/// occasional unlucky race — restarting the scene did not help, since the same sequence repeated
/// identically on every fresh session. Regression: a pad that appears and disappears well within
/// the stability window must never be bound to; the player must instead end up bound to whichever
/// pad is still present once the window elapses.
#[test]
fn test_transient_ghost_pad_never_strands_player_away_from_the_real_pad() {
    let mut app = setup_test_app();
    app.update();

    let ghost = connect_test_gamepad(&mut app);
    app.update();

    let mut inputs = test_input_map();
    inputs.gamepad_index = Some(0);
    let player = app.world_mut().spawn((
        CharacterController { inputs, ..test_character_controller() },
        PlayerIndex(0),
        BoundGamepad::default(),
    )).id();

    // gamepad_bind_system sees the ghost, but it hasn't been stable long enough yet — must not
    // bind to it (this is the exact tick that would have permanently locked onto it pre-fix).
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, None,
        "must not bind to a freshly-appeared pad before it has been stable for GAMEPAD_STABLE_CONNECT_SECS"
    );

    // The ghost disconnects on its own, well within the stability window — advance only a small,
    // sub-threshold amount of time first, mirroring how quickly the real hardware's spurious
    // entry vanished.
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.1));
    app.world_mut()
        .resource_mut::<Messages<GamepadConnectionEvent>>()
        .write(GamepadConnectionEvent::new(ghost, GamepadConnection::Disconnected));
    app.update();
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, None,
        "must still be pending — the only pad seen so far vanished before ever becoming stable"
    );

    // The real controller connects.
    let real_pad = connect_test_gamepad(&mut app);
    app.update();

    advance_past_stability_window(&mut app);
    app.world_mut().run_system_once(gamepad_bind_system).unwrap();
    assert_eq!(
        app.world().get::<BoundGamepad>(player).unwrap().0, Some(real_pad),
        "the player must end up bound to the real, stable pad — never permanently stranded on \
         the vanished ghost entry"
    );
}
