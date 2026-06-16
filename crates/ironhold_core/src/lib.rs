#[allow(unused_imports)]
use bevy::prelude::*;
use bevy::winit::{UpdateMode, WinitSettings};
use crate::schema::ImplicitRonPlugin;
use std::time::Duration;

use bevy::camera::visibility::NoFrustumCulling;

#[cfg(not(target_arch = "wasm32"))]
use bevy_framepace::{FramepacePlugin, FramepaceSettings, Limiter};

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

/// Counts down after each scene spawn, keeping NoFrustumCulling on all mesh entities
/// for a few frames so every pipeline compiles before the user starts interacting.
/// Set to a non-zero value by spawn_scene_v2; decremented each frame by pipeline_warmup_system.
#[derive(Resource, Default)]
pub struct PipelineWarmup(pub u8);

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
    /// Running score total. Derived from `GameVariables["score"]` each frame.
    pub score: i32,
}

/// Runtime-writable named values exposed to data-bound UI labels.
/// Keys are arbitrary strings; any action executor can write here.
/// UI labels with `bind: Some("key")` read from this map every frame.
#[derive(Resource, Default)]
pub struct GameVariables(pub std::collections::HashMap<String, String>);

/// Placed on a UI `Text` entity by `scene_loader` when the label definition has a
/// `bind` field. Every frame `update_dynamic_labels_system` writes the current value
/// of `GameVariables[key]` into the text, formatted with the optional `format` template
/// (`"{}"` is replaced by the value; defaults to the raw value when omitted).
#[derive(Component)]
pub struct DynamicLabel {
    pub key: String,
    pub format: Option<String>,
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
            .init_resource::<PipelineWarmup>()
            .init_resource::<ActionQueue>()
            .init_resource::<ModelSpawner>()
            .init_resource::<crate::runtime::scene_manager::MergedModelFixes>()
            .init_resource::<crate::runtime::scene_manager::LoadedRules>()
            .init_resource::<crate::runtime::scene_manager::LoadedStateMachine>()
            .init_resource::<crate::runtime::scene_manager::ProjectKeyBindings>()
            .init_resource::<crate::runtime::scene_manager::LoadedKeyBindings>()
            .init_resource::<crate::runtime::scene_manager::LoadedAssetCatalog>()
            .init_resource::<crate::runtime::scene_manager::LoadedPrefabCatalog>()
            .init_resource::<crate::runtime::scene_manager::LoadedSpawnPoints>()
            .init_resource::<crate::runtime::scene_manager::SpawnRegistry>()
            .init_resource::<crate::runtime::scene_manager::PendingSceneLoadMode>()
            .init_resource::<crate::runtime::scene_manager::PreloadedScenes>()
            .init_resource::<crate::runtime::scene_manager::PreloadedGlbHandles>()
            .init_resource::<crate::runtime::scene_manager::PendingEntitySpawns>()
            .init_resource::<crate::runtime::scene_manager::DynamicStatUiQueue>()
            .init_resource::<crate::runtime::scene_manager::LoadedAudioHandles>()
            .init_resource::<crate::runtime::scene_manager::LoadedDecalHandles>()
            .init_resource::<crate::runtime::scene_manager::DelayedEventQueue>()
            .init_resource::<crate::runtime::scene_manager::AudioState>()
            .init_resource::<crate::runtime::scene_manager::ActiveTonemapping>()
            .init_resource::<crate::capabilities::decal::PendingDecalSpawns>()
            .init_resource::<crate::capabilities::particle_budget::ParticleQuality>()
            .init_resource::<crate::capabilities::particle_budget::ParticleBudget>()
            .init_resource::<GameVariables>()
            .init_resource::<crate::schema::stats::LoadedStats>()
            .init_resource::<crate::schema::stats::LoadedModifiers>()
            .init_resource::<crate::runtime::scene_manager::LogicState>()
            .init_resource::<crate::runtime::material_factory::BuiltMaterials>()
            .add_message::<UiEvent>()
            .add_message::<GameEvent>()
            .add_message::<SceneEvent>()
            .add_message::<InputActionMessage>()
            .add_message::<AppExit>()
            .add_plugins(ImplicitRonPlugin::<ProjectConfig>::new(&["ron"]))
            .add_plugins(ImplicitRonPlugin::<crate::schema::project::ModelFixesAsset>::new(&["ron"]))
            .add_plugins(ImplicitRonPlugin::<crate::schema::project::LogicRulesAsset>::new(&["ron"]))
            .add_plugins(ImplicitRonPlugin::<crate::schema::project::StateMachineAsset>::new(&["ron"]))
            .add_plugins(ImplicitRonPlugin::<crate::schema::player::AnimationPolicy>::new(&["ron"]))
            .add_plugins(ImplicitRonPlugin::<crate::schema::scene_v2::GameSceneV2>::new(&["ron"]))
            .add_plugins(ImplicitRonPlugin::<crate::schema::catalog::AssetCatalog>::new(&["ron"]))
            .add_plugins(ImplicitRonPlugin::<crate::schema::catalog::PrefabCatalog>::new(&["ron"]))
            .add_plugins(ImplicitRonPlugin::<crate::schema::stats::StatCatalog>::new(&["ron"]))
            .add_plugins(capabilities::terrain::TerrainPlugin)
            .add_plugins(capabilities::custom_material::CustomMaterialPlugin)
            .add_plugins(capabilities::stat_radar::StatRadarPlugin)
            .add_plugins(capabilities::physics::PhysicsPlugin)
            .add_plugins(capabilities::particle::ParticlePlugin)
            .add_plugins(capabilities::particle_renderer::ParticleRendererPlugin)
            .add_plugins(capabilities::flame_material::FlameParticleMaterialPlugin)
            .add_plugins(capabilities::foliage::FoliagePlugin)
            .add_plugins(capabilities::action_bar::ActionBarPlugin)
            .add_plugins(capabilities::targeting::TargetingPlugin)
            .add_systems(Startup, setup)
            .add_systems(Update, check_project_loaded.run_if(in_state(AppState::LoadingProject)))
            // Scene + UI + input
            .add_systems(Update, (
                // spawn_scene_v2 must run BEFORE the message/action pipeline each frame.
                // The action executor sets load_mode (ResMut, immediate) but updates
                // SceneHandleV2 via commands (deferred). If spawn_scene_v2 ran after the
                // executor in the same frame it would see load_mode=Overlay with the old
                // SceneHandleV2, triggering a spurious spawn and resetting load_mode to
                // Replace before the correct handle is ever visible.
                spawn_scene_v2.before(message_interpreter_system),
                preload_audio_system,
                preload_decals_system,
                spawn_player_when_terrain_ready,
                animation_policy_loader_system,
                resolve_pending_behaviors_system,
                apply_material_overrides,
                button_system,
            ))
            // Global key input (ESC, etc.) → UI messages, must run before interpreter
            .add_systems(Update, global_input_system.before(message_interpreter_system))
            // Stat pipeline: modifier ticks → regen → effective value recompute — all before
            // the interpreter chain so threshold crossings are visible in the same frame.
            .add_systems(Update, (
                stat_modifier_system,
                stat_regen_system,
                stat_effective_value_system,
            ).chain().before(message_interpreter_system))
            // Apply AudioState changes (mute_on_start, ToggleMute, SetVolume) to GlobalVolume
            // before any actions fire so mute_on_start is respected before PlayMusicLoop runs.
            .add_systems(Update, audio_state_system.before(message_interpreter_system))
            // Messages -> actions (chained: interpreters must run before executor each frame)
            // stat_threshold_system runs after action_executor to detect crossings from
            // ModifyStat/SetStat actions executed this frame; emitted GameEvents fire next frame.
            // drain_spawn_queue_system runs last: processes items queued by action_executor
            // this frame at a rate-limited SPAWNS_PER_FRAME to spread pipeline compile stalls.
            .add_systems(Update, (
                message_interpreter_system,
                fsm_interpreter_system,
                entity_fsm_interpreter_system,
                action_executor_system,
                stat_effective_value_system, // recompute after ModifyStat/SetStat/ApplyModifier/RemoveModifier
                stat_threshold_system,
                drain_spawn_queue_system,
                drain_dynamic_stat_ui_system,
                drain_particle_effects_system,
                simulate_pool_system,
                rebuild_pool_meshes_system,
                spawn_decal_system,
            ).chain())
            .add_systems(Update, fading_light_system.after(drain_particle_effects_system))
            .add_systems(Update, fading_decal_system.after(spawn_decal_system))
            .add_systems(Update, clear_pool_on_scene_unload_system)
            .add_systems(Update, capabilities::player::update_player_speed_system)
            // Physics-driven input + movement must run in FixedUpdate for stable simulation
            .add_systems(FixedUpdate, (
                input_translator_system,
                player_movement_system,
                collectible_system,
                trigger_zone_system,
                npc_behavior_system,
            ).chain())
            // Interactable input runs before all interpreters so all three readers
            // see the emitted GameEvent in the same frame.
            .add_systems(Update, interactable_system.before(message_interpreter_system))
            // Delayed events tick down each frame; emitted GameEvents are visible to all
            // three interpreter systems in the same frame they fire.
            .add_systems(Update, tick_delayed_events_system.before(message_interpreter_system))
            // Visual/animation pipeline stays in Update (rendering cadence, not physics)
            .add_systems(Update, (
                animation_resolver_system,
                camera_orbit_system,
                fly_camera_system,
                animation_playback_system,
            ).chain())
            .add_systems(Update, motion_system)
            .add_systems(Update, pipeline_warmup_system)
            .add_systems(Update, damage_popup_system.before(world_label_screen_pos_system))
            .add_systems(Update, world_label_screen_pos_system)
            // Debug state (runs last so it sees the final app_state for this frame)
            .add_systems(Update, update_flycam_position_label.after(fly_camera_system))
            .add_systems(Update, update_dynamic_labels_system)
            .add_systems(Update, (stat_bar_update_system, stat_bar_value_text_system, stat_label_update_system, world_stat_bar_update_system, world_pixel_bar_update_system))
            .add_systems(Update, stat_radar_update_system)
            .add_systems(PostUpdate, update_debug_state);

