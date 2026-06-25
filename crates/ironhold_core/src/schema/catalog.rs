use bevy::prelude::*;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use super::material::MaterialDef;
use super::player::{CameraConfig, InputMap};

pub const ASSET_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const PREFAB_CATALOG_SCHEMA_VERSION: u32 = 2;

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum PrefabKind {
    Actor,
    Prop,
    Primitive,
    Foliage,
}

impl Default for PrefabKind {
    fn default() -> Self { Self::Actor }
}

// ─── Foliage structs ──────────────────────────────────────────────────────────

/// Full foliage definition for `kind: Foliage` prefabs.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct FoliageDef {
    /// Asset catalog model key for the trunk GLB. `None` for bushes / pure foliage.
    #[serde(default)]
    pub trunk: Option<String>,
    pub clusters: FoliageClustersDef,
    pub material: FoliageMaterialDef,
    /// Whether clusters cast shadows. Default `true` uses an alpha-clipped
    /// depth prepass so shadows match the leaf card silhouettes. Set `false`
    /// to disable shadow casting entirely (cheaper; useful for dense bushes
    /// or when the square-shadow artefact is acceptable).
    #[serde(default = "foliage_cast_shadows_default")]
    pub cast_shadows: bool,
}

fn foliage_cast_shadows_default() -> bool { true }

/// Controls how leaf card clusters are distributed.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct FoliageClustersDef {
    /// Number of foliage clusters to spawn.
    pub count: u32,
    /// Sphere radius used to distribute cluster centres (Fibonacci sphere).
    pub emitter_radius: f32,
    /// Number of leaf cards baked into each cluster mesh.
    pub leaves_per_cluster: u32,
    pub leaf_scale_min: f32,
    pub leaf_scale_max: f32,
    /// Lifts the emitter sphere above the entity origin, in metres.
    /// Set this to roughly the height where the trunk meets the branches.
    /// Default `0.0` places the sphere at ground level which is correct
    /// only for bushes — trees typically need `1.5`–`2.5`.
    #[serde(default)]
    pub crown_height: f32,
    /// Biases cluster placement toward the top of the sphere.
    /// `0.0` = full sphere, `0.5` = upper hemisphere only,
    /// `0.75` = upper quarter.  `0.6` is a good default for trees.
    #[serde(default)]
    pub height_bias: f32,
    /// Seed for the Fibonacci spiral offset.  Different seeds rotate the
    /// cluster arrangement so two trees of the same prefab look varied.
    #[serde(default)]
    pub seed: u32,
}

impl Default for FoliageClustersDef {
    fn default() -> Self {
        Self { count: 6, emitter_radius: 1.2, leaves_per_cluster: 24,
               leaf_scale_min: 0.3, leaf_scale_max: 0.6,
               crown_height: 0.0, height_bias: 0.6, seed: 0 }
    }
}

/// Visual appearance of the foliage material.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct FoliageMaterialDef {
    /// Asset catalog texture key — alpha-masked PNG brush stroke.
    pub leaf_texture: String,
    pub color_highlight: (f32, f32, f32),
    pub color_midtone:   (f32, f32, f32),
    pub color_shadow:    (f32, f32, f32),
    /// Discrete toon bands: 2, 3, or 4.
    pub toon_bands: u8,
    /// Darkens the shadow side (0.0 = off, 1.0 = full AO).
    pub ao_intensity: f32,
}

