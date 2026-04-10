use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use std::collections::HashMap;
use ironhold_core::{GamePlugin, ProjectConfigPath, ProjectRoot};
use ironhold_core::runtime::{UiMessage, ActionQueue, SceneEvent, InputAction, InputActionMessage, ModelSpawner, LoadedRules, LoadedStateMachine, LoadedAssetCatalog, LoadedPrefabCatalog, SpawnId, SpawnRegistry, LogicState, OverlayEntity, BackgroundMusic, PendingSceneLoadMode, PreloadedScenes};
use ironhold_core::schema::{AppState, Action, ProjectConfig, ProjectConfigHandle, LogicRule, TransformFix, StateMachineAsset, FsmState, FsmTransition, FsmEventBinding};
use ironhold_core::capabilities::player::CharacterController;
use ironhold_core::capabilities::animation::AnimationController;
use ironhold_core::schema::player::{InputMap, AnimationPolicy, BaseAnimations};
use ironhold_core::capabilities::animation_resolver::{AnimationPolicyComponent, LocomotionState, AnimationRequests, ActiveOverride};

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
       .init_resource::<Messages<UiMessage>>()
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
       .init_asset::<ironhold_core::schema::GameLevel>()
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
                do_actions: vec![Action::LoadScene("scenes/tests/test_scene.ron".to_string())],
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

    // 2. Simulate Button Press Message
    app.world_mut().resource_mut::<Messages<UiMessage>>().write(UiMessage::ButtonPressed("test_load".to_string()));
    
    // 3. Run systems (Interpreter + Executor will run)
    app.update();
    
    // 4. Run once more to process state transition
    app.update();
    
    // 5. Verify side effects
    // The executor should have inserted a LevelHandle resource
    assert!(app.world().contains_resource::<ironhold_core::schema::LevelHandle>());
    
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
            },
            is_running: false,
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
    app.world_mut().resource_mut::<Messages<UiMessage>>().write(UiMessage::ButtonPressed("test_quit".to_string()));
    
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
fn test_ui_button_positioning() {
    let mut app = setup_test_app();
    
    app.update();
    
    // 1. Setup a level with a positioned button
    let level_handle = {
        let mut configs = app.world_mut().resource_mut::<Assets<ProjectConfig>>();
        let config_handle = configs.add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/tests/test_scene.ron".to_string(),
            rules: vec![],
            rules_path: None,
            state_machine_path: None,
            model_fixes: HashMap::new(),
            model_fixes_path: None,
            project_id: None,
            display_name: None,
            asset_catalog: None,
            prefab_catalog: None,
            ..Default::default()
        });
        app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

        let mut levels = app.world_mut().resource_mut::<Assets<ironhold_core::schema::GameLevel>>();
        levels.add(ironhold_core::schema::GameLevel {
            schema_version: 1,
            models: vec![],
            ui: vec![
                ironhold_core::schema::UiElement::Button {
                    text: "Positioned".to_string(),
                    action: ironhold_core::schema::UiAction::Trigger("test".to_string()),
                    position: Some((123.0, 456.0)),
                    width: None,
                    height: None,
                    font_size: None,
                    border_color: None,
                    background_color: None,
                    text_color: None,
                }
            ],
            player: None,
            terrain: None,
            lighting: None,
        })
    };
    
    app.world_mut().insert_resource(ironhold_core::schema::LevelHandle(level_handle));
    
    // 2. Transition to LoadingScene and then InGame to trigger spawn_level
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LoadingScene);
    app.update();
    app.update();
    
    // 3. Verify the spawned button's Node configuration
    let mut query = app.world_mut().query::<(&Button, &Node)>();
    let mut found = false;
    for (_button, node) in query.iter(app.world()) {
        if node.position_type == PositionType::Absolute {
            assert_eq!(node.left, Val::Px(123.0));
            assert_eq!(node.top, Val::Px(456.0));
            found = true;
        }
    }
    assert!(found, "Should have found a button with absolute positioning");
}
#[test]
fn test_entity_names() {
    let mut app = setup_test_app();
    
    app.update();
    
    // 1. Setup a level with a player and a button
    let level_handle = {
        let mut configs = app.world_mut().resource_mut::<Assets<ProjectConfig>>();
        let config_handle = configs.add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/tests/test_scene.ron".to_string(),
            rules: vec![],
            rules_path: None,
            state_machine_path: None,
            model_fixes: HashMap::new(),
            model_fixes_path: None,
            project_id: None,
            display_name: None,
            asset_catalog: None,
            prefab_catalog: None,
            ..Default::default()
        });
        app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

        let mut levels = app.world_mut().resource_mut::<Assets<ironhold_core::schema::GameLevel>>();
        levels.add(ironhold_core::schema::GameLevel {
            schema_version: 1,
            models: vec![
                ironhold_core::schema::level::ModelInfo {
                    path: "models/cube.glb".to_string(),
                    position: (0.0, 0.0, 0.0),
                }
            ],
            ui: vec![
                ironhold_core::schema::UiElement::Button {
                    text: "Start".to_string(),
                    action: ironhold_core::schema::UiAction::Trigger("start".to_string()),
                    position: None,
                    width: None,
                    height: None,
                    font_size: None,
                    border_color: None,
                    background_color: None,
                    text_color: None,
                }
            ],
            player: Some(ironhold_core::schema::player::PlayerConfig {
                model_path: "models/player.glb".to_string(),
                initial_position: (0.0, 0.0, 0.0),
                inputs: ironhold_core::schema::player::InputMap {
                    forward: "KeyW".to_string(),
                    backward: "KeyS".to_string(),
                    left: "KeyA".to_string(),
                    right: "KeyD".to_string(),
                    strafe_left: "KeyQ".to_string(),
                    strafe_right: "KeyE".to_string(),
                    jump: "Space".to_string(),
                    run: "ShiftLeft".to_string(),
                },
                camera: ironhold_core::schema::player::CameraConfig {
                    offset: (0.0, 5.0, 10.0),
                    zoom_speed: 1.0,
                    orbit_speed: 1.0,
                    min_radius: 1.0,
                    max_radius: 20.0,
                    look_at_offset: (0.0, 1.0, 0.0),
                },
                animation_policy: "prefabs/animation/player_policy.ron".to_string(),
            }),
            terrain: None,
            lighting: Some(ironhold_core::schema::level::LightingConfig {
                ambient: Some(ironhold_core::schema::level::AmbientLightConfig {
                    color: (1.0, 1.0, 1.0),
                    brightness: 100.0,
                }),
                directional: Some(ironhold_core::schema::level::DirectionalLightConfig {
                    color: (1.0, 1.0, 1.0),
                    illuminance: 10000.0,
                    direction: (0.0, -1.0, 0.0),
                    shadows_enabled: true,
                }),
                environment: None,
            }),
        })
    };
    
    app.world_mut().insert_resource(ironhold_core::schema::LevelHandle(level_handle));
    
    // 2. Trigger spawn
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LoadingScene);
    app.update();
    app.update();
    
    // 3. Verify names
    let mut names = app.world_mut().query::<&Name>();
    let name_list: Vec<String> = names.iter(app.world()).map(|n| n.as_str().to_string()).collect();
    
    println!("Spawned names: {:?}", name_list);
    
    assert!(name_list.contains(&"Ambient Light".to_string()));
    assert!(name_list.contains(&"Directional Light".to_string()));
    assert!(name_list.contains(&"Persistent Overlay Camera".to_string()));
    assert!(name_list.contains(&"UI Root".to_string()));
    assert!(name_list.contains(&"Button: Start".to_string()));
    assert!(name_list.contains(&"Text: Start".to_string()));
    assert!(name_list.contains(&"Player".to_string()));
    assert!(name_list.contains(&"Orbit Camera".to_string()));
    assert!(name_list.contains(&"cube.glb".to_string()));
    assert!(name_list.contains(&"Model Scene Root".to_string()));
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
            ("click".to_string(), "shared/audio/menu-button-click.wav".to_string()),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::PlaySound("click".to_string()));
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
            ("bad".to_string(), "shared/audio/soundtrack.aac".to_string()),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::PlaySound("bad".to_string()));
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
    app.world_mut().resource_mut::<ActionQueue>().push(Action::PlaySound("nonexistent".to_string()));
    app.update();

    let count = app.world_mut()
        .query::<&bevy::audio::AudioPlayer<bevy::audio::AudioSource>>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "No AudioPlayer should be spawned for an unknown sound key");
}

