use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::camera::Viewport;
use bevy::math::Mat4;
use bevy_rapier3d::prelude::{Velocity, CollisionEvent};
use bevy_rapier3d::rapier::geometry::CollisionEventFlags;
use bevy::window::PrimaryWindow;
use ironhold_core::runtime::{SceneHandleV2, LoadedAssetCatalog, LoadedPrefabCatalog, ActiveViewBox, ActiveSplitScreen, DynamicSplitConfig, ActiveSplitSlotCount, GameEvent, SpawnRegistry, ActionQueue, UiEvent, TargetRingVisibilityMode};
use bevy::camera::visibility::RenderLayers;
use ironhold_core::runtime::scene_manager::{
    WorldLabel, WorldLabelRank, SpawnId,
    LoadedGamepadBindings, PendingJoinGamepad, PendingEntitySpawns, QueuedSpawn,
};
use ironhold_core::capabilities::targeting::ClickSelectable;
use ironhold_core::capabilities::action_bar::CurrentTarget;
use ironhold_core::capabilities::stat_display::{
    StatLabelMarker, WorldStatBarFillMarker, WorldPixelBarFillMarker, WorldIconBar,
    WorldTexturedBarFillMarker,
    StatWidgetSpawnCtx, spawn_stat_label_widget, spawn_world_stat_bar_widget,
};
use ironhold_core::schema::{AppState, ProjectConfig, ProjectConfigHandle, GameSceneV2, Action};
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, PrefabKind, ModelCatalogEntry, PrefabComponents, StatLabelDef, WorldStatBarDef, WorldStatBarStyle, MovementConfig};
use ironhold_core::schema::player::{CameraConfig, PartyZoomDef, SplitScreenDef, SplitOrientation, DynamicSplitDef, InputMap, PlayerModelSource, PlayerConfig};
use ironhold_core::schema::camera::CameraModeDef;
use ironhold_core::capabilities::player::{CharacterController, PlayerIndex, PlayerTarget, BoundGamepad, player_view_box_clamp_system, PLAYER_IDLE_FRICTION};
use ironhold_core::capabilities::camera::{
    ActiveCameraMode, CameraTargets, OrbitCameraMode, PartyCameraMode, party_camera_follow_system,
    SplitViewportSlot, split_screen_viewport_system, dynamic_split_screen_system, parse_orbit_button,
    MAX_SPLIT_PLAYERS, SplitScreenPlayerLabel, LinkedPlayerLabel, PLAYER_LABEL_COLORS,
    split_viewport_player_label_spawn_system, split_viewport_player_label_update_system,
};
use ironhold_core::capabilities::trigger_zone::{TriggerZone, TriggerZoneId, trigger_zone_system};
use ironhold_core::GameVariables;

mod support;
use support::{setup_test_app, connect_test_gamepad, press_gamepad_button, set_gamepad_axis};
use bevy::input::gamepad::{GamepadButton, GamepadAxis};

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
        collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
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

// ── party_camera_follow_system: unit-level ──────────────────────────────────────

#[test]
fn test_party_camera_frames_midpoint_and_scales_radius_with_separation() {
    let mut app = setup_test_app();
    app.update();

    let p1 = app.world_mut().spawn((
        test_character_controller(),
        Transform::from_xyz(-5.0, 0.0, 0.0),
    )).id();
    let p2 = app.world_mut().spawn((
        test_character_controller(),
        Transform::from_xyz(5.0, 0.0, 0.0),
    )).id();
    // Separation = 10.0; zoom_margin = 4.0 -> radius = 14.0, within [4, 20].
    let camera = app.world_mut().spawn((
        Transform::IDENTITY,
        ActiveCameraMode::Party(ironhold_core::capabilities::camera::PartyState {
            zoom_margin: 4.0,
            allow_manual_zoom: false,
            manual_zoom_offset: 0.0,
            zoom_speed: 10.0,
            orbit_speed: 0.5,
            min_radius: 4.0,
            max_radius: 20.0,
            pitch: 0.5,
            yaw: 0.0,
            look_at_offset: Vec3::new(0.0, 1.0, 0.0),
            min_pitch: 0.1,
            max_pitch: 0.9,
            orbit_lmb: true,
            orbit_rmb: true,
        }),
        PartyCameraMode,
        CameraTargets(vec![p1, p2]),
    )).id();

    app.world_mut().run_system_once(party_camera_follow_system).unwrap();

    let cam_transform = app.world().get::<Transform>(camera).unwrap();
    let midpoint = Vec3::new(0.0, 0.0, 0.0) + Vec3::new(0.0, 1.0, 0.0);
    let actual_radius = cam_transform.translation.distance(midpoint);
    assert!(
        (actual_radius - 14.0).abs() < 0.01,
        "expected radius ~14.0 (10.0 separation + 4.0 margin), got {actual_radius}"
    );
}

#[test]
fn test_party_camera_radius_clamps_to_max_when_players_far_apart() {
    let mut app = setup_test_app();
    app.update();

    let p1 = app.world_mut().spawn((test_character_controller(), Transform::from_xyz(-100.0, 0.0, 0.0))).id();
    let p2 = app.world_mut().spawn((test_character_controller(), Transform::from_xyz(100.0, 0.0, 0.0))).id();
    let camera = app.world_mut().spawn((
        Transform::IDENTITY,
        ActiveCameraMode::Party(ironhold_core::capabilities::camera::PartyState {
            zoom_margin: 4.0,
            allow_manual_zoom: false,
            manual_zoom_offset: 0.0,
            zoom_speed: 10.0,
            orbit_speed: 0.5,
            min_radius: 4.0,
            max_radius: 20.0,
            pitch: 0.5,
            yaw: 0.0,
            look_at_offset: Vec3::ZERO,
            min_pitch: 0.1,
            max_pitch: 0.9,
            orbit_lmb: true,
            orbit_rmb: true,
        }),
        PartyCameraMode,
        CameraTargets(vec![p1, p2]),
    )).id();

    app.world_mut().run_system_once(party_camera_follow_system).unwrap();

    let cam_transform = app.world().get::<Transform>(camera).unwrap();
    let actual_radius = cam_transform.translation.distance(Vec3::ZERO);
    assert!(
        (actual_radius - 20.0).abs() < 0.01,
        "radius must clamp to max_radius (20.0) when raw separation + margin exceeds it, got {actual_radius}"
    );
}

// ── player_view_box_clamp_system: unit-level ────────────────────────────────────

#[test]
fn test_view_box_clamps_position_and_zeroes_velocity_on_clamped_axis() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(ActiveViewBox(Some((-10.0, -10.0, 10.0, 10.0))));

    let outside = app.world_mut().spawn((
        test_character_controller(),
        Transform::from_xyz(15.0, 2.0, -20.0),
        Velocity { linvel: Vec3::new(3.0, -5.0, 3.0), angvel: Vec3::ZERO },
    )).id();

    app.world_mut().run_system_once(player_view_box_clamp_system).unwrap();

    let transform = app.world().get::<Transform>(outside).unwrap();
    let velocity = app.world().get::<Velocity>(outside).unwrap();
    assert_eq!(transform.translation.x, 10.0, "x must clamp to max_x");
    assert_eq!(transform.translation.z, -10.0, "z must clamp to min_z");
    assert_eq!(transform.translation.y, 2.0, "y (vertical/jump) must be untouched");
    assert_eq!(velocity.linvel.x, 0.0, "x velocity must be zeroed once clamped");
    assert_eq!(velocity.linvel.z, 0.0, "z velocity must be zeroed once clamped");
    assert_eq!(velocity.linvel.y, -5.0, "y velocity must be untouched");
}

#[test]
fn test_view_box_leaves_in_bounds_player_untouched() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(ActiveViewBox(Some((-10.0, -10.0, 10.0, 10.0))));

    let inside = app.world_mut().spawn((
        test_character_controller(),
        Transform::from_xyz(2.0, 0.5, -3.0),
        Velocity { linvel: Vec3::new(1.0, 0.0, -1.0), angvel: Vec3::ZERO },
    )).id();

    app.world_mut().run_system_once(player_view_box_clamp_system).unwrap();

    let transform = app.world().get::<Transform>(inside).unwrap();
    let velocity = app.world().get::<Velocity>(inside).unwrap();
    assert_eq!(transform.translation, Vec3::new(2.0, 0.5, -3.0));
    assert_eq!(velocity.linvel, Vec3::new(1.0, 0.0, -1.0));
}

#[test]
fn test_view_box_system_is_noop_when_no_box_configured() {
    let mut app = setup_test_app();
    app.update();
    // ActiveViewBox defaults to None via init_resource — no explicit insert needed.

    let far_away = app.world_mut().spawn((
        test_character_controller(),
        Transform::from_xyz(9999.0, 0.0, 9999.0),
        Velocity { linvel: Vec3::new(5.0, 0.0, 5.0), angvel: Vec3::ZERO },
    )).id();

    app.world_mut().run_system_once(player_view_box_clamp_system).unwrap();

    let transform = app.world().get::<Transform>(far_away).unwrap();
    assert_eq!(transform.translation, Vec3::new(9999.0, 0.0, 9999.0));
}

// ── Scene-load level: two-player spawn + shared/fallback camera ────────────────

fn two_player_catalogs(app: &mut App, party: Option<PartyZoomDef>) {
    two_player_catalogs_with_split(app, party, None);
}

fn two_player_catalogs_with_split(
    app: &mut App,
    party: Option<PartyZoomDef>,
    split: Option<SplitScreenDef>,
) {
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
            ("char_b".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-female-01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));

    let mut p1_camera = base_camera_config();
    p1_camera.party = party;
    p1_camera.split = split;

    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("test_player_1".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "char_a".to_string(),
                player_index: 0,
                components: PrefabComponents {
                    tags: vec!["player".to_string()],
                    camera: Some(p1_camera),
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

/// Drive a Replace-mode scene load of a two-player scene through `spawn_scene_v2`,
/// mirroring `scene_lifecycle_tests.rs`'s `drive_replace_load` pattern.
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

#[test]
fn test_single_player_with_no_camera_block_and_no_camera_mode_still_gets_default_orbit_camera() {
    // Acceptance criterion added during camera_modes.md v1's confirmation pass: the corrected
    // backward-compat rule is tag-driven (`tags: ["player"]`), not field-presence-driven — the
    // majority shape across this codebase's own example projects is a player prefab with NO
    // `camera:` block and NO `camera_mode:` at all, relying entirely on engine defaults. Every
    // other test in this file uses `test_player_2` (also camera-less) only inside a 2-player
    // split/party dispatch; this is the single-player `spawn_active_camera_for_player` fallback
    // path specifically.
    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("test_player_solo".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "char_a".to_string(),
                components: PrefabComponents {
                    tags: vec!["player".to_string()],
                    // Deliberately no `camera`, no `camera_mode` — the majority-shape case.
                    ..Default::default()
                },
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    let config_handle = app.world_mut().resource_mut::<Assets<ProjectConfig>>().add(ProjectConfig {
        schema_version: 1,
        initial_scene: "scenes/solo.ron".to_string(),
        ..Default::default()
    });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));
    let scene: GameSceneV2 = ron::de::from_str(r#"(
        schema_version: 2,
        entities: [
            (id: "p1", prefab: "test_player_solo", transform: (translation: (0.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
        ],
        ui: [],
    )"#).unwrap();
    let scene_handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LoadingScene);
    app.update();
    app.update();
    app.update();

    let orbit_count = app.world_mut().query::<&OrbitCameraMode>().iter(app.world()).count();
    assert_eq!(orbit_count, 1, "a camera-less player prefab must still spawn exactly one default Orbit-mode camera");

    let mut q = app.world_mut().query::<&ActiveCameraMode>();
    let orbit = q.iter(app.world()).find_map(|m| match m {
        ActiveCameraMode::Orbit(o) => Some(o),
        _ => None,
    }).expect("the spawned camera must be in Orbit mode");
    assert!(
        (orbit.min_radius - 2.0).abs() < 0.001 && (orbit.max_radius - 20.0).abs() < 0.001,
        "default_camera_config()'s min_radius=2.0/max_radius=20.0 must resolve onto the spawned \
         camera when no `camera:` block was authored, got min={} max={}", orbit.min_radius, orbit.max_radius
    );
}

#[test]
fn test_flycam_tagged_prefab_with_no_flycam_block_and_no_camera_mode_still_gets_default_flycam() {
    // Sibling to the Orbit test above, for the other half of the corrected backward-compat rule:
    // `tags: ["flycam"]` with no `flycam:` block and no `camera_mode:` (matches `terrain_demo`/
    // `custom_materials` in this repo's own asset projects) must still spawn a default Flycam.
    use ironhold_core::capabilities::camera::FlycamCameraMode;

    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog::default()));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("test_flycam_solo".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                components: PrefabComponents {
                    tags: vec!["flycam".to_string()],
                    // Deliberately no `flycam`, no `camera_mode`.
                    ..Default::default()
                },
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    let config_handle = app.world_mut().resource_mut::<Assets<ProjectConfig>>().add(ProjectConfig {
        schema_version: 1,
        initial_scene: "scenes/flycam_solo.ron".to_string(),
        ..Default::default()
    });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));
    let scene: GameSceneV2 = ron::de::from_str(r#"(
        schema_version: 2,
        entities: [
            (id: "fc1", prefab: "test_flycam_solo", transform: (translation: (0.0, 5.0, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
        ],
        ui: [],
    )"#).unwrap();
    let scene_handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LoadingScene);
    app.update();
    app.update();
    app.update();

    let flycam_count = app.world_mut().query::<&FlycamCameraMode>().iter(app.world()).count();
    assert_eq!(flycam_count, 1, "a flycam-tagged prefab with no flycam: block must still spawn exactly one default Flycam");
}

#[test]
fn test_two_players_spawn_with_shared_party_camera() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs(&mut app, Some(PartyZoomDef { zoom_margin: 4.0, allow_manual_zoom: false }));
    load_two_player_scene(&mut app);

    let controller_count = app.world_mut().query::<&CharacterController>().iter(app.world()).count();
    assert_eq!(controller_count, 2, "both player-tagged entities must spawn a CharacterController");

    let party_cam_count = app.world_mut().query::<&PartyCameraMode>().iter(app.world()).count();
    assert_eq!(party_cam_count, 1, "exactly one shared PartyOrbitCamera must spawn when `party` is configured");

    let solo_cam_count = app.world_mut().query::<&OrbitCameraMode>().iter(app.world()).count();
    assert_eq!(solo_cam_count, 0, "no per-player OrbitCamera should spawn alongside the shared party camera");
}

#[test]
fn test_two_players_without_party_block_falls_back_to_single_camera() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs(&mut app, None);
    load_two_player_scene(&mut app);

    let controller_count = app.world_mut().query::<&CharacterController>().iter(app.world()).count();
    assert_eq!(controller_count, 2, "both players still spawn even without a `party` block");

    let solo_cam_count = app.world_mut().query::<&OrbitCameraMode>().iter(app.world()).count();
    assert_eq!(
        solo_cam_count, 1,
        "missing `party` on a 2-player scene must fall back to exactly one OrbitCamera, \
         never two silently-competing per-player cameras"
    );

    let party_cam_count = app.world_mut().query::<&PartyCameraMode>().iter(app.world()).count();
    assert_eq!(party_cam_count, 0);
}

// ── trigger_zone_system: portal fires generically for any player (Stage 2 foundation) ──────
//
// Stage 2 (portal/teleport) needed zero new engine code because trigger_zone_system already
// queries `With<CharacterController>` generically rather than assuming a single player. These
// tests inject `CollisionEvent`s directly (bypassing real Rapier physics) for a deterministic,
// fast check of that assumption, including the documented same-tick double-fire quirk.

#[test]
fn test_trigger_zone_fires_entity_entered_for_a_single_player() {
    let mut app = setup_test_app();
    app.update();

    let p1 = app.world_mut().spawn(test_character_controller()).id();
    let zone = app.world_mut()
        .spawn((TriggerZone, TriggerZoneId("portal_to_room2".to_string())))
        .id();

    app.world_mut()
        .resource_mut::<Messages<CollisionEvent>>()
        .write(CollisionEvent::Started(p1, zone, CollisionEventFlags::SENSOR));

    app.world_mut().run_system_once(trigger_zone_system).unwrap();

    let fired = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(name) if name == "entity.entered:portal_to_room2"));
    assert!(fired, "expected entity.entered:portal_to_room2 when a player enters the zone");
}

#[test]
fn test_trigger_zone_fires_once_per_player_when_both_enter_same_tick() {
    let mut app = setup_test_app();
    app.update();

    let p1 = app.world_mut().spawn(test_character_controller()).id();
    let p2 = app.world_mut().spawn(test_character_controller()).id();
    let zone = app.world_mut()
        .spawn((TriggerZone, TriggerZoneId("portal_to_room2".to_string())))
        .id();

    // Both players enter the same portal in the same physics tick — the plan's "known, accepted
    // quirk": the matching rules.ron LoadScene fires twice (harmless re-trigger), not once.
    {
        let mut collisions = app.world_mut().resource_mut::<Messages<CollisionEvent>>();
        collisions.write(CollisionEvent::Started(p1, zone, CollisionEventFlags::SENSOR));
        collisions.write(CollisionEvent::Started(p2, zone, CollisionEventFlags::SENSOR));
    }

    app.world_mut().run_system_once(trigger_zone_system).unwrap();

    let entered_count = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .filter(|e| matches!(e, GameEvent::Trigger(name) if name == "entity.entered:portal_to_room2"))
        .count();
    assert_eq!(
        entered_count, 2,
        "both players entering the same tick must fire entity.entered twice, not once or zero \
         — this is the documented double-fire quirk rules.ron's LoadScene action tolerates"
    );
}

#[test]
fn test_trigger_zone_ignores_non_player_entities() {
    let mut app = setup_test_app();
    app.update();

    // An entity with no CharacterController (e.g. an NPC or physics prop) colliding with the
    // zone must not fire entity.entered — trigger_zone_system only checks for the player marker.
    let non_player = app.world_mut().spawn(Transform::default()).id();
    let zone = app.world_mut()
        .spawn((TriggerZone, TriggerZoneId("portal_to_room2".to_string())))
        .id();

    app.world_mut()
        .resource_mut::<Messages<CollisionEvent>>()
        .write(CollisionEvent::Started(non_player, zone, CollisionEventFlags::SENSOR));

    app.world_mut().run_system_once(trigger_zone_system).unwrap();

    let fired = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(name) if name == "entity.entered:portal_to_room2"));
    assert!(!fired, "a non-player entity colliding with the zone must not fire entity.entered");
}

// ── Stage 3: split_screen_viewport_system ───────────────────────────────────────

/// Spawns a `Window` tagged `PrimaryWindow` with the given PHYSICAL pixel size and an optional
/// scale factor override — `WindowResolution` stores physical pixels as its source of truth and
/// derives logical `width()`/`height()` by dividing by `scale_factor()`; `physical_size()`
/// (what `split_screen_viewport_system` reads) is exactly `(physical_width, physical_height)`
/// regardless of the scale factor. `setup_test_app()` uses `MinimalPlugins`, which spawns no
/// window at all, so tests exercising the viewport system need one manually.
fn spawn_primary_window(app: &mut App, physical_width: u32, physical_height: u32, scale_factor_override: f32) {
    use bevy::window::{Window, WindowResolution};
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(physical_width, physical_height)
                .with_scale_factor_override(scale_factor_override),
            ..default()
        },
        PrimaryWindow,
    ));
}

#[test]
fn test_split_screen_viewport_halves_window_vertically() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Vertical)));

    let cam0 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(0))).id();
    let cam1 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(1))).id();

    app.world_mut().run_system_once(split_screen_viewport_system).unwrap();

    let vp0 = app.world().get::<Camera>(cam0).unwrap().viewport.clone().unwrap();
    let vp1 = app.world().get::<Camera>(cam1).unwrap().viewport.clone().unwrap();

    assert_eq!(vp0.physical_position, UVec2::new(0, 0));
    assert_eq!(vp0.physical_size, UVec2::new(640, 720));
    assert_eq!(vp1.physical_position, UVec2::new(640, 0));
    assert_eq!(vp1.physical_size, UVec2::new(640, 720));
}

#[test]
fn test_split_screen_viewport_absorbs_odd_width_remainder() {
    let mut app = setup_test_app();
    app.update();
    // Odd physical width — the two halves must still sum to exactly the full window width.
    spawn_primary_window(&mut app, 1281, 720, 1.0);
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Vertical)));

    let cam0 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(0))).id();
    let cam1 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(1))).id();

    app.world_mut().run_system_once(split_screen_viewport_system).unwrap();

    let vp0 = app.world().get::<Camera>(cam0).unwrap().viewport.clone().unwrap();
    let vp1 = app.world().get::<Camera>(cam1).unwrap().viewport.clone().unwrap();

    assert_eq!(vp0.physical_size.x + vp1.physical_size.x, 1281, "halves must sum to the full physical width");
    assert_eq!(vp0.physical_size.x, 640);
    assert_eq!(vp1.physical_size.x, 641, "remainder pixel goes to the second half, not dropped");
}

#[test]
fn test_split_screen_viewport_halves_window_horizontally() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Horizontal)));

    let cam0 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(0))).id();
    let cam1 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(1))).id();

    app.world_mut().run_system_once(split_screen_viewport_system).unwrap();

    let vp0 = app.world().get::<Camera>(cam0).unwrap().viewport.clone().unwrap();
    let vp1 = app.world().get::<Camera>(cam1).unwrap().viewport.clone().unwrap();

    assert_eq!(vp0.physical_position, UVec2::new(0, 0));
    assert_eq!(vp0.physical_size, UVec2::new(1280, 360));
    assert_eq!(vp1.physical_position, UVec2::new(0, 360));
    assert_eq!(vp1.physical_size, UVec2::new(1280, 360));
    // Non-overlap: slot 1 must start exactly where slot 0 ends, not merely have a plausible size.
    assert_eq!(vp1.physical_position.y, vp0.physical_size.y, "slot 1 must start where slot 0 ends");
}

#[test]
fn test_split_screen_viewport_absorbs_odd_height_remainder() {
    let mut app = setup_test_app();
    app.update();
    // Odd physical height — the two halves must still sum to exactly the full window height.
    spawn_primary_window(&mut app, 1280, 721, 1.0);
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Horizontal)));

    let cam0 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(0))).id();
    let cam1 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(1))).id();

    app.world_mut().run_system_once(split_screen_viewport_system).unwrap();

    let vp0 = app.world().get::<Camera>(cam0).unwrap().viewport.clone().unwrap();
    let vp1 = app.world().get::<Camera>(cam1).unwrap().viewport.clone().unwrap();

    assert_eq!(vp0.physical_size.y + vp1.physical_size.y, 721, "halves must sum to the full physical height");
    assert_eq!(vp0.physical_size.y, 360);
    assert_eq!(vp1.physical_size.y, 361, "remainder pixel goes to the second half, not dropped");
    // Non-overlap: slot 1 must start exactly where slot 0 ends, even with the odd remainder.
    assert_eq!(vp1.physical_position.y, vp0.physical_size.y, "slot 1 must start where slot 0 ends");
}

#[test]
fn test_split_screen_viewport_unaffected_by_scale_factor_override() {
    let mut app = setup_test_app();
    app.update();
    // Same physical size as test_split_screen_viewport_halves_window_vertically but with a 2x
    // scale factor override — must produce the IDENTICAL viewport split. This locks in reading
    // `Window::physical_size()` directly: if a future change swapped that for
    // `width()`/`height()` (logical) * `scale_factor()` by hand and got the multiplication
    // backwards or forgot it entirely, this test would catch the resulting HiDPI regression.
    spawn_primary_window(&mut app, 1280, 720, 2.0);
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Vertical)));

    let cam0 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(0))).id();
    let cam1 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(1))).id();

    app.world_mut().run_system_once(split_screen_viewport_system).unwrap();

    let vp0 = app.world().get::<Camera>(cam0).unwrap().viewport.clone().unwrap();
    let vp1 = app.world().get::<Camera>(cam1).unwrap().viewport.clone().unwrap();

    assert_eq!(vp0.physical_size, UVec2::new(640, 720));
    assert_eq!(vp1.physical_position, UVec2::new(640, 0));
    assert_eq!(vp1.physical_size, UVec2::new(640, 720));
}

#[test]
fn test_split_screen_viewport_system_is_noop_when_no_split_active() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);
    // ActiveSplitScreen defaults to None via init_resource — no explicit insert needed.

    let cam0 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(0))).id();

    app.world_mut().run_system_once(split_screen_viewport_system).unwrap();

    assert!(
        app.world().get::<Camera>(cam0).unwrap().viewport.is_none(),
        "no ActiveSplitScreen orientation set means the system must not touch Camera.viewport"
    );
}

#[test]
fn test_parse_orbit_button_none_disables_both_mouse_buttons() {
    assert_eq!(parse_orbit_button("None"), (false, false));
}

// ── Stage 3: split + party mutual exclusion ─────────────────────────────────────

#[test]
fn test_split_and_party_both_set_split_wins() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app,
        Some(PartyZoomDef { zoom_margin: 4.0, allow_manual_zoom: false }),
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
    );
    load_two_player_scene(&mut app);

    let controller_count = app.world_mut().query::<&CharacterController>().iter(app.world()).count();
    assert_eq!(controller_count, 2, "both players still spawn regardless of the conflicting config");

    let party_cam_count = app.world_mut().query::<&PartyCameraMode>().iter(app.world()).count();
    assert_eq!(party_cam_count, 0, "party must NOT spawn when split is also set");

    let split_slot_count = app.world_mut().query::<&SplitViewportSlot>().iter(app.world()).count();
    assert_eq!(split_slot_count, 2, "split wins: one SplitViewportSlot camera per player");

    let orbit_cam_count = app.world_mut().query::<&OrbitCameraMode>().iter(app.world()).count();
    assert_eq!(orbit_cam_count, 2, "split spawns two real OrbitCameras, not a fallback single one");
}

#[test]
fn test_split_only_spawns_two_orbit_cameras_with_viewport_slots() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app,
        None,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
    );
    load_two_player_scene(&mut app);

    let slots: Vec<u32> = {
        let mut query = app.world_mut().query::<&SplitViewportSlot>();
        query.iter(app.world()).map(|s| s.0).collect()
    };
    let mut sorted = slots.clone();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1], "expected exactly one slot 0 and one slot 1, got {:?}", slots);
}

#[test]
fn test_split_screen_honors_camera_mode_orbit_not_just_legacy_camera_field() {
    // Regression test for a real bug 3 independent post-implementation reviews caught: the
    // split/party spawn dispatch originally read `player_config.camera` directly, silently
    // ignoring an authored `camera_mode: Orbit(...)` and falling back to `default_camera_config()`
    // whenever a migrated prefab dropped its legacy `camera:` block — exactly what shipped
    // `local_coop_demo`'s player_p1_split_h/player_p2_split_h (room4) did, regressing their
    // `orbit_button: "None"`/`zoom_speed: 0.0` split-screen mouse-decoupling. This pins the fix
    // by authoring the FIRST player via `camera_mode:` only (no `camera:` block at all) and
    // asserting its distinctive tuning survives into BOTH spawned split cameras.
    let mut app = setup_test_app();
    app.update();
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
                    // No `camera:` block at all — camera_mode is the ONLY source of tuning.
                    camera_mode: Some(CameraModeDef::Orbit(CameraConfig {
                        zoom_speed: 0.0,
                        orbit_button: "None".to_string(),
                        ..base_camera_config()
                    })),
                    split: Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
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
                    // Also camera_mode-only, no `split:` (only the first player's is read) — same
                    // pattern room4's real player_p2_split_h uses.
                    camera_mode: Some(CameraModeDef::Orbit(CameraConfig {
                        zoom_speed: 0.0,
                        orbit_button: "None".to_string(),
                        ..base_camera_config()
                    })),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));
    load_two_player_scene(&mut app);

    let split_orbit_states: Vec<(f32, bool, bool)> = {
        let mut q = app.world_mut().query::<(&ActiveCameraMode, &SplitViewportSlot)>();
        q.iter(app.world()).filter_map(|(m, _)| match m {
            ActiveCameraMode::Orbit(o) => Some((o.zoom_speed, o.orbit_lmb, o.orbit_rmb)),
            _ => None,
        }).collect()
    };
    assert_eq!(split_orbit_states.len(), 2, "both split cameras must be Orbit-mode");
    for (zoom_speed, orbit_lmb, orbit_rmb) in split_orbit_states {
        assert_eq!(zoom_speed, 0.0, "camera_mode's zoom_speed: 0.0 must reach the spawned split camera, not default_camera_config()'s 10.0");
        assert!(!orbit_lmb && !orbit_rmb, "camera_mode's orbit_button: \"None\" must disable mouse-orbit on the spawned split camera — a shared mouse must not orbit both players' cameras");
    }
}

#[test]
fn test_every_shared_mouse_disabled_split_camera_also_disables_character_rotate() {
    // Regression for a real bug found live during camera_modes.md v1's room4 playtest:
    // `camera_orbit_system` has no per-viewport cursor check (confirmed via a runtime
    // diagnostic) — it reads the mouse/keyboard state once per frame and applies it identically
    // to every active Orbit-mode camera. `orbit_button: "None"` disables the camera's own
    // mouse-orbit, but `character_rotate_button` (default `Some("Right")`) is a SEPARATE switch
    // that independently rotates the character model on RMB-drag, with the same no-per-viewport-
    // check limitation. Every one of local_coop_demo's 15 split-screen camera blocks had disabled
    // `orbit_button`/`zoom_speed` but never `character_rotate_button` — holding RMB and moving the
    // mouse spun EVERY split player's character at once, from either viewport. Parses the REAL
    // prefabs.ron (not synthetic data) through the exact same RON options the engine's
    // AssetLoader uses, so this catches the authored RON directly, not just the Rust types.
    let text = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/projects/local_coop_demo/prefabs/prefabs.ron")
    ).expect("read local_coop_demo/prefabs/prefabs.ron");
    let catalog: PrefabCatalog = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(&text)
        .expect("parse local_coop_demo/prefabs/prefabs.ron");

    let mut checked = 0;
    for (key, prefab) in &catalog.prefabs {
        // Any prefab that has already opted into disabling mouse-orbit (either via the legacy
        // `camera:` field or the new `camera_mode: Orbit(...)` sibling) is, by definition, a
        // split-screen (or otherwise shared-mouse) player — it must disable ALL THREE fields
        // together, not just the two `orbit_button`/`zoom_speed` this bug's fix already covers.
        let orbit_cfg = match (&prefab.components.camera, &prefab.components.camera_mode) {
            (_, Some(CameraModeDef::Orbit(cfg))) => Some(cfg),
            (Some(cfg), None) => Some(cfg),
            _ => None,
        };
        let Some(cfg) = orbit_cfg else { continue };
        if cfg.orbit_button != "None" {
            continue;
        }
        checked += 1;
        assert!(
            cfg.character_rotate_button.is_none(),
            "prefab '{key}' disables orbit_button (\"None\") for shared-mouse split-screen play \
             but leaves character_rotate_button at its default (Some(\"Right\")) — RMB-drag will \
             spin every split player's character at once. Set character_rotate_button: None \
             alongside orbit_button: \"None\" and zoom_speed: 0.0."
        );
    }
    assert!(checked >= 15, "expected to check at least the 15 known split-screen camera blocks, only found {checked} — did prefabs.ron structure change?");
}

// ── per_viewport_target_ring_visibility.md: RenderLayers on split cameras ───────

#[test]
fn test_own_viewport_only_false_is_the_default_and_spawns_no_render_layers_anywhere() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app,
        None,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
    );
    load_two_player_scene(&mut app);

    assert_eq!(
        *app.world().resource::<TargetRingVisibilityMode>(), TargetRingVisibilityMode::AllViewports,
        "every existing project has no own_viewport_only authored — the resource must default to AllViewports"
    );
    let render_layers_count = app.world_mut().query::<&RenderLayers>().iter(app.world()).count();
    assert_eq!(
        render_layers_count, 0,
        "regression: with own_viewport_only unset (today's behavior for every existing scene), \
         zero RenderLayers components must exist anywhere — this feature must be a zero-footprint \
         opt-in"
    );
}

#[test]
fn test_static_split_own_viewport_only_gives_each_camera_its_own_layer_plus_shared_layer_0() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app,
        None,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: true }),
    );
    load_two_player_scene(&mut app);

    assert_eq!(*app.world().resource::<TargetRingVisibilityMode>(), TargetRingVisibilityMode::OwnViewportOnly);

    let layers_by_slot: std::collections::HashMap<u32, RenderLayers> = {
        let mut query = app.world_mut().query::<(&SplitViewportSlot, &RenderLayers)>();
        query.iter(app.world()).map(|(s, l)| (s.0, l.clone())).collect()
    };
    assert_eq!(layers_by_slot.len(), 2, "both split cameras must carry a RenderLayers component");

    // test_player_1 (slot 0) has player_index: 0 -> reserved layer 1; test_player_2 (slot 1) has
    // player_index: 1 -> reserved layer 2. Both also keep layer 0 (ordinary scene geometry).
    let cam0 = &layers_by_slot[&0];
    assert!(cam0.intersects(&RenderLayers::layer(0)), "player 0's camera must still see ordinary scene geometry (layer 0)");
    assert!(cam0.intersects(&RenderLayers::layer(1)), "player 0's camera must see its own ring layer (1)");
    assert!(!cam0.intersects(&RenderLayers::layer(2)), "player 0's camera must NOT see player 1's ring layer (2)");

    let cam1 = &layers_by_slot[&1];
    assert!(cam1.intersects(&RenderLayers::layer(0)), "player 1's camera must still see ordinary scene geometry (layer 0)");
    assert!(cam1.intersects(&RenderLayers::layer(2)), "player 1's camera must see its own ring layer (2)");
    assert!(!cam1.intersects(&RenderLayers::layer(1)), "player 1's camera must NOT see player 0's ring layer (1)");
}