impl Default for FoliageMaterialDef {
    fn default() -> Self {
        Self {
            leaf_texture: String::new(),
            color_highlight: (0.45, 0.72, 0.25),
            color_midtone:   (0.28, 0.55, 0.15),
            color_shadow:    (0.12, 0.32, 0.08),
            toon_bands: 3,
            ao_intensity: 0.4,
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum ColliderShapeKind {
    Cuboid,
    Sphere,
    Cylinder,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum PrimitiveShapeKind {
    Cuboid,
    Sphere,
    Cylinder,
    Capsule3d,
    Cone,
    Torus,
    ConicalFrustum,
    Plane,
}

/// Maximum `particle_count` allowed in any `EffectDef`. Validated at catalog load time.
pub const MAX_PARTICLES_PER_EFFECT: u32 = 256;

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct AssetCatalog {
    pub schema_version: u32,
    #[serde(default)]
    pub models: HashMap<String, ModelCatalogEntry>,
    #[serde(default)]
    pub textures: HashMap<String, String>,
    #[serde(default)]
    pub audio: HashMap<String, AudioEntry>,
    #[serde(default)]
    pub materials: HashMap<String, MaterialDef>,
    /// Particle burst effect definitions. Keyed by a designer-chosen name (e.g. `"hit_spark"`).
    /// Referenced by `Action::SpawnEffect { key: "hit_spark", ... }` in rules and behavior files.
    #[serde(default)]
    pub effects: HashMap<String, EffectDef>,
    /// Ground decal texture paths. Keyed by a designer-chosen name (e.g. `"aoe_fire_circle"`).
    /// Referenced by `Action::ProjectDecal { key: "aoe_fire_circle", ... }` in rules and behavior files.
    /// Values are asset-relative paths to the texture file (e.g. `"shared/textures/decals/ring_thick.png"`).
    #[serde(default)]
    pub decals: HashMap<String, String>,
}

impl AssetCatalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != ASSET_CATALOG_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported AssetCatalog schema_version {} (expected {})",
                self.schema_version, ASSET_CATALOG_SCHEMA_VERSION
            ));
        }
        for (key, entry) in &self.models {
            if entry.path.is_empty() {
                return Err(format!("AssetCatalog model \"{}\" has empty path", key));
            }
        }
        for (key, path) in &self.decals {
            if path.is_empty() {
                return Err(format!("AssetCatalog decal \"{}\" has empty path", key));
            }
        }
        for (key, effect) in &self.effects {
            if effect.layers.is_empty() {
                if effect.particle_count > MAX_PARTICLES_PER_EFFECT {
                    return Err(format!(
                        "AssetCatalog effect \"{}\": particle_count {} exceeds the maximum of {} — \
                         split into multiple effects or reduce particle_count",
                        key, effect.particle_count, MAX_PARTICLES_PER_EFFECT
                    ));
                }
                if effect.flipbook.is_some() && effect.uv_distort > 0.0 {
                    return Err(format!(
                        "AssetCatalog effect \"{}\": `flipbook` and `uv_distort` cannot be used \
                         together — flipbook uses UV sub-rects; uv_distort animates UVs in the shader",
                        key
                    ));
                }
            } else {
                for (i, layer) in effect.layers.iter().enumerate() {
                    if layer.particle_count > MAX_PARTICLES_PER_EFFECT {
                        return Err(format!(
                            "AssetCatalog effect \"{}\": layer[{}] particle_count {} exceeds the \
                             maximum of {} — reduce particle_count",
                            key, i, layer.particle_count, MAX_PARTICLES_PER_EFFECT
                        ));
                    }
                    if layer.flipbook.is_some() && layer.uv_distort > 0.0 {
                        return Err(format!(
                            "AssetCatalog effect \"{}\": layer[{}] has both `flipbook` and \
                             `uv_distort > 0` — these cannot be combined",
                            key, i
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

impl Default for AssetCatalog {
    fn default() -> Self {
        Self {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            models: HashMap::new(),
            textures: HashMap::new(),
            audio: HashMap::new(),
            materials: HashMap::new(),
            effects: HashMap::new(),
            decals: HashMap::new(),
        }
    }
}

/// Axis used by the `Line` emitter shape.
#[derive(Deserialize, Clone, Debug, Default)]
pub enum LineAxis {
    #[default]
    Y,
    X,
    Z,
}

/// Spawn-position distribution for particles in a `LayerDef`.
/// `Point` (default) preserves the existing `emit_radius` disc behavior when `emit_radius > 0`.
#[derive(Deserialize, Clone, Debug, Default)]
pub enum EmitterShape {
    /// All particles spawn at the origin. Falls back to `emit_radius` disc scatter if > 0.
    #[default]
    Point,
    /// Uniform disc: particles scattered across a horizontal disc of given radius.
    Disc { radius: f32 },
    /// Ring: particles evenly spaced around a circle circumference.
    Ring { radius: f32 },
    /// Sphere surface: particles uniformly distributed over a sphere using Fibonacci mapping.
    Sphere { radius: f32 },
    /// Line: particles spaced along a segment of given length.
    Line { length: f32, #[serde(default)] axis: LineAxis },
    /// Arc: particles evenly spaced along a partial ring.
    Arc { radius: f32, angle_deg: f32 },
}

/// Velocity falloff curve applied over particle lifetime. Scales the per-frame position
/// step; the stored `velocity` vector is unchanged.
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
pub enum VelocityCurve {
    /// Constant speed throughout lifetime. Default.
    #[default]
    Linear,
    /// Fast start, decelerates to a stop (good for impact bursts and shards).
    EaseOut,
    /// Slow start, accelerates toward end (good for rising energy or charge effects).
    EaseIn,
    /// Fast → slow → fast: peaks at start and end, troughs at mid-life (orbit-like bob).
    Pulse,
}

/// Quality level for `Action::SetParticleQuality` and the `ParticleQuality` resource.
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
pub enum QualityLevel {
    Minimal,
    Low,
    Medium,
    #[default]
    High,
}

/// Per-layer explicit particle counts for each quality tier.
/// When present on a `LayerDef`, these values bypass the global quality multiplier.
/// `high` is optional — when absent, the layer's `particle_count` is used as-is at High quality.
///
/// Example (RON): `quality: ( minimal: 1, low: 2, medium: 4 )`
/// Example (RON): `quality: ( minimal: 1, low: 2, medium: 4, high: 16 )`
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct QualityOverride {
    pub minimal: u32,
    pub low: u32,
    pub medium: u32,
    /// When `None`, falls back to the layer's `particle_count` at High quality.
    #[serde(default)]
    pub high: Option<u32>,
}

/// Budget priority for an effect. Controls shedding behaviour when the live particle
/// count approaches `ParticleBudget::max_count`.
///
/// Example (RON): `priority: Player`
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
pub enum EffectPriority {
    /// Always spawns at full count; may briefly exceed the budget.
    Player,
    /// Halved when budget is tight (minimum 1). Default.
    #[default]
    Npc,
    /// Silently skipped when the budget is exhausted.
    Ambient,
}

/// Sprite-sheet (flipbook) animation for a particle layer. Authored as `flipbook: (...)` on
/// `LayerDef` or `EffectDef`. Each particle advances through frames over its lifetime.
///
/// Row order: top-to-bottom, left-to-right (matches Aseprite / Photoshop sprite sheet export).
/// `loop: false` holds the last frame until `lifetime_secs` expires (despawn via lifetime).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlipbookDef {
    /// Number of columns in the sprite sheet. Must be ≥ 1.
    pub cols: u8,
    /// Number of rows in the sprite sheet. Must be ≥ 1.
    pub rows: u8,
    /// Frames per second. At 24.0 fps a 4×4 sheet plays in 0.67 s.
    pub fps: f32,
    /// When `true`, loops the animation for the full lifetime. Default: `false` (hold last frame).
    #[serde(default)]
    pub r#loop: bool,
}

/// A single emitter layer within an `EffectDef`. When `EffectDef.layers` is non-empty,
/// each layer is spawned independently at the same origin. All fields behave identically
/// to the matching flat fields on `EffectDef`.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct LayerDef {
    #[serde(default = "default_particle_count")]
    pub particle_count: u32,
    pub lifetime_secs: f32,
    #[serde(default)] pub speed: f32,
    #[serde(default)] pub speed_jitter: f32,
    #[serde(default = "default_spread_deg")] pub spread_deg: f32,
    #[serde(default = "default_effect_offset")] pub offset: (f32, f32, f32),
    #[serde(default)] pub emit_radius: f32,
    #[serde(default = "default_particle_size")] pub size: f32,
    #[serde(default)] pub size_end: Option<f32>,
    #[serde(default)] pub size_jitter: f32,
    pub color_start: (f32, f32, f32, f32),
    #[serde(default)] pub color_mid: Option<(f32, f32, f32, f32)>,
    pub color_end: (f32, f32, f32, f32),
    #[serde(default)] pub gravity: f32,
    #[serde(default)] pub turbulence: f32,
    #[serde(default)] pub sprite: Option<String>,
    #[serde(default)] pub sprites: Vec<String>,
    #[serde(default)] pub additive: bool,
    #[serde(default)] pub uv_distort: f32,
    #[serde(default)] pub uv_scroll_speed: f32,
    // ── Extended behaviours ──────────────────────────────────────────────────
    /// Rotation of the billboard quad at spawn, in degrees. Default: 0.
    #[serde(default)] pub rotation_start_deg: f32,
    /// Rotation at end of lifetime. Ignored when `rotation_speed_deg != 0`. Default: 0.
    #[serde(default)] pub rotation_end_deg: f32,
    /// Constant angular velocity in degrees/second. When non-zero, overrides
    /// `rotation_start_deg` / `rotation_end_deg` and spins at a fixed rate. Default: 0.
    #[serde(default)] pub rotation_speed_deg: f32,
    /// Independent billboard width override (metres). When set, overrides `size` for X.
    /// Use `size_x < size_y` for tall narrow shapes (flame tongues, shards). Default: None.
    #[serde(default)] pub size_x: Option<f32>,
    /// Independent billboard height override (metres). Default: None.
    #[serde(default)] pub size_y: Option<f32>,
    /// End-of-life billboard width. Defaults to `size_end` when not set. Default: None.
    #[serde(default)] pub size_x_end: Option<f32>,
    /// End-of-life billboard height. Defaults to `size_end` when not set. Default: None.
    #[serde(default)] pub size_y_end: Option<f32>,
    /// Spawn-position distribution. Overrides `emit_radius` when not `Point`. Default: `Point`.
    #[serde(default)] pub emitter: EmitterShape,
    /// Velocity scaling curve over lifetime. Default: `Linear` (constant speed).
    #[serde(default)] pub velocity_curve: VelocityCurve,
    /// Per-tier explicit particle counts. When set, bypasses the global quality multiplier.
    /// `High` always uses `particle_count`. Example: `quality: ( minimal: 1, low: 2, medium: 4 )`.
    #[serde(default)] pub quality: Option<QualityOverride>,
    /// Sprite-sheet animation. When set, each particle advances through UV frames over its
    /// lifetime. Cannot be combined with `uv_distort > 0` — validated at catalog load time.
    #[serde(default)] pub flipbook: Option<FlipbookDef>,
}

/// Particle burst effect definition. Authored in `AssetCatalog.effects` and referenced
/// by `Action::SpawnEffect { key: "...", entity: "{self}" }` in rules and behavior files.
///
/// **Single-layer** (existing format): set `lifetime_secs`, `color_start`, `color_end` and any
/// emission fields. All flat fields are used directly.
///
/// **Multi-layer**: set `layers: [( … ), ( … )]` and omit flat fields. Each layer is spawned
/// independently at the same origin; flat fields are ignored. `particle_count` must be ≤ 256
/// per layer — validated at catalog load time.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct EffectDef {
    /// Number of particles spawned. Must be ≤ 256; validated at catalog load time.
    /// Ignored when `layers` is non-empty.
    #[serde(default = "default_particle_count")]
    pub particle_count: u32,
    /// Seconds until all particles have faded out and despawned. Required for single-layer
    /// effects; unused (may be omitted) when `layers` is non-empty.
    #[serde(default = "default_lifetime_secs")]
    pub lifetime_secs: f32,
    /// Initial speed of each particle in m/s.
    #[serde(default)]
    pub speed: f32,
    /// Speed randomness: actual per-particle speed is in `[speed - speed_jitter, speed + speed_jitter]`.
    /// Determined by a deterministic per-index hash (no random state). Default: 0.0.
    #[serde(default)]
    pub speed_jitter: f32,
    /// Emission cone half-angle in **degrees** from the +Y axis.
    /// `0` = all particles go straight up, `90` = hemisphere, `180` = full sphere. Default: 180.
    #[serde(default = "default_spread_deg")]
    pub spread_deg: f32,
    /// World-space offset added to the resolved spawn position.
    /// When spawning relative to an entity, `(0.0, 1.0, 0.0)` places the burst at roughly chest
    /// height for a 1.8 m entity. Applies to both `entity`-resolved and explicit `position` spawns.
    /// Default: `(0.0, 1.0, 0.0)`.
    #[serde(default = "default_effect_offset")]
    pub offset: (f32, f32, f32),
    /// Radius of the horizontal disc from which particles are scattered at spawn (metres).
    /// Distributes spawn positions across the emitter surface so fire appears to emerge from
    /// the whole log area rather than a single point. Default: 0.0 (point emission).
    #[serde(default)]
    pub emit_radius: f32,
    /// Sphere radius of each particle in metres at spawn. Default: 0.06.
    #[serde(default = "default_particle_size")]
    pub size: f32,
    /// Particle sphere radius at end of lifetime. Interpolated linearly from `size`.
    /// `None` = constant size throughout the particle's life. Default: None.
    #[serde(default)]
    pub size_end: Option<f32>,
    /// Per-particle size randomness in metres. Each particle starts at `size ± size_jitter`.
    /// Uses a deterministic per-index hash independent of `speed_jitter`. Default: 0.0.
    #[serde(default)]
    pub size_jitter: f32,
    /// RGBA colour at spawn (linear sRGB, 0.0–1.0). Alpha 1.0 = fully opaque/bright.
    /// Required for single-layer effects; unused (may be omitted) when `layers` is non-empty.
    #[serde(default = "default_color_white")]
    pub color_start: (f32, f32, f32, f32),
    /// Optional midpoint colour for a three-stop gradient (start → mid → end).
    /// When `Some`, colour transitions start→mid in the first half of lifetime, then
    /// mid→end in the second. Useful for fire: white/yellow base → orange core → dark red tip.
    /// Default: `None` (linear two-stop interpolation).
    #[serde(default)]
    pub color_mid: Option<(f32, f32, f32, f32)>,
    /// RGBA colour at end of lifetime (linear sRGB). Alpha 0.0 = fully invisible.
    /// Required for single-layer effects; unused (may be omitted) when `layers` is non-empty.
    #[serde(default = "default_color_transparent")]
    pub color_end: (f32, f32, f32, f32),
    /// Y-axis acceleration in m/s². Negative = falls, positive = rises.
    /// Reference: `-2.0` light sparks, `-9.8` Earth-like, `0.0` floaty, `+2.0` rising embers.
    #[serde(default)]
    pub gravity: f32,
    /// Per-frame lateral noise applied to velocity (m/s²). Creates billowing and swirling
    /// instead of straight-line trajectories. Each particle has a unique noise phase set
    /// at spawn, so they move independently. Default: 0.0 (no turbulence).
    #[serde(default)]
    pub turbulence: f32,
    /// Optional texture key from `AssetCatalog.textures` for billboard sprite rendering.
    /// When set, particles are spawned as camera-facing flat quads with this texture applied
    /// as a base colour texture. The gradient colours (color_start/mid/end) tint the sprite.
    /// Default: `None` — particles are sphere meshes coloured by the gradient only.
    #[serde(default)]
    pub sprite: Option<String>,
    /// Optional list of texture keys for billboard sprite rendering. When non-empty, each
    /// particle in the burst picks a texture by a deterministic per-index hash, giving visual
    /// variety within a single burst without random state. Takes precedence over `sprite`.
    /// Default: empty.
    #[serde(default)]
    pub sprites: Vec<String>,
    /// Selects the alpha blending mode for sprite particles. Has no effect when `sprite` is `None`.
    /// `true` → `AlphaMode::Add` (bright areas add to background — good for fire and glow).
    /// `false` (default) → `AlphaMode::Blend` (standard alpha compositing — good for smoke).
    #[serde(default)]
    pub additive: bool,
    /// UV distortion strength for the flame particle shader. When non-zero the particle
    /// uses `FlameParticleMaterial` instead of `StandardMaterial`, animating the sprite UVs
    /// with tip-weighted sine waves so the flame wavers and flickers organically.
    /// Range [0..1]: 0.0 = no distortion (static sprite), 0.4 = natural campfire flicker,
    /// 1.0 = very heavy distortion. Has no effect when `sprite` is `None`. Default: 0.0.
    #[serde(default)]
    pub uv_distort: f32,
    /// UV scroll speed: how many texture heights the sprite shifts upward per second.
    /// Combine with `uv_distort` for a flowing flame look. Default: 0.0 (no scroll).
    /// Has no effect when `sprite` is `None`.
    #[serde(default)]
    pub uv_scroll_speed: f32,
    // ── Extended behaviours ──────────────────────────────────────────────────
    /// Rotation of the billboard quad at spawn, in degrees. Default: 0.
    #[serde(default)] pub rotation_start_deg: f32,
    /// Rotation at end of lifetime. Ignored when `rotation_speed_deg != 0`. Default: 0.
    #[serde(default)] pub rotation_end_deg: f32,
    /// Constant angular velocity in degrees/second. When non-zero, overrides
    /// `rotation_start_deg` / `rotation_end_deg` and spins at a fixed rate. Default: 0.
    #[serde(default)] pub rotation_speed_deg: f32,
    /// Independent billboard width override (metres). When set, overrides `size` for X. Default: None.
    #[serde(default)] pub size_x: Option<f32>,
    /// Independent billboard height override (metres). Default: None.
    #[serde(default)] pub size_y: Option<f32>,
    /// End-of-life billboard width. Defaults to `size_end` when not set. Default: None.
    #[serde(default)] pub size_x_end: Option<f32>,
    /// End-of-life billboard height. Defaults to `size_end` when not set. Default: None.
    #[serde(default)] pub size_y_end: Option<f32>,
    /// Spawn-position distribution. Overrides `emit_radius` when not `Point`. Default: `Point`.
    #[serde(default)] pub emitter: EmitterShape,
    /// Velocity scaling curve over lifetime. Default: `Linear` (constant speed).
    #[serde(default)] pub velocity_curve: VelocityCurve,
    /// Multi-layer emitter definitions. When non-empty, each entry is spawned independently
    /// at the same origin and all flat fields above are ignored. Allows complex effects
    /// (e.g. campfire body + hot core) to be defined in a single catalog key.
    #[serde(default)]
    pub layers: Vec<LayerDef>,
    /// Optional dynamic point light spawned at the effect origin when the effect fires.
    /// Fades in and out over the authored durations, then despawns automatically.
    /// Capped at `MAX_FADING_LIGHTS` simultaneous lights; excess spawns are silently skipped.
    #[serde(default)]
    pub light: Option<EffectLightDef>,
    /// Budget priority for this effect. Controls shedding order when `ParticleBudget::max_count`
    /// is approached. `Ambient` is dropped first; `Player` always fires. Default: `Npc`.
    #[serde(default)]
    pub priority: EffectPriority,
    /// Per-tier explicit particle count for single-layer effects. When set, bypasses the
    /// global quality multiplier for this effect. `High` always uses `particle_count`.
    /// Copied into `LayerDef` via `From<&EffectDef>` so single-layer effects support overrides.
    #[serde(default)]
    pub quality: Option<QualityOverride>,
    /// Sprite-sheet animation for single-layer effects. Copied into `LayerDef` via
    /// `From<&EffectDef>`. Cannot be combined with `uv_distort > 0`.
    #[serde(default)]
    pub flipbook: Option<FlipbookDef>,
}

/// Dynamic point light attached to a particle effect. Authored in `EffectDef.light`.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct EffectLightDef {
    /// RGB colour of the light (linear, 0.0–1.0 per channel).
    pub color: (f32, f32, f32),
    /// Peak luminous power in lumens (Bevy's physical units). 8000 ≈ warm campfire glow.
    pub intensity: f32,
    /// Radius of influence in metres.
    pub range: f32,
    /// Seconds to fade from 0 to `intensity`. Use 0.0 for an instant flash.
    pub fade_in_secs: f32,
    /// Seconds to fade from `intensity` back to 0 before despawn.
    pub fade_out_secs: f32,
    /// Total lifetime of the light in seconds. When `None`, defaults to the longest layer
    /// lifetime in the effect (or `EffectDef.lifetime_secs` for single-layer effects).
    #[serde(default)]
    pub duration_secs: Option<f32>,
}

impl From<&EffectDef> for LayerDef {
    fn from(d: &EffectDef) -> Self {
        Self {
            particle_count:     d.particle_count,
            lifetime_secs:      d.lifetime_secs,
            speed:              d.speed,
            speed_jitter:       d.speed_jitter,
            spread_deg:         d.spread_deg,
            offset:             d.offset,
            emit_radius:        d.emit_radius,
            size:               d.size,
            size_end:           d.size_end,
            size_jitter:        d.size_jitter,
            color_start:        d.color_start,
            color_mid:          d.color_mid,
            color_end:          d.color_end,
            gravity:            d.gravity,
            turbulence:         d.turbulence,
            sprite:             d.sprite.clone(),
            sprites:            d.sprites.clone(),
            additive:           d.additive,
            uv_distort:         d.uv_distort,
            uv_scroll_speed:    d.uv_scroll_speed,
            rotation_start_deg: d.rotation_start_deg,
            rotation_end_deg:   d.rotation_end_deg,
            rotation_speed_deg: d.rotation_speed_deg,
            size_x:             d.size_x,
            size_y:             d.size_y,
            size_x_end:         d.size_x_end,
            size_y_end:         d.size_y_end,
            emitter:            d.emitter.clone(),
            velocity_curve:     d.velocity_curve.clone(),
            quality:            d.quality.clone(),
            flipbook:           d.flipbook.clone(),
        }
    }
}

fn default_particle_count() -> u32 { 12 }
fn default_spread_deg() -> f32 { 180.0 }
fn default_effect_offset() -> (f32, f32, f32) { (0.0, 1.0, 0.0) }
fn default_particle_size() -> f32 { 0.06 }
fn default_lifetime_secs() -> f32 { 1.0 }
fn default_color_white() -> (f32, f32, f32, f32) { (1.0, 1.0, 1.0, 1.0) }
fn default_color_transparent() -> (f32, f32, f32, f32) { (1.0, 1.0, 1.0, 0.0) }

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ModelCatalogEntry {
    pub path: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct AudioEntry {
    pub path: String,
    #[serde(default = "default_audio_volume")]
    pub volume: f32,
}

fn default_audio_volume() -> f32 { 1.0 }

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct PrefabCatalog {
    pub schema_version: u32,
    #[serde(default)]
    pub prefabs: HashMap<String, PrefabDef>,
}

impl PrefabCatalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != PREFAB_CATALOG_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported PrefabCatalog schema_version {} (expected {})",
                self.schema_version, PREFAB_CATALOG_SCHEMA_VERSION
            ));
        }
        for (key, prefab) in &self.prefabs {
            if prefab.kind == PrefabKind::Foliage && prefab.foliage.is_none() {
                return Err(format!(
                    "Prefab \"{}\" has kind Foliage but no `foliage` block",
                    key
                ));
            }
            if prefab.kind == PrefabKind::Foliage {
                let def = prefab.foliage.as_ref().unwrap();
                if def.material.leaf_texture.is_empty() {
                    return Err(format!(
                        "Prefab \"{}\": `foliage.material.leaf_texture` must not be empty",
                        key
                    ));
                }
                if def.clusters.leaves_per_cluster == 0 {
                    return Err(format!(
                        "Prefab \"{}\": `foliage.clusters.leaves_per_cluster` must be > 0",
                        key
                    ));
                }
                if !(2..=4).contains(&def.material.toon_bands) {
                    return Err(format!(
                        "Prefab \"{}\": `foliage.material.toon_bands` must be 2, 3, or 4",
                        key
                    ));
                }
            }
            if prefab.kind == PrefabKind::Primitive && prefab.children.is_empty() && prefab.shape.is_none() {
                return Err(format!(
                    "Prefab \"{}\" has kind Primitive but no `shape` field (required for single-mesh primitives)",
                    key
                ));
            }
            if prefab.kind == PrefabKind::Primitive && !prefab.model.is_empty() {
                return Err(format!(
                    "Prefab \"{}\": `model` must be empty for Primitive prefabs; use `shape` instead",
                    key
                ));
            }
            for (i, child) in prefab.children.iter().enumerate() {
                match (&child.prefab, &child.shape) {
                    (Some(_), Some(_)) => {
                        return Err(format!(
                            "Prefab \"{}\", child {}: `shape` and `prefab` are mutually exclusive",
                            key, i
                        ));
                    }
                    (None, None) => {
                        return Err(format!(
                            "Prefab \"{}\", child {}: must set either `shape` or `prefab`",
                            key, i
                        ));
                    }
                    (Some(nested_key), _) if !self.prefabs.contains_key(nested_key.as_str()) => {
                        return Err(format!(
                            "Prefab \"{}\", child {}: nested prefab \"{}\" not found in catalog",
                            key, i, nested_key
                        ));
                    }
                    _ => {}
                }
            }
        }
        // Cycle detection — DFS to find circular nested-prefab references.
        let mut visited: HashSet<String> = HashSet::new();
        for key in self.prefabs.keys() {
            if !visited.contains(key.as_str()) {
                let mut visiting: HashSet<String> = HashSet::new();
                if prefab_has_cycle(key, &self.prefabs, &mut visiting, &mut visited) {
                    return Err(format!(
                        "Circular nested-prefab reference detected (cycle includes \"{}\")",
                        key
                    ));
                }
            }
        }
        Ok(())
    }
}

