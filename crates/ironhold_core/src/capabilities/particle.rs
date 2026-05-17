use bevy::prelude::*;
use crate::schema::catalog::EffectDef;
use crate::runtime::scene_manager::{LevelEntity, LoadedAssetCatalog};

// ─── Types ────────────────────────────────────────────────────────────────────

/// Per-particle runtime state. Carried alongside `Mesh3d` + `MeshMaterial3d<StandardMaterial>`.
/// `particle_system` ticks velocity, size lerp, colour lerp, billboard rotation, and despawn.
#[derive(Component)]
pub struct Particle {
    pub velocity: Vec3,
    pub elapsed: f32,
    pub duration: f32,
    pub start_size: f32,
    pub end_size: Option<f32>,
    pub gravity: f32,
    /// Per-frame lateral noise magnitude (m/s²). 0.0 = straight-line trajectories.
    pub turbulence: f32,
    /// Unique noise phase per particle — set from index at spawn so particles in the
    /// same burst have independent turbulence paths.
    pub noise_seed: f32,
    pub color_start: LinearRgba,
    /// Optional midpoint for a three-stop colour gradient (start → mid → end).
    pub color_mid: Option<LinearRgba>,
    pub color_end: LinearRgba,
    /// Unique material handle per particle — updated each frame for colour animation.
    pub mat_handle: Handle<StandardMaterial>,
    /// When `true`, `particle_system` rotates this entity each frame to face the active `Camera3d`.
    /// Set automatically for effects that have a `sprite` key in their `EffectDef`.
    pub is_billboard: bool,
}

/// Shared mesh handles created once at startup, reused across all particle entities.
/// Particles are sized via `Transform::scale` so only one mesh asset is needed per shape.
#[derive(Resource, Default)]
pub struct ParticleMeshCache {
    /// Unit sphere (`radius: 1.0`) — used by effects with no `sprite` key.
    pub sphere: Option<Handle<Mesh>>,
    /// Unit quad (`1.0 × 1.0` in the XY plane, face normal in +Z) — used by sprite billboard effects.
    pub quad: Option<Handle<Mesh>>,
}

/// Resolved effect waiting to be spawned. Pushed by `action_executor_system`, drained by
/// `drain_particle_effects_system`.
pub struct QueuedParticleEffect {
    pub origin: Vec3,
    pub def: EffectDef,
}

/// FIFO queue of particle effects to spawn. Populated by the action executor;
/// drained each frame by `drain_particle_effects_system`.
#[derive(Resource, Default)]
pub struct PendingParticleEffects(pub Vec<QueuedParticleEffect>);

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Returns a deterministic unit direction for particle index `i` out of `count` within
/// a cone of half-angle `half_angle_rad` around +Y (0 = straight up, PI = full sphere).
/// Uses a spherical-cap golden-angle spiral — no random state required.
fn fibonacci_cone_dir(i: u32, count: u32, half_angle_rad: f32) -> Vec3 {
    if count == 1 {
        return Vec3::Y;
    }
    let t = i as f32 / (count as f32 - 1.0);
    let cos_max = half_angle_rad.cos().clamp(-1.0, 1.0);
    let cos_theta = 1.0 - t * (1.0 - cos_max);
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    const GOLDEN_ANGLE: f32 = 2.399_963; // 2π * (1 − 1/φ)
    let phi = GOLDEN_ANGLE * i as f32;
    Vec3::new(phi.cos() * sin_theta, cos_theta, phi.sin() * sin_theta)
}