#[test]
fn test_static_split_own_viewport_only_keys_camera_layer_on_player_index_not_spawn_order() {
    // Deliberately reversed: the scene's first entity (spawn-order index 0) uses the prefab with
    // player_index: 1, and the second (spawn-order index 1) uses player_index: 0 — proving the
    // reserved layer is keyed on `PlayerConfig.player_index`, not the spawn loop's `i`, which can
    // diverge from it (see the feature plan's plan-review note).
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
            ("char_b".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-female-01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    let mut p1_camera = base_camera_config();
    p1_camera.split = Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: true });
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("test_player_reversed_1".to_string(), PrefabDef {
                kind: PrefabKind::Actor, model: "char_a".to_string(), player_index: 1,
                components: PrefabComponents { tags: vec!["player".to_string()], camera: Some(p1_camera), ..Default::default() },
                ..Default::default()
            }),
            ("test_player_reversed_0".to_string(), PrefabDef {
                kind: PrefabKind::Actor, model: "char_b".to_string(), player_index: 0,
                components: PrefabComponents { tags: vec!["player".to_string()], ..Default::default() },
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    let config_handle = app.world_mut().resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig { schema_version: 1, initial_scene: "scenes/t.ron".to_string(), ..Default::default() });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));
    let scene: GameSceneV2 = ron::de::from_str(r#"(
        schema_version: 2,
        entities: [
            (id: "first_spawned", prefab: "test_player_reversed_1", transform: (translation: (-4.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
            (id: "second_spawned", prefab: "test_player_reversed_0", transform: (translation: (4.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
        ],
        ui: [],
    )"#).unwrap();
    let scene_handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LoadingScene);
    app.update();
    app.update();
    app.update();

    let layers_by_slot: std::collections::HashMap<u32, RenderLayers> = {
        let mut query = app.world_mut().query::<(&SplitViewportSlot, &RenderLayers)>();
        query.iter(app.world()).map(|(s, l)| (s.0, l.clone())).collect()
    };
    // Slot 0 is the first-spawned entity, whose prefab has player_index: 1 -> reserved layer 2,
    // NOT layer 1 (which a spawn-order-keyed bug would have wrongly assigned).
    assert!(layers_by_slot[&0].intersects(&RenderLayers::layer(2)), "slot 0's camera belongs to player_index 1 and must carry layer 2");
    assert!(!layers_by_slot[&0].intersects(&RenderLayers::layer(1)), "slot 0's camera must NOT carry layer 1 just because it spawned first");
    // Slot 1 is the second-spawned entity, whose prefab has player_index: 0 -> reserved layer 1.
    assert!(layers_by_slot[&1].intersects(&RenderLayers::layer(1)), "slot 1's camera belongs to player_index 0 and must carry layer 1");
    assert!(!layers_by_slot[&1].intersects(&RenderLayers::layer(2)), "slot 1's camera must NOT carry layer 2 just because it spawned second");
}

#[test]
fn test_dynamic_split_own_viewport_only_gives_party_camera_the_full_layer_union() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app,
        None,
        Some(SplitScreenDef {
            orientation: SplitOrientation::Vertical,
            dynamic: Some(DynamicSplitDef { split_distance: 5.0, merge_distance: 3.0, merged_zoom_margin: 3.0, merged_allow_manual_zoom: false }),
            own_viewport_only: true,
        }),
    );
    load_two_player_scene(&mut app); // players are 8.0 apart (-4,0,0)/(4,0,0) -> starts split

    let split_layers: Vec<RenderLayers> = {
        let mut q = app.world_mut().query_filtered::<&RenderLayers, With<SplitViewportSlot>>();
        q.iter(app.world()).cloned().collect()
    };
    assert_eq!(split_layers.len(), 2, "both split cameras must carry a RenderLayers component");

    let party_layers = {
        let mut q = app.world_mut().query_filtered::<&RenderLayers, With<PartyCameraMode>>();
        q.iter(app.world()).next().cloned()
    };
    let party_layers = party_layers.expect(
        "the shared party/merged camera must ALSO carry a RenderLayers component — a componentless \
         party camera (implicit layer 0 only) would render zero rings once any ring restricts \
         itself to a non-zero layer, breaking the 'merged view shows all rings' guarantee"
    );
    for layer in 0..=4 {
        assert!(
            party_layers.intersects(&RenderLayers::layer(layer)),
            "party camera must carry the full union {{0,1,2,3,4}} (layer 0 plus every reserved \
             ring layer) so it still sees every player's ring while merged; missing layer {layer}"
        );
    }
}

#[test]
fn test_grid_split_own_viewport_only_gives_each_of_four_cameras_its_own_reserved_layer() {
    // Only the Vertical tests above cover own_viewport_only's camera-layer assignment — Grid is
    // the only orientation where more than 2 cameras get layers, and the only place a
    // `% MAX_SPLIT_PLAYERS` slip would actually show up (debug-detective/system-architect finding).
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: true }),
    );
    load_n_player_scene(&mut app, MAX_SPLIT_PLAYERS);

    let layers_by_slot: std::collections::HashMap<u32, RenderLayers> = {
        let mut query = app.world_mut().query::<(&SplitViewportSlot, &RenderLayers)>();
        query.iter(app.world()).map(|(s, l)| (s.0, l.clone())).collect()
    };
    assert_eq!(layers_by_slot.len(), MAX_SPLIT_PLAYERS as usize, "all 4 Grid cameras must carry a RenderLayers component");

    for slot in 0..MAX_SPLIT_PLAYERS {
        let layers = &layers_by_slot[&slot];
        let own_layer = 1 + slot % MAX_SPLIT_PLAYERS;
        assert!(layers.intersects(&RenderLayers::layer(0)), "slot {slot}'s camera must still see ordinary scene geometry (layer 0)");
        assert!(layers.intersects(&RenderLayers::layer(own_layer as usize)), "slot {slot}'s camera must see its own reserved layer {own_layer}");
        for other_slot in 0..MAX_SPLIT_PLAYERS {
            if other_slot == slot { continue; }
            let other_layer = 1 + other_slot % MAX_SPLIT_PLAYERS;
            assert!(
                !layers.intersects(&RenderLayers::layer(other_layer as usize)),
                "slot {slot}'s camera must NOT see slot {other_slot}'s reserved layer {other_layer}"
            );
        }
    }
}

// ── Stage 6: Grid orientation (N-way split) ─────────────────────────────────────

#[test]
fn test_split_screen_viewport_grid_4way_quadrants() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Grid)));
    app.world_mut().insert_resource(ActiveSplitSlotCount(Some(4)));

    let cams: Vec<Entity> = (0..4)
        .map(|i| app.world_mut().spawn((Camera::default(), SplitViewportSlot(i))).id())
        .collect();

    app.world_mut().run_system_once(split_screen_viewport_system).unwrap();

    let vp = |e: Entity| app.world().get::<Camera>(e).unwrap().viewport.clone().unwrap();
    let (vp0, vp1, vp2, vp3) = (vp(cams[0]), vp(cams[1]), vp(cams[2]), vp(cams[3]));

    // 2x2 grid: slot 0 = top-left, 1 = top-right, 2 = bottom-left, 3 = bottom-right.
    assert_eq!(vp0.physical_position, UVec2::new(0, 0));
    assert_eq!(vp0.physical_size, UVec2::new(640, 360));
    assert_eq!(vp1.physical_position, UVec2::new(640, 0));
    assert_eq!(vp1.physical_size, UVec2::new(640, 360));
    assert_eq!(vp2.physical_position, UVec2::new(0, 360));
    assert_eq!(vp2.physical_size, UVec2::new(640, 360));
    assert_eq!(vp3.physical_position, UVec2::new(640, 360));
    assert_eq!(vp3.physical_size, UVec2::new(640, 360));
}

#[test]
fn test_split_screen_viewport_grid_absorbs_odd_dimension_remainder() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1281, 721, 1.0);
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Grid)));
    app.world_mut().insert_resource(ActiveSplitSlotCount(Some(4)));

    let cams: Vec<Entity> = (0..4)
        .map(|i| app.world_mut().spawn((Camera::default(), SplitViewportSlot(i))).id())
        .collect();

    app.world_mut().run_system_once(split_screen_viewport_system).unwrap();

    let vp = |e: Entity| app.world().get::<Camera>(e).unwrap().viewport.clone().unwrap();
    let (vp0, vp1, vp2, vp3) = (vp(cams[0]), vp(cams[1]), vp(cams[2]), vp(cams[3]));

    assert_eq!(vp0.physical_size.x + vp1.physical_size.x, 1281, "row must sum to the full physical width");
    assert_eq!(vp2.physical_size.x + vp3.physical_size.x, 1281, "row must sum to the full physical width");
    assert_eq!(vp0.physical_size.y + vp2.physical_size.y, 721, "column must sum to the full physical height");
    assert_eq!(vp1.physical_size.y + vp3.physical_size.y, 721, "column must sum to the full physical height");
    assert_eq!(vp1.physical_size, UVec2::new(641, 360), "right column absorbs the width remainder");
    assert_eq!(vp3.physical_size, UVec2::new(641, 361), "bottom-right cell absorbs both remainders");
}

#[test]
fn test_split_screen_viewport_grid_count_three_leaves_one_dead_quadrant() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Grid)));
    app.world_mut().insert_resource(ActiveSplitSlotCount(Some(3)));

    // Only 3 cameras exist — slot 3's cell (bottom-right of the 2x2 grid) is simply never
    // rendered to, since no camera claims it. Must not panic.
    let cams: Vec<Entity> = (0..3)
        .map(|i| app.world_mut().spawn((Camera::default(), SplitViewportSlot(i))).id())
        .collect();

    app.world_mut().run_system_once(split_screen_viewport_system).unwrap();

    for cam in &cams {
        assert!(app.world().get::<Camera>(*cam).unwrap().viewport.is_some());
    }
}

#[test]
fn test_split_screen_viewport_grid_noop_when_slot_count_none() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Grid)));
    // ActiveSplitSlotCount defaults to None via init_resource — no explicit insert.

    let cam0 = app.world_mut().spawn((Camera::default(), SplitViewportSlot(0))).id();

    app.world_mut().run_system_once(split_screen_viewport_system).unwrap();

    assert!(
        app.world().get::<Camera>(cam0).unwrap().viewport.is_none(),
        "Grid orientation with no ActiveSplitSlotCount must not touch Camera.viewport"
    );
}

#[test]
fn test_numpad_key_parsing() {
    assert_eq!(InputMap::parse_key("Numpad0"), Some(KeyCode::Numpad0));
    assert_eq!(InputMap::parse_key("Numpad1"), Some(KeyCode::Numpad1));
    assert_eq!(InputMap::parse_key("Numpad4"), Some(KeyCode::Numpad4));
    assert_eq!(InputMap::parse_key("Numpad5"), Some(KeyCode::Numpad5));
    assert_eq!(InputMap::parse_key("Numpad9"), Some(KeyCode::Numpad9));
}

/// A single lowercase ASCII letter is case-insensitive (e.g. `"q"` resolves the same as `"Q"`) —
/// this is what keeps `3rd_person_game_demo`'s existing `key: "i"` action-bar slot alive across
/// `action_bar_custom_hotkeys`'s removal of the old hardcoded `DIGIT_KEYS` table. Only single
/// letters get this leniency; multi-character names stay case-sensitive.
#[test]
fn test_parse_key_single_lowercase_letter_is_case_insensitive() {
    assert_eq!(InputMap::parse_key("q"), Some(KeyCode::KeyQ));
    assert_eq!(InputMap::parse_key("Q"), Some(KeyCode::KeyQ));
    assert_eq!(InputMap::parse_key("i"), Some(KeyCode::KeyI));
}

#[test]
fn test_parse_key_multi_character_names_stay_case_sensitive() {
    assert_eq!(InputMap::parse_key("space"), None, "\"Space\" is valid; \"space\" is not");
    assert_eq!(InputMap::parse_key("keyq"), None, "\"KeyQ\" is valid; \"keyq\" is not");
    assert_eq!(InputMap::parse_key("f2"), None, "\"F2\" is valid; \"f2\" is not");
}

#[test]
fn test_parse_gamepad_button_recognizes_full_supported_set() {
    use bevy::input::gamepad::GamepadButton;
    assert_eq!(InputMap::parse_gamepad_button("South"), Some(GamepadButton::South));
    assert_eq!(InputMap::parse_gamepad_button("East"), Some(GamepadButton::East));
    assert_eq!(InputMap::parse_gamepad_button("North"), Some(GamepadButton::North));
    assert_eq!(InputMap::parse_gamepad_button("West"), Some(GamepadButton::West));
    assert_eq!(InputMap::parse_gamepad_button("LeftTrigger"), Some(GamepadButton::LeftTrigger));
    assert_eq!(InputMap::parse_gamepad_button("LeftTrigger2"), Some(GamepadButton::LeftTrigger2));
    assert_eq!(InputMap::parse_gamepad_button("RightTrigger"), Some(GamepadButton::RightTrigger));
    assert_eq!(InputMap::parse_gamepad_button("RightTrigger2"), Some(GamepadButton::RightTrigger2));
    assert_eq!(InputMap::parse_gamepad_button("Select"), Some(GamepadButton::Select));
    assert_eq!(InputMap::parse_gamepad_button("Start"), Some(GamepadButton::Start));
    assert_eq!(InputMap::parse_gamepad_button("LeftThumb"), Some(GamepadButton::LeftThumb));
    assert_eq!(InputMap::parse_gamepad_button("RightThumb"), Some(GamepadButton::RightThumb));
    assert_eq!(InputMap::parse_gamepad_button("DPadUp"), Some(GamepadButton::DPadUp));
    assert_eq!(InputMap::parse_gamepad_button("DPadDown"), Some(GamepadButton::DPadDown));
    assert_eq!(InputMap::parse_gamepad_button("DPadLeft"), Some(GamepadButton::DPadLeft));
    assert_eq!(InputMap::parse_gamepad_button("DPadRight"), Some(GamepadButton::DPadRight));
}

#[test]
fn test_parse_gamepad_button_unrecognized_name_is_none() {
    assert_eq!(InputMap::parse_gamepad_button("south"), None, "case-sensitive, unlike single-letter keyboard keys");
    assert_eq!(InputMap::parse_gamepad_button("A"), None, "\"South\" is the name, not the Xbox label \"A\"");
    assert_eq!(InputMap::parse_gamepad_button("Triangle"), None, "\"North\" is the name, not the PlayStation label \"Triangle\"");
}

/// Builds `n` player prefabs (`test_player_1`..`test_player_n`), alternating the two shared
/// character models. Only the first player's `camera.split` is set — mirrors
/// `two_player_catalogs_with_split`'s pattern, generalized to N players for Stage 6's Grid tests.
fn n_player_catalogs_with_split(app: &mut App, n: u32, split: Option<SplitScreenDef>) {
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
            ("char_b".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-female-01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));

    let mut prefabs = std::collections::HashMap::new();
    for i in 0..n {
        let camera = if i == 0 {
            let mut c = base_camera_config();
            c.split = split.clone();
            Some(c)
        } else {
            None
        };
        prefabs.insert(format!("test_player_{}", i + 1), PrefabDef {
            kind: PrefabKind::Actor,
            model: if i % 2 == 0 { "char_a".to_string() } else { "char_b".to_string() },
            player_index: i,
            components: PrefabComponents {
                tags: vec!["player".to_string()],
                camera,
                ..Default::default()
            },
            ..Default::default()
        });
    }
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog { prefabs, ..Default::default() }));
}

/// Drives a Replace-mode load of an N-player scene, mirroring `load_two_player_scene`.
fn load_n_player_scene(app: &mut App, n: u32) {
    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let mut entities_ron = String::new();
    for i in 0..n {
        let x = -4.0 + (i as f32) * 3.0;
        entities_ron.push_str(&format!(
            r#"(id: "p{i}", prefab: "test_player_{idx}", transform: (translation: ({x:.1}, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),"#,
            i = i, idx = i + 1, x = x
        ));
    }
    let ron_str = format!(
        "(schema_version: 2, entities: [{entities_ron}], ui: [])"
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

#[test]
fn test_grid_split_spawns_four_viewport_slots_and_sets_slot_count() {
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, 4,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_n_player_scene(&mut app, 4);

    let controller_count = app.world_mut().query::<&CharacterController>().iter(app.world()).count();
    assert_eq!(controller_count, 4, "all 4 players must spawn a CharacterController");

    let mut slots: Vec<u32> = {
        let mut query = app.world_mut().query::<&SplitViewportSlot>();
        query.iter(app.world()).map(|s| s.0).collect()
    };
    slots.sort();
    assert_eq!(slots, vec![0, 1, 2, 3], "expected one SplitViewportSlot per player, 0 through 3");

    assert_eq!(
        app.world().resource::<ActiveSplitSlotCount>().0,
        Some(4),
        "ActiveSplitSlotCount must be set for a Grid scene"
    );
}

#[test]
fn test_vertical_split_scene_leaves_slot_count_none() {
    // Regression: Vertical/Horizontal must NOT populate ActiveSplitSlotCount — only Grid does.
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app,
        None,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
    );
    load_two_player_scene(&mut app);

    assert_eq!(
        app.world().resource::<ActiveSplitSlotCount>().0,
        None,
        "Vertical split must leave ActiveSplitSlotCount at None"
    );
}

#[test]
fn test_grid_split_with_five_players_caps_at_max_and_spawns_fifth_cameraless() {
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, 5,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_n_player_scene(&mut app, 5);

    let controller_count = app.world_mut().query::<&CharacterController>().iter(app.world()).count();
    assert_eq!(controller_count, 5, "all 5 players still spawn a CharacterController");

    let slot_count = app.world_mut().query::<&SplitViewportSlot>().iter(app.world()).count();
    assert_eq!(
        slot_count, MAX_SPLIT_PLAYERS as usize,
        "5th player must spawn cameraless — Grid caps at MAX_SPLIT_PLAYERS"
    );

    assert_eq!(app.world().resource::<ActiveSplitSlotCount>().0, Some(MAX_SPLIT_PLAYERS));
}

// ── local_coop_hot_join_leave.md v1: Action::JoinPlayer ─────────────────────────

/// Builds a Grid-split scene RON starting with `initial` scene-authored players
/// (`test_player_1..initial`, reusing `n_player_catalogs_with_split`'s catalog — call that first
/// with `n = MAX_SPLIT_PLAYERS` so every slot's join prefab already exists), plus
/// `spawn_points`/`join_prefab_keys` entries for every slot `0..MAX_SPLIT_PLAYERS` (slots below
/// `initial` get `None` — already scene-authored — slots at/above it map to
/// `test_player_{slot+1}`, mirroring room8's `[None, None, Some(...), Some(...)]` convention).
/// `spawn_points` use **1-based** `player_N_start` keys (`N = slot + 1`), matching every real
/// project scene (room6/room7/room8) and the `Action::JoinPlayer` executor's own `next_slot + 1`
/// lookup — this is deliberate: an earlier version of this helper used 0-based keys, which
/// matched a since-fixed off-by-one bug in the executor and masked it (alignment-reviewer
/// finding, local_coop_hot_join_leave.md).
fn load_grid_scene_with_join_slots(app: &mut App, initial: u32) {
    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let mut entities_ron = String::new();
    for i in 0..initial {
        let x = -4.0 + (i as f32) * 3.0;
        entities_ron.push_str(&format!(
            r#"(id: "p{i}", prefab: "test_player_{idx}", transform: (translation: ({x:.1}, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),"#,
            i = i, idx = i + 1, x = x
        ));
    }
    let mut spawn_points_ron = String::new();
    let mut join_keys_ron = String::new();
    for slot in 0..MAX_SPLIT_PLAYERS {
        let x = -4.0 + (slot as f32) * 3.0;
        spawn_points_ron.push_str(&format!(
            r#""player_{one_based}_start": ({x:.1}, 0.5, 0.0), "#,
            one_based = slot + 1, x = x
        ));
        if slot < initial {
            join_keys_ron.push_str("None, ");
        } else {
            join_keys_ron.push_str(&format!("Some(\"test_player_{}\"), ", slot + 1));
        }
    }
    let ron_str = format!(
        "(schema_version: 2, entities: [{entities_ron}], ui: [], \
         spawn_points: {{{spawn_points_ron}}}, join_prefab_keys: [{join_keys_ron}])"
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

#[test]
fn test_hot_join_grows_two_player_grid_scene_to_three_then_four() {
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_grid_scene_with_join_slots(&mut app, 2);

    assert_eq!(app.world_mut().query::<&CharacterController>().iter(app.world()).count(), 2);
    assert_eq!(app.world().resource::<ActiveSplitSlotCount>().0, Some(2));
    let existing_indices: std::collections::BTreeSet<u32> = {
        let mut q = app.world_mut().query::<&PlayerIndex>();
        q.iter(app.world()).map(|p| p.0).collect()
    };
    assert_eq!(existing_indices, std::collections::BTreeSet::from([0, 1]));

    // First join: 2 -> 3. One update is enough — action_executor_system and
    // drain_spawn_queue_system are chained within the same frame (see spawn_tests.rs's
    // Action::Spawn tests, which use the same one-update convention).
    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();

    assert_eq!(
        app.world_mut().query::<&CharacterController>().iter(app.world()).count(), 3,
        "3rd player must spawn"
    );
    assert_eq!(app.world().resource::<ActiveSplitSlotCount>().0, Some(3));
    let mut slots: Vec<u32> = {
        let mut q = app.world_mut().query::<&SplitViewportSlot>();
        q.iter(app.world()).map(|s| s.0).collect()
    };
    slots.sort();
    assert_eq!(slots, vec![0, 1, 2], "existing 2 cameras must be untouched, 1 new camera added");
    let indices_after_first: std::collections::BTreeSet<u32> = {
        let mut q = app.world_mut().query::<&PlayerIndex>();
        q.iter(app.world()).map(|p| p.0).collect()
    };
    assert_eq!(
        indices_after_first, std::collections::BTreeSet::from([0, 1, 2]),
        "existing players keep their PlayerIndex; joiner gets a unique new one (slot 2)"
    );

    // Second join: 3 -> 4 (the cap).
    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();

    let lobby_full = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(name) if name == "coop.lobby_full"));
    assert!(lobby_full, "reaching the cap must emit coop.lobby_full");

    assert_eq!(
        app.world_mut().query::<&CharacterController>().iter(app.world()).count(), 4,
        "4th player must spawn"
    );
    assert_eq!(app.world().resource::<ActiveSplitSlotCount>().0, Some(MAX_SPLIT_PLAYERS));
    let mut slots2: Vec<u32> = {
        let mut q = app.world_mut().query::<&SplitViewportSlot>();
        q.iter(app.world()).map(|s| s.0).collect()
    };
    slots2.sort();
    assert_eq!(slots2, vec![0, 1, 2, 3]);
}

#[test]
fn test_hot_join_own_viewport_only_gives_the_joined_players_camera_its_own_reserved_layer() {
    // The one path that resolves own_viewport_only from `Res<TargetRingVisibilityMode>` instead
    // of an in-scope `SplitScreenDef` — proves that read is wired correctly end to end
    // (system-architect/debug-detective finding: this combination was previously untested).
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: true }),
    );
    load_grid_scene_with_join_slots(&mut app, 2);

    assert_eq!(
        *app.world().resource::<TargetRingVisibilityMode>(), TargetRingVisibilityMode::OwnViewportOnly,
        "the resource must resolve to OwnViewportOnly at scene load, before any join happens"
    );

    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();

    let layers_by_slot: std::collections::HashMap<u32, RenderLayers> = {
        let mut query = app.world_mut().query::<(&SplitViewportSlot, &RenderLayers)>();
        query.iter(app.world()).map(|(s, l)| (s.0, l.clone())).collect()
    };
    let joined_layers = layers_by_slot.get(&2).expect("the 3rd (hot-joined) player's camera must carry a RenderLayers component");
    assert!(joined_layers.intersects(&RenderLayers::layer(0)), "hot-joined camera must still see ordinary scene geometry (layer 0)");
    assert!(joined_layers.intersects(&RenderLayers::layer(3)), "hot-joined player (player_index 2) must see its own reserved layer 3 (1 + 2 % 4)");
    assert!(!joined_layers.intersects(&RenderLayers::layer(1)), "hot-joined camera must NOT see player 0's reserved layer 1");
    assert!(!joined_layers.intersects(&RenderLayers::layer(2)), "hot-joined camera must NOT see player 1's reserved layer 2");
}

#[test]
fn test_action_spawn_fallback_camera_gets_no_render_layers_in_own_viewport_only_scene() {
    // Named test for the specific non-insertion path `camera_modes.md` v1's confirmation pass
    // flagged as the real risk of collapsing three camera-spawn call sites into one shared
    // helper: `drain_spawn_queue_system`'s non-hot-join branch (`Action::Spawn`/character-select)
    // spawns its own dedicated full-window Orbit-mode camera via `spawn_player_entity`, which must
    // still get NO `RenderLayers` component (and the existing warn! must still fire) in an
    // `own_viewport_only` scene — unlike hot-join's `spawn_split_camera_for_player`, which
    // correctly gets one (see the test directly above).
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: true }),
    );
    load_grid_scene_with_join_slots(&mut app, 2);
    assert_eq!(*app.world().resource::<TargetRingVisibilityMode>(), TargetRingVisibilityMode::OwnViewportOnly);

    // Simulate a non-hot-join Action::Spawn of a player prefab (e.g. character-select) landing
    // directly in PendingEntitySpawns — the same shape `Action::Spawn`'s executor arm produces,
    // with `is_hot_join: false` (unlike the sibling test above, which pushes `Action::JoinPlayer`).
    app.world_mut().resource_mut::<PendingEntitySpawns>().0.push_back(QueuedSpawn {
        prefab_def: PrefabDef::default(),
        model_path: String::new(),
        transform: Transform::from_xyz(20.0, 0.5, 0.0),
        spawn_id: "spawned_player_test".to_string(),
        prefab_key: "test_player_3".to_string(),
        project_root: String::new(),
        player_config: Some(minimal_player_config(None, 5, None)),
        is_hot_join: false,
    });
    app.update();

    let cameras_without_layers = app.world_mut()
        .query_filtered::<Entity, (With<CameraTargets>, Without<RenderLayers>)>()
        .iter(app.world())
        .count();
    assert_eq!(
        cameras_without_layers, 1,
        "exactly one camera (the Action::Spawn fallback camera) must carry NO RenderLayers \
         component — every pre-existing split-screen camera in this own_viewport_only scene has \
         one, so a count of 1 proves the fallback camera specifically was left componentless, not \
         that RenderLayers insertion broke everywhere"
    );
}

#[test]
fn test_hot_join_spawns_at_the_correct_1_based_spawn_point_not_on_top_of_an_existing_player() {
    // Regression for the alignment-reviewer's off-by-one finding: the executor must resolve
    // spawn_points["player_{next_slot + 1}_start"], not "player_{next_slot}_start" — the latter
    // would place the 3rd joiner (next_slot = 2) at "player_2_start", which is exactly where
    // the scene-authored 2nd player already stands.
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_grid_scene_with_join_slots(&mut app, 2);

    let existing_x_positions: Vec<f32> = {
        let mut q = app.world_mut().query::<(&CharacterController, &Transform)>();
        q.iter(app.world()).map(|(_, t)| t.translation.x).collect()
    };

    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();

    let joiner_x = {
        let mut q = app.world_mut().query::<(&PlayerIndex, &Transform)>();
        q.iter(app.world())
            .find(|(idx, _)| idx.0 == 2)
            .map(|(_, t)| t.translation.x)
            .expect("joiner with PlayerIndex(2) must exist")
    };
    // "player_3_start" (1-based key for slot 2) was authored at x = -4.0 + 2.0 * 3.0 = 2.0 in
    // load_grid_scene_with_join_slots — distinct from every existing player's x position.
    assert_eq!(joiner_x, 2.0, "joiner must land on its own 1-based spawn_points entry");
    for x in existing_x_positions {
        assert_ne!(
            joiner_x, x,
            "joiner must not spawn on top of any pre-existing player's position"
        );
    }
}

#[test]
fn test_hot_join_at_cap_warns_and_noops() {
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_grid_scene_with_join_slots(&mut app, MAX_SPLIT_PLAYERS);
    assert_eq!(app.world().resource::<ActiveSplitSlotCount>().0, Some(MAX_SPLIT_PLAYERS));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();
    app.update();

    assert_eq!(
        app.world_mut().query::<&CharacterController>().iter(app.world()).count(),
        MAX_SPLIT_PLAYERS as usize,
        "join at the cap must no-op — no 5th player"
    );
    assert_eq!(app.world().resource::<ActiveSplitSlotCount>().0, Some(MAX_SPLIT_PLAYERS));
}

#[test]
fn test_hot_join_in_vertical_split_scene_warns_and_noops() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app, None,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
    );
    load_two_player_scene(&mut app);
    assert_eq!(app.world().resource::<ActiveSplitSlotCount>().0, None);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();
    app.update();

    assert_eq!(
        app.world_mut().query::<&CharacterController>().iter(app.world()).count(), 2,
        "Vertical split scenes don't support hot-join in v1 — no-op"
    );
}

#[test]
fn test_hot_join_in_party_scene_warns_and_noops() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app,
        Some(PartyZoomDef { zoom_margin: 2.0, allow_manual_zoom: true }),
        None,
    );
    load_two_player_scene(&mut app);
    assert_eq!(app.world().resource::<ActiveSplitSlotCount>().0, None);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();
    app.update();

    assert_eq!(
        app.world_mut().query::<&CharacterController>().iter(app.world()).count(), 2,
        "party-mode scenes don't support hot-join in v1 — no-op"
    );
}