/// DFS cycle detection over the nested-prefab graph.
/// `visiting` = keys currently on the call stack (grey); `visited` = fully explored (black).
fn prefab_has_cycle(
    key: &str,
    prefabs: &HashMap<String, PrefabDef>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> bool {
    if visiting.contains(key) { return true; }
    if visited.contains(key)  { return false; }
    visiting.insert(key.to_string());
    if let Some(prefab) = prefabs.get(key) {
        for child in &prefab.children {
            if let Some(nested) = &child.prefab {
                if prefab_has_cycle(nested, prefabs, visiting, visited) {
                    return true;
                }
            }
        }
    }
    visiting.remove(key);
    visited.insert(key.to_string());
    false
}

impl Default for PrefabCatalog {
    fn default() -> Self {
        Self {
            schema_version: PREFAB_CATALOG_SCHEMA_VERSION,
            prefabs: HashMap::new(),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct PrefabDef {
    pub kind: PrefabKind,
    #[serde(default)]
    pub model: String,  // key into AssetCatalog.models; empty for Primitive and Foliage
    /// Shape for `kind: Primitive` prefabs. Required when kind is Primitive, None otherwise.
    #[serde(default)]
    pub shape: Option<PrimitiveShapeKind>,
    /// Foliage definition for `kind: Foliage` prefabs.
    #[serde(default)]
    pub foliage: Option<FoliageDef>,
    #[serde(default)]
    pub animation_policy: Option<String>,
    /// Optional material key from AssetCatalog.materials to override the model's embedded material.
    #[serde(default)]
    pub material: Option<String>,
    #[serde(default)]
    pub components: PrefabComponents,
    /// Shape parameters for `kind: "primitive"`. All fields are optional; hardcoded defaults apply
    /// when omitted so minimal RON is still valid.
    #[serde(default)]
    pub primitive: Option<PrimitiveParams>,
    /// For `kind: "primitive"` only — child meshes composing this prefab.
    /// When non-empty, `model` and `primitive` at the top level are ignored for mesh
    /// building; each child is spawned as a mesh entity under a shared parent anchor.
    #[serde(default)]
    pub children: Vec<ChildPrimitiveDef>,
    /// Optional continuous transform animation applied to this entity at runtime.
    /// Supports world-space rotation and sinusoidal vertical bob.
    #[serde(default)]
    pub motion: Option<MotionDef>,
    /// Path (project-relative) to a `.behavior.ron` (`StateMachineAsset`) that drives
    /// per-entity FSM logic. Loaded asynchronously; initial state is set once loaded.
    #[serde(default)]
    pub behavior: Option<String>,
    /// When set, the entity emits `entity.interacted:{id}` when the player is within
    /// `radius` metres and presses the interact key (configured via `inputs.interact` in
    /// the player prefab; default: `"KeyF"`).
    #[serde(default)]
    pub interactable: Option<InteractableDef>,
    /// When set, a Rapier sensor collider is spawned and the entity emits
    /// `entity.entered:{id}` / `entity.exited:{id}` on player overlap.
    #[serde(default)]
    pub trigger_zone: Option<TriggerZoneDef>,
    /// Per-instance stat shapes. At spawn time each template produces one `LiveStat` inside
    /// a `StatMap` component on the entity. Address instance stats with `"{spawn_id}.{key}"`.
    /// `{self}` inside `emit` strings is replaced with the entity's spawn ID.
    #[serde(default)]
    pub stat_templates: Vec<crate::schema::stats::StatTemplateDef>,
    /// Optional floating stat label. When set, the scene loader spawns a world-space `Text2d`
    /// that follows this entity and updates its text with the resolved stat each frame.
    /// `{self}` in `stat_key` is replaced with the entity's spawn ID at load time.
    #[serde(default)]
    pub stat_label: Option<StatLabelDef>,
    /// Optional floating stat bar above this entity. Renders as two overlapping `Text2d`
    /// entities (background track + animated fill) using Unicode block characters.
    /// `{self}` in `stat_key` is replaced with the entity's spawn ID at load time.
    #[serde(default)]
    pub world_stat_bar: Option<WorldStatBarDef>,
    /// One or more static physics colliders for `kind: "actor"` / `kind: "prop"` prefabs.
    /// All shapes are combined into a single Rapier compound `RigidBody::Fixed` so the player
    /// can stand on or collide with the GLB without primitive wrappers. Use multiple entries
    /// to approximate curved geometry (arches, irregular props) or multi-part shapes (chest lid
    /// + base). An empty list means no physics collider is attached.
    #[serde(default)]
    pub colliders: Vec<ColliderDef>,
    /// When `true`, left-clicking near this entity on screen sets it as `CurrentTarget` and
    /// emits `target.clicked:{id}` and `target.changed:{id}` into the pipeline. Selection is
    /// screen-space proximity — the entity whose projected position is nearest the cursor
    /// (within a fixed pixel radius) — resolved from the entity's `GlobalTransform`, NOT a
    /// mesh raycast. This works for animated/skinned GLB characters as well as primitives.
    #[serde(default)]
    pub click_selectable: bool,
    /// When `true`, this entity participates in Tab-cycle targeting (nearest-first within
    /// `target_range`). Pressing Tab selects the next entity; Shift+Tab reverses.
    /// Setting a target emits `target.changed:{id}` into the pipeline.
    #[serde(default)]
    pub targetable: bool,
    /// Per-prefab target-indicator ring colour override (RGBA), highest precedence.
    /// When set, this colour is used directly regardless of `indicator_category` or the
    /// scene `target_indicator.color`. Only meaningful when the prefab is selectable.
    #[serde(default)]
    pub indicator_color: Option<(f32, f32, f32, f32)>,
    /// Category key looked up in the scene's `target_indicator.named_colors` map to pick
    /// the ring colour. Ignored if `indicator_color` is set. Falls through to scene
    /// `target_indicator.color` when the key is absent from the map.
    #[serde(default)]
    pub indicator_category: Option<String>,
    /// Vertical offset (metres) from the entity world origin used when projecting to screen
    /// space for click-selection. Defaults to 1.0 (body centre for human-scale characters).
    /// Set lower for ground-hugging creatures (e.g. 0.4 for a snake, 0.6 for a spider).
    /// Only meaningful when `click_selectable: true`.
    #[serde(default = "default_select_aim_height")]
    pub select_aim_height: f32,
    /// Project-relative path to a `.dialogue.ron` file that drives conversation with this entity.
    /// When set, the scene loader inserts a `DialoguePath` component on the spawned entity.
    /// The `dialogue_tick_system` detects `entity.interacted:{id}` for entities with this component
    /// and automatically fires `Action::StartDialogue` — no rule wiring required.
    /// Example: `"dialogues/npc_intro.dialogue.ron"`.
    #[serde(default)]
    pub dialogue: Option<String>,
    /// When set, this entity has an inventory container with the given slot count.
    /// Inventory components are cleared on scene load (use `PlayerInventory` resource for persistence).
    #[serde(default)]
    pub inventory: Option<InventoryContainerDef>,
    /// When set, this entity acts as a merchant. `Action::OpenShop(entity_id)` populates
    /// the scene's `ShopPanel` with this entity's stock.
    #[serde(default)]
    pub merchant: Option<MerchantDef>,
    /// Display name shown in the nameplate widget.
    /// Falls back to the prefab key (e.g. `"orc_enemy"`) when `None`.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Nameplate visibility override.
    /// `None` — inherit scene `show_nameplates` + `faction_filter` (default).
    /// `Some(true)` — always show (bypasses faction filter; respects `max_distance`).
    /// `Some(false)` — never show, even when the scene has `show_nameplates: true`.
    #[serde(default)]
    pub nameplate: Option<bool>,
}

pub(crate) fn default_select_aim_height() -> f32 { 1.0 }

/// One physics collider shape in a `PrefabDef.colliders` list.
/// All geometry fields are optional; reasonable defaults apply.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ColliderDef {
    pub shape: ColliderShapeKind,
    /// Half-extents override for Cuboid: `(width, height, depth)` in world units.
    #[serde(default)]
    pub size: Option<(f32, f32, f32)>,
    /// Radius for Sphere / Cylinder.
    #[serde(default)]
    pub radius: Option<f32>,
    /// Total height for Cylinder.
    #[serde(default)]
    pub height: Option<f32>,
    /// Local-space offset of this shape from the entity origin.
    #[serde(default)]
    pub offset: (f32, f32, f32),
    /// Euler rotation in degrees (XYZ order) for this shape's local orientation. Default: `(0, 0, 0)`.
    #[serde(default)]
    pub rotation_euler_deg: (f32, f32, f32),
}

/// Continuous transform animation for a prefab entity.
/// Converted to a `Motion` Bevy component at spawn time.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct MotionDef {
    /// World-space continuous rotation in radians per second (x, y, z).
    /// Example: `(0.0, 1.5, 0.0)` spins around world Y at ~14 RPM.
    #[serde(default)]
    pub rotate: Option<(f32, f32, f32)>,
    /// Sinusoidal vertical bob: `(amplitude_meters, frequency_hz)`.
    /// Example: `(0.15, 0.8)` = ±15 cm at 0.8 Hz.
    #[serde(default)]
    pub bob: Option<(f32, f32)>,
}

/// Configuration for the Interactable capability.
/// Emits `entity.interacted:{id}` when the player is within `radius` metres and presses the
/// interact key (configured via `inputs.interact` in the player prefab; default: `"KeyF"`).
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct InteractableDef {
    /// Metres — player must be closer than this to interact.
    pub radius: f32,
    /// Optional text shown near the entity when the player is in range.
    #[serde(default)]
    pub hint_text: Option<String>,
}

/// Configuration for the TriggerZone capability.
/// Spawns a Rapier sphere sensor; emits `entity.entered:{id}` / `entity.exited:{id}`.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TriggerZoneDef {
    /// Radius of the sphere sensor in metres.
    #[serde(default = "default_trigger_radius")]
    pub radius: f32,
}

fn default_trigger_radius() -> f32 { 2.0 }

/// Floating world-space label that tracks a live stat and updates its text each frame.
/// Authored in `PrefabDef.stat_label`. `{self}` in `stat_key` is resolved at scene load.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct StatLabelDef {
    /// Stat key to display. Supports `{self}` substitution.
    /// Entity-local: `"{self}.health"` → `"dummy_01.health"`. Global: `"player_health"`.
    pub stat_key: String,
    /// World-space offset from the entity's origin in metres. Default: 2.5 units up.
    #[serde(default = "default_stat_label_offset")]
    pub offset: (f32, f32, f32),
    /// Font size in screen pixels. Default: 16.
    #[serde(default = "default_stat_label_font_size")]
    pub font_size: f32,
    /// Label colour as linear RGBA (0.0–1.0). Default: bright green.
    #[serde(default = "default_stat_label_color")]
    pub color: (f32, f32, f32, f32),
    /// Show `"current / max"` instead of just `"current"`. Default: true.
    #[serde(default = "default_stat_label_show_max")]
    pub show_max: bool,
}

