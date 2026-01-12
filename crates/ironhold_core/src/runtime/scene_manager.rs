use bevy::prelude::*;
use crate::schema::*;
use crate::runtime::actions::*;
use crate::runtime::messages::*;
use crate::capabilities::player::CharacterController;
use crate::capabilities::animation::AnimationController;
use crate::capabilities::camera::OrbitCamera;
use std::collections::HashMap;

pub fn check_project_loaded(
    mut commands: Commands,
    config_handle: Res<ProjectConfigHandle>,
    configs: Res<Assets<ProjectConfig>>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if let Some(config) = configs.get(&config_handle.0) {
        println!("Project Config Loaded. Initial Scene: {}", config.initial_scene);
        
        // Load the initial scene
        let scene_handle = asset_server.load(config.initial_scene.clone());
        commands.insert_resource(LevelHandle(scene_handle));
        
        next_state.set(AppState::LoadingScene);
    }
}

pub fn spawn_level(
    mut commands: Commands,
    level_handle: Option<Res<LevelHandle>>,
    levels: Res<Assets<GameLevel>>,
    asset_server: Res<AssetServer>,
    mut events: MessageReader<AssetEvent<GameLevel>>,
    mut next_state: ResMut<NextState<AppState>>,
    state: Res<State<AppState>>,
    current_entities: Query<Entity, With<LevelEntity>>,
) {
    let Some(level_handle) = level_handle else { return; };
    
    let mut ready_to_spawn = false;

    for event in events.read() {
        if event.is_loaded_with_dependencies(&level_handle.0) {
            ready_to_spawn = true;
        }
    }

    if *state.get() == AppState::LoadingScene || *state.get() == AppState::LoadingProject {
         if levels.get(&level_handle.0).is_some() {
             ready_to_spawn = true;
         }
    }

    if ready_to_spawn {
        if let Some(level) = levels.get(&level_handle.0) {
            
            // Only spawn if we are NOT already InGame to avoid duplication loops 
            if *state.get() == AppState::InGame {
                return; 
            }
            
            println!("Level Loaded! Spawning {} models and {} ui elements", level.models.len(), level.ui.len());
            
            for entity in current_entities.iter() {
                commands.entity(entity).despawn();
            }

            for model in &level.models {
                commands.spawn((
                    SceneRoot(asset_server.load(model.path.clone())),
                    Transform::from_translation(Vec3::from(model.position)),
                    LevelEntity,
                ));
            }

            if !level.ui.is_empty() {
                 commands.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    LevelEntity,
                ))
                .with_children(|parent| {
                    for element in &level.ui {
                        match element {
                            UiElement::Button { text, action } => {
                                parent.spawn((
                                    Button,
                                    Node {
                                        width: Val::Px(150.0),
                                        height: Val::Px(65.0),
                                        border: UiRect::all(Val::Px(5.0)),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    BorderColor::from(Color::BLACK),
                                    BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                                    action.clone(),
                                ))
                                .with_children(|parent| {
                                    parent.spawn((
                                        Text::new(text),
                                        TextFont {
                                            font_size: 33.0,
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                                    ));
                                });
                            }
                        }
                    }
                });
            }
            
            // Spawn Player
            if let Some(player_config) = &level.player {
                let gltf_path = player_config.model_path.split('#').next().unwrap_or("").to_string();
                let gltf_handle = asset_server.load(gltf_path.clone());

                let player_entity = commands.spawn((
                    SceneRoot(asset_server.load(player_config.model_path.clone())),
                    Transform::from_translation(Vec3::from(player_config.initial_position)),
                    LevelEntity,
                    CharacterController {
                        walk_speed: 3.0,
                        run_speed: 6.0,
                        rot_speed: 3.0,
                        inputs: player_config.inputs.clone(),
                        is_running: false,
                    },
                    AnimationController {
                        animations: player_config.animations.clone(),
                        current: player_config.animations.idle.clone(),
                        last_played: String::new(),
                        gltf_path,
                        gltf_handle,
                        node_indices: HashMap::new(),
                        graph_initialized: false,
                    }
                )).id();

                // Spawn Orbit Camera matching config
                let start_pos = Vec3::from(player_config.initial_position) + Vec3::from(player_config.camera.offset);
                
                commands.spawn((
                    Camera3d::default(),
                    Transform::from_translation(start_pos).looking_at(Vec3::from(player_config.initial_position), Vec3::Y),
                    LevelEntity,
                    OrbitCamera {
                        target: player_entity,
                        radius: Vec3::from(player_config.camera.offset).length(),
                        offset: Vec3::from(player_config.camera.offset),
                        zoom_speed: player_config.camera.zoom_speed,
                        orbit_speed: player_config.camera.orbit_speed,
                        min_radius: player_config.camera.min_radius,
                        max_radius: player_config.camera.max_radius,
                        pitch: 0.5, // Approx starting pitch
                        yaw: 0.0,
                        look_at_offset: Vec3::from(player_config.camera.look_at_offset),
                    }
                ));
            } else {
                // No player - spawn a default camera for UI/static scenes
                println!("No player in scene, spawning default camera...");
                commands.spawn((
                    Camera3d::default(),
                    Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
                    LevelEntity,
                ));
            }
            
            next_state.set(AppState::InGame);
        }
    }
}

pub fn message_interpreter_system(
    mut ui_events: MessageReader<UiMessage>,
    mut action_queue: ResMut<ActionQueue>,
) {
    for event in ui_events.read() {
        match event {
            UiMessage::ButtonPressed(path) => {
                action_queue.push(Action::LoadScene(path.clone()));
            }
        }
    }
}

pub fn action_executor_system(
    mut commands: Commands,
    mut action_queue: ResMut<ActionQueue>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    while let Some(action) = action_queue.pop() {
        match action {
            Action::LoadScene(path) => {
                println!("Executing Action::LoadScene: {}", path);
                let handle = asset_server.load(path);
                commands.insert_resource(LevelHandle(handle));
                next_state.set(AppState::LoadingScene);
            }
        }
    }
}
