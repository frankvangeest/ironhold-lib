use bevy::prelude::*;
use crate::schema::catalog::EffectDef;
use crate::runtime::scene_manager::LevelEntity;

// ─── Types ────────────────────────────────────────────────────────────────────

/// Per-particle runtime state. Carried alongside `Mesh3d` + `MeshMaterial3d<StandardMaterial>`.
/// `particle_system` ticks velocity, size lerp, color lerp, and despawn.
#[derive(Component)]
pub struct Particle {
    pub velocity: Vec3,
    pub elapsed: f32,
    pub duration: f32,
    pub start_size: f32,
    pub end_size: Option<f32>,
    pub gravity: f32,
    pub color_start: LinearRgba,
    pub color_end: LinearRgba,
    /// Unique material handle per particle — updated each frame for color animation.
    pub mat_handle: Handle<StandardMaterial>,
}

/// Shared sphere mesh created once at startup, reused across all particle entities.
/// Particles are sized via `Transform::scale` so only one mesh asset is needed.
#[derive(Resource, Default)]
pub struct ParticleMeshCache {
    pub sphere: Option<Handle<Mesh>>,
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

// ─── Direction helpers ────────────────────────────────────────────────────────

/// Returns a deterministic unit direction for particle index `i` out of `count` within
/// a cone of half-angle `half_angle_rad` around +Y (0 = straight up, PI = full sphere).
/// Uses a spherical-cap golden-angle spiral — no random state required.
fn fibonacci_cone_dir(i: u32, count: u32, half_angle_rad: f32) -> Vec3 {
    if count == 1 {
        return Vec3::Y;
    }
    // t runs 0 → 1 uniformly across all particles.
    let t = i as f32 / (count as f32 - 1.0);
    // cos_theta spans from 1 (straight up) down to cos(half_angle_rad) (edge of cone).
    let cos_max = half_angle_rad.cos().clamp(-1.0, 1.0);
    let cos_theta = 1.0 - t * (1.0 - cos_max);
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    // Golden angle in radians — gives good spiral coverage on the cap.
    const GOLDEN_ANGLE: f32 = 2.399_963; // 2π * (1 − 1/φ)
    let phi = GOLDEN_ANGLE * i as f32;
    Vec3::new(phi.cos() * sin_theta, cos_theta, phi.sin() * sin_theta)
}

/// Returns a deterministic speed offset in `[-jitter, +jitter]` for particle index `i`.
fn hash_jitter(i: u32, jitter: f32) -> f32 {
    if jitter <= 0.0 { return 0.0; }
    // Multiplicative xorshift hash → uniform [0, 1] → remap to [-1, 1].
    let mut h = i.wrapping_mul(2_654_435_761);
    h ^= h >> 16;
    let t = (h & 0xFFFF) as f32 / 65535.0;
    (t * 2.0 - 1.0) * jitter
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Creates the shared sphere mesh at startup and stores it in `ParticleMeshCache`.
pub fn particle_startup_system(
    mut cache: ResMut<ParticleMeshCache>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    cache.sphere = Some(meshes.add(Sphere { radius: 1.0 }));
}

/// Drains `PendingParticleEffects` and spawns the actual mesh entities.
/// Runs in the interpreter chain after `action_executor_system`.
pub fn drain_particle_effects_system(
    mut pending: ResMut<PendingParticleEffects>,
    mut commands: Commands,
    cache: Res<ParticleMeshCache>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(sphere) = cache.sphere.clone() else { return };

    for effect in pending.0.drain(..) {
        let def = &effect.def;
        let half_angle = def.spread_deg.clamp(0.0, 180.0).to_radians();
        let color_start = Color::srgba(
            def.color_start.0, def.color_start.1, def.color_start.2, def.color_start.3,
        ).to_linear();
        let color_end = Color::srgba(
            def.color_end.0, def.color_end.1, def.color_end.2, def.color_end.3,
        ).to_linear();

        for i in 0..def.particle_count {
            let dir = fibonacci_cone_dir(i, def.particle_count, half_angle);
            let speed = def.speed + hash_jitter(i, def.speed_jitter);
            let velocity = dir * speed;

            let mat = materials.add(StandardMaterial {
                base_color: Color::from(color_start),
                unlit: true,
                alpha_mode: AlphaMode::Add,
                ..default()
            });

            commands.spawn((
                Mesh3d(sphere.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(effect.origin).with_scale(Vec3::splat(def.size)),
                Visibility::default(),
                LevelEntity,
                Particle {
                    velocity,
                    elapsed: 0.0,
                    duration: def.lifetime_secs,
                    start_size: def.size,
                    end_size: def.size_end,
                    gravity: def.gravity,
                    color_start,
                    color_end,
                    mat_handle: mat,
                },
            ));
        }
    }
}

/// Ticks all live particles: applies gravity, integrates velocity, lerps size and color,
/// despawns when `lifetime_secs` has elapsed.
/// Change-detection guards prevent redundant render updates.
pub fn particle_system(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut particle, mut transform) in query.iter_mut() {
        particle.elapsed += dt;
        let t = (particle.elapsed / particle.duration).min(1.0);

        // Integrate gravity then velocity.
        particle.velocity.y += particle.gravity * dt;
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

        // Color lerp — only write when the change is visible.
        let cs = particle.color_start;
        let ce = particle.color_end;
        let new_color = LinearRgba {
            red:   cs.red   + (ce.red   - cs.red)   * t,
            green: cs.green + (ce.green - cs.green) * t,
            blue:  cs.blue  + (ce.blue  - cs.blue)  * t,
            alpha: cs.alpha + (ce.alpha - cs.alpha) * t,
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
