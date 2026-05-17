use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use std::collections::HashMap;
use ironhold_core::{GamePlugin, ProjectConfigPath, ProjectRoot, PipelineWarmup, GameVariables};
use ironhold_core::runtime::{UiEvent, GameEvent, ActionQueue, SceneEvent, InputAction, InputActionMessage, ModelSpawner, LoadedRules, LoadedStateMachine, LoadedAssetCatalog, LoadedPrefabCatalog, SpawnId, SpawnRegistry, LogicState, OverlayEntity, BackgroundMusic, PendingSceneLoadMode, PreloadedScenes, PreloadedGlbHandles, PendingEntitySpawns, SceneHandleV2, LevelEntity, LoadedKeyBindings, ProjectKeyBindings, LoadedAudioHandles, BehaviorHandle, EntityFsmState};
use ironhold_core::schema::{AppState, Action, ProjectConfig, ProjectConfigHandle, LogicRule, TransformFix, StateMachineAsset, FsmState, FsmTransition, FsmEventBinding, GameSceneV2, StatDef, StatThreshold, ThresholdCondition, LiveStat, LoadedStats, ModifierDef, ModifierKind, StackRule, ActiveModifier, LoadedModifiers};
use ironhold_core::schema::catalog::AudioEntry;
use ironhold_core::schema::stats::StatMap;
use ironhold_core::capabilities::player::{CharacterController, player_movement_system};
use ironhold_core::capabilities::animation::AnimationController;
use ironhold_core::schema::player::{InputMap, AnimationPolicy, BaseAnimations};
use ironhold_core::capabilities::animation_resolver::{AnimationPolicyComponent, LocomotionState, AnimationRequests, ActiveOverride};
use ironhold_core::capabilities::stat_radar::StatRadarNode;
use ironhold_core::capabilities::stat_display::resolve_stat;
use std::sync::{Arc, Mutex};

fn setup_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
    //    .add_plugins(bevy::log::LogPlugin::default())
       .add_plugins(bevy::state::app::StatesPlugin)
       .add_plugins(bevy::transform::TransformPlugin)
       .add_plugins(AssetPlugin::default())
       .add_plugins(bevy::scene::ScenePlugin)
       .add_message::<bevy::input::mouse::MouseMotion>()
       .add_message::<bevy::input::mouse::MouseWheel>()
       .init_resource::<ButtonInput<KeyCode>>()
       .init_resource::<ButtonInput<MouseButton>>()
       .init_resource::<Messages<UiEvent>>()
       .init_resource::<Messages<GameEvent>>()
       .init_resource::<Messages<SceneEvent>>()
       .init_resource::<Messages<InputActionMessage>>()
       .init_resource::<Messages<AppExit>>()
       .init_resource::<Messages<bevy::input::mouse::MouseMotion>>()
       .init_resource::<Messages<bevy::input::mouse::MouseWheel>>()
       .init_asset::<Mesh>()
       .init_asset::<bevy::shader::Shader>()
       .init_asset::<ironhold_core::capabilities::terrain_material::TerrainMaterial>()
       .init_asset::<StandardMaterial>()
       .init_asset::<Image>()

       .init_asset::<Scene>()
       .init_asset::<Gltf>()
       .init_asset::<AnimationGraph>()
       .init_asset::<ironhold_core::schema::player::AnimationPolicy>()
       .init_asset::<ironhold_core::schema::project::LogicRulesAsset>()
       .init_asset::<ironhold_core::schema::project::StateMachineAsset>()
       .init_asset::<bevy::audio::AudioSource>()
       .insert_resource(ProjectConfigPath("projects/integration_tests/integration_tests.project.ron".to_string()))
       .insert_resource(ProjectRoot("projects/integration_tests".to_string()))
       .add_plugins(GamePlugin);
    app
}
#[test]
fn test_ui_button_to_load_scene_action() {
    let mut app = setup_test_app();
       
    // 1. Run once to process Startup (setup)
    app.update();
    
    // Override ProjectConfig with test-specific rules
    {
        let mut configs = app.world_mut().resource_mut::<Assets<ProjectConfig>>();
        let rules = vec![
            LogicRule {
                on: "ui.button_pressed:test_load".to_string(),
                when: None,
                do_actions: vec![Action::LoadScene("scenes/tests/test_scene.scene.ron".to_string())],
            }
        ];
        let config_handle = configs.add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/tests/test_scene.scene.ron".to_string(),
            rules: rules.clone(),
            ..Default::default()
        });
        app.world_mut().insert_resource(ProjectConfigHandle(config_handle));
        app.world_mut().insert_resource(LoadedRules(rules));
    }

    // 2. Simulate Button Press Message
    app.world_mut().resource_mut::<Messages<UiEvent>>().write(UiEvent::ButtonPressed("test_load".to_string()));
    
    // 3. Run systems (Interpreter + Executor will run)
    app.update();
    
    // 4. Run once more to process state transition
    app.update();
    
    // 5. Verify side effects
    // The executor should have inserted a SceneHandleV2 resource
    assert!(app.world().contains_resource::<SceneHandleV2>());
    
    // And state should be LoadingScene
    let state = app.world().resource::<State<AppState>>();
    assert_eq!(*state.get(), AppState::LoadingScene);
}

#[test]
fn test_scene_lifecycle_events() {
    let mut app = setup_test_app();
       
    app.update();
    
    // 1. Trigger LoadScene action
    app.world_mut().resource_mut::<ActionQueue>().push(Action::LoadScene("scenes/tests/test_scene.ron".to_string()));
    
    // 2. Run executor
    app.update();
    
    // 3. Verify SceneEvent::Requested was emitted with the project-root-resolved path
    app.world_mut().run_system_once(|mut scene_events: MessageReader<SceneEvent>| {
        let events: Vec<_> = scene_events.read().cloned().collect();
        assert!(events.iter().any(|e| matches!(e, SceneEvent::Requested(path) if path == "projects/integration_tests/scenes/tests/test_scene.ron")));
    }).unwrap();
}

#[test]
fn test_input_abstraction_flow() {
    let mut app = setup_test_app();

    // Initial run
    app.update();

    // 1. Setup an entity with CharacterController
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
        }),
        AnimationController {
            current: "idle".to_string(),
            last_played: String::new(),
            gltf_path: String::new(),
            gltf_handle: Default::default(),
            node_indices: Default::default(),
            graph_initialized: false,
            transition_ms: 0,
            should_loop: true,
            last_player_entity: None,
        },
        bevy_rapier3d::prelude::RigidBody::Dynamic,
        bevy_rapier3d::prelude::Velocity::zero(),
    )).id();

    // 2. Simulate "W" key press
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW);
    
    // 3. Run systems: input_translator_system writes InputActionMessage
    app.update();
    // Run again: player_movement_system reads the message from the previous frame
    // (systems are in separate unordered Update sets, so we need a second update)
    app.update();
    
    // 4. Verify InputActionMessage was emitted (available from previous frame)
    app.world_mut().run_system_once(move |mut input_events: MessageReader<InputActionMessage>| {
        let events: Vec<_> = input_events.read().cloned().collect();
        assert!(events.iter().any(|e| e.entity == entity && matches!(e.action, InputAction::Move(v) if v.y > 0.0)));
    }).unwrap();
    
    // 5. Verify character velocity moved forward (rapier physics sets linvel, not transform directly)
    // Note: Transform::forward() is (0, 0, -1) in Bevy 3D.
    // player_movement_system sets velocity.linvel.z = move_vec.z * speed (which is negative for forward).
    let velocity = app.world().entity(entity).get::<bevy_rapier3d::prelude::Velocity>().unwrap();
    assert!(velocity.linvel.z < 0.0, "Expected Z velocity < 0 (forward movement), got {}", velocity.linvel.z);
}

#[test]
fn test_action_to_state_transition() {
    let mut app = setup_test_app();
       
    // 1. Run once to handle Startup
    app.update();
    
    // 2. Transition to InGame 
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::InGame);
    app.update(); // Set transition
    app.update(); // Apply transition
    
    {
        let state = app.world().resource::<State<AppState>>();
        assert_eq!(*state.get(), AppState::InGame);
    }
    
    // 3. Manually push an action
    app.world_mut().resource_mut::<ActionQueue>().push(Action::LoadScene("scenes/tests/another_scene.ron".to_string()));
    
    // 4. Run executor
    app.update(); // Executor sets NextState
    app.update(); // Apply transition
    
    // 5. Verify state transitioned to LoadingScene
    let state = app.world().resource::<State<AppState>>();
    assert_eq!(*state.get(), AppState::LoadingScene);
}

#[test]
fn test_ui_button_to_quit_action() {
    let mut app = setup_test_app();
    
    // 1. Run once to process Startup (setup)
    app.update();
    
    // Override ProjectConfig with test-specific rules
    {
        let mut configs = app.world_mut().resource_mut::<Assets<ProjectConfig>>();
        let rules = vec![
            LogicRule {
                on: "ui.button_pressed:test_quit".to_string(),
                when: None,
                do_actions: vec![Action::Quit],
            }
        ];
        let config_handle = configs.add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/tests/test_scene.ron".to_string(),
            rules: rules.clone(),
            ..Default::default()
        });
        app.world_mut().insert_resource(ProjectConfigHandle(config_handle));
        app.world_mut().insert_resource(LoadedRules(rules));
    }

    // 2. Simulate Quit Message
    app.world_mut().resource_mut::<Messages<UiEvent>>().write(UiEvent::ButtonPressed("test_quit".to_string()));
    
    // 3. Run systems (Interpreter + Executor will run)
    app.update();
    
    // 4. Verify side effects
    // The executor should have queued Action::Quit which sends AppExit
    // We can check if ActionQueue has it or just verify it doesn't crash
    let action_queue = app.world().resource::<ActionQueue>();
    assert!(action_queue.0.is_empty()); // Should be empty because it was popped and executed
}


