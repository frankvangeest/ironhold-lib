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
                gamepad_index: None, look_left: None, look_right: None, look_up: None, look_down: None,
                gamepad_jump: "South".to_string(), gamepad_run: "East".to_string(),
                gamepad_interact: "West".to_string(), gamepad_target_next: "North".to_string(),
                gamepad_deadzone: 0.15,
            },
            is_running: false,
            jump_velocity: 5.94,
            double_jump_enabled: false,
            double_jump_velocity: 5.94,
            jumps_used: 0,
            max_jumps: 1,
            collider_radius: 0.4,
            ground_cast_length: 0.3,
            max_walkable_slope_deg: 45.0,
            coyote_time_secs: 0.1,
            coyote_ticks_remaining: 0,
            jump_air_grace: 0, jump_liftoff_y: None,
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

/// Regression: a non-split (ordinary) scene must keep spawning exactly the pre-existing
/// shadow+main pair, no `WorldLabelRank` overhead — `ActiveSplitScreen`/`DynamicSplitConfig`
/// both default to `None` via `setup_test_app()`.
#[test]
fn test_show_damage_popup_spawns_exactly_two_entities_when_not_split_screen() {
    use ironhold_core::runtime::scene_manager::WorldLabel;
    use ironhold_core::capabilities::damage_popup::DamagePopup;

    let mut app = setup_test_app();
    app.update();

    let entity = app.world_mut().spawn((
        SpawnId("enemy_01".to_string()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("enemy_01".to_string(), entity);

    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::ShowDamagePopup { entity: "enemy_01".to_string(), amount: -10.0 });
    app.update();

    let count = app
        .world_mut()
        .query::<(&WorldLabel, &DamagePopup)>()
        .iter(app.world())
        .count();
    assert_eq!(count, 2,
        "a non-split scene must spawn exactly 2 WorldLabel+DamagePopup entities (shadow + main), \
         unchanged from before per-player split-screen support");
}

/// Phase 2 (`per_player_split_screen_targeting.md`): in a split-screen scene, `ShowDamagePopup`
/// must duplicate across `WorldLabelRank`s (same mechanism `stat_label`/`world_stat_bar` already
/// use) — otherwise the popup only ever renders in the single highest-priority active viewport,
/// regardless of which player's action actually triggered it.
#[test]
fn test_show_damage_popup_duplicates_ranks_when_split_screen_active() {
    use ironhold_core::runtime::scene_manager::{WorldLabel, WorldLabelRank, ActiveSplitScreen};
    use ironhold_core::capabilities::damage_popup::DamagePopup;
    use ironhold_core::capabilities::camera::MAX_SPLIT_PLAYERS;
    use ironhold_core::schema::player::SplitOrientation;

    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(ActiveSplitScreen(Some(SplitOrientation::Vertical)));

    let entity = app.world_mut().spawn((
        SpawnId("enemy_01".to_string()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("enemy_01".to_string(), entity);

    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::ShowDamagePopup { entity: "enemy_01".to_string(), amount: -10.0 });
    app.update();

    let ranks: Vec<Option<u8>> = {
        let mut q = app.world_mut().query::<(&WorldLabel, &DamagePopup, Option<&WorldLabelRank>)>();
        q.iter(app.world()).map(|(_, _, rank)| rank.map(|r| r.0)).collect()
    };
    assert_eq!(
        ranks.len(), 2 * MAX_SPLIT_PLAYERS as usize,
        "split-screen must spawn a shadow+main pair per rank (0..MAX_SPLIT_PLAYERS), not just one pair"
    );
    for rank in 0..MAX_SPLIT_PLAYERS as u8 {
        let expected = if rank == 0 { None } else { Some(rank) };
        assert_eq!(
            ranks.iter().filter(|r| **r == expected).count(), 2,
            "expected exactly 2 entities (shadow + main) at rank {}", rank
        );
    }
}

#[test]
fn test_target_indicator_spawns_on_set_target_and_despawns_on_clear() {
    use ironhold_core::capabilities::target_indicator::TrackingTarget;
    use ironhold_core::capabilities::player::PlayerTarget;
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

    // The indicator system reads each player entity's own PlayerTarget (Phase 1 of
    // per_player_split_screen_targeting.md) — a CharacterController + PlayerTarget stand-in for
    // a real player here, since the full spawn pipeline isn't exercised in this test.
    let player = app.world_mut().spawn((
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
            collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
        },
        PlayerTarget::default(),
    )).id();

    // Configure a target indicator (texture path need not resolve in tests).
    app.world_mut().insert_resource(LoadedTargetIndicator(Some(ResolvedTargetIndicator {
        texture_path: "shared/textures/decals/ring_thick.png".to_string(),
        radius: 1.2,
        color: (0.3, 0.8, 1.0, 0.75),
        offset_y: 0.05,
        named_colors: std::collections::HashMap::new(),
    })));

    // Selecting a target must spawn a TrackingTarget indicator entity.
    app.world_mut().get_mut::<PlayerTarget>(player).unwrap().0 = Some("target_01".to_string());
    app.update();

    let indicator_count = app.world_mut()
        .query::<&TrackingTarget>()
        .iter(app.world())
        .count();
    assert_eq!(indicator_count, 1, "one TrackingTarget entity must exist after selecting a target");

    // Clearing the target must despawn the indicator within one frame.
    app.world_mut().get_mut::<PlayerTarget>(player).unwrap().0 = None;
    app.update();
    // Despawn is deferred (Commands); run one more update to flush.
    app.update();

    let indicator_count_after = app.world_mut()
        .query::<&TrackingTarget>()
        .iter(app.world())
        .count();
    assert_eq!(indicator_count_after, 0, "TrackingTarget entity must be despawned after clearing the target");
}

/// Shared by the two `RenderLayers`/`TargetRingVisibilityMode` ring tests below — same field
/// values as this file's existing inline `CharacterController` literals, just de-duplicated since
/// both new tests need one.
fn test_character_controller_for_ring_tests() -> CharacterController {
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
        collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
    }
}

/// `per_viewport_target_ring_visibility.md` regression: `TargetRingVisibilityMode` defaults to
/// `AllViewports` (init_resource'd, never explicitly set here), so a spawned ring must carry no
/// `RenderLayers` component at all — zero footprint for every existing scene.
#[test]
fn test_target_indicator_ring_has_no_render_layers_when_visibility_mode_is_default() {
    use ironhold_core::capabilities::target_indicator::TrackingTarget;
    use ironhold_core::capabilities::player::PlayerTarget;
    use ironhold_core::runtime::scene_manager::{LoadedTargetIndicator, ResolvedTargetIndicator, TargetRingVisibilityMode};
    use bevy::camera::visibility::RenderLayers;

    let mut app = setup_test_app();
    app.update();

    assert_eq!(*app.world().resource::<TargetRingVisibilityMode>(), TargetRingVisibilityMode::AllViewports);

    let entity = app.world_mut().spawn((
        SpawnId("target_01".to_string()),
        Transform::from_translation(Vec3::new(3.0, 0.0, 5.0)),
        GlobalTransform::from(Transform::from_translation(Vec3::new(3.0, 0.0, 5.0))),
    )).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("target_01".to_string(), entity);

    let player = app.world_mut().spawn((
        test_character_controller_for_ring_tests(),
        PlayerTarget::default(),
    )).id();

    app.world_mut().insert_resource(LoadedTargetIndicator(Some(ResolvedTargetIndicator {
        texture_path: "shared/textures/decals/ring_thick.png".to_string(),
        radius: 1.2,
        color: (0.3, 0.8, 1.0, 0.75),
        offset_y: 0.05,
        named_colors: std::collections::HashMap::new(),
    })));

    app.world_mut().get_mut::<PlayerTarget>(player).unwrap().0 = Some("target_01".to_string());
    app.update();

    let ring_entity = {
        let mut q = app.world_mut().query::<(Entity, &TrackingTarget)>();
        q.iter(app.world()).map(|(e, _)| e).next().expect("ring must spawn")
    };
    assert!(
        app.world().get::<RenderLayers>(ring_entity).is_none(),
        "with own_viewport_only unset (today's behavior), the ring must carry NO RenderLayers \
         component at all"
    );
}

/// `per_viewport_target_ring_visibility.md`: when `TargetRingVisibilityMode::OwnViewportOnly` is
/// set, each player's ring gets a `RenderLayers` restricted to that player's own reserved layer
/// (`1 + player_index % MAX_SPLIT_PLAYERS`), matching the camera-side layer assigned in
/// `local_coop_tests.rs`'s `test_static_split_own_viewport_only_gives_each_camera_its_own_layer_plus_shared_layer_0`.
#[test]
fn test_target_indicator_ring_own_viewport_only_gets_its_owning_players_reserved_layer() {
    use ironhold_core::capabilities::target_indicator::TrackingTarget;
    use ironhold_core::capabilities::player::{PlayerTarget, PlayerIndex};
    use ironhold_core::runtime::scene_manager::{LoadedTargetIndicator, ResolvedTargetIndicator, TargetRingVisibilityMode};
    use bevy::camera::visibility::RenderLayers;

    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(TargetRingVisibilityMode::OwnViewportOnly);

    let entity_a = app.world_mut().spawn((
        SpawnId("enemy_a".to_string()),
        GlobalTransform::from_translation(Vec3::new(1.0, 0.0, 0.0)),
    )).id();
    let entity_b = app.world_mut().spawn((
        SpawnId("enemy_b".to_string()),
        GlobalTransform::from_translation(Vec3::new(-1.0, 0.0, 0.0)),
    )).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_a".to_string(), entity_a);
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_b".to_string(), entity_b);

    app.world_mut().insert_resource(LoadedTargetIndicator(Some(ResolvedTargetIndicator {
        texture_path: "shared/textures/decals/ring_thick.png".to_string(),
        radius: 1.2,
        color: (0.3, 0.8, 1.0, 0.75),
        offset_y: 0.05,
        named_colors: std::collections::HashMap::new(),
    })));

    let player0 = app.world_mut().spawn((
        test_character_controller_for_ring_tests(),
        PlayerTarget(Some("enemy_a".to_string())),
        PlayerIndex(0),
    )).id();
    let player1 = app.world_mut().spawn((
        test_character_controller_for_ring_tests(),
        PlayerTarget(Some("enemy_b".to_string())),
        PlayerIndex(1),
    )).id();

    app.update();

    let layers_by_owner: std::collections::HashMap<Entity, RenderLayers> = {
        let mut q = app.world_mut().query::<(&TrackingTarget, &RenderLayers)>();
        q.iter(app.world()).map(|(t, l)| (t.owner, l.clone())).collect()
    };
    assert_eq!(layers_by_owner.len(), 2, "both rings must carry a RenderLayers component in OwnViewportOnly mode");

    let p0_layers = &layers_by_owner[&player0];
    assert!(p0_layers.intersects(&RenderLayers::layer(1)), "player 0's ring must carry its reserved layer 1");
    assert!(!p0_layers.intersects(&RenderLayers::layer(2)), "player 0's ring must NOT carry player 1's layer 2");

    let p1_layers = &layers_by_owner[&player1];
    assert!(p1_layers.intersects(&RenderLayers::layer(2)), "player 1's ring must carry its reserved layer 2");
    assert!(!p1_layers.intersects(&RenderLayers::layer(1)), "player 1's ring must NOT carry player 0's layer 1");
}

/// Phase 1 (`per_player_split_screen_targeting.md`): when 2+ players are present, every target
/// indicator ring is tinted by the fixed `PLAYER_LABEL_COLORS` palette (same one the split-screen
/// "P{n}" corner HUD label uses) instead of the usual per-target prefab/category/scene colour
/// precedence, so it's visually obvious whose ring is whose.
#[test]
fn test_target_indicator_tints_rings_per_player_when_multiplayer() {
    use ironhold_core::capabilities::target_indicator::TrackingTarget;
    use ironhold_core::capabilities::player::{PlayerTarget, PlayerIndex};
    use ironhold_core::capabilities::camera::PLAYER_LABEL_COLORS;
    use ironhold_core::runtime::scene_manager::{LoadedTargetIndicator, ResolvedTargetIndicator};

    let mut app = setup_test_app();
    app.update();

    let entity_a = app.world_mut().spawn((
        SpawnId("enemy_a".to_string()),
        GlobalTransform::from_translation(Vec3::new(1.0, 0.0, 0.0)),
    )).id();
    let entity_b = app.world_mut().spawn((
        SpawnId("enemy_b".to_string()),
        GlobalTransform::from_translation(Vec3::new(-1.0, 0.0, 0.0)),
    )).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_a".to_string(), entity_a);
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_b".to_string(), entity_b);

    app.world_mut().insert_resource(LoadedTargetIndicator(Some(ResolvedTargetIndicator {
        texture_path: "shared/textures/decals/ring_thick.png".to_string(),
        radius: 1.2,
        color: (0.3, 0.8, 1.0, 0.75),
        offset_y: 0.05,
        named_colors: std::collections::HashMap::new(),
    })));

    let player1 = app.world_mut().spawn((
        CharacterController {
            walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
            inputs: InputMap {
                forward: "KeyW".to_string(), backward: "KeyS".to_string(),
                left: "KeyA".to_string(), right: "KeyD".to_string(),
                strafe_left: "KeyQ".to_string(), strafe_right: "KeyE".to_string(),
                jump: "Space".to_string(), run: "ShiftLeft".to_string(),
                interact: "KeyF".to_string(), strafe_mouse_button: None,
                target_next: "Tab".to_string(), target_range: 30.0, gamepad_index: None, look_left: None, look_right: None, look_up: None, look_down: None,
                gamepad_jump: "South".to_string(), gamepad_run: "East".to_string(),
                gamepad_interact: "West".to_string(), gamepad_target_next: "North".to_string(),
                gamepad_deadzone: 0.15,
            },
            is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
            double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
            collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
        },
        PlayerTarget(Some("enemy_a".to_string())),
        PlayerIndex(0),
    )).id();
    let _player2 = app.world_mut().spawn((
        CharacterController {
            walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
            inputs: InputMap {
                forward: "KeyW".to_string(), backward: "KeyS".to_string(),
                left: "KeyA".to_string(), right: "KeyD".to_string(),
                strafe_left: "KeyQ".to_string(), strafe_right: "KeyE".to_string(),
                jump: "Space".to_string(), run: "ShiftLeft".to_string(),
                interact: "KeyF".to_string(), strafe_mouse_button: None,
                target_next: "Tab".to_string(), target_range: 30.0, gamepad_index: None, look_left: None, look_right: None, look_up: None, look_down: None,
                gamepad_jump: "South".to_string(), gamepad_run: "East".to_string(),
                gamepad_interact: "West".to_string(), gamepad_target_next: "North".to_string(),
                gamepad_deadzone: 0.15,
            },
            is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
            double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
            collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
        },
        PlayerTarget(Some("enemy_b".to_string())),
        PlayerIndex(1),
    )).id();

    app.update();

    let rings: Vec<(Entity, Handle<StandardMaterial>)> = {
        let mut q = app.world_mut().query::<(&TrackingTarget, &MeshMaterial3d<StandardMaterial>)>();
        q.iter(app.world()).map(|(t, m)| (t.owner, m.0.clone())).collect()
    };
    assert_eq!(rings.len(), 2, "each player must get their own ring");

    let materials = app.world().resource::<Assets<StandardMaterial>>();
    for (owner, mat_handle) in rings {
        let expected_idx = if owner == player1 { 0 } else { 1 };
        let expected = PLAYER_LABEL_COLORS[expected_idx].to_srgba();
        let actual = materials.get(&mat_handle).unwrap().base_color.to_srgba();
        assert!(
            (actual.red - expected.red).abs() < 0.001
                && (actual.green - expected.green).abs() < 0.001
                && (actual.blue - expected.blue).abs() < 0.001,
            "ring for player index {} must be tinted by PLAYER_LABEL_COLORS[{}], got {:?} expected {:?}",
            expected_idx, expected_idx, actual, expected
        );
    }
}

/// Regression for a confirmed bug (found via playtest console log during
/// `per_player_camera_look_controls.md`'s playtest, 2026-07-19; unrelated to that feature —
/// `target_indicator_system` reads one `existing` query snapshot and iterates it twice (dead-
/// target cleanup, then owner-retarget replacement); since `Commands` are deferred, a ring whose
/// tracked target dies AND whose owner retargets in the same frame was queued for despawn twice,
/// which Bevy handled gracefully but logged as a "Entity despawned: ... is invalid" warning.
/// Fixed via a per-frame `despawn_queued: HashSet<Entity>` guard shared by both passes.
///
/// **Known limitation, investigated and left as-is** (system-architect + debug-detective review,
/// 2026-07-19): a double-despawn of an already-despawned entity is silently absorbed by Bevy's
/// generation check, so the only observable symptom is the log line itself — this test's
/// end-state assertions (ring counts, surviving owner) pass identically whether the `HashSet`
/// guard is present or not, and would stay green even if the fix were reverted. A genuinely
/// discriminating test would need to assert on that log line; this was attempted (capturing
/// `tracing` output via `bevy::log::{tracing, tracing_subscriber}`, which needs no new
/// dependency) and empirically failed to catch it: Bevy's despawn error handler logs through the
/// old `log` crate facade (`bevy_ecs::error::handler::warn` calls `log::warn!`, confirmed by
/// reading `bevy_ecs-0.18.0/src/error/handler.rs`), not `tracing::warn!` directly — in a real
/// running app `LogPlugin` bridges `log` into `tracing` via `tracing_log::LogTracer`, but
/// `setup_test_app` registers no `LogPlugin`, so that bridge never installs and a
/// `tracing`-only subscriber sees nothing. Properly capturing `log`-facade output would need a
/// custom `log::Log` implementation installed process-wide via `log::set_boxed_logger` (a
/// once-per-process global, requiring `std::sync::Once` + thread-local capture buffers to stay
/// safe across this suite's parallel test threads) — real, reusable test infrastructure in its
/// own right, not a one-line addition to this fix. Logged as a follow-up in
/// `planning/claude_suggestions.md` rather than built here under this bug fix's scope. This test
/// still has real value: it exercises the exact double-trigger scenario and confirms correct
/// end-state behavior (no panic, no leftover/incorrect ring), which is what a reader can verify
/// without needing to trust the log-observability gap explained above.
#[test]
fn test_target_indicator_ring_not_double_despawned_when_target_dies_and_owner_retargets_same_frame() {
    use ironhold_core::capabilities::target_indicator::TrackingTarget;
    use ironhold_core::capabilities::player::{PlayerTarget, PlayerIndex};
    use ironhold_core::runtime::scene_manager::{LoadedTargetIndicator, ResolvedTargetIndicator};

    let mut app = setup_test_app();
    app.update();

    let enemy_a = app.world_mut().spawn((
        SpawnId("enemy_a".to_string()),
        GlobalTransform::from_translation(Vec3::new(1.0, 0.0, 0.0)),
    )).id();
    let enemy_b = app.world_mut().spawn((
        SpawnId("enemy_b".to_string()),
        GlobalTransform::from_translation(Vec3::new(-1.0, 0.0, 0.0)),
    )).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_a".to_string(), enemy_a);
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_b".to_string(), enemy_b);

    app.world_mut().insert_resource(LoadedTargetIndicator(Some(ResolvedTargetIndicator {
        texture_path: "shared/textures/decals/ring_thick.png".to_string(),
        radius: 1.2,
        color: (0.3, 0.8, 1.0, 0.75),
        offset_y: 0.05,
        named_colors: std::collections::HashMap::new(),
    })));

    let player_a = app.world_mut().spawn((
        CharacterController {
            walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
            inputs: InputMap {
                forward: "KeyW".to_string(), backward: "KeyS".to_string(),
                left: "KeyA".to_string(), right: "KeyD".to_string(),
                strafe_left: "KeyQ".to_string(), strafe_right: "KeyE".to_string(),
                jump: "Space".to_string(), run: "ShiftLeft".to_string(),
                interact: "KeyF".to_string(), strafe_mouse_button: None,
                target_next: "Tab".to_string(), target_range: 30.0, gamepad_index: None, look_left: None, look_right: None, look_up: None, look_down: None,
                gamepad_jump: "South".to_string(), gamepad_run: "East".to_string(),
                gamepad_interact: "West".to_string(), gamepad_target_next: "North".to_string(),
                gamepad_deadzone: 0.15,
            },
            is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
            double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
            collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
        },
        PlayerTarget(Some("enemy_a".to_string())),
        PlayerIndex(0),
    )).id();
    let player_b = app.world_mut().spawn((
        CharacterController {
            walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
            inputs: InputMap {
                forward: "KeyW".to_string(), backward: "KeyS".to_string(),
                left: "KeyA".to_string(), right: "KeyD".to_string(),
                strafe_left: "KeyQ".to_string(), strafe_right: "KeyE".to_string(),
                jump: "Space".to_string(), run: "ShiftLeft".to_string(),
                interact: "KeyF".to_string(), strafe_mouse_button: None,
                target_next: "Tab".to_string(), target_range: 30.0, gamepad_index: None, look_left: None, look_right: None, look_up: None, look_down: None,
                gamepad_jump: "South".to_string(), gamepad_run: "East".to_string(),
                gamepad_interact: "West".to_string(), gamepad_target_next: "North".to_string(),
                gamepad_deadzone: 0.15,
            },
            is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
            double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
            collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
        },
        PlayerTarget(Some("enemy_b".to_string())),
        PlayerIndex(1),
    )).id();

    // Both players get their own ring.
    app.update();
    let ring_count = app.world_mut().query::<&TrackingTarget>().iter(app.world()).count();
    assert_eq!(ring_count, 2, "each player must get their own ring before the repro step");

    // The double-trigger: in the SAME frame, player_a's target entity dies (dead-target cleanup
    // pass would despawn player_a's ring) AND player_a's PlayerTarget changes (owner-retarget
    // pass would ALSO try to despawn the same ring) — this is the exact race the bug report
    // described. player_b is left completely untouched as the control.
    app.world_mut().despawn(enemy_a);
    app.world_mut().get_mut::<PlayerTarget>(player_a).unwrap().0 = None;

    // Must not panic — see the doc comment above for why a log-level assertion isn't feasible
    // here, and why this end-state check is still the meaningful part of this test.
    app.update();

    let remaining: Vec<Entity> = {
        let mut q = app.world_mut().query::<(Entity, &TrackingTarget)>();
        q.iter(app.world()).map(|(e, _)| e).collect()
    };
    assert_eq!(
        remaining.len(), 1,
        "player_a's ring must be gone (cleared target) and no new ring spawned for player_a; \
         player_b's ring must be the only one left"
    );
    let owners: Vec<Entity> = {
        let mut q = app.world_mut().query::<&TrackingTarget>();
        q.iter(app.world()).map(|t| t.owner).collect()
    };
    assert_eq!(owners, vec![player_b], "the surviving ring must belong to player_b, untouched by player_a's repro step");
}

/// System-architect review finding (Phase 1, `per_player_split_screen_targeting.md`):
/// `Action::SetTarget`/`ClearTarget` must mirror the primary player's `PlayerTarget`, not just
/// write the global `CurrentTarget` resource — otherwise the ring (`target_indicator_system`)
/// and `target_auto_clear_system`, both now driven by `PlayerTarget`, would silently stop
/// reacting to these two rule-driven actions. Drives the actual `Action` pipeline (not a direct
/// `PlayerTarget` mutation) so this test would have caught that regression.
#[test]
fn test_set_target_and_clear_target_actions_mirror_into_primary_player() {
    use ironhold_core::capabilities::target_indicator::TrackingTarget;
    use ironhold_core::capabilities::player::{PlayerTarget, PlayerIndex};
    use ironhold_core::capabilities::action_bar::CurrentTarget;
    use ironhold_core::runtime::scene_manager::{LoadedTargetIndicator, ResolvedTargetIndicator};

    let mut app = setup_test_app();
    app.update();

    let player = app.world_mut().spawn((
        CharacterController {
            walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
            inputs: InputMap {
                forward: "KeyW".to_string(), backward: "KeyS".to_string(),
                left: "KeyA".to_string(), right: "KeyD".to_string(),
                strafe_left: "KeyQ".to_string(), strafe_right: "KeyE".to_string(),
                jump: "Space".to_string(), run: "ShiftLeft".to_string(),
                interact: "KeyF".to_string(), strafe_mouse_button: None,
                target_next: "Tab".to_string(), target_range: 30.0, gamepad_index: None, look_left: None, look_right: None, look_up: None, look_down: None,
                gamepad_jump: "South".to_string(), gamepad_run: "East".to_string(),
                gamepad_interact: "West".to_string(), gamepad_target_next: "North".to_string(),
                gamepad_deadzone: 0.15,
            },
            is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
            double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
            collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
        },
        PlayerTarget::default(),
        PlayerIndex(0),
    )).id();

    let target_entity = app.world_mut().spawn((
        SpawnId("enemy_01".to_string()),
        GlobalTransform::from_translation(Vec3::new(2.0, 0.0, 0.0)),
    )).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_01".to_string(), target_entity);

    app.world_mut().insert_resource(LoadedTargetIndicator(Some(ResolvedTargetIndicator {
        texture_path: "shared/textures/decals/ring_thick.png".to_string(),
        radius: 1.0,
        color: (0.3, 0.8, 1.0, 0.75),
        offset_y: 0.05,
        named_colors: std::collections::HashMap::new(),
    })));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SetTarget("enemy_01".to_string()));
    app.update();
    // action_executor_system and target_indicator_system have no relative ordering constraint,
    // so target_indicator_system may run before action_executor_system within this same frame —
    // its Changed<PlayerTarget> read wouldn't see the write until the following frame. One more
    // update lets it observe the change (same "despawn is deferred" pattern used elsewhere below).
    app.update();

    assert_eq!(
        app.world().get::<PlayerTarget>(player).unwrap().0.as_deref(), Some("enemy_01"),
        "Action::SetTarget must mirror into the primary player's PlayerTarget, not just CurrentTarget"
    );
    assert_eq!(app.world().resource::<CurrentTarget>().0.as_deref(), Some("enemy_01"));
    let ring_count = app.world_mut().query::<&TrackingTarget>().iter(app.world()).count();
    assert_eq!(ring_count, 1, "the ring must actually spawn via the real Action::SetTarget path, not just a direct PlayerTarget mutation");

    app.world_mut().resource_mut::<ActionQueue>().push(Action::ClearTarget);
    app.update();
    app.update(); // despawn is deferred via Commands

    assert_eq!(
        app.world().get::<PlayerTarget>(player).unwrap().0, None,
        "Action::ClearTarget must clear the primary player's PlayerTarget, not just CurrentTarget"
    );
    assert_eq!(app.world().resource::<CurrentTarget>().0, None);
    let ring_count_after = app.world_mut().query::<&TrackingTarget>().iter(app.world()).count();
    assert_eq!(ring_count_after, 0, "the ring must despawn via the real Action::ClearTarget path");
}

// Regression coverage for `planning/features/monster_corpse_loot.md` v1's debug-detective
// finding: `interactable_system` fires `entity.interacted` for every interactable within radius
// on one keypress, not just the nearest, so two nearby lootable entities can each queue their own
// `OpenContainer` in the same frame. Before the fix, `Action::OpenContainer` incremented
// `panels_open` unconditionally on every call — a second open while one was already active
// over-incremented a counter that only ever gets decremented once per `CloseContainer`,
// permanently suppressing interact/collectible-pickup/tab-targeting (all gated on
// `panels_open == 0`) until the next `LoadScene`.
#[test]
fn test_open_container_twice_without_close_does_not_double_count_panels_open() {
    use ironhold_core::capabilities::inventory::{ContainerPanelMarker, Inventory, LoadedInventoryUi};
    use ironhold_core::runtime::scene_manager::SpawnRegistry;

    let mut app = setup_test_app();

    // A single ContainerPanel UI node, as any project with lootable containers has.
    app.world_mut().spawn((ContainerPanelMarker { columns: 3, rows: 3, font_size: 14.0 }, Visibility::Hidden));

    // Two separate lootable entities (e.g. two corpses that both landed within interact range).
    let corpse_a = app.world_mut().spawn((SpawnId("corpse_a".to_string()), Inventory::new(6))).id();
    let corpse_b = app.world_mut().spawn((SpawnId("corpse_b".to_string()), Inventory::new(6))).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("corpse_a".to_string(), corpse_a);
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("corpse_b".to_string(), corpse_b);

    app.update();

    // Both OpenContainer calls land in the same frame, exactly like two interactables hit by one
    // interact keypress.
    {
        let mut queue = app.world_mut().resource_mut::<ActionQueue>();
        queue.push(Action::OpenContainer("corpse_a".to_string()));
        queue.push(Action::OpenContainer("corpse_b".to_string()));
    }
    app.update();

    assert_eq!(
        app.world().resource::<LoadedInventoryUi>().panels_open, 1,
        "a second OpenContainer while one container is already open must not double-count panels_open"
    );

    // A single matching CloseContainer must fully clear it back to 0 — proving the counter isn't
    // left permanently stuck above 0 from the earlier over-increment.
    app.world_mut().resource_mut::<ActionQueue>().push(Action::CloseContainer);
    app.update();

    assert_eq!(
        app.world().resource::<LoadedInventoryUi>().panels_open, 0,
        "one CloseContainer must fully release the panel opened by two OpenContainer calls"
    );
}

// Same class of bug as the OpenContainer test above, found in Action::OpenShop by the
// debug-detective investigation into the monster_corpse_loot.md v1 playtest report — two
// interactable merchants both in range of one interact press could each queue their own OpenShop
// in the same frame, over-incrementing panels_open exactly like the OpenContainer case.
#[test]
fn test_open_shop_twice_without_close_does_not_double_count_panels_open() {
    use ironhold_core::capabilities::inventory::{ShopPanelMarker, ShopEntriesContainerMarker, LoadedInventoryUi};
    use ironhold_core::runtime::scene_manager::{SpawnRegistry, PrefabKey, LoadedPrefabCatalog};
    use ironhold_core::schema::catalog::{PrefabCatalog, PrefabDef, PrefabKind, MerchantDef};
    use std::collections::HashMap;

    let mut app = setup_test_app();

    // Two merchant prefabs, each with their own (empty) stock — contents don't matter for this test.
    let mut prefabs = HashMap::new();
    for key in ["merchant_a", "merchant_b"] {
        prefabs.insert(key.to_string(), PrefabDef {
            kind: PrefabKind::Prop,
            merchant: Some(MerchantDef { stock: vec![], currency_stat: "gold".to_string() }),
            ..Default::default()
        });
    }
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog { schema_version: 2, prefabs }));

    // A single ShopPanel UI node with its entries container child, as any project with a shop has.
    let shop_panel = app.world_mut().spawn((ShopPanelMarker { font_size: 14.0 }, Visibility::Hidden)).id();
    app.world_mut().spawn((ShopEntriesContainerMarker, ChildOf(shop_panel)));

    // Two separate interactable merchants.
    let merchant_a = app.world_mut().spawn((SpawnId("merchant_a_01".to_string()), PrefabKey("merchant_a".to_string()))).id();
    let merchant_b = app.world_mut().spawn((SpawnId("merchant_b_01".to_string()), PrefabKey("merchant_b".to_string()))).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("merchant_a_01".to_string(), merchant_a);
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("merchant_b_01".to_string(), merchant_b);

    app.update();

    // Both OpenShop calls land in the same frame, exactly like two interactables hit by one
    // interact keypress.
    {
        let mut queue = app.world_mut().resource_mut::<ActionQueue>();
        queue.push(Action::OpenShop("merchant_a_01".to_string()));
        queue.push(Action::OpenShop("merchant_b_01".to_string()));
    }
    app.update();

    assert_eq!(
        app.world().resource::<LoadedInventoryUi>().panels_open, 1,
        "a second OpenShop while one shop is already open must not double-count panels_open"
    );

    app.world_mut().resource_mut::<ActionQueue>().push(Action::CloseShop);
    app.update();

    assert_eq!(
        app.world().resource::<LoadedInventoryUi>().panels_open, 0,
        "one CloseShop must fully release the panel opened by two OpenShop calls"
    );
}
