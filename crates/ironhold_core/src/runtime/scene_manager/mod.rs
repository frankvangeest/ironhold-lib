use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDimension, TextureViewDescriptor,
};
use std::collections::{BTreeMap, HashMap};

use crate::ProjectRoot;
use crate::schema::*;
use crate::schema::scene_v2::GameSceneV2;
use crate::schema::catalog::{AssetCatalog, PrefabCatalog};
use crate::schema::player::{PlayerConfig, AnimationPolicy};
use crate::runtime::model_spawner::ModelSpawner;
use crate::runtime::material_factory::BuiltMaterials;
use crate::capabilities::terrain_material::TerrainMaterial;

pub mod project_loader;
pub mod scene_loader;
pub mod entity_spawner;
pub mod message_interpreter;
pub mod action_executor;

pub use project_loader::*;
pub use scene_loader::*;
pub use entity_spawner::*;
pub use message_interpreter::*;
pub use action_executor::*;

// ─── Resources ────────────────────────────────────────────────────────────────

/// The final merged model-fix map, assembled from inline project fixes and (optionally) an
/// external `model_fixes.ron` file. Always available after project loading completes.
#[derive(Resource, Default)]
pub struct MergedModelFixes(pub HashMap<String, TransformFix>);

/// The rules loaded for the current project. Populated from inline `config.rules` (v1)
/// or from the external `logic/rules.ron` file (v2). Always available after project loading.
#[derive(Resource, Default, Clone)]
pub struct LoadedRules(pub Vec<LogicRule>);

/// The FSM loaded for the current project. `None` when the project uses `rules.ron` instead.
/// Populated from `logic/state_machine.ron` when `state_machine_path` is set in the project config.
#[derive(Resource, Default, Clone)]
pub struct LoadedStateMachine(pub Option<crate::schema::project::StateMachineAsset>);

/// The current named logic state for the message interpreter.
/// Rules with a matching `when` field only fire in that state.
/// Rules with `when: None` fire regardless of the current state.
/// Set via `Action::EnterState`; default is `""` (no active state).
#[derive(Resource, Default, Clone)]
pub struct LogicState(pub String);

/// Project-level key bindings, set once at project load and never modified.
/// Used by `spawn_scene_v2` to rebuild `LoadedKeyBindings` as the base layer each time
/// a new scene loads, so per-scene overrides don't bleed across scene transitions.
#[derive(Resource, Default, Clone)]
pub struct ProjectKeyBindings(pub HashMap<String, String>);

/// Active key bindings used by `global_input_system`.
/// On each Replace-mode scene load this is rebuilt as: project bindings + scene overrides.
/// Maps key name strings (e.g. "Escape") to event trigger names (e.g. "toggle_pause").
#[derive(Resource, Default, Clone)]
pub struct LoadedKeyBindings(pub HashMap<String, String>);

/// Holds pre-loaded scene asset handles so they stay cached and are ready instantly when needed.
/// Populated by `Action::PreloadScene`. Cleared on a full `LoadScene` so stale handles don't linger.
#[derive(Resource, Default)]
pub struct PreloadedScenes(pub Vec<Handle<GameSceneV2>>);

/// Runtime audio state — tracks the project-level volume ceiling, the current active fraction,
/// and whether the game is muted. Initialized from `ProjectConfig.audio` at project load.
/// Written by `Action::ToggleMute`, `Action::SetVolume`, and `Action::SyncAudioState`;
/// `audio_state_system` applies the effective volume to Bevy's `GlobalVolume` on each change.
#[derive(Resource)]
pub struct AudioState {
    /// Master volume ceiling from `ProjectConfig.audio.max_volume`. Default: 1.0.
    pub max_volume: f32,
    /// Current volume fraction (0.0–1.0), set by `Action::SetVolume`. Default: 1.0.
    pub active_fraction: f32,
    /// Whether the game is currently muted. Toggled by `Action::ToggleMute`. Default: false.
    pub muted: bool,
}

