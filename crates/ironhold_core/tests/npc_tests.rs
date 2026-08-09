use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use ironhold_core::runtime::{ActionQueue, SpawnId};
use ironhold_core::schema::Action;
use ironhold_core::capabilities::player::{CharacterController, SpeedMultiplier};

mod support;
use support::setup_test_app;

fn npc_aggro_test_player_controller() -> CharacterController {
    use ironhold_core::schema::player::InputMap;
    CharacterController {
        walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
        inputs: InputMap {
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
        },
        is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
        double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
        collider_radius: 0.4, ground_cast_length: 0.3, idle_drag: 0.8,
    }
}

fn npc_aggro_test_npc_agent(id: &str, on_player_near: ironhold_core::schema::catalog::NpcOnPlayerNear, pos: Vec3) -> ironhold_core::capabilities::NpcAgent {
    use ironhold_core::schema::catalog::NpcFaction;
    ironhold_core::capabilities::NpcAgent {
        npc_id: id.to_string(),
        faction: NpcFaction::Hostile,
        on_player_near,
        detection_radius: 8.0,
        chase_radius: 20.0,
        fov_cos: -1.0,
        requires_los: false,
        approach_distance: 2.0,
        patrol_speed: 2.0,
        chase_speed: 4.0,
        waypoints: vec![],
        current_waypoint: 0,
        state: ironhold_core::capabilities::NpcState::Idle,
        target: None,
        state_timer: 0.0,
        origin: pos,
        eye_height: 1.0,
        alerted_duration: 0.3,
        drag: 0.8,
        waypoint_reach_radius: 0.5,
        interact_leave_factor: 1.5,
        home_arrival_radius: 0.5,
        investigate_timeout_secs: 5.0,
        waypoint_wait_secs: 0.0,
        waypoint_wait_timer: 0.0,
        last_known_attacker_pos: None,
        investigate_timer: 0.0,
    }
}

/// Chase-faction NPC in Idle, outside detection radius → receives hit event → transitions to
/// Investigating so it walks toward the attacker's last-known position.
#[test]
fn test_npc_aggro_on_hit_idle_to_investigating() {
    use ironhold_core::capabilities::{NpcAgent, NpcState, npc_behavior_system};
    use ironhold_core::capabilities::npc::NpcHitQueue;
    use ironhold_core::schema::catalog::NpcOnPlayerNear;
    use bevy_rapier3d::prelude::Velocity;

    let mut app = setup_test_app();
    app.update();

    // Player at origin — 50 m from NPC, outside detection_radius (8.0).
    let player_pos = Vec3::ZERO;
    app.world_mut().spawn((
        Transform::from_translation(player_pos),
        GlobalTransform::default(),
        npc_aggro_test_player_controller(),
    ));

    // NPC at (50, 0, 0) — Idle, Chase faction, outside detection_radius.
    let npc_pos = Vec3::new(50.0, 0.0, 0.0);
    let npc_entity = app.world_mut().spawn((
        SpawnId("enemy_01".to_string()),
        Transform::from_translation(npc_pos),
        GlobalTransform::default(),
        Velocity { linvel: Vec3::ZERO, angvel: Vec3::ZERO },
        npc_aggro_test_npc_agent("enemy_01", NpcOnPlayerNear::Chase, npc_pos),
    )).id();

    // Populate NpcHitQueue with attacker position (mirrors npc_hit_relay_system).
    app.world_mut()
        .resource_mut::<NpcHitQueue>()
        .0.insert("enemy_01".to_string(), player_pos);
    let _ = app.world_mut().run_system_once(npc_behavior_system);

    let npc = app.world().entity(npc_entity).get::<NpcAgent>().unwrap();
    assert!(
        matches!(npc.state, NpcState::Investigating),
        "Chase NPC should transition Idle → Investigating when hit outside detection radius"
    );
    assert_eq!(
        npc.last_known_attacker_pos, Some(player_pos),
        "last_known_attacker_pos should be set to the attacker position"
    );
}

/// Flee NPC hit outside detection radius must stay Idle — gated by `on_player_near`.
#[test]
fn test_npc_flee_does_not_aggro_on_hit() {
    use ironhold_core::capabilities::{NpcAgent, NpcState, npc_behavior_system};
    use ironhold_core::capabilities::npc::NpcHitQueue;
    use ironhold_core::schema::catalog::NpcOnPlayerNear;
    use bevy_rapier3d::prelude::Velocity;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn((
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
        npc_aggro_test_player_controller(),
    ));

    let npc_pos = Vec3::new(50.0, 0.0, 0.0);
    let mut agent = npc_aggro_test_npc_agent("alpaka_01", NpcOnPlayerNear::Flee, npc_pos);
    agent.npc_id = "alpaka_01".to_string();
    let npc_entity = app.world_mut().spawn((
        SpawnId("alpaka_01".to_string()),
        Transform::from_translation(npc_pos),
        GlobalTransform::default(),
        Velocity { linvel: Vec3::ZERO, angvel: Vec3::ZERO },
        agent,
    )).id();

    app.world_mut()
        .resource_mut::<NpcHitQueue>()
        .0.insert("alpaka_01".to_string(), Vec3::ZERO);
    let _ = app.world_mut().run_system_once(npc_behavior_system);

    let npc = app.world().entity(npc_entity).get::<NpcAgent>().unwrap();
    assert!(
        matches!(npc.state, NpcState::Idle),
        "Flee NPC must not aggro on hit — should remain Idle"
    );
}

