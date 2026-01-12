use bevy::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;

pub mod schema;
pub mod runtime;
pub mod capabilities;
pub mod utils;

use crate::schema::*;
use crate::runtime::*;
use crate::capabilities::*;
use crate::utils::find_assets_folder;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<ActionQueue>()
            .add_message::<UiMessage>()
            .add_plugins(RonAssetPlugin::<GameLevel>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<ProjectConfig>::new(&["ron"]))
            .add_systems(Startup, setup)
            .add_systems(Update, check_project_loaded.run_if(in_state(AppState::LoadingProject)))
            .add_systems(Update, (
                spawn_level,
                button_system,
            ))
            .add_systems(Update, (
                message_interpreter_system,
                action_executor_system,
            ))
            .add_systems(Update, (
                player_movement_system,
                camera_orbit_system,
                animation_playback_system,
            ));
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut next_state: ResMut<NextState<AppState>>) {
    // Directional Light (Persistent)
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

fn button_system(
    interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &UiAction),
        (Changed<Interaction>, With<Button>),
    >,
    mut ui_events: MessageWriter<UiMessage>,
) {
    let mut interaction_query = interaction_query;
    for (interaction, mut color, action) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgb(0.35, 0.75, 0.35));
                match action {
                    UiAction::LoadScene(path) => {
                        println!("Button Pressed! Emitting UiMessage for scene: {}", path);
                        ui_events.write(UiMessage::ButtonPressed(path.clone()));
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
