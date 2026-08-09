// Integration tests for camera_modes v2 (planning/features/camera_modes.md):
// Action::SetCameraMode, the camera_modes: registry, CameraBlendState transitions, and their
// interaction with dynamic split-screen and hot-join. v1 (mode unification) is covered by
// local_coop_tests.rs; this file is scoped to what v2 added on top.

use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;

use ironhold_core::runtime::{ActionQueue, LoadedAssetCatalog, LoadedPrefabCatalog, SceneHandleV2};
use ironhold_core::schema::{AppState, ProjectConfig, ProjectConfigHandle, GameSceneV2, Action};
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, PrefabKind, ModelCatalogEntry, PrefabComponents};
use ironhold_core::schema::player::{CameraConfig, SplitScreenDef, SplitOrientation, PartyZoomDef, InputMap};
use ironhold_core::schema::camera::{CameraModeDef, EaseKind};
use ironhold_core::capabilities::camera::{
    ActiveCameraMode, CameraTargets, AuthoredCameraMode, CameraModeOverride, CameraBlendState,
    OrbitCameraMode, FixedCameraMode, PartyCameraMode, SplitViewportSlot,
    camera_blend_system, dynamic_split_screen_system,
};

mod support;
use support::setup_test_app;

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

fn base_camera_config() -> CameraConfig {
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
        transition: None,
    }
}

// ── Scene-load helpers ──────────────────────────────────────────────────────────

/// Registers one Orbit-mode player prefab (`camera_mode:`, not the legacy `camera:` field, per
/// v2's own demo convention) plus a `camera_modes:` registry entry loaded via `camera_modes_ron`
/// (raw RON for the `{ "key": ... }` map body, or empty string for none).
fn one_player_catalogs(app: &mut App) {
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
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
                    camera_mode: Some(CameraModeDef::Orbit(base_camera_config())),
                    inputs: Some(test_input_map()),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));
}

