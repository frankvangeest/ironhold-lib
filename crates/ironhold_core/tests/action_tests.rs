use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use ironhold_core::GameVariables;
use ironhold_core::runtime::{GameEvent, ActionQueue, InputAction, InputActionMessage, SpawnId, SpawnRegistry};
use ironhold_core::schema::Action;
use ironhold_core::capabilities::player::{CharacterController, SpeedMultiplier, player_movement_system};
use ironhold_core::capabilities::animation::AnimationController;
use ironhold_core::schema::player::{InputMap, AnimationPolicy, BaseAnimations};
use ironhold_core::capabilities::animation_resolver::{AnimationPolicyComponent, LocomotionState, AnimationRequests, ActiveOverride};

mod support;
use support::setup_test_app;

#[test]
fn test_log_action_updates_debug_last_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::Log("hello_world".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"hello_world\")");
}

#[test]
fn test_set_variable_writes_value() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::SetVariable("level".to_string(), "3".to_string()));
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(vars.0.get("level").map(String::as_str), Some("3"));
}

#[test]
fn test_set_variable_overwrites_previous_value() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::SetVariable("mode".to_string(), "easy".to_string()));
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::SetVariable("mode".to_string(), "hard".to_string()));
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("mode").map(String::as_str),
        Some("hard"),
        "Second SetVariable must overwrite the first"
    );
}

#[test]
fn test_increment_variable_starts_from_zero_when_unset() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::IncrementVariable("score".to_string(), 10));
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("score").map(String::as_str),
        Some("10"),
        "IncrementVariable on an unset key must start from 0"
    );
}

#[test]
fn test_increment_variable_accumulates() {
    let mut app = setup_test_app();
    app.update();

    for _ in 0..3 {
        app.world_mut().resource_mut::<ActionQueue>()
            .push(Action::IncrementVariable("score".to_string(), 10));
        app.update();
    }

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("score").map(String::as_str),
        Some("30"),
        "Three increments of 10 must accumulate to 30"
    );
}

#[test]
fn test_increment_variable_negative_delta() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::SetVariable("score".to_string(), "50".to_string()));
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::IncrementVariable("score".to_string(), -15));
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("score").map(String::as_str),
        Some("35"),
        "Negative delta must subtract from the current value"
    );
}

#[test]
fn test_increment_variable_on_non_numeric_string_treats_as_zero() {
    let mut app = setup_test_app();
    app.update();

    // Set a variable to a non-numeric value (e.g. a player name)
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::SetVariable("player_name".to_string(), "Hero".to_string()));
    app.update();

    // IncrementVariable on a non-numeric value must not crash; it warns and treats as 0
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::IncrementVariable("player_name".to_string(), 5));
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("player_name").map(String::as_str),
        Some("5"),
        "IncrementVariable on a non-numeric value must treat the current value as 0"
    );
}

