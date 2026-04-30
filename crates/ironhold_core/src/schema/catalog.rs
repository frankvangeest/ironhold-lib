use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use super::material::MaterialDef;

pub const ASSET_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const PREFAB_CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct AssetCatalog {
    pub schema_version: u32,
    #[serde(default)]
    pub models: HashMap<String, ModelCatalogEntry>,
    #[serde(default)]
    pub textures: HashMap<String, String>,
    #[serde(default)]
    pub audio: HashMap<String, String>,
    #[serde(default)]
    pub materials: HashMap<String, MaterialDef>,
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
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ModelCatalogEntry {
    pub path: String,
}

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
            match prefab.kind.as_str() {
                "actor" | "prop" | "primitive" => {}
                other => {
                    return Err(format!(
                        "Prefab \"{}\" has unknown kind \"{}\" (expected \"actor\", \"prop\", or \"primitive\")",
                        key, other
                    ));
                }
            }
        }
        Ok(())
    }
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
    pub kind: String,   // "actor", "prop", or "primitive"
    pub model: String,  // key into AssetCatalog.models; repurposed as shape name for "primitive" kind
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
    /// `radius` metres and presses the interact key (default: F).
    #[serde(default)]
    pub interactable: Option<InteractableDef>,
    /// When set, a Rapier sensor collider is spawned and the entity emits
    /// `entity.entered:{id}` / `entity.exited:{id}` on player overlap.
    #[serde(default)]
    pub trigger_zone: Option<TriggerZoneDef>,
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
/// Emits `entity.interacted:{id}` when the player is within `radius` metres and presses F.
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
}

fn default_approach_distance() -> f32 { 2.0 }
fn default_patrol_speed() -> f32 { 2.0 }
fn default_chase_speed() -> f32 { 4.5 }

/// Runtime-relevant prefab component data.
/// Additional design-time fields (health, ai, etc.) are silently ignored.
/// NOTE: deny_unknown_fields is intentionally absent — designer-only fields like
/// `health`, `ai`, `dialogue` are valid here and silently dropped at runtime.
#[derive(Deserialize, Debug, Clone, Default)]
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
}

/// Movement parameters that can be set on any primitive prefab with the "player" tag.
/// All fields are optional; omitting a field keeps the runtime default.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct MovementConfig {
    /// Walking speed in m/s. Default: 3.0.
    #[serde(default = "default_walk_speed")]
    pub walk_speed: f32,
    /// Running speed in m/s. Default: 6.0.
    #[serde(default = "default_run_speed")]
    pub run_speed: f32,
    /// Jump height. Default: `RelativeToHeight` with `percent: 100` (player's own height).
    #[serde(default)]
    pub jump: Option<JumpConfig>,
    /// Enable a second jump while airborne. Default: false.
    #[serde(default)]
    pub double_jump: bool,
    /// Height for the second jump. If omitted, uses the same height as `jump`.
    #[serde(default)]
    pub double_jump_height: Option<JumpConfig>,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            walk_speed: default_walk_speed(),
            run_speed: default_run_speed(),
            jump: None,
            double_jump: false,
            double_jump_height: None,
        }
    }
}

fn default_walk_speed() -> f32 { 5.0 }
fn default_run_speed() -> f32 { 10.0 }

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

/// One mesh component within a composite `kind: "primitive"` prefab.
/// All fields except `shape` are optional and default to zero/identity/grey.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ChildPrimitiveDef {
    /// Shape name — same vocabulary as the top-level `model` field for single primitives:
    /// `"Cuboid"`, `"Sphere"`, `"Cylinder"`, `"Capsule3d"`, `"Cone"`, `"Torus"`, `"ConicalFrustum"`.
    pub shape: String,
    /// Appearance overrides for this child. All sub-fields are optional.
    #[serde(default)]
    pub primitive: PrimitiveParams,
    /// Translation offset from the parent prefab's origin. Default: `(0, 0, 0)`.
    #[serde(default)]
    pub offset: (f32, f32, f32),
    /// Euler rotation in degrees (XYZ order) for this child. Default: `(0, 0, 0)`.
    #[serde(default)]
    pub rotation_euler_deg: (f32, f32, f32),
    /// Scale applied to this child. Default: `(1, 1, 1)`.
    #[serde(default = "one_vec3_child")]
    pub scale: (f32, f32, f32),
    /// Optional key into `AssetCatalog.materials` to override the default PBR material
    /// for this child mesh. When set, the built material is used instead of the colour
    /// and roughness values in `primitive`. Omit to keep the default behaviour.
    #[serde(default)]
    pub material: Option<String>,
}

fn one_vec3_child() -> (f32, f32, f32) { (1.0, 1.0, 1.0) }
