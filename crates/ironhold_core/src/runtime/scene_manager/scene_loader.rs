use bevy::prelude::*;
use bevy_rapier3d::prelude::{
    Collider, RigidBody, LockedAxes, Damping, Velocity, ExternalImpulse,
};
use crate::schema::*;
use crate::schema::scene_v2::GameSceneV2;
use crate::schema::player::PlayerConfig;
use crate::runtime::messages::*;
use crate::runtime::material_factory::MaterialFactory;
use crate::capabilities::player::CharacterController;
use crate::capabilities::camera::OrbitCamera;
use crate::capabilities::animation_resolver::{AnimationRequests, LocomotionState, ActiveOverride};
use super::{
    SceneV2Params, SceneMaterialParams,
    LevelEntity, OverlayEntity, PendingSceneLoadMode,
    LoadedSpawnPoints, SpawnRegistry, MergedModelFixes,
    ProjectKeyBindings, LoadedKeyBindings,
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
    project_key_bindings: Res<ProjectKeyBindings>,
    mut loaded_key_bindings: ResMut<LoadedKeyBindings>,
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

        let mut load_errors: Vec<String> = Vec::new();

        // Rebuild effective key bindings: project base + scene-level overrides.
        // This ensures bindings from a previous scene never bleed into the next one.
        {
            let mut effective = project_key_bindings.0.clone();
            for (key_name, trigger) in &scene.scene_key_bindings {
                if InputMap::parse_key(key_name).is_none() {
                    load_errors.push(format!(
                        "scene_key_bindings: unrecognised key name {:?} — binding will have no effect",
                        key_name
                    ));
                }
                effective.insert(key_name.clone(), trigger.clone());
            }
            *loaded_key_bindings = LoadedKeyBindings(effective);
        }

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
                &mut mats.custom,
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
        // A primitive prefab with tags: ["player"]: shape + params + spawn position.
        let mut primitive_player: Option<(String, crate::schema::catalog::PrimitiveParams, Vec3)> = None;
        let mut flycam_start: Option<Transform> = None;
        for entity_def in &scene.entities {
            let Some(prefab) = params.prefab_catalog.0.prefabs.get(&entity_def.prefab) else {
                load_errors.push(format!(
                    "entity '{}': prefab '{}' not found in catalog, entity skipped",
                    entity_def.id, entity_def.prefab
                ));
                continue;
            };

            let is_flycam = prefab.components.tags.contains(&"flycam".to_string());
            let is_player = prefab.components.tags.contains(&"player".to_string());

            // Build transform early — needed before model lookup so flycam can early-out.
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

            if is_flycam {
                // No model needed — just record the spawn transform.
                flycam_start = Some(transform);
                continue;
            }

            if prefab.kind == "primitive" {
                // ── Primitive player: collect and defer; camera spawned after entity loop ──
                if is_player {
                    let p = prefab.primitive.as_ref().cloned().unwrap_or_default();
                    primitive_player = Some((prefab.model.clone(), p, translation));
                    continue;
                }

                // ── Composite prefab: non-empty `children` list ───────────────────────────
                if !prefab.children.is_empty() {
                    let parent = commands.spawn((
                        Name::new(entity_def.id.clone()),
                        transform,
                        Visibility::default(),
                        LevelEntity,
                    )).id();
                    for child_def in &prefab.children {
                        let child_rot = Quat::from_euler(
                            EulerRot::XYZ,
                            child_def.rotation_euler_deg.0.to_radians(),
                            child_def.rotation_euler_deg.1.to_radians(),
                            child_def.rotation_euler_deg.2.to_radians(),
                        );
                        let child_tf = Transform {
                            translation: Vec3::from(child_def.offset),
                            rotation: child_rot,
                            scale: Vec3::from(child_def.scale),
                        };
                        if let Some(child_mesh) = build_primitive_mesh(&child_def.shape, &child_def.primitive) {
                            let child_mesh_h = mats.meshes.add(child_mesh);
                            let child_mat_h = mats.standard.add(
                                primitive_material(&child_def.primitive, project.primitive_default_color)
                            );
                            let child_entity = commands.spawn((
                                Name::new(child_def.shape.clone()),
                                Mesh3d(child_mesh_h),
                                MeshMaterial3d(child_mat_h),
                                child_tf,
                            )).id();
                            commands.entity(parent).add_child(child_entity);
                        } else {
                            load_errors.push(format!(
                                "entity '{}': unknown shape '{}' in composite prefab, child skipped",
                                entity_def.id, child_def.shape
                            ));
                        }
                    }
                    continue;
                }

                // ── Single primitive mesh ─────────────────────────────────────────────────
                let p = prefab.primitive.as_ref().cloned().unwrap_or_default();
                match build_primitive_mesh(&prefab.model, &p) {
                    Some(mesh) => {
                        let mesh_handle = mats.meshes.add(mesh);
                        let built_mat = prefab.material.as_ref()
                            .and_then(|key| mats.built.0.get(key));

                        let spawned = match built_mat {
                            Some(crate::runtime::material_factory::BuiltMaterialHandle::Standard(h)) => {
                                commands.spawn((
                                    Name::new(entity_def.id.clone()),
                                    Mesh3d(mesh_handle),
                                    MeshMaterial3d(h.clone()),
                                    transform,
                                    LevelEntity,
                                )).id()
                            }
                            Some(crate::runtime::material_factory::BuiltMaterialHandle::Terrain(h)) => {
                                commands.spawn((
                                    Name::new(entity_def.id.clone()),
                                    Mesh3d(mesh_handle),
                                    MeshMaterial3d(h.clone()),
                                    transform,
                                    LevelEntity,
                                )).id()
                            }
                            Some(crate::runtime::material_factory::BuiltMaterialHandle::Custom(h)) => {
                                commands.spawn((
                                    Name::new(entity_def.id.clone()),
                                    Mesh3d(mesh_handle),
                                    MeshMaterial3d(h.clone()),
                                    transform,
                                    LevelEntity,
                                )).id()
                            }
                            None => {
                                let mat_handle = mats.standard.add(
                                    primitive_material(&p, project.primitive_default_color)
                                );
                                commands.spawn((
                                    Name::new(entity_def.id.clone()),
                                    Mesh3d(mesh_handle),
                                    MeshMaterial3d(mat_handle),
                                    transform,
                                    LevelEntity,
                                )).id()
                            }
                        };

                        // Optional static physics collider
                        if p.physics {
                            if let Some(collider) = build_primitive_collider(&prefab.model, &p) {
                                commands.entity(spawned).insert((RigidBody::Fixed, collider));
                            } else {
                                load_errors.push(format!(
                                    "entity '{}': physics: true on shape '{}' — no collider builder, physics skipped",
                                    entity_def.id, prefab.model
                                ));
                            }
                        }
                    }
                    None => load_errors.push(format!(
                        "entity '{}': unknown primitive shape '{}', entity skipped",
                        entity_def.id, prefab.model
                    )),
                }
                continue;
            }

            let model_path = if let Some(catalog_entry) =
                params.asset_catalog.0.models.get(&prefab.model)
            {
                catalog_entry.path.clone()
            } else {
                load_errors.push(format!(
                    "entity '{}': model key '{}' not found in asset catalog, entity skipped",
                    entity_def.id, prefab.model
                ));
                continue;
            };

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

        if !load_errors.is_empty() {
            error!(
                "Scene '{}' — {} problem(s) found during load:\n  - {}",
                scene.name,
                load_errors.len(),
                load_errors.join("\n  - ")
            );
        }

        let tonemapping = scene.tonemapping.to_bevy();

        // ── Primitive player ─────────────────────────────────────────────────────────
        if let Some((shape, params, position)) = primitive_player {
            let cap_radius = params.radius.unwrap_or(0.4);
            let cap_half   = params.height.unwrap_or(0.5);
            let mesh = build_primitive_mesh(&shape, &params)
                .unwrap_or_else(|| Capsule3d { radius: cap_radius, half_length: cap_half }.mesh().build());
            let mesh_handle = mats.meshes.add(mesh);
            let mat_handle  = mats.standard.add(primitive_material(&params, project.primitive_default_color));

            let player_entity = commands.spawn((
                (
                    Name::new("Player"),
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(mat_handle),
                    Transform::from_translation(position),
                    Visibility::default(),
                    LevelEntity,
                ),
                (
                    CharacterController {
                        walk_speed: 3.0,
                        run_speed:  6.0,
                        rot_speed:  3.0,
                        inputs: default_input_map(),
                        is_running: false,
                    },
                    LocomotionState::default(),
                    AnimationRequests::default(),
                    ActiveOverride::default(),
                ),
                (
                    RigidBody::Dynamic,
                    Collider::capsule_y(cap_half, cap_radius),
                    LockedAxes::ROTATION_LOCKED,
                    Damping { linear_damping: 0.5, angular_damping: 0.5 },
                    Velocity::default(),
                    ExternalImpulse::default(),
                ),
            )).id();

            let cam = default_camera_config();
            let cam_offset = Vec3::from(cam.offset);
            commands.spawn((
                Name::new("Orbit Camera"),
                Camera3d::default(),
                tonemapping,
                Transform::from_translation(position + cam_offset)
                    .looking_at(position + Vec3::from(cam.look_at_offset), Vec3::Y),
                LevelEntity,
                OrbitCamera {
                    target:          player_entity,
                    radius:          cam_offset.length(),
                    offset:          cam_offset,
                    zoom_speed:      cam.zoom_speed,
                    orbit_speed:     cam.orbit_speed,
                    min_radius:      cam.min_radius,
                    max_radius:      cam.max_radius,
                    pitch:           0.5,
                    yaw:             0.0,
                    look_at_offset:  Vec3::from(cam.look_at_offset),
                },
            ));
        }
        // Spawn player (delayed if terrain present), flycam, or fallback camera.
        else if let Some(pc) = player_config {
            if scene.terrain.is_some() {
                info!("Terrain detected. Delaying player spawn...");
                commands.spawn((
                    crate::runtime::scene_manager::PendingPlayerConfig(pc),
                    crate::runtime::scene_manager::PendingTonemapping(tonemapping),
                ));
            } else {
                spawn_player_entity(
                    &mut commands,
                    &asset_server,
                    &merged_fixes.0,
                    &model_spawner,
                    &pc,
                    &params.project_root.0,
                    tonemapping,
                );
            }
        } else if let Some(fc_transform) = flycam_start {
            // Extract initial yaw/pitch from the spawn transform so the camera
            // starts oriented correctly and the first mouse move causes no jump.
            let (yaw, pitch, _) = fc_transform.rotation.to_euler(EulerRot::YXZ);
            info!(
                "Spawning FlyCamera at ({:.1}, {:.1}, {:.1})",
                fc_transform.translation.x,
                fc_transform.translation.y,
                fc_transform.translation.z,
            );
            commands.spawn((
                Name::new("FlyCamera"),
                Camera3d::default(),
                tonemapping,
                fc_transform,
                LevelEntity,
                crate::capabilities::flycam::FlyCamera { pitch, yaw, ..Default::default() },
            ));
        } else {
            info!("No player entity in v2 scene, spawning default camera...");
            commands.spawn((
                Name::new("Default Camera"),
                Camera3d::default(),
                tonemapping,
                Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
                LevelEntity,
            ));
        }

        // Spawn Terrain
        if let Some(terrain_v2) = &scene.terrain {
            info!("Spawning V2 Terrain...");
            let terrain_config = TerrainConfig {
                heightmap_path: terrain_v2.heightmap.clone(),
                splatmap_path: terrain_v2.splatmap.clone(),
                height_scale: terrain_v2.scale.1,
                horizontal_scale: terrain_v2.scale.0,
                position: terrain_v2.position.unwrap_or((0.0, 0.0, 0.0)),
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
            let panel_height = panel_def.height.map(Val::Px).unwrap_or(Val::Auto);
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
                            height: panel_height,
                            overflow: Overflow::clip(),
                            position_type: PositionType::Relative,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(pr, pg, pb, pa)),
                    ))
                    .with_children(|parent| {
                        for el in &ui_elements {
                            let node = if el.absolute {
                                Node {
                                    width: Val::Px(el.size.0),
                                    height: Val::Px(el.size.1),
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(el.position.0),
                                    top: Val::Px(el.position.1),
                                    ..default()
                                }
                            } else {
                                Node {
                                    width: Val::Px(el.size.0),
                                    height: Val::Px(el.size.1),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                }
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
    if el.kind == "rect" {
        // Non-interactive colored rectangle — no border, no text, no interaction.
        let (r, g, b, a) = el.color;
        parent.spawn((
            Name::new(format!("Rect: {}", el.id)),
            node,
            BackgroundColor(Color::srgba(r, g, b, a)),
        ));
        return;
    }

    if el.kind == "label" {
        let el_id = el.id.clone();
        parent
            .spawn((Name::new(format!("Label: {}", el.text)), node))
            .with_children(|parent| {
                let mut text_cmd = parent.spawn((
                    Name::new(format!("Text: {}", el.text)),
                    Text::new(el.text.clone()),
                    TextFont { font_size: 22.0, ..default() },
                    TextColor(Color::srgb(0.75, 0.75, 0.75)),
                ));
                if el_id == "flycam_position" {
                    text_cmd.insert(crate::capabilities::flycam::FlyCamPositionLabel);
                }
            });
    } else {
        let (r, g, b, a) = el.color;
        let bg_color = Color::srgba(r, g, b, a);
        let trigger = el.action.strip_prefix("ui.").unwrap_or(&el.action).to_string();
        let mut btn_node = node;
        btn_node.border = UiRect::all(Val::Px(5.0));
        parent
            .spawn((
                Name::new(format!("Button: {}", el.text)),
                Button,
                btn_node,
                BorderColor::from(Color::BLACK),
                BackgroundColor(bg_color),
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
                    brightness: lighting.ambient_brightness.unwrap_or(150.0),
                    ..default()
                },
                LevelEntity,
            ));
        }

        for (i, pl) in lighting.point_lights.iter().enumerate() {
            commands.spawn((
                Name::new(format!("Point Light {}", i)),
                PointLight {
                    color: Color::srgb(pl.color.0, pl.color.1, pl.color.2),
                    intensity: pl.intensity,
                    radius: pl.radius,
                    range: pl.range,
                    shadows_enabled: pl.shadows_enabled,
                    ..default()
                },
                Transform::from_translation(Vec3::new(pl.position.0, pl.position.1, pl.position.2)),
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

// ─── Primitive shape helpers ───────────────────────────────────────────────────

fn build_primitive_mesh(shape: &str, p: &crate::schema::catalog::PrimitiveParams) -> Option<Mesh> {
    Some(match shape {
        "Cuboid" => {
            let (x, y, z) = p.size.unwrap_or((3.0, 3.0, 3.0));
            Cuboid::new(x, y, z).mesh().build()
        }
        "Sphere" => Sphere::new(p.radius.unwrap_or(2.0)).mesh().build(),
        "Cylinder" => Cylinder::new(
            p.radius.unwrap_or(1.5),
            p.height.unwrap_or(4.0),
        ).mesh().build(),
        "Capsule3d" => Capsule3d {
            radius: p.radius.unwrap_or(1.5),
            half_length: p.height.unwrap_or(4.0),
        }.mesh().build(),
        "Cone" => Cone {
            radius: p.radius.unwrap_or(2.0),
            height: p.height.unwrap_or(4.0),
        }.mesh().build(),
        "Torus" => Torus::new(
            p.radius_top.unwrap_or(0.5),  // inner radius
            p.radius.unwrap_or(2.0),      // outer radius
        ).mesh().build(),
        "ConicalFrustum" => ConicalFrustum {
            radius_top:    p.radius_top.unwrap_or(1.0),
            radius_bottom: p.radius.unwrap_or(2.0),
            height:        p.height.unwrap_or(4.0),
        }.mesh().build(),
        _ => return None,
    })
}

/// Returns a Rapier3D static collider matching the given shape, or `None` for unsupported shapes.
fn build_primitive_collider(shape: &str, p: &crate::schema::catalog::PrimitiveParams) -> Option<Collider> {
    match shape {
        "Cuboid" => {
            let (x, y, z) = p.size.unwrap_or((3.0, 3.0, 3.0));
            Some(Collider::cuboid(x / 2.0, y / 2.0, z / 2.0))
        }
        "Sphere" => Some(Collider::ball(p.radius.unwrap_or(2.0))),
        "Cylinder" => Some(Collider::cylinder(
            p.height.unwrap_or(4.0) / 2.0,
            p.radius.unwrap_or(1.5),
        )),
        _ => None,
    }
}

/// Builds a `StandardMaterial` for a primitive shape.
///
/// Color priority (highest wins):
/// 1. `p.color` — set per-prefab in the prefab catalog RON
/// 2. `project_default` — set via `primitive_default_color` in the project RON
/// 3. Neutral grey `(0.7, 0.7, 0.7)` — engine fallback
pub(crate) fn primitive_material(
    p: &crate::schema::catalog::PrimitiveParams,
    project_default: Option<(f32, f32, f32)>,
) -> StandardMaterial {
    let (r, g, b) = p.color
        .or(project_default)
        .unwrap_or((0.7, 0.7, 0.7));
    StandardMaterial {
        base_color: Color::srgb(r, g, b),
        perceptual_roughness: p.roughness.unwrap_or(0.5),
        metallic: p.metallic.unwrap_or(0.0),
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::catalog::PrimitiveParams;

    fn color_of(mat: &StandardMaterial) -> (f32, f32, f32) {
        let c = mat.base_color.to_srgba();
        (
            (c.red   * 1000.0).round() / 1000.0,
            (c.green * 1000.0).round() / 1000.0,
            (c.blue  * 1000.0).round() / 1000.0,
        )
    }

    #[test]
    fn primitive_material_prefab_color_wins_over_all() {
        let p = PrimitiveParams {
            color: Some((1.0, 0.0, 0.0)),
            ..Default::default()
        };
        let mat = primitive_material(&p, Some((0.0, 1.0, 0.0)));
        assert_eq!(color_of(&mat), (1.0, 0.0, 0.0), "prefab color must take priority");
    }

    #[test]
    fn primitive_material_project_default_wins_over_grey() {
        let p = PrimitiveParams::default(); // no color set
        let mat = primitive_material(&p, Some((0.2, 0.4, 0.8)));
        assert_eq!(color_of(&mat), (0.2, 0.4, 0.8), "project default must win over grey fallback");
    }

    #[test]
    fn primitive_material_falls_back_to_grey_when_no_defaults() {
        let p = PrimitiveParams::default();
        let mat = primitive_material(&p, None);
        assert_eq!(color_of(&mat), (0.7, 0.7, 0.7), "should fall back to neutral grey");
    }

    #[test]
    fn primitive_material_roughness_and_metallic_defaults() {
        let p = PrimitiveParams::default();
        let mat = primitive_material(&p, None);
        assert!((mat.perceptual_roughness - 0.5).abs() < 1e-6);
        assert!((mat.metallic - 0.0).abs() < 1e-6);
    }

    #[test]
    fn primitive_material_roughness_and_metallic_overrides() {
        let p = PrimitiveParams {
            roughness: Some(0.1),
            metallic: Some(0.9),
            ..Default::default()
        };
        let mat = primitive_material(&p, None);
        assert!((mat.perceptual_roughness - 0.1).abs() < 1e-6);
        assert!((mat.metallic - 0.9).abs() < 1e-6);
    }
}