fn default_stat_label_offset() -> (f32, f32, f32) { (0.0, 2.5, 0.0) }
fn default_stat_label_font_size() -> f32 { 16.0 }
fn default_stat_label_color() -> (f32, f32, f32, f32) { (0.2, 0.9, 0.2, 1.0) }
fn default_stat_label_show_max() -> bool { true }

/// World-space stat bar floating above an entity. Visual mode chosen via `style`.
/// Authored in `PrefabDef.world_stat_bar`. `{self}` in `stat_key` is resolved at scene load.
/// Shared fields (`fill_color`, `bg_color`, `color_bands`) apply to all styles.
/// `style` defaults to `Ascii` — existing bars with no `style` field require no changes.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct WorldStatBarDef {
    /// Stat key — e.g. `"{self}.health"` (entity-local) or `"player_mana"` (global).
    pub stat_key: String,
    /// World-space offset from the entity's origin in metres. Default: `(0.0, 2.8, 0.0)`.
    #[serde(default = "default_world_bar_offset")]
    pub offset: (f32, f32, f32),
    /// Fill base colour (RGBA linear). Used when `color_bands` is absent or no band matches.
    /// Default: bright green `(0.15, 0.85, 0.15, 0.95)`.
    #[serde(default = "default_world_bar_fill_color")]
    pub fill_color: (f32, f32, f32, f32),
    /// Background / track colour (RGBA linear). Default: dark red-brown `(0.25, 0.08, 0.08, 0.75)`.
    #[serde(default = "default_world_bar_bg_color")]
    pub bg_color: (f32, f32, f32, f32),
    /// Threshold-based fill colour overrides. Each entry: `(min_ratio, rgba)`.
    /// The entry with the highest `min_ratio` ≤ current fill ratio is selected.
    /// Example: `[(0.0, red), (0.3, yellow), (0.6, green)]` — green ≥ 60%, yellow ≥ 30%, red otherwise.
    #[serde(default)]
    pub color_bands: Vec<(f32, (f32, f32, f32, f32))>,
    /// Visual rendering mode. Default: `Ascii` — existing bars require no `style` field.
    #[serde(default)]
    pub style: WorldStatBarStyle,
}

