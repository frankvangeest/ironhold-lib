use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use std::collections::HashMap;
use ironhold_core::{GamePlugin, ProjectConfigPath, ProjectRoot};
use ironhold_core::runtime::{UiMessage, ActionQueue, SceneEvent, InputAction, InputActionMessage, ModelSpawner, LoadedRules, LoadedAssetCatalog, LoadedPrefabCatalog, SpawnId, SpawnRegistry};
use ironhold_core::schema::{AppState, Action, ProjectConfig, ProjectConfigHandle, LogicRule, TransformFix};
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
            rules_path: None,
            model_fixes: HashMap::new(),
            model_fixes_path: None,
            project_id: None,
            display_name: None,
            asset_catalog: None,
            prefab_catalog: None,
            global_environment: None,
            global_key_bindings: std::collections::HashMap::new(),
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
            rules_path: None,
            model_fixes: HashMap::new(),
            model_fixes_path: None,
            project_id: None,
            display_name: None,
            asset_catalog: None,
            prefab_catalog: None,
            global_environment: None,
            global_key_bindings: std::collections::HashMap::new(),
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
            global_environment: None,
            global_key_bindings: std::collections::HashMap::new(),
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
            model_fixes: HashMap::new(),
            model_fixes_path: None,
            project_id: None,
            display_name: None,
            asset_catalog: None,
            prefab_catalog: None,
            global_environment: None,
            global_key_bindings: std::collections::HashMap::new(),
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
            model_fixes: HashMap::new(),
            model_fixes_path: None,
            project_id: None,
            display_name: None,
            asset_catalog: None,
            prefab_catalog: None,
            global_environment: None,
            global_key_bindings: std::collections::HashMap::new(),
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
                animation_policy: None,
                material: None,
                components: PrefabComponents::default(),
            }),
        ]),
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
                animation_policy: None,
                material: None,
                components: PrefabComponents::default(),
            }),
        ]),
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
                animation_policy: None,
                material: None,
                components: PrefabComponents::default(),
            }),
        ]),
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
