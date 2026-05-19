use bevy::prelude::*;
use crate::schema::catalog::EffectDef;
use crate::runtime::scene_manager::LoadedAssetCatalog;
use crate::capabilities::particle_renderer::{ParticlePool, PooledParticle};

// ─── Public queue types (unchanged API) ───────────────────────────────────────

/// Resolved effect waiting to be drained into the particle pool.
pub struct QueuedParticleEffect {
    pub origin: Vec3,
    pub def: EffectDef,
}

/// FIFO queue populated by the action executor; drained each frame by
/// `drain_particle_effects_system`.
#[derive(Resource, Default)]
pub struct PendingParticleEffects(pub Vec<QueuedParticleEffect>);

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Deterministic unit direction for particle index `i` of `count` within a cone
/// of half-angle `half_angle_rad` around +Y.  Golden-angle spiral, no RNG.
pub fn fibonacci_cone_dir(i: u32, count: u32, half_angle_rad: f32) -> Vec3 {
    if count == 1 { return Vec3::Y; }
    let t = i as f32 / (count as f32 - 1.0);
    let cos_max = half_angle_rad.cos().clamp(-1.0, 1.0);
    let cos_theta = 1.0 - t * (1.0 - cos_max);
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    const GOLDEN_ANGLE: f32 = 2.399_963;
    let phi = GOLDEN_ANGLE * i as f32;
    Vec3::new(phi.cos() * sin_theta, cos_theta, phi.sin() * sin_theta)
}

/// Deterministic jitter in `[-jitter, +jitter]` for particle index `i`.
pub fn hash_jitter(i: u32, seed_xor: u32, jitter: f32) -> f32 {
    if jitter <= 0.0 { return 0.0; }
    let mut h = (i ^ seed_xor).wrapping_mul(2_654_435_761);
    h ^= h >> 16;
    let t = (h & 0xFFFF) as f32 / 65535.0;
    (t * 2.0 - 1.0) * jitter
}

// ─── Systems ──────────────────────────────────────────────────────────────────

/// Drains `PendingParticleEffects` and pushes each particle into the CPU pool.
///
/// No ECS entities are spawned — all particles live in `ParticlePool` and are
/// rendered as billboard quads by `rebuild_pool_meshes_system`.
pub fn drain_particle_effects_system(
    mut pending: ResMut<PendingParticleEffects>,
    mut pool: ResMut<ParticlePool>,
    asset_catalog: Res<LoadedAssetCatalog>,
) {
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

        let has_sprites = !def.sprites.is_empty();

        for i in 0..def.particle_count {
            // Resolve texture path from the asset catalog.
            let texture_path: String = if has_sprites {
                let idx = {
                    let mut h = i.wrapping_mul(2_654_435_761) ^ 0xDEAD_BEEF_u32;
                    h ^= h >> 16;
                    (h as usize) % def.sprites.len()
                };
                let key = &def.sprites[idx];
                match asset_catalog.0.textures.get(key) {
                    Some(path) => path.clone(),
                    None => {
                        warn!("SpawnEffect: sprites[{}] key {:?} not in catalog", idx, key);
                        String::new()
                    }
                }
            } else if let Some(key) = &def.sprite {
                match asset_catalog.0.textures.get(key) {
                    Some(path) => path.clone(),
                    None => {
                        warn!("SpawnEffect: sprite key {:?} not in catalog", key);
                        String::new()
                    }
                }
            } else {
                String::new() // untextured (sphere-like quad)
            };

            let dir = fibonacci_cone_dir(i, def.particle_count, half_angle);
            let speed = def.speed + hash_jitter(i, 0x0000_0000, def.speed_jitter);
            let velocity = dir * speed;
            let start_size = (def.size + hash_jitter(i, 0x9E37_79B9, def.size_jitter)).max(0.001);

            let spawn_pos = if def.emit_radius > 0.0 {
                let angle = 2.399_963_f32 * i as f32;
                let r = def.emit_radius * ((i as f32 + 0.5) / def.particle_count as f32).sqrt();
                effect.origin + Vec3::new(r * angle.cos(), 0.0, r * angle.sin())
            } else {
                effect.origin
            };

            let noise_seed = (i as f32 * 2.399_963_f32).fract() * std::f32::consts::TAU;

            pool.alloc(PooledParticle {
                position: spawn_pos,
                velocity,
                elapsed: 0.0,
                duration: def.lifetime_secs,
                start_size,
                end_size: def.size_end,
                gravity: def.gravity,
                turbulence: def.turbulence,
                noise_seed,
                color_start,
                color_mid,
                color_end,
                is_additive: def.additive,
                texture_path,
                uv_scroll_speed: def.uv_scroll_speed,
                uv_distort: def.uv_distort,
            });
        }
    }
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingParticleEffects>();
    }
}