/// Visual rendering mode for `WorldStatBarDef`.
#[derive(Deserialize, Debug, Clone)]
pub enum WorldStatBarStyle {
    /// ASCII character bar (`=` fill on space track). Default mode.
    Ascii {
        /// Total character cells. Practical range: 1–32. Default: 10.
        #[serde(default = "default_world_bar_cells")]
        cells: u8,
        /// Font size in screen pixels. Default: 14.
        #[serde(default = "default_world_bar_font_size")]
        font_size: f32,
    },
    /// Pixel-rendered sprite-quad bar (border + background + fill).
    /// Size is in screen pixels — constant at all camera distances (no depth scaling in v1).
    Pixel {
        /// Bar dimensions in screen pixels `(width, height)`. Clamped to min `(1.0, 1.0)`.
        /// Default: `(64.0, 8.0)`.
        #[serde(default = "default_pixel_bar_size")]
        size: (f32, f32),
        /// Border thickness in screen pixels. `0.0` disables the border sprite.
        /// Clamped to `[0.0, height / 2.0]`. Default: `1.5`.
        #[serde(default = "default_pixel_bar_border")]
        border: f32,
        /// Border quad colour (RGBA linear). Default: near-black `(0.05, 0.05, 0.05, 1.0)`.
        #[serde(default = "default_pixel_bar_border_color")]
        border_color: (f32, f32, f32, f32),
    },
}