fn load_one_player_scene(app: &mut App, camera_modes_ron: &str) {
    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let ron_str = format!(
        r#"#![enable(implicit_some)]
        (
            schema_version: 2,
            entities: [
                (id: "p1", prefab: "test_player_1", transform: (translation: (0.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
            ],
            ui: [],
            camera_modes: {{ {camera_modes_ron} }},
        )"#
    );
    let scene: GameSceneV2 = ron::de::from_str(&ron_str).unwrap();
    let scene_handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadingScene);
    app.update(); // state transitions to LoadingScene
    app.update(); // spawn_scene_v2 fires
    app.update(); // commands flushed
}

fn two_player_split_catalogs(app: &mut App) {
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
            ("char_b".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-female-01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    let mut p1_camera = base_camera_config();
    p1_camera.split = Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false });
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("test_player_1".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "char_a".to_string(),
                player_index: 0,
                components: PrefabComponents {
                    tags: vec!["player".to_string()],
                    camera: Some(p1_camera),
                    inputs: Some(test_input_map()),
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
                    inputs: Some(test_input_map()),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));
}

fn load_two_player_split_scene(app: &mut App, camera_modes_ron: &str) {
    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let ron_str = format!(
        r#"#![enable(implicit_some)]
        (
            schema_version: 2,
            entities: [
                (id: "p1", prefab: "test_player_1", transform: (translation: (-4.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
                (id: "p2", prefab: "test_player_2", transform: (translation: (4.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
            ],
            ui: [],
            camera_modes: {{ {camera_modes_ron} }},
        )"#
    );
    let scene: GameSceneV2 = ron::de::from_str(&ron_str).unwrap();
    let scene_handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadingScene);
    app.update();
    app.update();
    app.update();
}

fn load_party_scene(app: &mut App) {
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
            ("char_b".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-female-01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    let mut p1_camera = base_camera_config();
    p1_camera.party = Some(PartyZoomDef { zoom_margin: 4.0, allow_manual_zoom: false });
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("test_player_1".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "char_a".to_string(),
                player_index: 0,
                components: PrefabComponents {
                    tags: vec!["player".to_string()],
                    camera: Some(p1_camera),
                    inputs: Some(test_input_map()),
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
                    inputs: Some(test_input_map()),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

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
    app.update();
    app.update();
    app.update();
}

// ── CameraBlendState (unit-level) ───────────────────────────────────────────────

#[test]
fn test_camera_blend_system_interpolates_toward_live_target_and_expires() {
    let mut app = setup_test_app();
    app.update();

    // The "live target" a per-mode system would have already written this frame — the blend
    // system interpolates the RENDERED transform from `from_translation` toward whatever
    // `Transform` currently holds, not toward a value it computes itself (Design A — see
    // camera_blend_system's own doc comment).
    let camera = app.world_mut().spawn((
        Transform::from_xyz(10.0, 0.0, 0.0),
        CameraBlendState {
            remaining: 1.0,
            duration: 1.0,
            ease: EaseKind::Linear,
            from_translation: Vec3::ZERO,
            from_rotation: Quat::IDENTITY,
            from_fov: 45.0,
            to_fov: 45.0,
        },
    )).id();

    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.5));
    app.world_mut().run_system_once(camera_blend_system).unwrap();

    let transform = app.world().get::<Transform>(camera).unwrap();
    // Halfway through a 1.0s linear blend from (0,0,0) toward (10,0,0) should land ~5.0.
    assert!(
        (transform.translation.x - 5.0).abs() < 0.5,
        "expected translation.x near 5.0 at the blend's midpoint, got {}", transform.translation.x
    );
    assert!(
        app.world().get::<CameraBlendState>(camera).is_some(),
        "blend should still be in progress"
    );

    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(1.0));
    app.world_mut().run_system_once(camera_blend_system).unwrap();

    assert!(
        app.world().get::<CameraBlendState>(camera).is_none(),
        "CameraBlendState must be removed once remaining reaches zero"
    );
}

#[test]
fn test_camera_blend_system_interpolates_fov() {
    let mut app = setup_test_app();
    app.update();

    let camera = app.world_mut().spawn((
        Transform::IDENTITY,
        Projection::Perspective(PerspectiveProjection { fov: 90.0_f32.to_radians(), ..default() }),
        CameraBlendState {
            remaining: 1.0,
            duration: 1.0,
            ease: EaseKind::Linear,
            from_translation: Vec3::ZERO,
            from_rotation: Quat::IDENTITY,
            from_fov: 40.0,
            to_fov: 90.0,
        },
    )).id();

    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.5));
    app.world_mut().run_system_once(camera_blend_system).unwrap();

    let Projection::Perspective(persp) = app.world().get::<Projection>(camera).unwrap() else {
        panic!("expected Projection::Perspective");
    };
    let fov_deg = persp.fov.to_degrees();
    assert!(
        (fov_deg - 65.0).abs() < 5.0,
        "expected FOV near the 40..90 midpoint (~65 deg) at t~0.5, got {fov_deg}"
    );
}

// ── dynamic_split_screen_system suspend/resume (unit-level) ─────────────────────

#[test]
fn test_dynamic_split_screen_system_skips_is_active_toggle_on_overridden_camera() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(ironhold_core::runtime::scene_manager::DynamicSplitConfig(Some(
        ironhold_core::schema::player::DynamicSplitDef {
            split_distance: 6.0,
            merge_distance: 3.0,
            merged_zoom_margin: 4.0,
            merged_allow_manual_zoom: false,
        },
    )));
    app.world_mut().insert_resource(ironhold_core::runtime::scene_manager::ActiveSplitScreen(None));

    let p1 = app.world_mut().spawn((CharacterController_stub(), Transform::from_xyz(-10.0, 0.0, 0.0))).id();
    let p2 = app.world_mut().spawn((CharacterController_stub(), Transform::from_xyz(10.0, 0.0, 0.0))).id();

    // Split camera A is overridden — must NOT be toggled active even though the players are far
    // enough apart to trigger a merged->split transition. Split camera B is untouched and must
    // toggle normally.
    let cam_a = app.world_mut().spawn((
        Camera { is_active: false, ..default() },
        OrbitCameraMode,
        SplitViewportSlot(0),
        CameraTargets(vec![p1]),
        CameraModeOverride,
    )).id();
    let cam_b = app.world_mut().spawn((
        Camera { is_active: false, ..default() },
        OrbitCameraMode,
        SplitViewportSlot(1),
        CameraTargets(vec![p2]),
    )).id();
    let party_cam = app.world_mut().spawn((
        Camera { is_active: true, ..default() },
        PartyCameraMode,
    )).id();

    app.world_mut().run_system_once(dynamic_split_screen_system).unwrap();

    assert!(!app.world().get::<Camera>(cam_a).unwrap().is_active, "overridden split camera must stay inactive, not follow the automatic merge/split toggle");
    assert!(app.world().get::<Camera>(cam_b).unwrap().is_active, "non-overridden split camera must activate on the merged->split transition");
    assert!(!app.world().get::<Camera>(party_cam).unwrap().is_active, "party camera (not overridden) must deactivate on the merged->split transition");
}

// A tiny stand-in so this file doesn't need to build a full CharacterController just to give
// dynamic_split_screen_system's `transforms: Query<&Transform>` something to query against —
// only `Transform` matters to that system, but it's spawned via a real CharacterController-typed
// helper name for readability at the call sites above.
#[allow(non_snake_case)]
fn CharacterController_stub() -> ironhold_core::capabilities::player::CharacterController {
    ironhold_core::capabilities::player::CharacterController {
        walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
        inputs: test_input_map(),
        is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
        double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
        collider_radius: 0.4, ground_cast_length: 0.3, idle_drag: 0.8,
    }
}

// ── Action::SetCameraMode — single player, registry + "default" round-trip ─────

#[test]
fn test_set_camera_mode_switches_to_registry_preset_and_swaps_marker() {
    let mut app = setup_test_app();
    app.update();
    one_player_catalogs(&mut app);
    load_one_player_scene(&mut app, r#""cine": Fixed((position: (20.0, 10.0, 0.0), look_at: (0.0, 0.0, 0.0), fov: 50.0))"#);

    // Sanity: spawned with the authored Orbit mode.
    let mut orbit_q = app.world_mut().query_filtered::<Entity, With<OrbitCameraMode>>();
    assert_eq!(orbit_q.iter(app.world()).count(), 1, "player must spawn with an Orbit camera");

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SetCameraMode { mode: "cine".to_string(), owner_player: None });
    app.update();

    let mut fixed_q = app.world_mut().query_filtered::<Entity, With<FixedCameraMode>>();
    let cameras: Vec<Entity> = fixed_q.iter(app.world()).collect();
    assert_eq!(cameras.len(), 1, "camera must now carry the Fixed marker, and only one camera should exist");
    let camera = cameras[0];
    assert!(app.world().get::<OrbitCameraMode>(camera).is_none(), "OrbitCameraMode must be removed on switch");
    assert!(matches!(app.world().get::<ActiveCameraMode>(camera).unwrap(), ActiveCameraMode::Fixed(_)));
    assert!(app.world().get::<CameraModeOverride>(camera).is_some(), "a registry-preset switch must mark the camera overridden");
}

#[test]
fn test_set_camera_mode_default_restores_authored_mode_and_clears_override() {
    let mut app = setup_test_app();
    app.update();
    one_player_catalogs(&mut app);
    load_one_player_scene(&mut app, r#""cine": Fixed((position: (20.0, 10.0, 0.0), look_at: (0.0, 0.0, 0.0), fov: 50.0))"#);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SetCameraMode { mode: "cine".to_string(), owner_player: None });
    app.update();
    app.world_mut().resource_mut::<ActionQueue>().push(Action::SetCameraMode { mode: "default".to_string(), owner_player: None });
    app.update();

    let mut q = app.world_mut().query::<(Entity, &ActiveCameraMode)>();
    let (camera, mode) = q.iter(app.world()).next().expect("exactly one camera");
    assert!(matches!(mode, ActiveCameraMode::Orbit(_)), "\"default\" must restore the scene-authored Orbit mode");
    assert!(app.world().get::<OrbitCameraMode>(camera).is_some(), "OrbitCameraMode marker must be restored");
    assert!(app.world().get::<FixedCameraMode>(camera).is_none(), "FixedCameraMode marker must be removed");
    assert!(app.world().get::<CameraModeOverride>(camera).is_none(), "\"default\" must clear the override marker");
}

#[test]
fn test_set_camera_mode_unknown_key_is_noop() {
    let mut app = setup_test_app();
    app.update();
    one_player_catalogs(&mut app);
    load_one_player_scene(&mut app, "");

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SetCameraMode { mode: "nonexistent".to_string(), owner_player: None });
    app.update(); // must not panic

    let mut q = app.world_mut().query::<&ActiveCameraMode>();
    let mode = q.iter(app.world()).next().expect("exactly one camera");
    assert!(matches!(mode, ActiveCameraMode::Orbit(_)), "an unresolvable mode key must leave the camera untouched");
}

#[test]
fn test_set_camera_mode_rejects_party_in_registry_leaving_camera_unchanged() {
    let mut app = setup_test_app();
    app.update();
    one_player_catalogs(&mut app);
    load_one_player_scene(&mut app, r#""shared": Party((zoom_margin: 4.0, min_radius: 4.0, max_radius: 20.0))"#);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SetCameraMode { mode: "shared".to_string(), owner_player: None });
    app.update(); // must not panic

    let mut q = app.world_mut().query::<(Entity, &ActiveCameraMode)>();
    let (camera, mode) = q.iter(app.world()).next().expect("exactly one camera");
    assert!(matches!(mode, ActiveCameraMode::Orbit(_)), "Party(...) in the registry must be rejected, leaving the camera on its prior mode");
    assert!(app.world().get::<CameraModeOverride>(camera).is_none(), "a rejected switch must not mark the camera overridden");
}

#[test]
fn test_set_camera_mode_with_transition_inserts_camera_blend_state() {
    let mut app = setup_test_app();
    app.update();
    one_player_catalogs(&mut app);
    load_one_player_scene(&mut app, r#""cine": Fixed((position: (20.0, 10.0, 0.0), look_at: (0.0, 0.0, 0.0), fov: 50.0, transition: (duration_secs: 0.4, ease: EaseInOut)))"#);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SetCameraMode { mode: "cine".to_string(), owner_player: None });
    app.update();

    let mut q = app.world_mut().query_filtered::<Entity, With<FixedCameraMode>>();
    let camera = q.iter(app.world()).next().expect("camera must exist");
    let blend = app.world().get::<CameraBlendState>(camera);
    assert!(blend.is_some(), "an authored transition: must insert CameraBlendState");
    assert_eq!(blend.unwrap().duration, 0.4);
}

#[test]
fn test_set_camera_mode_instant_cut_has_no_camera_blend_state() {
    let mut app = setup_test_app();
    app.update();
    one_player_catalogs(&mut app);
    load_one_player_scene(&mut app, r#""cine": Fixed((position: (20.0, 10.0, 0.0), look_at: (0.0, 0.0, 0.0), fov: 50.0))"#);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SetCameraMode { mode: "cine".to_string(), owner_player: None });
    app.update();

    let mut q = app.world_mut().query_filtered::<Entity, With<FixedCameraMode>>();
    let camera = q.iter(app.world()).next().expect("camera must exist");
    assert!(app.world().get::<CameraBlendState>(camera).is_none(), "absent transition: must be an instant cut, no CameraBlendState");
}

// ── Action::SetCameraMode — owner_player targeting (split-screen) ──────────────

#[test]
fn test_set_camera_mode_owner_player_targets_only_that_players_camera() {
    let mut app = setup_test_app();
    app.update();
    two_player_split_catalogs(&mut app);
    load_two_player_split_scene(&mut app, r#""cine": Fixed((position: (20.0, 10.0, 0.0), look_at: (0.0, 0.0, 0.0), fov: 50.0))"#);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SetCameraMode { mode: "cine".to_string(), owner_player: Some(0) });
    app.update();

    let mut fixed_q = app.world_mut().query_filtered::<Entity, With<FixedCameraMode>>();
    assert_eq!(fixed_q.iter(app.world()).count(), 1, "exactly one camera (player 0's) must switch to Fixed");
    let mut orbit_q = app.world_mut().query_filtered::<Entity, With<OrbitCameraMode>>();
    assert_eq!(orbit_q.iter(app.world()).count(), 1, "player 1's own camera must remain Orbit, untouched");
}

#[test]
fn test_set_camera_mode_owner_player_out_of_range_is_noop() {
    let mut app = setup_test_app();
    app.update();
    two_player_split_catalogs(&mut app);
    load_two_player_split_scene(&mut app, r#""cine": Fixed((position: (20.0, 10.0, 0.0), look_at: (0.0, 0.0, 0.0), fov: 50.0))"#);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SetCameraMode { mode: "cine".to_string(), owner_player: Some(5) });
    app.update(); // must not panic

    let mut fixed_q = app.world_mut().query_filtered::<Entity, With<FixedCameraMode>>();
    assert_eq!(fixed_q.iter(app.world()).count(), 0, "an out-of-range owner_player must leave every camera untouched");
    let mut orbit_q = app.world_mut().query_filtered::<Entity, With<OrbitCameraMode>>();
    assert_eq!(orbit_q.iter(app.world()).count(), 2, "both cameras must remain Orbit");
}

#[test]
fn test_set_camera_mode_owner_player_in_party_scene_is_noop() {
    let mut app = setup_test_app();
    app.update();
    load_party_scene(&mut app);

    let mut party_q = app.world_mut().query_filtered::<Entity, With<PartyCameraMode>>();
    assert_eq!(party_q.iter(app.world()).count(), 1, "scene must spawn one shared party camera");

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SetCameraMode {
        mode: "default".to_string(),
        owner_player: Some(0),
    });
    app.update(); // must not panic

    let mut party_q2 = app.world_mut().query_filtered::<Entity, With<PartyCameraMode>>();
    assert_eq!(party_q2.iter(app.world()).count(), 1, "the shared party camera must be untouched — owner_player has no single per-player camera to retarget there");
}

// ── Action::CameraShake — owner_player targeting ────────────────────────────────

#[test]
fn test_camera_shake_owner_player_targets_only_that_players_camera() {
    let mut app = setup_test_app();
    app.update();
    two_player_split_catalogs(&mut app);
    load_two_player_split_scene(&mut app, "");

    let mut cams = app.world_mut().query_filtered::<(Entity, &CameraTargets), With<OrbitCameraMode>>();
    let targets: Vec<(Entity, Entity)> = cams.iter(app.world())
        .map(|(e, t)| (e, t.0[0]))
        .collect();
    assert_eq!(targets.len(), 2);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::CameraShake { duration_secs: 0.4, intensity: 0.2, owner_player: Some(0) });
    app.update();

    let mut shaking = app.world_mut().query_filtered::<Entity, With<ironhold_core::capabilities::camera::CameraShakeState>>();
    assert_eq!(shaking.iter(app.world()).count(), 1, "owner_player: Some(0) must shake exactly one camera, not both");
}

// ── AuthoredCameraMode round-trip sanity ─────────────────────────────────────────

#[test]
fn test_authored_camera_mode_matches_scene_authored_mode_at_spawn() {
    let mut app = setup_test_app();
    app.update();
    one_player_catalogs(&mut app);
    load_one_player_scene(&mut app, "");

    let mut q = app.world_mut().query::<(Entity, &AuthoredCameraMode)>();
    let (_camera, authored) = q.iter(app.world()).next().expect("camera must exist");
    assert!(matches!(authored.0, CameraModeDef::Orbit(_)), "AuthoredCameraMode must record the scene-authored Orbit mode this player was actually spawned with");
}