impl Default for AudioState {
    fn default() -> Self { Self { max_volume: 1.0, active_fraction: 1.0, muted: false } }
}

impl AudioState {
    pub fn effective_volume(&self) -> f32 {
        if self.muted { 0.0 } else { (self.active_fraction * self.max_volume).clamp(0.0, 1.0) }
    }
}

/// Watches `AudioState` for changes and writes the computed effective volume to `GlobalVolume`.
/// Label updates are handled through the RON pipeline: `Action::SyncAudioState` emits
/// `audio.muted` / `audio.unmuted` events which the `global_on` bridge in `state_machine.ron`
/// maps to `SetVariable` — keeping label text in RON where designers can change it.
pub fn audio_state_system(
    audio_state: Res<AudioState>,
    mut global_volume: Option<ResMut<bevy::audio::GlobalVolume>>,
) {
    if !audio_state.is_changed() { return; }
    let effective = audio_state.effective_volume();
    if let Some(ref mut gv) = global_volume {
        gv.volume = bevy::audio::Volume::Linear(effective);
    }
}

/// Holds pre-loaded audio asset handles so the asset server cache is warm before first play.
/// Populated by `preload_audio_system` on each `SceneEvent::Ready`. Keeping these handles alive
/// prevents the asset server from evicting audio between scene loads, eliminating first-play I/O
/// latency (the main cause of audible delay on both native and web).
#[derive(Resource, Default)]
pub struct LoadedAudioHandles(pub Vec<Handle<bevy::audio::AudioSource>>);

/// Holds pre-loaded decal texture handles so HTTP fetch is complete before the first
/// `ProjectDecal` fires. Populated by `preload_decals_system` on each `SceneEvent::Ready`.
/// Cleared and repopulated on each scene transition.
#[derive(Resource, Default)]
pub struct LoadedDecalHandles(pub Vec<Handle<Image>>);

/// Resolved target-indicator config for the current scene.
/// `None` means no indicator is configured — the system early-exits silently.
/// Populated by `spawn_scene_v2` on scene load; cleared on full `LoadScene`.
#[derive(Resource, Default)]
pub struct LoadedTargetIndicator(pub Option<ResolvedTargetIndicator>);

/// Resolved (catalog key → texture path) target indicator config for the current scene.
pub struct ResolvedTargetIndicator {
    pub texture_path: String,
    pub radius: f32,
    /// Scene-level fallback colour used when a prefab has no `indicator_color` or
    /// `indicator_category`, or when the category key is absent from `named_colors`.
    pub color: (f32, f32, f32, f32),
    pub offset_y: f32,
    /// Named colour palette from `TargetIndicatorDef.named_colors`. Keyed by category string.
    pub named_colors: std::collections::HashMap<String, (f32, f32, f32, f32)>,
}

/// Holds pre-loaded GLTF scene handles for prefab models, populated by `Action::PreloadPrefab`.
/// Keeping handles alive prevents the asset server from evicting the decoded GLB between scene
/// loads, so the first `Action::Spawn` of that prefab doesn't block the WASM main thread with
/// HTTP fetch + GLTF decode. Cleared on full `LoadScene`.
#[derive(Resource, Default)]
pub struct PreloadedGlbHandles(pub Vec<Handle<bevy::scene::Scene>>);

/// Pending timed events. Each entry is `(remaining_secs, event_name)`.
/// `tick_delayed_events_system` ticks these down each frame and emits `GameEvent::Trigger`
/// when they expire. Cleared on `Action::LoadScene` so no stale events fire after transitions.
#[derive(Resource, Default)]
pub struct DelayedEventQueue(pub Vec<(f32, String)>);

/// Pre-resolved spawn data waiting to be executed. `Action::Spawn` enqueues here instead of
/// calling `spawn_prefab_instance` inline. `drain_spawn_queue_system` processes at most
/// `SPAWNS_PER_FRAME` items per frame, spreading WebGPU pipeline compile stalls across
/// frames when many prefabs are spawned at once (e.g. a wave spawn).
#[derive(Resource, Default)]
pub struct PendingEntitySpawns(pub std::collections::VecDeque<QueuedSpawn>);

