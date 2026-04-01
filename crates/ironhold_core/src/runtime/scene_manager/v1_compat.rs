/// Legacy v1 `GameLevel` scene loader. No new features will be added here.
/// Migrate projects to the v2 `.scene.ron` format to use the full feature set.
use bevy::prelude::*;
use crate::ProjectRoot;
use crate::schema::*;
use crate::runtime::messages::*;
use crate::runtime::model_spawner::ModelSpawner;
use super::{MergedModelFixes, PendingPlayerConfig};
use super::entity_spawner::spawn_player_entity;

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
                    .unwrap_or_default(),
            ));

            if let Err(e) = level.validate() {
                panic!("Invalid GameLevel: {}", e);
            }

            // Only spawn if we are NOT already InGame to avoid duplication loops
            if *state.get() == AppState::InGame {
                return;
            }

            info!(
                "Level Loaded! schema v{}, Spawning {} models and {} ui elements",
                level.schema_version,
                level.models.len(),
                level.ui.len()
            );

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
                commands
                    .spawn((
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

                                    parent
                                        .spawn((
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
                commands.spawn((Name::new("Terrain"), LevelEntity, terrain_config.clone()));
            }

            // Apply Lighting Config
            apply_lighting_v1(&mut commands, level, project, &asset_server, &mut images);

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

fn apply_lighting_v1(
    commands: &mut Commands,
    level: &GameLevel,
    project: &ProjectConfig,
    asset_server: &AssetServer,
    images: &mut Assets<Image>,
) {
    let lighting = level.lighting.as_ref();

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
                Transform::from_xyz(0.0, 0.0, 0.0).looking_to(dir, Vec3::Y),
                LevelEntity,
            ));
        }
    }

    let scene_env = lighting.and_then(|l| l.environment.as_ref());
    let env_config = scene_env.or(project.global_environment.as_ref());

    if let Some(env) = env_config {
        let (d_handle, s_handle) =
            if env.diffuse_path.is_none() && env.specular_path.is_none() {
                if let Some(fallback) = &env.fallback {
                    let img = super::generate_cubemap(fallback);
                    let handle = images.add(img);
                    (handle.clone(), handle)
                } else {
                    (Handle::default(), Handle::default())
                }
            } else {
                let d = env.diffuse_path.as_ref().map(|p| asset_server.load(p)).unwrap_or_default();
                let s = env.specular_path.as_ref().map(|p| asset_server.load(p)).unwrap_or_default();
                (d, s)
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