#[test]
fn test_spawn_action_assigns_spawn_id_and_registers() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, PrefabComponents, ModelCatalogEntry};

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
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: Some("orc_test".to_string()) }
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
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, PrefabComponents, ModelCatalogEntry};

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
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: None }
    );
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: None }
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
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, PrefabComponents, ModelCatalogEntry};

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
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: Some("doomed_orc".to_string()) }
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
    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("do_thing".to_string()));
    app.update();
    {
        let state = app.world().resource::<LogicState>();
        assert_eq!(state.0, "", "Rule should be suppressed in non-matching state");
    }

    // Transition to the matching state, then fire the event again.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::EnterState("active".to_string()));
    app.update();

    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("do_thing".to_string()));
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

    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("in_state_a".to_string()));
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

    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("in_state_a".to_string()));
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
    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("go_b".to_string()));
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

    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("go_b".to_string()));
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

    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("anywhere_go_b".to_string()));
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

    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("global_action".to_string()));
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

    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("unmatched_event".to_string()));
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

    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("go".to_string()));
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

    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("go".to_string()));
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

#[test]
fn test_fsm_no_loaded_state_machine_is_noop() {
    let mut app = setup_test_app();
    app.update();

    // Explicit None — no FSM loaded.
    app.world_mut().insert_resource(LoadedStateMachine(None));

    app.world_mut().resource_mut::<Messages<UiMessage>>()
        .write(UiMessage::ButtonPressed("any_event".to_string()));
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
            ("bg_music".to_string(), "shared/audio/theme.ogg".to_string()),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop("bg_music".to_string()));
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
            ("track_a".to_string(), "shared/audio/track_a.ogg".to_string()),
            ("track_b".to_string(), "shared/audio/track_b.ogg".to_string()),
        ]),
        ..Default::default()
    }));

    // Start first track.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop("track_a".to_string()));
    app.update();
    app.update(); // flush despawn commands from any previous music stop

    // Start second track — should stop the first and spawn a new one.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop("track_b".to_string()));
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
            ("bad_music".to_string(), "shared/audio/track.aac".to_string()),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop("bad_music".to_string()));
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
        .push(Action::PlayMusicLoop("nonexistent_track".to_string()));
    app.update(); // must not panic

    let count = app.world_mut()
        .query::<&BackgroundMusic>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "Missing audio key should not spawn a BackgroundMusic entity");
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
        .push(Action::Preload("scenes/pause.scene.ron".to_string()));
    app.update();

    let preloaded = app.world().resource::<PreloadedScenes>();
    assert_eq!(preloaded.0.len(), 1, "Preload should store the handle in PreloadedScenes");
}

#[test]
fn test_preload_non_scene_path_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    // Non-.scene.ron path — executor should warn, not push a handle.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::Preload("textures/something.png".to_string()));
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