/// Returns a deterministic offset in `[-jitter, +jitter]` for particle index `i`.
/// `seed_xor` is XORed into `i` before hashing so different properties (speed, size)
/// receive independent jitter streams from the same index without storing extra state.
fn hash_jitter(i: u32, seed_xor: u32, jitter: f32) -> f32 {
    if jitter <= 0.0 { return 0.0; }
    let mut h = (i ^ seed_xor).wrapping_mul(2_654_435_761);
    h ^= h >> 16;
    let t = (h & 0xFFFF) as f32 / 65535.0;
    (t * 2.0 - 1.0) * jitter
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Creates the shared mesh handles at startup and stores them in `ParticleMeshCache`.
pub fn particle_startup_system(
    mut cache: ResMut<ParticleMeshCache>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    cache.sphere = Some(meshes.add(Sphere { radius: 1.0 }));
    cache.quad = Some(meshes.add(Rectangle::new(1.0, 1.0)));
}

/// Drains `PendingParticleEffects` and spawns the actual mesh entities.
/// Sprite effects (`EffectDef.sprite` is `Some`) spawn billboard quads with the resolved
/// texture; plain effects spawn coloured sphere meshes (existing behaviour).
/// Runs in the interpreter chain after `action_executor_system`.
pub fn drain_particle_effects_system(
    mut pending: ResMut<PendingParticleEffects>,
    mut commands: Commands,
    cache: Res<ParticleMeshCache>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    asset_catalog: Res<LoadedAssetCatalog>,
) {
    let Some(sphere) = cache.sphere.clone() else { return };
    let Some(quad) = cache.quad.clone() else { return };

    for effect in pending.0.drain(..) {
        let def = &effect.def;
        let half_angle = def.spread_deg.clamp(0.0, 180.0).to_radians();

        let color_start = Color::srgba(
            def.color_start.0, def.color_start.1, def.color_start.2, def.color_start.3,
        ).to_linear();
        let color_mid = def.color_mid.map(|cm| {
            Color::srgba(cm.0, cm.1, cm.2, cm.3).to_linear()
        });
        let color_end = Color::srgba(
            def.color_end.0, def.color_end.1, def.color_end.2, def.color_end.3,
        ).to_linear();

        // Resolve sprite texture once per effect burst (shared across all particles in this burst).
        let sprite_texture: Option<Handle<Image>> = def.sprite.as_ref().and_then(|key| {
            match asset_catalog.0.textures.get(key) {
                Some(path) => Some(asset_server.load(path.clone())),
                None => {
                    warn!("SpawnEffect: sprite key {:?} not in catalog textures", key);
                    None
                }
            }
        });
        let is_sprite = sprite_texture.is_some();

        for i in 0..def.particle_count {
            let dir = fibonacci_cone_dir(i, def.particle_count, half_angle);
            // Independent hash streams: 0x00000000 for speed, 0x9E3779B9 for size.
            let speed = def.speed + hash_jitter(i, 0x0000_0000, def.speed_jitter);
            let velocity = dir * speed;

            let actual_size = (def.size + hash_jitter(i, 0x9E37_79B9, def.size_jitter))
                .max(0.001);

            // Scatter spawn position across a horizontal disc (volume emission).
            let spawn_pos = if def.emit_radius > 0.0 {
                let angle = 2.399_963_f32 * i as f32;
                let r = def.emit_radius * ((i as f32 + 0.5) / def.particle_count as f32).sqrt();
                effect.origin + Vec3::new(r * angle.cos(), 0.0, r * angle.sin())
            } else {
                effect.origin
            };

            // Unique turbulence phase per particle: golden-angle fraction → [0, 2π].
            let noise_seed = (i as f32 * 2.399_963_f32).fract() * std::f32::consts::TAU;

            let (mat, mesh) = if is_sprite {
                let alpha_mode = if def.additive { AlphaMode::Add } else { AlphaMode::Blend };
                let m = materials.add(StandardMaterial {
                    base_color: Color::from(color_start),
                    base_color_texture: sprite_texture.clone(),
                    unlit: true,
                    alpha_mode,
                    double_sided: true,
                    ..default()
                });
                (m, quad.clone())
            } else {
                let m = materials.add(StandardMaterial {
                    base_color: Color::from(color_start),
                    unlit: true,
                    alpha_mode: AlphaMode::Add,
                    ..default()
                });
                (m, sphere.clone())
            };

            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(spawn_pos).with_scale(Vec3::splat(actual_size)),
                Visibility::default(),
                LevelEntity,
                Particle {
                    velocity,
                    elapsed: 0.0,
                    duration: def.lifetime_secs,
                    start_size: actual_size,
                    end_size: def.size_end,
                    gravity: def.gravity,
                    turbulence: def.turbulence,
                    noise_seed,
                    color_start,
                    color_mid,
                    color_end,
                    mat_handle: mat,
                    is_billboard: is_sprite,
                },
            ));
        }
    }
}

