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

/// When inserted before app startup, overrides the project's `initial_scene` so a
/// specific scene can be loaded directly without going through the normal flow.
/// Used by the WASM test harness via the `?scene=<path>` URL parameter.
#[derive(Resource)]
pub struct InitialSceneOverride(pub String);

/// The directory prefix of the loaded project file, relative to the assets root.
/// Used to resolve project-relative paths (e.g. scene paths in project.ron).
/// Empty string means the project file is at the assets root.
#[derive(Resource, Clone, Default)]
pub struct ProjectRoot(pub String);

/// Live game state exposed to the DOM (WASM) and available to tests.
/// Updated every frame by `update_debug_state`.
#[derive(Resource, Default)]
pub struct DebugState {
    pub frame: u64,
    /// String form of the current `AppState` variant (e.g. `"InGame"`).
    pub app_state: String,
    /// Debug representation of the last `Action` executed (e.g. `PlayAnimation("dance")`).
    pub last_action: String,
    /// Asset path of the most recently fully-loaded scene.
    pub scene: String,
    /// Current named logic state set by `Action::EnterState`. Empty string means no active state.
    pub logic_state: String,
}

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
            .init_resource::<DebugState>()
            .init_resource::<ActionQueue>()
            .init_resource::<ModelSpawner>()
            .init_resource::<crate::runtime::scene_manager::MergedModelFixes>()
            .init_resource::<crate::runtime::scene_manager::LoadedRules>()
            .init_resource::<crate::runtime::scene_manager::LoadedStateMachine>()
            .init_resource::<crate::runtime::scene_manager::LoadedKeyBindings>()
            .init_resource::<crate::runtime::scene_manager::LoadedAssetCatalog>()
            .init_resource::<crate::runtime::scene_manager::LoadedPrefabCatalog>()
            .init_resource::<crate::runtime::scene_manager::LoadedSpawnPoints>()
            .init_resource::<crate::runtime::scene_manager::SpawnRegistry>()
            .init_resource::<crate::runtime::scene_manager::PendingSceneLoadMode>()
            .init_resource::<crate::runtime::scene_manager::PreloadedScenes>()
            .init_resource::<crate::runtime::scene_manager::LogicState>()
            .init_resource::<crate::runtime::material_factory::BuiltMaterials>()
            .add_message::<UiMessage>()
            .add_message::<SceneEvent>()
            .add_message::<InputActionMessage>()
            .add_message::<AppExit>()
            .add_plugins(RonAssetPlugin::<GameLevel>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<ProjectConfig>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::project::ModelFixesAsset>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::project::LogicRulesAsset>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::project::StateMachineAsset>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::player::AnimationPolicy>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::scene_v2::GameSceneV2>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::catalog::AssetCatalog>::new(&["ron"]))
            .add_plugins(RonAssetPlugin::<crate::schema::catalog::PrefabCatalog>::new(&["ron"]))
            .add_plugins(capabilities::terrain::TerrainPlugin)
            .add_plugins(capabilities::custom_material::CustomMaterialPlugin)
            .add_plugins(capabilities::physics::PhysicsPlugin)
            .add_systems(Startup, setup)
            .add_systems(Update, check_project_loaded.run_if(in_state(AppState::LoadingProject)))
            // Scene + UI + input
            .add_systems(Update, (
                spawn_level,
                // spawn_scene_v2 must run BEFORE the message/action pipeline each frame.
                // The action executor sets load_mode (ResMut, immediate) but updates
                // SceneHandleV2 via commands (deferred). If spawn_scene_v2 ran after the
                // executor in the same frame it would see load_mode=Overlay with the old
                // SceneHandleV2, triggering a spurious spawn and resetting load_mode to
                // Replace before the correct handle is ever visible.
                spawn_scene_v2.before(message_interpreter_system),
                spawn_player_when_terrain_ready,
                animation_policy_loader_system,
                apply_material_overrides,
                button_system,
            ))
            // Global key input (ESC, etc.) → UI messages, must run before interpreter
            .add_systems(Update, global_input_system.before(message_interpreter_system))
            // Messages -> actions (chained: interpreters must run before executor each frame)
            .add_systems(Update, (
                message_interpreter_system,
                fsm_interpreter_system,
                action_executor_system,
            ).chain())
            // Physics-driven input + movement must run in FixedUpdate for stable simulation
            .add_systems(FixedUpdate, (
                input_translator_system,
                player_movement_system,
            ).chain())
            // Visual/animation pipeline stays in Update (rendering cadence, not physics)
            .add_systems(Update, (
                animation_resolver_system,
                camera_orbit_system,
                fly_camera_system,
                animation_playback_system,
            ).chain())
            // Debug state (runs last so it sees the final app_state for this frame)
            .add_systems(Update, update_flycam_position_label.after(fly_camera_system))
            .add_systems(PostUpdate, update_debug_state);

        #[cfg(target_arch = "wasm32")]
        app.add_systems(PostUpdate, sync_debug_state_to_dom.after(update_debug_state));
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

fn update_debug_state(
    mut debug: ResMut<DebugState>,
    state: Res<State<AppState>>,
    mut scene_events: MessageReader<SceneEvent>,
    logic_state: Res<crate::runtime::scene_manager::LogicState>,
) {
    debug.frame += 1;
    debug.app_state = format!("{:?}", state.get());
    debug.logic_state = logic_state.0.clone();
    for event in scene_events.read() {
        if let SceneEvent::Ready(path) = event {
            debug.scene = path.clone();
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_debug_state_to_dom(debug: Res<DebugState>) {
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let Some(el) = document.get_element_by_id("debug-state") else { return };
    let json = format!(
        r#"{{"frame":{},"app_state":"{}","last_action":"{}","scene":"{}","logic_state":"{}"}}"#,
        debug.frame,
        debug.app_state,
        debug.last_action.replace('"', "\\\""),
        debug.scene.replace('"', "\\\""),
        debug.logic_state.replace('"', "\\\""),
    );
    el.set_inner_html(&json);
}

pub fn start_app(project_path: Option<String>, scene_override: Option<String>) {
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
    if let Some(ref s) = scene_override {
        info!("Initial scene override: {}", s);
    }

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        file_path: asset_path,
        meta_check: bevy::asset::AssetMetaCheck::Never,
        ..default()
    }))
    .insert_resource(ProjectConfigPath(config_path))
    .insert_resource(ProjectRoot(project_root));

    if let Some(scene) = scene_override {
        app.insert_resource(InitialSceneOverride(scene));
    }

    app.add_plugins(GamePlugin).run();
}