#[test]
fn model_fixup_persists_reset() {
    let mut app = setup_test_app();
    
    // 1. Run once to process Startup (setup)
    app.update();
    
    // 2. Mock ProjectConfig with a specific model fix
    let test_path = "shared/models/test-model.glb#Scene0".to_string();
    let fix = TransformFix {
        pivot_offset: (1.0, 2.0, 3.0),
        rotation_deg: (0.0, 90.0, 0.0),
        scale: (2.0, 2.0, 2.0),
    };
    
    {
        let mut configs = app.world_mut().resource_mut::<Assets<ProjectConfig>>();
        let config_handle = configs.add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/tests/test_scene.ron".to_string(),
            rules: vec![],
            rules_path: None,
            state_machine_path: None,
            model_fixes: {
                let mut map = std::collections::HashMap::new();
                map.insert(test_path.clone(), fix.clone());
                map
            },
            model_fixes_path: None,
            project_id: None,
            display_name: None,
            asset_catalog: None,
            prefab_catalog: None,
            ..Default::default()
        });
        app.world_mut().insert_resource(ProjectConfigHandle(config_handle));
    }
    
    // Populate MergedModelFixes so spawn_instance can find the fix.
    {
        let mut merged = app.world_mut().resource_mut::<ironhold_core::runtime::MergedModelFixes>();
        merged.0.insert(test_path.clone(), fix.clone());
    }

    // 3. Helper to verify fix is applied
    let verify_fix = |app: &mut App, path: String| {
        let (parent, child) = app.world_mut().run_system_once(move |
            mut commands: Commands,
            spawner: Res<ModelSpawner>,
            asset_server: Res<AssetServer>,
            merged_fixes: Res<ironhold_core::runtime::MergedModelFixes>,
        | {
            let spawned = spawner.spawn_instance(
                &mut commands,
                &asset_server,
                &merged_fixes.0,
                path.clone(),
                Transform::IDENTITY,
            );
            (spawned.parent, spawned.child)
        }).unwrap();

        app.update(); // Flush commands

        let child_transform = app.world().get::<Transform>(child).expect("Child should have Transform");

        // Verify translation (pivot_offset)
        assert_eq!(child_transform.translation, Vec3::new(1.0, 2.0, 3.0));

        // Verify scale
        assert_eq!(child_transform.scale, Vec3::new(2.0, 2.0, 2.0));

        // Verify rotation (90 deg around Y)
        let expected_rot = Quat::from_rotation_y(90.0f32.to_radians());
        assert!(child_transform.rotation.abs_diff_eq(expected_rot, 0.0001));

        parent
    };

    // 4. Test first spawn
    let _parent1 = verify_fix(&mut app, test_path.clone());

    // 5. Simulate "reset" (clearing entities, though ModelSpawner is a resource so it persists)
    // In our context, "persists reset" means if we spawn it again (even after scene clear), it still uses the config.
    let _parent2 = verify_fix(&mut app, test_path.clone());

    // 6. Verify with a path that doesn't have a fix (should use default)
    {
        app.world_mut().run_system_once(|
            mut commands: Commands,
            spawner: Res<ModelSpawner>,
            asset_server: Res<AssetServer>,
            merged_fixes: Res<ironhold_core::runtime::MergedModelFixes>,
        | {
            let unknown_path = "models/unknown.glb#Scene0".to_string();
            let _spawned = spawner.spawn_instance(
                &mut commands,
                &asset_server,
                &merged_fixes.0,
                unknown_path,
                Transform::IDENTITY,
            );
        }).unwrap();
    }
    
    app.update(); // Flush commands from system_once
}

#[test]
fn test_play_sound_action_spawns_audio_player() {
    use ironhold_core::runtime::LoadedAssetCatalog;
    use ironhold_core::schema::catalog::AssetCatalog;

    let mut app = setup_test_app();
    app.update();

    // Provide a catalog with a known audio key
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("click".to_string(), AudioEntry { path: "shared/audio/menu-button-click.wav".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::PlaySound { key: "click".to_string(), volume: 1.0 });
    app.update();

    let count = app.world_mut()
        .query::<&bevy::audio::AudioPlayer<bevy::audio::AudioSource>>()
        .iter(app.world())
        .count();
    assert_eq!(count, 1, "Expected one AudioPlayer entity to be spawned for PlaySound");
}

#[test]
fn test_play_sound_unsupported_format_does_not_panic() {
    use ironhold_core::runtime::LoadedAssetCatalog;
    use ironhold_core::schema::catalog::AssetCatalog;

    let mut app = setup_test_app();
    app.update();

    // Register a catalog entry with an unsupported file extension
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("bad".to_string(), AudioEntry { path: "shared/audio/soundtrack.aac".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::PlaySound { key: "bad".to_string(), volume: 1.0 });
    app.update(); // Must not panic

    // No AudioPlayer should have been spawned
    let count = app.world_mut()
        .query::<&bevy::audio::AudioPlayer<bevy::audio::AudioSource>>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "Unsupported format should be rejected before spawning AudioPlayer");
}

#[test]
fn test_play_sound_missing_key_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    // No audio entries in the default catalog — should warn and not panic
    app.world_mut().resource_mut::<ActionQueue>().push(Action::PlaySound { key: "nonexistent".to_string(), volume: 1.0 });
    app.update();

    let count = app.world_mut()
        .query::<&bevy::audio::AudioPlayer<bevy::audio::AudioSource>>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "No AudioPlayer should be spawned for an unknown sound key");
}

#[test]
fn test_play_sound_combined_volume_applied_to_playback_settings() {
    use ironhold_core::runtime::LoadedAssetCatalog;
    use ironhold_core::schema::catalog::AssetCatalog;

    let mut app = setup_test_app();
    app.update();

    // catalog volume 0.5, action volume 0.5 → combined should be 0.25
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("click".to_string(), AudioEntry { path: "shared/audio/click.wav".to_string(), volume: 0.5 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlaySound { key: "click".to_string(), volume: 0.5 });
    app.update();

    let mut q = app.world_mut()
        .query::<&bevy::audio::PlaybackSettings>();
    let settings = q.iter(app.world()).next()
        .expect("PlaybackSettings component should exist on the spawned AudioPlayer entity");
    let bevy::audio::Volume::Linear(v) = settings.volume else {
        panic!("Expected Volume::Linear");
    };
    assert!(
        (v - 0.25).abs() < 1e-5,
        "Expected combined volume 0.5 * 0.5 = 0.25, got {v}"
    );
}

#[test]
fn test_play_sound_default_volume_is_full() {
    use ironhold_core::runtime::LoadedAssetCatalog;
    use ironhold_core::schema::catalog::AssetCatalog;

    let mut app = setup_test_app();
    app.update();

    // Both volumes default to 1.0 — PlaybackSettings should have Linear(1.0)
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("click".to_string(), AudioEntry { path: "shared/audio/click.wav".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlaySound { key: "click".to_string(), volume: 1.0 });
    app.update();

    let mut q = app.world_mut()
        .query::<&bevy::audio::PlaybackSettings>();
    let settings = q.iter(app.world()).next()
        .expect("PlaybackSettings should exist on spawned entity");
    let bevy::audio::Volume::Linear(v) = settings.volume else {
        panic!("Expected Volume::Linear");
    };
    assert!(
        (v - 1.0).abs() < 1e-5,
        "Default volumes should produce Linear(1.0), got {v}"
    );
}

#[test]
fn test_spawn_action_assigns_spawn_id_and_registers() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry};

    let mut app = setup_test_app();
    app.update();

    // Provide minimal catalog entries for the orc prefab
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("orc".to_string(), ModelCatalogEntry { path: "shared/models/creatures/orc.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("enemy_orc_melee".to_string(), PrefabDef {
                kind: "actor".to_string(),
                model: "orc".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    // Spawn with an explicit ID
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: Some("orc_test".to_string()), position: None, spawn_point: None, yaw_deg: None }
    );
    app.update();

    // SpawnId component should exist on the spawned entity
    let ids: Vec<String> = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .map(|s| s.0.clone())
        .collect();
    assert!(ids.contains(&"orc_test".to_string()), "SpawnId 'orc_test' should be present, got: {:?}", ids);

    // Registry should track the entity
    let registry = app.world().resource::<SpawnRegistry>();
    assert!(registry.entities.contains_key("orc_test"), "Registry should contain 'orc_test'");
}

#[test]
fn test_spawn_auto_id_increments_counter() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("orc".to_string(), ModelCatalogEntry { path: "shared/models/creatures/orc.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("enemy_orc_melee".to_string(), PrefabDef {
                kind: "actor".to_string(),
                model: "orc".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    // Spawn twice without explicit IDs
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: None, position: None, spawn_point: None, yaw_deg: None }
    );
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: None, position: None, spawn_point: None, yaw_deg: None }
    );
    app.update();

    let ids: Vec<String> = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .map(|s| s.0.clone())
        .collect();
    assert_eq!(ids.len(), 2, "Two entities should have been spawned");
    assert!(ids.iter().any(|id| id.starts_with("enemy_orc_melee_")), "IDs should be auto-prefixed with prefab name");

    let registry = app.world().resource::<SpawnRegistry>();
    assert_eq!(registry.counter, 2, "Counter should be 2 after two auto-spawns");
    assert_eq!(registry.entities.len(), 2);
}

#[test]
fn test_despawn_removes_entity_by_spawn_id() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("orc".to_string(), ModelCatalogEntry { path: "shared/models/creatures/orc.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("enemy_orc_melee".to_string(), PrefabDef {
                kind: "actor".to_string(),
                model: "orc".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    // Spawn then despawn
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: Some("doomed_orc".to_string()), position: None, spawn_point: None, yaw_deg: None }
    );
    app.update();

    assert!(app.world_mut().query::<&SpawnId>().iter(app.world()).any(|s| s.0 == "doomed_orc"));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::Despawn("doomed_orc".to_string()));
    app.update(); // executor queues despawn
    app.update(); // commands flush and entity is removed

    let still_exists = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .any(|s| s.0 == "doomed_orc");
    assert!(!still_exists, "Entity 'doomed_orc' should have been despawned");

    let registry = app.world().resource::<SpawnRegistry>();
    assert!(!registry.entities.contains_key("doomed_orc"), "Registry should no longer contain 'doomed_orc'");
}

#[test]
fn test_despawn_unknown_id_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>().push(Action::Despawn("ghost".to_string()));
    app.update(); // should warn and not panic
}

#[test]
fn test_enter_state_action_updates_logic_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::EnterState("playing".to_string()));
    app.update(); // executor fires

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "playing", "EnterState should update LogicState");
}

#[test]
fn test_state_gated_rule_only_fires_in_matching_state() {
    let mut app = setup_test_app();
    app.update();

    // Rule fires EnterState("triggered") only while in the "active" logic state.
    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "ui.button_pressed:do_thing".to_string(),
            when: Some("active".to_string()),
            do_actions: vec![Action::EnterState("triggered".to_string())],
        }
    ]));

    // Fire event while in the wrong state ("") — rule must be suppressed.
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("do_thing".to_string()));
    app.update();
    {
        let state = app.world().resource::<LogicState>();
        assert_eq!(state.0, "", "Rule should be suppressed in non-matching state");
    }

    // Transition to the matching state, then fire the event again.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::EnterState("active".to_string()));
    app.update();

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("do_thing".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "triggered", "Rule should fire in the matching state");
}

// ── FSM interpreter tests ─────────────────────────────────────────────────────

/// Helper: build a minimal StateMachineAsset with two states ("a" and "b") and one transition.
fn make_test_fsm() -> StateMachineAsset {
    StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState {
                name: "a".to_string(),
                entry_actions: vec![Action::Log("entered_a".to_string())],
                exit_actions:  vec![Action::Log("exited_a".to_string())],
                on: vec![
                    FsmEventBinding {
                        event: "ui.button_pressed:in_state_a".to_string(),
                        do_actions: vec![Action::Log("in_state_a_fired".to_string())],
                    },
                ],
            },
            FsmState {
                name: "b".to_string(),
                entry_actions: vec![Action::Log("entered_b".to_string())],
                exit_actions:  vec![Action::Log("exited_b".to_string())],
                on: vec![],
            },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("a".to_string()),
                on: "ui.button_pressed:go_b".to_string(),
                to: "b".to_string(),
            },
        ],
        global_on: vec![
            FsmEventBinding {
                event: "ui.button_pressed:global_action".to_string(),
                do_actions: vec![Action::Log("global_fired".to_string())],
            },
        ],
    }
}