impl Default for WorldStatBarStyle {
    fn default() -> Self {
        WorldStatBarStyle::Ascii {
            cells: default_world_bar_cells(),
            font_size: default_world_bar_font_size(),
        }
    }
}

fn default_world_bar_offset() -> (f32, f32, f32) { (0.0, 2.8, 0.0) }
fn default_world_bar_cells() -> u8 { 10 }
fn default_world_bar_font_size() -> f32 { 14.0 }
fn default_world_bar_fill_color() -> (f32, f32, f32, f32) { (0.15, 0.85, 0.15, 0.95) }
fn default_world_bar_bg_color() -> (f32, f32, f32, f32) { (0.25, 0.08, 0.08, 0.75) }
fn default_pixel_bar_size() -> (f32, f32) { (64.0, 8.0) }
fn default_pixel_bar_border() -> f32 { 1.5 }
fn default_pixel_bar_border_color() -> (f32, f32, f32, f32) { (0.05, 0.05, 0.05, 1.0) }

/// NPC faction — determines intent and which events fire.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum NpcFaction {
    Friendly,
    Hostile,
    Neutral,
}

/// What the NPC does once it has detected the player.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum NpcOnPlayerNear {
    /// Move toward the player; emits `npc.player_reached:{id}` when in range.
    Chase,
    /// Approach the player with friendly intent; emits `npc.player_reached:{id}`.
    Interact,
    /// Run away from the player.
    Flee,
    /// Stop and face the player; does not move.
    Alert,
}