/// Ticks all live particles: gravity, turbulence, velocity integration, size lerp,
/// colour lerp (two-stop or three-stop), billboard orientation, and despawn on lifetime expiry.
/// Change-detection guards prevent redundant render updates.
pub fn particle_system(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut Particle, &mut Transform)>,
    cam_query: Query<&GlobalTransform, With<Camera3d>>,
) {
    let cam_pos = cam_query.single().map(|gt| gt.translation()).ok();

    let dt = time.delta_secs();
    for (entity, mut particle, mut transform) in query.iter_mut() {
        particle.elapsed += dt;
        let t = (particle.elapsed / particle.duration).min(1.0);

        // Gravity: Y-axis acceleration.
        particle.velocity.y += particle.gravity * dt;

        // Turbulence: two-frequency sine-sum noise displaces XZ velocity each frame,
        // creating billowing and swirling instead of straight-line trajectories.
        if particle.turbulence > 0.0 {
            let s = particle.noise_seed;
            let te = particle.elapsed;
            let dx = ((te * 3.1 + s).sin() + (te * 1.7 + s * 2.3).sin() * 0.5)
                * particle.turbulence;
            let dz = ((te * 2.7 + s * 1.5).cos() + (te * 4.1 + s * 0.8).cos() * 0.5)
                * particle.turbulence;
            particle.velocity.x += dx * dt;
            particle.velocity.z += dz * dt;
        }

        let displacement = particle.velocity * dt;
        if displacement.length_squared() > 0.0 {
            transform.translation += displacement;
        }

        // Size lerp.
        if let Some(end_size) = particle.end_size {
            let new_size = particle.start_size + (end_size - particle.start_size) * t;
            if (transform.scale.x - new_size).abs() > 0.001 {
                transform.scale = Vec3::splat(new_size);
            }
        }

        // Colour lerp — two-stop (linear) or three-stop (with midpoint).
        let cs = particle.color_start;
        let ce = particle.color_end;
        let new_color = if let Some(cm) = particle.color_mid {
            if t < 0.5 {
                let t2 = t * 2.0;
                LinearRgba {
                    red:   cs.red   + (cm.red   - cs.red)   * t2,
                    green: cs.green + (cm.green - cs.green) * t2,
                    blue:  cs.blue  + (cm.blue  - cs.blue)  * t2,
                    alpha: cs.alpha + (cm.alpha - cs.alpha) * t2,
                }
            } else {
                let t2 = (t - 0.5) * 2.0;
                LinearRgba {
                    red:   cm.red   + (ce.red   - cm.red)   * t2,
                    green: cm.green + (ce.green - cm.green) * t2,
                    blue:  cm.blue  + (ce.blue  - cm.blue)  * t2,
                    alpha: cm.alpha + (ce.alpha - cm.alpha) * t2,
                }
            }
        } else {
            LinearRgba {
                red:   cs.red   + (ce.red   - cs.red)   * t,
                green: cs.green + (ce.green - cs.green) * t,
                blue:  cs.blue  + (ce.blue  - cs.blue)  * t,
                alpha: cs.alpha + (ce.alpha - cs.alpha) * t,
            }
        };

        if let Some(mat) = materials.get_mut(&particle.mat_handle) {
            let cur = mat.base_color.to_linear();
            if (cur.red   - new_color.red).abs()   > 0.01
            || (cur.green - new_color.green).abs() > 0.01
            || (cur.blue  - new_color.blue).abs()  > 0.01
            || (cur.alpha - new_color.alpha).abs() > 0.01
            {
                mat.base_color = Color::from(new_color);
            }
        }

        // Billboard: rotate the quad to face the active camera each frame.
        // Uses from_rotation_arc so the +Z face (where the texture shows) points at the camera.
        if particle.is_billboard {
            if let Some(cp) = cam_pos {
                let diff = cp - transform.translation;
                if diff.length_squared() > 1e-6 {
                    transform.rotation = Quat::from_rotation_arc(Vec3::Z, diff.normalize());
                }
            }
        }

        if particle.elapsed >= particle.duration {
            commands.entity(entity).despawn();
        }
    }
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticleMeshCache>()
            .init_resource::<PendingParticleEffects>()
            .add_systems(Startup, particle_startup_system)
            .add_systems(Update, particle_system);
    }
}