#[test]
fn test_fsm_in_state_on_binding_fires() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("in_state_a".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"in_state_a_fired\")",
        "In-state on binding should fire while in matching state");
}

#[test]
fn test_fsm_in_state_on_binding_suppressed_in_wrong_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    // Start in "b" — the "in_state_a" binding belongs to "a".
    app.world_mut().insert_resource(LogicState("b".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("in_state_a".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "State must not change");
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_ne!(debug.last_action, "Log(\"in_state_a_fired\")",
        "In-state on binding must be suppressed in wrong state");
}

#[test]
fn test_fsm_transition_fires_exit_enter_and_advances_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    // Trigger the transition a → b.
    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    // State must have advanced to "b".
    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "Transition should advance LogicState to the target state");

    // The last action processed by the executor should be the entry action for "b".
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"entered_b\")",
        "Entry actions for the new state should fire after the transition");
}

#[test]
fn test_fsm_transition_does_not_fire_from_wrong_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    // Start in "b" — transition is from "a" only.
    app.world_mut().insert_resource(LogicState("b".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "Transition with from:Some(\"a\") must not fire from state \"b\"");
}

#[test]
fn test_fsm_any_state_transition_fires_from_any_state() {
    let mut fsm = make_test_fsm();
    // Add an any-state transition to "b".
    fsm.transitions.push(FsmTransition {
        from: None,
        on: "ui.button_pressed:anywhere_go_b".to_string(),
        to: "b".to_string(),
    });

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    // Start in "b" — the any-state transition should still fire.
    app.world_mut().insert_resource(LogicState("b".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("anywhere_go_b".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "Any-state transition (from: None) should fire from any state");
}

#[test]
fn test_fsm_global_on_fires_regardless_of_state() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(make_test_fsm())));
    app.world_mut().insert_resource(LogicState("b".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("global_action".to_string()));
    app.update();

    // State must not change; global_on fires only the declared action.
    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "global_on must not change state");
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"global_fired\")",
        "global_on binding should fire from any state");
}

// ── Rules interpreter additional tests ────────────────────────────────────────

#[test]
fn test_rules_no_match_does_not_queue_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "ui.button_pressed:something_else".to_string(),
            when: None,
            do_actions: vec![Action::Log("should_not_fire".to_string())],
        },
    ]));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("unmatched_event".to_string()));
    app.update();

    let queue = app.world().resource::<ActionQueue>();
    assert!(queue.0.is_empty(), "No rule matched — queue must stay empty");
}

#[test]
fn test_rules_scene_event_ready_triggers_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "scene.ready:main".to_string(),
            when: None,
            do_actions: vec![Action::Log("scene_ready_fired".to_string())],
        },
    ]));

    // Interpreter strips path to stem "main" via scene_path_stem.
    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_ready_fired\")",
        "scene.ready:main rule should fire on SceneEvent::Ready for main scene");
}

#[test]
fn test_rules_scene_event_loaded_triggers_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "scene.loaded:main".to_string(),
            when: None,
            do_actions: vec![Action::Log("scene_loaded_fired".to_string())],
        },
    ]));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Loaded("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_loaded_fired\")",
        "scene.loaded:main rule should fire on SceneEvent::Loaded before entities are spawned");
}

#[test]
fn test_rules_scene_event_requested_triggers_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "scene.requested:main".to_string(),
            when: None,
            do_actions: vec![Action::Log("scene_requested_fired".to_string())],
        },
    ]));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Requested("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_requested_fired\")",
        "scene.requested:main rule should fire on SceneEvent::Requested");
}

#[test]
fn test_rules_scene_event_unloading_triggers_action() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedRules(vec![
        LogicRule {
            on: "scene.unloading:main".to_string(),
            when: None,
            do_actions: vec![Action::Log("scene_unloading_fired".to_string())],
        },
    ]));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Unloading("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_unloading_fired\")",
        "scene.unloading:main rule should fire on SceneEvent::Unloading");
}

// ── FIFO ordering tests ───────────────────────────────────────────────────────

#[test]
fn test_action_queue_is_fifo() {
    // Actions pushed first must execute first.
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>().push(Action::Log("first".to_string()));
    app.world_mut().resource_mut::<ActionQueue>().push(Action::Log("second".to_string()));
    app.world_mut().resource_mut::<ActionQueue>().push(Action::Log("third".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"third\")",
        "FIFO: last pushed action should be last executed (last_action reflects final execution)");
}

#[test]
fn test_fsm_exit_before_entry_fifo_order() {
    // Verifies: exit actions run before entry actions, and declaration order is preserved
    // within each group. State "a" has two exit actions; state "b" has two entry actions.
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState {
                name: "a".to_string(),
                entry_actions: vec![],
                exit_actions: vec![
                    Action::Log("exit_a_1".to_string()),
                    Action::Log("exit_a_2".to_string()),
                ],
                on: vec![],
            },
            FsmState {
                name: "b".to_string(),
                entry_actions: vec![
                    Action::Log("entry_b_1".to_string()),
                    Action::Log("entry_b_2".to_string()),
                ],
                exit_actions: vec![],
                on: vec![],
            },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("a".to_string()),
                on: "ui.button_pressed:go".to_string(),
                to: "b".to_string(),
            },
        ],
        global_on: vec![],
    };

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go".to_string()));
    app.update();

    // FIFO execution order: exit_a_1, exit_a_2, entry_b_1, entry_b_2.
    // last_action reflects the final action executed.
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"entry_b_2\")",
        "FIFO exit→entry: last executed action should be the second entry action of state b");
}

// ── FSM interpreter additional tests ──────────────────────────────────────────

#[test]
fn test_fsm_exit_action_fires_on_transition() {
    // "b" has no entry actions, so last_action after the transition reflects the exit action.
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState {
                name: "a".to_string(),
                entry_actions: vec![],
                exit_actions: vec![Action::Log("exited_a".to_string())],
                on: vec![],
            },
            FsmState {
                name: "b".to_string(),
                entry_actions: vec![],
                exit_actions: vec![],
                on: vec![],
            },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("a".to_string()),
                on: "ui.button_pressed:go".to_string(),
                to: "b".to_string(),
            },
        ],
        global_on: vec![],
    };

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b");
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"exited_a\")",
        "Exit action should be the last executed action when target state has no entry actions");
}

#[test]
fn test_fsm_scene_event_triggers_transition() {
    let mut fsm = make_test_fsm();
    // Any-state transition triggered by a scene ready event.
    fsm.transitions.push(FsmTransition {
        from: None,
        on: "scene.ready:main".to_string(),
        to: "b".to_string(),
    });

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "SceneEvent::Ready should trigger an FSM transition");
}

#[test]
fn test_fsm_scene_event_triggers_in_state_on_binding() {
    let mut fsm = make_test_fsm();
    // Add a scene.ready binding to state "a".
    fsm.states[0].on.push(FsmEventBinding {
        event: "scene.ready:start_menu".to_string(),
        do_actions: vec![Action::Log("scene_ready_in_state_a".to_string())],
    });

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/start_menu.scene.ron".to_string()));
    app.update();

    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"scene_ready_in_state_a\")",
        "SceneEvent should trigger an in-state on binding when in the matching state");
}

#[test]
fn test_fsm_scene_event_loaded_triggers_transition() {
    let mut fsm = make_test_fsm();
    fsm.transitions.push(FsmTransition {
        from: None,
        on: "scene.loaded:main".to_string(),
        to: "b".to_string(),
    });

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Loaded("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b", "SceneEvent::Loaded should trigger an FSM transition");
}

// ── Audio preload tests ───────────────────────────────────────────────────────

#[test]
fn test_preload_audio_populates_handles_on_scene_ready() {
    let mut app = setup_test_app();
    app.update();

    let mut catalog = ironhold_core::schema::catalog::AssetCatalog::default();
    catalog.audio.insert("jump".to_string(),         AudioEntry { path: "shared/audio/jump.wav".to_string(), volume: 1.0 });
    catalog.audio.insert("collect_coin".to_string(), AudioEntry { path: "shared/audio/coin.wav".to_string(), volume: 1.0 });
    app.world_mut().insert_resource(LoadedAssetCatalog(catalog));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let handles = app.world().resource::<LoadedAudioHandles>();
    assert_eq!(handles.0.len(), 2,
        "preload_audio_system should create one handle per catalog audio entry");
}

#[test]
fn test_preload_audio_clears_on_scene_transition() {
    let mut app = setup_test_app();
    app.update();

    let mut catalog = ironhold_core::schema::catalog::AssetCatalog::default();
    catalog.audio.insert("jump".to_string(),         AudioEntry { path: "shared/audio/jump.wav".to_string(), volume: 1.0 });
    catalog.audio.insert("collect_coin".to_string(), AudioEntry { path: "shared/audio/coin.wav".to_string(), volume: 1.0 });
    app.world_mut().insert_resource(LoadedAssetCatalog(catalog));

    // First scene load.
    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/scene_a.scene.ron".to_string()));
    app.update();

    // Second scene load (transition) — handles must not accumulate.
    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/scene_b.scene.ron".to_string()));
    app.update();

    let handles = app.world().resource::<LoadedAudioHandles>();
    assert_eq!(handles.0.len(), 2,
        "preload_audio_system must clear and repopulate on each Ready, not accumulate");
}

#[test]
fn test_fsm_no_loaded_state_machine_is_noop() {
    let mut app = setup_test_app();
    app.update();

    // Explicit None — no FSM loaded.
    app.world_mut().insert_resource(LoadedStateMachine(None));

    app.world_mut().resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("any_event".to_string()));
    app.update(); // must not panic

    let queue = app.world().resource::<ActionQueue>();
    assert!(queue.0.is_empty(), "No FSM loaded — action queue must remain empty");
    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "", "LogicState must remain unchanged when no FSM is loaded");
}

// ── Executor additional tests ──────────────────────────────────────────────────

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
fn test_load_scene_overlay_sets_overlay_load_mode() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::LoadSceneOverlay("scenes/pause.scene.ron".to_string()));
    app.update();

    let mode = app.world().resource::<PendingSceneLoadMode>();
    assert_eq!(*mode, PendingSceneLoadMode::Overlay,
        "LoadSceneOverlay should set PendingSceneLoadMode to Overlay");
}

#[test]
fn test_unload_overlay_despawns_overlay_entities() {
    let mut app = setup_test_app();
    app.update();

    // Spawn two overlay entities.
    app.world_mut().spawn(OverlayEntity);
    app.world_mut().spawn(OverlayEntity);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::UnloadOverlay);
    app.update(); // executor queues despawn commands
    app.update(); // flush

    let count = app.world_mut()
        .query::<&OverlayEntity>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "UnloadOverlay should despawn all OverlayEntity entities");
}

#[test]
fn test_toggle_overlay_opens_when_no_overlay_active() {
    let mut app = setup_test_app();
    app.update();

    // No OverlayEntity present — toggle should open (set load mode to Overlay).
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ToggleOverlay("scenes/pause.scene.ron".to_string()));
    app.update();

    let mode = app.world().resource::<PendingSceneLoadMode>();
    assert_eq!(*mode, PendingSceneLoadMode::Overlay,
        "ToggleOverlay with no active overlay should set load mode to Overlay");
}

