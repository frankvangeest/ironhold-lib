use bevy::prelude::*;
use crate::schema::project::ProjectConfig;

// Schema
use crate::schema::*;

// Runtime
use crate::runtime::actions::*;
use crate::runtime::messages::*;
use crate::runtime::model_spawner::*;

// Capabilities
use crate::capabilities::player::CharacterController;
use crate::capabilities::animation::AnimationController;
use crate::capabilities::camera::OrbitCamera;
use crate::capabilities::animation_resolver::{
    ActiveOverride, 
    AnimationPolicyComponent, 
    AnimationRequests, 
    LocomotionState,
};

use std::collections::HashMap;

pub fn check_project_loaded(
    mut commands: Commands,
    config_handle: Res<ProjectConfigHandle>,
    configs: Res<Assets<ProjectConfig>>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
    mut scene_events: MessageWriter<SceneEvent>,
) {
    if let Some(config) = configs.get(&config_handle.0) {
        if let Err(e) = config.validate() {
            panic!("Invalid ProjectConfig: {}", e);
        }
        info!("Project Config Loaded. Initial Scene: {} (schema v{})", config.initial_scene, config.schema_version);

        // Load the initial scene
        let scene_handle = asset_server.load(config.initial_scene.clone());
        commands.insert_resource(LevelHandle(scene_handle));
        
        scene_events.write(SceneEvent::Requested(config.initial_scene.clone()));
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
    mut scene_events: MessageWriter<SceneEvent>,
    config_handle: Res<ProjectConfigHandle>,
    configs: Res<Assets<ProjectConfig>>,
    model_spawner: Res<ModelSpawner>,
) {
    let Some(level_handle) = level_handle else { return; };
    let Some(project) = configs.get(&config_handle.0) else { return; };
    
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
            scene_events.write(SceneEvent::Loaded(
                asset_server
                    .get_path(&level_handle.0)
                    .map(|p| p.path().to_string_lossy().into_owned())
                    .unwrap_or_default()
            ));
            
            if let Err(e) = level.validate() {
                panic!("Invalid GameLevel: {}", e);
            }

            // Only spawn if we are NOT already InGame to avoid duplication loops 
            if *state.get() == AppState::InGame {
                return; 
            }
            
            info!("Level Loaded! schema v{}, Spawning {} models and {} ui elements", level.schema_version, level.models.len(), level.ui.len());
            
            if level.schema_version == 0 {
                warn!("GameLevel schema_version is 0 (missing). Please update to v1.");
            } else if level.schema_version > 1 {
                error!("GameLevel schema_version {} is newer than supported v1!", level.schema_version);
            }

            for entity in current_entities.iter() {
                commands.entity(entity).despawn();
            }

            for model in &level.models {
                let parent_tf = Transform {
                    translation: Vec3::new(model.position.0, model.position.1, model.position.2),
                    ..Default::default()
                };
                let spawned = model_spawner.spawn_instance(
                    &mut commands,
                    &asset_server,
                    &project,
                    model.path.clone(),
                    parent_tf,
                );
                commands.entity(spawned.parent).insert(LevelEntity);
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
                            UiElement::Button { 
                                text, 
                                action, 
                                position,
                                width,
                                height,
                                font_size,
                                border_color,
                                background_color,
                                text_color,
                            } => {
                                let mut node = Node {
                                    width: Val::Px(width.unwrap_or(200.0)),
                                    height: Val::Px(height.unwrap_or(65.0)),
                                    border: UiRect::all(Val::Px(5.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                };

                                if let Some((x, y)) = position {
                                    node.position_type = PositionType::Absolute;
                                    node.left = Val::Px(*x);
                                    node.top = Val::Px(*y);
                                }

                                let b_color = border_color
                                    .map(|(r, g, b, a)| Color::srgba(r, g, b, a))
                                    .unwrap_or(Color::BLACK);
                                let bg_color = background_color
                                    .map(|(r, g, b, a)| Color::srgba(r, g, b, a))
                                    .unwrap_or(Color::srgb(0.15, 0.15, 0.15));

                                parent.spawn((
                                    Button,
                                    node,
                                    BorderColor::from(b_color),
                                    BackgroundColor(bg_color),
                                    action.clone(),
                                ))
                                .with_children(|parent| {
                                    let t_color = text_color
                                        .map(|(r, g, b, a)| Color::srgba(r, g, b, a))
                                        .unwrap_or(Color::srgb(0.9, 0.9, 0.9));
                                    parent.spawn((
                                        Text::new(text),
                                        TextFont {
                                            font_size: font_size.unwrap_or(26.0),
                                            ..default()
                                        },
                                        TextColor(t_color),
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

                // Use ModelSpawner to spawn player character with fixes applied
                let spawned = model_spawner.spawn_instance(
                    &mut commands,
                    &asset_server,
                    &project,
                    player_config.model_path.clone(),
                    Transform::from_translation(Vec3::from(player_config.initial_position)),
                );
                
                // Add player-specific components to the parent entity
                let player_entity = spawned.parent;
                commands.entity(player_entity).insert((
                    LevelEntity,
                    CharacterController {
                        walk_speed: 3.0,
                        run_speed: 6.0,
                        rot_speed: 3.0,
                        inputs: player_config.inputs.clone(),
                        is_running: false,
                    },
                    LocomotionState::default(),
                    AnimationRequests::default(),
                    ActiveOverride::default(),
                    AnimationPolicyComponent(player_config.animation_policy.clone()),
                    AnimationController {
                        current: player_config.animation_policy.base.idle.clone(),
                        last_played: String::new(),
                        gltf_path,
                        gltf_handle,
                        node_indices: HashMap::new(),
                        graph_initialized: false,
                        transition_ms: 0,
                        should_loop: true,
                    },
                ));

                // Spawn Orbit Camera matching config
                let start_pos = Vec3::from(player_config.initial_position) + Vec3::from(player_config.camera.offset);
                
                commands.spawn((
                    Camera3d::default(),
                    Transform::from_translation(start_pos).looking_at(
                        Vec3::from(player_config.initial_position), 
                        Vec3::Y,
                    ),
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
                info!("No player in scene, spawning default camera...");
                commands.spawn((
                    Camera3d::default(),
                    Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
                    LevelEntity,
                ));
            }
            
            next_state.set(AppState::InGame);
            scene_events.write(SceneEvent::Ready(
                asset_server
                    .get_path(&level_handle.0)
                    .map(|p| p.path().to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ));
        }
    }
}

pub fn message_interpreter_system(
    mut ui_events: MessageReader<UiMessage>,
    mut action_queue: ResMut<ActionQueue>,
    config_handle: Res<ProjectConfigHandle>,
    configs: Res<Assets<ProjectConfig>>,
) {
    let Some(config) = configs.get(&config_handle.0) else { return; };

    for event in ui_events.read() {
        let event_name = match event {
            UiMessage::ButtonPressed(trigger) => format!("ui.button_pressed:{}", trigger),
        };

        for rule in &config.rules {
            if rule.on == event_name {
                for action in &rule.do_actions {
                    info!("Rule Matched! Event: {} -> Action: {:?}", event_name, action);
                    action_queue.push(action.clone());
                }
            }
        }
    }
}

pub fn action_executor_system(
    mut commands: Commands,
    mut action_queue: ResMut<ActionQueue>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
    mut scene_events: MessageWriter<SceneEvent>,
    mut animation_requests: Query<&mut AnimationRequests>,
) {
    while let Some(action) = action_queue.pop() {
        match action {
            Action::LoadScene(path) => {
                info!("Executing Action::LoadScene: {}", path);
                let handle = asset_server.load(path.clone());
                commands.insert_resource(LevelHandle(handle));
                scene_events.write(SceneEvent::Requested(path));
                next_state.set(AppState::LoadingScene);
            }
            Action::Quit => {
                info!("Executing Action::Quit");
                exit.write(AppExit::Success);
            }
            Action::Log(msg) => {
                info!("Action::Log: {}", msg);
            }
            Action::Spawn(path) => {
                info!("Executing Action::Spawn: {}", path);
                commands.spawn((
                    SceneRoot(asset_server.load(path.clone())),
                    Transform::default(),
                    LevelEntity,
                ));
            }
            Action::PlayAnimation(anim) => {
                info!("Executing Action::PlayAnimation: {}", anim);
                for mut req in &mut animation_requests {
                    req.queue.push_back(anim.clone());
                }
            }
        }
    }
}