#[test]
fn test_player_jump_emits_game_event() {
    let mut app = setup_test_app();
    app.update();

    // Spawn a minimal player entity; no Rapier context â†’ is_grounded defaults to true each frame.
    let entity = app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        CharacterController {
            walk_speed: 10.0,
            run_speed: 20.0,
            rot_speed: 2.0,
            inputs: InputMap {
                forward: "KeyW".to_string(),
                backward: "KeyS".to_string(),
                left: "KeyA".to_string(),
                right: "KeyD".to_string(),
                strafe_left: "KeyQ".to_string(),
                strafe_right: "KeyE".to_string(),
                jump: "Space".to_string(),
                run: "ShiftLeft".to_string(),
                interact: "KeyF".to_string(),
                strafe_mouse_button: Some("Left".to_string()),
                target_next: "Tab".to_string(),
                target_range: 30.0,
                gamepad_index: None,
            },
            is_running: false,
            jump_velocity: 5.94,
            double_jump_enabled: false,
            double_jump_velocity: 5.94,
            jumps_used: 0,
            max_jumps: 1,
            collider_radius: 0.4,
            ground_cast_length: 0.3,
            idle_drag: 0.8,
        },
        LocomotionState::default(),
        AnimationRequests::default(),
        ActiveOverride::default(),
        AnimationPolicyComponent(AnimationPolicy {
            base: BaseAnimations {
                idle: "idle".to_string(),
                walk: "walk".to_string(),
                run: "run".to_string(),
                jump_loop: "idle".to_string(),
            },
            clips: std::collections::HashMap::new(),
            overrides: vec![],
            default_transition_ms: None,
            animation_sources: vec![],
        }),
        AnimationController {
            current: "idle".to_string(),
            last_played: String::new(),
            gltf_path: String::new(),
            gltf_handle: Default::default(),
            source_handles: vec![],
            node_indices: Default::default(),
            graph_initialized: false,
            transition_ms: 0,
            should_loop: true,
            last_player_entity: None,
        },
        bevy_rapier3d::prelude::RigidBody::Dynamic,
        bevy_rapier3d::prelude::Velocity::zero(),
        SpeedMultiplier(1.0),
    )).id();

    // RapierPhysicsPlugin runs (even in integration tests, cfg(test) does not apply to lib
    // dependencies) and spawns a DefaultRapierContext entity.  With no physics simulation
    // running, cast_shape always returns None â†’ is_grounded=false â†’ no jump.
    // Remove that entity so player_movement_system falls into its headless else-branch
    // (is_grounded=true), letting us exercise the jump code path without a physics world.
    {
        use bevy_rapier3d::plugin::context::DefaultRapierContext;
        let rapier_entity = app.world_mut()
            .query_filtered::<Entity, With<DefaultRapierContext>>()
            .iter(app.world())
            .next();
        if let Some(e) = rapier_entity {
            app.world_mut().despawn(e);
        }
    }

    // Write InputActionMessage directly â€” bypasses input_translator and FixedUpdate timing.
    app.world_mut()
        .resource_mut::<Messages<InputActionMessage>>()
        .write(InputActionMessage { entity, action: InputAction::Jump(true) });

    // Run player_movement_system with a fresh MessageCursor (sees the Jump message).
    // GameEvent is written to messages_b synchronously inside the system.
    app.world_mut().run_system_once(player_movement_system).unwrap();

    // Verify upward velocity was applied.
    let vel = app.world().entity(entity).get::<bevy_rapier3d::prelude::Velocity>().unwrap();
    assert!(vel.linvel.y > 0.0, "Expected upward velocity after jump, got {}", vel.linvel.y);

    // Verify GameEvent::Trigger("player.jumped") is in the current-frame buffer.
    let has_jumped = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(name) if name == "player.jumped"));
    assert!(has_jumped, "Expected GameEvent::Trigger(\"player.jumped\") in messages after jump");
}

#[test]
fn test_emit_event_after_delay_fires_game_event_when_elapsed() {
    use ironhold_core::runtime::scene_manager::DelayedEventQueue;
    let mut app = setup_test_app();
    app.update();

    // Seed a nearly-expired entry directly.
    app.world_mut()
        .resource_mut::<DelayedEventQueue>()
        .0
        .push((0.001, "entity.respawning:dummy_01".to_string()));

    // Advance time enough for the entry to expire.
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(100));

    app.update();

    // Queue should be empty â€” the entry fired and was removed.
    let queue = app.world().resource::<DelayedEventQueue>();
    assert!(queue.0.is_empty(), "expired entry must be removed from DelayedEventQueue");

    // The GameEvent must have been written.
    let events = app.world().resource::<Messages<GameEvent>>();
    let found = events.iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(n) if n == "entity.respawning:dummy_01"));
    assert!(found, "tick_delayed_events_system must emit the event when it expires");
}

#[test]
fn test_emit_event_after_delay_does_not_fire_before_elapsed() {
    use ironhold_core::runtime::scene_manager::DelayedEventQueue;
    let mut app = setup_test_app();
    app.update();

    app.world_mut()
        .resource_mut::<DelayedEventQueue>()
        .0
        .push((15.0, "entity.respawning:dummy_01".to_string()));

    // Advance only a little â€” should not fire yet.
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(500));

    app.update();

    let queue = app.world().resource::<DelayedEventQueue>();
    assert_eq!(queue.0.len(), 1, "entry must remain when delay has not elapsed");
}