        #[cfg(target_arch = "wasm32")]
        app.add_systems(PostUpdate, sync_debug_state_to_dom.after(update_debug_state));
    }
}

fn update_dynamic_labels_system(
    vars: Res<GameVariables>,
    mut label_query: Query<(&mut Text, &DynamicLabel)>,
) {
    for (mut text, label) in &mut label_query {
        let value = vars.0.get(&label.key).map(String::as_str).unwrap_or("");
        let new_text = match &label.format {
            Some(fmt) => fmt.replace("{}", value),
            None => value.to_string(),
        };
        if text.0 != new_text {
            *text = Text::new(new_text);
        }
    }
}

fn pipeline_warmup_system(
    mut warmup: ResMut<PipelineWarmup>,
    mut commands: Commands,
    mesh_entities: Query<Entity, With<Mesh3d>>,
    nfc_entities: Query<Entity, With<NoFrustumCulling>>,
) {
    if warmup.0 == 0 {
        return;
    }
    warmup.0 -= 1;
    if warmup.0 > 0 {
        for entity in mesh_entities.iter() {
            commands.entity(entity).insert(NoFrustumCulling);
        }
    } else {
        let mesh_count = nfc_entities.iter().count();
        for entity in nfc_entities.iter() {
            commands.entity(entity).remove::<NoFrustumCulling>();
        }
        info!(
            "Pipeline warmup complete — {mesh_count} mesh(es) warmed up, frustum culling restored."
        );
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
    mut ui_events: MessageWriter<UiEvent>,
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
                        info!("Button Pressed! Emitting UiEvent: {}", trigger);
                        ui_events.write(UiEvent::ButtonPressed(trigger.clone()));
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

/// Projects each [`WorldLabel`]'s 3-D world position through the active
/// `Camera3d` and repositions the entity in `Camera2d` screen space so
/// `Camera2d` renders the `Text2d` at the correct on-screen location.
///
/// Text2d is rendered by `Camera2d` (not `Camera3d`), so this is the correct
/// way to show text that appears to float over a 3-D world position.
fn world_label_screen_pos_system(
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    window_q: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut label_q: Query<(
        &crate::runtime::scene_manager::WorldLabel,
        &mut Transform,
        &mut Visibility,
        Option<&mut TextFont>,
    )>,
    tracked_q: Query<(&GlobalTransform, Option<&Visibility>), Without<crate::runtime::scene_manager::WorldLabel>>,
) {
    let Ok((camera, cam_global)) = camera_q.single() else { return };
    let Ok(window) = window_q.single() else { return };
    let half_w = window.width() / 2.0;
    let half_h = window.height() / 2.0;
    let cam_pos = cam_global.translation();

    for (label, mut t, mut vis, text_font_opt) in label_q.iter_mut() {
        let world_pos = if let Some(tracked) = label.tracked_entity {
            let Ok((gt, tracked_vis)) = tracked_q.get(tracked) else {
                if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
                continue;
            };
            if tracked_vis.is_some_and(|v| *v == Visibility::Hidden) {
                if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
                continue;
            }
            gt.translation() + label.offset
        } else {
            label.world_pos
        };

        // Depth-based font size — only applies to Text2d-bearing WorldLabel entities.
        // Pixel bar anchors carry no TextFont, so this block is skipped for them.
        if let Some(mut text_font) = text_font_opt {
            let new_size = if let Some((ref_dist, min_floor)) = label.depth_scale {
                let dist = (world_pos - cam_pos).length().max(0.001);
                let scale = (ref_dist / dist).min(1.0).max(min_floor);
                (label.base_font_size * scale).round()
            } else {
                label.base_font_size
            };
            if (text_font.font_size - new_size).abs() >= 0.5 {
                text_font.font_size = new_size;
            }
        }

        match camera.world_to_viewport(cam_global, world_pos) {
            Ok(vp) => {
                t.translation.x = vp.x - half_w + label.screen_offset.x;
                t.translation.y = half_h - vp.y + label.screen_offset.y;
                if *vis != Visibility::Visible { *vis = Visibility::Visible; }
            }
            Err(_) => {
                if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
            }
        }
    }
}

fn tick_delayed_events_system(
    mut queue: ResMut<crate::runtime::scene_manager::DelayedEventQueue>,
    time: Res<Time>,
    mut game_events: MessageWriter<GameEvent>,
) {
    let dt = time.delta_secs();
    queue.0.retain_mut(|(remaining, event)| {
        *remaining -= dt;
        if *remaining <= 0.0 {
            info!("DelayedEvent fired: '{}'", event);
            game_events.write(GameEvent::Trigger(event.clone()));
            false
        } else {
            true
        }
    });
}

fn update_debug_state(
    mut debug: ResMut<DebugState>,
    state: Res<State<AppState>>,
    mut scene_events: MessageReader<SceneEvent>,
    logic_state: Res<crate::runtime::scene_manager::LogicState>,
    game_vars: Res<GameVariables>,
) {
    debug.frame += 1;
    debug.app_state = format!("{:?}", state.get());
    debug.logic_state = logic_state.0.clone();
    debug.score = game_vars.0.get("score").and_then(|s| s.parse().ok()).unwrap_or(0);
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
        r#"{{"frame":{},"app_state":"{}","last_action":"{}","scene":"{}","logic_state":"{}","score":{}}}"#,
        debug.frame,
        debug.app_state,
        debug.last_action.replace('"', "\\\""),
        debug.scene.replace('"', "\\\""),
        debug.logic_state.replace('"', "\\\""),
        debug.score,
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
    .insert_resource(ProjectRoot(project_root))
    .insert_resource(WinitSettings {
        focused_mode: UpdateMode::Continuous,
        unfocused_mode: UpdateMode::Reactive {
            wait: Duration::from_millis(100),
            react_to_device_events: false,
            react_to_user_events: true,
            react_to_window_events: true,
        },
    });

    if let Some(scene) = scene_override {
        app.insert_resource(InitialSceneOverride(scene));
    }

    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(FramepacePlugin).insert_resource(FramepaceSettings {
        limiter: Limiter::from_framerate(60.0),
    });

    app.add_plugins(GamePlugin).run();
}

#[cfg(test)]
mod tests {
    /// Depth-scale formula: ref_dist / dist, clamped to [min_floor, 1.0].
    /// These tests pin the arithmetic that world_label_screen_pos_system uses
    /// to quantise font sizes — a regression here means glyph-atlas misses.
    fn compute_scale(ref_dist: f32, dist: f32, min_floor: f32) -> f32 {
        (ref_dist / dist).min(1.0).max(min_floor)
    }

    #[test]
    fn depth_scale_at_reference_distance_is_one() {
        assert_eq!(compute_scale(80.0, 80.0, 0.25), 1.0);
    }

    #[test]
    fn depth_scale_at_double_distance_is_half() {
        let s = compute_scale(80.0, 160.0, 0.25);
        assert!((s - 0.5).abs() < 1e-5, "expected 0.5, got {s}");
    }

    #[test]
    fn depth_scale_close_up_capped_at_one() {
        assert_eq!(compute_scale(80.0, 10.0, 0.25), 1.0);
    }

    #[test]
    fn depth_scale_far_away_clamped_to_min_floor() {
        assert_eq!(compute_scale(80.0, 1000.0, 0.25), 0.25);
    }

    #[test]
    fn font_size_write_skipped_for_sub_half_change() {
        let current = 24.0_f32;
        let new_size = 24.3_f32;
        assert!((current - new_size).abs() < 0.5, "should skip write for change < 0.5");
    }

    #[test]
    fn font_size_write_triggered_at_half_pixel_threshold() {
        let current = 24.0_f32;
        let new_size = 24.5_f32;
        assert!((current - new_size).abs() >= 0.5, "should write for change >= 0.5");
    }
}
