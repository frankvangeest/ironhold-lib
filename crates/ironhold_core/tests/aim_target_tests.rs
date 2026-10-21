//! `FirstPersonCameraDef.aim_target` (`planning/features/first_person_starter.md`, v2): the crosshair, not the
//! mouse cursor, decides the player's `PlayerTarget`.
//!
//! The test app has a primary `Window` (the aim system falls back to its size when a camera has no viewport yet),
//! the real Rapier world (line of sight needs colliders and a stepped query pipeline) and a clock pinned to one
//! 64 Hz tick per `app.update()`. The camera sits at the player's eye (0, 1.6, 0) looking down -Z; a target's aim
//! point is its origin + 1.0 m (the default `SelectAimHeight`), so a target at y = 0.6 is dead centre.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;
use bevy_rapier3d::prelude::*;
use std::time::Duration;

use ironhold_core::capabilities::camera::{
    ActiveCameraMode, CameraTargets, FirstPersonCameraMode, FirstPersonState,
};
use ironhold_core::capabilities::player::{CharacterController, PlayerTarget};
use ironhold_core::capabilities::targeting::{project_to_viewport, Targetable};
use ironhold_core::runtime::scene_manager::SpawnRegistry;
use ironhold_core::runtime::SpawnId;
use ironhold_core::schema::camera::AimTargetDef;
use ironhold_core::schema::player::InputMap;
use ironhold_core::GameVariables;

mod support;
use support::setup_test_app;

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

fn fp_state(aim_target: Option<AimTargetDef>) -> FirstPersonState {
    FirstPersonState {
        eye_offset: Vec3::new(0.0, 1.6, 0.0),
        sensitivity: 0.002,
        pitch: 0.0,
        min_pitch: -1.4,
        max_pitch: 1.4,
        look_speed: 2.5,
        invert_y: false,
        capture_cursor: false,
        hide_own_body: false,
        aim_target,
    }
}

/// A player and its first-person camera. Returns `(app, player)`.
fn aim_app(aim_target: Option<AimTargetDef>) -> (App, Entity) {
    let mut app = setup_test_app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 64.0)));
    app.update();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    let player = app
        .world_mut()
        .spawn((Transform::default(), controller(), PlayerTarget::default()))
        .id();
    app.world_mut().spawn((
        Transform::from_xyz(0.0, 1.6, 0.0),
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 90.0_f32.to_radians(),
            ..default()
        }),
        ActiveCameraMode::FirstPerson(fp_state(aim_target)),
        CameraTargets(vec![player]),
        FirstPersonCameraMode,
    ));
    settle(&mut app);
    (app, player)
}

fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
}

/// Spawns a targetable and registers its id like the scene loader does: `target_auto_clear_system` clears any
/// target whose id is missing from the `SpawnRegistry`, so an unregistered one would be cleared every frame.
fn register(app: &mut App, id: &str, entity: Entity) {
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert(id.to_string(), entity);
}

fn spawn_target(app: &mut App, id: &str, at: Vec3) -> Entity {
    let e = app.world_mut().spawn((Targetable, SpawnId(id.to_string()), Transform::from_translation(at))).id();
    register(app, id, e);
    e
}

fn target_of(app: &App, player: Entity) -> Option<String> {
    app.world().get::<PlayerTarget>(player).unwrap().0.clone()
}

/// A target 10 m straight ahead, dead centre.
const AHEAD: Vec3 = Vec3::new(0.0, 0.6, -10.0);

// ── selection ──────────────────────────────────────────────────────────────────────────────

#[test]
fn the_target_under_the_crosshair_becomes_the_players_target() {
    let (mut app, player) = aim_app(Some(AimTargetDef::default()));
    spawn_target(&mut app, "dummy", AHEAD);
    settle(&mut app);
    assert_eq!(target_of(&app, player).as_deref(), Some("dummy"));
    assert_eq!(
        app.world().resource::<GameVariables>().0.get("target_id").map(String::as_str),
        Some("dummy"),
        "the target_* variables follow the crosshair like any other selection"
    );
}

#[test]
fn only_targets_inside_the_angular_catch_circle_count() {
    let (mut app, player) = aim_app(Some(AimTargetDef::default())); // radius_deg 3.0: 10 m ahead is about 0.52 m
    let off = spawn_target(&mut app, "off", AHEAD + Vec3::new(2.0, 0.0, 0.0));
    settle(&mut app);
    assert_eq!(target_of(&app, player), None, "2 m off the view axis at 10 m is about 11 degrees, outside 3");
    app.world_mut().entity_mut(off).insert(Transform::from_translation(AHEAD + Vec3::new(0.4, 0.0, 0.0)));
    settle(&mut app);
    assert_eq!(target_of(&app, player).as_deref(), Some("off"), "0.4 m off at 10 m is about 2.3 degrees, inside 3");
}

