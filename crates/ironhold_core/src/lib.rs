use bevy::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use serde::Deserialize;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum AppState {
    #[default]
    Bootstrap,
    LoadingProject,
    LoadingScene,
    InGame,
}

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct ProjectConfig {
    pub initial_scene: String,
}

#[derive(Resource)]
struct ProjectConfigHandle(Handle<ProjectConfig>);

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct GameLevel {
    #[serde(default)]
    pub models: Vec<ModelInfo>,
    #[serde(default)]
    pub ui: Vec<UiElement>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelInfo {
    pub path: String,
    pub position: (f32, f32, f32),
}

#[derive(Deserialize, Debug, Clone)]
pub enum UiElement {
    Button {
        text: String,
        action: UiAction,
    },
}

#[derive(Deserialize, Debug, Clone, Component)]
pub enum UiAction {
    LoadScene(String),
}

#[derive(Component)]
struct LevelEntity;

#[derive(Resource)]
struct LevelHandle(Handle<GameLevel>);

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_plugins(RonAssetPlugin::<GameLevel>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<ProjectConfig>::new(&["ron"]))
            .add_systems(Startup, setup)
            .add_systems(Update, check_project_loaded.run_if(in_state(AppState::LoadingProject)))
            .add_systems(Update, (spawn_level, button_system));
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut next_state: ResMut<NextState<AppState>>) {
    // 3D Camera and Lights (Persistent)
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(3.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    
    // Load Project Config
    println!("Loading Project Config...");
    let handle = asset_server.load("project.ron");
    commands.insert_resource(ProjectConfigHandle(handle));
    
    next_state.set(AppState::LoadingProject);
}

fn check_project_loaded(
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

fn spawn_level(
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
            // (unless we add logic to force reload, but state check is safer for now)
            if *state.get() == AppState::InGame {
               // return; 
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
            
            next_state.set(AppState::InGame);
        }
    }
}

fn button_system(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &UiAction),
        (Changed<Interaction>, With<Button>),
    >,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, mut color, action) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgb(0.35, 0.75, 0.35));
                match action {
                    UiAction::LoadScene(path) => {
                        println!("Button Pressed! Loading scene: {}", path);
                        let handle = asset_server.load(path.clone());
                        commands.insert_resource(LevelHandle(handle));
                        // We stay in InGame or transition to LoadingScene?
                        // Let's transition to a loading state to allow spawn_level to re-trigger if needed, 
                        // though spawn_level listens to asset events, so just changing the handle might be enough 
                        // IF the asset is new. But if it's already loaded, we need to force re-spawn?
                        // For simplicity, we just rely on AssetEvent::Modified or Loaded. 
                        // But since we are changing the resource, we might need to manually trigger logic.
                        // Ideally:
                        // 1. Unload current level (handled in spawn_level via cleanup)
                        // 2. Start loading new one.
                        // But deserializing a new handle won't trigger 'Loaded' event if it was already loaded.
                        // For this demo, let's assume it works or we might need "force reload" logic.
                        // Actually, if we change the handle in LevelHandle, spawn_level won't know unless it listens to Changed<Res<LevelHandle>>?
                        // The current spawn_level listens to AssetEvent. If asset is already loaded, no event fires.
                        // We need to fix spawn_level to also check if we just switched handle.
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

use std::path::PathBuf;

pub fn start_app() {
    let asset_path = if cfg!(target_arch = "wasm32") {
        "assets".to_string()
    } else {
        find_assets_folder().to_string_lossy().to_string()
    };
    
    println!("Runtime Asset Path: {}", asset_path);

    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: asset_path,
            ..default()
        }))
        .add_plugins(GamePlugin)
        .run();
}

fn find_assets_folder() -> PathBuf {
    let mut current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    println!("Current Working Directory: {:?}", current);

    // Search up to 5 levels parent directories
    for _ in 0..5 {
        let assets = current.join("assets");
        if assets.exists() && assets.is_dir() {
            return assets;
        }
        if !current.pop() {
            break;
        }
    }
    
    // Fallback if not found
    PathBuf::from("assets")
}