#[test]
fn test_hot_join_with_no_join_prefab_key_for_next_slot_warns_and_noops() {
    let mut app = setup_test_app();
    app.update();
    // Only 2 catalog prefabs exist and the Grid scene starts at 2 — join_prefab_keys has no
    // entry at all for slot 2 (the scene author simply never configured a 3rd join slot).
    n_player_catalogs_with_split(
        &mut app, 2,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_n_player_scene(&mut app, 2);
    assert_eq!(app.world().resource::<ActiveSplitSlotCount>().0, Some(2));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();
    app.update();

    assert_eq!(
        app.world_mut().query::<&CharacterController>().iter(app.world()).count(), 2,
        "no join_prefab_keys entry for the next slot must no-op, not panic"
    );
}

#[test]
fn test_hot_join_rejects_non_player_tagged_join_prefab() {
    // debug-detective finding: Action::JoinPlayer must mirror Action::Spawn's
    // `tags: ["player"]` guard — without it, a join_prefab_keys typo pointing at a non-player
    // prefab would be silently assembled and spawned as a player.
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    {
        let mut catalog = app.world_mut().resource_mut::<LoadedPrefabCatalog>();
        let joiner = catalog.0.prefabs.get_mut("test_player_3").expect("test_player_3 must exist");
        joiner.components.tags.clear();
    }
    load_grid_scene_with_join_slots(&mut app, 2);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();

    assert_eq!(
        app.world_mut().query::<&CharacterController>().iter(app.world()).count(), 2,
        "a join_prefab_keys entry with no player tag must no-op, not spawn a player"
    );
}

#[test]
fn test_hot_join_rejects_primitive_kind_join_prefab_instead_of_panicking() {
    // debug-detective finding: a primitive-shaped join prefab would otherwise be assembled with
    // PlayerModelSource::Primitive and panic in spawn_player_entity_core, since the hot-join
    // drain branch always passes None for PrimitivePlayerCtx (GLB-only in v1).
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    {
        let mut catalog = app.world_mut().resource_mut::<LoadedPrefabCatalog>();
        let joiner = catalog.0.prefabs.get_mut("test_player_3").expect("test_player_3 must exist");
        joiner.kind = PrefabKind::Primitive;
    }
    load_grid_scene_with_join_slots(&mut app, 2);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();

    assert_eq!(
        app.world_mut().query::<&CharacterController>().iter(app.world()).count(), 2,
        "a primitive-kind join prefab must no-op with a warning, not panic"
    );
}

#[test]
fn test_hot_join_same_frame_double_join_assigns_distinct_slots() {
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_grid_scene_with_join_slots(&mut app, 2);

    // Two JoinPlayer actions queued before any update runs — both are processed by the same
    // action_executor_system pass. Without the queued-is_hot_join-count fix, both would compute
    // next_slot = 2 and collide.
    {
        let mut queue = app.world_mut().resource_mut::<ActionQueue>();
        queue.push(Action::JoinPlayer);
        queue.push(Action::JoinPlayer);
    }
    app.update();
    app.update();

    assert_eq!(
        app.world_mut().query::<&CharacterController>().iter(app.world()).count(), 4,
        "both same-frame joins must spawn distinct players"
    );
    let mut slots: Vec<u32> = {
        let mut q = app.world_mut().query::<&SplitViewportSlot>();
        q.iter(app.world()).map(|s| s.0).collect()
    };
    slots.sort();
    assert_eq!(slots, vec![0, 1, 2, 3], "no slot collision — one joiner got 2, the other 3");
    let indices: std::collections::BTreeSet<u32> = {
        let mut q = app.world_mut().query::<&PlayerIndex>();
        q.iter(app.world()).map(|p| p.0).collect()
    };
    assert_eq!(indices, std::collections::BTreeSet::from([0, 1, 2, 3]));
}

#[test]
fn test_hot_joined_player_stat_bar_duplicates_ranks_like_scene_load_spawned_players() {
    // Validates the scope-cut thesis: a hot-joined player's world_stat_bar goes through the same
    // spawn_player_entity_core -> DynamicStatUiQueue -> drain_dynamic_stat_ui_system path as any
    // other player, so it gets MAX_SPLIT_PLAYERS ranked siblings with zero widget-side changes.
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    {
        let mut catalog = app.world_mut().resource_mut::<LoadedPrefabCatalog>();
        let joiner = catalog.0.prefabs.get_mut("test_player_3").expect("test_player_3 must exist");
        joiner.stat_templates = vec![ironhold_core::schema::stats::StatTemplateDef {
            key: "mana".to_string(),
            base: 40.0,
            min: 0.0,
            max: 100.0,
            regen_rate: 0.0,
            regen_delay: 0.0,
            thresholds: vec![],
        }];
        joiner.world_stat_bar = Some(ascii_world_stat_bar_def("{self}.mana"));
    }
    load_grid_scene_with_join_slots(&mut app, 2);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();
    app.update();
    app.update();

    let fill_rank_count = app.world_mut()
        .query::<&WorldLabelRank>()
        .iter(app.world())
        .count();
    assert!(
        fill_rank_count > 0,
        "hot-joined player's world_stat_bar must spawn ranked siblings via the existing \
         DynamicStatUiQueue path — same as any Action::Spawn-created entity"
    );
}

// ── gamepad_hot_join.md: unclaimed_gamepad_trigger_system + Action::JoinPlayer binding ─────────

/// Builds a minimal but fully-valid `PlayerConfig` for hand-constructing a `QueuedSpawn` —
/// only used to simulate an in-flight `is_hot_join` entry sitting in `PendingEntitySpawns`
/// (production code always builds these via the private `assemble_player_config`, which isn't
/// visible to this external test crate).
fn minimal_player_config(gamepad_index: Option<usize>, player_index: u32, bound_gamepad: Option<Entity>) -> PlayerConfig {
    PlayerConfig {
        model_source: PlayerModelSource::Glb("char_a".to_string()),
        initial_position: (0.0, 0.5, 0.0),
        camera: base_camera_config(),
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

#[test]
fn test_unclaimed_gamepad_trigger_excludes_pad_claimed_by_live_player() {
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_grid_scene_with_join_slots(&mut app, 2);
    app.world_mut().insert_resource(LoadedGamepadBindings(std::collections::HashMap::from([
        ("South".to_string(), "join".to_string()),
    ])));

    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    // Claim the pad exactly like a live player's `gamepad_bind_system` resolution would —
    // `unclaimed_gamepad_trigger_system`'s `claimed` set is sourced from `BoundGamepad` directly,
    // not from the live positional `InputMap.gamepad_index` (gamepad_player_binding_hardening.md).
    {
        let mut q = app.world_mut().query::<&mut BoundGamepad>();
        let mut bound = q.iter_mut(app.world_mut()).next().expect("a live player must exist");
        bound.0 = Some(gamepad);
    }

    press_gamepad_button(&mut app, gamepad, GamepadButton::South);
    app.update();

    assert_eq!(
        app.world().resource::<PendingJoinGamepad>().0, None,
        "a pad already claimed by a live player must never trigger a join"
    );
}

#[test]
fn test_unclaimed_gamepad_trigger_excludes_pad_mid_flight_via_pending_spawn() {
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_grid_scene_with_join_slots(&mut app, 2);
    app.world_mut().insert_resource(LoadedGamepadBindings(std::collections::HashMap::from([
        ("South".to_string(), "join".to_string()),
    ])));

    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    // Simulate an is_hot_join QueuedSpawn already claiming this pad, still undrained — mirrors
    // the `queued_hot_joins` same-frame double-join guard the executor already has.
    // `bound_gamepad: Some(gamepad)` mirrors the real `Action::JoinPlayer` hand-off (the captured
    // pad is written directly to `PlayerConfig.bound_gamepad`, no positional round-trip) — this is
    // exactly what `unclaimed_gamepad_trigger_system`'s `claimed` set now reads for an undrained
    // hot-join entry (`gamepad_player_binding_hardening.md`).
    app.world_mut().resource_mut::<PendingEntitySpawns>().0.push_back(QueuedSpawn {
        prefab_def: PrefabDef::default(),
        model_path: String::new(),
        transform: Transform::IDENTITY,
        spawn_id: "in_flight_test".to_string(),
        prefab_key: "test_player_3".to_string(),
        project_root: String::new(),
        player_config: Some(minimal_player_config(Some(0), 2, Some(gamepad))),
        is_hot_join: true,
    });

    press_gamepad_button(&mut app, gamepad, GamepadButton::South);
    app.update();

    assert_eq!(
        app.world().resource::<PendingJoinGamepad>().0, None,
        "a pad already claimed by an undrained is_hot_join PendingEntitySpawns entry must never \
         trigger a second join"
    );
}

#[test]
fn test_unclaimed_gamepad_trigger_never_captures_a_pad_with_no_press() {
    // "Phantom/dead duplicate pad" regression: the documented Xbox 360 dual-registration quirk
    // produces a SECOND, permanently-dead gamepad entry alongside the real one — connected, but
    // never reporting a press. Simulates that shape directly: two connected pads, only one ever
    // pressed, to prove the never-pressed one is excluded specifically because it never produces
    // a `just_pressed` edge — not merely because only one pad happens to exist in the test.
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_grid_scene_with_join_slots(&mut app, 2);
    app.world_mut().insert_resource(LoadedGamepadBindings(std::collections::HashMap::from([
        ("South".to_string(), "join".to_string()),
    ])));

    let phantom = connect_test_gamepad(&mut app);
    let live = connect_test_gamepad(&mut app);
    app.update();
    app.update(); // no press ever sent to either — the phantom never gets one for real

    assert_eq!(
        app.world().resource::<PendingJoinGamepad>().0, None,
        "merely being connected (never pressed) must never trigger a join, even with a second, \
         live-but-idle pad also connected"
    );

    // Now the live pad presses — must be captured correctly, proving the phantom really was
    // excluded by its lack of a press edge, not by some other accident (e.g. only one pad existing).
    press_gamepad_button(&mut app, live, GamepadButton::South);
    app.update();
    assert_eq!(
        app.world().resource::<PendingJoinGamepad>().0, Some(live),
        "the live pad's press must still be captured correctly once it actually presses"
    );
    let _ = phantom;
}

#[test]
fn test_pending_join_gamepad_is_frame_scoped_not_sticky() {
    // Regression for the system-architect's staleness finding: a gamepad-bound trigger with no
    // Action::JoinPlayer consumer this frame (e.g. a pause button) must never leave a stale pad
    // identity for a later, unrelated keyboard-triggered join to inherit.
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_grid_scene_with_join_slots(&mut app, 2);
    app.world_mut().insert_resource(LoadedGamepadBindings(std::collections::HashMap::from([
        ("Start".to_string(), "toggle_pause".to_string()),
    ])));

    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    press_gamepad_button(&mut app, gamepad, GamepadButton::Start);
    app.update();
    assert_eq!(
        app.world().resource::<PendingJoinGamepad>().0, Some(gamepad),
        "detection captures ANY LoadedGamepadBindings match this frame, not only a literal join"
    );

    // Next frame: no new press. The resource must reset, not carry the pad forward.
    app.update();
    assert_eq!(
        app.world().resource::<PendingJoinGamepad>().0, None,
        "PendingJoinGamepad must not survive into a frame with no new qualifying press"
    );

    // A keyboard-triggered join processed now must not inherit the stale pad identity.
    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();

    let joiner_gamepad_index = {
        let mut q = app.world_mut().query::<(&PlayerIndex, &CharacterController)>();
        q.iter(app.world())
            .find(|(idx, _)| idx.0 == 2)
            .map(|(_, c)| c.inputs.gamepad_index)
            .expect("joiner with PlayerIndex(2) must exist")
    };
    assert_eq!(
        joiner_gamepad_index, None,
        "a keyboard-triggered join must never inherit a stale pad identity from an earlier, \
         unrelated gamepad button press"
    );
}

#[test]
fn test_two_gamepads_pressed_same_frame_captures_only_lowest_sorted_index() {
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_grid_scene_with_join_slots(&mut app, 2);
    app.world_mut().insert_resource(LoadedGamepadBindings(std::collections::HashMap::from([
        ("South".to_string(), "join".to_string()),
    ])));

    let gp_a = connect_test_gamepad(&mut app);
    let gp_b = connect_test_gamepad(&mut app);
    app.update();

    let lowest = if gp_a.index() < gp_b.index() { gp_a } else { gp_b };

    press_gamepad_button(&mut app, gp_a, GamepadButton::South);
    press_gamepad_button(&mut app, gp_b, GamepadButton::South);
    app.update();

    assert_eq!(
        app.world().resource::<PendingJoinGamepad>().0, Some(lowest),
        "exactly one pad's press is captured per frame, deterministically the lowest sorted index"
    );

    // Regression for the debug-detective/system-architect finding: an earlier version capped
    // only the PendingJoinGamepad *capture*, not the UiEvent *emission* — so a second unclaimed
    // pad's simultaneous press still fired its own "join" event, which the rules pipeline would
    // turn into a second Action::JoinPlayer with no pad bound (a permanently half-controlled
    // player, since v1 has no hot-leave to undo it). Assert directly on the emitted messages,
    // not just the resource, so this can't silently regress the same way again.
    let join_event_count = app.world()
        .resource::<Messages<UiEvent>>()
        .iter_current_update_messages()
        .filter(|e| matches!(e, UiEvent::ButtonPressed(t) if t == "join"))
        .count();
    assert_eq!(
        join_event_count, 1,
        "only one pad's press may be serviced (emitted as a UiEvent) per frame, even though two \
         unclaimed pads pressed the bound button in the same frame"
    );
}

/// `gamepad_player_binding_hardening.md`: a hot-joined player's `BoundGamepad` is set directly
/// from the pad that pressed the join button (`PlayerConfig.bound_gamepad`, drained from
/// `PendingJoinGamepad`) — no round-trip through `InputMap.gamepad_index`'s sorted *position* and
/// no re-resolution window through `gamepad_bind_system`'s pending-bind retry a frame later.
/// Regression-relevant: the join prefab (`test_player_3`, `n_player_catalogs_with_split`) sets no
/// `gamepad_index` at all, so a positional round-trip would leave the joiner permanently unbound
/// — the exact bug this direct hand-off exists to close.
#[test]
fn test_two_gamepads_join_on_consecutive_frames_each_bind_via_bound_gamepad_directly() {
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, MAX_SPLIT_PLAYERS,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_grid_scene_with_join_slots(&mut app, 2);
    app.world_mut().insert_resource(LoadedGamepadBindings(std::collections::HashMap::from([
        ("South".to_string(), "join".to_string()),
    ])));

    let gp_a = connect_test_gamepad(&mut app);
    let gp_b = connect_test_gamepad(&mut app);
    app.update();

    // Frame 1: gamepad A presses join and a JoinPlayer is queued in the same frame (mirrors how
    // the real rules pipeline fires it same-frame via ui.button_pressed:join).
    press_gamepad_button(&mut app, gp_a, GamepadButton::South);
    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();

    let first_joiner_bound_gamepad = {
        let mut q = app.world_mut().query::<(&PlayerIndex, &BoundGamepad)>();
        q.iter(app.world())
            .find(|(idx, _)| idx.0 == 2)
            .map(|(_, b)| b.0)
            .expect("first joiner with PlayerIndex(2) must exist")
    };
    assert_eq!(
        first_joiner_bound_gamepad, Some(gp_a),
        "first joiner must be bound directly to the exact gamepad Entity that pressed join, \
         even though that pad's own prefab authors no gamepad_index seed at all"
    );

    // Frame 2: gamepad B presses join.
    press_gamepad_button(&mut app, gp_b, GamepadButton::South);
    app.world_mut().resource_mut::<ActionQueue>().push(Action::JoinPlayer);
    app.update();

    let second_joiner_bound_gamepad = {
        let mut q = app.world_mut().query::<(&PlayerIndex, &BoundGamepad)>();
        q.iter(app.world())
            .find(|(idx, _)| idx.0 == 3)
            .map(|(_, b)| b.0)
            .expect("second joiner with PlayerIndex(3) must exist")
    };
    assert_eq!(
        second_joiner_bound_gamepad, Some(gp_b),
        "second joiner must be bound directly to gamepad B, distinct from the first joiner's binding"
    );
    // The first joiner's own binding must be completely unaffected by the second join.
    let first_joiner_bound_gamepad_after = {
        let mut q = app.world_mut().query::<(&PlayerIndex, &BoundGamepad)>();
        q.iter(app.world())
            .find(|(idx, _)| idx.0 == 2)
            .map(|(_, b)| b.0)
            .expect("first joiner must still exist")
    };
    assert_eq!(
        first_joiner_bound_gamepad_after, Some(gp_a),
        "the first joiner's binding must not change when a second player joins later"
    );
}

// ── gamepad_action_bar_slots.md: gamepad-routed ActionSlotUi ────────────────────

/// Core correctness case: a `gamepad_key`-bound slot owned by player 1 must fire only from
/// player 1's own configured gamepad — pressing the same button on a *different* connected pad
/// must not fire it, even though both pads are live and pressed.
#[test]
fn test_gamepad_action_bar_slot_fires_only_from_owning_players_own_pad() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;

    let mut app = setup_test_app();
    app.update();

    let gp_a = connect_test_gamepad(&mut app);
    let gp_b = connect_test_gamepad(&mut app);
    app.update();

    let mut sorted = [gp_a, gp_b];
    sorted.sort_by_key(|e| e.index());
    let b_index = sorted.iter().position(|&e| e == gp_b).unwrap();

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: None,
        resolved_gamepad_button: Some(GamepadButton::South),
        do_actions: vec![Action::SetVariable("p1_fired".to_string(), "yes".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: Some(1),
    });

    // Player 1 is bound to gamepad B directly — not A's. `BoundGamepad` (not the seed-only
    // `inputs.gamepad_index`) is what `action_bar_input_system` actually reads post-refactor.
    let mut controller = test_character_controller();
    controller.inputs.gamepad_index = Some(b_index);
    app.world_mut().spawn((
        SpawnId("player_02".to_string()),
        controller,
        PlayerTarget::default(),
        PlayerIndex(1),
        BoundGamepad(Some(gp_b)),
    ));

    // Press South on gamepad A (NOT player 1's own pad) — must not fire.
    press_gamepad_button(&mut app, gp_a, GamepadButton::South);
    app.update();
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("p1_fired"), None,
        "a press on a different player's/unclaimed pad must never fire this slot"
    );

    // Press South on gamepad B (player 1's own pad) — must fire.
    press_gamepad_button(&mut app, gp_b, GamepadButton::South);
    app.update();
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("p1_fired").map(String::as_str), Some("yes"),
        "a press on the owning player's own gamepad must fire the slot"
    );
}

/// The plan's headline acceptance criterion, tested directly at runtime (not just proven
/// indirectly via the unclaimed-pad case above): two *live* players, each with their own
/// gamepad and their own bar, both defaulting `gamepad_key: "South"` — pressing one player's own
/// pad must fire only that player's slot, never the other's, even though both are simultaneously
/// live and bound to the identical button name.
#[test]
fn test_two_players_two_pads_same_gamepad_key_each_fires_only_their_own_slot() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;

    let mut app = setup_test_app();
    app.update();

    let gp_a = connect_test_gamepad(&mut app);
    let gp_b = connect_test_gamepad(&mut app);
    app.update();

    let mut sorted = [gp_a, gp_b];
    sorted.sort_by_key(|e| e.index());
    let a_index = sorted.iter().position(|&e| e == gp_a).unwrap();
    let b_index = sorted.iter().position(|&e| e == gp_b).unwrap();

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: None,
        resolved_gamepad_button: Some(GamepadButton::South),
        do_actions: vec![Action::SetVariable("p0_fired".to_string(), "yes".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: Some(0),
    });
    app.world_mut().spawn(ActionSlotUi {
        slot_key: "2".to_string(),
        resolved_key: None,
        resolved_gamepad_button: Some(GamepadButton::South),
        do_actions: vec![Action::SetVariable("p1_fired".to_string(), "yes".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: Some(1),
    });

    let mut controller0 = test_character_controller();
    controller0.inputs.gamepad_index = Some(a_index);
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        controller0,
        PlayerTarget::default(),
        PlayerIndex(0),
        BoundGamepad(Some(gp_a)),
    ));
    let mut controller1 = test_character_controller();
    controller1.inputs.gamepad_index = Some(b_index);
    app.world_mut().spawn((
        SpawnId("player_02".to_string()),
        controller1,
        PlayerTarget::default(),
        PlayerIndex(1),
        BoundGamepad(Some(gp_b)),
    ));

    // Player 0 presses their own pad — only player 0's slot fires.
    press_gamepad_button(&mut app, gp_a, GamepadButton::South);
    app.update();
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("p0_fired").map(String::as_str), Some("yes"),
        "player 0's own pad press must fire player 0's slot"
    );
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("p1_fired"), None,
        "player 0's press must never fire player 1's slot, even though both bind the same button name"
    );

    // Player 1 presses their own pad — only player 1's slot fires (player 0's stays as set above,
    // unaffected — proving this press didn't re-trigger it).
    press_gamepad_button(&mut app, gp_b, GamepadButton::South);
    app.update();
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("p1_fired").map(String::as_str), Some("yes"),
        "player 1's own pad press must fire player 1's slot"
    );
}

/// A slot with both `key` and `gamepad_key` bound must fire from either device independently.
#[test]
fn test_gamepad_action_bar_slot_with_both_key_and_gamepad_key_fires_from_either_device() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;
    use ironhold_core::schema::stats::{LoadedStats, LiveStat};

    let mut app = setup_test_app();
    app.update();
    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    // "tally" accumulates +1 per actual fire (unlike the SetVariable below, which just writes a
    // constant and can't distinguish one fire from two) — used at the end of this test to prove a
    // slot with both devices pressed in the SAME frame still fires exactly once, not twice.
    app.world_mut().resource_mut::<LoadedStats>().0.insert("tally".to_string(), LiveStat::new(stat_def(0.0, 999.0)));

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: Some(GamepadButton::South),
        do_actions: vec![
            Action::SetVariable("fire_count".to_string(), "1".to_string()),
            Action::ModifyStat { key: "tally".to_string(), delta: 1.0 },
        ],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });
    let mut controller = test_character_controller();
    controller.inputs.gamepad_index = Some(0);
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        controller,
        PlayerTarget::default(),
        BoundGamepad(Some(gamepad)),
    ));

    // Keyboard press alone fires it.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("fire_count").map(String::as_str), Some("1"),
        "keyboard press must fire a slot with both key and gamepad_key bound"
    );
    app.world_mut().resource_mut::<GameVariables>().0.remove("fire_count");
    // `release` alone only clears `pressed`/sets `just_released` — `just_pressed` would otherwise
    // latch true forever with no InputPlugin registered to clear it each frame (this test harness
    // deliberately runs on MinimalPlugins, see support/mod.rs), which would make the "gamepad
    // alone" assertion below pass for the wrong reason (a stale keyboard just_pressed bit, not the
    // actual gamepad press). Must clear both explicitly — see the identical pattern this test
    // borrows from elsewhere in this file.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::Digit1);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear_just_pressed(KeyCode::Digit1);
    app.update();

    // Gamepad press alone (no keyboard) also fires it.
    press_gamepad_button(&mut app, gamepad, GamepadButton::South);
    app.update();
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("fire_count").map(String::as_str), Some("1"),
        "gamepad press must also fire the same slot, independent of the keyboard binding"
    );
    assert_eq!(
        app.world().resource::<LoadedStats>().0["tally"].current, 2.0,
        "sanity check: tally must be exactly 2 after the two independent single-device fires above"
    );

    // Both devices pressed in the SAME frame — the slot must still fire exactly once, not twice
    // (it's a single `for slot in slots.iter()` iteration gated by `keyboard_fired ||
    // gamepad_fired`, not two independent fire paths).
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    press_gamepad_button(&mut app, gamepad, GamepadButton::South);
    app.update();
    assert_eq!(
        app.world().resource::<LoadedStats>().0["tally"].current, 3.0,
        "pressing both keyboard and gamepad in the same frame must fire the slot exactly once (tally 2 -> 3), not twice"
    );
}

/// An unparseable `gamepad_key` — represented here directly as `resolved_gamepad_button: None`,
/// mirroring what the scene loader resolves it to (the loader's own `warn!` is a scene-load-time
/// concern, tested via `ironhold_cli validate`, not this runtime system) — must never fire from
/// gamepad, while any keyboard binding on the same slot keeps working unaffected.
#[test]
fn test_action_bar_slot_with_unresolved_gamepad_button_never_fires_from_gamepad_keyboard_still_works() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;

    let mut app = setup_test_app();
    app.update();
    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None, // unparseable gamepad_key resolves to None
        do_actions: vec![Action::SetVariable("fired".to_string(), "yes".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });
    let mut controller = test_character_controller();
    controller.inputs.gamepad_index = Some(0);
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        controller,
        PlayerTarget::default(),
        BoundGamepad(Some(gamepad)),
    ));

    // A gamepad press can't fire it — there's no resolved button to check `just_pressed` against.
    press_gamepad_button(&mut app, gamepad, GamepadButton::South);
    app.update();
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("fired"), None,
        "a slot with no resolved gamepad button must never fire from gamepad"
    );

    // The keyboard binding is unaffected.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("fired").map(String::as_str), Some("yes"),
        "the slot's keyboard binding must still work when gamepad_key is unresolved"
    );
}

/// Regression: an ordinary keyboard-only slot (no `gamepad_key` authored at all) must behave
/// exactly as before this feature — including the on-unmatched-owner cooldown-event path, which
/// the restructured `action_bar_input_system` must still emit without ever needing to resolve a
/// player when the slot has no gamepad binding.
#[test]
fn test_keyboard_only_action_bar_slot_on_cooldown_emits_event_without_gamepad_binding() {
    use ironhold_core::capabilities::action_bar::{ActionSlotUi, CooldownMap};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("fired".to_string(), "yes".to_string())],
        cooldown_secs: Some(5.0),
        cost: None,
        owner_player: None,
    });
    app.world_mut().resource_mut::<CooldownMap>().0.insert("1".to_string(), (3.0, 5.0));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    let on_cooldown = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(t) if t == "action_bar.on_cooldown:1"));
    assert!(on_cooldown, "keyboard-only slot on cooldown must still emit action_bar.on_cooldown, unchanged by this feature");
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("fired"), None,
        "a slot on cooldown must not fire, regardless of gamepad_key"
    );
}

/// The gamepad-only-fire cooldown gate is genuinely new code (the keyboard cooldown check above
/// runs before player resolution and can't cover this case — a gamepad press needs the owning
/// player's own `gamepad_index`, resolved only after `players.iter().find(...)` succeeds). Without
/// this second, symmetric check, a gamepad press on a cooling-down slot would bypass the cooldown
/// gate entirely: fire `do_actions` off-cooldown and emit no `action_bar.on_cooldown` event.
#[test]
fn test_gamepad_only_action_bar_slot_on_cooldown_emits_event_and_does_not_fire() {
    use ironhold_core::capabilities::action_bar::{ActionSlotUi, CooldownMap};

    let mut app = setup_test_app();
    app.update();
    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: None,
        resolved_gamepad_button: Some(GamepadButton::South),
        do_actions: vec![Action::SetVariable("fired".to_string(), "yes".to_string())],
        cooldown_secs: Some(5.0),
        cost: None,
        owner_player: None,
    });
    app.world_mut().resource_mut::<CooldownMap>().0.insert("1".to_string(), (3.0, 5.0));

    let mut controller = test_character_controller();
    controller.inputs.gamepad_index = Some(0);
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        controller,
        PlayerTarget::default(),
        BoundGamepad(Some(gamepad)),
    ));

    press_gamepad_button(&mut app, gamepad, GamepadButton::South);
    app.update();

    let on_cooldown = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(t) if t == "action_bar.on_cooldown:1"));
    assert!(on_cooldown, "gamepad-only press on a cooling-down slot must emit action_bar.on_cooldown");
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("fired"), None,
        "a slot on cooldown must not fire from a gamepad press either"
    );
}

// ── Stage 5: dynamic_split_screen_system (unit-level) ───────────────────────────

fn test_orbit_state() -> ironhold_core::capabilities::camera::OrbitState {
    ironhold_core::capabilities::camera::OrbitState {
        radius: 10.0,
        offset: Vec3::new(0.0, 4.5, 9.0),
        zoom_speed: 0.0,
        orbit_speed: 0.4,
        min_radius: 4.5,
        max_radius: 9.0,
        pitch: 0.5,
        yaw: 0.0,
        look_at_offset: Vec3::ZERO,
        min_pitch: 0.1,
        max_pitch: 0.9,
        orbit_lmb: false,
        orbit_rmb: false,
        character_rotate_lmb: false,
        character_rotate_rmb: false,
        look_left_key: None,
        look_right_key: None,
        look_up_key: None,
        look_down_key: None,
        look_speed: 2.0,
        gamepad_deadzone: 0.15,
    }
}

fn test_orbit_camera(target: Entity) -> (ActiveCameraMode, OrbitCameraMode, CameraTargets) {
    (ActiveCameraMode::Orbit(test_orbit_state()), OrbitCameraMode, CameraTargets(vec![target]))
}

/// Reads back the `OrbitState` payload of an `Orbit`-mode camera spawned via `test_orbit_camera`/
/// `test_orbit_state`. Panics if `camera` isn't currently in `Orbit` mode — every call site here
/// spawns one directly, so a mismatch means the test itself is wrong, not a real runtime case.
fn get_orbit<'a>(app: &'a App, camera: Entity) -> &'a ironhold_core::capabilities::camera::OrbitState {
    match app.world().get::<ActiveCameraMode>(camera).unwrap() {
        ActiveCameraMode::Orbit(o) => o,
        _ => panic!("expected an Orbit-mode camera at {camera:?}"),
    }
}

fn test_party_orbit_camera(targets: Vec<Entity>) -> (ActiveCameraMode, PartyCameraMode, CameraTargets) {
    (ActiveCameraMode::Party(ironhold_core::capabilities::camera::PartyState {
        zoom_margin: 3.0,
        allow_manual_zoom: false,
        manual_zoom_offset: 0.0,
        zoom_speed: 10.0,
        orbit_speed: 0.5,
        min_radius: 4.0,
        max_radius: 20.0,
        pitch: 0.5,
        yaw: 0.0,
        look_at_offset: Vec3::ZERO,
        min_pitch: 0.1,
        max_pitch: 0.9,
        orbit_lmb: true,
        orbit_rmb: true,
    }), PartyCameraMode, CameraTargets(targets))
}

/// Spawns the minimal 3-camera rig `dynamic_split_screen_system` operates on: two "player"
/// entities (just a `Transform` — the system only reads position), two split cameras each
/// tagged `OrbitCamera{target}` + `SplitViewportSlot`, and one party camera tagged
/// `PartyOrbitCamera` — mirroring the real shape `spawn_players_and_camera`'s dynamic branch
/// produces, minus everything the system itself doesn't touch (no Actor model, no
/// `CharacterController`). Returns `(cam0, cam1, party_cam)` for asserting on `Camera.is_active`.
fn spawn_dynamic_rig(app: &mut App, p0_pos: Vec3, p1_pos: Vec3, split_active: bool) -> (Entity, Entity, Entity) {
    let p0 = app.world_mut().spawn(Transform::from_translation(p0_pos)).id();
    let p1 = app.world_mut().spawn(Transform::from_translation(p1_pos)).id();
    let cam0 = app.world_mut().spawn((
        Camera { is_active: split_active, order: 0, ..default() },
        test_orbit_camera(p0),
        SplitViewportSlot(0),
    )).id();
    let cam1 = app.world_mut().spawn((
        Camera { is_active: split_active, order: 1, ..default() },
        test_orbit_camera(p1),
        SplitViewportSlot(1),
    )).id();
    let party_cam = app.world_mut().spawn((
        Camera { is_active: !split_active, order: 2, ..default() },
        test_party_orbit_camera(vec![p0, p1]),
    )).id();
    (cam0, cam1, party_cam)
}

fn dynamic_config(split_distance: f32, merge_distance: f32) -> DynamicSplitConfig {
    DynamicSplitConfig(Some(DynamicSplitDef {
        split_distance,
        merge_distance,
        merged_zoom_margin: 3.0,
        merged_allow_manual_zoom: false,
    }))
}

#[test]
fn test_dynamic_split_stays_merged_within_hysteresis_band() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(dynamic_config(10.0, 6.0));
    app.world_mut().insert_resource(ActiveSplitScreen(None));
    let (cam0, cam1, party) = spawn_dynamic_rig(&mut app, Vec3::new(-4.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0), false);

    app.world_mut().run_system_once(dynamic_split_screen_system).unwrap();

    assert_eq!(app.world().resource::<ActiveSplitScreen>().0, None, "distance 8.0 is within the hysteresis band, not past split_distance — must stay merged");
    assert!(app.world().get::<Camera>(party).unwrap().is_active);
    assert!(!app.world().get::<Camera>(cam0).unwrap().is_active);
    assert!(!app.world().get::<Camera>(cam1).unwrap().is_active);
}

#[test]
fn test_dynamic_split_stays_split_within_hysteresis_band() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(dynamic_config(10.0, 6.0));
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Vertical)));
    // Same 8.0 separation as the merged test above, but starting split — hysteresis must keep
    // it split (8.0 is not below merge_distance 6.0), proving the same physical distance can be
    // valid in either state depending on which side of the band it was approached from.
    let (cam0, cam1, party) = spawn_dynamic_rig(&mut app, Vec3::new(-4.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0), true);

    app.world_mut().run_system_once(dynamic_split_screen_system).unwrap();

    assert_eq!(app.world().resource::<ActiveSplitScreen>().0, Some(SplitOrientation::Vertical));
    assert!(!app.world().get::<Camera>(party).unwrap().is_active);
    assert!(app.world().get::<Camera>(cam0).unwrap().is_active);
    assert!(app.world().get::<Camera>(cam1).unwrap().is_active);
}

#[test]
fn test_dynamic_split_transitions_to_vertical_split_past_split_distance() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(dynamic_config(10.0, 6.0));
    app.world_mut().insert_resource(ActiveSplitScreen(None));
    // dx = 15.0, dz = 0.0 -> horizontal separation dominates -> Vertical (side-by-side) split.
    let (cam0, cam1, party) = spawn_dynamic_rig(&mut app, Vec3::new(0.0, 0.0, 0.0), Vec3::new(15.0, 0.0, 0.0), false);

    app.world_mut().run_system_once(dynamic_split_screen_system).unwrap();

    assert_eq!(app.world().resource::<ActiveSplitScreen>().0, Some(SplitOrientation::Vertical));
    assert!(!app.world().get::<Camera>(party).unwrap().is_active);
    assert!(app.world().get::<Camera>(cam0).unwrap().is_active);
    assert!(app.world().get::<Camera>(cam1).unwrap().is_active);
}

#[test]
fn test_dynamic_split_transitions_to_horizontal_split_when_depth_separation_dominates() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(dynamic_config(10.0, 6.0));
    app.world_mut().insert_resource(ActiveSplitScreen(None));
    // dx = 0.0, dz = 15.0 -> depth separation dominates -> Horizontal (top/bottom) split.
    let (cam0, cam1, party) = spawn_dynamic_rig(&mut app, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 15.0), false);

    app.world_mut().run_system_once(dynamic_split_screen_system).unwrap();

    assert_eq!(app.world().resource::<ActiveSplitScreen>().0, Some(SplitOrientation::Horizontal));
    assert!(!app.world().get::<Camera>(party).unwrap().is_active);
    assert!(app.world().get::<Camera>(cam0).unwrap().is_active);
    assert!(app.world().get::<Camera>(cam1).unwrap().is_active);
}

#[test]
fn test_dynamic_split_transitions_to_merged_below_merge_distance() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(dynamic_config(10.0, 6.0));
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Vertical)));
    let (cam0, cam1, party) = spawn_dynamic_rig(&mut app, Vec3::new(-2.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), true);

    app.world_mut().run_system_once(dynamic_split_screen_system).unwrap();

    assert_eq!(app.world().resource::<ActiveSplitScreen>().0, None, "distance 4.0 is below merge_distance 6.0 -> must merge");
    assert!(app.world().get::<Camera>(party).unwrap().is_active);
    assert!(!app.world().get::<Camera>(cam0).unwrap().is_active);
    assert!(!app.world().get::<Camera>(cam1).unwrap().is_active);
}

#[test]
fn test_dynamic_split_orientation_stays_locked_while_already_split() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(dynamic_config(10.0, 6.0));
    // Already split, locked Vertical from an earlier transition. Now positioned so depth
    // separation dominates (would pick Horizontal if freshly transitioning) but still far apart
    // (distance 15.0 stays above merge_distance 6.0, so no merge->re-split cycle happens).
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Vertical)));
    let (cam0, cam1, party) = spawn_dynamic_rig(&mut app, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 15.0), true);

    app.world_mut().run_system_once(dynamic_split_screen_system).unwrap();

    assert_eq!(
        app.world().resource::<ActiveSplitScreen>().0,
        Some(SplitOrientation::Vertical),
        "orientation must stay locked to whatever it was at the last merge->split transition, \
         not recomputed every frame while already split"
    );
    assert!(!app.world().get::<Camera>(party).unwrap().is_active);
    assert!(app.world().get::<Camera>(cam0).unwrap().is_active);
    assert!(app.world().get::<Camera>(cam1).unwrap().is_active);
}