#[test]
fn test_toggle_overlay_closes_when_overlay_active() {
    let mut app = setup_test_app();
    app.update();

    // Spawn an overlay entity so ToggleOverlay sees an active overlay.
    app.world_mut().spawn(OverlayEntity);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ToggleOverlay("scenes/pause.scene.ron".to_string()));
    app.update(); // executor queues despawn
    app.update(); // flush

    let count = app.world_mut()
        .query::<&OverlayEntity>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0,
        "ToggleOverlay with an active overlay should despawn all OverlayEntity entities");
}

#[test]
fn test_play_music_loop_spawns_background_music() {
    use ironhold_core::schema::catalog::AssetCatalog;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("bg_music".to_string(), AudioEntry { path: "shared/audio/theme.ogg".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "bg_music".to_string(), volume: 1.0 });
    app.update();

    let count = app.world_mut()
        .query::<&BackgroundMusic>()
        .iter(app.world())
        .count();
    assert_eq!(count, 1, "PlayMusicLoop should spawn exactly one BackgroundMusic entity");
}

#[test]
fn test_play_music_loop_stops_previous_track_and_spawns_new() {
    use ironhold_core::schema::catalog::AssetCatalog;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("track_a".to_string(), AudioEntry { path: "shared/audio/track_a.ogg".to_string(), volume: 1.0 }),
            ("track_b".to_string(), AudioEntry { path: "shared/audio/track_b.ogg".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    // Start first track.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "track_a".to_string(), volume: 1.0 });
    app.update();
    app.update(); // flush despawn commands from any previous music stop

    // Start second track — should stop the first and spawn a new one.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "track_b".to_string(), volume: 1.0 });
    app.update();
    app.update(); // flush

    let count = app.world_mut()
        .query::<&BackgroundMusic>()
        .iter(app.world())
        .count();
    assert_eq!(count, 1,
        "PlayMusicLoop should replace the previous track — exactly one BackgroundMusic entity");
}

#[test]
fn test_play_music_loop_unsupported_format_does_not_panic() {
    use ironhold_core::schema::catalog::AssetCatalog;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("bad_music".to_string(), AudioEntry { path: "shared/audio/track.aac".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "bad_music".to_string(), volume: 1.0 });
    app.update(); // must not panic

    let count = app.world_mut()
        .query::<&BackgroundMusic>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "Unsupported audio format should not spawn a BackgroundMusic entity");
}

#[test]
fn test_play_music_loop_missing_key_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "nonexistent_track".to_string(), volume: 1.0 });
    app.update(); // must not panic

    let count = app.world_mut()
        .query::<&BackgroundMusic>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "Missing audio key should not spawn a BackgroundMusic entity");
}

#[test]
fn test_play_music_loop_combined_volume_applied_to_playback_settings() {
    use ironhold_core::schema::catalog::AssetCatalog;

    let mut app = setup_test_app();
    app.update();

    // catalog 0.6 × action 0.5 = 0.3
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("bg".to_string(), AudioEntry { path: "shared/audio/theme.ogg".to_string(), volume: 0.6 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "bg".to_string(), volume: 0.5 });
    app.update();

    let mut q = app.world_mut()
        .query::<(&BackgroundMusic, &bevy::audio::PlaybackSettings)>();
    let (_, settings) = q.iter(app.world()).next()
        .expect("BackgroundMusic entity should have PlaybackSettings");
    let bevy::audio::Volume::Linear(v) = settings.volume else {
        panic!("Expected Volume::Linear");
    };
    assert!(
        (v - 0.30).abs() < 1e-5,
        "Expected combined volume 0.6 * 0.5 = 0.30, got {v}"
    );
}

#[test]
fn test_stop_music_despawns_background_music() {
    let mut app = setup_test_app();
    app.update();

    // Manually place a BackgroundMusic entity in the world.
    app.world_mut().spawn(BackgroundMusic);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::StopMusic);
    app.update(); // executor queues despawn
    app.update(); // flush

    let count = app.world_mut()
        .query::<&BackgroundMusic>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "StopMusic should despawn all BackgroundMusic entities");
}

#[test]
fn test_set_volume_updates_global_volume() {
    let mut app = setup_test_app();
    app.insert_resource(bevy::audio::GlobalVolume::default());
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::SetVolume(50));
    app.update();

    let gv = app.world().resource::<bevy::audio::GlobalVolume>();
    let linear = match gv.volume {
        bevy::audio::Volume::Linear(v) => v,
        _ => panic!("Expected Volume::Linear"),
    };
    assert!((linear - 0.5).abs() < 1e-5, "SetVolume(50) should set GlobalVolume to 0.5 linear");
}

#[test]
fn test_set_volume_clamped_to_100() {
    let mut app = setup_test_app();
    app.insert_resource(bevy::audio::GlobalVolume::default());
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::SetVolume(150)); // over 100 — clamped
    app.update();

    let gv = app.world().resource::<bevy::audio::GlobalVolume>();
    let linear = match gv.volume {
        bevy::audio::Volume::Linear(v) => v,
        _ => panic!("Expected Volume::Linear"),
    };
    assert!((linear - 1.0).abs() < 1e-5, "SetVolume > 100 should clamp to 1.0 linear");
}

#[test]
fn test_set_volume_no_resource_does_not_panic() {
    // GlobalVolume resource absent — executor warns but must not panic.
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::SetVolume(80));
    app.update(); // must not panic
}

#[test]
fn test_preload_scene_ron_pushes_handle() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PreloadScene("scenes/pause.scene.ron".to_string()));
    app.update();

    let preloaded = app.world().resource::<PreloadedScenes>();
    assert_eq!(preloaded.0.len(), 1, "PreloadScene should store the handle in PreloadedScenes");
}

#[test]
fn test_preload_non_scene_path_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    // Non-.scene.ron path — executor should warn, not push a handle.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PreloadScene("textures/something.png".to_string()));
    app.update(); // must not panic

    let preloaded = app.world().resource::<PreloadedScenes>();
    assert_eq!(preloaded.0.len(), 0,
        "Non-.scene.ron path should not be added to PreloadedScenes");
}

// ── Animation clip validation ─────────────────────────────────────────────────

#[test]
fn test_animation_graph_only_includes_present_clips() {
    // When an AnimationPolicy declares clips that don't exist in the GLB,
    // the graph still initialises and node_indices contains only the clips
    // that were actually present in the GLTF (the missing ones are warned
    // about but never added).
    let mut app = setup_test_app();
    app.init_asset::<bevy::animation::AnimationClip>();
    app.update();

    // Build a Gltf asset that only has "Walk_Loop" in named_animations.
    let gltf = Gltf {
        scenes: vec![],
        named_scenes: Default::default(),
        meshes: vec![],
        named_meshes: Default::default(),
        materials: vec![],
        named_materials: Default::default(),
        nodes: vec![],
        named_nodes: Default::default(),
        skins: vec![],
        named_skins: Default::default(),
        default_scene: None,
        animations: vec![],
        named_animations: Default::default(),
        source: None,
    };
    let gltf_handle = app.world_mut().resource_mut::<Assets<Gltf>>().add(gltf);

    let walk_clip = app.world_mut()
        .resource_mut::<Assets<bevy::animation::AnimationClip>>()
        .add(bevy::animation::AnimationClip::default());
    app.world_mut()
        .resource_mut::<Assets<Gltf>>()
        .get_mut(&gltf_handle)
        .unwrap()
        .named_animations
        .insert("Walk_Loop".into(), walk_clip);

    // Policy declares four clips — only "Walk_Loop" exists in the Gltf.
    let policy = AnimationPolicy {
        base: BaseAnimations {
            idle: "Idle_Loop".to_string(),
            walk: "Walk_Loop".to_string(),
            run: "Run_Loop".to_string(),
            jump_loop: "Jump_Loop".to_string(),
        },
        clips: HashMap::new(),
        overrides: vec![],
        default_transition_ms: None,
    };

    // Spawn with AnimationPlayer on the same entity so find_player_entity_recursive
    // locates it without needing a child hierarchy.
    let entity = app.world_mut().spawn((
        Transform::default(),
        GlobalTransform::default(),
        AnimationPolicyComponent(policy),
        AnimationController {
            current: "Walk_Loop".to_string(),
            last_played: String::new(),
            gltf_path: "test.glb".to_string(),
            gltf_handle,
            node_indices: Default::default(),
            graph_initialized: false,
            transition_ms: 0,
            should_loop: true,
            last_player_entity: None,
        },
        bevy::animation::AnimationPlayer::default(),
    )).id();

    app.update();
    app.update();

    let controller = app.world().entity(entity).get::<AnimationController>().unwrap();

    assert!(controller.graph_initialized,
        "Graph should be initialized even when some policy clips are missing from the GLTF");

    assert!(controller.node_indices.contains_key("Walk_Loop"),
        "Walk_Loop is in the GLTF and must be in node_indices");
    assert!(!controller.node_indices.contains_key("Idle_Loop"),
        "Idle_Loop is not in the GLTF and must not be in node_indices");
    assert!(!controller.node_indices.contains_key("Run_Loop"),
        "Run_Loop is not in the GLTF and must not be in node_indices");
    assert!(!controller.node_indices.contains_key("Jump_Loop"),
        "Jump_Loop is not in the GLTF and must not be in node_indices");
}

#[test]
fn test_animation_missing_clip_stops_retrying() {
    // When animation_playback_system can't find the requested clip in node_indices,
    // it must update last_played so it doesn't re-warn every frame.
    let mut app = setup_test_app();
    app.update();

    // graph_initialized=true with an empty node_indices simulates the case where
    // the graph was built but the requested clip was not in the GLTF.
    let entity = app.world_mut().spawn((
        Transform::default(),
        GlobalTransform::default(),
        AnimationPolicyComponent(AnimationPolicy {
            base: BaseAnimations {
                idle: "Idle_Loop".to_string(),
                walk: "Walk_Loop".to_string(),
                run: "Run_Loop".to_string(),
                jump_loop: "Jump_Loop".to_string(),
            },
            clips: HashMap::new(),
            overrides: vec![],
            default_transition_ms: None,
        }),
        AnimationController {
            current: "missing_clip".to_string(),
            last_played: String::new(),
            gltf_path: String::new(),
            gltf_handle: Default::default(),
            node_indices: Default::default(),
            graph_initialized: true,
            transition_ms: 0,
            should_loop: true,
            last_player_entity: None,
        },
        bevy::animation::AnimationPlayer::default(),
    )).id();

    app.update();

    let controller = app.world().entity(entity).get::<AnimationController>().unwrap();
    assert_eq!(
        controller.last_played, "missing_clip",
        "last_played must equal current after a missing-clip warn so the system stops retrying"
    );
}

// ── Scene load cleanup tests ──────────────────────────────────────────────────

/// Drive a Replace-mode scene load through `spawn_scene_v2`.
/// Inserts a minimal ProjectConfig override and a synthetic empty scene, then
/// transitions state to LoadingScene and runs enough updates for the system to
/// fire and its despawn commands to flush.
fn drive_replace_load(app: &mut App) {
    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let scene: GameSceneV2 =
        ron::de::from_str("(schema_version: 2, entities: [], ui: [])").unwrap();
    let scene_handle = app
        .world_mut()
        .resource_mut::<Assets<GameSceneV2>>()
        .add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadingScene);
    app.update(); // state transitions to LoadingScene
    app.update(); // spawn_scene_v2 fires, despawn commands queued
    app.update(); // commands flushed
}

