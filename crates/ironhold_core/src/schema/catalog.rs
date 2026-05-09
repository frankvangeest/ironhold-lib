use bevy::prelude::*;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use super::material::MaterialDef;
use super::player::{CameraConfig, InputMap};

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
    pub audio: HashMap<String, AudioEntry>,
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
            match prefab.kind.as_str() {
                "actor" | "prop" | "primitive" => {}
                other => {
                    return Err(format!(
                        "Prefab \"{}\" has unknown kind \"{}\" (expected \"actor\", \"prop\", or \"primitive\")",
                        key, other
                    ));
                }
            }
            for (i, child) in prefab.children.iter().enumerate() {
                match (&child.prefab, child.shape.as_str()) {
                    (Some(_), shape) if !shape.is_empty() => {
                        return Err(format!(
                            "Prefab \"{}\", child {}: `shape` and `prefab` are mutually exclusive",
                            key, i
                        ));
                    }
                    (None, "") => {
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
    /// One or more static physics colliders for `kind: "actor"` / `kind: "prop"` prefabs.
    /// All shapes are combined into a single Rapier compound `RigidBody::Fixed` so the player
    /// can stand on or collide with the GLB without primitive wrappers. Use multiple entries
    /// to approximate curved geometry (arches, irregular props) or multi-part shapes (chest lid
    /// + base). An empty list means no physics collider is attached.
    #[serde(default)]
    pub colliders: Vec<ColliderDef>,
}

/// One physics collider shape in a `PrefabDef.colliders` list.
/// All geometry fields are optional; reasonable defaults apply.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ColliderDef {
    /// `"Cuboid"`, `"Sphere"`, or `"Cylinder"`.
    pub shape: String,
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

/// One element within a composite `kind: "primitive"` prefab.
/// Either an inline primitive shape (`shape` set, `prefab` absent) or a nested prefab
/// reference (`prefab` set, `shape` absent). The transform fields apply to both variants.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ChildPrimitiveDef {
    /// Inline primitive shape — same vocabulary as the top-level `model` field:
    /// `"Cuboid"`, `"Sphere"`, `"Cylinder"`, `"Capsule3d"`, `"Cone"`, `"Torus"`, `"ConicalFrustum"`.
    /// Leave empty (or omit) when `prefab` is set.
    #[serde(default)]
    pub shape: String,
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