/// Data-driven NPC behaviour configuration.
/// Place this in `PrefabComponents.npc` in `prefabs.ron` to give an entity NPC AI.
#[derive(Deserialize, Debug, Clone)]
pub struct NpcDef {
    pub faction: NpcFaction,
    /// What the NPC does upon detecting the player.
    pub on_player_near: NpcOnPlayerNear,
    /// Metres — NPC enters Alerted state when player is inside this radius.
    pub detection_radius: f32,
    /// Metres — NPC gives up chasing and returns to patrol beyond this radius.
    pub chase_radius: f32,
    /// Forward FOV in degrees. `None` = 360° awareness (no blind spot).
    /// Example: `Some(120.0)` — player must be within a 120° forward cone.
    #[serde(default)]
    pub fov_degrees: Option<f32>,
    /// When `true`, a Rapier ray cast from the NPC's eye to the player must
    /// succeed before detection triggers (walls/obstacles block sight).
    #[serde(default)]
    pub requires_los: bool,
    /// Stop approaching at this distance — interact / attack range.
    #[serde(default = "default_approach_distance")]
    pub approach_distance: f32,
    /// m/s while walking the patrol route.
    #[serde(default = "default_patrol_speed")]
    pub patrol_speed: f32,
    /// m/s while chasing or fleeing.
    #[serde(default = "default_chase_speed")]
    pub chase_speed: f32,
    /// Patrol waypoints as offsets relative to the NPC's spawn position.
    /// Empty → NPC idles in place.
    #[serde(default)]
    pub patrol_waypoints: Vec<(f32, f32, f32)>,
    /// Eye height above the entity origin used for line-of-sight ray casts.
    /// Default: 0.9 m (reasonable for a ~1.8 m tall humanoid).
    #[serde(default = "default_npc_eye_height")]
    pub eye_height: f32,
    /// Seconds the NPC pauses in the Alerted state before acting.
    /// Default: 0.3 s.
    #[serde(default = "default_npc_alerted_duration")]
    pub alerted_duration: f32,
    /// Velocity decay multiplier applied each physics tick when the NPC is not moving.
    /// Values closer to 1.0 are slippery; closer to 0.0 stop instantly. Default: 0.8.
    #[serde(default = "default_npc_drag")]
    pub drag: f32,
    /// Metres from a waypoint at which the NPC advances to the next one. Default: 0.5 m.
    #[serde(default = "default_npc_waypoint_reach_radius")]
    pub waypoint_reach_radius: f32,
    /// Seconds to stand idle at each waypoint before moving on. 0.0 = advance immediately.
    #[serde(default)]
    pub waypoint_wait_secs: f32,
    /// Multiplier applied to `approach_distance` to define the leave-interact threshold.
    /// The NPC exits Interact state when `distance > approach_distance * interact_leave_factor`.
    /// Default: 1.5.
    #[serde(default = "default_npc_interact_leave_factor")]
    pub interact_leave_factor: f32,
    /// Metres from spawn origin at which the NPC considers itself home and ends Return state.
    /// Default: 0.5.
    #[serde(default = "default_npc_home_arrival_radius")]
    pub home_arrival_radius: f32,
    /// Rapier `linear_damping` on the NPC capsule rigid body. Default: 0.5.
    #[serde(default = "default_linear_damping")]
    pub linear_damping: f32,
    /// Rapier `angular_damping` on the NPC capsule rigid body. Default: 0.5.
    #[serde(default = "default_angular_damping")]
    pub angular_damping: f32,
    /// Radius of the physics capsule collider. Default: 0.35 m (humanoid).
    /// Tune for very large (dragon) or very small (imp) creatures.
    #[serde(default)]
    pub collider_radius: Option<f32>,
    /// Total height of the physics capsule collider. Default: 1.6 m (humanoid).
    /// Tune for creatures significantly taller or shorter than a humanoid.
    #[serde(default)]
    pub collider_height: Option<f32>,
    /// Seconds the NPC walks toward the last-known attacker position before giving up.
    /// Resets on each subsequent hit — enables kiting. Default: 5.0 s.
    #[serde(default = "default_npc_investigate_timeout")]
    pub investigate_timeout_secs: f32,
}

fn default_approach_distance() -> f32 { 2.0 }
fn default_patrol_speed() -> f32 { 2.0 }
fn default_chase_speed() -> f32 { 4.5 }
fn default_npc_eye_height() -> f32 { 0.9 }
fn default_npc_alerted_duration() -> f32 { 0.3 }
fn default_npc_drag() -> f32 { 0.8 }
fn default_npc_waypoint_reach_radius() -> f32 { 0.5 }
fn default_npc_interact_leave_factor() -> f32 { 1.5 }
fn default_npc_home_arrival_radius() -> f32 { 0.5 }
fn default_npc_investigate_timeout() -> f32 { 10.0 }

/// Speed, sensitivity, and key-binding tuning for a free-flying camera.
/// All fields are optional — omitting them keeps the compiled-in defaults.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct FlyCamDef {
    /// Normal movement speed in units/second. Default: 100.0.
    #[serde(default = "default_flycam_speed")]
    pub speed: f32,
    /// Movement speed when Shift is held, in units/second. Default: 200.0.
    #[serde(default = "default_flycam_fast_speed")]
    pub fast_speed: f32,
    /// Mouse look sensitivity in radians per pixel. Default: 0.002.
    #[serde(default = "default_flycam_sensitivity")]
    pub sensitivity: f32,
    /// Key for moving forward. Default: `"KeyW"`.
    #[serde(default = "default_flycam_forward")]
    pub forward: String,
    /// Key for moving backward. Default: `"KeyS"`.
    #[serde(default = "default_flycam_backward")]
    pub backward: String,
    /// Key for strafing left. Default: `"KeyA"`.
    #[serde(default = "default_flycam_left")]
    pub left: String,
    /// Key for strafing right. Default: `"KeyD"`.
    #[serde(default = "default_flycam_right")]
    pub right: String,
    /// Key for ascending. Default: `"Space"`.
    #[serde(default = "default_flycam_up")]
    pub up: String,
    /// Key for descending. Default: `"KeyQ"`.
    #[serde(default = "default_flycam_down")]
    pub down: String,
    /// Mouse button that activates look mode. `"Left"`, `"Right"`, or `"Either"`. Default: `"Either"`.
    #[serde(default = "default_flycam_look_button")]
    pub look_button: String,
}

impl Default for FlyCamDef {
    fn default() -> Self {
        Self {
            speed: default_flycam_speed(),
            fast_speed: default_flycam_fast_speed(),
            sensitivity: default_flycam_sensitivity(),
            forward: default_flycam_forward(),
            backward: default_flycam_backward(),
            left: default_flycam_left(),
            right: default_flycam_right(),
            up: default_flycam_up(),
            down: default_flycam_down(),
            look_button: default_flycam_look_button(),
        }
    }
}

fn default_flycam_speed() -> f32 { 100.0 }
fn default_flycam_fast_speed() -> f32 { 200.0 }
fn default_flycam_sensitivity() -> f32 { 0.002 }
fn default_flycam_forward() -> String { "KeyW".to_string() }
fn default_flycam_backward() -> String { "KeyS".to_string() }
fn default_flycam_left() -> String { "KeyA".to_string() }
fn default_flycam_right() -> String { "KeyD".to_string() }
fn default_flycam_up() -> String { "Space".to_string() }
fn default_flycam_down() -> String { "KeyQ".to_string() }
fn default_flycam_look_button() -> String { "Either".to_string() }

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct PrefabComponents {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub movement: MovementConfig,
    /// Maps event names to asset catalog audio keys.
    /// Used by systems to look up what sound to play for a given event.
    /// Example: `{ "collect": "collect_coin", "jump": "jump" }`
    #[serde(default)]
    pub sounds: HashMap<String, String>,
    /// NPC behaviour definition. When set, the runtime attaches an `NpcAgent`
    /// component and a dynamic physics body to the spawned entity.
    #[serde(default)]
    pub npc: Option<NpcDef>,
    /// Key bindings for the player character. Falls back to WASD defaults when absent.
    /// Only read for prefabs with `tags: ["player"]`.
    #[serde(default)]
    pub inputs: Option<InputMap>,
    /// Speed and sensitivity tuning for a free-flying camera.
    /// Only read for prefabs with `tags: ["flycam"]`.
    #[serde(default)]
    pub flycam: Option<FlyCamDef>,
    /// Orbit camera configuration for the player.
    /// Only read for prefabs with `tags: ["player"]`.
    /// When omitted, engine defaults apply (offset 10 m behind, 5 m up).
    #[serde(default)]
    pub camera: Option<CameraConfig>,
}