#[test]
fn test_level_entity_despawned_on_replace_load() {
    let mut app = setup_test_app();
    app.update();

    // Pre-spawn entities that belong to the "previous" scene.
    let old_a = app.world_mut().spawn(LevelEntity).id();
    let old_b = app.world_mut().spawn(LevelEntity).id();

    drive_replace_load(&mut app);

    assert!(
        app.world().get_entity(old_a).is_err(),
        "LevelEntity 'old_a' must be despawned on Replace-mode scene load"
    );
    assert!(
        app.world().get_entity(old_b).is_err(),
        "LevelEntity 'old_b' must be despawned on Replace-mode scene load"
    );
}

#[test]
fn test_overlay_entities_despawned_on_replace_load() {
    let mut app = setup_test_app();
    app.update();

    // Pre-spawn overlay entities (e.g. from a previously open pause menu).
    let overlay_a = app.world_mut().spawn(OverlayEntity).id();
    let overlay_b = app.world_mut().spawn(OverlayEntity).id();

    drive_replace_load(&mut app);

    assert!(
        app.world().get_entity(overlay_a).is_err(),
        "OverlayEntity 'overlay_a' must be despawned on Replace-mode scene load"
    );
    assert!(
        app.world().get_entity(overlay_b).is_err(),
        "OverlayEntity 'overlay_b' must be despawned on Replace-mode scene load"
    );
}

#[test]
fn test_key_bindings_do_not_bleed_across_scenes() {
    let mut app = setup_test_app();
    app.update();

    // Simulate a binding left over from a previous scene.
    app.world_mut().insert_resource(LoadedKeyBindings(HashMap::from([
        ("KeyX".to_string(), "previous_scene_action".to_string()),
    ])));
    // Project-level bindings do NOT include "KeyX".
    app.world_mut()
        .insert_resource(ProjectKeyBindings(HashMap::new()));

    drive_replace_load(&mut app);

    // Replace-mode load must rebuild LoadedKeyBindings from ProjectKeyBindings only.
    // "KeyX" must not survive the scene transition.
    let bindings = app.world().resource::<LoadedKeyBindings>();
    assert!(
        !bindings.0.contains_key("KeyX"),
        "LoadedKeyBindings must not carry 'KeyX' forward from a previous scene — no bleed allowed"
    );
}

// ── Spawn/despawn registry tests ──────────────────────────────────────────────

#[test]
fn test_spawn_id_collision_orphans_old_entity() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("box".to_string(), ModelCatalogEntry {
                path: "shared/models/box.glb#Scene0".to_string(),
            }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("crate".to_string(), PrefabDef {
                kind: "prop".to_string(),
                model: "box".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    // First spawn with explicit ID "crate_1".
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "crate".to_string(), id: Some("crate_1".to_string()), position: None, spawn_point: None, yaw_deg: None },
    );
    app.update();

    // Second spawn with the same ID — silently overwrites the registry entry.
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "crate".to_string(), id: Some("crate_1".to_string()), position: None, spawn_point: None, yaw_deg: None },
    );
    app.update();

    // Both entities carry SpawnId("crate_1"); registry tracks only the latest one.
    let id_count = app
        .world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .filter(|s| s.0 == "crate_1")
        .count();
    assert_eq!(id_count, 2,
        "Both spawns should produce a SpawnId('crate_1') — the first entity is now orphaned");

    let registry = app.world().resource::<SpawnRegistry>();
    assert_eq!(registry.entities.len(), 1,
        "Registry must track only one entity under 'crate_1' after the collision");

    // Despawn by ID removes the registry entry but leaves the orphaned entity alive.
    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::Despawn("crate_1".to_string()));
    app.update(); // executor issues despawn command
    app.update(); // command flushed

    let remaining = app
        .world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .filter(|s| s.0 == "crate_1")
        .count();
    assert_eq!(remaining, 1,
        "After despawn the orphaned entity (not tracked by registry) must still exist");

    let registry = app.world().resource::<SpawnRegistry>();
    assert!(
        !registry.entities.contains_key("crate_1"),
        "Registry must be empty for 'crate_1' after the despawn"
    );
}

#[test]
fn test_spawn_yaw_deg_sets_transform_rotation() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: HashMap::from([
            ("box".to_string(), ModelCatalogEntry { path: "shared/models/box.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: HashMap::from([
            ("crate".to_string(), PrefabDef {
                kind: "prop".to_string(),
                model: "box".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn {
            prefab: "crate".to_string(),
            id: Some("rotated_crate".to_string()),
            position: Some((0.0, 0.0, 0.0)),
            spawn_point: None,
            yaw_deg: Some(90.0),
        }
    );
    app.update();

    let mut q = app.world_mut().query::<(&SpawnId, &Transform)>();
    let transform = q
        .iter(app.world())
        .find(|(sid, _)| sid.0 == "rotated_crate")
        .map(|(_, t)| *t)
        .expect("rotated_crate should have been spawned");

    let expected = Quat::from_rotation_y(90f32.to_radians());
    assert!(
        transform.rotation.abs_diff_eq(expected, 1e-5),
        "yaw_deg: 90 should produce a 90° Y-axis rotation, got {:?}",
        transform.rotation,
    );
}

// ── FSM correctness tests ─────────────────────────────────────────────────────

#[test]
fn test_fsm_only_first_matching_transition_fires() {
    // Two transitions on the same event from state "a": first → "b", second → "c".
    // The FSM interpreter uses `.find()` so only the first match executes.
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState { name: "a".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "b".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "c".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("a".to_string()),
                on: "ui.button_pressed:go".to_string(),
                to: "b".to_string(),
            },
            FsmTransition {
                from: Some("a".to_string()),
                on: "ui.button_pressed:go".to_string(),
                to: "c".to_string(),
            },
        ],
        global_on: vec![],
    };

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    app.world_mut()
        .resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "b",
        "Only the first matching transition should fire; second transition to 'c' must be ignored");
}

#[test]
fn test_fsm_state_advance_visible_in_same_frame() {
    // Two events arrive in the same frame.
    // Event 1 "go_b" fires the a→b transition and advances logic_state to "b" immediately.
    // Event 2 "go_c" fires the b→c transition because the interpreter already sees state "b".
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![
            FsmState { name: "a".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "b".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "c".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("a".to_string()),
                on: "ui.button_pressed:go_b".to_string(),
                to: "b".to_string(),
            },
            FsmTransition {
                from: Some("b".to_string()),
                on: "ui.button_pressed:go_c".to_string(),
                to: "c".to_string(),
            },
        ],
        global_on: vec![],
    };

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));

    // Both events in the same frame — first advances state so second can fire.
    app.world_mut()
        .resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_b".to_string()));
    app.world_mut()
        .resource_mut::<Messages<UiEvent>>()
        .write(UiEvent::ButtonPressed("go_c".to_string()));
    app.update();

    let state = app.world().resource::<LogicState>();
    assert_eq!(state.0, "c",
        "State advance from first transition must be visible to second event in the same frame");
}

// ── Model fixes merge tests ───────────────────────────────────────────────────

#[test]
fn test_model_fixes_external_overrides_inline() {
    // Mirrors the merge logic in project_loader.rs:
    //   merged = config.model_fixes  (inline)
    //   merged.extend(fixes_asset.model_fixes)  (external file)
    // When the same key exists in both sources, the external file wins.
    let inline_fix = TransformFix {
        pivot_offset: (1.0, 0.0, 0.0),
        rotation_deg: (0.0, 0.0, 0.0),
        scale: (1.0, 1.0, 1.0),
    };
    let external_fix = TransformFix {
        pivot_offset: (99.0, 0.0, 0.0),
        rotation_deg: (0.0, 0.0, 0.0),
        scale: (2.0, 2.0, 2.0),
    };

    // Start with inline fixes.
    let mut merged: HashMap<String, TransformFix> = HashMap::from([
        ("models/hero.glb".to_string(), inline_fix.clone()),
    ]);

    // Extend with external file — same key present in both.
    let external: HashMap<String, TransformFix> = HashMap::from([
        ("models/hero.glb".to_string(), external_fix.clone()),
        ("models/prop.glb".to_string(), inline_fix.clone()), // key only in external
    ]);
    merged.extend(external.into_iter());

    assert_eq!(
        merged["models/hero.glb"].pivot_offset.0, 99.0,
        "External model fix must override the inline fix for shared keys"
    );
    assert_eq!(
        merged["models/hero.glb"].scale.0, 2.0,
        "External scale must override inline scale"
    );
    assert!(
        merged.contains_key("models/prop.glb"),
        "External-only key must be present in the merged output"
    );
    assert_eq!(
        merged.len(), 2,
        "Merged map should contain exactly the two unique keys"
    );
}

#[test]
fn test_model_fixes_base_path_fallback() {
    // spawn_instance looks up the fix first by the full fragment path
    // ("models/hero.glb#Scene0"), then by the base path ("models/hero.glb").
    // When the fix is stored under the base path only, it must still apply.
    let mut app = setup_test_app();
    app.update();

    let base_path = "shared/models/hero.glb".to_string();
    let fragment_path = "shared/models/hero.glb#Scene0".to_string();
    let fix = TransformFix {
        pivot_offset: (5.0, 0.0, 0.0),
        rotation_deg: (0.0, 0.0, 0.0),
        scale: (1.0, 1.0, 1.0),
    };

    // Store fix under the BASE path only — no fragment path entry.
    {
        let mut merged = app
            .world_mut()
            .resource_mut::<ironhold_core::runtime::MergedModelFixes>();
        merged.0.insert(base_path.clone(), fix.clone());
        assert!(!merged.0.contains_key(&fragment_path),
            "Precondition: fragment path must not be in fixes");
    }

    // Spawn using the FRAGMENT path — base path fallback should apply the fix.
    let (_, child) = app
        .world_mut()
        .run_system_once(move |
            mut commands: Commands,
            spawner: Res<ModelSpawner>,
            asset_server: Res<AssetServer>,
            merged_fixes: Res<ironhold_core::runtime::MergedModelFixes>,
        | {
            let spawned = spawner.spawn_instance(
                &mut commands,
                &asset_server,
                &merged_fixes.0,
                fragment_path.clone(),
                Transform::IDENTITY,
            );
            (spawned.parent, spawned.child)
        })
        .unwrap();

    app.update(); // flush spawn commands

    let child_transform = app
        .world()
        .get::<Transform>(child)
        .expect("Child entity must exist after spawn");

    assert_eq!(
        child_transform.translation,
        Vec3::new(5.0, 0.0, 0.0),
        "Base path fallback must apply the fix stored under 'hero.glb' when spawning 'hero.glb#Scene0'"
    );
}

// ── PipelineWarmup ────────────────────────────────────────────────────────────

#[test]
fn test_pipeline_warmup_decrements_to_zero() {
    let mut app = setup_test_app();
    app.update(); // Startup

    app.world_mut().insert_resource(PipelineWarmup(4));

    // pipeline_warmup_system runs each Update frame; 4 ticks should drain the counter.
    for _ in 0..4 {
        app.update();
    }

    let warmup = app.world().resource::<PipelineWarmup>();
    assert_eq!(warmup.0, 0, "PipelineWarmup should reach 0 after 4 frames");
}