#[test]
fn test_show_floating_text_spawns_world_label() {
    // ShowFloatingText must spawn WorldLabel entities (main + shadow) when the
    // target entity exists in the SpawnRegistry with a GlobalTransform.
    use ironhold_core::runtime::scene_manager::WorldLabel;
    use ironhold_core::capabilities::damage_popup::DamagePopup;

    let mut app = setup_test_app();
    app.update();

    let entity = app.world_mut().spawn((
        SpawnId("hero".to_string()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("hero".to_string(), entity);

    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::ShowFloatingText {
            entity: "hero".to_string(),
            text: "Hello!".to_string(),
            offset: None,
        });
    app.update();

    // Expect exactly 2 WorldLabel entities with DamagePopup (shadow + main).
    let count = app
        .world_mut()
        .query::<(&WorldLabel, &DamagePopup)>()
        .iter(app.world())
        .count();
    assert_eq!(count, 2,
        "ShowFloatingText must spawn 2 WorldLabel+DamagePopup entities (shadow + main)");
}

#[test]
fn test_show_floating_text_offset_overrides_default_spawn_offset() {
    // When ShowFloatingText is given an explicit offset, the WorldLabel world_pos
    // must use that offset rather than the DamagePopupStyle default.
    use ironhold_core::runtime::scene_manager::WorldLabel;
    use ironhold_core::capabilities::damage_popup::DamagePopup;

    let mut app = setup_test_app();
    app.update();

    let entity = app.world_mut().spawn((
        SpawnId("hero".to_string()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("hero".to_string(), entity);

    // Use a distinctive Y value that differs from the default (1.2).
    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::ShowFloatingText {
            entity: "hero".to_string(),
            text: "Custom!".to_string(),
            offset: Some((0.0, 3.5, 0.0)),
        });
    app.update();

    let labels: Vec<_> = app
        .world_mut()
        .query::<(&WorldLabel, &DamagePopup)>()
        .iter(app.world())
        .map(|(l, _)| l.offset)
        .collect();

    assert_eq!(labels.len(), 2, "shadow + main must both be spawned");
    for offset in &labels {
        assert!(
            (offset.y - 3.5_f32).abs() < 0.001,
            "ShowFloatingText offset override must set WorldLabel.offset.y to 3.5, got {}",
            offset.y
        );
    }
}

#[test]
fn test_target_indicator_spawns_on_set_target_and_despawns_on_clear() {
    use ironhold_core::capabilities::target_indicator::TrackingTarget;
    use ironhold_core::capabilities::action_bar::CurrentTarget;
    use ironhold_core::runtime::scene_manager::{LoadedTargetIndicator, ResolvedTargetIndicator};

    let mut app = setup_test_app();
    app.update();

    // Register a world entity with a GlobalTransform in the SpawnRegistry.
    let entity = app.world_mut().spawn((
        SpawnId("target_01".to_string()),
        Transform::from_translation(Vec3::new(3.0, 0.0, 5.0)),
        GlobalTransform::from(Transform::from_translation(Vec3::new(3.0, 0.0, 5.0))),
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("target_01".to_string(), entity);

    // Configure a target indicator (texture path need not resolve in tests).
    app.world_mut().insert_resource(LoadedTargetIndicator(Some(ResolvedTargetIndicator {
        texture_path: "shared/textures/decals/ring_thick.png".to_string(),
        radius: 1.2,
        color: (0.3, 0.8, 1.0, 0.75),
        offset_y: 0.05,
        named_colors: std::collections::HashMap::new(),
    })));

    // Selecting a target must spawn a TrackingTarget indicator entity.
    app.world_mut().resource_mut::<CurrentTarget>().0 = Some("target_01".to_string());
    app.update();

    let indicator_count = app.world_mut()
        .query::<&TrackingTarget>()
        .iter(app.world())
        .count();
    assert_eq!(indicator_count, 1, "one TrackingTarget entity must exist after selecting a target");

    // Clearing the target must despawn the indicator within one frame.
    app.world_mut().resource_mut::<CurrentTarget>().0 = None;
    app.update();
    // Despawn is deferred (Commands); run one more update to flush.
    app.update();

    let indicator_count_after = app.world_mut()
        .query::<&TrackingTarget>()
        .iter(app.world())
        .count();
    assert_eq!(indicator_count_after, 0, "TrackingTarget entity must be despawned after clearing the target");
}