#[test]
fn test_dynamic_split_system_is_noop_when_config_none() {
    let mut app = setup_test_app();
    app.update();
    // DynamicSplitConfig defaults to None via init_resource — no explicit insert needed.
    app.world_mut().insert_resource(ActiveSplitScreen(None));
    let (cam0, cam1, party) = spawn_dynamic_rig(&mut app, Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0), false);

    app.world_mut().run_system_once(dynamic_split_screen_system).unwrap();

    assert_eq!(app.world().resource::<ActiveSplitScreen>().0, None);
    assert!(app.world().get::<Camera>(party).unwrap().is_active);
    assert!(!app.world().get::<Camera>(cam0).unwrap().is_active);
    assert!(!app.world().get::<Camera>(cam1).unwrap().is_active);
}

#[test]
fn test_dynamic_split_guards_fewer_than_two_split_cameras() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(dynamic_config(10.0, 6.0));
    app.world_mut().insert_resource(ActiveSplitScreen(None));
    // Only one split camera exists (e.g. mid scene-transition) — must not panic.
    let p0 = app.world_mut().spawn(Transform::from_xyz(0.0, 0.0, 0.0)).id();
    app.world_mut().spawn((
        Camera { is_active: false, order: 0, ..default() },
        test_orbit_camera(p0),
        SplitViewportSlot(0),
    ));

    app.world_mut().run_system_once(dynamic_split_screen_system).unwrap();

    assert_eq!(app.world().resource::<ActiveSplitScreen>().0, None, "must no-op, not panic, when fewer than 2 split cameras exist");
}

// ── Stage 5: spawn_players_and_camera's dynamic branch (scene-load level) ───────

#[test]
fn test_dynamic_split_initial_state_starts_split_when_distance_exceeds_threshold() {
    let mut app = setup_test_app();
    app.update();
    // load_two_player_scene spawns players at (-4,0.5,0) and (4,0.5,0) -> distance 8.0.
    two_player_catalogs_with_split(
        &mut app,
        None,
        Some(SplitScreenDef {
            orientation: SplitOrientation::Vertical,
            dynamic: Some(DynamicSplitDef { split_distance: 5.0, merge_distance: 3.0, merged_zoom_margin: 3.0, merged_allow_manual_zoom: false }),
            own_viewport_only: false,
        }),
    );
    load_two_player_scene(&mut app);

    assert_eq!(app.world().resource::<ActiveSplitScreen>().0, Some(SplitOrientation::Vertical), "8.0 > split_distance 5.0 -> must start split");

    let party_active: Vec<bool> = {
        let mut q = app.world_mut().query_filtered::<&Camera, With<PartyCameraMode>>();
        q.iter(app.world()).map(|c| c.is_active).collect()
    };
    assert_eq!(party_active, vec![false], "party camera must start inactive when the scene starts split");

    let split_cams: Vec<(bool, isize)> = {
        let mut q = app.world_mut().query_filtered::<&Camera, With<SplitViewportSlot>>();
        q.iter(app.world()).map(|c| (c.is_active, c.order)).collect()
    };
    assert_eq!(split_cams.len(), 2);
    assert!(split_cams.iter().all(|(active, _)| *active), "both split cameras must start active when the scene starts split");
    let mut orders: Vec<isize> = split_cams.iter().map(|(_, o)| *o).collect();
    orders.sort();
    assert_eq!(orders, vec![0, 1], "split cameras must keep distinct orders 0/1 even in dynamic mode");
}

#[test]
fn test_dynamic_split_initial_state_starts_merged_when_within_threshold() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app,
        None,
        Some(SplitScreenDef {
            orientation: SplitOrientation::Vertical,
            dynamic: Some(DynamicSplitDef { split_distance: 12.0, merge_distance: 6.0, merged_zoom_margin: 3.0, merged_allow_manual_zoom: false }),
            own_viewport_only: false,
        }),
    );
    load_two_player_scene(&mut app);

    assert_eq!(app.world().resource::<ActiveSplitScreen>().0, None, "8.0 < split_distance 12.0 -> must start merged");

    let party_active: Vec<bool> = {
        let mut q = app.world_mut().query_filtered::<&Camera, With<PartyCameraMode>>();
        q.iter(app.world()).map(|c| c.is_active).collect()
    };
    assert_eq!(party_active, vec![true], "party camera must start active when the scene starts merged");

    let split_active: Vec<bool> = {
        let mut q = app.world_mut().query_filtered::<&Camera, With<SplitViewportSlot>>();
        q.iter(app.world()).map(|c| c.is_active).collect()
    };
    assert!(split_active.iter().all(|active| !active), "both split cameras must start inactive when the scene starts merged");
}

#[test]
fn test_dynamic_split_merge_distance_clamped_when_not_less_than_split_distance() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app,
        None,
        Some(SplitScreenDef {
            orientation: SplitOrientation::Vertical,
            // Authored backwards on purpose — must warn and clamp, not panic or misbehave.
            dynamic: Some(DynamicSplitDef { split_distance: 5.0, merge_distance: 6.0, merged_zoom_margin: 3.0, merged_allow_manual_zoom: false }),
            own_viewport_only: false,
        }),
    );
    load_two_player_scene(&mut app);

    let config = app.world().resource::<DynamicSplitConfig>().0.clone().expect("dynamic config must still be Some after clamping");
    assert_eq!(config.split_distance, 5.0, "split_distance is untouched by the clamp");
    assert!(config.merge_distance < config.split_distance, "merge_distance must be clamped below split_distance, got {}", config.merge_distance);
}

// ── Player HUD labels: split_viewport_player_label_spawn_system / update_system ─────

#[test]
fn test_split_labels_spawn_once_per_grid_camera_with_player_index() {
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, 4,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_n_player_scene(&mut app, 4);
    app.update();

    let linked: Vec<Entity> = {
        let mut q = app.world_mut().query_filtered::<&LinkedPlayerLabel, With<SplitScreenPlayerLabel>>();
        q.iter(app.world()).map(|l| l.0).collect()
    };
    assert_eq!(linked.len(), 4, "every one of the 4 Grid split cameras must get exactly one linked label");
    let mut distinct = linked.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 4, "labels must be distinct entities, not shared/aliased");

    // Extra frames must not spawn duplicates — Added<SplitViewportSlot> only fires once.
    app.update();
    app.update();
    let linked_after = app.world_mut().query::<&LinkedPlayerLabel>().iter(app.world()).count();
    assert_eq!(linked_after, 4, "no duplicate labels spawn on later frames");
}

#[test]
fn test_split_label_text_and_color_match_player_index_not_material() {
    let mut app = setup_test_app();
    app.update();
    n_player_catalogs_with_split(
        &mut app, 4,
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None, own_viewport_only: false }),
    );
    load_n_player_scene(&mut app, 4);
    app.update();

    let cams: Vec<(Entity, Entity)> = {
        let mut q = app.world_mut().query::<(&CameraTargets, &LinkedPlayerLabel)>();
        q.iter(app.world()).filter_map(|(t, l)| t.0.first().copied().map(|target| (target, l.0))).collect()
    };
    assert_eq!(cams.len(), 4);
    for (target, label_entity) in cams {
        let idx = app.world().get::<PlayerIndex>(target)
            .expect("split camera target must carry PlayerIndex").0;
        let text = app.world().get::<Text>(label_entity).unwrap();
        assert_eq!(
            text.0, format!("P{}", idx + 1),
            "label text must read the target's PlayerIndex, not spawn/slot order"
        );
        let color = app.world().get::<TextColor>(label_entity).unwrap();
        assert_eq!(
            color.0, PLAYER_LABEL_COLORS[idx as usize],
            "label color must come from the fixed palette, independent of any material field \
             (rooms 3/4/5 have none at all)"
        );
    }
}

#[test]
fn test_split_label_position_converts_physical_viewport_to_window_logical_coords() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let target = app.world_mut().spawn(PlayerIndex(0)).id();
    let camera = app.world_mut().spawn((
        Camera {
            is_active: true,
            viewport: Some(Viewport {
                physical_position: UVec2::new(0, 0),
                physical_size: UVec2::new(640, 720),
                ..default()
            }),
            ..default()
        },
        test_orbit_camera(target),
        SplitViewportSlot(0),
    )).id();

    app.world_mut().run_system_once(split_viewport_player_label_spawn_system).unwrap();
    app.world_mut().run_system_once(split_viewport_player_label_update_system).unwrap();

    let label = app.world().get::<LinkedPlayerLabel>(camera).unwrap().0;
    let node = app.world().get::<Node>(label).unwrap();
    // Top-right anchored inside the 640x720 (already-logical, scale_factor 1.0) cell.
    assert_eq!(node.top, Val::Px(8.0));
    match node.left {
        Val::Px(px) => assert!(
            (px - (640.0 - 48.0 - 8.0)).abs() < 0.01,
            "left must sit at the cell's right edge minus label width and margin, got {px}"
        ),
        other => panic!("expected Val::Px, got {other:?}"),
    }
}

#[test]
fn test_split_label_position_unaffected_by_hidpi_scale_factor_override() {
    let mut app = setup_test_app();
    app.update();
    // 2x scale factor override, mirroring
    // test_split_screen_viewport_unaffected_by_scale_factor_override: a physical viewport must
    // convert to the SAME logical (window-space, not viewport-space) position regardless of DPI.
    spawn_primary_window(&mut app, 2560, 1440, 2.0);

    let target = app.world_mut().spawn(PlayerIndex(1)).id();
    let camera = app.world_mut().spawn((
        Camera {
            is_active: true,
            viewport: Some(Viewport {
                physical_position: UVec2::new(1280, 0),
                physical_size: UVec2::new(1280, 1440),
                ..default()
            }),
            ..default()
        },
        test_orbit_camera(target),
        SplitViewportSlot(1),
    )).id();

    app.world_mut().run_system_once(split_viewport_player_label_spawn_system).unwrap();
    app.world_mut().run_system_once(split_viewport_player_label_update_system).unwrap();

    let label = app.world().get::<LinkedPlayerLabel>(camera).unwrap().0;
    let node = app.world().get::<Node>(label).unwrap();
    // Physical right edge 2560 / scale_factor 2.0 = logical 1280 — identical logical position to
    // a 1x, 1280-wide-cell window, proving the conversion is scale-factor independent.
    assert_eq!(node.top, Val::Px(8.0));
    match node.left {
        Val::Px(px) => assert!(
            (px - (1280.0 - 48.0 - 8.0)).abs() < 0.01,
            "logical position must be scale-factor-independent, got {px}"
        ),
        other => panic!("expected Val::Px, got {other:?}"),
    }
}

#[test]
fn test_split_label_visibility_mirrors_camera_is_active_across_merge_split_with_no_stale_frame() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let p0 = app.world_mut().spawn((Transform::default(), PlayerIndex(0))).id();
    let p1 = app.world_mut().spawn((Transform::default(), PlayerIndex(1))).id();
    let cam0 = app.world_mut().spawn((
        Camera {
            is_active: true, order: 0,
            viewport: Some(Viewport { physical_position: UVec2::ZERO, physical_size: UVec2::new(640, 720), ..default() }),
            ..default()
        },
        test_orbit_camera(p0),
        SplitViewportSlot(0),
    )).id();
    let cam1 = app.world_mut().spawn((
        Camera {
            is_active: true, order: 1,
            viewport: Some(Viewport { physical_position: UVec2::new(640, 0), physical_size: UVec2::new(640, 720), ..default() }),
            ..default()
        },
        test_orbit_camera(p1),
        SplitViewportSlot(1),
    )).id();

    app.world_mut().run_system_once(split_viewport_player_label_spawn_system).unwrap();
    app.world_mut().run_system_once(split_viewport_player_label_update_system).unwrap();

    let label0 = app.world().get::<LinkedPlayerLabel>(cam0).unwrap().0;
    let label1 = app.world().get::<LinkedPlayerLabel>(cam1).unwrap().0;
    assert_eq!(*app.world().get::<Visibility>(label0).unwrap(), Visibility::Visible);
    assert_eq!(*app.world().get::<Visibility>(label1).unwrap(), Visibility::Visible);

    // Simulate dynamic_split_screen_system's merge — it flips is_active the same frame the
    // viewport is (re)computed; per architecture review, this system must read the fresh
    // is_active immediately, with no stale Visible frame left over from before the merge.
    app.world_mut().get_mut::<Camera>(cam0).unwrap().is_active = false;
    app.world_mut().get_mut::<Camera>(cam1).unwrap().is_active = false;
    app.world_mut().run_system_once(split_viewport_player_label_update_system).unwrap();

    assert_eq!(*app.world().get::<Visibility>(label0).unwrap(), Visibility::Hidden, "must hide immediately on merge, no stale Visible frame");
    assert_eq!(*app.world().get::<Visibility>(label1).unwrap(), Visibility::Hidden);

    // And back to split — must reappear immediately too, no stale Hidden frame.
    app.world_mut().get_mut::<Camera>(cam0).unwrap().is_active = true;
    app.world_mut().get_mut::<Camera>(cam1).unwrap().is_active = true;
    app.world_mut().run_system_once(split_viewport_player_label_update_system).unwrap();

    assert_eq!(*app.world().get::<Visibility>(label0).unwrap(), Visibility::Visible, "must show immediately on re-split, no stale Hidden frame");
    assert_eq!(*app.world().get::<Visibility>(label1).unwrap(), Visibility::Visible);
}

#[test]
fn test_no_split_labels_spawn_for_party_mode_scene() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs(&mut app, Some(PartyZoomDef { zoom_margin: 4.0, allow_manual_zoom: false }));
    load_two_player_scene(&mut app);
    app.update();

    let split_slot_count = app.world_mut().query::<&SplitViewportSlot>().iter(app.world()).count();
    assert_eq!(split_slot_count, 0, "party mode must not spawn any SplitViewportSlot camera");

    let label_count = app.world_mut().query::<&SplitScreenPlayerLabel>().iter(app.world()).count();
    assert_eq!(
        label_count, 0,
        "no HUD corner labels should spawn when there is no SplitViewportSlot to attach one to"
    );
}

#[test]
fn test_no_split_labels_spawn_for_single_player_fallback_scene() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs(&mut app, None); // no party, no split -> falls back to a single OrbitCamera
    load_two_player_scene(&mut app);
    app.update();

    let label_count = app.world_mut().query::<&SplitScreenPlayerLabel>().iter(app.world()).count();
    assert_eq!(
        label_count, 0,
        "fallback single-camera scenes have no SplitViewportSlot and must spawn no corner label"
    );
}

// ── world_label_screen_pos_system: viewport-aware camera selection ─────────────
//
// world_label_screen_pos_system is private (registered directly into GamePlugin's
// Update schedule, not exported), so these drive it via `app.update()` rather than
// `run_system_once`, mirroring nameplate_tests.rs's pattern for the same reason.
//
// `setup_test_app()` uses `MinimalPlugins` — no render plugin ever runs
// bevy_render's `camera_system`, so `Camera.computed` (clip_from_view, target_info)
// is never populated for a bare `Camera::default()`. `ortho_camera_bundle` fills
// it in by hand with a real orthographic projection so `world_to_viewport`/
// `logical_viewport_rect` behave exactly as they would with a real render plugin,
// and the screen-space math stays simple and exact enough to assert on directly.

/// Builds a `(Transform, GlobalTransform, Camera)` with a working orthographic
/// projection. `half_extent` is the view-space half-width/height mapped to NDC
/// `[-1, 1]` — smaller values simulate a more "zoomed in" split-screen camera, so a
/// point can be deliberately placed inside one camera's frustum and outside another's.
fn ortho_camera_bundle(
    position: Vec3,
    look_at: Vec3,
    half_extent: f32,
    is_active: bool,
    order: isize,
    viewport: Option<Viewport>,
    window_physical_size: UVec2,
) -> (Transform, GlobalTransform, Camera) {
    let transform = Transform::from_translation(position).looking_at(look_at, Vec3::Y);
    let global = GlobalTransform::from(transform);
    let clip_from_view = Mat4::orthographic_rh(-half_extent, half_extent, -half_extent, half_extent, 0.1, 1000.0);
    let camera = Camera {
        is_active,
        order,
        viewport,
        computed: bevy::camera::ComputedCameraValues {
            clip_from_view,
            target_info: Some(bevy::camera::RenderTargetInfo {
                physical_size: window_physical_size,
                scale_factor: 1.0,
            }),
            ..default()
        },
        ..default()
    };
    (transform, global, camera)
}

fn fixed_world_label(world_pos: Vec3) -> WorldLabel {
    WorldLabel {
        world_pos,
        tracked_entity: None,
        offset: Vec3::ZERO,
        base_font_size: 16.0,
        depth_scale: None,
        screen_offset: Vec2::ZERO,
    }
}

#[test]
fn test_world_label_single_camera_regression_unaffected_by_multi_camera_fix() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let (t, g, camera) = ortho_camera_bundle(
        Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 10.0, true, 0, None, UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), camera, t, g));

    let label = app.world_mut().spawn((
        fixed_world_label(Vec3::ZERO),
        Transform::default(),
        Visibility::Hidden,
    )).id();

    app.update();

    let transform = app.world().get::<Transform>(label).unwrap();
    assert!(transform.translation.x.abs() < 0.5, "expected screen-centered x, got {}", transform.translation.x);
    assert!(transform.translation.y.abs() < 0.5, "expected screen-centered y, got {}", transform.translation.y);
    assert_eq!(*app.world().get::<Visibility>(label).unwrap(), Visibility::Visible);
}

#[test]
fn test_world_label_resolves_against_the_split_camera_whose_viewport_actually_shows_it() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    // Two split cameras, each framing its own player far apart in world space —
    // mirrors a real dynamic/grid split rig, not two coincidentally-identical cameras.
    let (t0, g0, cam0) = ortho_camera_bundle(
        Vec3::new(-10.0, 0.0, 10.0), Vec3::new(-10.0, 0.0, 0.0), 5.0, true, 0,
        Some(Viewport { physical_position: UVec2::ZERO, physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam0, t0, g0, SplitViewportSlot(0)));

    let (t1, g1, cam1) = ortho_camera_bundle(
        Vec3::new(10.0, 0.0, 10.0), Vec3::new(10.0, 0.0, 0.0), 5.0, true, 1,
        Some(Viewport { physical_position: UVec2::new(640, 0), physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam1, t1, g1, SplitViewportSlot(1)));

    let label_left = app.world_mut().spawn((
        fixed_world_label(Vec3::new(-10.0, 0.0, 0.0)), Transform::default(), Visibility::Hidden,
    )).id();
    let label_right = app.world_mut().spawn((
        fixed_world_label(Vec3::new(10.0, 0.0, 0.0)), Transform::default(), Visibility::Hidden,
    )).id();

    app.update();

    let tl = app.world().get::<Transform>(label_left).unwrap();
    assert!(
        (tl.translation.x - (-320.0)).abs() < 0.5,
        "a point at cam0's target must resolve centered in the LEFT half, got x={}", tl.translation.x
    );
    assert_eq!(*app.world().get::<Visibility>(label_left).unwrap(), Visibility::Visible);

    let tr = app.world().get::<Transform>(label_right).unwrap();
    assert!(
        (tr.translation.x - 320.0).abs() < 0.5,
        "a point at cam1's target must resolve centered in the RIGHT half, got x={}", tr.translation.x
    );
    assert_eq!(*app.world().get::<Visibility>(label_right).unwrap(), Visibility::Visible);
}

#[test]
fn test_world_label_hides_when_no_active_camera_viewport_shows_it() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let (t0, g0, cam0) = ortho_camera_bundle(
        Vec3::new(-10.0, 0.0, 10.0), Vec3::new(-10.0, 0.0, 0.0), 5.0, true, 0,
        Some(Viewport { physical_position: UVec2::ZERO, physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam0, t0, g0, SplitViewportSlot(0)));

    let (t1, g1, cam1) = ortho_camera_bundle(
        Vec3::new(10.0, 0.0, 10.0), Vec3::new(10.0, 0.0, 0.0), 5.0, true, 1,
        Some(Viewport { physical_position: UVec2::new(640, 0), physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam1, t1, g1, SplitViewportSlot(1)));

    // Far outside either camera's narrow (half_extent=5.0) frustum — off-viewport for both.
    let label = app.world_mut().spawn((
        fixed_world_label(Vec3::new(1000.0, 0.0, 0.0)), Transform::default(), Visibility::Visible,
    )).id();

    app.update();

    assert_eq!(
        *app.world().get::<Visibility>(label).unwrap(), Visibility::Hidden,
        "a point off-frustum/off-viewport for every active camera must hide the label, \
         matching the pre-existing off-frustum contract"
    );
}

#[test]
fn test_world_label_repositions_immediately_across_merge_split_transition_with_no_stale_frame() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let (t_split, g_split, cam_split) = ortho_camera_bundle(
        Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 5.0, true, 0,
        Some(Viewport { physical_position: UVec2::ZERO, physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    let cam_split_entity = app.world_mut()
        .spawn((Camera3d::default(), cam_split, t_split, g_split, SplitViewportSlot(0)))
        .id();

    // Same transform/projection as the split camera, but full-window viewport and inactive —
    // mirrors dynamic_split_screen_system's PartyOrbitCamera, which never has a SplitViewportSlot.
    let (t_party, g_party, cam_party) = ortho_camera_bundle(
        Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 5.0, false, 1, None, UVec2::new(1280, 720),
    );
    let cam_party_entity = app.world_mut()
        .spawn((Camera3d::default(), cam_party, t_party, g_party))
        .id();

    let label = app.world_mut().spawn((
        fixed_world_label(Vec3::ZERO), Transform::default(), Visibility::Hidden,
    )).id();

    app.update();
    let split_x = app.world().get::<Transform>(label).unwrap().translation.x;
    assert!(
        (split_x - (-320.0)).abs() < 0.5,
        "while split is active, must resolve via the left-half split camera, got x={split_x}"
    );

    // Simulate dynamic_split_screen_system's merge — it flips is_active on both cameras
    // atomically within one frame; the label must not lag a frame behind.
    app.world_mut().get_mut::<Camera>(cam_split_entity).unwrap().is_active = false;
    app.world_mut().get_mut::<Camera>(cam_party_entity).unwrap().is_active = true;
    app.update();

    let merged_x = app.world().get::<Transform>(label).unwrap().translation.x;
    assert!(
        (merged_x - 0.0).abs() < 0.5,
        "immediately after merging, must resolve via the full-window party camera with no \
         stale split-half position, got x={merged_x}"
    );
}

// ── Anchor-style depth scaling (nameplate/bar anchors with no TextFont of their own) ────
//
// `world_label_screen_pos_system` scales an anchor's whole child subtree (Text2d/Mesh2d/Sprite)
// via `Transform.scale` when the WorldLabel entity itself carries no `TextFont` — see
// planning/backlog.md "Nameplate/health-bar spacing looks wrong at the zoom extremes". Text2d-
// bearing WorldLabel entities (Ascii bars, stat_label) are covered separately by the pre-existing
// font-size path and are unaffected by this branch.

fn anchor_world_label(world_pos: Vec3, depth_scale: Option<(f32, f32)>) -> WorldLabel {
    WorldLabel {
        world_pos,
        tracked_entity: None,
        offset: Vec3::ZERO,
        base_font_size: 1.0,
        depth_scale,
        screen_offset: Vec2::ZERO,
    }
}

fn anchor_world_label_with_screen_offset(
    world_pos: Vec3, depth_scale: Option<(f32, f32)>, screen_offset: Vec2,
) -> WorldLabel {
    WorldLabel {
        world_pos,
        tracked_entity: None,
        offset: Vec3::ZERO,
        base_font_size: 1.0,
        depth_scale,
        screen_offset,
    }
}

#[test]
fn test_world_label_anchor_scale_shrinks_with_distance_past_reference() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let (t, g, camera) = ortho_camera_bundle(
        Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 10.0, true, 0, None, UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), camera, t, g));

    // Camera is 10.0 world units from the label; reference_distance 5.0, no floor ->
    // scale = (5.0 / 10.0).min(1.0).max(0.0) = 0.5.
    let label = app.world_mut().spawn((
        anchor_world_label(Vec3::ZERO, Some((5.0, 0.0))), Transform::default(), Visibility::Hidden,
    )).id();

    app.update();

    let scale = app.world().get::<Transform>(label).unwrap().scale;
    assert!(
        (scale.x - 0.5).abs() < 0.01 && (scale.y - 0.5).abs() < 0.01,
        "an anchor beyond its reference_distance must shrink proportionally, got {scale:?}"
    );
}

#[test]
fn test_world_label_anchor_scale_clamped_to_one_when_closer_than_reference() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let (t, g, camera) = ortho_camera_bundle(
        Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 10.0, true, 0, None, UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), camera, t, g));

    // Camera is 10.0 world units away; reference_distance 50.0 -> raw ratio 5.0, clamped to 1.0.
    // This is the exact configuration a scene omitting `label_depth_scale` falls back to
    // (default reference_distance 50.0) once a per-label override forces scaling on.
    let label = app.world_mut().spawn((
        anchor_world_label(Vec3::ZERO, Some((50.0, 0.0))), Transform::default(), Visibility::Hidden,
    )).id();

    app.update();

    let scale = app.world().get::<Transform>(label).unwrap().scale;
    assert!(
        (scale.x - 1.0).abs() < 0.01,
        "an anchor closer than its reference_distance must clamp to scale 1.0 (never grow), got {scale:?}"
    );
}

#[test]
fn test_world_label_anchor_scale_respects_min_floor() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let (t, g, camera) = ortho_camera_bundle(
        Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 10.0, true, 0, None, UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), camera, t, g));

    // Camera is 10.0 world units away; reference_distance 2.0 -> raw ratio 0.2, floored to 0.4.
    let label = app.world_mut().spawn((
        anchor_world_label(Vec3::ZERO, Some((2.0, 0.4))), Transform::default(), Visibility::Hidden,
    )).id();

    app.update();

    let scale = app.world().get::<Transform>(label).unwrap().scale;
    assert!(
        (scale.x - 0.4).abs() < 0.01,
        "an anchor's scale must never shrink past its configured min_scale floor, got {scale:?}"
    );
}

#[test]
fn test_world_label_anchor_scale_stays_default_when_depth_scale_none() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let (t, g, camera) = ortho_camera_bundle(
        Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 10.0, true, 0, None, UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), camera, t, g));

    // No `label_depth_scale` in scope -> the anchor branch must be a true no-op, not an active
    // reset to 1.0 every frame (a real anchor never has a non-identity scale today, but a future
    // feature that scales an anchor for its own reasons must not have this system silently fight
    // it back to 1.0 every frame). Spawn with a deliberately non-identity scale to prove that.
    let label = app.world_mut().spawn((
        anchor_world_label(Vec3::ZERO, None),
        Transform::from_scale(Vec3::new(0.5, 0.5, 1.0)),
        Visibility::Hidden,
    )).id();

    app.update();

    let scale = app.world().get::<Transform>(label).unwrap().scale;
    assert!(
        (scale.x - 0.5).abs() < 0.01 && (scale.y - 0.5).abs() < 0.01,
        "an anchor with no depth_scale configured must be left untouched, not reset to 1.0, got {scale:?}"
    );
}

/// `screen_offset` (nameplate zoom-spacing fix round 2: stacking co-located widgets by pixels
/// instead of drifting world offsets) must reach the final screen translation, unscaled when
/// `depth_scale` is `None` (factor 1.0) — this is the flow-through this mechanism needs and had
/// zero coverage for before this test (architect + debug-detective both flagged it).
#[test]
fn test_world_label_screen_offset_applies_unscaled_when_depth_scale_none() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let (t, g, camera) = ortho_camera_bundle(
        Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 10.0, true, 0, None, UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), camera, t, g));

    // A label at world origin projects to screen-centre (0, 0) with this camera (see the
    // single-camera regression test above) — so the final translation is exactly screen_offset.
    let label = app.world_mut().spawn((
        anchor_world_label_with_screen_offset(Vec3::ZERO, None, Vec2::new(0.0, 50.0)),
        Transform::default(), Visibility::Hidden,
    )).id();

    app.update();

    let translation = app.world().get::<Transform>(label).unwrap().translation;
    assert!(
        (translation.x - 0.0).abs() < 0.5 && (translation.y - 50.0).abs() < 0.5,
        "screen_offset must reach the final translation unscaled when depth_scale is None, got {translation:?}"
    );
}

/// `screen_offset` must be multiplied by the same depth-scale factor as the widget itself, so a
/// stacked pixel gap shrinks together with the widgets around it instead of staying a fixed size.
#[test]
fn test_world_label_screen_offset_scales_with_depth_scale_factor() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let (t, g, camera) = ortho_camera_bundle(
        Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 10.0, true, 0, None, UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), camera, t, g));

    // Camera is 10.0 world units from the label; reference_distance 5.0 -> factor 0.5.
    // Expected: screen_offset (0, 50) * 0.5 = (0, 25).
    let label = app.world_mut().spawn((
        anchor_world_label_with_screen_offset(Vec3::ZERO, Some((5.0, 0.0)), Vec2::new(0.0, 50.0)),
        Transform::default(), Visibility::Hidden,
    )).id();

    app.update();

    let translation = app.world().get::<Transform>(label).unwrap().translation;
    assert!(
        (translation.x - 0.0).abs() < 0.5 && (translation.y - 25.0).abs() < 0.5,
        "screen_offset must scale down by the same depth_scale factor as the widget it stacks against, got {translation:?}"
    );
}

// ── WorldLabelRank: multi-viewport duplication (2026-07-10 playtest amendment) ─────
//
// Frank's playtest of the fix above found that a portal simultaneously visible in 2 active
// split viewports (e.g. player 1 approaches the portal where player 2 is already standing)
// only showed its label in one viewport — correct for room5's dynamic merge (only one camera
// is ever active then) but wrong for a fixed split screen, where both viewports are always
// simultaneously rendered. `WorldLabelRank` lets `scene_loader.rs` spawn one sibling label per
// possible active-camera rank so each simultaneously-visible viewport gets its own copy.

#[test]
fn test_world_label_rank_siblings_both_resolve_when_point_visible_in_both_active_viewports() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    // Two split cameras with overlapping frustums (half_extent=10 is wide enough that each
    // camera also sees the other's target) — mirrors two players standing near the same portal.
    let (t0, g0, cam0) = ortho_camera_bundle(
        Vec3::new(-3.0, 0.0, 10.0), Vec3::new(-3.0, 0.0, 0.0), 10.0, true, 0,
        Some(Viewport { physical_position: UVec2::ZERO, physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam0, t0, g0, SplitViewportSlot(0)));

    let (t1, g1, cam1) = ortho_camera_bundle(
        Vec3::new(3.0, 0.0, 10.0), Vec3::new(3.0, 0.0, 0.0), 10.0, true, 1,
        Some(Viewport { physical_position: UVec2::new(640, 0), physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam1, t1, g1, SplitViewportSlot(1)));

    // Rank 0 (implicit — no WorldLabelRank) mirrors scene_loader.rs's primary sibling.
    let label_rank0 = app.world_mut().spawn((
        fixed_world_label(Vec3::ZERO), Transform::default(), Visibility::Hidden,
    )).id();
    // Rank 1 mirrors scene_loader.rs's first extra sibling.
    let label_rank1 = app.world_mut().spawn((
        fixed_world_label(Vec3::ZERO), WorldLabelRank(1), Transform::default(), Visibility::Hidden,
    )).id();

    app.update();

    // Deterministic order picks cam0 (slot 0) for rank 0, cam1 (slot 1) for rank 1.
    let t_rank0 = app.world().get::<Transform>(label_rank0).unwrap();
    assert!(
        (t_rank0.translation.x - (-224.0)).abs() < 0.5,
        "rank 0 must resolve via cam0 (left half), got x={}", t_rank0.translation.x
    );
    assert_eq!(*app.world().get::<Visibility>(label_rank0).unwrap(), Visibility::Visible);

    let t_rank1 = app.world().get::<Transform>(label_rank1).unwrap();
    assert!(
        (t_rank1.translation.x - 224.0).abs() < 0.5,
        "rank 1 must resolve via cam1 (right half) SIMULTANEOUSLY with rank 0, got x={}", t_rank1.translation.x
    );
    assert_eq!(
        *app.world().get::<Visibility>(label_rank1).unwrap(), Visibility::Visible,
        "both viewports can see the point at once — rank 1 must NOT hide just because rank 0 is shown"
    );
}

#[test]
fn test_world_label_rank_hides_when_fewer_active_cameras_than_ranks() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    // Only ONE active camera this time — rank 1 has no 2nd qualifying camera to bind to.
    let (t0, g0, camera) = ortho_camera_bundle(
        Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 10.0, true, 0, None, UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), camera, t0, g0));

    let label_rank0 = app.world_mut().spawn((
        fixed_world_label(Vec3::ZERO), Transform::default(), Visibility::Hidden,
    )).id();
    let label_rank1 = app.world_mut().spawn((
        fixed_world_label(Vec3::ZERO), WorldLabelRank(1), Transform::default(), Visibility::Visible,
    )).id();

    app.update();

    assert_eq!(
        *app.world().get::<Visibility>(label_rank0).unwrap(), Visibility::Visible,
        "rank 0 must still resolve via the single active camera"
    );
    assert_eq!(
        *app.world().get::<Visibility>(label_rank1).unwrap(), Visibility::Hidden,
        "rank 1 must hide independently — only one active camera exists, so there is no 2nd \
         qualifying camera for it to bind to"
    );
}

