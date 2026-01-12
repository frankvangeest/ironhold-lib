use bevy::prelude::*;
use ironhold_core::GamePlugin;
use ironhold_core::runtime::{UiMessage, Action, ActionQueue};
use ironhold_core::schema::AppState;
use ironhold_core::ProjectConfigPath;

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
    app.world_mut().resource_mut::<Messages<UiMessage>>().write(UiMessage::ButtonPressed("test_scene.ron".to_string()));
    
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
    app.world_mut().resource_mut::<ActionQueue>().push(Action::LoadScene("another_scene.ron".to_string()));
    
    // 4. Run executor
    app.update(); // Executor sets NextState
    app.update(); // Apply transition
    
    // 5. Verify state transitioned to LoadingScene
    let state = app.world().resource::<State<AppState>>();
    assert_eq!(*state.get(), AppState::LoadingScene);
}
