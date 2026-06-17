use bevy::prelude::*;
use crate::runtime::scene_manager::{LevelEntity, LoadedTargetIndicator, SpawnRegistry};
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

/// Spawns, moves, and despawns the target-indicator ground ring.
///
/// Mesh and material are built once per scene (cached in `Local`) and reused for every
/// target change — avoids per-switch asset allocation and WebGPU pipeline recompiles.
///
/// - `LoadedTargetIndicator.is_changed()` triggers a rebuild of the cached assets.
/// - `CurrentTarget.is_changed()` triggers spawn / despawn.
/// - Every frame: moves the live indicator to match its tracked entity's XZ position;
///   despawns it if the tracked entity is gone.
pub fn target_indicator_system(
    mut commands: Commands,
    current_target: Res<CurrentTarget>,
    indicator_cfg: Res<LoadedTargetIndicator>,
    registry: Res<SpawnRegistry>,
    global_transforms: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    existing: Query<(Entity, &TrackingTarget)>,
    mut transforms: Query<&mut Transform, With<TrackingTarget>>,
    // Cached mesh + material built once per scene. Rebuilt whenever LoadedTargetIndicator changes.
    mut cached: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    // Rebuild cached assets whenever the scene config changes.
    if indicator_cfg.is_changed() {
        *cached = indicator_cfg.0.as_ref().map(|cfg| {
            let (r, g, b, a) = cfg.color;
            let texture: Handle<Image> = asset_server.load(cfg.texture_path.clone());
            let mat = materials.add(StandardMaterial {
                base_color_texture: Some(texture),
                base_color: Color::srgba(r, g, b, a),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                depth_bias: 64.0,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            let d = cfg.radius * 2.0;
            let mesh = meshes.add(Plane3d::default().mesh().size(d, d));
            (mesh, mat)
        });
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

    let Some((mesh_handle, mat_handle)) = cached.as_ref() else {
        return;
    };

    let Some(&target_entity) = registry.entities.get(target_id) else {
        warn!("target_indicator: target '{}' not in spawn registry", target_id);
        return;
    };

    let Ok(gt) = global_transforms.get(target_entity) else {
        return;
    };

    let p = gt.translation();

    commands.spawn((
        Name::new(format!("TargetIndicator:{}", target_id)),
        Mesh3d(mesh_handle.clone()),
        MeshMaterial3d(mat_handle.clone()),
        Transform::from_translation(Vec3::new(p.x, cfg.offset_y, p.z)),
        TrackingTarget(target_entity),
        LevelEntity,
    ));
}