/// All data `drain_spawn_queue_system` needs to call `spawn_prefab_instance` or, when
/// `player_config` is `Some`, `spawn_player_entity`. Resolved upfront by the action executor
/// so the drain system needs no catalog access.
pub struct QueuedSpawn {
    pub prefab_def: crate::schema::catalog::PrefabDef,
    pub model_path: String,
    pub transform: Transform,
    pub spawn_id: String,
    /// Prefab catalog key (for `PrefabKey`), distinct from `spawn_id`.
    pub prefab_key: String,
    pub project_root: String,
    /// When the prefab has `tags: ["player"]`, the executor assembles a `PlayerConfig` here.
    /// `drain_spawn_queue_system` calls `spawn_player_entity` instead of `spawn_prefab_instance`.
    pub player_config: Option<crate::schema::player::PlayerConfig>,
}

/// Stores the tonemapping setting from the most recently loaded scene.
/// Read by `drain_spawn_queue_system` when spawning a player via `Action::Spawn`
/// (the orbit camera needs the same tonemapping as the rest of the scene).
#[derive(Resource, Clone, Copy)]
pub struct ActiveTonemapping(pub bevy::core_pipeline::tonemapping::Tonemapping);

impl Default for ActiveTonemapping {
    fn default() -> Self {
        Self(bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted)
    }
}

/// One entry queued by `drain_spawn_queue_system` for each dynamic spawn that has a
/// `stat_label` and/or `world_stat_bar` on its `PrefabDef`. Drained each frame by
/// `drain_dynamic_stat_ui_system` in scene_loader.rs, which contains all the mesh/
/// material spawning code for those widgets.
pub struct DynamicStatUiEntry {
    /// The spawned entity the widget should track.
    pub entity: Entity,
    /// Pre-resolved (with `{self}` replaced) stat key and label def.
    pub stat_label: Option<(String, StatLabelDef)>,
    /// Pre-resolved (with `{self}` replaced) stat key and bar def.
    pub world_stat_bar: Option<(String, WorldStatBarDef)>,
}

/// Pending stat-label and world-stat-bar spawns for entities created by `Action::Spawn`.
/// Populated by `drain_spawn_queue_system`, drained by `drain_dynamic_stat_ui_system`.
#[derive(Resource, Default)]
pub struct DynamicStatUiQueue(pub Vec<DynamicStatUiEntry>);

#[derive(Resource)]
pub struct SceneHandleV2(pub Handle<GameSceneV2>);

#[derive(Resource, Default, Clone)]
pub struct LoadedAssetCatalog(pub AssetCatalog);

#[derive(Resource, Default, Clone)]
pub struct LoadedPrefabCatalog(pub PrefabCatalog);

/// Named spawn points from the most recently loaded scene. Available to `Action::Spawn`
/// so dynamically spawned entities can use scene-defined positions.
#[derive(Resource, Default, Clone)]
pub struct LoadedSpawnPoints(pub BTreeMap<String, (f32, f32, f32)>);

/// Tracks externally-loaded project config files that are still loading.
/// Inserted by `check_project_loaded` on the first frame the project config is ready,
/// removed implicitly once the project transitions to `LoadingScene`.
#[derive(Resource)]
pub struct PendingProjectLoads {
    pub model_fixes: Option<Handle<ModelFixesAsset>>,
    pub rules: Option<Handle<LogicRulesAsset>>,
    pub state_machine: Option<Handle<crate::schema::project::StateMachineAsset>>,
    pub asset_catalog: Option<Handle<AssetCatalog>>,
    pub prefab_catalog: Option<Handle<PrefabCatalog>>,
    pub stats: Option<Handle<crate::schema::stats::StatCatalog>>,
    pub items: Option<Handle<crate::schema::items::ItemCatalog>>,
}

/// Tracks all dynamically spawned entities by their spawn ID.
/// Cleared automatically on scene load so stale IDs don't linger.
#[derive(Resource, Default)]
pub struct SpawnRegistry {
    pub counter: u64,
    pub entities: BTreeMap<String, Entity>,
}