/// Regression guard for the exact mistake made during this fix's first pass: `local_coop_demo`'s
/// portal room-name labels are authored via a scene entity's `label:` field (`EntityLabelDef`,
/// `tracked_entity`), spawned by scene_loader.rs's separate `pending_labels` loop — NOT via
/// scene-level `world_labels:` (fixed world position, no tracked entity). An earlier revision of
/// the `WorldLabelRank` duplication only touched the `world_labels:` loop, so Frank's playtest
/// still reproduced the bug exactly — the fix had never run for this project. This drives a real
/// scene load through `spawn_scene_v2` (not a hand-built `WorldLabel`) so it would have caught
/// that mistake.
#[test]
fn test_entity_label_ranks_spawn_for_tracked_entity_labels_not_just_world_labels() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("test_portal".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "char_a".to_string(),
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
            (id: "portal", prefab: "test_portal", transform: (translation: (0.0, 0.0, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0)), label: Some((text: "Room 4", offset: (0.0, 4.0, 0.0)))),
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

    let ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&WorldLabel, Option<&WorldLabelRank>)>();
        q.iter(app.world())
            .filter(|(wl, _)| wl.tracked_entity.is_some())
            .map(|(_, rank)| rank.map(|r| r.0))
            .collect()
    };
    assert_eq!(
        ranks.len(), 4,
        "an entity `label:` field must spawn MAX_SPLIT_PLAYERS (4) ranked siblings, not just 1 \
         — this is the actual mechanism local_coop_demo's portal room-name labels use, distinct \
         from scene-level `world_labels:`"
    );
    let mut sorted_ranks: Vec<u8> = ranks.iter().map(|r| r.unwrap_or(0)).collect();
    sorted_ranks.sort();
    assert_eq!(sorted_ranks, vec![0, 1, 2, 3], "expected exactly one of each rank 0-3");
}

// ── Split-screen viewport-aware click-to-select (Phase 2, split_screen_camera_followups.md) ───

fn set_cursor_position(app: &mut App, x: f64, y: f64) {
    let mut win_q = app.world_mut().query::<&mut Window>();
    let mut window = win_q.single_mut(app.world_mut()).unwrap();
    window.set_physical_cursor_position(Some((x, y).into()));
}


#[test]
fn test_click_select_resolves_against_the_viewport_the_cursor_is_actually_over() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    // Deliberately spawn the RIGHT camera (slot 1) first and the LEFT camera (slot 0) second, so
    // a naive "first active camera in query iteration order" pick (the old bug's
    // `.find(|c| c.is_active)`) would choose the wrong one regardless of where the cursor
    // actually is — proving the fix picks by viewport-contains-cursor, not iteration order.
    let right_player = app.world_mut().spawn(PlayerTarget::default()).id();
    let (t1, g1, cam1) = ortho_camera_bundle(
        Vec3::new(3.0, 0.0, 10.0), Vec3::new(3.0, 0.0, 0.0), 10.0, true, 1,
        Some(Viewport { physical_position: UVec2::new(640, 0), physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam1, t1, g1, SplitViewportSlot(1), test_orbit_camera(right_player)));

    let left_player = app.world_mut().spawn(PlayerTarget::default()).id();
    let (t0, g0, cam0) = ortho_camera_bundle(
        Vec3::new(-3.0, 0.0, 10.0), Vec3::new(-3.0, 0.0, 0.0), 10.0, true, 0,
        Some(Viewport { physical_position: UVec2::ZERO, physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam0, t0, g0, SplitViewportSlot(0), test_orbit_camera(left_player)));

    // One selectable sits exactly at the left camera's look-at target (projects to the left
    // viewport's centre, screen (320, 360)); another at the right camera's look-at target
    // (projects to the right viewport's centre, screen (960, 360)).
    app.world_mut().spawn((
        SpawnId("left_entity".to_string()),
        GlobalTransform::from_translation(Vec3::new(-3.0, 0.0, 0.0)),
        ClickSelectable,
    ));
    app.world_mut().spawn((
        SpawnId("right_entity".to_string()),
        GlobalTransform::from_translation(Vec3::new(3.0, 0.0, 0.0)),
        ClickSelectable,
    ));

    // Click at the LEFT viewport's centre.
    set_cursor_position(&mut app, 320.0, 360.0);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();

    assert_eq!(
        app.world().resource::<CurrentTarget>().0.as_deref(), Some("left_entity"),
        "a click in the left viewport must select the entity actually near the left camera's \
         view, not silently evaluate against the right camera (which was spawned first)"
    );
}

#[test]
fn test_click_select_resolves_against_the_other_viewport_when_cursor_moves_there() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let left_player = app.world_mut().spawn(PlayerTarget::default()).id();
    let (t0, g0, cam0) = ortho_camera_bundle(
        Vec3::new(-3.0, 0.0, 10.0), Vec3::new(-3.0, 0.0, 0.0), 10.0, true, 0,
        Some(Viewport { physical_position: UVec2::ZERO, physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam0, t0, g0, SplitViewportSlot(0), test_orbit_camera(left_player)));

    let right_player = app.world_mut().spawn(PlayerTarget::default()).id();
    let (t1, g1, cam1) = ortho_camera_bundle(
        Vec3::new(3.0, 0.0, 10.0), Vec3::new(3.0, 0.0, 0.0), 10.0, true, 1,
        Some(Viewport { physical_position: UVec2::new(640, 0), physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam1, t1, g1, SplitViewportSlot(1), test_orbit_camera(right_player)));

    app.world_mut().spawn((
        SpawnId("left_entity".to_string()),
        GlobalTransform::from_translation(Vec3::new(-3.0, 0.0, 0.0)),
        ClickSelectable,
    ));
    app.world_mut().spawn((
        SpawnId("right_entity".to_string()),
        GlobalTransform::from_translation(Vec3::new(3.0, 0.0, 0.0)),
        ClickSelectable,
    ));

    // Click at the RIGHT viewport's centre this time.
    set_cursor_position(&mut app, 960.0, 360.0);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();

    assert_eq!(
        app.world().resource::<CurrentTarget>().0.as_deref(), Some("right_entity"),
        "a click in the right viewport must select the entity near the right camera's view"
    );
}

#[test]
fn test_click_select_single_camera_regression_unaffected_by_viewport_fix() {
    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let player = app.world_mut().spawn(PlayerTarget::default()).id();
    let (t, g, camera) = ortho_camera_bundle(
        Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 10.0, true, 0, None, UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), camera, t, g, test_orbit_camera(player)));

    app.world_mut().spawn((
        SpawnId("only_entity".to_string()),
        GlobalTransform::from_translation(Vec3::ZERO),
        ClickSelectable,
    ));

    set_cursor_position(&mut app, 640.0, 360.0);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();

    assert_eq!(
        app.world().resource::<CurrentTarget>().0.as_deref(), Some("only_entity"),
        "an ordinary single-camera (non-split) scene must still select correctly after the \
         viewport-aware fix — regression guard"
    );
}

// ── Per-player targeting (Phase 1, per_player_split_screen_targeting.md) ───────────

fn test_targetable_at(id: &str, pos: Vec3) -> impl Bundle {
    (
        SpawnId(id.to_string()),
        Transform::from_translation(pos),
        GlobalTransform::from_translation(pos),
        ironhold_core::capabilities::targeting::Targetable,
    )
}

#[test]
fn test_click_select_only_changes_the_clicking_players_target() {

    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);

    let left_player = app.world_mut().spawn((PlayerTarget::default(), PlayerIndex(0))).id();
    let (t0, g0, cam0) = ortho_camera_bundle(
        Vec3::new(-3.0, 0.0, 10.0), Vec3::new(-3.0, 0.0, 0.0), 10.0, true, 0,
        Some(Viewport { physical_position: UVec2::ZERO, physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam0, t0, g0, SplitViewportSlot(0), test_orbit_camera(left_player)));

    let right_player = app.world_mut().spawn((PlayerTarget::default(), PlayerIndex(1))).id();
    let (t1, g1, cam1) = ortho_camera_bundle(
        Vec3::new(3.0, 0.0, 10.0), Vec3::new(3.0, 0.0, 0.0), 10.0, true, 1,
        Some(Viewport { physical_position: UVec2::new(640, 0), physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam1, t1, g1, SplitViewportSlot(1), test_orbit_camera(right_player)));

    app.world_mut().spawn((
        SpawnId("left_entity".to_string()),
        GlobalTransform::from_translation(Vec3::new(-3.0, 0.0, 0.0)),
        ClickSelectable,
    ));

    // Click the left viewport — only the left (primary) player should get a target.
    set_cursor_position(&mut app, 320.0, 360.0);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();

    assert_eq!(
        app.world().get::<PlayerTarget>(left_player).unwrap().0.as_deref(), Some("left_entity"),
        "the clicking player's own PlayerTarget must be set"
    );
    assert_eq!(
        app.world().get::<PlayerTarget>(right_player).unwrap().0, None,
        "the non-clicking player's PlayerTarget must be completely unaffected"
    );
}

#[test]
fn test_tab_targeting_each_player_cycles_independently() {

    let mut app = setup_test_app();
    app.update();

    let mut p1_inputs = test_input_map();
    p1_inputs.target_next = "Tab".to_string();
    let player1 = app.world_mut().spawn((
        CharacterController { inputs: p1_inputs, ..test_character_controller() },
        PlayerTarget::default(),
        PlayerIndex(0),
        Transform::default(),
        GlobalTransform::default(),
    )).id();

    let mut p2_inputs = test_input_map();
    p2_inputs.target_next = "KeyT".to_string();
    let player2 = app.world_mut().spawn((
        CharacterController { inputs: p2_inputs, ..test_character_controller() },
        PlayerTarget::default(),
        PlayerIndex(1),
        Transform::default(),
        GlobalTransform::default(),
    )).id();

    app.world_mut().spawn(test_targetable_at("enemy_a", Vec3::new(2.0, 0.0, 0.0)));
    app.world_mut().spawn(test_targetable_at("enemy_b", Vec3::new(-2.0, 0.0, 0.0)));

    // Player 2 presses their own key first — only player 2's target should change.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyT);
    app.update();

    assert_eq!(app.world().get::<PlayerTarget>(player1).unwrap().0, None, "player 1 pressed nothing, must stay untargeted");
    assert!(app.world().get::<PlayerTarget>(player2).unwrap().0.is_some(), "player 2's own key press must select a target for player 2");

    // `release()` alone doesn't clear the `just_pressed` bookkeeping (only `pressed`) — without
    // this, KeyT's stale just_pressed bit would still be set below when Tab is pressed, making
    // player 2 spuriously react a second time in the same frame as player 1's Tab press.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::KeyT);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear_just_pressed(KeyCode::KeyT);
    app.update();

    // Now player 1 presses Tab — only player 1's target should change; player 2's is untouched.
    let player2_target_before = app.world().get::<PlayerTarget>(player2).unwrap().0.clone();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Tab);
    app.update();

    assert!(app.world().get::<PlayerTarget>(player1).unwrap().0.is_some(), "player 1's own Tab press must select a target for player 1");
    assert_eq!(
        app.world().get::<PlayerTarget>(player2).unwrap().0, player2_target_before,
        "player 1's Tab press must not disturb player 2's already-selected target"
    );
}

/// Regression: `interactable_system` previously used `player_query.single()`, which fails and
/// early-returns for *every* player the moment a scene has 2+ `CharacterController`s — interact
/// silently did nothing for anyone, keyboard or gamepad, in any local-coop/split-screen scene.
/// Found during `gamepad_controller_input.md`'s plan review (system-architect, 2026-07-19), fixed
/// as a per-player loop mirroring `tab_targeting_system`'s shape.
#[test]
fn test_interact_fires_for_pressing_player_with_two_players_present() {
    use ironhold_core::capabilities::interactable::Interactable;
    use ironhold_core::runtime::scene_manager::SpawnId;

    let mut app = setup_test_app();
    app.update();

    let mut p1_inputs = test_input_map();
    p1_inputs.interact = "KeyF".to_string();
    app.world_mut().spawn((
        CharacterController { inputs: p1_inputs, ..test_character_controller() },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    let mut p2_inputs = test_input_map();
    p2_inputs.interact = "KeyH".to_string();
    app.world_mut().spawn((
        CharacterController { inputs: p2_inputs, ..test_character_controller() },
        Transform::from_xyz(100.0, 0.0, 0.0),
    ));

    app.world_mut().spawn((
        Transform::from_xyz(1.0, 0.0, 0.0),
        SpawnId("chest_01".to_string()),
        Interactable { radius: 2.0, hint_text: None },
    ));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyF);
    app.update();

    let interacted = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(name) if name == "entity.interacted:chest_01"));
    assert!(
        interacted,
        "player 1's interact key must still fire entity.interacted with a second player present \
         (previously player_query.single() failed and no one could interact at all in local-coop)"
    );

    // Companion assertion (system-architect finding): confirm the loop actually evaluates player
    // 2 independently, not just "player 1 still works despite a second CharacterController
    // existing" — player 2 is far from the prop, so their own key press should produce a miss,
    // not silence (silence would mean player 2's iteration never ran at all).
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::KeyF);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear_just_pressed(KeyCode::KeyF);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyH);
    app.update();

    let p2_missed = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(name) if name == "player.attack_missed"));
    assert!(
        p2_missed,
        "player 2's own interact key must be evaluated independently and produce a miss \
         (nothing in range) — proves the loop services a non-first player, not just player 1"
    );
}

/// `gamepad_controller_input.md`: a custom `gamepad_interact` button fires `entity.interacted:{id}`
/// exactly as the keyboard `interact` key does.
#[test]
fn test_gamepad_interact_button_fires_entity_interacted() {
    use ironhold_core::capabilities::interactable::Interactable;
    use ironhold_core::runtime::scene_manager::SpawnId;

    let mut app = setup_test_app();
    app.update();
    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    let mut inputs = test_input_map();
    inputs.gamepad_index = Some(0);
    inputs.gamepad_interact = "West".to_string();
    app.world_mut().spawn((
        CharacterController { inputs, ..test_character_controller() },
        Transform::from_xyz(0.0, 0.0, 0.0),
        BoundGamepad(Some(gamepad)),
    ));
    app.world_mut().spawn((
        Transform::from_xyz(1.0, 0.0, 0.0),
        SpawnId("chest_01".to_string()),
        Interactable { radius: 2.0, hint_text: None },
    ));

    press_gamepad_button(&mut app, gamepad, GamepadButton::West);
    app.update();

    let interacted = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(name) if name == "entity.interacted:chest_01"));
    assert!(interacted, "gamepad_interact button press must fire entity.interacted, same as the keyboard interact key");
}

/// `gamepad_controller_input.md`: gamepad-interact works in local co-op, not just single-player —
/// the plan originally scoped this to single-player only (blocked by `interactable_system`'s
/// `player_query.single()` bug), but that bug is now fixed (see the regression test above), so
/// gamepad-interact folds into the same per-player loop for free. A gamepad-routed player and a
/// keyboard-routed player must each interact independently.
#[test]
fn test_gamepad_interact_works_independently_in_two_player_local_coop() {
    use ironhold_core::capabilities::interactable::Interactable;
    use ironhold_core::runtime::scene_manager::SpawnId;

    let mut app = setup_test_app();
    app.update();
    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    // Player 1 — gamepad-routed, near the interactable.
    let mut p1_inputs = test_input_map();
    p1_inputs.gamepad_index = Some(0);
    p1_inputs.gamepad_interact = "West".to_string();
    app.world_mut().spawn((
        CharacterController { inputs: p1_inputs, ..test_character_controller() },
        Transform::from_xyz(0.0, 0.0, 0.0),
        BoundGamepad(Some(gamepad)),
    ));

    // Player 2 — keyboard-routed, far away; presses nothing this frame. Every player entity
    // always carries `BoundGamepad` in production (inserted unconditionally by
    // `spawn_player_entity_core`) — `None` here since player 2 has no gamepad_index authored.
    let mut p2_inputs = test_input_map();
    p2_inputs.interact = "KeyH".to_string();
    app.world_mut().spawn((
        CharacterController { inputs: p2_inputs, ..test_character_controller() },
        Transform::from_xyz(100.0, 0.0, 0.0),
        BoundGamepad::default(),
    ));

    app.world_mut().spawn((
        Transform::from_xyz(1.0, 0.0, 0.0),
        SpawnId("chest_01".to_string()),
        Interactable { radius: 2.0, hint_text: None },
    ));

    press_gamepad_button(&mut app, gamepad, GamepadButton::West);
    app.update();

    let interacted = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(name) if name == "entity.interacted:chest_01"));
    assert!(
        interacted,
        "the gamepad-routed player's interact button must fire entity.interacted with a \
         keyboard-routed co-op partner present — proves gamepad-interact works in local co-op, \
         not just single-player"
    );
}

/// `gamepad_controller_input.md`: a custom `gamepad_target_next` button advances Tab-targeting
/// for its own player only, in a 2-player local-coop scene, matching the keyboard binding's
/// existing per-player behavior (`test_tab_targeting_each_player_cycles_independently`).
#[test]
fn test_gamepad_target_next_advances_targeting_independently_in_two_player_scene() {
    let mut app = setup_test_app();
    app.update();
    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    let mut p1_inputs = test_input_map();
    p1_inputs.gamepad_index = Some(0);
    p1_inputs.gamepad_target_next = "North".to_string();
    let player1 = app.world_mut().spawn((
        CharacterController { inputs: p1_inputs, ..test_character_controller() },
        PlayerTarget::default(),
        PlayerIndex(0),
        Transform::default(),
        GlobalTransform::default(),
        BoundGamepad(Some(gamepad)),
    )).id();

    let mut p2_inputs = test_input_map();
    p2_inputs.target_next = "KeyT".to_string();
    let player2 = app.world_mut().spawn((
        CharacterController { inputs: p2_inputs, ..test_character_controller() },
        PlayerTarget::default(),
        PlayerIndex(1),
        Transform::default(),
        GlobalTransform::default(),
        BoundGamepad::default(),
    )).id();

    app.world_mut().spawn(test_targetable_at("enemy_a", Vec3::new(2.0, 0.0, 0.0)));

    press_gamepad_button(&mut app, gamepad, GamepadButton::North);
    app.update();

    assert!(
        app.world().get::<PlayerTarget>(player1).unwrap().0.is_some(),
        "player 1's gamepad_target_next button press must select a target for player 1"
    );
    assert_eq!(
        app.world().get::<PlayerTarget>(player2).unwrap().0, None,
        "player 1's gamepad button press must not affect player 2's target"
    );
}

/// `gamepad_controller_input.md`: right-stick-Y camera pitch moves in the same direction as the
/// keyboard `look_up` key — pushing the stick up (positive `RightStickY`, matching this
/// codebase's existing `LeftStickY`-drives-forward-movement convention) increases pitch toward
/// `max_pitch`, exactly like `look_up`. Direction-asserting, not just clamp-asserting, per
/// `per_player_camera_look_controls.md`'s established test pattern.
#[test]
fn test_gamepad_right_stick_y_increases_pitch_same_direction_as_look_up() {
    let mut app = setup_test_app();
    app.update();
    let gamepad = connect_test_gamepad(&mut app);
    app.update();

    let player = app.world_mut().spawn((
        test_character_controller(),
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        // `camera_orbit_system` resolves gamepad input via `BoundGamepad` through `CameraTargets`
        // (`OrbitState` carries no gamepad_index — a spawn-frozen positional copy would be wrong;
        // see `gamepad_player_binding_hardening.md`).
        BoundGamepad(Some(gamepad)),
    )).id();

    let camera = app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState { pitch: 0.5, ..test_orbit_state() }),
        OrbitCameraMode,
        CameraTargets(vec![player]),
    )).id();

    let pitch_before = get_orbit(&app, camera).pitch;

    set_gamepad_axis(&mut app, gamepad, GamepadAxis::RightStickY, 1.0);
    // Large deterministic delta, same rationale as the keyboard look_up test above — a real
    // wall-clock tick between two rapid successive app.update() calls could be near-zero.
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
    app.update();

    let pitch_after = get_orbit(&app, camera).pitch;
    assert!(
        pitch_after > pitch_before,
        "pushing right-stick-Y up (positive value) must increase pitch, same direction as the \
         keyboard look_up key — got {} -> {}", pitch_before, pitch_after
    );
}

#[test]
fn test_only_primary_player_target_mirrors_into_current_target_and_global_events() {

    let mut app = setup_test_app();
    app.update();

    let mut p2_inputs = test_input_map();
    p2_inputs.target_next = "KeyT".to_string();
    app.world_mut().spawn((
        CharacterController { inputs: p2_inputs, ..test_character_controller() },
        PlayerTarget::default(),
        PlayerIndex(1),
        Transform::default(),
        GlobalTransform::default(),
    ));
    // A primary (index 0) player must also exist for `is_multiplayer` to reflect 2 players —
    // absent here on purpose to isolate "does a non-primary change ever leak into CurrentTarget",
    // matching the primitive-player path's "no PlayerIndex at all" shape too.

    app.world_mut().spawn(test_targetable_at("enemy_a", Vec3::new(2.0, 0.0, 0.0)));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyT);
    app.update();

    assert_eq!(
        app.world().resource::<CurrentTarget>().0, None,
        "a non-primary player's target selection must never mirror into the global CurrentTarget resource"
    );
    let changed_fired = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(name) if name.starts_with("target.changed")));
    assert!(!changed_fired, "a non-primary player's target selection must not emit global target.changed events");
}

#[test]
fn test_target_auto_clear_is_per_player() {

    let mut app = setup_test_app();
    app.update();

    let player1 = app.world_mut().spawn((
        test_character_controller(), PlayerTarget(Some("hidden_enemy".to_string())), PlayerIndex(0),
    )).id();
    let player2 = app.world_mut().spawn((
        test_character_controller(), PlayerTarget(Some("visible_enemy".to_string())), PlayerIndex(1),
    )).id();

    let hidden = app.world_mut().spawn((SpawnId("hidden_enemy".to_string()), Visibility::Hidden)).id();
    let visible = app.world_mut().spawn((SpawnId("visible_enemy".to_string()), Visibility::Visible)).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("hidden_enemy".to_string(), hidden);
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("visible_enemy".to_string(), visible);

    app.update();

    assert_eq!(app.world().get::<PlayerTarget>(player1).unwrap().0, None, "player 1's target must auto-clear once hidden");
    assert_eq!(
        app.world().get::<PlayerTarget>(player2).unwrap().0.as_deref(), Some("visible_enemy"),
        "player 2's target must be untouched — it is still visible"
    );
}

#[test]
fn test_legacy_target_vars_blank_when_multiplayer() {

    let mut app = setup_test_app();
    app.update();

    let mut p1_inputs = test_input_map();
    p1_inputs.target_next = "Tab".to_string();
    app.world_mut().spawn((
        CharacterController { inputs: p1_inputs, ..test_character_controller() },
        PlayerTarget::default(), PlayerIndex(0),
        Transform::default(), GlobalTransform::default(),
    ));
    // Second player present so this scene counts as multiplayer, even though only player 1 acts.
    app.world_mut().spawn((
        CharacterController { ..test_character_controller() },
        PlayerTarget::default(), PlayerIndex(1),
        Transform::default(), GlobalTransform::default(),
    ));

    app.world_mut().spawn(test_targetable_at("enemy_a", Vec3::new(2.0, 0.0, 0.0)));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Tab);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(vars.0.get("target_display").map(String::as_str), Some(""), "target_display must be blank in a 2+ player scene");
    assert_eq!(vars.0.get("target_name").map(String::as_str), Some(""), "target_name must be blank in a 2+ player scene");
    assert_eq!(vars.0.get("target_id").map(String::as_str), Some(""), "target_id must be blank in a 2+ player scene");
}

#[test]
fn test_legacy_target_vars_populate_when_single_player() {

    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn((
        CharacterController { ..test_character_controller() },
        PlayerTarget::default(),
        Transform::default(), GlobalTransform::default(),
    ));
    app.world_mut().spawn(test_targetable_at("enemy_a", Vec3::new(2.0, 0.0, 0.0)));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Tab);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("target_display").map(String::as_str), Some("enemy_a"),
        "single-player scenes must keep populating the legacy target_display var, unchanged from before per-player targeting"
    );
}

// ── nameplate_visibility_system store-and-read agreement (Phase 3, split_screen_camera_followups.md) ───

fn nameplate_test_config(max_distance: f32) -> ironhold_core::capabilities::nameplate::NameplateSceneConfig {
    use ironhold_core::capabilities::nameplate::NameplateSceneConfig;
    use ironhold_core::schema::scene_v2::{NameplateOptionsDef, NameplateFactionFilter};
    NameplateSceneConfig {
        enabled: true,
        player_enabled: false,
        options: Some(NameplateOptionsDef {
            faction_filter: NameplateFactionFilter::All,
            max_distance,
            offset: (0.0, 0.0, 0.0),
            name_font_size: 14.0,
            name_color: (0.95, 0.95, 0.95, 1.0),
            text_shadow: false,
            stat_bars: vec![],
            bar_width: 100.0,
            bar_height: 6.0,
            bar_spacing: 9.0,
            show_player_nameplate: false,
        }),
    }
}

/// `nameplate_visibility_system` must evaluate distance against the exact camera that
/// `world_label_screen_pos_system` actually selected to position the anchor — not an
/// independently re-selected one. Two split cameras are both able to *project* the tracked
/// point (both are geometrically close enough), but only the left camera's viewport
/// (`SplitViewportSlot(0)`) actually contains it, so it is the one that positions the anchor.
/// Its distance to the point (10.0) is under `max_distance` (10.5); the right camera's distance
/// to the same point (~11.66) is NOT. If `nameplate_visibility_system` picked the right camera
/// (e.g. by re-selecting "nearest" independently) the anchor would wrongly hide.
#[test]
fn test_nameplate_visibility_agrees_with_world_label_selected_camera_in_split_screen() {
    use ironhold_core::capabilities::nameplate::{
        NameplateTag, NameplateAnchor, NameplateAnchorWidget, NameplateCameraDistance,
    };

    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);
    app.world_mut().insert_resource(nameplate_test_config(10.5));

    // Left camera (slot 0): 10.0 units from its own look-at target.
    let (t0, g0, cam0) = ortho_camera_bundle(
        Vec3::new(-3.0, 0.0, 10.0), Vec3::new(-3.0, 0.0, 0.0), 10.0, true, 0,
        Some(Viewport { physical_position: UVec2::ZERO, physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam0, t0, g0, SplitViewportSlot(0)));

    // Right camera (slot 1): ~11.66 units from the LEFT camera's target — farther, and its
    // viewport does not contain the projected point, so it must never be the selected camera.
    let (t1, g1, cam1) = ortho_camera_bundle(
        Vec3::new(3.0, 0.0, 10.0), Vec3::new(3.0, 0.0, 0.0), 10.0, true, 1,
        Some(Viewport { physical_position: UVec2::new(640, 0), physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam1, t1, g1, SplitViewportSlot(1)));

    let tracked = app.world_mut().spawn((
        NameplateTag { display_name: "Ally".to_string(), prefab_override: None },
        Transform::from_translation(Vec3::new(-3.0, 0.0, 0.0)),
        GlobalTransform::from(Transform::from_translation(Vec3::new(-3.0, 0.0, 0.0))),
    )).id();

    let anchor = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tracked),
            offset: Vec3::ZERO,
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        NameplateCameraDistance::default(),
        Visibility::Hidden,
        Transform::default(),
    )).id();
    app.world_mut().entity_mut(tracked).insert(NameplateAnchor(anchor));

    app.update();

    let stashed = app.world().get::<NameplateCameraDistance>(anchor).unwrap().0;
    assert!(
        stashed.is_some_and(|d| (d - 10.0).abs() < 0.01),
        "expected the anchor to stash the LEFT camera's distance (10.0), got {:?}", stashed
    );
    assert_eq!(
        *app.world().get::<Visibility>(anchor).unwrap(), Visibility::Visible,
        "distance-culling must agree with world_label_screen_pos_system's own camera pick \
         (left camera, distance 10.0 < max_distance 10.5) — using the right camera's distance \
         (~11.66) would have wrongly hidden the anchor"
    );
}

/// When the tracked point is off every active camera's viewport, `world_label_screen_pos_system`
/// clears `NameplateCameraDistance` to `None` and `nameplate_visibility_system` must treat that
/// as out-of-range (hidden) — even though the point may be well within `max_distance` of some
/// camera in raw world-space terms. Matches the pre-existing `.single()` no-op contract for "no
/// qualifying camera."
#[test]
fn test_nameplate_visibility_hides_when_anchor_is_off_every_active_viewport() {
    use ironhold_core::capabilities::nameplate::{
        NameplateTag, NameplateAnchor, NameplateAnchorWidget, NameplateCameraDistance,
    };

    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);
    // Generous max_distance — a raw-distance-only check would pass easily.
    app.world_mut().insert_resource(nameplate_test_config(1000.0));

    let (t0, g0, cam0) = ortho_camera_bundle(
        Vec3::new(-10.0, 0.0, 10.0), Vec3::new(-10.0, 0.0, 0.0), 5.0, true, 0,
        Some(Viewport { physical_position: UVec2::ZERO, physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam0, t0, g0, SplitViewportSlot(0)));

    let (t1, g1, cam1) = ortho_camera_bundle(
        Vec3::new(10.0, 0.0, 10.0), Vec3::new(10.0, 0.0, 0.0), 5.0, true, 1,
        Some(Viewport { physical_position: UVec2::new(640, 0), physical_size: UVec2::new(640, 720), ..default() }),
        UVec2::new(1280, 720),
    );
    app.world_mut().spawn((Camera3d::default(), cam1, t1, g1, SplitViewportSlot(1)));

    // Far outside either camera's narrow (half_extent=5.0) frustum — off-viewport for both,
    // but well within the generous max_distance from either camera's position.
    let tracked = app.world_mut().spawn((
        NameplateTag { display_name: "OffScreenAlly".to_string(), prefab_override: None },
        Transform::from_translation(Vec3::new(1000.0, 0.0, 0.0)),
        GlobalTransform::from(Transform::from_translation(Vec3::new(1000.0, 0.0, 0.0))),
    )).id();

    let anchor = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tracked),
            offset: Vec3::ZERO,
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        NameplateCameraDistance::default(),
        Visibility::Visible,
        Transform::default(),
    )).id();
    app.world_mut().entity_mut(tracked).insert(NameplateAnchor(anchor));

    app.update();

    assert_eq!(
        app.world().get::<NameplateCameraDistance>(anchor).unwrap().0, None,
        "no active camera's viewport contains the point, so the stashed distance must be cleared"
    );
    assert_eq!(
        *app.world().get::<Visibility>(anchor).unwrap(), Visibility::Hidden,
        "an anchor off every active viewport must be treated as out-of-range regardless of \
         max_distance"
    );
}

// ── Phase 4 (split_screen_camera_followups.md): stat label / Ascii world stat bar
// duplication, gated on the loading scene actually being split-screen ──────────────────

fn stat_label_def(stat_key: &str) -> StatLabelDef {
    StatLabelDef {
        stat_key: stat_key.to_string(),
        offset: (0.0, 2.5, 0.0),
        screen_offset: (0.0, 0.0),
        font_size: 16.0,
        color: (0.2, 0.9, 0.2, 1.0),
        show_max: true,
    }
}

fn ascii_world_stat_bar_def(stat_key: &str) -> WorldStatBarDef {
    WorldStatBarDef {
        stat_key: stat_key.to_string(),
        offset: (0.0, 2.8, 0.0),
        screen_offset: (0.0, 0.0),
        fill_color: (0.15, 0.85, 0.15, 0.95),
        bg_color: (0.25, 0.08, 0.08, 0.75),
        color_bands: vec![],
        style: WorldStatBarStyle::Ascii { cells: 10, font_size: 14.0 },
    }
}

fn pixel_world_stat_bar_def(stat_key: &str) -> WorldStatBarDef {
    WorldStatBarDef {
        stat_key: stat_key.to_string(),
        offset: (0.0, 2.8, 0.0),
        screen_offset: (0.0, 0.0),
        fill_color: (0.15, 0.85, 0.15, 0.95),
        bg_color: (0.25, 0.08, 0.08, 0.75),
        color_bands: vec![],
        style: WorldStatBarStyle::Pixel { size: (48.0, 6.0), border: 1.5, border_color: (0.05, 0.05, 0.05, 1.0) },
    }
}

