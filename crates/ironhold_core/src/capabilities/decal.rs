use bevy::prelude::*;
use crate::runtime::scene_manager::LevelEntity;

// ─── Components ───────────────────────────────────────────────────────────────

/// Drives a flat ground-plane quad that fades out and despawns after `duration_secs`.
/// Optionally pulses its opacity at `pulse_speed` cycles per second.
#[derive(Component)]
pub struct FadingDecal {
    pub duration_secs: f32,
    pub elapsed: f32,
    pub pulse_speed: f32,
    /// Base RGBA stored so the pulse formula can rebuild the tint each frame.
    pub base_color: LinearRgba,
}

/// When present, the decal follows the XZ position of this target entity each frame.
/// Useful for character aura effects and persistent debuff circles under moving enemies.
#[derive(Component)]
pub struct TrackedDecal(pub Entity);

// ─── Pending queue ────────────────────────────────────────────────────────────

pub struct QueuedDecal {
    pub texture_path: String,
    pub world_pos: Vec3,
    pub radius: f32,
    pub duration_secs: f32,
    pub color: (f32, f32, f32, f32),
    pub pulse_speed: f32,
    pub track_entity: Option<Entity>,
}

/// FIFO queue populated by `action_executor_system`; drained each frame by `spawn_decal_system`.
#[derive(Resource, Default)]
pub struct PendingDecalSpawns(pub Vec<QueuedDecal>);

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Drains `PendingDecalSpawns` and spawns one flat quad entity per queued decal.
///
/// Each spawned entity gets:
/// - `Mesh3d` — a 1×1 XZ-plane quad scaled by `radius * 2` in X and Z.
/// - `MeshMaterial3d<StandardMaterial>` — unlit, AlphaBlend, `depth_bias: 128`.
/// - `FadingDecal` — lifetime + pulse state.
/// - `LevelEntity` — cleaned up on scene transition.
/// - `TrackedDecal` — if the decal should follow an entity in XZ (optional).
pub fn spawn_decal_system(
    mut commands: Commands,
    mut pending: ResMut<PendingDecalSpawns>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    for queued in pending.0.drain(..) {
        let (r, g, b, a) = queued.color;
        let texture: Handle<Image> = asset_server.load(queued.texture_path.clone());
        let mat = materials.add(StandardMaterial {
            base_color_texture: Some(texture),
            base_color: Color::srgba(r, g, b, a),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            depth_bias: 128.0,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        let mesh = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));
        let d = queued.radius * 2.0;
        let transform = Transform {
            translation: Vec3::new(queued.world_pos.x, 0.02, queued.world_pos.z),
            scale: Vec3::new(d, 1.0, d),
            ..default()
        };
        let fading = FadingDecal {
            duration_secs: queued.duration_secs,
            elapsed: 0.0,
            pulse_speed: queued.pulse_speed,
            base_color: Color::srgba(r, g, b, a).to_linear(),
        };
        let mut cmd = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            transform,
            fading,
            LevelEntity,
        ));
        if let Some(tracked) = queued.track_entity {
            cmd.insert(TrackedDecal(tracked));
        }
    }
}

/// Ticks `FadingDecal` lifetime, applies pulse opacity, updates the material colour,
/// optionally follows a `TrackedDecal` entity in XZ, and despawns at end of lifetime.
pub fn fading_decal_system(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(
        Entity,
        &mut FadingDecal,
        &MeshMaterial3d<StandardMaterial>,
        &mut Transform,
        Option<&TrackedDecal>,
    )>,
    target_transforms: Query<&GlobalTransform, Without<FadingDecal>>,
) {
    let dt = time.delta_secs();
    for (entity, mut decal, mat_handle, mut transform, tracked) in &mut query {
        decal.elapsed += dt;

        // Update XZ position to follow tracked entity.
        if let Some(TrackedDecal(target)) = tracked {
            if let Ok(gt) = target_transforms.get(*target) {
                let p = gt.translation();
                transform.translation.x = p.x;
                transform.translation.z = p.z;
            }
        }

        let base = decal.base_color;
        let alpha = if decal.pulse_speed > 0.0 {
            let pulse = 0.7 + 0.3 * (decal.elapsed * decal.pulse_speed * std::f32::consts::TAU).sin();
            base.alpha * pulse
        } else {
            base.alpha
        };

        // Fade out over the last 20 % of the lifetime.
        let fade_start = decal.duration_secs * 0.8;
        let final_alpha = if decal.elapsed > fade_start {
            let t = (decal.elapsed - fade_start) / (decal.duration_secs - fade_start);
            alpha * (1.0 - t).max(0.0)
        } else {
            alpha
        };

        if let Some(mat) = materials.get_mut(mat_handle.id()) {
            mat.base_color = Color::linear_rgba(base.red, base.green, base.blue, final_alpha);
        }

        if decal.elapsed >= decal.duration_secs {
            commands.entity(entity).despawn();
        }
    }
}
