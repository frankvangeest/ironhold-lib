use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy_rapier3d::prelude::{Velocity, CollisionEvent};
use bevy_rapier3d::rapier::geometry::CollisionEventFlags;
use ironhold_core::runtime::{SceneHandleV2, LoadedAssetCatalog, LoadedPrefabCatalog, ActiveViewBox, GameEvent};
use ironhold_core::schema::{AppState, ProjectConfig, ProjectConfigHandle, GameSceneV2};
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, PrefabKind, ModelCatalogEntry, PrefabComponents};
use ironhold_core::schema::player::{CameraConfig, PartyZoomDef, InputMap};
use ironhold_core::capabilities::player::{CharacterController, player_view_box_clamp_system};
use ironhold_core::capabilities::camera::{OrbitCamera, PartyOrbitCamera, party_camera_follow_system};
use ironhold_core::capabilities::trigger_zone::{TriggerZone, TriggerZoneId, trigger_zone_system};

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
        gamepad_index: None,
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
        PartyOrbitCamera {
            targets: vec![p1, p2],
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
        },
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
        PartyOrbitCamera {
            targets: vec![p1, p2],
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
        },
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
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("char_a".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-male-01.glb#Scene0".to_string() }),
            ("char_b".to_string(), ModelCatalogEntry { path: "shared/models/characters/character-female-01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));

    let mut p1_camera = base_camera_config();
    p1_camera.party = party;

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
fn test_two_players_spawn_with_shared_party_camera() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs(&mut app, Some(PartyZoomDef { zoom_margin: 4.0, allow_manual_zoom: false }));
    load_two_player_scene(&mut app);

    let controller_count = app.world_mut().query::<&CharacterController>().iter(app.world()).count();
    assert_eq!(controller_count, 2, "both player-tagged entities must spawn a CharacterController");

    let party_cam_count = app.world_mut().query::<&PartyOrbitCamera>().iter(app.world()).count();
    assert_eq!(party_cam_count, 1, "exactly one shared PartyOrbitCamera must spawn when `party` is configured");

    let solo_cam_count = app.world_mut().query::<&OrbitCamera>().iter(app.world()).count();
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

    let solo_cam_count = app.world_mut().query::<&OrbitCamera>().iter(app.world()).count();
    assert_eq!(
        solo_cam_count, 1,
        "missing `party` on a 2-player scene must fall back to exactly one OrbitCamera, \
         never two silently-competing per-player cameras"
    );

    let party_cam_count = app.world_mut().query::<&PartyOrbitCamera>().iter(app.world()).count();
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