#[test]
fn a_wider_radius_catches_what_the_default_misses() {
    let (mut app, player) = aim_app(Some(AimTargetDef { radius_deg: 15.0, ..default() }));
    spawn_target(&mut app, "wide", AHEAD + Vec3::new(2.0, 0.0, 0.0));
    settle(&mut app);
    assert_eq!(target_of(&app, player).as_deref(), Some("wide"));
}

#[test]
fn targets_beyond_range_or_behind_the_camera_are_ignored() {
    let (mut app, player) = aim_app(Some(AimTargetDef { range: 20.0, ..default() }));
    spawn_target(&mut app, "far", Vec3::new(0.0, 0.6, -30.0));
    spawn_target(&mut app, "behind", Vec3::new(0.0, 0.6, 8.0));
    settle(&mut app);
    assert_eq!(target_of(&app, player), None);
}

#[test]
fn the_target_nearest_the_centre_wins_and_ties_break_by_spawn_id() {
    let (mut app, player) = aim_app(Some(AimTargetDef { radius_deg: 10.0, ..default() }));
    spawn_target(&mut app, "near", AHEAD + Vec3::new(0.3, 0.0, 0.0));
    spawn_target(&mut app, "farther", AHEAD + Vec3::new(0.9, 0.0, 0.0));
    settle(&mut app);
    assert_eq!(target_of(&app, player).as_deref(), Some("near"));

    // Mirror-image targets are the same distance from the centre: SpawnId order decides, not entity order.
    let (mut app, player) = aim_app(Some(AimTargetDef { radius_deg: 10.0, ..default() }));
    spawn_target(&mut app, "b", AHEAD + Vec3::new(0.4, 0.0, 0.0));
    spawn_target(&mut app, "a", AHEAD + Vec3::new(-0.4, 0.0, 0.0));
    settle(&mut app);
    assert_eq!(target_of(&app, player).as_deref(), Some("a"), "equal distance: the smaller SpawnId, whatever the spawn order");
}

#[test]
fn the_target_is_cleared_when_it_leaves_the_crosshair_and_hidden_targets_are_skipped() {
    let (mut app, player) = aim_app(Some(AimTargetDef::default()));
    let t = spawn_target(&mut app, "dummy", AHEAD);
    settle(&mut app);
    assert_eq!(target_of(&app, player).as_deref(), Some("dummy"));

    app.world_mut().entity_mut(t).insert(Transform::from_translation(AHEAD + Vec3::new(5.0, 0.0, 0.0)));
    settle(&mut app);
    assert_eq!(target_of(&app, player), None, "no longer under the crosshair: cleared");

    app.world_mut().entity_mut(t).insert((Transform::from_translation(AHEAD), Visibility::Hidden));
    settle(&mut app);
    assert_eq!(target_of(&app, player), None, "a hidden (dead) target cannot be aimed at");
}

#[test]
fn a_camera_without_aim_target_leaves_the_players_target_alone() {
    let (mut app, player) = aim_app(None);
    spawn_target(&mut app, "dummy", AHEAD);
    spawn_target(&mut app, "other", AHEAD + Vec3::new(8.0, 0.0, 0.0));
    app.world_mut().get_mut::<PlayerTarget>(player).unwrap().0 = Some("other".to_string());
    settle(&mut app);
    assert_eq!(target_of(&app, player).as_deref(), Some("other"), "no aim_target: the crosshair target ('dummy') is not applied and 'other' stays");
}

// ── line of sight (real Rapier colliders) ──────────────────────────────────────────────────

fn spawn_collider_target(app: &mut App, id: &str, at: Vec3) {
    let e = app
        .world_mut()
        .spawn((
            Targetable,
            SpawnId(id.to_string()),
            Transform::from_translation(at),
            RigidBody::Fixed,
            Collider::capsule_y(0.5, 0.4),
        ))
        .id();
    register(app, id, e);
}

fn spawn_wall(app: &mut App, at: Vec3) {
    app.world_mut().spawn((RigidBody::Fixed, Collider::cuboid(4.0, 4.0, 0.2), Transform::from_translation(at)));
}