// ─── Components ───────────────────────────────────────────────────────────────

/// Marks every entity that belongs to the currently loaded scene.
/// On a full `LoadScene` all `LevelEntity` entities are despawned before new ones are spawned.
/// Overlay entities use `OverlayEntity` instead.
#[derive(Component)]
pub struct LevelEntity;

/// A screen-space annotation rendered by Camera2d but anchored to a 3D position.
///
/// Each frame `world_label_screen_pos_system` determines the world position:
/// - `tracked_entity = Some(e)`: uses that entity's `GlobalTransform` translation + `offset`.
/// - `tracked_entity = None`: uses the fixed `world_pos` directly.
///
/// The resulting world position is projected through Camera3d and the label's
/// Transform is repositioned in Camera2d screen space.
#[derive(Component)]
pub struct WorldLabel {
    /// Fixed world position (used when `tracked_entity` is None).
    pub world_pos: Vec3,
    /// Entity whose world position this label follows (per-entity labels).
    pub tracked_entity: Option<Entity>,
    /// Offset added to the tracked entity's position (or to `world_pos` — not used for fixed labels).
    pub offset: Vec3,
    /// Authored font size. Stored here so the projection system can recompute
    /// the displayed size each frame without drifting from the original value.
    pub base_font_size: f32,
    /// Resolved depth-scale config: `Some((reference_distance, min_scale_floor))`.
    /// `None` means no depth scaling — font size is always `base_font_size`.
    /// `min_scale_floor` is 0.0 when the designer omitted `min_scale`.
    pub depth_scale: Option<(f32, f32)>,
    /// Screen-space pixel offset applied after world→viewport projection.
    /// Used for drop-shadow duplicates. Zero for all standard labels.
    pub screen_offset: Vec2,
}

/// Stable handle attached to every entity spawned via `Action::Spawn`.
/// Used by `Action::Despawn` to locate and remove the entity.
#[derive(Component, Debug, Clone)]
pub struct SpawnId(pub String);

/// The prefab catalog key this entity was spawned from (e.g. `"enemy_orc_melee"`).
/// Distinct from `SpawnId` which is the per-instance id (e.g. `"orc_01"`). Lets systems
/// show a human-readable type name alongside the instance id (targeting UI, debug, etc.).
#[derive(Component, Debug, Clone)]
pub struct PrefabKey(pub String);

/// Single source of truth for the standard metadata every addressable spawned entity gets,
/// and the only place that registers it in the `SpawnRegistry`. Every spawn site (GLB
/// actor/prop, single-mesh primitive, composite primitive, foliage root, both player paths,
/// and dynamic `Action::Spawn`) routes through this so the set can never drift per-path —
/// the divergence it replaces caused real bugs (GLB actors missing `SpawnId`, GLB player
/// missing `SpeedMultiplier`/`SpawnId`, dynamic spawns missing `PrefabKey`/`LevelEntity`).
///
/// Always inserts `SpawnId` + `PrefabKey` + `LevelEntity` and registers the entity by id.
/// `ClickSelectable`/`Targetable`/`SelectAimHeight` markers are inserted per the flags
/// (players pass `false, false, 1.0`). Player-specific components stay at the call site.
pub fn tag_spawned_entity(
    ec: &mut bevy::ecs::system::EntityCommands,
    registry: &mut SpawnRegistry,
    id: &str,
    prefab_key: &str,
    click_selectable: bool,
    targetable: bool,
    select_aim_height: f32,
) {
    let entity = ec.id();
    ec.insert((SpawnId(id.to_string()), PrefabKey(prefab_key.to_string()), LevelEntity));
    if click_selectable {
        ec.insert(crate::capabilities::targeting::ClickSelectable);
        ec.insert(crate::capabilities::targeting::SelectAimHeight(select_aim_height));
    }
    if targetable {
        ec.insert(crate::capabilities::targeting::Targetable);
    }
    registry.entities.insert(id.to_string(), entity);
}

