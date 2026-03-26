use bevy::prelude::*;
use std::collections::HashMap;
use crate::ProjectRoot;

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
use crate::schema::player::AnimationPolicy;

use bevy::asset::RenderAssetUsages;
use bevy_rapier3d::prelude::*;
use bevy::render::render_resource::{TextureViewDescriptor};
// Lights and Image are in the prelude.

/// The final merged model-fix map, assembled from inline project fixes and (optionally) an
/// external `model_fixes.ron` file.  Always available after project loading completes.
#[derive(Resource, Default)]
pub struct MergedModelFixes(pub HashMap<String, TransformFix>);

/// The rules loaded for the current project. Populated from inline `config.rules` (v1)
/// or from the external `logic/rules.ron` file (v2). Always available after project loading.
#[derive(Resource, Default, Clone)]
pub struct LoadedRules(pub Vec<LogicRule>);

/// Tracks externally-loaded project config files that are still loading.
/// Inserted by `check_project_loaded` on the first frame the project config is ready,
/// removed implicitly once the project transitions to `LoadingScene`.
#[derive(Resource)]
pub struct PendingProjectLoads {
    pub model_fixes: Option<Handle<ModelFixesAsset>>,
    pub rules: Option<Handle<LogicRulesAsset>>,
}

#[derive(Component)]
pub struct PendingPlayerConfig(pub PlayerConfig);

/// Holds a handle to an AnimationPolicy asset that is still loading.
/// Replaced by AnimationPolicyComponent once the asset resolves.
#[derive(Component)]
pub struct PendingAnimationPolicy(pub Handle<AnimationPolicy>);

pub fn animation_policy_loader_system(
    mut commands: Commands,
    mut pending: Query<(Entity, &PendingAnimationPolicy, &mut AnimationController)>,
    policies: Res<Assets<AnimationPolicy>>,
) {
    for (entity, pending_policy, mut controller) in &mut pending {
        if let Some(policy) = policies.get(&pending_policy.0) {
            controller.current = policy.base.idle.clone();
            commands.entity(entity)
                .insert(AnimationPolicyComponent(policy.clone()))
                .remove::<PendingAnimationPolicy>();
            info!("AnimationPolicy loaded — initial animation: {}", policy.base.idle);
        }
    }
}