/// Hit event for an unknown NPC id must not crash or affect unrelated NPCs.
#[test]
fn test_npc_aggro_unknown_id_is_noop() {
    use ironhold_core::capabilities::{NpcAgent, NpcState, npc_behavior_system};
    use ironhold_core::capabilities::npc::NpcHitQueue;
    use ironhold_core::schema::catalog::NpcOnPlayerNear;
    use bevy_rapier3d::prelude::Velocity;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn((
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
        npc_aggro_test_player_controller(),
    ));

    let npc_pos = Vec3::new(50.0, 0.0, 0.0);
    let npc_entity = app.world_mut().spawn((
        SpawnId("orc_01".to_string()),
        Transform::from_translation(npc_pos),
        GlobalTransform::default(),
        Velocity { linvel: Vec3::ZERO, angvel: Vec3::ZERO },
        npc_aggro_test_npc_agent("orc_01", NpcOnPlayerNear::Chase, npc_pos),
    )).id();

    // Hit event for a completely different id — must not affect orc_01.
    app.world_mut()
        .resource_mut::<NpcHitQueue>()
        .0.insert("does_not_exist".to_string(), Vec3::ZERO);
    let _ = app.world_mut().run_system_once(npc_behavior_system);

    let npc = app.world().entity(npc_entity).get::<NpcAgent>().unwrap();
    assert!(
        matches!(npc.state, NpcState::Idle),
        "Unknown hit id must not affect unrelated NPCs — orc_01 should remain Idle"
    );
}

/// Investigating NPC that times out with no new hit transitions to Return.
#[test]
fn test_npc_investigating_timeout_returns() {
    use ironhold_core::capabilities::{NpcAgent, NpcState, npc_behavior_system};
    use ironhold_core::schema::catalog::NpcOnPlayerNear;
    use bevy_rapier3d::prelude::Velocity;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn((
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
        npc_aggro_test_player_controller(),
    ));

    let npc_pos = Vec3::new(50.0, 0.0, 0.0);
    let mut agent = npc_aggro_test_npc_agent("snake_01", NpcOnPlayerNear::Chase, npc_pos);
    agent.state = NpcState::Investigating;
    agent.last_known_attacker_pos = Some(Vec3::new(45.0, 0.0, 0.0));
    // Set timer to just past the timeout — no new hit arrives, player not visible.
    agent.investigate_timer = 5.1;
    agent.investigate_timeout_secs = 5.0;

    let npc_entity = app.world_mut().spawn((
        SpawnId("snake_01".to_string()),
        Transform::from_translation(npc_pos),
        GlobalTransform::default(),
        Velocity { linvel: Vec3::ZERO, angvel: Vec3::ZERO },
        agent,
    )).id();

    let _ = app.world_mut().run_system_once(npc_behavior_system);

    let npc = app.world().entity(npc_entity).get::<NpcAgent>().unwrap();
    assert!(
        matches!(npc.state, NpcState::Return),
        "Investigating NPC should transition to Return after timeout with no new hit"
    );
}

/// Hit during Investigating resets the timer and updates the last-known position.
#[test]
fn test_npc_investigating_hit_refresh_resets_timer() {
    use ironhold_core::capabilities::{NpcAgent, NpcState, npc_behavior_system};
    use ironhold_core::capabilities::npc::NpcHitQueue;
    use ironhold_core::schema::catalog::NpcOnPlayerNear;
    use bevy_rapier3d::prelude::Velocity;

    let mut app = setup_test_app();
    app.update();

    let player_pos = Vec3::new(30.0, 0.0, 0.0);
    app.world_mut().spawn((
        Transform::from_translation(player_pos),
        GlobalTransform::default(),
        npc_aggro_test_player_controller(),
    ));

    let npc_pos = Vec3::new(50.0, 0.0, 0.0);
    let mut agent = npc_aggro_test_npc_agent("snake_02", NpcOnPlayerNear::Chase, npc_pos);
    agent.state = NpcState::Investigating;
    agent.last_known_attacker_pos = Some(Vec3::new(45.0, 0.0, 0.0));
    agent.investigate_timer = 3.5; // near timeout but not past it
    agent.investigate_timeout_secs = 5.0;

    let npc_entity = app.world_mut().spawn((
        SpawnId("snake_02".to_string()),
        Transform::from_translation(npc_pos),
        GlobalTransform::default(),
        Velocity { linvel: Vec3::ZERO, angvel: Vec3::ZERO },
        agent,
    )).id();

    // Another hit arrives — should update position and reset timer.
    app.world_mut()
        .resource_mut::<NpcHitQueue>()
        .0.insert("snake_02".to_string(), player_pos);
    let _ = app.world_mut().run_system_once(npc_behavior_system);

    let npc = app.world().entity(npc_entity).get::<NpcAgent>().unwrap();
    assert!(
        matches!(npc.state, NpcState::Investigating),
        "NPC should remain Investigating after a hit refreshes the timer"
    );
    assert_eq!(
        npc.last_known_attacker_pos, Some(player_pos),
        "last_known_attacker_pos should be updated to the new hit position"
    );
    assert!(
        npc.investigate_timer < 0.1,
        "investigate_timer should be reset to ~0 after a new hit"
    );
}

