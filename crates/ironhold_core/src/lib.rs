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
    #[serde(default)]
    pub player: Option<PlayerConfig>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelInfo {
    pub path: String,
    pub position: (f32, f32, f32),
}

#[derive(Deserialize, Debug, Clone)]
pub struct PlayerConfig {
    pub model_path: String,
    pub initial_position: (f32, f32, f32),
    pub camera: CameraConfig,
    pub inputs: InputMap,
    pub animations: AnimationMap,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CameraConfig {
    pub offset: (f32, f32, f32),
    pub look_at_offset: (f32, f32, f32),
    pub zoom_speed: f32,
    pub orbit_speed: f32,
    pub min_radius: f32,
    pub max_radius: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct InputMap {
    pub forward: String,
    pub backward: String,
    pub left: String,
    pub right: String,
    pub strafe_left: String,
    pub strafe_right: String,
    pub jump: String,
}

impl InputMap {
    fn key(&self, name: &str) -> Option<KeyCode> {
        let s = match name {
            "forward" => &self.forward,
            "backward" => &self.backward,
            "left" => &self.left,
            "right" => &self.right,
            "strafe_left" => &self.strafe_left,
            "strafe_right" => &self.strafe_right,
            "jump" => &self.jump,
            _ => return None,
        };
        Self::parse_key(s)
    }
    
    fn parse_key(s: &str) -> Option<KeyCode> {
        match s {
            "KeyW" | "W" => Some(KeyCode::KeyW),
            "KeyA" | "A" => Some(KeyCode::KeyA),
            "KeyS" | "S" => Some(KeyCode::KeyS),
            "KeyD" | "D" => Some(KeyCode::KeyD),
            "KeyQ" | "Q" => Some(KeyCode::KeyQ),
            "KeyE" | "E" => Some(KeyCode::KeyE),
            "Space" => Some(KeyCode::Space),
            "ShiftLeft" => Some(KeyCode::ShiftLeft),
            "ShiftRight" => Some(KeyCode::ShiftRight),
            _ => None,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct AnimationMap {
    pub idle: String,
    pub walk: String,
    pub run: String,
    pub jump_enter: String,
    pub jump_loop: String,
    pub jump_exit: String,
    pub death: String,
    pub dance: String,
    pub crouch_idle: String,
    pub crouch_forward: String,
    pub roll: String,
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

#[derive(Component)]
pub struct CharacterController {
    pub speed: f32,
    pub rot_speed: f32,
    pub inputs: InputMap,
}

#[derive(Component)]
pub struct OrbitCamera {
    pub target: Entity,
    pub radius: f32,
    pub offset: Vec3,
    pub zoom_speed: f32,
    pub orbit_speed: f32,
    pub min_radius: f32,
    pub max_radius: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub look_at_offset: Vec3,
}

#[derive(Component)]
pub struct AnimationController {
    pub animations: AnimationMap,
    pub current: String,
}

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
            .add_systems(Update, (spawn_level, button_system, player_movement_system, camera_orbit_system));
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
            if *state.get() == AppState::InGame {
                return; 
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
            
            // Spawn Player
            if let Some(player_config) = &level.player {
                println!("Spawning Player...");
                let player_entity = commands.spawn((
                    SceneRoot(asset_server.load(player_config.model_path.clone())),
                    Transform::from_translation(Vec3::from(player_config.initial_position)),
                    LevelEntity,
                    CharacterController {
                        speed: 5.0,
                        rot_speed: 3.0,
                        inputs: player_config.inputs.clone(),
                    },
                    AnimationController {
                        animations: player_config.animations.clone(),
                        current: player_config.animations.idle.clone(),
                    }
                )).id();

                // Spawn Orbit Camera matching config
                let start_pos = Vec3::from(player_config.initial_position) + Vec3::from(player_config.camera.offset);
                
                commands.spawn((
                    Camera3d::default(),
                    Transform::from_translation(start_pos).looking_at(Vec3::from(player_config.initial_position), Vec3::Y),
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
                println!("No player in scene, spawning default camera...");
                commands.spawn((
                    Camera3d::default(),
                    Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
                    LevelEntity,
                ));
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
                        // Transition to LoadingScene to allow spawn_level to run
                        next_state.set(AppState::LoadingScene);
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

fn player_movement_system(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &CharacterController)>,
) {
    for (mut transform, controller) in &mut query {
        let mut velocity = Vec3::ZERO;
        let mut rotation = 0.0;
        
        let forward = transform.forward();
        let right = transform.right();

        if let Some(key) = controller.inputs.key("forward") {
            if keyboard_input.pressed(key) { velocity += *forward; }
        }
        if let Some(key) = controller.inputs.key("backward") {
            if keyboard_input.pressed(key) { velocity -= *forward; }
        }
        if let Some(key) = controller.inputs.key("strafe_right") {
            if keyboard_input.pressed(key) { velocity += *right; }
        }
        if let Some(key) = controller.inputs.key("strafe_left") {
            if keyboard_input.pressed(key) { velocity -= *right; }
        }
        
        // Turning
        if let Some(key) = controller.inputs.key("left") {
            if keyboard_input.pressed(key) { rotation += 1.0; }
        }
        if let Some(key) = controller.inputs.key("right") {
            if keyboard_input.pressed(key) { rotation -= 1.0; }
        }

        // Apply Rotation
        if rotation != 0.0 {
            transform.rotate_y(rotation * controller.rot_speed * time.delta_secs());
        }

        // Apply Movement
        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize();
            transform.translation += velocity * controller.speed * time.delta_secs();
        }
    }
}

fn camera_orbit_system(
    time: Res<Time>,
    mut mouse_motion_events: EventReader<bevy::input::mouse::MouseMotion>,
    mut mouse_wheel_events: EventReader<bevy::input::mouse::MouseWheel>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut camera_query: Query<(&mut Transform, &mut OrbitCamera), Without<CharacterController>>,
    mut character_query: Query<&mut Transform, (With<CharacterController>, Without<OrbitCamera>)>,
) {
    // Collect mouse motion
    let mut mouse_delta = Vec2::ZERO;
    for event in mouse_motion_events.read() {
        mouse_delta += event.delta;
    }

    let zoom_delta: f32 = mouse_wheel_events.read().map(|e| e.y).sum();

    for (mut cam_transform, mut orbit) in &mut camera_query {
        // Zoom
        if zoom_delta != 0.0 {
            orbit.radius -= zoom_delta * orbit.zoom_speed * time.delta_secs();
            orbit.radius = orbit.radius.clamp(orbit.min_radius, orbit.max_radius);
        }

        // Orbit Logic
        let lmb_pressed = mouse_button_input.pressed(MouseButton::Left);
        let rmb_pressed = mouse_button_input.pressed(MouseButton::Right);

        if lmb_pressed || rmb_pressed {
            // Yaw (Left/Right)
            orbit.yaw -= mouse_delta.x * orbit.orbit_speed * time.delta_secs();
            
            // Pitch (Up/Down)
            orbit.pitch -= mouse_delta.y * orbit.orbit_speed * time.delta_secs();
            // Clamp pitch to avoid flipping
            orbit.pitch = orbit.pitch.clamp(0.1, 1.5); 
        }
        
        // If RMB pressed, also rotate character if possible
        if rmb_pressed {
             if let Ok(mut char_transform) = character_query.get_mut(orbit.target) {
                 // We rotate character Y to match camera Yaw (inverse or direct depends on orbit logic)
                 // Usually character faces where camera looks.
                 // orbit.yaw is angle around target.
                 // Let's set character rotation? Or just add delta?
                 // Simple approach: apply mouse delta x to character rotation.
                 char_transform.rotate_y(-mouse_delta.x * orbit.orbit_speed * time.delta_secs());
             }
        }

        // Update Camera Position
        if let Ok(char_transform) = character_query.get(orbit.target) {
            let target_pos = char_transform.translation + orbit.look_at_offset;
            
            // Calculate offset based on yaw/pitch
            let rot = Quat::from_axis_angle(Vec3::Y, orbit.yaw) * Quat::from_axis_angle(Vec3::X, -orbit.pitch);
            let offset = rot * Vec3::Z * orbit.radius;
            
            cam_transform.translation = target_pos + offset;
            cam_transform.look_at(target_pos, Vec3::Y);
        }
    }
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

