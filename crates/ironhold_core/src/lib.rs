#[allow(unused_imports)]
use bevy::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;

pub mod schema;
pub mod runtime;
pub mod capabilities;
pub mod utils;

// Optional debug inspector (native + web)
#[cfg(feature = "inspector")]
pub mod inspector;


use crate::schema::*;
use crate::runtime::*;
use crate::capabilities::*;
use crate::utils::find_assets_folder;

#[derive(Resource)]
pub struct ProjectConfigPath(pub String);

/// The directory prefix of the loaded project file, relative to the assets root.
/// Used to resolve project-relative paths (e.g. scene paths in project.ron).
/// Empty string means the project file is at the assets root.
#[derive(Resource, Clone, Default)]
pub struct ProjectRoot(pub String);

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "inspector")]
        inspector::add_inspector_plugins(app);
        // Register custom component types for runtime inspection (bevy-inspector-egui)
        #[cfg(feature = "inspector")]
        {
            app.register_type::<LocomotionState>();
            app.register_type::<ActiveOverride>();
            app.register_type::<AnimationController>();
        }

        app.init_state::<AppState>()
            .init_resource::<ActionQueue>()
            .init_resource::<ModelSpawner>()
            .init_resource::<crate::runtime::scene_manager::MergedModelFixes>()
            .init_resource::<crate::runtime::scene_manager::LoadedRules>()
            .init_resource::<crate::runtime::scene_manager::LoadedAssetCatalog>()
            .init_resource::<crate::runtime::scene_manager::LoadedPrefabCatalog>()
            .add_message::<UiMessage>()
            .add_message::<SceneEvent>()
            .add_message::<InputActionMessage>()
            .add_message::<AppExit>()
            .add_plugins(RonAssetPlugin::<GameLevel>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<ProjectConfig>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::project::ModelFixesAsset>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::project::LogicRulesAsset>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::player::AnimationPolicy>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::scene_v2::GameSceneV2>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::catalog::AssetCatalog>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::catalog::PrefabCatalog>::new(&["ron"]))
            .add_plugins(capabilities::terrain::TerrainPlugin)
            .add_plugins(capabilities::physics::PhysicsPlugin)
            .add_systems(Startup, setup)
            .add_systems(Update, check_project_loaded.run_if(in_state(AppState::LoadingProject)))
            // Scene + UI + input
            .add_systems(Update, (
                spawn_level,
                spawn_scene_v2,
                spawn_player_when_terrain_ready,
                animation_policy_loader_system,
                button_system,
                input_translator_system,
            ))
            // Messages -> actions
            .add_systems(Update, (
                message_interpreter_system,
                action_executor_system,
            ))
            // Capability pipeline (ordered): movement -> resolver -> playback
            .add_systems(Update, (
                player_movement_system,
                animation_resolver_system,
                camera_orbit_system,
                animation_playback_system,
            ).chain());
    }
}

fn setup(
    mut commands: Commands, 
    asset_server: Res<AssetServer>, 
    mut next_state: ResMut<NextState<AppState>>,
    config_path: Res<ProjectConfigPath>,
) {
    info!("SETUP Startup System Running: config_path={}", config_path.0);
    // Persistent UI camera for overlays (Egui / Inspector). Not tagged LevelEntity,
    // so it survives scene transitions.
    commands.spawn((
        Name::new("Persistent Overlay Camera"),
        Camera2d,
        // bevy::ui::IsDefaultUiCamera,
        bevy::prelude::Camera {
            order: 1000, 
            clear_color: ClearColorConfig::None,
            ..default()
        },
    ));

    
    // Load Project Config
    let handle = asset_server.load(config_path.0.clone());
    commands.insert_resource(ProjectConfigHandle(handle));
    info!("Inserted ProjectConfigHandle");
    next_state.set(AppState::LoadingProject);
}

fn button_system(
    interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &UiAction),
        (Changed<Interaction>, With<Button>),
    >,
    mut ui_events: MessageWriter<UiMessage>,
    #[cfg(feature = "inspector")]
    inspector_enabled: Option<Res<crate::inspector::InspectorEnabled>>,
) {
    #[cfg(feature = "inspector")]
    if let Some(enabled) = inspector_enabled {
        if enabled.0 {
            return;
        }
    }

    let mut interaction_query = interaction_query;
    for (interaction, mut color, action) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgb(0.35, 0.75, 0.35));
                match action {
                    UiAction::Trigger(trigger) => {
                        info!("Button Pressed! Emitting UiMessage: {}", trigger);
                        ui_events.write(UiMessage::ButtonPressed(trigger.clone()));
                    }
                }
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgb(0.25, 0.25, 0.25));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgb(0.15, 0.15, 0.15));
            }
        }
    }
}

pub fn start_app(project_path: Option<String>) {
    let asset_path = if cfg!(target_arch = "wasm32") {
        "assets".to_string()
    } else {
        find_assets_folder().to_string_lossy().to_string()
    };
    
    let config_path = project_path.unwrap_or_else(|| "projects/quick_scene/quick_scene.project.ron".to_string());

    let project_root = std::path::Path::new(&config_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .replace('\\', "/")
        .to_string();
    
    info!("Runtime Asset Path: {}", asset_path);
    info!("Project Config Path: {}", config_path);

    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: asset_path,
            meta_check: bevy::asset::AssetMetaCheck::Never,
            ..default()
        }))
        .insert_resource(ProjectConfigPath(config_path))
        .insert_resource(ProjectRoot(project_root))
        .add_plugins(GamePlugin)
        .run();
}