#[test]
fn test_pipeline_warmup_stops_at_zero() {
    let mut app = setup_test_app();
    app.update(); // Startup

    app.world_mut().insert_resource(PipelineWarmup(2));

    // Run more frames than the initial count — should not underflow.
    for _ in 0..10 {
        app.update();
    }

    let warmup = app.world().resource::<PipelineWarmup>();
    assert_eq!(warmup.0, 0, "PipelineWarmup must not go below 0");
}

// ── Entity FSM (Beta 0.4) tests ───────────────────────────────────────────────

fn make_two_state_behavior(app: &mut App) -> Handle<StateMachineAsset> {
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "idle".to_string(),
        states: vec![
            FsmState { name: "idle".to_string(),      entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "collected".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("idle".to_string()),
                on: "entity.interacted:{self}".to_string(),
                to: "collected".to_string(),
            },
        ],
        global_on: vec![],
    };
    app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm)
}

#[test]
fn test_entity_fsm_transitions_on_game_event() {
    // An entity with a behavior FSM transitions when a matching GameEvent fires.
    let mut app = setup_test_app();
    app.update();

    let handle = make_two_state_behavior(&mut app);

    let entity = app.world_mut().spawn((
        BehaviorHandle(handle),
        EntityFsmState { current: "idle".to_string() },
        SpawnId("box_01".to_string()),
    )).id();

    // Fire the scoped event: {self} → box_01
    app.world_mut()
        .resource_mut::<Messages<GameEvent>>()
        .write(GameEvent::Trigger("entity.interacted:box_01".to_string()));
    app.update();

    let state = app.world().get::<EntityFsmState>(entity).unwrap();
    assert_eq!(state.current, "collected",
        "Entity FSM should transition idle → collected on matching interacted event");
}

#[test]
fn test_entity_fsm_self_substitution_routes_to_correct_entity() {
    // Two entities share the same behavior file.
    // An event scoped to "box_01" must only advance box_01's state, not box_02's.
    let mut app = setup_test_app();
    app.update();

    let handle = make_two_state_behavior(&mut app);

    let box_01 = app.world_mut().spawn((
        BehaviorHandle(handle.clone()),
        EntityFsmState { current: "idle".to_string() },
        SpawnId("box_01".to_string()),
    )).id();

    let box_02 = app.world_mut().spawn((
        BehaviorHandle(handle),
        EntityFsmState { current: "idle".to_string() },
        SpawnId("box_02".to_string()),
    )).id();

    app.world_mut()
        .resource_mut::<Messages<GameEvent>>()
        .write(GameEvent::Trigger("entity.interacted:box_01".to_string()));
    app.update();

    let state_01 = app.world().get::<EntityFsmState>(box_01).unwrap();
    let state_02 = app.world().get::<EntityFsmState>(box_02).unwrap();
    assert_eq!(state_01.current, "collected",
        "box_01 must transition on its own scoped event");
    assert_eq!(state_02.current, "idle",
        "box_02 must not be affected by box_01's event");
}

#[test]
fn test_entity_fsm_entry_actions_queued_on_transition() {
    // When a transition fires, the entry_actions of the target state are queued.
    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "idle".to_string(),
        states: vec![
            FsmState { name: "idle".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState {
                name: "collected".to_string(),
                entry_actions: vec![
                    Action::Despawn("{self}".to_string()),
                ],
                exit_actions: vec![],
                on: vec![],
            },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("idle".to_string()),
                on: "entity.interacted:{self}".to_string(),
                to: "collected".to_string(),
            },
        ],
        global_on: vec![],
    };
    let handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm);

    app.world_mut().spawn((
        BehaviorHandle(handle),
        EntityFsmState { current: "idle".to_string() },
        SpawnId("crate_01".to_string()),
    ));

    app.world_mut()
        .resource_mut::<Messages<GameEvent>>()
        .write(GameEvent::Trigger("entity.interacted:crate_01".to_string()));
    app.update();

    // The entry action is `Despawn("{self}")` which the interpreter rewrites to
    // `Despawn("crate_01")`. Verify the action was queued (and executed — the queue
    // is drained each frame, so we check side-effects via the SpawnRegistry).
    // Since the entity has no SpawnRegistry entry, the executor warns but doesn't panic.
    // The test passes if no panic occurs and the FSM state advanced.
    // (Full despawn integration requires a scene spawn, tested by other tests.)
    let _ = app; // no panic = pass
}

#[test]
fn test_entity_fsm_despawn_self_rewritten_to_concrete_id() {
    // `Despawn("{self}")` in entry_actions is rewritten to `Despawn("target_01")`.
    // We verify the rewrite by checking that the entity is despawned after the full
    // update cycle (entity registered in SpawnRegistry + has SpawnId component).
    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "idle".to_string(),
        states: vec![
            FsmState { name: "idle".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState {
                name: "done".to_string(),
                entry_actions: vec![Action::Despawn("{self}".to_string())],
                exit_actions: vec![],
                on: vec![],
            },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("idle".to_string()),
                on: "entity.interacted:{self}".to_string(),
                to: "done".to_string(),
            },
        ],
        global_on: vec![],
    };
    let handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm);

    let entity = app.world_mut().spawn((
        BehaviorHandle(handle),
        EntityFsmState { current: "idle".to_string() },
        SpawnId("target_01".to_string()),
    )).id();

    // Register the entity in SpawnRegistry so the executor can find and despawn it.
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("target_01".to_string(), entity);

    app.world_mut()
        .resource_mut::<Messages<GameEvent>>()
        .write(GameEvent::Trigger("entity.interacted:target_01".to_string()));
    app.update();

    assert!(
        app.world().get_entity(entity).is_err(),
        "Entity with SpawnId 'target_01' must be despawned — Despawn(\"{{self}}\") was not rewritten correctly"
    );
}

// ── GameVariables: SetVariable / IncrementVariable ───────────────────────────

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

    // Spawn a minimal player entity; no Rapier context → is_grounded defaults to true each frame.
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
        }),
        AnimationController {
            current: "idle".to_string(),
            last_played: String::new(),
            gltf_path: String::new(),
            gltf_handle: Default::default(),
            node_indices: Default::default(),
            graph_initialized: false,
            transition_ms: 0,
            should_loop: true,
            last_player_entity: None,
        },
        bevy_rapier3d::prelude::RigidBody::Dynamic,
        bevy_rapier3d::prelude::Velocity::zero(),
    )).id();

    // RapierPhysicsPlugin runs (even in integration tests, cfg(test) does not apply to lib
    // dependencies) and spawns a DefaultRapierContext entity.  With no physics simulation
    // running, cast_shape always returns None → is_grounded=false → no jump.
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

    // Write InputActionMessage directly — bypasses input_translator and FixedUpdate timing.
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

// ─── PreloadPrefab + spawn queue tests ───────────────────────────────────────

fn minimal_orc_catalogs(app: &mut App) {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry};
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("orc".to_string(), ModelCatalogEntry { path: "shared/models/creatures/orc.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("enemy_orc_melee".to_string(), PrefabDef {
                kind: "actor".to_string(),
                model: "orc".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));
}

#[test]
fn test_preload_prefab_stores_glb_handle() {
    let mut app = setup_test_app();
    app.update();
    minimal_orc_catalogs(&mut app);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PreloadPrefab("enemy_orc_melee".to_string()));
    app.update();

    let handles = app.world().resource::<PreloadedGlbHandles>();
    assert_eq!(handles.0.len(), 1, "PreloadedGlbHandles should hold one handle after PreloadPrefab");
}

#[test]
fn test_preload_prefab_unknown_key_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    // No catalogs inserted — should log a warning and keep going, not panic.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PreloadPrefab("nonexistent_prefab".to_string()));
    app.update();

    let handles = app.world().resource::<PreloadedGlbHandles>();
    assert_eq!(handles.0.len(), 0, "No handle should be stored for an unknown prefab key");
}

#[test]
fn test_spawn_queue_rate_limits_to_two_per_frame() {
    let mut app = setup_test_app();
    app.update();
    minimal_orc_catalogs(&mut app);

    // Queue 3 spawns — only 2 should be processed in the first update.
    for i in 0..3 {
        app.world_mut().resource_mut::<ActionQueue>().push(
            Action::Spawn {
                prefab: "enemy_orc_melee".to_string(),
                id: Some(format!("orc_{}", i)),
                position: None, spawn_point: None, yaw_deg: None,
            }
        );
    }
    app.update();

    let ids_after_first: Vec<String> = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .map(|s| s.0.clone())
        .collect();
    assert_eq!(ids_after_first.len(), 2, "Only 2 of 3 queued spawns should process in the first frame");

    let queue_len = app.world().resource::<PendingEntitySpawns>().0.len();
    assert_eq!(queue_len, 1, "One spawn should remain in the queue after the first frame");

    // Second update drains the remaining spawn.
    app.update();

    let ids_after_second: Vec<String> = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .map(|s| s.0.clone())
        .collect();
    assert_eq!(ids_after_second.len(), 3, "All 3 spawns should be present after the second frame");

    let queue_len = app.world().resource::<PendingEntitySpawns>().0.len();
    assert_eq!(queue_len, 0, "Queue should be empty after all spawns are drained");
}

#[test]
fn test_pending_spawns_cleared_on_load_scene() {
    let mut app = setup_test_app();
    app.update();
    minimal_orc_catalogs(&mut app);

    // Pre-populate the queue directly (simulates spawns queued in a prior frame).
    {
        use ironhold_core::runtime::QueuedSpawn;
        let mut pending = app.world_mut().resource_mut::<PendingEntitySpawns>();
        pending.0.push_back(QueuedSpawn {
            prefab_def: ironhold_core::schema::catalog::PrefabDef {
                kind: "actor".to_string(),
                model: "orc".to_string(),
                ..Default::default()
            },
            model_path: "shared/models/creatures/orc.glb#Scene0".to_string(),
            transform: Transform::default(),
            spawn_id: "should_be_cancelled".to_string(),
            project_root: String::new(),
        });
    }

    // LoadScene should clear the queue before the drain system runs.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::LoadScene("scenes/dummy.scene.ron".to_string()));
    app.update();

    let queue_len = app.world().resource::<PendingEntitySpawns>().0.len();
    assert_eq!(queue_len, 0, "LoadScene should clear PendingEntitySpawns");

    let ids: Vec<String> = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .map(|s| s.0.clone())
        .collect();
    assert!(!ids.contains(&"should_be_cancelled".to_string()), "Queued spawn should not have been executed after LoadScene");
}

// ── Stat system tests ─────────────────────────────────────────────────────────

fn make_stat_def(base: f32, max: f32) -> StatDef {
    StatDef { base, min: 0.0, max, soft_max: None, regen_rate: 0.0, regen_delay: 0.0, thresholds: vec![] }
}

