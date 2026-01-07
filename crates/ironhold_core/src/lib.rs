use bevy::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use serde::Deserialize;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RonAssetPlugin::<GameLevel>::new(&["ron"]))
            .add_systems(Startup, setup)
            .add_systems(Update, spawn_level);
    }
}

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct GameLevel {
    pub models: Vec<ModelInfo>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelInfo {
    pub path: String,
    pub position: (f32, f32, f32),
}

#[derive(Resource)]
struct LevelHandle(Handle<GameLevel>);

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // 3D Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Directional Light
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(3.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    
    // Load the scene
    let handle = asset_server.load("scenes/main.ron");
    commands.insert_resource(LevelHandle(handle));
    
    println!("Loading Game Level...");
}

fn spawn_level(
    mut commands: Commands,
    level_handle: Res<LevelHandle>,
    levels: Res<Assets<GameLevel>>,
    asset_server: Res<AssetServer>,
    mut events: MessageReader<AssetEvent<GameLevel>>,
) {
    for event in events.read() {
        if event.is_loaded_with_dependencies(&level_handle.0) {
            if let Some(level) = levels.get(&level_handle.0) {
                println!("Level Loaded! Spawning {} models", level.models.len());
                for model in &level.models {
                    commands.spawn(SceneRoot(
                        asset_server.load(model.path.clone())
                    ))
                    .insert(Transform::from_translation(Vec3::from(model.position)));
                   
                }
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