/// Drives a scene load with 2 players (first player's `camera.split` set iff `split` is
/// `Some`) plus a third, non-player `test_stat_prop` entity carrying both `stat_label` and
/// `world_stat_bar` (Ascii). Mirrors `two_player_catalogs_with_split`/`load_two_player_scene`
/// above but adds the stat-widget prop needed to exercise Phase 4.
fn load_two_player_scene_with_stat_prop(app: &mut App, split: Option<SplitScreenDef>) {
    two_player_catalogs_with_split(app, None, split);
    app.world_mut()
        .resource_mut::<LoadedPrefabCatalog>()
        .0
        .prefabs
        .insert("test_stat_prop".to_string(), PrefabDef {
            kind: PrefabKind::Prop,
            model: "char_a".to_string(),
            stat_label: Some(stat_label_def("{self}.health")),
            world_stat_bar: Some(ascii_world_stat_bar_def("{self}.health")),
            ..Default::default()
        });

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
            (id: "dummy_01", prefab: "test_stat_prop", transform: (translation: (0.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
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

#[test]
fn test_stat_widgets_duplicate_ranks_when_scene_is_split_screen() {
    let mut app = setup_test_app();
    app.update();
    load_two_player_scene_with_stat_prop(
        &mut app,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
    );

    let label_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&StatLabelMarker, Option<&WorldLabelRank>)>();
        q.iter(app.world()).map(|(_, rank)| rank.map(|r| r.0)).collect()
    };
    assert_eq!(
        label_ranks.len(), MAX_SPLIT_PLAYERS as usize,
        "a stat_label in a split-screen scene must spawn MAX_SPLIT_PLAYERS ranked siblings, \
         same as the world_labels/label duplication pattern"
    );
    let mut sorted: Vec<u8> = label_ranks.iter().map(|r| r.unwrap_or(0)).collect();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1, 2, 3], "expected exactly one stat label of each rank 0-3");

    let bar_fill_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&WorldStatBarFillMarker, Option<&WorldLabelRank>)>();
        q.iter(app.world()).map(|(_, rank)| rank.map(|r| r.0)).collect()
    };
    assert_eq!(
        bar_fill_ranks.len(), MAX_SPLIT_PLAYERS as usize,
        "an Ascii world_stat_bar's fill entity must also spawn MAX_SPLIT_PLAYERS ranked siblings"
    );
    let mut sorted_bar: Vec<u8> = bar_fill_ranks.iter().map(|r| r.unwrap_or(0)).collect();
    sorted_bar.sort();
    assert_eq!(sorted_bar, vec![0, 1, 2, 3], "expected exactly one bar fill of each rank 0-3");

    // The background track has no marker component distinguishing it from other WorldLabels,
    // but it must still duplicate 1:1 with the fill — count every WorldLabel tracking the prop
    // entity and subtract the (already-verified) fill count to isolate the backgrounds.
    let prop_entity = *app.world().resource::<ironhold_core::runtime::SpawnRegistry>()
        .entities.get("dummy_01").expect("prop entity must register in SpawnRegistry");
    let total_tracking_prop = app.world_mut()
        .query::<&WorldLabel>()
        .iter(app.world())
        .filter(|l| l.tracked_entity == Some(prop_entity))
        .count();
    // 4 stat-label ranks + 4 bar-bg ranks + 4 bar-fill ranks = 12.
    assert_eq!(
        total_tracking_prop, 12,
        "expected 4 stat-label + 4 bar-background + 4 bar-fill WorldLabels tracking the prop"
    );
}

#[test]
fn test_stat_widgets_stay_single_instance_in_non_split_scene() {
    let mut app = setup_test_app();
    app.update();
    load_two_player_scene_with_stat_prop(&mut app, None);

    let label_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&StatLabelMarker, Option<&WorldLabelRank>)>();
        q.iter(app.world()).map(|(_, rank)| rank.map(|r| r.0)).collect()
    };
    assert_eq!(
        label_ranks, vec![None],
        "a non-split scene must spawn exactly 1 stat label with no WorldLabelRank at all — \
         pixel-identical to pre-Phase-4 behavior, no rank-duplication overhead"
    );

    let bar_fill_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&WorldStatBarFillMarker, Option<&WorldLabelRank>)>();
        q.iter(app.world()).map(|(_, rank)| rank.map(|r| r.0)).collect()
    };
    assert_eq!(
        bar_fill_ranks, vec![None],
        "a non-split scene must spawn exactly 1 Ascii world_stat_bar fill entity, no rank siblings"
    );

    let prop_entity = *app.world().resource::<ironhold_core::runtime::SpawnRegistry>()
        .entities.get("dummy_01").expect("prop entity must register in SpawnRegistry");
    let total_tracking_prop = app.world_mut()
        .query::<&WorldLabel>()
        .iter(app.world())
        .filter(|l| l.tracked_entity == Some(prop_entity))
        .count();
    // 1 stat label + 1 bar background + 1 bar fill = 3.
    assert_eq!(
        total_tracking_prop, 3,
        "expected exactly 1 stat-label + 1 bar-background + 1 bar-fill WorldLabel, no duplication"
    );
}

/// Regression guard (debug-detective finding during Phase 4 review): a lone player whose
/// prefab happens to carry a `camera.split` block (e.g. copy-pasted from a co-op prefab) must
/// NOT trigger rank duplication — `spawn_players_and_camera` itself falls back to a single
/// full-window camera whenever fewer than 2 players are present, regardless of `split`, so the
/// gate must check `player_configs.len() >= 2` too, not just `split.is_some()`.
#[test]
fn test_stat_widgets_stay_single_instance_with_one_player_carrying_split_config() {
    let mut app = setup_test_app();
    app.update();

    two_player_catalogs_with_split(
        &mut app, None,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
    );
    app.world_mut()
        .resource_mut::<LoadedPrefabCatalog>()
        .0
        .prefabs
        .insert("test_stat_prop".to_string(), PrefabDef {
            kind: PrefabKind::Prop,
            model: "char_a".to_string(),
            stat_label: Some(stat_label_def("{self}.health")),
            ..Default::default()
        });

    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    // Only ONE player entity ("p1"), despite its prefab carrying `camera.split`.
    let scene: GameSceneV2 = ron::de::from_str(r#"(
        schema_version: 2,
        entities: [
            (id: "p1", prefab: "test_player_1", transform: (translation: (0.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
            (id: "dummy_01", prefab: "test_stat_prop", transform: (translation: (0.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
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

    let label_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&StatLabelMarker, Option<&WorldLabelRank>)>();
        q.iter(app.world()).map(|(_, rank)| rank.map(|r| r.0)).collect()
    };
    assert_eq!(
        label_ranks, vec![None],
        "a single-player scene must spawn exactly 1 stat label with no rank duplication, even \
         though its prefab's camera.split block is set — matching spawn_players_and_camera's own \
         `entities.len() < 2` fallback to a single full-window camera"
    );
}

// ── pixel_world_stat_bar_split_screen_duplication.md: Pixel-style world_stat_bar duplication
// through the real spawn_scene_v2 pipeline (not just the isolated ctx-level unit tests above) ──

/// Mirrors `load_two_player_scene_with_stat_prop` but the prop's `world_stat_bar` uses `Pixel`
/// style instead of `Ascii`, and registers `Assets<ColorMaterial>` (needed by Pixel bars, not
/// registered by `setup_test_app`'s headless plugin set).
fn load_two_player_scene_with_pixel_stat_prop(app: &mut App, split: Option<SplitScreenDef>) {
    app.world_mut().init_resource::<Assets<ColorMaterial>>();
    two_player_catalogs_with_split(app, None, split);
    app.world_mut()
        .resource_mut::<LoadedPrefabCatalog>()
        .0
        .prefabs
        .insert("test_stat_prop".to_string(), PrefabDef {
            kind: PrefabKind::Prop,
            model: "char_a".to_string(),
            world_stat_bar: Some(pixel_world_stat_bar_def("{self}.health")),
            ..Default::default()
        });

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
            (id: "dummy_01", prefab: "test_stat_prop", transform: (translation: (0.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
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

#[test]
fn test_pixel_world_stat_bar_duplicates_ranks_when_scene_is_split_screen() {
    let mut app = setup_test_app();
    app.update();
    load_two_player_scene_with_pixel_stat_prop(
        &mut app,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
    );

    // Fill children carry no WorldLabelRank of their own (only the anchor does — see
    // spawn_world_stat_bar_widget's doc comment), so assert fill COUNT here and rank identity
    // on the anchors (the WorldLabel-bearing entities) below.
    let fill_count = app.world_mut().query::<&WorldPixelBarFillMarker>().iter(app.world()).count();
    assert_eq!(
        fill_count, MAX_SPLIT_PLAYERS as usize,
        "a Pixel world_stat_bar spawned via the real scene-load pipeline in a split-screen scene \
         must spawn MAX_SPLIT_PLAYERS fill entities, same as Ascii already does"
    );

    let prop_entity = *app.world().resource::<ironhold_core::runtime::SpawnRegistry>()
        .entities.get("dummy_01").expect("prop entity must register in SpawnRegistry");
    let anchor_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&WorldLabel, Option<&WorldLabelRank>)>();
        q.iter(app.world())
            .filter(|(l, _)| l.tracked_entity == Some(prop_entity))
            .map(|(_, r)| r.map(|r| r.0))
            .collect()
    };
    assert_eq!(
        anchor_ranks.len(), MAX_SPLIT_PLAYERS as usize,
        "must spawn MAX_SPLIT_PLAYERS anchors (one WorldLabel each) tracking the prop"
    );
    let mut sorted: Vec<u8> = anchor_ranks.iter().map(|r| r.unwrap_or(0)).collect();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1, 2, 3], "expected exactly one anchor of each rank 0-3");
}

#[test]
fn test_pixel_world_stat_bar_stays_single_instance_in_non_split_scene() {
    let mut app = setup_test_app();
    app.update();
    load_two_player_scene_with_pixel_stat_prop(&mut app, None);

    let fill_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&WorldPixelBarFillMarker, Option<&WorldLabelRank>)>();
        q.iter(app.world()).map(|(_, rank)| rank.map(|r| r.0)).collect()
    };
    assert_eq!(
        fill_ranks, vec![None],
        "a non-split scene must spawn exactly 1 Pixel world_stat_bar fill entity via the real \
         scene-load pipeline — pixel-identical to pre-fix behavior, no rank-duplication overhead"
    );
}

// ── world_icon_stat_bar.md: Icon-style world_stat_bar through the real spawn_scene_v2
// pipeline (not just the isolated ctx-level unit tests above) ───────────────────────────

fn load_two_player_scene_with_icon_stat_prop(app: &mut App, split: Option<SplitScreenDef>) {
    app.world_mut().init_resource::<Assets<TextureAtlasLayout>>();
    two_player_catalogs_with_split(app, None, split);
    app.world_mut()
        .resource_mut::<LoadedAssetCatalog>()
        .0
        .textures
        .insert("ui_icons".to_string(), "shared/ui/ui_icons.png".to_string());
    app.world_mut()
        .resource_mut::<LoadedPrefabCatalog>()
        .0
        .prefabs
        .insert("test_stat_prop".to_string(), PrefabDef {
            kind: PrefabKind::Prop,
            model: "char_a".to_string(),
            world_stat_bar: Some(icon_world_stat_bar_def("{self}.health")),
            ..Default::default()
        });

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
            (id: "dummy_01", prefab: "test_stat_prop", transform: (translation: (0.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
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

#[test]
fn test_icon_world_stat_bar_duplicates_ranks_when_scene_is_split_screen() {
    let mut app = setup_test_app();
    app.update();
    load_two_player_scene_with_icon_stat_prop(
        &mut app,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
    );

    let prop_entity = *app.world().resource::<ironhold_core::runtime::SpawnRegistry>()
        .entities.get("dummy_01").expect("prop entity must register in SpawnRegistry");
    let anchor_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&WorldIconBar, &WorldLabel, Option<&WorldLabelRank>)>();
        q.iter(app.world())
            .filter(|(_, l, _)| l.tracked_entity == Some(prop_entity))
            .map(|(_, _, r)| r.map(|r| r.0))
            .collect()
    };
    assert_eq!(
        anchor_ranks.len(), MAX_SPLIT_PLAYERS as usize,
        "an Icon world_stat_bar spawned via the real scene-load pipeline in a split-screen scene \
         must spawn MAX_SPLIT_PLAYERS anchors, same as Pixel already does"
    );
    let mut sorted: Vec<u8> = anchor_ranks.iter().map(|r| r.unwrap_or(0)).collect();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1, 2, 3], "expected exactly one anchor of each rank 0-3");
}

#[test]
fn test_icon_world_stat_bar_stays_single_instance_in_non_split_scene() {
    let mut app = setup_test_app();
    app.update();
    load_two_player_scene_with_icon_stat_prop(&mut app, None);

    let prop_entity = *app.world().resource::<ironhold_core::runtime::SpawnRegistry>()
        .entities.get("dummy_01").expect("prop entity must register in SpawnRegistry");
    let anchor_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&WorldIconBar, &WorldLabel, Option<&WorldLabelRank>)>();
        q.iter(app.world())
            .filter(|(_, l, _)| l.tracked_entity == Some(prop_entity))
            .map(|(_, _, r)| r.map(|r| r.0))
            .collect()
    };
    assert_eq!(
        anchor_ranks, vec![None],
        "a non-split scene must spawn exactly 1 Icon world_stat_bar anchor via the real \
         scene-load pipeline — no rank-duplication overhead"
    );
}

// ── Per-viewport target HUD readout (Phase 1, per_player_split_screen_targeting.md) ─────

fn load_two_player_scene_with_target_hud(app: &mut App, author_target_hud: bool) {
    two_player_catalogs_with_split(
        app, None,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
    );
    app.world_mut()
        .resource_mut::<LoadedPrefabCatalog>()
        .0
        .prefabs
        .insert("test_hud_enemy".to_string(), PrefabDef {
            kind: PrefabKind::Prop,
            model: "char_a".to_string(),
            targetable: true,
            ..Default::default()
        });

    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let target_hud_block = if author_target_hud { "target_hud: Some((show: Full))," } else { "" };
    let ron_text = format!(
        r#"(
        schema_version: 2,
        {}
        entities: [
            (id: "p1", prefab: "test_player_1", transform: (translation: (-4.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
            (id: "p2", prefab: "test_player_2", transform: (translation: (4.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
            (id: "enemy_01", prefab: "test_hud_enemy", transform: (translation: (0.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
        ],
        ui: [],
    )"#,
        target_hud_block
    );
    let scene: GameSceneV2 = ron::de::from_str(&ron_text).unwrap();
    let scene_handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadingScene);
    app.update();
    app.update();
    app.update();
}

#[test]
fn test_target_hud_shows_each_players_own_target_independently() {
    use ironhold_core::capabilities::player::PlayerTarget;
    use ironhold_core::capabilities::camera::LinkedTargetHud;

    let mut app = setup_test_app();
    app.update();
    spawn_primary_window(&mut app, 1280, 720, 1.0);
    load_two_player_scene_with_target_hud(&mut app, true);

    let enemy_entity = *app.world().resource::<SpawnRegistry>()
        .entities.get("enemy_01").expect("enemy must register in SpawnRegistry");

    // Map each split camera's HUD readout entity to its owning player entity.
    let cams: Vec<(Entity, Entity)> = {
        let mut q = app.world_mut().query::<(&CameraTargets, &LinkedTargetHud)>();
        q.iter(app.world()).filter_map(|(t, l)| t.0.first().copied().map(|target| (target, l.0))).collect()
    };
    assert_eq!(cams.len(), 2, "both split cameras must get a target-HUD readout entity");

    // Only player 1 (the first split camera's target) selects the enemy.
    let (player1, hud1) = cams[0];
    let (_player2, hud2) = cams[1];
    app.world_mut().get_mut::<PlayerTarget>(player1).unwrap().0 = Some("enemy_01".to_string());
    let _ = enemy_entity; // registered above purely to confirm the scene wired the prop correctly
    app.update();

    let text1 = app.world().get::<Text>(hud1).unwrap();
    assert!(text1.0.contains("enemy_01"), "player 1's own readout must show their selected target, got {:?}", text1.0);
    let vis1 = app.world().get::<Visibility>(hud1).unwrap();
    assert_eq!(*vis1, Visibility::Visible, "player 1's readout must be visible once a target is selected");

    let text2 = app.world().get::<Text>(hud2).unwrap();
    assert!(text2.0.is_empty(), "player 2's readout must stay empty — player 2 selected nothing");
    let vis2 = app.world().get::<Visibility>(hud2).unwrap();
    assert_eq!(*vis2, Visibility::Hidden, "player 2's readout must stay hidden — player 2 has no target");
}

#[test]
fn test_target_hud_absent_without_scene_config_block() {
    use ironhold_core::capabilities::camera::SplitScreenTargetHud;

    let mut app = setup_test_app();
    app.update();
    load_two_player_scene_with_target_hud(&mut app, false);

    let hud_count = app.world_mut().query::<&SplitScreenTargetHud>().iter(app.world()).count();
    assert_eq!(
        hud_count, 0,
        "a scene with no target_hud: block must spawn zero HUD readout entities — opt-in, \
         matching target_indicator:'s own opt-in pattern"
    );
}

// ── Per-player stat pools (per_player_stat_pools.md) ────────────────────────────

fn stat_def(base: f32, max: f32) -> ironhold_core::schema::StatDef {
    ironhold_core::schema::StatDef {
        base, min: 0.0, max, soft_max: None, regen_rate: 0.0, regen_delay: 0.0, thresholds: vec![],
    }
}

#[test]
fn test_action_bar_cost_deducts_from_owning_players_own_stat_map_independently() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;
    use ironhold_core::schema::scene_v2::SlotCost;
    use ironhold_core::schema::stats::{StatMap, LoadedStats, LiveStat};
    use ironhold_core::schema::Action;

    let mut app = setup_test_app();
    app.update();

    // A global "mana" stat with an easily-distinguishable value — if either player's deduct
    // wrongly routed here instead of their own StatMap, this value would move and the final
    // assertion below would catch it (debug-detective finding: an absent-key assertion alone
    // can't discriminate a misroute, since a global miss just warns and no-ops either way).
    app.world_mut().resource_mut::<LoadedStats>().0.insert("mana".to_string(), LiveStat::new(stat_def(999.0, 999.0)));

    let mut p1_stats = StatMap::default();
    p1_stats.0.insert("mana".to_string(), LiveStat::new(stat_def(50.0, 50.0)));
    let mut p2_stats = StatMap::default();
    p2_stats.0.insert("mana".to_string(), LiveStat::new(stat_def(30.0, 30.0)));

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("p1_fired".to_string(), "yes".to_string())],
        cooldown_secs: None,
        cost: Some(SlotCost { stat: "mana".to_string(), amount: 20.0 }),
        owner_player: Some(0),
    });
    app.world_mut().spawn(ActionSlotUi {
        slot_key: "2".to_string(),
        resolved_key: Some(KeyCode::Digit2),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("p2_fired".to_string(), "yes".to_string())],
        cooldown_secs: None,
        cost: Some(SlotCost { stat: "mana".to_string(), amount: 20.0 }),
        owner_player: Some(1),
    });

    let p1_entity = app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        test_character_controller(),
        PlayerTarget::default(),
        PlayerIndex(0),
        p1_stats,
    )).id();
    let p2_entity = app.world_mut().spawn((
        SpawnId("player_02".to_string()),
        test_character_controller(),
        PlayerTarget::default(),
        PlayerIndex(1),
        p2_stats,
    )).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("player_01".to_string(), p1_entity);
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("player_02".to_string(), p2_entity);

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit2);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(vars.0.get("p1_fired").map(String::as_str), Some("yes"), "player 1's slot must fire — their own pool covers the cost");
    assert_eq!(vars.0.get("p2_fired").map(String::as_str), Some("yes"), "player 2's slot must fire — their own pool covers the cost");

    let p1_after = app.world().get::<StatMap>(p1_entity).unwrap();
    assert_eq!(p1_after.0["mana"].current, 30.0, "player 1's own pool must be deducted (50 - 20)");
    let p2_after = app.world().get::<StatMap>(p2_entity).unwrap();
    assert_eq!(p2_after.0["mana"].current, 10.0, "player 2's own pool must be deducted independently (30 - 20), not cross-drained from player 1's spend");

    let loaded_stats = app.world().resource::<LoadedStats>();
    assert_eq!(loaded_stats.0["mana"].current, 999.0, "the shared global pool must never be touched when both players have their own StatMap-backed pool");
}

#[test]
fn test_action_bar_cost_falls_back_to_global_pool_when_player_has_no_stat_map() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;
    use ironhold_core::schema::scene_v2::SlotCost;
    use ironhold_core::schema::stats::{LoadedStats, LiveStat};
    use ironhold_core::schema::Action;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<LoadedStats>().0.insert("mana".to_string(), LiveStat::new(stat_def(40.0, 40.0)));

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("fired".to_string(), "yes".to_string())],
        cooldown_secs: None,
        cost: Some(SlotCost { stat: "mana".to_string(), amount: 15.0 }),
        owner_player: None,
    });
    // No StatMap component at all — matches every existing single-player project (and any player
    // prefab that doesn't declare `stat_templates`).
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        test_character_controller(),
        PlayerTarget::default(),
    ));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(vars.0.get("fired").map(String::as_str), Some("yes"));
    let loaded_stats = app.world().resource::<LoadedStats>();
    assert_eq!(loaded_stats.0["mana"].current, 25.0, "must fall back to the shared global pool exactly as before per-player stat pools existed, when the player has no StatMap");
}

#[test]
fn test_action_bar_cost_check_uses_own_pool_even_when_global_pool_would_cover_it() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;
    use ironhold_core::schema::scene_v2::SlotCost;
    use ironhold_core::schema::stats::{StatMap, LoadedStats, LiveStat};
    use ironhold_core::schema::Action;

    let mut app = setup_test_app();
    app.update();

    // A huge global "mana" pool that would easily cover the cost — must be irrelevant once the
    // player has their own (too-low) StatMap entry for the same key.
    app.world_mut().resource_mut::<LoadedStats>().0.insert("mana".to_string(), LiveStat::new(stat_def(999.0, 999.0)));

    let mut p1_stats = StatMap::default();
    p1_stats.0.insert("mana".to_string(), LiveStat::new(stat_def(5.0, 100.0)));

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("fired".to_string(), "yes".to_string())],
        cooldown_secs: None,
        cost: Some(SlotCost { stat: "mana".to_string(), amount: 20.0 }),
        owner_player: Some(0),
    });
    let p1_entity = app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        test_character_controller(),
        PlayerTarget::default(),
        PlayerIndex(0),
        p1_stats,
    )).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("player_01".to_string(), p1_entity);

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(vars.0.get("fired"), None, "must be blocked by the player's own insufficient pool, not silently pass because the global pool has plenty");
    let p1_after = app.world().get::<StatMap>(p1_entity).unwrap();
    assert_eq!(p1_after.0["mana"].current, 5.0, "the player's own pool must be untouched — the slot never fired");
}

#[test]
fn test_action_bar_visual_dim_reflects_owning_players_own_pool_independently() {
    use ironhold_core::capabilities::action_bar::{ActionSlotUi, CooldownOverlay};
    use ironhold_core::schema::scene_v2::SlotCost;
    use ironhold_core::schema::stats::{StatMap, LiveStat};

    let mut app = setup_test_app();
    app.update();

    let mut p1_stats = StatMap::default();
    p1_stats.0.insert("mana".to_string(), LiveStat::new(stat_def(5.0, 100.0))); // below cost
    let mut p2_stats = StatMap::default();
    p2_stats.0.insert("mana".to_string(), LiveStat::new(stat_def(100.0, 100.0))); // covers cost

    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        test_character_controller(),
        PlayerTarget::default(),
        PlayerIndex(0),
        p1_stats,
    ));
    app.world_mut().spawn((
        SpawnId("player_02".to_string()),
        test_character_controller(),
        PlayerTarget::default(),
        PlayerIndex(1),
        p2_stats,
    ));

    let slot1 = app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![],
        cooldown_secs: None,
        cost: Some(SlotCost { stat: "mana".to_string(), amount: 20.0 }),
        owner_player: Some(0),
    }).id();
    let overlay1 = app.world_mut().spawn((
        CooldownOverlay { slot_key: "1".to_string() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        ChildOf(slot1),
    )).id();

    let slot2 = app.world_mut().spawn(ActionSlotUi {
        slot_key: "2".to_string(),
        resolved_key: Some(KeyCode::Digit2),
        resolved_gamepad_button: None,
        do_actions: vec![],
        cooldown_secs: None,
        cost: Some(SlotCost { stat: "mana".to_string(), amount: 20.0 }),
        owner_player: Some(1),
    }).id();
    let overlay2 = app.world_mut().spawn((
        CooldownOverlay { slot_key: "2".to_string() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        ChildOf(slot2),
    )).id();

    app.update();

    let bg1 = app.world().get::<BackgroundColor>(overlay1).unwrap();
    assert!((bg1.0.alpha() - 0.45).abs() < 0.01, "player 1's overlay must dim — their own pool (5) is below the cost (20)");
    let bg2 = app.world().get::<BackgroundColor>(overlay2).unwrap();
    assert!(bg2.0.alpha() < 0.01, "player 2's overlay must stay undimmed — their own pool (100) covers the cost, independent of player 1's shortage");
}

#[test]
fn test_action_bar_cost_regen_on_one_players_pool_does_not_affect_the_others_dim_state() {
    use ironhold_core::capabilities::action_bar::{ActionSlotUi, CooldownOverlay};
    use ironhold_core::schema::scene_v2::SlotCost;
    use ironhold_core::schema::stats::{StatMap, LiveStat};

    let mut app = setup_test_app();
    app.update();

    // Player 1 regenerates past the cost threshold; player 2 starts already-sufficient and never
    // changes — the point is that player 1 crossing their own threshold must not perturb player
    // 2's independently-resolved dim state.
    let mut p1_stats = StatMap::default();
    p1_stats.0.insert("mana".to_string(), LiveStat::new(stat_def(0.0, 100.0)));
    let mut p2_stats = StatMap::default();
    p2_stats.0.insert("mana".to_string(), LiveStat::new(stat_def(100.0, 100.0)));

    let p1_entity = app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        test_character_controller(),
        PlayerTarget::default(),
        PlayerIndex(0),
        p1_stats,
    )).id();
    app.world_mut().spawn((
        SpawnId("player_02".to_string()),
        test_character_controller(),
        PlayerTarget::default(),
        PlayerIndex(1),
        p2_stats,
    ));

    let slot1 = app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![],
        cooldown_secs: None,
        cost: Some(SlotCost { stat: "mana".to_string(), amount: 20.0 }),
        owner_player: Some(0),
    }).id();
    let overlay1 = app.world_mut().spawn((
        CooldownOverlay { slot_key: "1".to_string() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        ChildOf(slot1),
    )).id();

    let slot2 = app.world_mut().spawn(ActionSlotUi {
        slot_key: "2".to_string(),
        resolved_key: Some(KeyCode::Digit2),
        resolved_gamepad_button: None,
        do_actions: vec![],
        cooldown_secs: None,
        cost: Some(SlotCost { stat: "mana".to_string(), amount: 20.0 }),
        owner_player: Some(1),
    }).id();
    let overlay2 = app.world_mut().spawn((
        CooldownOverlay { slot_key: "2".to_string() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        ChildOf(slot2),
    )).id();

    app.update();
    let bg1_before = app.world().get::<BackgroundColor>(overlay1).unwrap().0.alpha();
    assert!((bg1_before - 0.45).abs() < 0.01, "player 1 starts below the cost — dimmed");

    // Directly push player 1's own pool past the threshold (simulating regen having occurred),
    // without touching player 2's at all.
    app.world_mut().get_mut::<StatMap>(p1_entity).unwrap().0.get_mut("mana").unwrap().current = 50.0;
    app.update();

    let bg1_after = app.world().get::<BackgroundColor>(overlay1).unwrap();
    assert!(bg1_after.0.alpha() < 0.01, "player 1's overlay must clear once their own pool crosses the cost threshold");
    let bg2_after = app.world().get::<BackgroundColor>(overlay2).unwrap();
    assert!(bg2_after.0.alpha() < 0.01, "player 2's overlay must remain unaffected by player 1's pool change");
}

// ── Player stat widgets (player_stat_widgets.md) ────────────────────────────────

// Part A: direct unit coverage of the extracted spawn_stat_label_widget/spawn_world_stat_bar_widget
// helpers (capabilities/stat_display.rs) — the actual entity-spawning logic factored out of
// scene_loader.rs's two Phase-B loops and drain_dynamic_stat_ui_system. These exercise the helpers
// in isolation (a bare tracked entity, no scene/player involved) so a regression here can't be
// confused with a player-wiring bug (covered separately below).

#[test]
fn test_spawn_stat_label_widget_spawns_one_entity_when_not_split_screen() {
    let mut app = setup_test_app();
    app.update();

    let tracked = app.world_mut().spawn(SpawnId("dummy_01".to_string())).id();
    let def = stat_label_def("{self}.health");
    app.world_mut().run_system_once(move |mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>| {
        let ctx = StatWidgetSpawnCtx {
            meshes: &mut meshes,
            color_materials: None,
            depth_scale: None,
            is_split_screen: false,
            atlas_layouts: None,
            asset_server: None,
            asset_catalog: None,
        };
        spawn_stat_label_widget(&mut commands, tracked, "dummy_01.health", &def, &ctx);
    }).unwrap();

    let mut q = app.world_mut().query::<(&StatLabelMarker, &WorldLabel, Option<&WorldLabelRank>)>();
    let results: Vec<_> = q.iter(app.world()).collect();
    assert_eq!(results.len(), 1, "a non-split scene must spawn exactly one stat label entity, no ranked siblings");
    let (marker, label, rank) = results[0];
    assert_eq!(marker.stat_key, "dummy_01.health");
    assert_eq!(label.tracked_entity, Some(tracked));
    assert!(rank.is_none(), "rank 0 (implicit) must carry no WorldLabelRank component");
}

#[test]
fn test_spawn_stat_label_widget_spawns_ranked_siblings_when_split_screen() {
    let mut app = setup_test_app();
    app.update();

    let tracked = app.world_mut().spawn(SpawnId("dummy_01".to_string())).id();
    let def = stat_label_def("{self}.health");
    app.world_mut().run_system_once(move |mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>| {
        let ctx = StatWidgetSpawnCtx {
            meshes: &mut meshes,
            color_materials: None,
            depth_scale: None,
            is_split_screen: true,
            atlas_layouts: None,
            asset_server: None,
            asset_catalog: None,
        };
        spawn_stat_label_widget(&mut commands, tracked, "dummy_01.health", &def, &ctx);
    }).unwrap();

    let mut q = app.world_mut().query::<(&StatLabelMarker, Option<&WorldLabelRank>)>();
    let ranks: Vec<u8> = q.iter(app.world()).map(|(_, r)| r.map(|r| r.0).unwrap_or(0)).collect();
    assert_eq!(ranks.len(), MAX_SPLIT_PLAYERS as usize, "a split-screen ctx must spawn MAX_SPLIT_PLAYERS ranked siblings");
    let mut sorted = ranks.clone();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1, 2, 3]);
}

#[test]
fn test_spawn_world_stat_bar_widget_pixel_style_spawns_anchor_and_children_without_duplication() {
    let mut app = setup_test_app();
    app.update();
    // Pixel-style bars need Assets<ColorMaterial> — not registered by setup_test_app (headless,
    // no 2D rendering plugin), so this test provides it locally, matching the real runtime's
    // SceneMaterialParams.color_materials being Option-typed for exactly this reason.
    app.world_mut().init_resource::<Assets<ColorMaterial>>();

    let tracked = app.world_mut().spawn(SpawnId("dummy_01".to_string())).id();
    let def = WorldStatBarDef {
        stat_key: "{self}.health".to_string(),
        offset: (0.0, 2.5, 0.0),
        screen_offset: (0.0, 0.0),
        fill_color: (0.15, 0.85, 0.15, 1.0),
        bg_color: (0.2, 0.05, 0.05, 0.85),
        color_bands: vec![],
        style: WorldStatBarStyle::Pixel { size: (48.0, 6.0), border: 1.5, border_color: (0.05, 0.05, 0.05, 1.0) },
    };
    app.world_mut().run_system_once(move |
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut color_materials: Option<ResMut<Assets<ColorMaterial>>>,
    | {
        let mut ctx = StatWidgetSpawnCtx {
            meshes: &mut meshes,
            color_materials: color_materials.as_deref_mut(),
            depth_scale: None,
            is_split_screen: false,
            atlas_layouts: None,
            asset_server: None,
            asset_catalog: None,
        };
        spawn_world_stat_bar_widget(&mut commands, tracked, "dummy_01.health", &def, &mut ctx);
    }).unwrap();

    let fill_count = app.world_mut().query::<&WorldPixelBarFillMarker>().iter(app.world()).count();
    assert_eq!(fill_count, 1, "a non-split ctx must spawn exactly one Pixel fill entity, no rank overhead");

    // Anchor + border + bg + fill = 4 entities total tracking the entity via WorldLabel (the
    // anchor is the only one with a WorldLabel; border/bg/fill are its Bevy-hierarchy children).
    let anchor_count = app.world_mut().query::<&WorldLabel>().iter(app.world())
        .filter(|l| l.tracked_entity == Some(tracked)).count();
    assert_eq!(anchor_count, 1, "exactly one anchor WorldLabel must track the entity");

    let mut q = app.world_mut().query::<&ChildOf>();
    let child_count = q.iter(app.world()).count();
    assert!(child_count >= 3, "border + background + fill must all be spawned as children of the anchor");
}

