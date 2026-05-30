use bevy::prelude::*;
use crate::schema::catalog::{EffectDef, EffectPriority, EmitterShape, LayerDef, LineAxis, QualityLevel, QualityOverride};
use crate::runtime::scene_manager::{LoadedAssetCatalog, LevelEntity};
use crate::capabilities::particle_renderer::{ParticlePool, PooledParticle};
use crate::capabilities::fading_light::{FadingLight, MAX_FADING_LIGHTS};
use crate::capabilities::particle_budget::{ParticleBudget, ParticleQuality};

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

/// Spawn position for particle `i` of `count` from a layer.
fn emitter_spawn_pos(origin: Vec3, layer: &LayerDef, i: u32) -> Vec3 {
    let count = layer.particle_count;
    match &layer.emitter {
        EmitterShape::Point => {
            if layer.emit_radius > 0.0 {
                let angle = 2.399_963_f32 * i as f32;
                let r = layer.emit_radius * ((i as f32 + 0.5) / count as f32).sqrt();
                origin + Vec3::new(r * angle.cos(), 0.0, r * angle.sin())
            } else {
                origin
            }
        }
        EmitterShape::Disc { radius } => {
            let angle = 2.399_963_f32 * i as f32;
            let r = radius * ((i as f32 + 0.5) / count as f32).sqrt();
            origin + Vec3::new(r * angle.cos(), 0.0, r * angle.sin())
        }
        EmitterShape::Ring { radius } => {
            let angle = if count > 1 {
                (i as f32 / count as f32) * std::f32::consts::TAU
            } else {
                0.0
            };
            origin + Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin())
        }
        EmitterShape::Sphere { radius } => {
            let dir = fibonacci_cone_dir(i, count, std::f32::consts::PI);
            origin + dir * radius
        }
        EmitterShape::Line { length, axis } => {
            let t = if count > 1 { (i as f32 / (count - 1) as f32) - 0.5 } else { 0.0 };
            let offset = match axis {
                LineAxis::X => Vec3::new(length * t, 0.0, 0.0),
                LineAxis::Y => Vec3::new(0.0, length * t, 0.0),
                LineAxis::Z => Vec3::new(0.0, 0.0, length * t),
            };
            origin + offset
        }
        EmitterShape::Arc { radius, angle_deg } => {
            let half = angle_deg.to_radians() * 0.5;
            let angle = if count > 1 {
                -half + (i as f32 / (count - 1) as f32) * 2.0 * half
            } else {
                0.0
            };
            origin + Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin())
        }
    }
}

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

    // Pre-compute rotation constants for this layer.
    let rotation_start_rad = layer.rotation_start_deg.to_radians();
    let rotation_end_rad = if layer.rotation_speed_deg != 0.0 {
        rotation_start_rad + layer.rotation_speed_deg.to_radians() * layer.lifetime_secs
    } else {
        layer.rotation_end_deg.to_radians()
    };

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
        let start_size_x = layer.size_x.unwrap_or(start_size);
        let start_size_y = layer.size_y.unwrap_or(start_size);
        let end_size_x = layer.size_x_end.or(layer.size_end);
        let end_size_y = layer.size_y_end.or(layer.size_end);

        let spawn_pos = emitter_spawn_pos(origin, layer, i);
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
            rotation_rad: rotation_start_rad,
            rotation_start_rad,
            rotation_end_rad,
            start_size_x,
            start_size_y,
            end_size_x,
            end_size_y,
            velocity_curve: layer.velocity_curve.clone(),
            flipbook_cols: layer.flipbook.as_ref().map_or(0, |f| f.cols),
            flipbook_rows: layer.flipbook.as_ref().map_or(0, |f| f.rows),
            flipbook_fps:  layer.flipbook.as_ref().map_or(0.0, |f| f.fps),
            flipbook_loop: layer.flipbook.as_ref().map_or(false, |f| f.r#loop),
        });
    }
}

/// Applies the quality multiplier to `count`, or uses per-tier override when present.
/// Always returns at least 1.
fn scaled_count(count: u32, quality_override: &Option<QualityOverride>, quality: &ParticleQuality) -> u32 {
    if let Some(q) = quality_override {
        match quality.level {
            QualityLevel::Minimal => q.minimal,
            QualityLevel::Low     => q.low,
            QualityLevel::Medium  => q.medium,
            QualityLevel::High    => q.high.unwrap_or(count),
        }
    } else {
        (count as f32 * quality.multiplier()).round().max(1.0) as u32
    }
}

/// Applies budget gating: returns the number of particles that may actually spawn.
/// `Ambient`: 0 when budget is full. `Npc`: halved (min 1). `Player`: always full.
fn budgeted_count(count: u32, priority: &EffectPriority, live: u32, max: u32) -> u32 {
    if live + count <= max {
        return count;
    }
    match priority {
        EffectPriority::Player  => count,
        EffectPriority::Npc     => (count / 2).max(1),
        EffectPriority::Ambient => 0,
    }
}

/// Drains `PendingParticleEffects` and pushes each particle into the CPU pool.
///
/// Quality is applied via `ParticleQuality`: the global multiplier scales `particle_count`,
/// or per-layer `quality` overrides bypass it. Budget gating then checks the live pool count
/// against `ParticleBudget::max_count` and sheds effects by priority (`Ambient` first).
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
    quality: Res<ParticleQuality>,
    budget: Res<ParticleBudget>,
) {
    let current_light_count = live_lights.iter().count();
    let mut lights_spawned = 0usize;

    // Scan the pool once; increment `live` as we commit allocations this frame.
    // This is O(n) once rather than O(n × effects × layers), and correctly accounts
    // for particles allocated earlier in the same drain pass when gating later effects.
    let mut live = pool.particles.iter().filter(|p| p.is_alive()).count() as u32;

    for effect in pending.0.drain(..) {
        let def = &effect.def;
        if def.layers.is_empty() {
            let mut layer = LayerDef::from(def);
            let q_count = scaled_count(layer.particle_count, &layer.quality, &quality);
            let final_count = budgeted_count(q_count, &def.priority, live, budget.max_count);
            if final_count > 0 {
                layer.particle_count = final_count;
                alloc_layer(effect.origin, &layer, &mut pool, &asset_catalog);
                live += final_count;
            }
        } else {
            for layer_def in &def.layers {
                let mut layer = layer_def.clone();
                let q_count = scaled_count(layer.particle_count, &layer.quality, &quality);
                let final_count = budgeted_count(q_count, &def.priority, live, budget.max_count);
                if final_count > 0 {
                    layer.particle_count = final_count;
                    alloc_layer(effect.origin, &layer, &mut pool, &asset_catalog);
                    live += final_count;
                }
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