#[test]
fn a_wall_between_the_crosshair_and_the_target_blocks_it() {
    let (mut app, player) = aim_app(Some(AimTargetDef::default()));
    spawn_collider_target(&mut app, "dummy", AHEAD);
    spawn_wall(&mut app, Vec3::new(0.0, 2.0, -5.0));
    settle(&mut app);
    assert_eq!(target_of(&app, player), None, "a wall in the way: not targetable through it");
}

#[test]
fn an_unobstructed_collider_target_is_reached_by_the_ray() {
    let (mut app, player) = aim_app(Some(AimTargetDef::default()));
    spawn_collider_target(&mut app, "dummy", AHEAD);
    settle(&mut app);
    assert_eq!(target_of(&app, player).as_deref(), Some("dummy"), "the ray hits the target itself, which counts as clear");
}

#[test]
fn line_of_sight_can_be_switched_off() {
    let (mut app, player) = aim_app(Some(AimTargetDef { require_line_of_sight: false, ..default() }));
    spawn_collider_target(&mut app, "dummy", AHEAD);
    spawn_wall(&mut app, Vec3::new(0.0, 2.0, -5.0));
    settle(&mut app);
    assert_eq!(target_of(&app, player).as_deref(), Some("dummy"), "require_line_of_sight: false aims through walls");
}

#[test]
fn a_blocked_nearest_target_falls_back_to_the_next_one() {
    let (mut app, player) = aim_app(Some(AimTargetDef { radius_deg: 10.0, ..default() }));
    spawn_collider_target(&mut app, "hidden_behind_wall", AHEAD);
    spawn_wall(&mut app, Vec3::new(0.0, 2.0, -5.0));
    // Slightly off-centre and in front of the wall, so it is clear.
    spawn_collider_target(&mut app, "in_front", Vec3::new(0.4, 0.6, -3.0));
    settle(&mut app);
    assert_eq!(target_of(&app, player).as_deref(), Some("in_front"));
}

// ── Tab selection is off for a crosshair-targeted player ───────────────────────────────────

fn press_tab(app: &mut App) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Tab);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::Tab);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
}

#[test]
fn tab_does_not_cycle_a_crosshair_targeted_player_but_still_cycles_others() {
    // The aiming player: a Tab press with a target in range (but NOT under the crosshair) must do nothing.
    let (mut app, player) = aim_app(Some(AimTargetDef::default()));
    app.world_mut().entity_mut(player).insert(GlobalTransform::default());
    spawn_target(&mut app, "side", Vec3::new(6.0, 0.6, -3.0));
    settle(&mut app);
    assert_eq!(target_of(&app, player), None, "precondition: not under the crosshair");
    press_tab(&mut app);
    settle(&mut app);
    assert_eq!(target_of(&app, player), None, "Tab is off while aim_target drives the player");

    // Control: the same scene with a camera that has no aim_target cycles on Tab.
    let (mut app, player) = aim_app(None);
    app.world_mut().entity_mut(player).insert(GlobalTransform::default());
    spawn_target(&mut app, "side", Vec3::new(6.0, 0.6, -3.0));
    settle(&mut app);
    press_tab(&mut app);
    assert_eq!(target_of(&app, player).as_deref(), Some("side"), "control: Tab cycles without aim_target");
}

// ── the pure projection ─────────────────────────────────────────────────────────────────────

#[test]
fn project_to_viewport_matches_a_pinhole_camera() {
    let view = GlobalTransform::default(); // at the origin looking down -Z
    let size = Vec2::new(800.0, 600.0);
    let fov = 90.0_f32.to_radians();
    let centre = project_to_viewport(&view, fov, size, Vec3::new(0.0, 0.0, -10.0)).unwrap();
    assert!((centre - Vec2::new(400.0, 300.0)).length() < 1e-3, "straight ahead is the viewport centre, got {centre}");
    // 90 degree vertical fov: a point 10 m ahead and 10 m up sits on the top edge.
    let top = project_to_viewport(&view, fov, size, Vec3::new(0.0, 10.0, -10.0)).unwrap();
    assert!((top.y - 0.0).abs() < 1e-3 && (top.x - 400.0).abs() < 1e-3, "got {top}");
    // To the right is +x on screen.
    let right = project_to_viewport(&view, fov, size, Vec3::new(1.0, 0.0, -10.0)).unwrap();
    assert!(right.x > 400.0 && (right.y - 300.0).abs() < 1e-3);
    assert!(project_to_viewport(&view, fov, size, Vec3::new(0.0, 0.0, 5.0)).is_none(), "behind the camera");
}