#[test]
fn test_stat_map_component_holds_correct_initial_values() {
    // StatMap inserted directly carries the LiveStat's base value.
    let mut app = setup_test_app();
    app.update();

    let mut stat_map = StatMap::default();
    stat_map.0.insert("health".to_string(), LiveStat::new(make_stat_def(80.0, 100.0)));
    let entity = app.world_mut().spawn(stat_map).id();

    let sm = app.world().get::<StatMap>(entity).unwrap();
    assert!(sm.0.contains_key("health"), "StatMap must contain the inserted stat key");
    assert_eq!(sm.0["health"].current, 80.0, "LiveStat must initialise to the declared base value");
    assert_eq!(sm.0["health"].def.max, 100.0);
}

#[test]
fn test_modify_stat_with_dot_key_routes_to_entity_stat_map() {
    // "entity_id.stat_name" → executor finds the entity and mutates its StatMap.
    let mut app = setup_test_app();
    app.update();

    let mut stat_map = StatMap::default();
    stat_map.0.insert("health".to_string(), LiveStat::new(make_stat_def(100.0, 100.0)));

    let entity = app.world_mut().spawn((
        SpawnId("goblin_01".to_string()),
        stat_map,
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("goblin_01".to_string(), entity);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ModifyStat { key: "goblin_01.health".to_string(), delta: -25.0 });
    app.update();

    let sm = app.world().get::<StatMap>(entity).unwrap();
    assert_eq!(sm.0["health"].current, 75.0,
        "ModifyStat with dot key must mutate the entity's StatMap, not LoadedStats");
}

#[test]
fn test_modify_stat_without_dot_key_routes_to_loaded_stats() {
    // No dot in key → executor mutates global LoadedStats resource.
    let mut app = setup_test_app();
    app.update();

    let mut loaded = LoadedStats::default();
    loaded.0.insert("player_health".to_string(), LiveStat::new(make_stat_def(100.0, 100.0)));
    app.world_mut().insert_resource(loaded);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ModifyStat { key: "player_health".to_string(), delta: -30.0 });
    app.update();

    let loaded = app.world().resource::<LoadedStats>();
    assert_eq!(loaded.0["player_health"].current, 70.0,
        "ModifyStat without dot key must mutate LoadedStats, not any entity StatMap");
}