/// Single source of truth for whether a spawned entity should get a `NameplateTag`.
/// `nameplate: Some(false)` always suppresses; otherwise `show` (scene-level
/// `show_nameplates` or the dynamic-spawn equivalent) or an explicit `nameplate: Some(true))`
/// opt-in enables it.
pub fn should_insert_nameplate(nameplate: Option<bool>, show: bool) -> bool {
    nameplate != Some(false) && (show || nameplate == Some(true))
}

/// Temporary component inserted at spawn time when a prefab has a `behavior` path.
/// Replaced by `BehaviorHandle` + `EntityFsmState` once the asset resolves.
#[derive(Component)]
pub struct PendingBehavior(pub Handle<crate::schema::project::StateMachineAsset>);

/// Keeps the loaded `StateMachineAsset` handle alive so the asset server does not
/// evict the behavior between scene loads. Replaced from `PendingBehavior`.
#[derive(Component)]
pub struct BehaviorHandle(pub Handle<crate::schema::project::StateMachineAsset>);

/// Runtime FSM state for an entity with a `BehaviorHandle`.
/// The current field holds the name of the active state (matches a state in the asset).
#[derive(Component, Default)]
pub struct EntityFsmState {
    pub current: String,
}

#[derive(Component)]
pub struct PendingPlayerConfig(pub PlayerConfig);

/// Stores the scene's tonemapping alongside `PendingPlayerConfig` for the terrain-delayed
/// player spawn path. `spawn_player_when_terrain_ready` reads this to apply the correct
/// tonemapping to the orbit camera once terrain generation is complete.
#[derive(Component)]
pub struct PendingTonemapping(pub bevy::core_pipeline::tonemapping::Tonemapping);

/// Holds a handle to an AnimationPolicy asset that is still loading.
/// Replaced by AnimationPolicyComponent once the asset resolves.
#[derive(Component)]
pub struct PendingAnimationPolicy(pub Handle<AnimationPolicy>);

/// Marks entities that belong to an overlay scene (e.g. pause menu, HUD).
/// Overlay entities are despawned by `Action::UnloadOverlay` and by any full `LoadScene`.
#[derive(Component)]
pub struct OverlayEntity;

/// Marker component for the currently playing background music entity.
/// Used by `Action::PlayMusicLoop` to stop the previous track before starting a new one.
#[derive(Component)]
pub struct BackgroundMusic;

// ─── Load mode ────────────────────────────────────────────────────────────────

/// Controls whether the next `SceneHandleV2` load replaces the world or adds as an overlay.
/// Set by `action_executor_system` before loading; reset to Replace by `spawn_scene_v2` after use.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum PendingSceneLoadMode {
    #[default]
    Replace,
    Overlay,
}

// ─── SystemParams ─────────────────────────────────────────────────────────────

/// Bundled SystemParam for spawn/despawn operations in `action_executor_system`.
/// Groups resources that would otherwise push the system past Bevy's 16-param limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SpawnParams<'w, 's> {
    pub registry: ResMut<'w, SpawnRegistry>,
    pub prefab_catalog: Res<'w, LoadedPrefabCatalog>,
    pub spawn_points: Res<'w, LoadedSpawnPoints>,
    pub model_spawner: Res<'w, ModelSpawner>,
    pub merged_fixes: Res<'w, MergedModelFixes>,
    pub spawned: Query<'w, 's, (Entity, &'static SpawnId)>,
    pub pending_spawns: ResMut<'w, PendingEntitySpawns>,
    pub pending_particles: ResMut<'w, crate::capabilities::particle::PendingParticleEffects>,
    pub pending_decals: ResMut<'w, crate::capabilities::decal::PendingDecalSpawns>,
    pub particle_quality: ResMut<'w, crate::capabilities::particle_budget::ParticleQuality>,
}

