use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::camera::Viewport;
use bevy_rapier3d::prelude::{Velocity, CollisionEvent};
use bevy_rapier3d::rapier::geometry::CollisionEventFlags;
use bevy::window::PrimaryWindow;
use ironhold_core::runtime::{SceneHandleV2, LoadedAssetCatalog, LoadedPrefabCatalog, ActiveViewBox, ActiveSplitScreen, DynamicSplitConfig, ActiveSplitSlotCount, GameEvent};
use ironhold_core::schema::{AppState, ProjectConfig, ProjectConfigHandle, GameSceneV2};
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, PrefabKind, ModelCatalogEntry, PrefabComponents};
use ironhold_core::schema::player::{CameraConfig, PartyZoomDef, SplitScreenDef, SplitOrientation, DynamicSplitDef, InputMap};
use ironhold_core::capabilities::player::{CharacterController, PlayerIndex, player_view_box_clamp_system};
use ironhold_core::capabilities::camera::{
    OrbitCamera, PartyOrbitCamera, party_camera_follow_system,
    SplitViewportSlot, split_screen_viewport_system, dynamic_split_screen_system, parse_orbit_button,
    MAX_SPLIT_PLAYERS, SplitScreenPlayerLabel, LinkedPlayerLabel, PLAYER_LABEL_COLORS,
    split_viewport_player_label_spawn_system, split_viewport_player_label_update_system,
};
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
        split: None,
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
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None }),
    );
    load_two_player_scene(&mut app);

    let controller_count = app.world_mut().query::<&CharacterController>().iter(app.world()).count();
    assert_eq!(controller_count, 2, "both players still spawn regardless of the conflicting config");

    let party_cam_count = app.world_mut().query::<&PartyOrbitCamera>().iter(app.world()).count();
    assert_eq!(party_cam_count, 0, "party must NOT spawn when split is also set");

    let split_slot_count = app.world_mut().query::<&SplitViewportSlot>().iter(app.world()).count();
    assert_eq!(split_slot_count, 2, "split wins: one SplitViewportSlot camera per player");

    let orbit_cam_count = app.world_mut().query::<&OrbitCamera>().iter(app.world()).count();
    assert_eq!(orbit_cam_count, 2, "split spawns two real OrbitCameras, not a fallback single one");
}

#[test]
fn test_split_only_spawns_two_orbit_cameras_with_viewport_slots() {
    let mut app = setup_test_app();
    app.update();
    two_player_catalogs_with_split(
        &mut app,
        None,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None }),
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
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None }),
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
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None }),
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
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None }),
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

// ── Stage 5: dynamic_split_screen_system (unit-level) ───────────────────────────

fn test_orbit_camera(target: Entity) -> OrbitCamera {
    OrbitCamera {
        target,
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
    }
}

fn test_party_orbit_camera(targets: Vec<Entity>) -> PartyOrbitCamera {
    PartyOrbitCamera {
        targets,
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
    }
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
        }),
    );
    load_two_player_scene(&mut app);

    assert_eq!(app.world().resource::<ActiveSplitScreen>().0, Some(SplitOrientation::Vertical), "8.0 > split_distance 5.0 -> must start split");

    let party_active: Vec<bool> = {
        let mut q = app.world_mut().query_filtered::<&Camera, With<PartyOrbitCamera>>();
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
        }),
    );
    load_two_player_scene(&mut app);

    assert_eq!(app.world().resource::<ActiveSplitScreen>().0, None, "8.0 < split_distance 12.0 -> must start merged");

    let party_active: Vec<bool> = {
        let mut q = app.world_mut().query_filtered::<&Camera, With<PartyOrbitCamera>>();
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
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None }),
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
        Some(SplitScreenDef { orientation: SplitOrientation::Grid, dynamic: None }),
    );
    load_n_player_scene(&mut app, 4);
    app.update();

    let cams: Vec<(Entity, Entity)> = {
        let mut q = app.world_mut().query::<(&OrbitCamera, &LinkedPlayerLabel)>();
        q.iter(app.world()).map(|(o, l)| (o.target, l.0)).collect()
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