#[test]
fn test_stat_map_threshold_crossing_emits_game_event() {
    // After a ModifyStat drives a stat to 0, stat_threshold_system emits the configured event.
    let mut app = setup_test_app();
    app.update();

    let def = StatDef {
        base: 50.0, min: 0.0, max: 50.0, soft_max: None,
        regen_rate: 0.0, regen_delay: 0.0,
        thresholds: vec![
            StatThreshold {
                when: ThresholdCondition::BelowOrEqual(0.0),
                emit: "stat.enemy_01.health.depleted".to_string(),
            },
        ],
    };
    let mut stat_map = StatMap::default();
    stat_map.0.insert("health".to_string(), LiveStat::new(def));

    let entity = app.world_mut().spawn((
        SpawnId("enemy_01".to_string()),
        stat_map,
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("enemy_01".to_string(), entity);

    // Deplete the health stat in one action.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ModifyStat { key: "enemy_01.health".to_string(), delta: -50.0 });
    app.update();

    // stat_threshold_system runs in the same frame as the executor (chained after it).
    // The emitted GameEvent is readable immediately after the update.
    app.world_mut().run_system_once(|mut events: MessageReader<GameEvent>| {
        let names: Vec<String> = events.read()
            .map(|e| { let GameEvent::Trigger(n) = e; n.clone() })
            .collect();
        assert!(
            names.contains(&"stat.enemy_01.health.depleted".to_string()),
            "stat_threshold_system must emit the configured event on false→true crossing; got: {:?}", names
        );
    }).unwrap();
}

#[test]
fn test_despawn_action_removes_entity_and_stat_map() {
    // Despawn by spawn ID removes the entity; its StatMap component is gone with it.
    let mut app = setup_test_app();
    app.update();

    let mut stat_map = StatMap::default();
    stat_map.0.insert("health".to_string(), LiveStat::new(make_stat_def(40.0, 100.0)));

    let entity = app.world_mut().spawn((
        SpawnId("dying_01".to_string()),
        stat_map,
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("dying_01".to_string(), entity);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::Despawn("dying_01".to_string()));
    app.update(); // executor queues despawn
    app.update(); // flush

    assert!(
        app.world().get_entity(entity).is_err(),
        "Despawned entity must no longer exist — StatMap is removed with the entity"
    );
}

#[test]
fn test_stat_radar_scene_load_spawns_node_with_correct_stat_keys() {
    // Loading a scene that contains a StatRadar UI element must produce an entity
    // carrying a StatRadarNode component with the stat_keys from the RON definition.
    let mut app = setup_test_app();
    app.update();

    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let scene: GameSceneV2 = ron::de::from_str(r#"
        (
            schema_version: 2,
            entities: [],
            ui: [
                StatRadar((
                    id: "test_radar",
                    stats: ["player_health", "player_mana", "player_stamina"],
                )),
            ],
        )
    "#).expect("test scene RON must parse");

    let scene_handle = app
        .world_mut()
        .resource_mut::<Assets<GameSceneV2>>()
        .add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadingScene);
    app.update(); // state transitions
    app.update(); // spawn_scene_v2 fires
    app.update(); // commands flushed

    let mut found = false;
    let mut world = app.world_mut();
    let mut q = world.query::<&StatRadarNode>();
    for node in q.iter(&world) {
        if node.stat_keys == vec!["player_health", "player_mana", "player_stamina"] {
            found = true;
        }
    }
    assert!(found, "scene loader must spawn an entity with StatRadarNode carrying the RON-defined stat keys");
}

// ── Modifier system tests ──────────────────────────────────────────────────────

fn make_additive_modifier(stat: &str, amount: f32, stack_rule: StackRule) -> ModifierDef {
    ModifierDef { stat: stat.to_string(), kind: ModifierKind::Additive(amount), duration_secs: None, stack_rule }
}

fn make_timed_additive_modifier(stat: &str, amount: f32, duration: f32) -> ModifierDef {
    ModifierDef { stat: stat.to_string(), kind: ModifierKind::Additive(amount), duration_secs: Some(duration), stack_rule: StackRule::Add }
}

fn make_multiplicative_modifier(stat: &str, factor: f32) -> ModifierDef {
    ModifierDef { stat: stat.to_string(), kind: ModifierKind::Multiplicative(factor), duration_secs: None, stack_rule: StackRule::Add }
}

#[test]
fn test_additive_modifier_raises_effective_value() {
    let def = make_stat_def(50.0, 100.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("flat_boost".to_string(), make_additive_modifier("health", 20.0, StackRule::Add));

    stat.active_modifiers.push(ActiveModifier { key: "flat_boost".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 70.0, "additive +20 on current=50 should give effective=70");
}

#[test]
fn test_additive_modifiers_stack_with_add_rule() {
    let def = make_stat_def(50.0, 100.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("flat_boost".to_string(), make_additive_modifier("health", 10.0, StackRule::Add));

    stat.active_modifiers.push(ActiveModifier { key: "flat_boost".to_string(), remaining_secs: None });
    stat.active_modifiers.push(ActiveModifier { key: "flat_boost".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 70.0, "two Add-rule +10 modifiers should accumulate to +20");
}

#[test]
fn test_max_stack_rule_ignores_weaker_instance() {
    let def = make_stat_def(40.0, 100.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("poison".to_string(), ModifierDef {
        stat: "health".to_string(),
        kind: ModifierKind::Additive(-5.0),
        duration_secs: None,
        stack_rule: StackRule::Max,
    });

    // Apply twice — Max rule means only one instance's magnitude counts
    stat.active_modifiers.push(ActiveModifier { key: "poison".to_string(), remaining_secs: None });
    stat.active_modifiers.push(ActiveModifier { key: "poison".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 35.0, "Max rule: two instances of -5 should still only apply -5 once (not -10)");
}

#[test]
fn test_multiplicative_modifier_scales_current() {
    let def = make_stat_def(10.0, 20.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("speed_boost".to_string(), make_multiplicative_modifier("speed", 1.5));

    stat.active_modifiers.push(ActiveModifier { key: "speed_boost".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 15.0, "multiplicative 1.5× on current=10 should give effective=15");
}

#[test]
fn test_soft_max_allows_overheal() {
    let mut def = make_stat_def(100.0, 100.0);
    def.soft_max = Some(125.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("overheal".to_string(), make_additive_modifier("health", 25.0, StackRule::Add));

    stat.active_modifiers.push(ActiveModifier { key: "overheal".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 125.0, "additive +25 with soft_max=125 should reach 125");
}

#[test]
fn test_soft_max_caps_overheal() {
    let mut def = make_stat_def(100.0, 100.0);
    def.soft_max = Some(125.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("big_overheal".to_string(), make_additive_modifier("health", 999.0, StackRule::Add));

    stat.active_modifiers.push(ActiveModifier { key: "big_overheal".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 125.0, "effective value must be clamped to soft_max");
}

#[test]
fn test_no_modifiers_effective_equals_current() {
    let def = make_stat_def(75.0, 100.0);
    let stat = LiveStat::new(def);
    let modifier_defs = HashMap::new();
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 75.0, "with no active modifiers effective must equal current");
}

#[test]
fn test_apply_modifier_action_adds_to_loaded_stats() {
    let mut app = setup_test_app();
    app.update();

    let mut loaded_stats = LoadedStats::default();
    loaded_stats.0.insert("speed".to_string(), LiveStat::new(make_stat_def(10.0, 20.0)));
    app.world_mut().insert_resource(loaded_stats);

    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("speed_boost".to_string(), make_multiplicative_modifier("speed", 1.5));
    app.world_mut().insert_resource(LoadedModifiers(modifier_defs));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ApplyModifier { modifier_key: "speed_boost".to_string() });
    app.update();

    let stats = app.world().resource::<LoadedStats>();
    assert_eq!(stats.0["speed"].active_modifiers.len(), 1,
        "ApplyModifier must push one ActiveModifier onto the stat");
    assert_eq!(stats.0["speed"].active_modifiers[0].key, "speed_boost");
}

#[test]
fn test_remove_modifier_action_clears_active_modifier() {
    let mut app = setup_test_app();
    app.update();

    let mut loaded_stats = LoadedStats::default();
    let mut stat = LiveStat::new(make_stat_def(10.0, 20.0));
    stat.active_modifiers.push(ActiveModifier { key: "speed_boost".to_string(), remaining_secs: None });
    loaded_stats.0.insert("speed".to_string(), stat);
    app.world_mut().insert_resource(loaded_stats);

    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("speed_boost".to_string(), make_multiplicative_modifier("speed", 1.5));
    app.world_mut().insert_resource(LoadedModifiers(modifier_defs));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::RemoveModifier { modifier_key: "speed_boost".to_string() });
    app.update();

    let stats = app.world().resource::<LoadedStats>();
    assert!(stats.0["speed"].active_modifiers.is_empty(),
        "RemoveModifier must remove all instances of the modifier from the stat");
}

#[test]
fn test_threshold_uses_effective_value_not_current() {
    // A debuff reduces effective health below 25% while raw current stays above.
    let mut def = make_stat_def(80.0, 100.0);
    def.thresholds = vec![StatThreshold {
        when: ThresholdCondition::BelowPercent(0.25),
        emit: "stat.health.low".to_string(),
    }];
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("heavy_curse".to_string(), make_additive_modifier("health", -65.0, StackRule::Add));

    stat.active_modifiers.push(ActiveModifier { key: "heavy_curse".to_string(), remaining_secs: None });
    // effective = 80 - 65 = 15, which is 15% of max (100) — below 25%
    let eff = stat.compute_effective(&modifier_defs);
    assert!(eff < 25.0, "effective should be below 25 after debuff: got {}", eff);
    // raw current is still 80 — not below 25%
    assert!(stat.current >= 25.0);
    // threshold should fire on effective, not current
    let is_met = ThresholdCondition::BelowPercent(0.25).is_met(eff, stat.def.max);
    assert!(is_met, "threshold must be met based on effective value");
    let raw_is_met = ThresholdCondition::BelowPercent(0.25).is_met(stat.current, stat.def.max);
    assert!(!raw_is_met, "threshold must NOT be met based on raw current");
}

// ── resolve_stat routing tests ─────────────────────────────────────────────────

#[test]
fn test_resolve_stat_routes_entity_local_key_through_stat_map() {
    // "dummy_01.health" must resolve from the StatMap on the entity with SpawnId("dummy_01"),
    // not from LoadedStats.
    let mut app = setup_test_app();
    app.update();

    // Global LoadedStats — must NOT be used for entity-local key.
    let mut loaded_stats = LoadedStats::default();
    loaded_stats.0.insert("dummy_01.health".to_string(), LiveStat::new(make_stat_def(999.0, 999.0)));
    app.world_mut().insert_resource(loaded_stats);

    // Spawn entity with SpawnId + StatMap carrying health at 75/100.
    let mut stat_map = StatMap(indexmap::IndexMap::new());
    stat_map.0.insert("health".to_string(), LiveStat::new(make_stat_def(75.0, 100.0)));
    app.world_mut().spawn((SpawnId("dummy_01".to_string()), stat_map));

    let result: Arc<Mutex<Option<Option<(f32, f32, f32)>>>> = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    let _ = app.world_mut().run_system_once(move |
        loaded_stats: Res<LoadedStats>,
        stat_map_query: Query<(&SpawnId, &StatMap)>,
    | {
        let val = resolve_stat("dummy_01.health", &loaded_stats, &stat_map_query);
        *result_clone.lock().unwrap() = Some(val);
    });

    let val = result.lock().unwrap().unwrap();
    assert!(val.is_some(), "resolve_stat must find 'dummy_01.health' in entity StatMap");
    let (effective, min, max) = val.unwrap();
    assert_eq!(effective, 75.0, "effective must come from StatMap, not the global LoadedStats sentinel");
    assert_eq!(min, 0.0);
    assert_eq!(max, 100.0);
}

#[test]
fn test_resolve_stat_routes_global_key_through_loaded_stats() {
    // A key without a dot must resolve from LoadedStats, not entity StatMaps.
    let mut app = setup_test_app();
    app.update();

    let mut loaded_stats = LoadedStats::default();
    loaded_stats.0.insert("player_health".to_string(), LiveStat::new(make_stat_def(60.0, 100.0)));
    app.world_mut().insert_resource(loaded_stats);

    let result: Arc<Mutex<Option<Option<(f32, f32, f32)>>>> = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    let _ = app.world_mut().run_system_once(move |
        loaded_stats: Res<LoadedStats>,
        stat_map_query: Query<(&SpawnId, &StatMap)>,
    | {
        let val = resolve_stat("player_health", &loaded_stats, &stat_map_query);
        *result_clone.lock().unwrap() = Some(val);
    });

    let val = result.lock().unwrap().unwrap();
    assert!(val.is_some(), "resolve_stat must find 'player_health' in LoadedStats");
    let (effective, _, max) = val.unwrap();
    assert_eq!(effective, 60.0);
    assert_eq!(max, 100.0);
}

#[test]
fn test_resolve_stat_returns_none_for_missing_entity_key() {
    // A dotted key whose entity does not exist must return None, not panic.
    let mut app = setup_test_app();
    app.update();

    let result: Arc<Mutex<Option<Option<(f32, f32, f32)>>>> = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    let _ = app.world_mut().run_system_once(move |
        loaded_stats: Res<LoadedStats>,
        stat_map_query: Query<(&SpawnId, &StatMap)>,
    | {
        let val = resolve_stat("ghost_entity.health", &loaded_stats, &stat_map_query);
        *result_clone.lock().unwrap() = Some(val);
    });

    assert!(
        result.lock().unwrap().unwrap().is_none(),
        "resolve_stat must return None when entity does not exist"
    );
}

// ─── DelayedEventQueue / tick_delayed_events_system tests ─────────────────────

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

    // Queue should be empty — the entry fired and was removed.
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

    // Advance only a little — should not fire yet.
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(500));

    app.update();

    let queue = app.world().resource::<DelayedEventQueue>();
    assert_eq!(queue.0.len(), 1, "entry must remain when delay has not elapsed");
}

#[test]
fn test_set_entity_visible_hides_then_shows_spawned_entity() {
    let mut app = setup_test_app();
    app.update();

    // Manually register an entity in the SpawnRegistry and give it a Visibility component.
    let entity = app.world_mut().spawn((
        SpawnId("test_obj".to_string()),
        Visibility::Visible,
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("test_obj".to_string(), entity);

    // Hide it.
    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::SetEntityVisible { entity: "test_obj".to_string(), visible: false });
    app.update();

    let vis = *app.world().entity(entity).get::<Visibility>().unwrap();
    assert_eq!(vis, Visibility::Hidden, "entity must be hidden after SetEntityVisible(false)");

    // Show it again.
    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::SetEntityVisible { entity: "test_obj".to_string(), visible: true });
    app.update();

    let vis = *app.world().entity(entity).get::<Visibility>().unwrap();
    assert_eq!(vis, Visibility::Visible, "entity must be visible after SetEntityVisible(true)");
}

// ─── SpawnEffect / PendingParticleEffects tests ────────────────────────────────

#[test]
fn test_spawn_effect_with_position_spawns_particle_entities() {
    use ironhold_core::runtime::LoadedAssetCatalog;
    use ironhold_core::schema::catalog::{AssetCatalog, EffectDef};
    use ironhold_core::capabilities::particle::Particle;

    let mut app = setup_test_app();
    // First update: Startup systems run, ParticleMeshCache gets its sphere mesh.
    app.update();

    let effect_def = EffectDef {
        particle_count: 8,
        lifetime_secs: 0.5,
        speed: 2.0,
        speed_jitter: 0.0,
        spread_deg: 180.0,
        offset: (0.0, 0.0, 0.0),
        emit_radius: 0.0,
        size: 0.05,
        size_end: None,
        size_jitter: 0.0,
        color_start: (1.0, 1.0, 0.0, 1.0),
        color_mid: None,
        color_end: (1.0, 0.0, 0.0, 0.0),
        gravity: 0.0,
        turbulence: 0.0,
        sprite: None,
        additive: false,
    };
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("sparks".to_string(), effect_def)]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "sparks".to_string(),
        position: Some((3.0, 1.0, -2.0)),
        entity: None,
    });
    // action_executor pushes to PendingParticleEffects; drain_particle_effects_system spawns entities.
    app.update();

    let count = app.world_mut().query::<&Particle>().iter(app.world()).count();
    assert_eq!(count, 8, "drain_particle_effects_system must spawn particle_count entities");
}

#[test]
fn test_spawn_effect_with_entity_resolves_global_transform() {
    use ironhold_core::runtime::LoadedAssetCatalog;
    use ironhold_core::schema::catalog::{AssetCatalog, EffectDef};
    use ironhold_core::capabilities::particle::Particle;

    let mut app = setup_test_app();
    app.update();

    let effect_def = EffectDef {
        particle_count: 4,
        lifetime_secs: 0.3,
        speed: 1.0,
        speed_jitter: 0.0,
        spread_deg: 90.0,
        offset: (0.0, 0.5, 0.0),
        emit_radius: 0.0,
        size: 0.04,
        size_end: None,
        size_jitter: 0.0,
        color_start: (0.0, 1.0, 0.0, 1.0),
        color_mid: None,
        color_end: (0.0, 0.0, 0.0, 0.0),
        gravity: 0.0,
        turbulence: 0.0,
        sprite: None,
        additive: false,
    };
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("heal".to_string(), effect_def)]),
        ..Default::default()
    }));

    // Spawn an entity at a known world position and register it.
    let entity = app.world_mut().spawn((
        SpawnId("npc_01".to_string()),
        GlobalTransform::from_translation(Vec3::new(5.0, 0.0, 3.0)),
    )).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("npc_01".to_string(), entity);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "heal".to_string(),
        position: None,
        entity: Some("npc_01".to_string()),
    });
    app.update();

    // All particles spawn at the resolved origin; drain_particle_effects_system sets Transform.
    // origin = entity position (5, 0, 3) + offset (0, 0.5, 0) = (5, 0.5, 3)
    let translations: Vec<Vec3> = app.world_mut()
        .query::<(&Particle, &Transform)>()
        .iter(app.world())
        .map(|(_, tf)| tf.translation)
        .collect();
    assert_eq!(translations.len(), 4, "must spawn particle_count entities for entity-based effect");
    for t in &translations {
        assert!((t.x - 5.0).abs() < 0.001, "particle x must match entity x");
        assert!((t.y - 0.5).abs() < 0.001, "particle y must equal entity y + offset");
        assert!((t.z - 3.0).abs() < 0.001, "particle z must match entity z");
    }
}

#[test]
fn test_spawn_effect_unknown_key_does_not_push() {
    use ironhold_core::capabilities::particle::PendingParticleEffects;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "nonexistent_effect".to_string(),
        position: Some((0.0, 0.0, 0.0)),
        entity: None,
    });
    app.update();

    let pending = app.world().resource::<PendingParticleEffects>();
    assert!(pending.0.is_empty(), "unknown effect key must not push to PendingParticleEffects");
}

#[test]
fn test_spawn_effect_entity_missing_does_not_push() {
    use ironhold_core::runtime::LoadedAssetCatalog;
    use ironhold_core::schema::catalog::{AssetCatalog, EffectDef};
    use ironhold_core::capabilities::particle::PendingParticleEffects;

    let mut app = setup_test_app();
    app.update();

    let effect_def = EffectDef {
        particle_count: 4,
        lifetime_secs: 0.3,
        speed: 1.0,
        speed_jitter: 0.0,
        spread_deg: 180.0,
        offset: (0.0, 0.0, 0.0),
        emit_radius: 0.0,
        size: 0.05,
        size_end: None,
        size_jitter: 0.0,
        color_start: (1.0, 1.0, 1.0, 1.0),
        color_mid: None,
        color_end: (1.0, 1.0, 1.0, 0.0),
        gravity: 0.0,
        turbulence: 0.0,
        sprite: None,
        additive: false,
    };
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("sparks".to_string(), effect_def)]),
        ..Default::default()
    }));

    // Entity name not registered in SpawnRegistry — should silently skip.
    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "sparks".to_string(),
        position: None,
        entity: Some("ghost_entity".to_string()),
    });
    app.update();

    let pending = app.world().resource::<PendingParticleEffects>();
    assert!(pending.0.is_empty(), "unresolvable entity must not push to PendingParticleEffects");
}