/// `pixel_world_stat_bar_split_screen_duplication.md`: a split-screen ctx must duplicate the
/// Pixel bar's whole anchor+children hierarchy per rank, exactly like Ascii already does, while
/// sharing the border/background mesh+material assets across ranks (only the fill scales 1:1
/// with rank count).
#[test]
fn test_spawn_world_stat_bar_widget_pixel_style_duplicates_ranks_when_split_screen() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().init_resource::<Assets<ColorMaterial>>();

    let tracked = app.world_mut().spawn(SpawnId("dummy_01".to_string())).id();
    let def = WorldStatBarDef {
        stat_key: "{self}.health".to_string(),
        offset: (0.0, 2.5, 0.0),
        screen_offset: (0.0, 0.0),
        fill_color: (0.15, 0.85, 0.15, 1.0),
        bg_color: (0.2, 0.05, 0.05, 0.85),
        color_bands: vec![],
        style: WorldStatBarStyle::Pixel { size: (48.0, 6.0), border: 1.5, border_color: (0.05, 0.05, 0.05, 1.0) },
    };
    app.world_mut().run_system_once(move |
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut color_materials: Option<ResMut<Assets<ColorMaterial>>>,
    | {
        let mut ctx = StatWidgetSpawnCtx {
            meshes: &mut meshes,
            color_materials: color_materials.as_deref_mut(),
            depth_scale: None,
            is_split_screen: true,
            atlas_layouts: None,
            asset_server: None,
            asset_catalog: None,
        };
        spawn_world_stat_bar_widget(&mut commands, tracked, "dummy_01.health", &def, &mut ctx);
    }).unwrap();

    // Fill (and border/bg) children carry no WorldLabelRank of their own — only the anchor does,
    // since Bevy's hierarchy visibility propagation (InheritedVisibility) cascades the anchor's
    // Visibility::Hidden to its children automatically (see spawn_world_stat_bar_widget's doc
    // comment). So: fill COUNT must scale with rank, and rank identity is asserted on the anchor.
    let fill_count = app.world_mut().query::<&WorldPixelBarFillMarker>().iter(app.world()).count();
    assert_eq!(
        fill_count, MAX_SPLIT_PLAYERS as usize,
        "a split-screen ctx must spawn MAX_SPLIT_PLAYERS Pixel fill entities, same as Ascii"
    );

    let anchor_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&WorldLabel, Option<&WorldLabelRank>)>();
        q.iter(app.world())
            .filter(|(l, _)| l.tracked_entity == Some(tracked))
            .map(|(_, r)| r.map(|r| r.0))
            .collect()
    };
    assert_eq!(
        anchor_ranks.len(), MAX_SPLIT_PLAYERS as usize,
        "must spawn MAX_SPLIT_PLAYERS anchors (one WorldLabel each), not just MAX_SPLIT_PLAYERS fills"
    );
    let mut sorted_anchor_ranks: Vec<u8> = anchor_ranks.iter().map(|r| r.unwrap_or(0)).collect();
    sorted_anchor_ranks.sort();
    assert_eq!(sorted_anchor_ranks, vec![0, 1, 2, 3], "expected exactly one anchor of each rank 0-3");

    // Regression guard: border/background geometry is shared (registered once, cloned per rank),
    // so Assets<Mesh>/<ColorMaterial> must NOT grow 4x — only the per-rank fill mesh/material,
    // which must remain independent (rank count), should scale.
    let mesh_count = app.world().resource::<Assets<Mesh>>().iter().count();
    let mat_count = app.world().resource::<Assets<ColorMaterial>>().iter().count();
    // 1 shared border mesh + 1 shared bg mesh + 4 distinct fill meshes (one per rank) = 6.
    assert_eq!(
        mesh_count, 2 + MAX_SPLIT_PLAYERS as usize,
        "border/bg meshes must be registered once and shared across ranks — only fill meshes \
         (one per rank) should scale with MAX_SPLIT_PLAYERS"
    );
    assert_eq!(
        mat_count, 2 + MAX_SPLIT_PLAYERS as usize,
        "border/bg materials must be registered once and shared across ranks — only fill \
         materials (one per rank) should scale with MAX_SPLIT_PLAYERS"
    );
}

/// Nameplate zoom-spacing fix: `Pixel`-style `world_stat_bar` anchors used to hardcode
/// `depth_scale: None` regardless of `ctx.depth_scale` (a pre-existing, documented v1 exclusion —
/// see planning/backlog.md). They now pass `ctx.depth_scale` straight through, same as every
/// other `world_stat_bar` style — this is the one-line change under test, not a formula test
/// (that's covered by the `test_world_label_anchor_scale_*` group above). Icon and Textured
/// anchors changed identically at their own spawn sites in this same function; not re-tested here
/// since the change is mechanically identical, not per-style logic.
#[test]
fn test_spawn_world_stat_bar_widget_pixel_style_anchor_inherits_ctx_depth_scale() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().init_resource::<Assets<ColorMaterial>>();

    let tracked = app.world_mut().spawn(SpawnId("dummy_01".to_string())).id();
    let def = WorldStatBarDef {
        stat_key: "{self}.health".to_string(),
        offset: (0.0, 2.5, 0.0),
        screen_offset: (0.0, 0.0),
        fill_color: (0.15, 0.85, 0.15, 1.0),
        bg_color: (0.2, 0.05, 0.05, 0.85),
        color_bands: vec![],
        style: WorldStatBarStyle::Pixel { size: (48.0, 6.0), border: 1.5, border_color: (0.05, 0.05, 0.05, 1.0) },
    };
    app.world_mut().run_system_once(move |
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut color_materials: Option<ResMut<Assets<ColorMaterial>>>,
    | {
        let mut ctx = StatWidgetSpawnCtx {
            meshes: &mut meshes,
            color_materials: color_materials.as_deref_mut(),
            depth_scale: Some((8.0, 0.5)),
            is_split_screen: false,
            atlas_layouts: None,
            asset_server: None,
            asset_catalog: None,
        };
        spawn_world_stat_bar_widget(&mut commands, tracked, "dummy_01.health", &def, &mut ctx);
    }).unwrap();

    let anchor_depth_scale = app.world_mut().query::<&WorldLabel>().iter(app.world())
        .find(|l| l.tracked_entity == Some(tracked))
        .expect("anchor must have been spawned")
        .depth_scale;
    assert_eq!(
        anchor_depth_scale,
        Some((8.0, 0.5)),
        "a Pixel bar anchor must inherit ctx.depth_scale instead of the pre-fix hardcoded None"
    );
}

/// `WorldStatBarDef.screen_offset` (nameplate zoom-spacing fix round 2) must reach the spawned
/// anchor's `WorldLabel.screen_offset` — the schema-to-runtime wiring half of that mechanism,
/// complementing the depth_scale flow-through test above.
#[test]
fn test_spawn_world_stat_bar_widget_pixel_style_anchor_inherits_def_screen_offset() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().init_resource::<Assets<ColorMaterial>>();

    let tracked = app.world_mut().spawn(SpawnId("dummy_01".to_string())).id();
    let def = WorldStatBarDef {
        stat_key: "{self}.health".to_string(),
        offset: (0.0, 2.5, 0.0),
        screen_offset: (0.0, -22.0),
        fill_color: (0.15, 0.85, 0.15, 1.0),
        bg_color: (0.2, 0.05, 0.05, 0.85),
        color_bands: vec![],
        style: WorldStatBarStyle::Pixel { size: (48.0, 6.0), border: 1.5, border_color: (0.05, 0.05, 0.05, 1.0) },
    };
    app.world_mut().run_system_once(move |
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut color_materials: Option<ResMut<Assets<ColorMaterial>>>,
    | {
        let mut ctx = StatWidgetSpawnCtx {
            meshes: &mut meshes,
            color_materials: color_materials.as_deref_mut(),
            depth_scale: None,
            is_split_screen: false,
            atlas_layouts: None,
            asset_server: None,
            asset_catalog: None,
        };
        spawn_world_stat_bar_widget(&mut commands, tracked, "dummy_01.health", &def, &mut ctx);
    }).unwrap();

    let anchor_screen_offset = app.world_mut().query::<&WorldLabel>().iter(app.world())
        .find(|l| l.tracked_entity == Some(tracked))
        .expect("anchor must have been spawned")
        .screen_offset;
    assert_eq!(
        anchor_screen_offset,
        Vec2::new(0.0, -22.0),
        "a Pixel bar anchor must inherit def.screen_offset instead of the pre-fix hardcoded Vec2::ZERO"
    );
}

// ── world_icon_stat_bar.md: Icon-style world_stat_bar (discrete per-cell sprites) ────────

fn icon_test_catalog() -> AssetCatalog {
    let mut catalog = AssetCatalog::default();
    catalog.textures.insert("ui_icons".to_string(), "shared/ui/ui_icons.png".to_string());
    catalog
}

fn icon_world_stat_bar_def(stat_key: &str) -> WorldStatBarDef {
    WorldStatBarDef {
        stat_key: stat_key.to_string(),
        offset: (0.0, 2.8, 0.0),
        screen_offset: (0.0, 0.0),
        fill_color: (0.15, 0.85, 0.15, 0.95),
        bg_color: (0.25, 0.08, 0.08, 0.75),
        color_bands: vec![],
        style: WorldStatBarStyle::Icon {
            icon_sheet: "ui_icons".to_string(),
            icon_cols: 8, icon_rows: 8, icon_cell_size: 64,
            filled_index: 12, empty_index: 13,
            cells: 5, spacing: 4.0, size: (24.0, 24.0),
        },
    }
}

#[test]
fn test_world_icon_bar_update_system_computes_ceil_rounded_fill_count() {
    use ironhold_core::capabilities::stat_display::world_icon_bar_update_system;
    use ironhold_core::schema::stats::{StatMap, LiveStat};

    let mut app = setup_test_app();
    app.update();
    app.world_mut().init_resource::<Assets<TextureAtlasLayout>>();
    let catalog = icon_test_catalog();

    let mut stats = StatMap::default();
    stats.0.insert("health".to_string(), LiveStat::new(stat_def(100.0, 100.0)));
    let tracked = app.world_mut().spawn((SpawnId("dummy_01".to_string()), stats)).id();

    let def = icon_world_stat_bar_def("dummy_01.health");
    app.world_mut().run_system_once(move |
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
        asset_server: Res<AssetServer>,
    | {
        let mut ctx = StatWidgetSpawnCtx {
            meshes: &mut meshes,
            color_materials: None,
            depth_scale: None,
            is_split_screen: false,
            atlas_layouts: Some(&mut atlas_layouts),
            asset_server: Some(&asset_server),
            asset_catalog: Some(&catalog),
        };
        spawn_world_stat_bar_widget(&mut commands, tracked, "dummy_01.health", &def, &mut ctx);
    }).unwrap();

    // Cells are spawned in order 0..cells as children of the (only, non-split) anchor —
    // capture that order once so each ratio check reads cells in spawn order.
    let cell_entities: Vec<Entity> = {
        let mut q = app.world_mut().query::<(&WorldIconBar, &Children)>();
        q.iter(app.world()).next().expect("exactly one Icon anchor").1.iter().collect()
    };
    assert_eq!(cell_entities.len(), 5, "test def uses cells: 5");

    let set_health = |app: &mut App, value: f32| {
        let mut stat_map = app.world_mut().get_mut::<StatMap>(tracked).unwrap();
        let s = stat_map.0.get_mut("health").unwrap();
        s.current = value;
        s.effective = value;
    };
    let filled_cells = |app: &mut App| -> usize {
        cell_entities.iter()
            .filter(|&&e| app.world().get::<Sprite>(e).unwrap()
                .texture_atlas.as_ref().unwrap().index == 12)
            .count()
    };

    // 0% (dead) -> 0 filled — the only ratio where 0 filled cells is correct.
    set_health(&mut app, 0.0);
    app.world_mut().run_system_once(world_icon_bar_update_system).unwrap();
    assert_eq!(filled_cells(&mut app), 0, "exactly 0%% health must show 0 filled cells");

    // 1% (barely alive) -> ceil(0.01*5)=1, floored up to max(1, ..) -> 1 filled, never 0.
    set_health(&mut app, 1.0);
    app.world_mut().run_system_once(world_icon_bar_update_system).unwrap();
    assert_eq!(filled_cells(&mut app), 1, "1%% health must never round down to 0 filled cells");

    // 60% -> ceil(0.6*5)=3 filled.
    set_health(&mut app, 60.0);
    app.world_mut().run_system_once(world_icon_bar_update_system).unwrap();
    assert_eq!(filled_cells(&mut app), 3, "60%% health on a 5-cell bar must show 3 filled cells");

    // 95% -> ceil(0.95*5)=ceil(4.75)=5 filled — full, even though not literally 100%%.
    set_health(&mut app, 95.0);
    app.world_mut().run_system_once(world_icon_bar_update_system).unwrap();
    assert_eq!(filled_cells(&mut app), 5, "95%% health on a 5-cell bar must show full (ceil rounds up)");

    // 100% -> 5 filled (regression: full health must never show fewer than all cells).
    set_health(&mut app, 100.0);
    app.world_mut().run_system_once(world_icon_bar_update_system).unwrap();
    assert_eq!(filled_cells(&mut app), 5, "100%% health must show all 5 cells filled");
}

#[test]
fn test_spawn_world_stat_bar_widget_icon_style_spawns_anchor_and_cells_without_duplication() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().init_resource::<Assets<TextureAtlasLayout>>();
    let catalog = icon_test_catalog();

    let tracked = app.world_mut().spawn(SpawnId("dummy_01".to_string())).id();
    let def = icon_world_stat_bar_def("{self}.health");
    app.world_mut().run_system_once(move |
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
        asset_server: Res<AssetServer>,
    | {
        let mut ctx = StatWidgetSpawnCtx {
            meshes: &mut meshes,
            color_materials: None,
            depth_scale: None,
            is_split_screen: false,
            atlas_layouts: Some(&mut atlas_layouts),
            asset_server: Some(&asset_server),
            asset_catalog: Some(&catalog),
        };
        spawn_world_stat_bar_widget(&mut commands, tracked, "dummy_01.health", &def, &mut ctx);
    }).unwrap();

    let anchor_count = app.world_mut().query::<(&WorldIconBar, &WorldLabel)>().iter(app.world())
        .filter(|(_, l)| l.tracked_entity == Some(tracked)).count();
    assert_eq!(anchor_count, 1, "a non-split ctx must spawn exactly one Icon anchor, no rank overhead");

    let mut q = app.world_mut().query::<&Sprite>();
    let sprite_count = q.iter(app.world()).count();
    assert_eq!(sprite_count, 5, "must spawn exactly `cells` (5) Sprite children, one per cell");
}

/// `world_icon_stat_bar.md`: a split-screen ctx must duplicate the Icon bar's whole
/// anchor+children hierarchy per rank, exactly like Pixel already does, while sharing the
/// texture + TextureAtlasLayout across ranks/cells (only the anchor count and Sprite count
/// scale with rank — the underlying atlas asset does not).
#[test]
fn test_spawn_world_stat_bar_widget_icon_style_duplicates_ranks_when_split_screen() {
    let mut app = setup_test_app();
    app.update();
    app.world_mut().init_resource::<Assets<TextureAtlasLayout>>();
    let catalog = icon_test_catalog();

    let tracked = app.world_mut().spawn(SpawnId("dummy_01".to_string())).id();
    let def = icon_world_stat_bar_def("{self}.health");
    app.world_mut().run_system_once(move |
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
        asset_server: Res<AssetServer>,
    | {
        let mut ctx = StatWidgetSpawnCtx {
            meshes: &mut meshes,
            color_materials: None,
            depth_scale: None,
            is_split_screen: true,
            atlas_layouts: Some(&mut atlas_layouts),
            asset_server: Some(&asset_server),
            asset_catalog: Some(&catalog),
        };
        spawn_world_stat_bar_widget(&mut commands, tracked, "dummy_01.health", &def, &mut ctx);
    }).unwrap();

    let anchor_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&WorldIconBar, &WorldLabel, Option<&WorldLabelRank>)>();
        q.iter(app.world())
            .filter(|(_, l, _)| l.tracked_entity == Some(tracked))
            .map(|(_, _, r)| r.map(|r| r.0))
            .collect()
    };
    assert_eq!(
        anchor_ranks.len(), MAX_SPLIT_PLAYERS as usize,
        "a split-screen ctx must spawn MAX_SPLIT_PLAYERS Icon anchors, same as Pixel"
    );
    let mut sorted: Vec<u8> = anchor_ranks.iter().map(|r| r.unwrap_or(0)).collect();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1, 2, 3], "expected exactly one anchor of each rank 0-3");

    let sprite_count = app.world_mut().query::<&Sprite>().iter(app.world()).count();
    assert_eq!(
        sprite_count, 5 * MAX_SPLIT_PLAYERS as usize,
        "5 cells x MAX_SPLIT_PLAYERS ranks = 20 Sprite children total"
    );

    // Regression guard: the shared TextureAtlasLayout asset must be registered once, not once
    // per rank — only the per-rank/per-cell Sprite entities should scale, not the atlas asset.
    let layout_count = app.world().resource::<Assets<TextureAtlasLayout>>().iter().count();
    assert_eq!(layout_count, 1, "TextureAtlasLayout must be registered once and cloned across ranks/cells");
}

// ── world_textured_stat_bar.md: Textured-style world_stat_bar (9-sliced continuous fill) ──

fn textured_test_catalog() -> AssetCatalog {
    let mut catalog = AssetCatalog::default();
    catalog.textures.insert("healthbar_sheet".to_string(), "shared/ui/rounded-healthbar-texture-sheet.png".to_string());
    catalog
}

fn textured_world_stat_bar_def(stat_key: &str) -> WorldStatBarDef {
    WorldStatBarDef {
        stat_key: stat_key.to_string(),
        offset: (0.0, 2.3, 0.0),
        screen_offset: (0.0, 0.0),
        fill_color: (0.15, 0.85, 0.15, 0.95),
        bg_color: (0.25, 0.08, 0.08, 0.75),
        color_bands: vec![],
        style: WorldStatBarStyle::Textured {
            texture_sheet: "healthbar_sheet".to_string(),
            fill_rect: (0.0, 0.0, 48.0, 17.0),
            empty_rect: (0.0, 17.0, 48.0, 17.0),
            size: (72.0, 14.0),
            slice_border: (8.0, 8.0, 8.0, 8.0),
        },
    }
}

#[test]
fn test_spawn_world_stat_bar_widget_textured_style_spawns_anchor_and_two_sprites_without_duplication() {
    let mut app = setup_test_app();
    app.update();
    let catalog = textured_test_catalog();

    let tracked = app.world_mut().spawn(SpawnId("dummy_01".to_string())).id();
    let def = textured_world_stat_bar_def("{self}.health");
    app.world_mut().run_system_once(move |
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        asset_server: Res<AssetServer>,
    | {
        let mut ctx = StatWidgetSpawnCtx {
            meshes: &mut meshes,
            color_materials: None,
            depth_scale: None,
            is_split_screen: false,
            atlas_layouts: None,
            asset_server: Some(&asset_server),
            asset_catalog: Some(&catalog),
        };
        spawn_world_stat_bar_widget(&mut commands, tracked, "dummy_01.health", &def, &mut ctx);
    }).unwrap();

    let fill_count = app.world_mut().query::<&WorldTexturedBarFillMarker>().iter(app.world()).count();
    assert_eq!(fill_count, 1, "a non-split ctx must spawn exactly one Textured fill entity, no rank overhead");

    let anchor_count = app.world_mut().query::<&WorldLabel>().iter(app.world())
        .filter(|l| l.tracked_entity == Some(tracked)).count();
    assert_eq!(anchor_count, 1, "exactly one anchor WorldLabel must track the entity");

    let sprites: Vec<Entity> = app.world_mut().query::<(Entity, &Sprite)>().iter(app.world())
        .map(|(e, _)| e).collect();
    assert_eq!(sprites.len(), 2, "empty + fill = exactly 2 Sprite children");

    // Both layers must share ONE image handle (one asset_server.load call, cloned) rather than
    // resolving the catalog key twice.
    let images: Vec<_> = sprites.iter()
        .map(|&e| app.world().get::<Sprite>(e).unwrap().image.clone())
        .collect();
    assert_eq!(images[0], images[1], "empty and fill layers must share one cloned Handle<Image>");
}

/// `world_textured_stat_bar.md`: a split-screen ctx must duplicate the Textured bar's whole
/// anchor+children hierarchy per rank, exactly like Pixel/Icon already do, while sharing the one
/// image handle across every layer and every rank (only entity counts scale with rank).
#[test]
fn test_spawn_world_stat_bar_widget_textured_style_duplicates_ranks_when_split_screen() {
    let mut app = setup_test_app();
    app.update();
    let catalog = textured_test_catalog();

    let tracked = app.world_mut().spawn(SpawnId("dummy_01".to_string())).id();
    let def = textured_world_stat_bar_def("{self}.health");
    app.world_mut().run_system_once(move |
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        asset_server: Res<AssetServer>,
    | {
        let mut ctx = StatWidgetSpawnCtx {
            meshes: &mut meshes,
            color_materials: None,
            depth_scale: None,
            is_split_screen: true,
            atlas_layouts: None,
            asset_server: Some(&asset_server),
            asset_catalog: Some(&catalog),
        };
        spawn_world_stat_bar_widget(&mut commands, tracked, "dummy_01.health", &def, &mut ctx);
    }).unwrap();

    let fill_count = app.world_mut().query::<&WorldTexturedBarFillMarker>().iter(app.world()).count();
    assert_eq!(
        fill_count, MAX_SPLIT_PLAYERS as usize,
        "a split-screen ctx must spawn MAX_SPLIT_PLAYERS Textured fill entities, same as Pixel/Icon"
    );

    let anchor_ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&WorldLabel, Option<&WorldLabelRank>)>();
        q.iter(app.world())
            .filter(|(l, _)| l.tracked_entity == Some(tracked))
            .map(|(_, r)| r.map(|r| r.0))
            .collect()
    };
    assert_eq!(
        anchor_ranks.len(), MAX_SPLIT_PLAYERS as usize,
        "must spawn MAX_SPLIT_PLAYERS anchors, same as Pixel/Icon"
    );
    let mut sorted: Vec<u8> = anchor_ranks.iter().map(|r| r.unwrap_or(0)).collect();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1, 2, 3], "expected exactly one anchor of each rank 0-3");

    let sprites: Vec<Entity> = app.world_mut().query::<(Entity, &Sprite)>().iter(app.world())
        .map(|(e, _)| e).collect();
    assert_eq!(
        sprites.len(), 2 * MAX_SPLIT_PLAYERS as usize,
        "2 layers x MAX_SPLIT_PLAYERS ranks = 8 Sprite children total"
    );

    // Regression guard: the one image handle is shared across every layer and rank, not
    // re-resolved per rank — otherwise this would silently multiply asset_server.load calls.
    let images: std::collections::HashSet<_> = sprites.iter()
        .map(|&e| app.world().get::<Sprite>(e).unwrap().image.id())
        .collect();
    assert_eq!(images.len(), 1, "all 8 sprites across every rank must share one Handle<Image>");
}

// Part B: players get first-class stat widgets via the same DynamicStatUiQueue mechanism —
// end-to-end coverage through the real spawn_scene_v2 path (test_player_1 declares
// stat_templates/stat_label/world_stat_bar; test_player_2 declares neither).

fn load_two_player_scene_with_player_stat_widget(app: &mut App) {
    two_player_catalogs(app, None);
    {
        let mut catalog = app.world_mut().resource_mut::<LoadedPrefabCatalog>();
        let p1 = catalog.0.prefabs.get_mut("test_player_1").expect("test_player_1 must exist");
        p1.stat_templates = vec![ironhold_core::schema::stats::StatTemplateDef {
            key: "mana".to_string(),
            base: 40.0,
            min: 0.0,
            max: 100.0,
            regen_rate: 0.0,
            regen_delay: 0.0,
            thresholds: vec![],
        }];
        p1.stat_label = Some(stat_label_def("{self}.mana"));
        p1.world_stat_bar = Some(ascii_world_stat_bar_def("{self}.mana"));
        // test_player_2 deliberately keeps neither field — the "no widget authored, no widget
        // spawned, no warning" baseline case.
    }
    load_two_player_scene(app);
}

#[test]
fn test_player_stat_widget_spawns_and_resolves_against_that_players_own_stat_map() {
    let mut app = setup_test_app();
    app.update();
    load_two_player_scene_with_player_stat_widget(&mut app);

    let p1_entity = *app.world().resource::<ironhold_core::runtime::SpawnRegistry>()
        .entities.get("p1").expect("player 1 must register in SpawnRegistry");
    let p2_entity = *app.world().resource::<ironhold_core::runtime::SpawnRegistry>()
        .entities.get("p2").expect("player 2 must register in SpawnRegistry");

    // {self} must resolve against THIS player's own spawn_id ("p1"), not the literal template.
    let label_query_result: Vec<(String, Entity)> = {
        let mut q = app.world_mut().query::<(&StatLabelMarker, &WorldLabel)>();
        q.iter(app.world()).map(|(m, l)| (m.stat_key.clone(), l.tracked_entity.unwrap())).collect()
    };
    assert_eq!(label_query_result.len(), 1, "only player 1 authored a stat_label — exactly one must spawn");
    assert_eq!(label_query_result[0].0, "p1.mana", "{{self}} must resolve against player 1's own spawn_id");
    assert_eq!(label_query_result[0].1, p1_entity, "the widget must track player 1's entity, not player 2's");

    let bar_fill_count_tracking_p2 = {
        let mut q = app.world_mut().query::<(&WorldStatBarFillMarker, &WorldLabel)>();
        q.iter(app.world()).filter(|(_, l)| l.tracked_entity == Some(p2_entity)).count()
    };
    assert_eq!(bar_fill_count_tracking_p2, 0, "player 2 authored no world_stat_bar — none should track them");

    // Let stat_label_update_system resolve the text against player 1's real StatMap component
    // (built by spawn_player_entity_core from the same stat_templates field).
    app.update();
    let p1_stat_map = app.world().get::<ironhold_core::schema::stats::StatMap>(p1_entity)
        .expect("player 1 must have a StatMap — stat_templates was declared on their prefab");
    assert_eq!(p1_stat_map.0["mana"].current, 40.0);

    let text_query_result: Vec<String> = {
        let mut q = app.world_mut().query::<(&StatLabelMarker, &Text2d)>();
        q.iter(app.world()).map(|(_, t)| t.0.clone()).collect()
    };
    assert_eq!(
        text_query_result, vec!["40 / 100".to_string()],
        "the label must resolve the live value from player 1's own StatMap, proving the full \
         PrefabDef.stat_label -> PlayerConfig -> DynamicStatUiQueue -> resolve_stat pipeline works"
    );
}

#[test]
fn test_player_stat_widget_duplicates_ranks_when_scene_is_split_screen() {
    // Debug-detective finding (2026-07-17): the non-split end-to-end test above proves the
    // player -> DynamicStatUiQueue -> drain pipeline resolves correctly, but drain_dynamic_stat_ui_
    // system's split-screen rank-duplication gate reads ActiveSplitScreen/DynamicSplitConfig,
    // which spawn_players_and_camera sets via DEFERRED commands.insert_resource — a different
    // code path than the non-split test exercises (that path short-circuits before any split
    // resource is touched). This test proves the split-screen case specifically, through the real
    // scene-load pipeline (not by passing is_split_screen: true directly into a ctx, which would
    // bypass the exact resource-timing this is meant to verify).
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app, None,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false }),
    );
    {
        let mut catalog = app.world_mut().resource_mut::<LoadedPrefabCatalog>();
        let p1 = catalog.0.prefabs.get_mut("test_player_1").expect("test_player_1 must exist");
        p1.stat_templates = vec![ironhold_core::schema::stats::StatTemplateDef {
            key: "mana".to_string(), base: 40.0, min: 0.0, max: 100.0,
            regen_rate: 0.0, regen_delay: 0.0, thresholds: vec![],
        }];
        p1.stat_label = Some(stat_label_def("{self}.mana"));
    }
    load_two_player_scene(&mut app);

    let p1_entity = *app.world().resource::<ironhold_core::runtime::SpawnRegistry>()
        .entities.get("p1").expect("player 1 must register in SpawnRegistry");

    let ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&StatLabelMarker, &WorldLabel, Option<&WorldLabelRank>)>();
        q.iter(app.world())
            .filter(|(_, l, _)| l.tracked_entity == Some(p1_entity))
            .map(|(_, _, r)| r.map(|r| r.0))
            .collect()
    };
    assert_eq!(
        ranks.len(), MAX_SPLIT_PLAYERS as usize,
        "a player's stat_label, pushed through DynamicStatUiQueue in a split-screen scene, must \
         spawn MAX_SPLIT_PLAYERS ranked siblings tracking that player — same as any NPC/prop \
         stat_label — proving ActiveSplitScreen/DynamicSplitConfig are actually populated by the \
         time drain_dynamic_stat_ui_system runs for a player-triggered push"
    );
    let mut sorted: Vec<u8> = ranks.iter().map(|r| r.unwrap_or(0)).collect();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1, 2, 3]);
}

// ── per_player_camera_look_controls.md: keyboard camera look (camera_orbit_system) ─────────────

use ironhold_core::capabilities::camera::camera_orbit_system;

#[test]
fn test_keyboard_look_left_rotates_only_the_bound_camera_not_a_sibling() {
    let mut app = setup_test_app();
    app.update();

    let p0 = app.world_mut().spawn((test_character_controller(), Transform::IDENTITY)).id();
    let p1 = app.world_mut().spawn((test_character_controller(), Transform::IDENTITY)).id();

    let cam_with_look = app.world_mut().spawn((
        Transform::IDENTITY,
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState { look_left_key: Some(KeyCode::KeyZ), ..test_orbit_state() }),
        OrbitCameraMode,
        CameraTargets(vec![p0]),
    )).id();
    // Bound to a DIFFERENT key, not left unbound — this is the actual 4-way grid scenario
    // (every scheme's look keys are distinct), a stronger proof than an unbound sibling would be.
    let cam_without_look = app.world_mut().spawn((
        Transform::IDENTITY,
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState { look_left_key: Some(KeyCode::Comma), ..test_orbit_state() }),
        OrbitCameraMode,
        CameraTargets(vec![p1]),
    )).id();

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyZ);

    // `run_system_once` bypasses the schedule, so `Time` never ticks on its own — advance it
    // manually before each run, matching the pattern used elsewhere in this test suite
    // (`action_tests.rs`'s `Time::advance_by`).
    for _ in 0..5 {
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));
        app.world_mut().run_system_once(camera_orbit_system).unwrap();
    }

    let yaw_with_look = get_orbit(&app, cam_with_look).yaw;
    let yaw_without_look = get_orbit(&app, cam_without_look).yaw;
    assert!(
        yaw_with_look > 0.0,
        "look_left held should increase yaw over several ticks, got {yaw_with_look}"
    );
    assert_eq!(
        yaw_without_look, 0.0,
        "a camera bound to a DIFFERENT look_left key (Comma) must be completely unaffected by \
         another player's KeyZ press — per-player independence is the whole point of this feature"
    );
}

#[test]
fn test_keyboard_look_right_decreases_yaw() {
    let mut app = setup_test_app();
    app.update();

    let p0 = app.world_mut().spawn((test_character_controller(), Transform::IDENTITY)).id();
    let cam = app.world_mut().spawn((
        Transform::IDENTITY,
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState { look_right_key: Some(KeyCode::KeyX), ..test_orbit_state() }),
        OrbitCameraMode,
        CameraTargets(vec![p0]),
    )).id();

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyX);
    for _ in 0..5 {
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));
        app.world_mut().run_system_once(camera_orbit_system).unwrap();
    }

    let yaw = get_orbit(&app, cam).yaw;
    assert!(yaw < 0.0, "look_right held should decrease yaw, got {yaw}");
}

#[test]
fn test_keyboard_look_up_increases_pitch_toward_max_and_clamps() {
    let mut app = setup_test_app();
    app.update();

    let p0 = app.world_mut().spawn((test_character_controller(), Transform::IDENTITY)).id();
    let cam = app.world_mut().spawn((
        Transform::IDENTITY,
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState { look_up_key: Some(KeyCode::KeyC), pitch: 0.5, ..test_orbit_state() }),
        OrbitCameraMode,
        CameraTargets(vec![p0]),
    )).id();

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyC);
    // A single large tick (well past what's needed to hit max_pitch at look_speed 2.0 rad/s)
    // clamps in one system run.
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs(5));
    app.world_mut().run_system_once(camera_orbit_system).unwrap();

    let orbit = get_orbit(&app, cam);
    assert!(
        orbit.pitch > 0.5,
        "look_up held should increase pitch toward max_pitch (matching this codebase's mouse \
         convention, not a literal 'up = sky' reading), got {}", orbit.pitch
    );
    assert!(
        (orbit.pitch - orbit.max_pitch).abs() < 0.001,
        "sustained look_up hold must clamp at max_pitch (0.9), got {}", orbit.pitch
    );
}

#[test]
fn test_keyboard_look_down_decreases_pitch_toward_min_and_clamps() {
    let mut app = setup_test_app();
    app.update();

    let p0 = app.world_mut().spawn((test_character_controller(), Transform::IDENTITY)).id();
    let cam = app.world_mut().spawn((
        Transform::IDENTITY,
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState { look_down_key: Some(KeyCode::KeyV), pitch: 0.5, ..test_orbit_state() }),
        OrbitCameraMode,
        CameraTargets(vec![p0]),
    )).id();

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyV);
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs(5));
    app.world_mut().run_system_once(camera_orbit_system).unwrap();

    let orbit = get_orbit(&app, cam);
    assert!(orbit.pitch < 0.5, "look_down held should decrease pitch, got {}", orbit.pitch);
    assert!(
        (orbit.pitch - orbit.min_pitch).abs() < 0.001,
        "sustained look_down hold must clamp at min_pitch (0.1), got {}", orbit.pitch
    );
}

#[test]
fn test_no_look_keys_bound_is_unaffected_by_keyboard_input_regression() {
    // Regression: an OrbitCamera with every look_*_key at None (today's default for every
    // existing scene) must not react to ANY keyboard input at all, matching pre-feature behavior
    // exactly — this is the byte-for-byte backward-compatibility guarantee the plan requires.
    let mut app = setup_test_app();
    app.update();

    let p0 = app.world_mut().spawn((test_character_controller(), Transform::IDENTITY)).id();
    let cam = app.world_mut().spawn((Transform::IDENTITY, test_orbit_camera(p0))).id();

    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::KeyW);
        keys.press(KeyCode::KeyZ);
        keys.press(KeyCode::KeyX);
        keys.press(KeyCode::KeyC);
        keys.press(KeyCode::KeyV);
    }

    for _ in 0..5 {
        app.world_mut().run_system_once(camera_orbit_system).unwrap();
    }

    let orbit = get_orbit(&app, cam);
    assert_eq!(orbit.yaw, 0.0, "no look keys bound -> yaw must stay exactly at its spawned value");
    assert_eq!(orbit.pitch, 0.5, "no look keys bound -> pitch must stay exactly at its spawned value");
}