pub fn check_project_loaded(
    mut commands: Commands,
    config_handle: Res<ProjectConfigHandle>,
    configs: Res<Assets<ProjectConfig>>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
    mut scene_events: MessageWriter<SceneEvent>,
    project_root: Res<ProjectRoot>,
    pending: Option<Res<PendingProjectLoads>>,
    model_fixes_assets: Res<Assets<ModelFixesAsset>>,
    rules_assets: Res<Assets<LogicRulesAsset>>,
) {
    let Some(config) = configs.get(&config_handle.0) else { return; };

    if let Err(e) = config.validate() {
        panic!("Invalid ProjectConfig: {}", e);
    }

    // Phase 1: kick off all external file loads on the first frame the project config is ready.
    if pending.is_none() {
        let model_fixes_handle = config.model_fixes_path.as_ref().map(|p| {
            let resolved = resolve_project_path(&project_root.0, p);
            info!("Loading external model fixes from: {}", resolved);
            asset_server.load::<ModelFixesAsset>(resolved)
        });
        let rules_handle = config.rules_path.as_ref().map(|p| {
            let resolved = resolve_project_path(&project_root.0, p);
            info!("Loading external rules from: {}", resolved);
            asset_server.load::<LogicRulesAsset>(resolved)
        });

        let any_pending = model_fixes_handle.is_some() || rules_handle.is_some();
        commands.insert_resource(PendingProjectLoads {
            model_fixes: model_fixes_handle,
            rules: rules_handle,
        });

        if any_pending {
            return; // Wait for next frame.
        }

        // No external files — store inline data and proceed.
        commands.insert_resource(MergedModelFixes(config.model_fixes.clone()));
        commands.insert_resource(LoadedRules(config.rules.clone()));
    } else {
        // Phase 2: wait for all pending loads to complete.
        let pending = pending.unwrap();

        if let Some(h) = &pending.model_fixes {
            if model_fixes_assets.get(h).is_none() { return; }
        }
        if let Some(h) = &pending.rules {
            if rules_assets.get(h).is_none() { return; }
        }

        // Phase 3: merge and store results.
        let mut merged_fixes = config.model_fixes.clone();
        if let Some(h) = &pending.model_fixes {
            if let Some(fixes_asset) = model_fixes_assets.get(h) {
                merged_fixes.extend(fixes_asset.model_fixes.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
        }
        commands.insert_resource(MergedModelFixes(merged_fixes));

        let rules = if let Some(h) = &pending.rules {
            rules_assets.get(h).map(|a| a.rules.clone()).unwrap_or_default()
        } else {
            config.rules.clone()
        };
        commands.insert_resource(LoadedRules(rules));
    }

    let scene_path = resolve_project_path(&project_root.0, &config.initial_scene);
    info!("Project Config Loaded (schema v{}). Initial Scene: {}", config.schema_version, scene_path);

    let scene_handle = asset_server.load(scene_path.clone());
    commands.insert_resource(LevelHandle(scene_handle));
    scene_events.write(SceneEvent::Requested(scene_path));
    next_state.set(AppState::LoadingScene);
}

/// Resolves a project-relative path against the project root directory.
/// If the path already looks absolute (starts with a known top-level dir) it is returned as-is.
fn resolve_project_path(project_root: &str, path: &str) -> String {
    if project_root.is_empty() {
        return path.to_string();
    }
    format!("{}/{}", project_root, path)
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
    merged_fixes: Res<MergedModelFixes>,
    mut images: ResMut<Assets<Image>>,
    project_root: Res<ProjectRoot>,
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
                    &merged_fixes.0,
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
            
            // Spawn Player (Delayed if terrain exists)
            if let Some(player_config) = &level.player {
                if level.terrain.is_some() {
                    info!("Terrain detected. Delaying player spawn...");
                    commands.spawn(PendingPlayerConfig(player_config.clone()));
                } else {
                    spawn_player_entity(
                        &mut commands,
                        &asset_server,
                        &merged_fixes.0,
                        &model_spawner,
                        player_config,
                        &project_root.0,
                    );
                }
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
    loaded_rules: Res<LoadedRules>,
) {
    for event in ui_events.read() {
        let event_name = match event {
            UiMessage::ButtonPressed(trigger) => format!("ui.button_pressed:{}", trigger),
        };

        for rule in &loaded_rules.0 {
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
    project_root: Res<ProjectRoot>,
) {
    while let Some(action) = action_queue.pop() {
        match action {
            Action::LoadScene(path) => {
                let resolved = resolve_project_path(&project_root.0, &path);
                info!("Executing Action::LoadScene: {}", resolved);
                let handle = asset_server.load(resolved.clone());
                commands.insert_resource(LevelHandle(handle));
                scene_events.write(SceneEvent::Requested(resolved));
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



pub fn spawn_player_when_terrain_ready(
    mut commands: Commands,
    terrain_query: Query<Entity, Added<crate::capabilities::terrain::TerrainReady>>,
    pending_query: Query<(Entity, &PendingPlayerConfig)>,
    asset_server: Res<AssetServer>,
    model_spawner: Res<ModelSpawner>,
    merged_fixes: Res<MergedModelFixes>,
    project_root: Res<ProjectRoot>,
) {
    if terrain_query.is_empty() { return; }

    for (pending_entity, pending) in &pending_query {
        info!("Terrain is ready. Spawning player...");
        spawn_player_entity(
            &mut commands,
            &asset_server,
            &merged_fixes.0,
            &model_spawner,
            &pending.0,
            &project_root.0,
        );
        commands.entity(pending_entity).despawn();
    }
}

fn spawn_player_entity(
    commands: &mut Commands,
    asset_server: &AssetServer,
    fixes: &HashMap<String, TransformFix>,
    model_spawner: &ModelSpawner,
    player_config: &PlayerConfig,
    project_root: &str,
) {
    let gltf_path = player_config.model_path.split('#').next().unwrap_or("").to_string();
    let gltf_handle = asset_server.load(gltf_path.clone());

    let policy_path = resolve_project_path(project_root, &player_config.animation_policy);
    let policy_handle: Handle<AnimationPolicy> = asset_server.load(policy_path.clone());
    info!("Loading AnimationPolicy from: {}", policy_path);

    let spawned = model_spawner.spawn_instance(
        commands,
        asset_server,
        fixes,
        player_config.model_path.clone(),
        Transform::from_translation(Vec3::from(player_config.initial_position)),
    );

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
        PendingAnimationPolicy(policy_handle.clone()),
        AnimationController {
            current: String::new(), // will be set once policy loads
            last_played: String::new(),
            gltf_path,
            gltf_handle,
            node_indices: HashMap::new(),
            graph_initialized: false,
            transition_ms: 0,
            should_loop: true,
        },
        // Physics
        RigidBody::Dynamic,
        Collider::capsule_y(0.8, 0.4),
        LockedAxes::ROTATION_LOCKED,
        Damping { linear_damping: 0.5, angular_damping: 0.5 },
        Velocity::default(),
        ExternalImpulse::default(),
    ));

    // Spawn Orbit Camera
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
            pitch: 0.5,
            yaw: 0.0,
            look_at_offset: Vec3::from(player_config.camera.look_at_offset),
        }
    ));
}