#[test]
fn test_camera_shake_inserts_component_on_orbit_camera() {
    use ironhold_core::capabilities::camera::{ActiveCameraMode, OrbitState, OrbitCameraMode, CameraTargets, CameraShakeState};

    let mut app = setup_test_app();

    // Spawn a stub player so the camera's CameraTargets can point at it.
    let player = app.world_mut().spawn((
        Transform::default(),
        GlobalTransform::default(),
        npc_aggro_test_player_controller(),
        SpeedMultiplier(1.0),
    )).id();

    // Spawn an orbit-mode camera targeting the player.
    let camera_entity = app.world_mut().spawn((
        Transform::default(),
        GlobalTransform::default(),
        ActiveCameraMode::Orbit(OrbitState {
            radius: 5.0,
            offset: bevy::math::Vec3::ZERO,
            zoom_speed: 1.0,
            orbit_speed: 1.0,
            min_radius: 1.0,
            max_radius: 20.0,
            pitch: 0.3,
            yaw: 0.0,
            look_at_offset: bevy::math::Vec3::ZERO,
            min_pitch: -0.5,
            max_pitch: 1.0,
            orbit_lmb: false,
            orbit_rmb: true,
            character_rotate_rmb: false,
            character_rotate_lmb: false,
            look_left_key: None,
            look_right_key: None,
            look_up_key: None,
            look_down_key: None,
            look_speed: 2.0,
            gamepad_deadzone: 0.15,
        }),
        OrbitCameraMode,
        CameraTargets(vec![player]),
    )).id();

    app.update();

    // Fire CameraShake through the action queue.
    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::CameraShake { duration_secs: 0.5, intensity: 0.2, owner_player: None });

    app.update();

    let shake = app.world().entity(camera_entity).get::<CameraShakeState>();
    assert!(shake.is_some(), "CameraShakeState should be inserted on the orbit camera entity");
    let shake = shake.unwrap();
    assert!((shake.remaining - 0.5).abs() < 0.05, "remaining should be ~0.5 s");
    assert!((shake.intensity - 0.2).abs() < 0.001, "intensity should be 0.2");
}

#[test]
fn test_camera_shake_no_orbit_camera_is_noop() {
    // No orbit camera in the scene — the action should log a warning and not panic.
    let mut app = setup_test_app();
    app.update();

    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::CameraShake { duration_secs: 0.3, intensity: 0.1, owner_player: None });

    // Should not panic.
    app.update();
}

#[test]
fn test_camera_shake_component_removed_after_expiry() {
    use ironhold_core::capabilities::camera::{ActiveCameraMode, OrbitState, OrbitCameraMode, CameraTargets, CameraShakeState, camera_shake_system};

    let mut app = setup_test_app();

    let player = app.world_mut().spawn((
        Transform::default(),
        GlobalTransform::default(),
        npc_aggro_test_player_controller(),
        SpeedMultiplier(1.0),
    )).id();

    let camera_entity = app.world_mut().spawn((
        Transform::default(),
        GlobalTransform::default(),
        ActiveCameraMode::Orbit(OrbitState {
            radius: 5.0,
            offset: bevy::math::Vec3::ZERO,
            zoom_speed: 1.0,
            orbit_speed: 1.0,
            min_radius: 1.0,
            max_radius: 20.0,
            pitch: 0.3,
            yaw: 0.0,
            look_at_offset: bevy::math::Vec3::ZERO,
            min_pitch: -0.5,
            max_pitch: 1.0,
            orbit_lmb: false,
            orbit_rmb: true,
            character_rotate_rmb: false,
            character_rotate_lmb: false,
            look_left_key: None,
            look_right_key: None,
            look_up_key: None,
            look_down_key: None,
            look_speed: 2.0,
            gamepad_deadzone: 0.15,
        }),
        OrbitCameraMode,
        CameraTargets(vec![player]),
        // Insert an already-expired shake (remaining <= 0) to trigger removal.
        CameraShakeState {
            remaining: -0.01,
            duration: 0.1,
            intensity: 0.1,
        },
    )).id();

    // One system tick should remove the expired component.
    let _ = app.world_mut().run_system_once(camera_shake_system);

    let shake = app.world().entity(camera_entity).get::<CameraShakeState>();
    assert!(shake.is_none(), "CameraShakeState should be removed when remaining <= 0");
}
