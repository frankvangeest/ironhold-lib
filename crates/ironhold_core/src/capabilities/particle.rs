use bevy::prelude::*;
use crate::schema::catalog::{EffectDef, LayerDef};
use crate::runtime::scene_manager::{LoadedAssetCatalog, LevelEntity};
use crate::capabilities::particle_renderer::{ParticlePool, PooledParticle};
use crate::capabilities::fading_light::{FadingLight, MAX_FADING_LIGHTS};

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

/// Allocates all particles for one layer into the pool.
fn alloc_layer(origin: Vec3, layer: &LayerDef, pool: &mut ParticlePool, asset_catalog: &LoadedAssetCatalog) {
    let half_angle = layer.spread_deg.clamp(0.0, 180.0).to_radians();
    let color_start = Color::srgba(
        layer.color_start.0, layer.color_start.1, layer.color_start.2, layer.color_start.3,
    ).to_linear();
    let color_mid = layer.color_mid.map(|cm| Color::srgba(cm.0, cm.1, cm.2, cm.3).to_linear());
    let color_end = Color::srgba(
        layer.color_end.0, layer.color_end.1, layer.color_end.2, layer.color_end.3,
    ).to_linear();
    let has_sprites = !layer.sprites.is_empty();

    for i in 0..layer.particle_count {
        let texture_path: String = if has_sprites {
            let idx = {
                let mut h = i.wrapping_mul(2_654_435_761) ^ 0xDEAD_BEEF_u32;
                h ^= h >> 16;
                (h as usize) % layer.sprites.len()
            };
            let key = &layer.sprites[idx];
            match asset_catalog.0.textures.get(key) {
                Some(path) => path.clone(),
                None => {
                    warn!("SpawnEffect: sprites[{}] key {:?} not in catalog", idx, key);
                    String::new()
                }
            }
        } else if let Some(key) = &layer.sprite {
            match asset_catalog.0.textures.get(key) {
                Some(path) => path.clone(),
                None => {
                    warn!("SpawnEffect: sprite key {:?} not in catalog", key);
                    String::new()
                }
            }
        } else {
            String::new()
        };

        let dir = fibonacci_cone_dir(i, layer.particle_count, half_angle);
        let speed = layer.speed + hash_jitter(i, 0x0000_0000, layer.speed_jitter);
        let velocity = dir * speed;
        let start_size = (layer.size + hash_jitter(i, 0x9E37_79B9, layer.size_jitter)).max(0.001);

        let spawn_pos = if layer.emit_radius > 0.0 {
            let angle = 2.399_963_f32 * i as f32;
            let r = layer.emit_radius * ((i as f32 + 0.5) / layer.particle_count as f32).sqrt();
            origin + Vec3::new(r * angle.cos(), 0.0, r * angle.sin())
        } else {
            origin
        };
        let noise_seed = (i as f32 * 2.399_963_f32).fract() * std::f32::consts::TAU;

        pool.alloc(PooledParticle {
            position: spawn_pos,
            velocity,
            elapsed: 0.0,
            duration: layer.lifetime_secs,
            start_size,
            end_size: layer.size_end,
            gravity: layer.gravity,
            turbulence: layer.turbulence,
            noise_seed,
            color_start,
            color_mid,
            color_end,
            is_additive: layer.additive,
            texture_path,
            uv_scroll_speed: layer.uv_scroll_speed,
            uv_distort: layer.uv_distort,
        });
    }
}

/// Drains `PendingParticleEffects` and pushes each particle into the CPU pool.
///
/// Single-layer effects use the flat `EffectDef` fields. Multi-layer effects (`layers`
/// non-empty) spawn each layer independently at the same origin.
/// Pool particles live in `ParticlePool` and are rendered as billboard quads by
/// `rebuild_pool_meshes_system`. If the effect has a `light` block and the live fading-light
/// count is below `MAX_FADING_LIGHTS`, a `PointLight + FadingLight + LevelEntity` entity is
/// also spawned at the effect origin.
pub fn drain_particle_effects_system(
    mut commands: Commands,
    mut pending: ResMut<PendingParticleEffects>,
    mut pool: ResMut<ParticlePool>,
    asset_catalog: Res<LoadedAssetCatalog>,
    live_lights: Query<(), With<FadingLight>>,
) {
    let current_light_count = live_lights.iter().count();
    let mut lights_spawned = 0usize;

    for effect in pending.0.drain(..) {
        let def = &effect.def;
        if def.layers.is_empty() {
            let layer = LayerDef::from(def);
            alloc_layer(effect.origin, &layer, &mut pool, &asset_catalog);
        } else {
            for layer in &def.layers {
                alloc_layer(effect.origin, layer, &mut pool, &asset_catalog);
            }
        }

        if let Some(light_def) = &def.light {
            if current_light_count + lights_spawned < MAX_FADING_LIGHTS {
                let duration = light_def.duration_secs.unwrap_or_else(|| {
                    if def.layers.is_empty() {
                        def.lifetime_secs
                    } else {
                        def.layers.iter().map(|l| l.lifetime_secs).fold(0.0_f32, f32::max)
                    }
                });
                commands.spawn((
                    PointLight {
                        color: Color::srgb(light_def.color.0, light_def.color.1, light_def.color.2),
                        intensity: 0.0,
                        range: light_def.range,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(effect.origin),
                    FadingLight {
                        peak_intensity: light_def.intensity,
                        fade_in_secs: light_def.fade_in_secs,
                        fade_out_secs: light_def.fade_out_secs,
                        duration_secs: duration,
                        elapsed: 0.0,
                    },
                    LevelEntity,
                ));
                lights_spawned += 1;
            }
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
