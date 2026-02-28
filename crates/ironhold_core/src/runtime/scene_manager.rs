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
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{TextureViewDescriptor};
// Lights and Image are in the prelude.

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
    mut images: ResMut<Assets<Image>>,
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
                    Name::new("UI Root"),
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
                                    Name::new(format!("Button: {}", text)),
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
                                        Name::new(format!("Text: {}", text)),
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
                    Name::new("Player"),
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
                    Name::new("Orbit Camera"),
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
                    Name::new("Default Camera"),
                    Camera3d::default(),
                    Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
                    LevelEntity,
                ));
            }
            
            // Spawn Terrain
            if let Some(terrain_config) = &level.terrain {
                info!("Spawning Terrain...");
                commands.spawn((
                    Name::new("Terrain"),
                    LevelEntity,
                    terrain_config.clone(),
                ));
            }

            // Apply Lighting Config
            apply_lighting(
                &mut commands,
                &level,
                &project,
                &asset_server,
                &mut images,
            );
            
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

fn apply_lighting(
    commands: &mut Commands,
    level: &GameLevel,
    project: &ProjectConfig,
    asset_server: &AssetServer,
    images: &mut Assets<Image>,
) {
    let lighting = level.lighting.as_ref();
    
    // 1. Ambient Light
    if let Some(ambient) = lighting.and_then(|l| l.ambient.as_ref()) {
        commands.spawn((
            Name::new("Ambient Light"),
            AmbientLight {
                color: Color::srgba(ambient.color.0, ambient.color.1, ambient.color.2, 1.0),
                brightness: ambient.brightness,
                ..default()
            },
            LevelEntity,
        ));
    }

    // 2. Directional Light
    if let Some(dl) = lighting.and_then(|l| l.directional.as_ref()) {
        let dir = Vec3::new(dl.direction.0, dl.direction.1, dl.direction.2).normalize();
        if !dir.is_nan() {
            commands.spawn((
                Name::new("Directional Light"),
                DirectionalLight {
                    color: Color::srgba(dl.color.0, dl.color.1, dl.color.2, 1.0),
                    illuminance: dl.illuminance,
                    shadows_enabled: dl.shadows_enabled,
                    ..default()
                },
                // Transform looking FROM the light TO the center (origin)
                // Direction is "normalize(0.3, -1.0, 0.2)" which is pointing DOWN and slightly forward/right.
                // Light direction is the direction the rays are travelling.
                // In Bevy, DirectionalLight points forward (-Z) relative to the Transform.
                Transform::from_xyz(0.0, 0.0, 0.0).looking_to(dir, Vec3::Y),
                LevelEntity,
            ));
        }
    }

    // 3. Environment Map Light
    // Merge scene and project environments
    let scene_env = lighting.and_then(|l| l.environment.as_ref());
    let env_config = scene_env.or(project.global_environment.as_ref());

    if let Some(env) = env_config {
        let diffuse_handle = env.diffuse_path.as_ref().map(|p| asset_server.load(p));
        let specular_handle = env.specular_path.as_ref().map(|p| asset_server.load(p));

        // Note: For now we just load what's there. 
        // If they are missing, we should fallback to generated.
        // However, asset_server.load is async, so we can't easily check 'missing' here without checking disk.
        // For the sake of the requirement: "Generate... if no .ktx2 file is present".
        
        // We'll spawn a "Loader" entity that checks for asset readiness or we can just try to load 
        // and if it fails (not easily checkable here), it stays black.
        
        // To strictly follow "If specified but not found, generate", we'd need to check Path.
        // But for WASM compatibility, Path doesn't work.
        
        // Better approach: If paths are None, OR if we want to force generation via fallback config.
        let (d_handle, s_handle) = if env.diffuse_path.is_none() && env.specular_path.is_none() {
            if let Some(fallback) = &env.fallback {
                let img = generate_cubemap(fallback);
                let handle = images.add(img);
                (handle.clone(), handle)
            } else {
                (Handle::default(), Handle::default())
            }
        } else {
            (diffuse_handle.unwrap_or_default(), specular_handle.unwrap_or_default())
        };

        if d_handle != Handle::default() || s_handle != Handle::default() {
            commands.spawn((
                Name::new("Environment Map Light"),
                EnvironmentMapLight {
                    diffuse_map: d_handle,
                    specular_map: s_handle,
                    intensity: env.intensity,
                    ..default()
                },
                LevelEntity,
            ));
        }
    }
}

fn generate_cubemap(config: &crate::schema::level::GeneratedEnvironmentMapLight) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDimension};
    
    // Create a tiny 1x1 6-layer cubemap
    // Colors: 0: +X, 1: -X, 2: +Y (top), 3: -Y (bottom), 4: +Z, 5: -Z
    let top = [
        (config.top_color.0 * 255.0) as u8,
        (config.top_color.1 * 255.0) as u8,
        (config.top_color.2 * 255.0) as u8,
        255
    ];
    let bottom = [
        (config.bottom_color.0 * 255.0) as u8,
        (config.bottom_color.1 * 255.0) as u8,
        (config.bottom_color.2 * 255.0) as u8,
        255
    ];
    
    // Mid color for sides
    let mid = [
        ((config.top_color.0 + config.bottom_color.0) * 0.5 * 255.0) as u8,
        ((config.top_color.1 + config.bottom_color.1) * 0.5 * 255.0) as u8,
        ((config.top_color.2 + config.bottom_color.2) * 0.5 * 255.0) as u8,
        255
    ];

    let mut data = Vec::new();
    for i in 0..6 {
        let color = match i {
            2 => top,
            3 => bottom,
            _ => mid,
        };
        data.extend_from_slice(&color);
    }

    let mut image = Image::new(
        Extent3d { width: 1, height: 1, depth_or_array_layers: 6 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    
    image
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
                let name = path.split('/').last().unwrap_or(&path).to_string();
                commands.spawn((
                    Name::new(name),
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