/// A bundled SystemParam grouping the catalog resources to stay within Bevy's 16-param limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SceneV2Params<'w> {
    pub scene_handle: Option<Res<'w, SceneHandleV2>>,
    pub scenes: Res<'w, Assets<GameSceneV2>>,
    pub config_handle: Res<'w, ProjectConfigHandle>,
    pub configs: Res<'w, Assets<ProjectConfig>>,
    pub asset_catalog: Res<'w, LoadedAssetCatalog>,
    pub prefab_catalog: Res<'w, LoadedPrefabCatalog>,
    pub project_root: Res<'w, ProjectRoot>,
    pub inventory_ui: ResMut<'w, crate::capabilities::inventory::LoadedInventoryUi>,
    pub container_ui: ResMut<'w, crate::capabilities::inventory::LoadedContainerUi>,
    pub loaded_item_catalog: Res<'w, crate::capabilities::inventory::LoadedItemCatalog>,
}

/// Bundles scene-load state resources to keep `action_executor_system` under Bevy's 16-param limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SceneStateParams<'w, 's> {
    pub load_mode: ResMut<'w, PendingSceneLoadMode>,
    pub preloaded: ResMut<'w, PreloadedScenes>,
    pub preloaded_glbs: ResMut<'w, PreloadedGlbHandles>,
    pub logic_state: ResMut<'w, LogicState>,
    pub game_vars: ResMut<'w, crate::GameVariables>,
    pub loaded_stats: ResMut<'w, crate::schema::stats::LoadedStats>,
    pub loaded_modifiers: Res<'w, crate::schema::stats::LoadedModifiers>,
    pub stat_map_query: Query<'w, 's, (&'static SpawnId, &'static mut crate::schema::stats::StatMap)>,
    pub global_transforms: Query<'w, 's, &'static GlobalTransform>,
    pub delayed_events: ResMut<'w, DelayedEventQueue>,
    pub project_config: Option<Res<'w, ProjectConfig>>,
    pub current_target: ResMut<'w, crate::capabilities::action_bar::CurrentTarget>,
    /// Lets `SetTarget` resolve a target's prefab key for the target UI variables.
    pub prefab_keys: Query<'w, 's, &'static PrefabKey>,
    pub audio_state: ResMut<'w, AudioState>,
    /// Used by `Action::ResetToSpawn` to read the NPC's stored spawn origin.
    pub npc_agents: Query<'w, 's, &'static crate::capabilities::npc::NpcAgent>,
    /// Used by `Action::ResetToSpawn` to teleport the entity's transform.
    pub transforms: Query<'w, 's, &'static mut Transform>,
    /// Used by `Action::ResetToSpawn` to zero residual velocity after teleport.
    pub npc_velocities: Query<'w, 's, &'static mut bevy_rapier3d::prelude::Velocity>,
    /// Used by `Action::CameraShake` to insert `CameraShakeState` on the active orbit camera.
    pub orbit_cameras: Query<'w, 's, Entity, With<crate::capabilities::camera::OrbitCamera>>,
    /// Cleared on `LoadScene` so stale dialogue state doesn't bleed across scene transitions.
    pub active_dialogue: ResMut<'w, crate::capabilities::dialogue::ActiveDialogue>,
    /// Player inventory — persists across scene loads.
    pub player_inventory: ResMut<'w, crate::capabilities::inventory::PlayerInventory>,
    /// Loaded item catalog for the current project.
    pub loaded_item_catalog: Res<'w, crate::capabilities::inventory::LoadedItemCatalog>,
    /// ECS entities for InventoryPanel and ShopPanel (set by scene loader).
    pub inventory_ui: ResMut<'w, crate::capabilities::inventory::LoadedInventoryUi>,
    /// ECS entity for ContainerPanel and active container (set by scene loader / OpenContainer).
    pub container_ui: ResMut<'w, crate::capabilities::inventory::LoadedContainerUi>,
    /// Entity-attached inventories (containers like chests).
    pub container_inventories: Query<'w, 's, (&'static SpawnId, &'static mut crate::capabilities::inventory::Inventory)>,
    /// Visibility query for the InventoryPanel node.
    /// `Without<ShopPanelMarker>` makes this disjoint from `shop_panel_q` so both can borrow `&mut Visibility`.
    pub inventory_panel_q: Query<'w, 's, (Entity, &'static mut Visibility), (With<crate::capabilities::inventory::InventoryPanelMarker>, Without<crate::capabilities::inventory::ShopPanelMarker>, Without<crate::capabilities::inventory::ContainerPanelMarker>)>,
    /// Visibility query for the ShopPanel node. Also reads `ShopPanelMarker` for `font_size`.
    /// `Without<InventoryPanelMarker>` makes this disjoint from `inventory_panel_q`.
    pub shop_panel_q: Query<'w, 's, (Entity, &'static mut Visibility, &'static crate::capabilities::inventory::ShopPanelMarker), (With<crate::capabilities::inventory::ShopPanelMarker>, Without<crate::capabilities::inventory::InventoryPanelMarker>, Without<crate::capabilities::inventory::ContainerPanelMarker>)>,
    /// Used by `OpenShop` to find the entries-area child so only entries are despawned on re-open,
    /// preserving the header + close button.
    pub shop_entries_q: Query<'w, 's, (Entity, &'static ChildOf), With<crate::capabilities::inventory::ShopEntriesContainerMarker>>,
    /// Visibility query for the ContainerPanel node.
    pub container_panel_q: Query<'w, 's, (Entity, &'static mut Visibility), (With<crate::capabilities::inventory::ContainerPanelMarker>, Without<crate::capabilities::inventory::InventoryPanelMarker>, Without<crate::capabilities::inventory::ShopPanelMarker>)>,
}

/// Bundles material-related assets to keep `spawn_scene_v2` under Bevy's 16-param limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SceneMaterialParams<'w> {
    pub images: ResMut<'w, Assets<Image>>,
    pub standard: ResMut<'w, Assets<StandardMaterial>>,
    pub terrain: ResMut<'w, Assets<TerrainMaterial>>,
    pub custom: ResMut<'w, Assets<crate::capabilities::custom_material::CustomMaterial>>,
    pub built: ResMut<'w, BuiltMaterials>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub radar: ResMut<'w, Assets<crate::capabilities::stat_radar::RadarMaterial>>,
    pub color_materials: Option<ResMut<'w, Assets<ColorMaterial>>>,
    pub atlas_layouts: Option<ResMut<'w, Assets<TextureAtlasLayout>>>,
}

// ─── Utilities ────────────────────────────────────────────────────────────────

/// Resolves a project-relative path against the project root directory.
/// If `project_root` is empty the path is returned as-is.
pub fn resolve_project_path(project_root: &str, path: &str) -> String {
    if project_root.is_empty() {
        return path.to_string();
    }
    format!("{}/{}", project_root, path)
}

/// Generates a minimal 1×1 cubemap `Image` from top/bottom gradient colours.
/// Used as a fallback environment map when no `.ktx2` assets are provided.
pub(crate) fn generate_cubemap(config: &crate::schema::GeneratedEnvironmentMapLight) -> Image {
    let top = [
        (config.top_color.0 * 255.0) as u8,
        (config.top_color.1 * 255.0) as u8,
        (config.top_color.2 * 255.0) as u8,
        255,
    ];
    let bottom = [
        (config.bottom_color.0 * 255.0) as u8,
        (config.bottom_color.1 * 255.0) as u8,
        (config.bottom_color.2 * 255.0) as u8,
        255,
    ];
    let mid = [
        ((config.top_color.0 + config.bottom_color.0) * 0.5 * 255.0) as u8,
        ((config.top_color.1 + config.bottom_color.1) * 0.5 * 255.0) as u8,
        ((config.top_color.2 + config.bottom_color.2) * 0.5 * 255.0) as u8,
        255,
    ];

    let mut data = Vec::new();
    for i in 0..6 {
        let color = match i {
            2 => top,
            3 => bottom,
            _ => mid,
        };
        data.extend_from_slice(&color);
    }

    let mut image = Image::new(
        Extent3d { width: 1, height: 1, depth_or_array_layers: 6 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );

    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });

    image
}
