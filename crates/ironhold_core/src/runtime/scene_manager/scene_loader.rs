use bevy::prelude::*;
use crate::schema::*;
use crate::schema::scene_v2::GameSceneV2;
use crate::schema::player::PlayerConfig;
use crate::runtime::messages::*;
use crate::runtime::material_factory::MaterialFactory;
use super::{
    SceneV2Params, SceneMaterialParams,
    LevelEntity, OverlayEntity, PendingSceneLoadMode,
    LoadedSpawnPoints, SpawnRegistry, MergedModelFixes,
};
use super::entity_spawner::{
    spawn_prefab_instance, spawn_player_entity, default_camera_config, default_input_map,
};

pub fn spawn_scene_v2(
    mut commands: Commands,
    params: SceneV2Params,
    asset_server: Res<AssetServer>,
    mut events: MessageReader<AssetEvent<GameSceneV2>>,
    mut next_state: ResMut<NextState<AppState>>,
    state: Res<State<AppState>>,
    level_entities: Query<Entity, With<LevelEntity>>,
    overlay_entities: Query<Entity, With<OverlayEntity>>,
    mut scene_events: MessageWriter<SceneEvent>,
    model_spawner: Res<crate::runtime::model_spawner::ModelSpawner>,
    merged_fixes: Res<MergedModelFixes>,
    mut mats: SceneMaterialParams,
    mut spawn_registry: ResMut<SpawnRegistry>,
    mut load_mode: ResMut<PendingSceneLoadMode>,
) {
    let Some(scene_handle) = params.scene_handle.as_ref() else { return; };
    let Some(project) = params.configs.get(&params.config_handle.0) else { return; };
    let is_overlay = *load_mode == PendingSceneLoadMode::Overlay;

    let mut ready_to_spawn = false;
    for event in events.read() {
        if event.is_loaded_with_dependencies(&scene_handle.0) {
            ready_to_spawn = true;
        }
    }
    let can_spawn_state = *state.get() == AppState::LoadingScene
        || *state.get() == AppState::LoadingProject
        || (is_overlay && *state.get() == AppState::InGame);
    if can_spawn_state && params.scenes.get(&scene_handle.0).is_some() {
        ready_to_spawn = true;
    }

    if !ready_to_spawn { return; }
    if *state.get() == AppState::InGame && !is_overlay { return; }

    let Some(scene) = params.scenes.get(&scene_handle.0) else { return; };

    scene_events.write(SceneEvent::Loaded(
        asset_server
            .get_path(&scene_handle.0)
            .map(|p| p.path().to_string_lossy().into_owned())
            .unwrap_or_default(),
    ));

    info!(
        "Scene V2 Loaded! name={}, {} entities, {} ui",
        scene.name,
        scene.entities.len(),
        scene.ui.len()
    );

    // Always remove any existing overlay (loading a new scene or new overlay replaces it).
    for entity in overlay_entities.iter() {
        commands.entity(entity).despawn();
    }

    if is_overlay {
        // Overlay mode: keep the game world, only spawn the UI section.
        // Reset load_mode immediately so subsequent loads default to Replace.
        *load_mode = PendingSceneLoadMode::Replace;
    } else {
        // Replace mode: tear down the existing world.
        commands.insert_resource(LoadedSpawnPoints(scene.spawn_points.clone()));
        spawn_registry.entities.clear();
        spawn_registry.counter = 0;

        for entity in level_entities.iter() {
            commands.entity(entity).despawn();
        }

        // Build material handles from the asset catalog for this scene.
        mats.built.0.clear();
        for (name, mat_def) in &params.asset_catalog.0.materials {
            let handle = MaterialFactory::build(
                &asset_server,
                &mut mats.standard,
                &mut mats.terrain,
                name,
                mat_def,
            );
            mats.built.0.insert(name.clone(), handle);
        }
        if !params.asset_catalog.0.materials.is_empty() {
            info!("Built {} material(s) from asset catalog", params.asset_catalog.0.materials.len());
        }

        // Spawn entities from prefabs
        let mut player_config: Option<PlayerConfig> = None;
        for entity_def in &scene.entities {
            let Some(prefab) = params.prefab_catalog.0.prefabs.get(&entity_def.prefab) else {
                warn!(
                    "Prefab '{}' not found in catalog, skipping entity '{}'",
                    entity_def.prefab, entity_def.id
                );
                continue;
            };

            let model_path = if let Some(catalog_entry) =
                params.asset_catalog.0.models.get(&prefab.model)
            {
                catalog_entry.path.clone()
            } else {
                warn!(
                    "Model key '{}' not found in asset catalog, skipping entity '{}'",
                    prefab.model, entity_def.id
                );
                continue;
            };

            let is_player = prefab.components.tags.contains(&"player".to_string());

            let t = &entity_def.transform;
            let translation = Vec3::new(t.translation.0, t.translation.1, t.translation.2);
            let rotation = Quat::from_euler(
                EulerRot::XYZ,
                t.rotation_euler_deg.0.to_radians(),
                t.rotation_euler_deg.1.to_radians(),
                t.rotation_euler_deg.2.to_radians(),
            );
            let scale = Vec3::new(t.scale.0, t.scale.1, t.scale.2);
            let transform = Transform { translation, rotation, scale };

            if is_player {
                let animation_policy = prefab
                    .animation_policy
                    .clone()
                    .unwrap_or_else(|| "prefabs/animation/player_policy.ron".to_string());
                player_config = Some(PlayerConfig {
                    model_path,
                    initial_position: (translation.x, translation.y, translation.z),
                    camera: default_camera_config(),
                    inputs: default_input_map(),
                    animation_policy,
                });
            } else {
                let parent = spawn_prefab_instance(
                    &mut commands,
                    &asset_server,
                    &model_spawner,
                    &merged_fixes.0,
                    &params.project_root.0,
                    prefab,
                    model_path,
                    transform,
                    &entity_def.id,
                );
                commands.entity(parent).insert(LevelEntity);
            }
        }

        // Spawn player (delayed if terrain present)
        if let Some(pc) = player_config {
            if scene.terrain.is_some() {
                info!("Terrain detected. Delaying player spawn...");
                commands.spawn(crate::runtime::scene_manager::PendingPlayerConfig(pc));
            } else {
                spawn_player_entity(
                    &mut commands,
                    &asset_server,
                    &merged_fixes.0,
                    &model_spawner,
                    &pc,
                    &params.project_root.0,
                );
            }
        } else {
            info!("No player entity in v2 scene, spawning default camera...");
            commands.spawn((
                Name::new("Default Camera"),
                Camera3d::default(),
                Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
                LevelEntity,
            ));
        }

        // Spawn Terrain
        if let Some(terrain_v2) = &scene.terrain {
            info!("Spawning V2 Terrain...");
            use crate::schema::level::TerrainConfig;
            let terrain_config = TerrainConfig {
                heightmap_path: terrain_v2.heightmap.clone(),
                splatmap_path: terrain_v2.splatmap.clone(),
                height_scale: terrain_v2.scale.1 * 100.0,
                horizontal_scale: terrain_v2.scale.0 * 100.0,
                position: (0.0, -10.0, 0.0),
                chunk_size: terrain_v2.chunk_size,
                material_paths: terrain_v2.material_paths.clone(),
            };
            commands.spawn((Name::new("Terrain"), LevelEntity, terrain_config));
        }

        // Apply lighting
        apply_lighting_v2(&mut commands, scene, project, &asset_server, &mut mats.images);

        next_state.set(AppState::InGame);
    } // end if !is_overlay

    // Spawn UI — always runs for both Replace and Overlay mode.
    if !scene.ui.is_empty() {
        if let Some(panel_def) = &scene.ui_panel {
            // Panel mode: full-screen flex root → centered panel box → column of elements.
            let mut root_cmd = commands.spawn((
                Name::new("UI Root"),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
            if is_overlay {
                root_cmd.insert(OverlayEntity);
            } else {
                root_cmd.insert(LevelEntity);
            }
            let (pr, pg, pb, pa) = panel_def.background_color;
            let padding = panel_def.padding;
            let gap = panel_def.gap;
            let panel_width = panel_def.width.map(Val::Px).unwrap_or(Val::Auto);
            let ui_elements = scene.ui.clone();
            root_cmd.with_children(|parent| {
                parent
                    .spawn((
                        Name::new("Panel"),
                        Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            padding: UiRect::all(Val::Px(padding)),
                            row_gap: Val::Px(gap),
                            width: panel_width,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(pr, pg, pb, pa)),
                    ))
                    .with_children(|parent| {
                        for el in &ui_elements {
                            let node = Node {
                                width: Val::Px(el.size.0),
                                height: Val::Px(el.size.1),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            };
                            spawn_ui_element_node(parent, el, node);
                        }
                    });
            });
        } else {
            // Absolute positioning mode.
            let mut root_cmd = commands.spawn((
                Name::new("UI Root"),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
            if is_overlay {
                root_cmd.insert(OverlayEntity);
            } else {
                root_cmd.insert(LevelEntity);
            }
            root_cmd.with_children(|parent| {
                for el in &scene.ui {
                    let node = Node {
                        width: Val::Px(el.size.0),
                        height: Val::Px(el.size.1),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        position_type: PositionType::Absolute,
                        left: Val::Px(el.position.0),
                        top: Val::Px(el.position.1),
                        ..default()
                    };
                    spawn_ui_element_node(parent, el, node);
                }
            });
        }
    }

    // Only emit Ready for full scene loads. Overlays must not overwrite debug.scene
    // (which tracks the active main scene) — otherwise the scene.unloading:<name>
    // event fires with the overlay's name instead of the main scene's name, breaking
    // rules like `scene.unloading:main → StopMusic`.
    if !is_overlay {
        scene_events.write(SceneEvent::Ready(
            asset_server
                .get_path(&scene_handle.0)
                .map(|p| p.path().to_string_lossy().into_owned())
                .unwrap_or_default(),
        ));
    }
}

fn spawn_ui_element_node(
    parent: &mut ChildSpawnerCommands,
    el: &crate::schema::scene_v2::UiElementDefV2,
    node: Node,
) {
    if el.kind == "label" {
        parent
            .spawn((Name::new(format!("Label: {}", el.text)), node))
            .with_children(|parent| {
                parent.spawn((
                    Name::new(format!("Text: {}", el.text)),
                    Text::new(el.text.clone()),
                    TextFont { font_size: 22.0, ..default() },
                    TextColor(Color::srgb(0.75, 0.75, 0.75)),
                ));
            });
    } else {
        let trigger = el.action.strip_prefix("ui.").unwrap_or(&el.action).to_string();
        let mut btn_node = node;
        btn_node.border = UiRect::all(Val::Px(5.0));
        parent
            .spawn((
                Name::new(format!("Button: {}", el.text)),
                Button,
                btn_node,
                BorderColor::from(Color::BLACK),
                BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                UiAction::Trigger(trigger),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Name::new(format!("Text: {}", el.text)),
                    Text::new(el.text.clone()),
                    TextFont { font_size: 26.0, ..default() },
                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                ));
            });
    }
}

fn apply_lighting_v2(
    commands: &mut Commands,
    scene: &crate::schema::scene_v2::GameSceneV2,
    project: &ProjectConfig,
    asset_server: &AssetServer,
    images: &mut Assets<Image>,
) {
    if let Some(lighting) = &scene.lighting {
        if let Some((r, g, b)) = lighting.ambient {
            commands.spawn((
                Name::new("Ambient Light"),
                AmbientLight {
                    color: Color::srgba(r, g, b, 1.0),
                    brightness: 150.0,
                    ..default()
                },
                LevelEntity,
            ));
        }

        if let Some(dl) = &lighting.directional {
            let rot = Quat::from_euler(
                EulerRot::XYZ,
                dl.rotation_euler_deg.0.to_radians(),
                dl.rotation_euler_deg.1.to_radians(),
                dl.rotation_euler_deg.2.to_radians(),
            );
            commands.spawn((
                Name::new("Directional Light"),
                DirectionalLight {
                    color: Color::srgba(dl.color.0, dl.color.1, dl.color.2, 1.0),
                    illuminance: dl.intensity,
                    shadows_enabled: dl.shadows_enabled,
                    ..default()
                },
                Transform::from_rotation(rot),
                LevelEntity,
            ));
        }
    }

    // Environment map from project global_environment
    if let Some(env) = &project.global_environment {
        let (d_handle, s_handle) = if env.diffuse_path.is_none() && env.specular_path.is_none() {
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