/// Movement parameters for any prefab with the "player" tag (primitive or GLB).
/// All fields are optional; omitting a field keeps the runtime default.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct MovementConfig {
    /// Walking speed in m/s. Default: 5.0.
    #[serde(default = "default_walk_speed")]
    pub walk_speed: f32,
    /// Running speed in m/s. Default: 10.0.
    #[serde(default = "default_run_speed")]
    pub run_speed: f32,
    /// Yaw rotation speed in rad/s. Default: 3.0.
    #[serde(default)]
    pub rot_speed: Option<f32>,
    /// Jump height. Default: `RelativeToHeight` with `percent: 100` (player's own height).
    #[serde(default)]
    pub jump: Option<JumpConfig>,
    /// Enable a second jump while airborne. Default: false.
    #[serde(default)]
    pub double_jump: bool,
    /// Height for the second jump. If omitted, uses the same height as `jump`.
    #[serde(default)]
    pub double_jump_height: Option<JumpConfig>,
    /// Capsule collider radius (GLB players). Ignored for primitive players (use shape `radius`). Default: 0.4 m.
    #[serde(default)]
    pub collider_radius: Option<f32>,
    /// Capsule total height (GLB players). Ignored for primitive players (use shape `height`). Default: 1.8 m.
    #[serde(default)]
    pub collider_height: Option<f32>,
    /// Velocity decay multiplier each physics tick when no input is given (XZ plane).
    /// Lower values stop the player faster; higher values are more slippery. Default: 0.8.
    #[serde(default = "default_idle_drag")]
    pub idle_drag: f32,
    /// Rapier `linear_damping` on the player capsule rigid body. Default: 0.5.
    #[serde(default = "default_linear_damping")]
    pub linear_damping: f32,
    /// Rapier `angular_damping` on the player capsule rigid body. Default: 0.5.
    #[serde(default = "default_angular_damping")]
    pub angular_damping: f32,
    /// Distance (metres) the ground-detection sphere is swept downward each frame.
    /// Decrease for flat terrain; increase for uneven terrain or fast vertical movement. Default: 0.3.
    #[serde(default = "default_ground_cast_length")]
    pub ground_cast_length: f32,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            walk_speed: default_walk_speed(),
            run_speed: default_run_speed(),
            rot_speed: None,
            jump: None,
            double_jump: false,
            double_jump_height: None,
            collider_radius: None,
            collider_height: None,
            idle_drag: default_idle_drag(),
            linear_damping: default_linear_damping(),
            angular_damping: default_angular_damping(),
            ground_cast_length: default_ground_cast_length(),
        }
    }
}

fn default_walk_speed() -> f32 { 5.0 }
fn default_run_speed() -> f32 { 10.0 }
fn default_idle_drag() -> f32 { 0.8 }
fn default_linear_damping() -> f32 { 0.5 }
fn default_angular_damping() -> f32 { 0.5 }
fn default_ground_cast_length() -> f32 { 0.3 }

/// Jump height expressed either as an absolute world-space value or as a fraction
/// of the entity's own height.
///
/// RON requires an explicit `Some(...)` wrapper because the field type is `Option<JumpConfig>`.
/// Struct variant fields go directly inside the variant's parentheses (no extra nesting):
/// ```ron
/// jump: Some(Fixed(height: 2.5))
/// jump: Some(RelativeToHeight(percent: 100.0))
/// ```
#[derive(Deserialize, Debug, Clone)]
pub enum JumpConfig {
    /// Jump to exactly `height` metres above the current position.
    Fixed { height: f32 },
    /// Jump to `percent / 100 × entity_height` metres. `100` = one full entity height.
    RelativeToHeight { percent: f32 },
}

/// Dimension and appearance overrides for `kind: "primitive"` prefabs.
///
/// Field semantics by shape:
/// - `size`       → Cuboid (x, y, z)
/// - `radius`     → Sphere radius | Cylinder/Capsule/Cone radius | Torus outer radius | ConicalFrustum bottom radius
/// - `radius_top` → ConicalFrustum top radius | Torus inner radius
/// - `height`     → total visual height for all shapes (Cylinder, Capsule3d, Cone, ConicalFrustum)
/// - `color`      → base color as linear sRGB (r, g, b) in the 0.0–1.0 range
/// - `roughness`  → perceptual roughness (0 = mirror, 1 = fully rough; default 0.5)
/// - `metallic`   → metallic factor (0 = dielectric, 1 = full metal; default 0.0)
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct PrimitiveParams {
    #[serde(default)]
    pub size: Option<(f32, f32, f32)>,
    #[serde(default)]
    pub radius: Option<f32>,
    #[serde(default)]
    pub radius_top: Option<f32>,
    #[serde(default)]
    pub height: Option<f32>,
    #[serde(default)]
    pub color: Option<(f32, f32, f32)>,
    #[serde(default)]
    pub roughness: Option<f32>,
    #[serde(default)]
    pub metallic: Option<f32>,
    /// When `true`, a static `RigidBody::Fixed` Rapier collider is spawned alongside
    /// this mesh. Supported shapes: `Cuboid`, `Sphere`, `Cylinder`.
    /// Other shapes emit a warning and skip the collider.
    #[serde(default)]
    pub physics: bool,
    /// When `true`, a ghost Rapier `Sensor` collider is spawned. The entity has no
    /// physical presence but generates `CollisionEvent`s when overlapped by other
    /// colliders. `sensor` takes precedence over `physics` if both are set.
    /// Supported shapes: same as `physics`.
    #[serde(default)]
    pub sensor: bool,
}

/// One element within a composite `kind: Primitive` prefab.
/// Either an inline primitive shape (`shape` set, `prefab` absent) or a nested prefab
/// reference (`prefab` set, `shape` absent). The transform fields apply to both variants.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ChildPrimitiveDef {
    /// Inline primitive shape. Leave `None` (or omit) when `prefab` is set.
    #[serde(default)]
    pub shape: Option<PrimitiveShapeKind>,
    /// Appearance overrides for inline primitive children. All sub-fields optional.
    #[serde(default)]
    pub primitive: PrimitiveParams,
    /// Translation offset from the parent prefab's origin. Default: `(0, 0, 0)`.
    #[serde(default)]
    pub offset: (f32, f32, f32),
    /// Euler rotation in degrees (XYZ order). Default: `(0, 0, 0)`.
    #[serde(default)]
    pub rotation_euler_deg: (f32, f32, f32),
    /// Scale applied to this child. Default: `(1, 1, 1)`.
    #[serde(default = "one_vec3_child")]
    pub scale: (f32, f32, f32),
    /// Optional key into `AssetCatalog.materials` to override the default PBR material.
    /// Only used for inline primitive children; ignored when `prefab` is set.
    #[serde(default)]
    pub material: Option<String>,
    /// Nested prefab reference — key into `PrefabCatalog.prefabs`.
    /// Mutually exclusive with `shape`. When set, a Bevy child anchor is spawned at the
    /// offset/rotation/scale above, and the referenced prefab's children are spawned under it.
    #[serde(default)]
    pub prefab: Option<String>,
}

fn one_vec3_child() -> (f32, f32, f32) { (1.0, 1.0, 1.0) }

// ─── Inventory / merchant ─────────────────────────────────────────────────────

/// One item entry in `InventoryContainerDef.initial_items`.
#[derive(Deserialize, Debug, Clone)]
pub struct InitialItemEntry {
    pub item_key: String,
    /// How many to add. Default: 1.
    #[serde(default = "default_item_count_one")]
    pub count: u32,
}

fn default_item_count_one() -> u32 { 1 }

/// Declares that a prefab entity has an inventory container.
/// Entity-attached `Inventory` components are cleared on scene load.
/// For player-persistent inventory use the `PlayerInventory` resource instead.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct InventoryContainerDef {
    /// Number of item slots. Minimum 4. Default: 9.
    #[serde(default = "default_max_slots")]
    pub max_slots: usize,
    /// Items pre-placed in the container at spawn time.
    /// Placed in slot order; excess items are silently ignored when slots are full.
    #[serde(default)]
    pub initial_items: Vec<InitialItemEntry>,
}

fn default_max_slots() -> usize { 9 }

/// Declares that a prefab entity is a merchant.
/// `Action::OpenShop(entity_id)` populates the scene's `ShopPanel` from this def.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct MerchantDef {
    pub stock: Vec<ShopEntry>,
    /// Global stat key used as currency (e.g. `"gold"`). Default: `"gold"`.
    #[serde(default = "default_currency_stat")]
    pub currency_stat: String,
}

fn default_currency_stat() -> String { "gold".to_string() }

/// One item line in a merchant's stock list.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ShopEntry {
    /// Key in the `ItemCatalog`.
    pub item_key: String,
    /// Price to buy this item from the merchant (deducted from the currency stat).
    pub buy_price: u32,
    /// Price the merchant pays when the player sells this item (added to currency stat).
    pub sell_price: u32,
    /// Finite stock — restocks at scene load. `None` means unlimited.
    #[serde(default)]
    pub stock_count: Option<u32>,
}
