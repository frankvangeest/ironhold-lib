use std::collections::HashMap;
use bevy::prelude::*;
use crate::runtime::scene_manager::{LevelEntity, LoadedPrefabCatalog, LoadedTargetIndicator, PrefabKey, ResolvedTargetIndicator, SpawnRegistry};
use crate::capabilities::action_bar::CurrentTarget;

/// Marks the active target-indicator entity and records which world entity it tracks.
#[derive(Component)]
pub struct TrackingTarget(pub Entity);

pub struct TargetIndicatorPlugin;

impl Plugin for TargetIndicatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, target_indicator_system);
    }
}

/// Resolves the indicator ring colour for `target_entity` using the three-tier precedence:
/// 1. Prefab `indicator_color` (direct RGBA override, highest priority)
/// 2. Prefab `indicator_category` looked up in `cfg.named_colors`
/// 3. Scene-level `cfg.color` fallback
fn resolve_indicator_color(
    target_entity: Entity,
    prefab_keys: &Query<&PrefabKey>,
    catalog: &LoadedPrefabCatalog,
    cfg: &ResolvedTargetIndicator,
) -> (f32, f32, f32, f32) {
    let Ok(PrefabKey(key)) = prefab_keys.get(target_entity) else {
        return cfg.color;
    };
    let Some(prefab) = catalog.0.prefabs.get(key) else {
        return cfg.color;
    };
    if let Some(c) = prefab.indicator_color {
        return c;
    }
    if let Some(cat) = prefab.indicator_category.as_deref() {
        if let Some(c) = cfg.named_colors.get(cat) {
            return *c;
        }
    }
    cfg.color
}

/// Spawns, moves, and despawns the target-indicator ground ring.
///
/// Mesh is built once per scene (radius-independent of colour). Materials are memoised
/// per resolved RGBA colour — only a new colour seen for the first time in a scene causes
/// a new `StandardMaterial` allocation. Both caches are cleared when the scene changes.
///
/// - `LoadedTargetIndicator.is_changed()` clears and rebuilds the mesh cache.
/// - `CurrentTarget.is_changed()` resolves the colour and spawns / despawns the indicator.
/// - Every frame: moves the live indicator to match its tracked entity's XZ position;
///   despawns it if the tracked entity is gone.
pub fn target_indicator_system(
    mut commands: Commands,
    current_target: Res<CurrentTarget>,
    indicator_cfg: Res<LoadedTargetIndicator>,
    registry: Res<SpawnRegistry>,
    prefab_catalog: Res<LoadedPrefabCatalog>,
    prefab_keys: Query<&PrefabKey>,
    global_transforms: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    existing: Query<(Entity, &TrackingTarget)>,
    mut transforms: Query<&mut Transform, With<TrackingTarget>>,
    // Cached mesh handle (radius-driven, colour-independent). None when no indicator is configured.
    mut cached_mesh: Local<Option<Handle<Mesh>>>,
    // Memoised material handles keyed by resolved RGBA colour bits. Cleared on scene change.
    mut cached_mats: Local<HashMap<[u32; 4], Handle<StandardMaterial>>>,
) {
    // Rebuild mesh cache whenever the scene config changes; clear the material memo too.
    if indicator_cfg.is_changed() {
        *cached_mesh = indicator_cfg.0.as_ref().map(|cfg| {
            let d = cfg.radius * 2.0;
            meshes.add(Plane3d::default().mesh().size(d, d))
        });
        cached_mats.clear();
    }

    // Move any existing indicator to follow its tracked entity.
    for (indicator_entity, TrackingTarget(tracked)) in &existing {
        match global_transforms.get(*tracked) {
            Ok(gt) => {
                let p = gt.translation();
                if let Ok(mut tf) = transforms.get_mut(indicator_entity) {
                    if (tf.translation.x - p.x).abs() > 0.001
                        || (tf.translation.z - p.z).abs() > 0.001
                    {
                        tf.translation.x = p.x;
                        tf.translation.z = p.z;
                    }
                }
            }
            Err(_) => {
                commands.entity(indicator_entity).despawn();
            }
        }
    }

    if !current_target.is_changed() {
        return;
    }

    // Despawn all existing indicators whenever target changes.
    for (indicator_entity, _) in &existing {
        commands.entity(indicator_entity).despawn();
    }

    let Some(target_id) = current_target.0.as_ref() else {
        return;
    };

    let Some(cfg) = &indicator_cfg.0 else {
        return;
    };

    let Some(mesh_handle) = cached_mesh.as_ref() else {
        return;
    };

    let Some(&target_entity) = registry.entities.get(target_id) else {
        warn!("target_indicator: target '{}' not in spawn registry", target_id);
        return;
    };

    let Ok(gt) = global_transforms.get(target_entity) else {
        return;
    };

    // Resolve colour via three-tier precedence, then retrieve or create a memoised material.
    let rgba = resolve_indicator_color(target_entity, &prefab_keys, &prefab_catalog, cfg);
    let color_key = [rgba.0.to_bits(), rgba.1.to_bits(), rgba.2.to_bits(), rgba.3.to_bits()];
    let mat_handle = cached_mats.entry(color_key).or_insert_with(|| {
        let texture: Handle<Image> = asset_server.load(cfg.texture_path.clone());
        materials.add(StandardMaterial {
            base_color_texture: Some(texture),
            base_color: Color::srgba(rgba.0, rgba.1, rgba.2, rgba.3),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            depth_bias: 64.0,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    }).clone();

    let p = gt.translation();
    commands.spawn((
        Name::new(format!("TargetIndicator:{}", target_id)),
        Mesh3d(mesh_handle.clone()),
        MeshMaterial3d(mat_handle),
        Transform::from_translation(Vec3::new(p.x, cfg.offset_y, p.z)),
        TrackingTarget(target_entity),
        LevelEntity,
    ));
}
