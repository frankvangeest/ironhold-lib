use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::camera::Viewport;
use bevy::math::Mat4;
use bevy_rapier3d::prelude::{Velocity, CollisionEvent};
use bevy_rapier3d::rapier::geometry::CollisionEventFlags;
use bevy::window::PrimaryWindow;
use ironhold_core::runtime::{SceneHandleV2, LoadedAssetCatalog, LoadedPrefabCatalog, ActiveViewBox, ActiveSplitScreen, DynamicSplitConfig, ActiveSplitSlotCount, GameEvent, SpawnRegistry};
use ironhold_core::runtime::scene_manager::{WorldLabel, WorldLabelRank, SpawnId};
use ironhold_core::capabilities::targeting::ClickSelectable;
use ironhold_core::capabilities::action_bar::CurrentTarget;
use ironhold_core::capabilities::stat_display::{
    StatLabelMarker, WorldStatBarFillMarker, WorldPixelBarFillMarker,
    StatWidgetSpawnCtx, spawn_stat_label_widget, spawn_world_stat_bar_widget,
};
use ironhold_core::schema::{AppState, ProjectConfig, ProjectConfigHandle, GameSceneV2};
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, PrefabKind, ModelCatalogEntry, PrefabComponents, StatLabelDef, WorldStatBarDef, WorldStatBarStyle};
use ironhold_core::schema::player::{CameraConfig, PartyZoomDef, SplitScreenDef, SplitOrientation, DynamicSplitDef, InputMap};
use ironhold_core::capabilities::player::{CharacterController, PlayerIndex, PlayerTarget, player_view_box_clamp_system};
use ironhold_core::capabilities::camera::{
    OrbitCamera, PartyOrbitCamera, party_camera_follow_system,
    SplitViewportSlot, split_screen_viewport_system, dynamic_split_screen_system, parse_orbit_button,
    MAX_SPLIT_PLAYERS, SplitScreenPlayerLabel, LinkedPlayerLabel, PLAYER_LABEL_COLORS,
    split_viewport_player_label_spawn_system, split_viewport_player_label_update_system,
};
use ironhold_core::capabilities::trigger_zone::{TriggerZone, TriggerZoneId, trigger_zone_system};
use ironhold_core::GameVariables;

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
        font_size: 16.0,
        color: (0.2, 0.9, 0.2, 1.0),
        show_max: true,
    }
}

fn ascii_world_stat_bar_def(stat_key: &str) -> WorldStatBarDef {
    WorldStatBarDef {
        stat_key: stat_key.to_string(),
        offset: (0.0, 2.8, 0.0),
        fill_color: (0.15, 0.85, 0.15, 0.95),
        bg_color: (0.25, 0.08, 0.08, 0.75),
        color_bands: vec![],
        style: WorldStatBarStyle::Ascii { cells: 10, font_size: 14.0 },
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
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None }),
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
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None }),
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

// ── Per-viewport target HUD readout (Phase 1, per_player_split_screen_targeting.md) ─────

fn load_two_player_scene_with_target_hud(app: &mut App, author_target_hud: bool) {
    two_player_catalogs_with_split(
        app, None,
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None }),
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
        let mut q = app.world_mut().query::<(&OrbitCamera, &LinkedTargetHud)>();
        q.iter(app.world()).map(|(o, l)| (o.target, l.0)).collect()
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
        do_actions: vec![Action::SetVariable("p1_fired".to_string(), "yes".to_string())],
        cooldown_secs: None,
        cost: Some(SlotCost { stat: "mana".to_string(), amount: 20.0 }),
        owner_player: Some(0),
    });
    app.world_mut().spawn(ActionSlotUi {
        slot_key: "2".to_string(),
        resolved_key: Some(KeyCode::Digit2),
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
        };
        spawn_world_stat_bar_widget(&mut commands, tracked, "dummy_01.health", &def, &mut ctx);
    }).unwrap();

    let fill_count = app.world_mut().query::<&WorldPixelBarFillMarker>().iter(app.world()).count();
    assert_eq!(fill_count, 1, "exactly one Pixel fill entity must spawn — Pixel bars do not rank-duplicate");

    // Anchor + border + bg + fill = 4 entities total tracking the entity via WorldLabel (the
    // anchor is the only one with a WorldLabel; border/bg/fill are its Bevy-hierarchy children).
    let anchor_count = app.world_mut().query::<&WorldLabel>().iter(app.world())
        .filter(|l| l.tracked_entity == Some(tracked)).count();
    assert_eq!(anchor_count, 1, "exactly one anchor WorldLabel must track the entity");

    let mut q = app.world_mut().query::<&ChildOf>();
    let child_count = q.iter(app.world()).count();
    assert!(child_count >= 3, "border + background + fill must all be spawned as children of the anchor");
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
        Some(SplitScreenDef { orientation: SplitOrientation::Vertical, dynamic: None }),
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
