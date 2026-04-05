use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDimension, TextureViewDescriptor,
};
use std::collections::HashMap;

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
pub mod v1_compat;

pub use project_loader::*;
pub use scene_loader::*;
pub use entity_spawner::*;
pub use message_interpreter::*;
pub use action_executor::*;
pub use v1_compat::*;

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

/// Global key bindings loaded from the project config.
/// Maps key name strings (e.g. "Escape") to event trigger names (e.g. "toggle_pause").
#[derive(Resource, Default, Clone)]
pub struct LoadedKeyBindings(pub HashMap<String, String>);

/// Holds pre-loaded scene asset handles so they stay cached and are ready instantly when needed.
/// Populated by `Action::Preload`. Cleared on a full `LoadScene` so stale handles don't linger.
#[derive(Resource, Default)]
pub struct PreloadedScenes(pub Vec<Handle<GameSceneV2>>);

#[derive(Resource)]
pub struct SceneHandleV2(pub Handle<GameSceneV2>);

#[derive(Resource, Default, Clone)]
pub struct LoadedAssetCatalog(pub AssetCatalog);

#[derive(Resource, Default, Clone)]
pub struct LoadedPrefabCatalog(pub PrefabCatalog);

/// Named spawn points from the most recently loaded scene. Available to `Action::Spawn`
/// so dynamically spawned entities can use scene-defined positions.
#[derive(Resource, Default, Clone)]
pub struct LoadedSpawnPoints(pub HashMap<String, (f32, f32, f32)>);

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
}

/// Tracks all dynamically spawned entities by their spawn ID.
/// Cleared automatically on scene load so stale IDs don't linger.
#[derive(Resource, Default)]
pub struct SpawnRegistry {
    pub counter: u64,
    pub entities: HashMap<String, Entity>,
}

// ─── Components ───────────────────────────────────────────────────────────────

/// Stable handle attached to every entity spawned via `Action::Spawn`.
/// Used by `Action::Despawn` to locate and remove the entity.
#[derive(Component, Debug, Clone)]
pub struct SpawnId(pub String);

#[derive(Component)]
pub struct PendingPlayerConfig(pub PlayerConfig);

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
}

/// Bundles scene-load state resources to keep `action_executor_system` under Bevy's 16-param limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SceneStateParams<'w> {
    pub load_mode: ResMut<'w, PendingSceneLoadMode>,
    pub preloaded: ResMut<'w, PreloadedScenes>,
    pub logic_state: ResMut<'w, LogicState>,
}

/// Bundles material-related assets to keep `spawn_scene_v2` under Bevy's 16-param limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SceneMaterialParams<'w> {
    pub images: ResMut<'w, Assets<Image>>,
    pub standard: ResMut<'w, Assets<StandardMaterial>>,
    pub terrain: ResMut<'w, Assets<TerrainMaterial>>,
    pub built: ResMut<'w, BuiltMaterials>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
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
pub(crate) fn generate_cubemap(config: &crate::schema::level::GeneratedEnvironmentMapLight) -> Image {
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