#[test]
fn test_scene_load_resolves_look_keys_and_look_speed_onto_spawned_split_orbit_camera() {
    // Debug-detective finding: every existing test constructs `OrbitCamera` directly with the
    // look fields already populated, so the RON-string -> component-field resolution at the
    // actual spawn site (`entity_spawner.rs::spawn_orbit_camera_for_player`, the split-screen
    // path) was never exercised — a mis-wire there (e.g. `look_left` resolving into
    // `look_right_key`) would compile and pass every other test. This drives a real scene load
    // through that exact call site instead of spawning a bare component.
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
            ("char_b".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-female-01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));

    let mut p1_camera = base_camera_config();
    p1_camera.split = Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: false });
    p1_camera.look_speed = 3.5;
    let p1_inputs = InputMap { look_left: Some("KeyZ".to_string()), look_up: Some("KeyC".to_string()), ..test_input_map() };

    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("test_player_1".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "char_a".to_string(),
                player_index: 0,
                components: PrefabComponents {
                    tags: vec!["player".to_string()],
                    camera: Some(p1_camera),
                    inputs: Some(p1_inputs),
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

    load_two_player_scene(&mut app);

    // ActiveCameraMode doesn't derive Clone, so snapshot just the fields this test needs.
    struct LookSnapshot {
        look_left_key: Option<KeyCode>,
        look_right_key: Option<KeyCode>,
        look_up_key: Option<KeyCode>,
        look_down_key: Option<KeyCode>,
        look_speed: f32,
    }
    let mut q = app.world_mut().query::<&ActiveCameraMode>();
    let cams: Vec<LookSnapshot> = q.iter(app.world()).filter_map(|mode| {
        let ActiveCameraMode::Orbit(c) = mode else { return None };
        Some(LookSnapshot {
            look_left_key: c.look_left_key, look_right_key: c.look_right_key,
            look_up_key: c.look_up_key, look_down_key: c.look_down_key, look_speed: c.look_speed,
        })
    }).collect();
    assert_eq!(cams.len(), 2, "split-screen scene must spawn one Orbit-mode camera per player");

    let p1_cam = cams.iter().find(|c| c.look_left_key == Some(KeyCode::KeyZ))
        .expect("player 1's camera (the one with a custom look_left) must exist among the spawned cameras");
    assert_eq!(p1_cam.look_left_key, Some(KeyCode::KeyZ), "look_left \"KeyZ\" must resolve onto look_left_key");
    assert_eq!(p1_cam.look_up_key, Some(KeyCode::KeyC), "look_up \"KeyC\" must resolve onto look_up_key");
    assert_eq!(p1_cam.look_right_key, None, "an unauthored look_right must resolve to None, not leak KeyZ");
    assert_eq!(p1_cam.look_down_key, None, "an unauthored look_down must resolve to None");
    assert!(
        (p1_cam.look_speed - 3.5).abs() < 0.001,
        "CameraConfig.look_speed (3.5) must resolve onto OrbitCamera.look_speed, got {}", p1_cam.look_speed
    );

    let p2_cam = cams.iter().find(|c| c.look_left_key.is_none() && (c.look_speed - 2.0).abs() < 0.001)
        .expect("player 2's camera (no inputs authored, default look_speed) must exist");
    assert_eq!(p2_cam.look_right_key, None);
    assert_eq!(p2_cam.look_up_key, None);
    assert_eq!(p2_cam.look_down_key, None);
}

#[test]
fn test_parse_key_recognizes_new_punctuation_set() {
    // Added specifically for the Arrows control scheme's camera-look bindings (Comma/Period sit
    // beside the arrow cluster) — the rest of the row was added alongside for a complete set.
    assert_eq!(InputMap::parse_key("Comma"), Some(KeyCode::Comma));
    assert_eq!(InputMap::parse_key("Period"), Some(KeyCode::Period));
    assert_eq!(InputMap::parse_key("Semicolon"), Some(KeyCode::Semicolon));
    assert_eq!(InputMap::parse_key("Quote"), Some(KeyCode::Quote));
    assert_eq!(InputMap::parse_key("Slash"), Some(KeyCode::Slash));
    assert_eq!(InputMap::parse_key("BracketLeft"), Some(KeyCode::BracketLeft));
    assert_eq!(InputMap::parse_key("BracketRight"), Some(KeyCode::BracketRight));
    assert_eq!(InputMap::parse_key("Minus"), Some(KeyCode::Minus));
    assert_eq!(InputMap::parse_key("Equal"), Some(KeyCode::Equal));
}

// ── player_model_source_unification.md: primitive players through the unified spawn path ───────

fn load_single_entity_scene(app: &mut App, prefab_key: &str, terrain: Option<()>) {
    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let terrain_block = if terrain.is_some() {
        r#"Some((
            heightmap: "projects/terrain_demo/terrain/heightmap.png",
            splatmap: "shared/terrain/splatmap.png",
            scale: (0.5, 30.0, 0.5),
            material_paths: ["shared/terrain/grass.png"],
        ))"#
    } else {
        "None"
    };
    let ron_str = format!(r#"(
        schema_version: 2,
        entities: [
            (id: "p1", prefab: "{prefab_key}", transform: (translation: (0.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
        ],
        ui: [],
        terrain: {terrain_block},
    )"#);
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

fn primitive_player_prefab(player_index: u32, mana_base: f32, material: Option<&str>) -> PrefabDef {
    PrefabDef {
        kind: PrefabKind::Primitive,
        player_index,
        material: material.map(|m| m.to_string()),
        stat_templates: vec![ironhold_core::schema::stats::StatTemplateDef {
            key: "mana".to_string(), base: mana_base, min: 0.0, max: 100.0,
            regen_rate: 0.0, regen_delay: 0.0, thresholds: vec![],
        }],
        components: PrefabComponents { tags: vec!["player".to_string()], ..Default::default() },
        ..Default::default()
    }
}

#[test]
fn test_primitive_player_gets_player_index_stat_map_and_material_like_glb_player() {
    use ironhold_core::runtime::material_factory::PendingMaterialOverride;
    use ironhold_core::schema::stats::StatMap;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog::default()));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("prim_player".to_string(), primitive_player_prefab(3, 42.0, Some("tint_blue"))),
        ]),
        ..Default::default()
    }));

    load_single_entity_scene(&mut app, "prim_player", None);

    let results: Vec<(PlayerIndex, )> = {
        let mut q = app.world_mut().query::<(&CharacterController, &PlayerIndex)>();
        q.iter(app.world()).map(|(_, idx)| (*idx,)).collect()
    };
    assert_eq!(results.len(), 1, "one primitive player must spawn with a CharacterController");
    assert_eq!(
        results[0].0.0, 3,
        "PlayerIndex must be forwarded from the prefab for a primitive player, matching GLB \
         players — this is the exact gap player_model_source_unification.md v1 closes"
    );

    let stat_map_count = app.world_mut().query::<&StatMap>().iter(app.world()).count();
    assert_eq!(
        stat_map_count, 1,
        "a primitive player with `stat_templates` set must get a StatMap, matching GLB players"
    );
    let sm = app.world_mut().query::<&StatMap>().iter(app.world()).next().unwrap();
    assert!(
        (sm.0.get("mana").unwrap().current - 42.0).abs() < 0.01,
        "StatMap's initial value must come from the prefab's stat_templates base"
    );

    let mat_count = app.world_mut().query::<&PendingMaterialOverride>().iter(app.world()).count();
    assert_eq!(
        mat_count, 1,
        "a primitive player with `material` set must get PendingMaterialOverride, matching GLB \
         players — previously the primitive path never read PrefabDef.material at all"
    );
}

#[test]
fn test_two_primitive_players_get_distinct_player_index_and_independent_stat_maps() {
    use ironhold_core::schema::stats::StatMap;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog::default()));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("prim_p1".to_string(), primitive_player_prefab(0, 100.0, None)),
            ("prim_p2".to_string(), primitive_player_prefab(1, 60.0, None)),
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
            (id: "p1", prefab: "prim_p1", transform: (translation: (-2.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
            (id: "p2", prefab: "prim_p2", transform: (translation: (2.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0))),
        ],
        ui: [],
    )"#).unwrap();
    let scene_handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LoadingScene);
    app.update();
    app.update();
    app.update();

    // This is the direct proof the old single-primitive-player structural cap (a bare
    // `Option<(...)>` collector, not a `Vec`) is actually gone, not just asserted in prose.
    let controller_count = app.world_mut().query::<&CharacterController>().iter(app.world()).count();
    assert_eq!(controller_count, 2, "both primitive players must spawn — the structural cap is gone");

    let mut indices: Vec<u32> = {
        let mut q = app.world_mut().query::<&PlayerIndex>();
        q.iter(app.world()).map(|i| i.0).collect()
    };
    indices.sort();
    assert_eq!(indices, vec![0, 1], "each primitive player must get its own prefab's player_index");

    let mut mana_values: Vec<f32> = {
        let mut q = app.world_mut().query::<&StatMap>();
        q.iter(app.world()).map(|sm| sm.0.get("mana").unwrap().current).collect()
    };
    mana_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(
        mana_values, vec![60.0, 100.0],
        "each primitive player must get an independent StatMap matching its own prefab's base value"
    );
}

// ── player_model_source_unification.md v2: mixed GLB + primitive pair (room10) ─────────────────

#[test]
fn test_mixed_glb_and_primitive_players_get_distinct_index_independent_stat_maps_and_split_cameras() {
    use bevy_rapier3d::prelude::{Friction, CoefficientCombineRule};
    use ironhold_core::schema::stats::StatMap;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));

    let mut p1_camera = base_camera_config();
    p1_camera.split = Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None, own_viewport_only: true });

    let mut prim_p2 = primitive_player_prefab(1, 60.0, None);
    prim_p2.display_name = Some("Player 2 (primitive)".to_string());

    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("test_player_1".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "char_a".to_string(),
                player_index: 0,
                stat_templates: vec![ironhold_core::schema::stats::StatTemplateDef {
                    key: "mana".to_string(), base: 100.0, min: 0.0, max: 100.0,
                    regen_rate: 0.0, regen_delay: 0.0, thresholds: vec![],
                }],
                components: PrefabComponents {
                    tags: vec!["player".to_string()],
                    camera: Some(p1_camera),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ("test_player_2".to_string(), prim_p2),
        ]),
        ..Default::default()
    }));

    load_two_player_scene(&mut app);

    // Mirrors room10's shape (a GLB player + a primitive player sharing one split scene), not a
    // load of room10.scene.ron itself — a room10 edit won't be caught here. This only asserts the
    // split-camera count; it does NOT assert own_viewport_only's per-camera RenderLayers, which
    // `test_static_split_own_viewport_only_gives_each_camera_its_own_layer_plus_shared_layer_0`
    // already covers independently of model source (RenderLayers assignment doesn't branch on
    // `PlayerModelSource` at all — see `capabilities/camera.rs`).
    let split_slot_count = app.world_mut().query::<&SplitViewportSlot>().iter(app.world()).count();
    assert_eq!(split_slot_count, 2, "a mixed GLB+primitive pair must still get one split camera per player");

    let mut indices: Vec<u32> = {
        let mut q = app.world_mut().query::<&PlayerIndex>();
        q.iter(app.world()).map(|i| i.0).collect()
    };
    indices.sort();
    assert_eq!(indices, vec![0, 1], "GLB and primitive players must each get their own prefab's player_index");

    let mut mana_values: Vec<f32> = {
        let mut q = app.world_mut().query::<&StatMap>();
        q.iter(app.world()).map(|sm| sm.0.get("mana").unwrap().current).collect()
    };
    mana_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(
        mana_values, vec![60.0, 100.0],
        "GLB and primitive players must each get an independent StatMap, regardless of model source"
    );

    // player_model_source_unification.md v2's Friction reconciliation: both players must carry
    // the SAME Friction coefficient regardless of model source (previously primitive-only, so a
    // GLB player sat at Rapier's default 0.5/`Average` while a primitive player got its own
    // 0.0/`Min`). Asserted per PlayerIndex, not just a world-wide count — a count alone would
    // pass identically for two primitive players and wouldn't actually pin that the *GLB* player
    // (index 0) gained it.
    //
    // Not asserting a specific hardcoded value (originally `0.15` for both): `load_two_player_
    // scene`'s three plain `app.update()` calls don't pin `TimeUpdateStrategy` to zero the way
    // the dedicated physics-behavior test files do, so `player_movement_system` (registered in
    // `FixedUpdate` by `GamePlugin`) can genuinely fire during scene load — confirmed empirically:
    // this assertion failed with `[(0, 0.0), (1, 0.0)]` once the wall-friction velocity-crush fix
    // made the coefficient state-dependent, because the freshly-spawned players hadn't yet
    // registered as grounded by that point. The value is real physics state, not a fixed spawn
    // constant, once `player_movement_system` has had a chance to run at all — so this asserts
    // model-source *parity* (the actual bug this test was written for) and that the shared value
    // is one `player_movement_system` could legitimately have produced, rather than pinning
    // exactly which of the two it happens to be at this specific tick.
    let mut friction_by_index: Vec<(u32, f32)> = {
        let mut q = app.world_mut().query::<(&PlayerIndex, &Friction)>();
        q.iter(app.world()).map(|(idx, f)| (idx.0, f.coefficient)).collect()
    };
    friction_by_index.sort_by_key(|(idx, _)| *idx);
    // Explicit length check before indexing below: a player entity missing `Friction` entirely
    // (the exact regression this test was originally written to catch — a GLB player once shipped
    // with none at all, see the comment above) would silently drop out of the `(&PlayerIndex,
    // &Friction)` query rather than produce a clean value mismatch, since `Option<&mut Friction>`
    // in `player_movement_system`'s own query (added by the wall-friction velocity-crush fix)
    // means a missing `Friction` no longer excludes the entity from *that* system either —
    // debug-detective review finding: the absent-component fallback the `Option` allows is
    // Rapier's own default (0.5/`Average`), worse than this fix's `0.0`, and undiagnosable at
    // runtime, so a regression here is exactly the failure mode worth a clear test message for.
    assert_eq!(
        friction_by_index.len(), 2,
        "both players must have a Friction component at all — one missing it entirely would \
         silently drop out of this query: {friction_by_index:?}"
    );
    assert_eq!(
        friction_by_index[0].1, friction_by_index[1].1,
        "the GLB player (index 0) and the primitive player (index 1) must carry the SAME Friction \
         coefficient, regardless of model source — previously only the primitive player had any \
         Friction at all: {friction_by_index:?}"
    );
    assert!(
        friction_by_index[0].1 == PLAYER_IDLE_FRICTION || friction_by_index[0].1 == 0.0,
        "Friction coefficient must be one of the two values `player_movement_system` ever sets \
         (`PLAYER_IDLE_FRICTION` while grounded-and-idle, `0.0` otherwise): got {:?}",
        friction_by_index[0].1
    );
    let combine_rules_ok = {
        let mut q = app.world_mut().query::<&Friction>();
        q.iter(app.world()).all(|f| matches!(f.combine_rule, CoefficientCombineRule::Min))
    };
    assert!(combine_rules_ok, "every player's Friction must use CoefficientCombineRule::Min");
}

#[test]
fn test_terrain_scene_skips_primitive_player_with_no_crash() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog::default()));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("prim_player".to_string(), primitive_player_prefab(0, 100.0, None)),
        ]),
        ..Default::default()
    }));

    // Must not panic — a primitive player combined with `terrain: Some(...)` is v3-deferred
    // (see player_model_source_unification.md); scene_loader.rs warns and skips it rather than
    // spawning it immediately regardless of terrain (untested territory before this feature).
    load_single_entity_scene(&mut app, "prim_player", Some(()));

    let controller_count = app.world_mut().query::<&CharacterController>().iter(app.world()).count();
    assert_eq!(
        controller_count, 0,
        "a primitive player in a terrain scene must not spawn in v1 — terrain-deferred primitive \
         players are v3 scope, not silently spawned anyway"
    );
}

// ── Scroll-wheel zoom normalization ─────────────────────────────────────────────
//
// Regression for `planning/backlog.md`'s "Scroll-wheel orbit zoom snaps straight to
// min_radius/max_radius instead of stepping gradually" bug, found live during camera_modes.md
// v2's playtest. Root cause: `camera_orbit_system`/`party_camera_follow_system` summed raw
// `MouseWheel.y` with no per-event bound — on platforms/OS configs that report a large "lines"
// magnitude for a single physical click, that one click's delta already exceeded the whole
// min_radius..max_radius range. Fixed via `normalized_wheel_delta` (`capabilities/camera.rs`):
// `Line`-unit events are clamped to ±1.0 per event before being summed; `Pixel`-unit (trackpad)
// events are scaled down instead of clamped, since they're already fine-grained.

fn write_wheel_event(app: &mut App, y: f32, unit: bevy::input::mouse::MouseScrollUnit) {
    app.world_mut()
        .resource_mut::<Messages<bevy::input::mouse::MouseWheel>>()
        .write(bevy::input::mouse::MouseWheel { unit, x: 0.0, y, window: Entity::PLACEHOLDER });
}

#[test]
fn test_camera_orbit_zoom_line_event_clamped_to_one_notch_regardless_of_reported_magnitude() {
    let mut app = setup_test_app();
    app.update();

    let player = app.world_mut().spawn(test_character_controller()).id();
    let cam = app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState {
            zoom_speed: 8.0,
            min_radius: 3.0,
            max_radius: 18.0,
            radius: 10.0,
            ..test_orbit_state()
        }),
        OrbitCameraMode,
        CameraTargets(vec![player]),
    )).id();

    // Simulate the exact failure mode: one physical wheel click, but the OS/driver reports a
    // huge "lines" magnitude for it (observed on Windows with a high scroll-speed setting).
    write_wheel_event(&mut app, 120.0, bevy::input::mouse::MouseScrollUnit::Line);
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
    let dt = app.world().resource::<Time>().delta_secs();
    app.world_mut().run_system_once(camera_orbit_system).unwrap();

    let radius = get_orbit(&app, cam).radius;
    let expected = 10.0 - 1.0 * 8.0 * dt;
    assert!(
        (radius - expected).abs() < 0.001,
        "a single notch (even one OS-reported as y=120) must change radius by exactly \
         1.0 * zoom_speed * dt = {expected}, got {radius}"
    );
    assert!(
        radius > 3.0 && radius < 18.0,
        "one scroll click must not snap to min_radius/max_radius, got {radius}"
    );
}

#[test]
fn test_camera_orbit_zoom_negative_line_event_also_clamped_to_one_notch() {
    let mut app = setup_test_app();
    app.update();

    let player = app.world_mut().spawn(test_character_controller()).id();
    let cam = app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState {
            zoom_speed: 8.0,
            min_radius: 3.0,
            max_radius: 18.0,
            radius: 10.0,
            ..test_orbit_state()
        }),
        OrbitCameraMode,
        CameraTargets(vec![player]),
    )).id();

    write_wheel_event(&mut app, -500.0, bevy::input::mouse::MouseScrollUnit::Line);
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
    let dt = app.world().resource::<Time>().delta_secs();
    app.world_mut().run_system_once(camera_orbit_system).unwrap();

    let radius = get_orbit(&app, cam).radius;
    let expected = 10.0 + 1.0 * 8.0 * dt;
    assert!(
        (radius - expected).abs() < 0.001,
        "a single reversed notch (even one OS-reported as y=-500) must change radius by exactly \
         1.0 * zoom_speed * dt = {expected}, got {radius}"
    );
}

#[test]
fn test_camera_orbit_zoom_sums_multiple_genuine_line_notches_in_one_frame() {
    let mut app = setup_test_app();
    app.update();

    let player = app.world_mut().spawn(test_character_controller()).id();
    let cam = app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState {
            zoom_speed: 8.0,
            min_radius: 3.0,
            max_radius: 18.0,
            radius: 10.0,
            ..test_orbit_state()
        }),
        OrbitCameraMode,
        CameraTargets(vec![player]),
    )).id();

    // Two separate, genuine one-notch events landing in the same frame (fast scrolling) must
    // both count — the per-event clamp must not be mistaken for a per-frame cap.
    write_wheel_event(&mut app, 1.0, bevy::input::mouse::MouseScrollUnit::Line);
    write_wheel_event(&mut app, 1.0, bevy::input::mouse::MouseScrollUnit::Line);
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
    let dt = app.world().resource::<Time>().delta_secs();
    app.world_mut().run_system_once(camera_orbit_system).unwrap();

    let radius = get_orbit(&app, cam).radius;
    let expected = 10.0 - 2.0 * 8.0 * dt;
    assert!(
        (radius - expected).abs() < 0.001,
        "two genuine one-notch events in the same frame must sum to a delta of 2.0, got radius \
         {radius} (expected {expected})"
    );
}

#[test]
fn test_camera_orbit_zoom_pixel_unit_sub_notch_stays_proportionate() {
    let mut app = setup_test_app();
    app.update();

    let player = app.world_mut().spawn(test_character_controller()).id();
    let cam = app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState {
            zoom_speed: 8.0,
            min_radius: 3.0,
            max_radius: 18.0,
            radius: 10.0,
            ..test_orbit_state()
        }),
        OrbitCameraMode,
        CameraTargets(vec![player]),
    )).id();

    // A small sub-notch trackpad delta stays fine-grained and proportionate (this is the case
    // the /SCROLL_PIXELS_PER_LINE division exists for — the sibling test below covers a
    // full-notch-or-larger Pixel event, which the per-event clamp bounds instead).
    write_wheel_event(&mut app, 25.0, bevy::input::mouse::MouseScrollUnit::Pixel);
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
    let dt = app.world().resource::<Time>().delta_secs();
    app.world_mut().run_system_once(camera_orbit_system).unwrap();

    let radius = get_orbit(&app, cam).radius;
    let expected = 10.0 - 0.25 * 8.0 * dt; // 25.0 / 100.0 == 0.25 lines, well under the clamp
    assert!(
        (radius - expected).abs() < 0.001,
        "a 25px sub-notch Pixel swipe must normalize proportionately to 0.25 lines (25/100), \
         got radius {radius} (expected {expected})"
    );
}

#[test]
fn test_camera_orbit_zoom_pixel_unit_full_notch_clamped_regardless_of_dpi_inflation() {
    let mut app = setup_test_app();
    app.update();

    let player = app.world_mut().spawn(test_character_controller()).id();
    let cam = app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState {
            zoom_speed: 8.0,
            min_radius: 3.0,
            max_radius: 18.0,
            radius: 10.0,
            ..test_orbit_state()
        }),
        OrbitCameraMode,
        CameraTargets(vec![player]),
    )).id();

    // Winit's web backend multiplies a Pixel-unit delta by the page's `devicePixelRatio` before
    // Bevy ever sees it, so at 3x display scaling one physical notch can arrive as y=900 (a
    // 100px browser notch * 3x + margin) rather than the ~100 a 1x display would report. Dividing
    // alone would let this scale with DPI; the per-event clamp on the Pixel branch (mirroring
    // Line's) bounds it to exactly one notch-equivalent regardless.
    write_wheel_event(&mut app, 900.0, bevy::input::mouse::MouseScrollUnit::Pixel);
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
    let dt = app.world().resource::<Time>().delta_secs();
    app.world_mut().run_system_once(camera_orbit_system).unwrap();

    let radius = get_orbit(&app, cam).radius;
    let expected = 10.0 - 1.0 * 8.0 * dt; // clamped to 1.0, not 9.0 (900.0 / 100.0)
    assert!(
        (radius - expected).abs() < 0.001,
        "a DPI-inflated single notch (y=900) must clamp to exactly 1.0 line, not scale with \
         DPI, got radius {radius} (expected {expected})"
    );
    assert!(
        radius > 3.0 && radius < 18.0,
        "one physical notch, however the platform reports its magnitude, must not snap to \
         min_radius/max_radius, got {radius}"
    );
}

#[test]
fn test_camera_orbit_zoom_many_full_notch_pixel_events_capped_per_frame() {
    let mut app = setup_test_app();
    app.update();

    let player = app.world_mut().spawn(test_character_controller()).id();
    let cam = app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState {
            zoom_speed: 8.0,
            min_radius: 3.0,
            max_radius: 18.0,
            radius: 10.0,
            ..test_orbit_state()
        }),
        OrbitCameraMode,
        CameraTargets(vec![player]),
    )).id();

    // Each individual event clamps to 1.0 (per the sibling test above), but 5 of them landing in
    // the same frame (a hitched/slow frame batching several notches) would still sum to 5.0
    // without a frame-level backstop — MAX_WHEEL_NOTCHES_PER_FRAME (3.0) is what catches this.
    for _ in 0..5 {
        write_wheel_event(&mut app, 900.0, bevy::input::mouse::MouseScrollUnit::Pixel);
    }
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
    let dt = app.world().resource::<Time>().delta_secs();
    app.world_mut().run_system_once(camera_orbit_system).unwrap();

    let radius = get_orbit(&app, cam).radius;
    let expected = 10.0 - 3.0 * 8.0 * dt; // capped at MAX_WHEEL_NOTCHES_PER_FRAME, not 5.0
    assert!(
        (radius - expected).abs() < 0.001,
        "5 full-notch events in one frame must cap at MAX_WHEEL_NOTCHES_PER_FRAME (3.0), got \
         radius {radius} (expected {expected})"
    );
}

#[test]
fn test_camera_orbit_zoom_fractional_line_delta_stays_proportionate() {
    let mut app = setup_test_app();
    app.update();

    let player = app.world_mut().spawn(test_character_controller()).id();
    let cam = app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState {
            zoom_speed: 8.0,
            min_radius: 3.0,
            max_radius: 18.0,
            radius: 10.0,
            ..test_orbit_state()
        }),
        OrbitCameraMode,
        CameraTargets(vec![player]),
    )).id();

    // winit emits a fractional `LineDelta` for a `WM_MOUSEWHEEL` delta smaller than one full
    // notch (120 raw units) — this must stay 0.25, not get promoted to a full notch. Distinguishes
    // `.clamp(-1.0, 1.0)` (correct) from `.signum()` (a wrong implementation that would still
    // pass every other test in this file, since none of them use a sub-1.0 magnitude).
    write_wheel_event(&mut app, 0.25, bevy::input::mouse::MouseScrollUnit::Line);
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
    let dt = app.world().resource::<Time>().delta_secs();
    app.world_mut().run_system_once(camera_orbit_system).unwrap();

    let radius = get_orbit(&app, cam).radius;
    let expected = 10.0 - 0.25 * 8.0 * dt;
    assert!(
        (radius - expected).abs() < 0.001,
        "a fractional Line delta (0.25) must stay proportionate, not round up to a full notch, \
         got radius {radius} (expected {expected})"
    );
}

#[test]
fn test_camera_orbit_zoom_non_finite_event_ignored_not_poisoning_state() {
    let mut app = setup_test_app();
    app.update();

    let player = app.world_mut().spawn(test_character_controller()).id();
    let cam = app.world_mut().spawn((
        Transform::default(),
        ActiveCameraMode::Orbit(ironhold_core::capabilities::camera::OrbitState {
            zoom_speed: 8.0,
            min_radius: 3.0,
            max_radius: 18.0,
            radius: 10.0,
            ..test_orbit_state()
        }),
        OrbitCameraMode,
        CameraTargets(vec![player]),
    )).id();

    // f32::clamp passes NaN through unchanged, so a malformed event's `y` must be filtered out
    // before clamping/summing — otherwise `OrbitState.radius` becomes NaN permanently (a NaN
    // radius can never be un-set by any later legitimate scroll, since NaN survives every clamp
    // and every arithmetic operation performed on it).
    write_wheel_event(&mut app, f32::NAN, bevy::input::mouse::MouseScrollUnit::Line);
    write_wheel_event(&mut app, 1.0, bevy::input::mouse::MouseScrollUnit::Line);
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
    let dt = app.world().resource::<Time>().delta_secs();
    app.world_mut().run_system_once(camera_orbit_system).unwrap();

    let radius = get_orbit(&app, cam).radius;
    assert!(radius.is_finite(), "a NaN event must not poison OrbitState.radius, got {radius}");
    let expected = 10.0 - 1.0 * 8.0 * dt;
    assert!(
        (radius - expected).abs() < 0.001,
        "the NaN event must be dropped entirely, leaving only the genuine 1.0 notch's effect, \
         got radius {radius} (expected {expected})"
    );
}

#[test]
fn test_party_camera_manual_zoom_line_event_clamped_to_one_notch() {
    let mut app = setup_test_app();
    app.update();

    let p1 = app.world_mut().spawn((test_character_controller(), Transform::from_xyz(-5.0, 0.0, 0.0))).id();
    let p2 = app.world_mut().spawn((test_character_controller(), Transform::from_xyz(5.0, 0.0, 0.0))).id();
    // Separation = 10.0, zoom_margin = 4.0 -> base radius 14.0 before any manual zoom, well
    // inside [4, 20] so a clamp on the derived radius itself can't mask an unclamped offset.
    let camera = app.world_mut().spawn((
        Transform::IDENTITY,
        ActiveCameraMode::Party(ironhold_core::capabilities::camera::PartyState {
            zoom_margin: 4.0,
            allow_manual_zoom: true,
            manual_zoom_offset: 0.0,
            zoom_speed: 10.0,
            orbit_speed: 0.5,
            min_radius: 4.0,
            max_radius: 20.0,
            pitch: 0.5,
            yaw: 0.0,
            look_at_offset: Vec3::ZERO,
            min_pitch: 0.1,
            max_pitch: 0.9,
            orbit_lmb: true,
            orbit_rmb: true,
        }),
        PartyCameraMode,
        CameraTargets(vec![p1, p2]),
    )).id();

    write_wheel_event(&mut app, 300.0, bevy::input::mouse::MouseScrollUnit::Line);
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
    let dt = app.world().resource::<Time>().delta_secs();
    app.world_mut().run_system_once(party_camera_follow_system).unwrap();

    let cam_transform = app.world().get::<Transform>(camera).unwrap();
    let actual_radius = cam_transform.translation.distance(Vec3::ZERO);
    let expected = 14.0 - 1.0 * 10.0 * dt;
    assert!(
        (actual_radius - expected).abs() < 0.01,
        "a single manual-zoom notch (even one OS-reported as y=300) must move the derived \
         radius by exactly 1.0 * zoom_speed * dt from 14.0, got {actual_radius} (expected \
         {expected})"
    );
}

#[test]
fn test_party_camera_manual_zoom_offset_does_not_bank_past_the_radius_clamp() {
    let mut app = setup_test_app();
    app.update();

    let p1 = app.world_mut().spawn((test_character_controller(), Transform::from_xyz(-5.0, 0.0, 0.0))).id();
    let p2 = app.world_mut().spawn((test_character_controller(), Transform::from_xyz(5.0, 0.0, 0.0))).id();
    // Separation = 10.0, zoom_margin = 4.0 -> base radius (before offset) 14.0.
    let camera = app.world_mut().spawn((
        Transform::IDENTITY,
        ActiveCameraMode::Party(ironhold_core::capabilities::camera::PartyState {
            zoom_margin: 4.0,
            allow_manual_zoom: true,
            manual_zoom_offset: -50.0,
            zoom_speed: 10.0,
            orbit_speed: 0.5,
            min_radius: 4.0,
            max_radius: 20.0,
            pitch: 0.5,
            yaw: 0.0,
            look_at_offset: Vec3::ZERO,
            min_pitch: 0.1,
            max_pitch: 0.9,
            orbit_lmb: true,
            orbit_rmb: true,
        }),
        PartyCameraMode,
        CameraTargets(vec![p1, p2]),
    )).id();

    // `manual_zoom_offset` starts at -50.0 — simulating the pre-fix bug's end state after many
    // scroll notches accumulated well past what the radius clamp could ever use (10.0 max_dist +
    // 4.0 margin - 50.0 offset would derive a radius of -36.0, clamped to min_radius). A frame
    // with no wheel input at all must still re-derive the offset down to exactly what the clamped
    // radius used (-10.0), not leave the -50.0 "dead reserve" sitting there.
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
    app.world_mut().run_system_once(party_camera_follow_system).unwrap();

    let radius_at_min = app.world().get::<Transform>(camera).unwrap().translation.distance(Vec3::ZERO);
    assert!((radius_at_min - 4.0).abs() < 0.01, "expected radius clamped to min_radius (4.0), got {radius_at_min}");
    let offset_after_rebank = match app.world().get::<ActiveCameraMode>(camera).unwrap() {
        ActiveCameraMode::Party(p) => p.manual_zoom_offset,
        _ => panic!("expected a Party-mode camera"),
    };
    assert!(
        (offset_after_rebank - (-10.0)).abs() < 0.01,
        "manual_zoom_offset must be re-derived down to exactly what the clamped radius used \
         (-10.0 = 4.0 - 10.0 - 4.0), not left at the stale -50.0, got {offset_after_rebank}"
    );

    // One single reversed notch must move the camera immediately — if the offset had stayed
    // banked past what the clamp used, this first reversed notch would still read back radius
    // == 4.0 instead of visibly responding.
    write_wheel_event(&mut app, -1.0, bevy::input::mouse::MouseScrollUnit::Line);
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
    let dt = app.world().resource::<Time>().delta_secs();
    app.world_mut().run_system_once(party_camera_follow_system).unwrap();

    let radius_after_one_reversed_notch = app.world().get::<Transform>(camera).unwrap().translation.distance(Vec3::ZERO);
    let expected = 4.0 + 1.0 * 10.0 * dt;
    assert!(
        (radius_after_one_reversed_notch - expected).abs() < 0.01,
        "manual_zoom_offset must not bank a hidden reserve past what the radius clamp used — a \
         single reversed notch after hitting min_radius must move the camera immediately, got \
         {radius_after_one_reversed_notch} (expected {expected})"
    );
}
