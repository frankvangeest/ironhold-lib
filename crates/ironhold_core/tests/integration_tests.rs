use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use ironhold_core::{GamePlugin, ProjectConfigPath};
use ironhold_core::runtime::{UiMessage, Action, ActionQueue, SceneEvent, InputAction, InputActionMessage};
use ironhold_core::schema::AppState;
use ironhold_core::capabilities::player::CharacterController;
use ironhold_core::capabilities::animation::AnimationController;
use ironhold_core::schema::player::{InputMap, AnimationMap};

#[test]
fn test_ui_button_to_load_scene_action() {
    let mut app = App::new();
    
    app.add_plugins(MinimalPlugins)
       .add_plugins(bevy::state::app::StatesPlugin)
       .add_plugins(AssetPlugin::default())
       .add_message::<bevy::input::mouse::MouseMotion>()
       .add_message::<bevy::input::mouse::MouseWheel>()
       .init_resource::<ButtonInput<KeyCode>>()
       .init_resource::<ButtonInput<MouseButton>>()
       .init_resource::<Assets<Mesh>>()
       .init_resource::<Assets<StandardMaterial>>()
       .init_resource::<Assets<Gltf>>()
       .init_resource::<Assets<AnimationGraph>>()
       .insert_resource(ProjectConfigPath("project.ron".to_string()))
       .add_plugins(GamePlugin);
       
    // 1. Run once to process Startup (setup)
    app.update();
    
    // 2. Simulate Button Press Message
    app.world_mut().resource_mut::<Messages<UiMessage>>().write(UiMessage::ButtonPressed("scenes/tests/test_scene.ron".to_string()));
    
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
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
       .add_plugins(bevy::state::app::StatesPlugin)
       .add_plugins(AssetPlugin::default())
       .add_message::<bevy::input::mouse::MouseMotion>()
       .add_message::<bevy::input::mouse::MouseWheel>()
       .init_resource::<ButtonInput<KeyCode>>()
       .init_resource::<ButtonInput<MouseButton>>()
       .init_resource::<Assets<Mesh>>()
       .init_resource::<Assets<StandardMaterial>>()
       .init_resource::<Assets<Gltf>>()
       .init_resource::<Assets<AnimationGraph>>()
       .insert_resource(ProjectConfigPath("project.ron".to_string()))
       .add_plugins(GamePlugin);
       
    app.update();
    
    // 1. Trigger LoadScene action
    app.world_mut().resource_mut::<ActionQueue>().push(Action::LoadScene("scenes/tests/test_scene.ron".to_string()));
    
    // 2. Run executor
    app.update();
    
    // 3. Verify SceneEvent::Requested was emitted
    app.world_mut().run_system_once(|mut scene_events: MessageReader<SceneEvent>| {
        let events: Vec<_> = scene_events.read().cloned().collect();
        assert!(events.iter().any(|e| matches!(e, SceneEvent::Requested(path) if path == "scenes/tests/test_scene.ron")));
    }).unwrap();
}

#[test]
fn test_input_abstraction_flow() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
       .add_plugins(bevy::state::app::StatesPlugin)
       .add_plugins(AssetPlugin::default())
       .add_message::<bevy::input::mouse::MouseMotion>()
       .add_message::<bevy::input::mouse::MouseWheel>()
       .init_resource::<ButtonInput<KeyCode>>()
       .init_resource::<ButtonInput<MouseButton>>()
       .init_resource::<Assets<Mesh>>()
       .init_resource::<Assets<StandardMaterial>>()
       .init_resource::<Assets<Gltf>>()
       .init_resource::<Assets<AnimationGraph>>()
       .insert_resource(ProjectConfigPath("project.ron".to_string()))
       .add_plugins(GamePlugin);

    // Initial run
    app.update();

    // 1. Setup an entity with CharacterController
    let entity = app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
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
        AnimationController {
            animations: AnimationMap {
                idle: "idle".to_string(),
                walk: "walk".to_string(),
                run: "run".to_string(),
                jump_enter: "jump".to_string(),
                jump_loop: "jump".to_string(),
                jump_exit: "jump".to_string(),
                death: "death".to_string(),
                dance: "dance".to_string(),
                crouch_idle: "crouch".to_string(),
                crouch_forward: "crouch".to_string(),
                roll: "roll".to_string(),
            },
            current: "idle".to_string(),
            last_played: String::new(),
            gltf_path: String::new(),
            gltf_handle: Default::default(),
            node_indices: Default::default(),
            graph_initialized: false,
        },
    )).id();

    // 2. Simulate "W" key press
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW);
    
    // 3. Run systems: input_translator_system -> player_movement_system
    app.update();
    
    // 4. Verify InputActionMessage was emitted
    app.world_mut().run_system_once(move |mut input_events: MessageReader<InputActionMessage>| {
        let events: Vec<_> = input_events.read().cloned().collect();
        assert!(events.iter().any(|e| e.entity == entity && matches!(e.action, InputAction::Move(v) if v.y > 0.0)));
    }).unwrap();
    
    // 5. Verify character transform moved forward (+Y in our abstract Move, which translates to transform.forward() in player_movement_system)
    // Note: Transform::forward() is usually (0, 0, -1) in Bevy 3D.
    // In our player_movement_system: velocity += *forward * dir.y;
    let transform = app.world().entity(entity).get::<Transform>().unwrap();
    assert!(transform.translation.z < 0.0); // Moved forward in Bevy's -Z direction
}

#[test]
fn test_action_to_state_transition() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
       .add_plugins(bevy::state::app::StatesPlugin)
       .add_plugins(AssetPlugin::default())
       .add_message::<bevy::input::mouse::MouseMotion>()
       .add_message::<bevy::input::mouse::MouseWheel>()
       .init_resource::<ButtonInput<KeyCode>>()
       .init_resource::<ButtonInput<MouseButton>>()
       .init_resource::<Assets<Mesh>>()
       .init_resource::<Assets<StandardMaterial>>()
       .init_resource::<Assets<Gltf>>()
       .init_resource::<Assets<AnimationGraph>>()
       .insert_resource(ProjectConfigPath("project.ron".to_string()))
       .add_plugins(GamePlugin);
       
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
    let mut app = App::new();
    
    app.add_plugins(MinimalPlugins)
       .add_plugins(bevy::state::app::StatesPlugin)
       .add_plugins(AssetPlugin::default())
       .add_message::<bevy::input::mouse::MouseMotion>()
       .add_message::<bevy::input::mouse::MouseWheel>()
       .init_resource::<ButtonInput<KeyCode>>()
       .init_resource::<ButtonInput<MouseButton>>()
       .init_resource::<Assets<Mesh>>()
       .init_resource::<Assets<StandardMaterial>>()
       .init_resource::<Assets<Gltf>>()
       .init_resource::<Assets<AnimationGraph>>()
       .insert_resource(ProjectConfigPath("project.ron".to_string()))
       .add_plugins(GamePlugin);
       
    // 1. Run once to process Startup (setup)
    app.update();
    
    // 2. Simulate Quit Message
    app.world_mut().resource_mut::<Messages<UiMessage>>().write(UiMessage::Quit);
    
    // 3. Run systems (Interpreter + Executor will run)
    app.update();
    
    // 4. Verify side effects
    // The executor should have queued Action::Quit which sends AppExit
    // We can check if ActionQueue has it or just verify it doesn't crash
    let action_queue = app.world().resource::<ActionQueue>();
    assert!(action_queue.0.is_empty()); // Should be empty because it was popped and executed
}
