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
}

/// Runtime-relevant prefab component data.
/// Additional design-time fields (health, ai, etc.) are silently ignored.
/// NOTE: deny_unknown_fields is intentionally absent — designer-only fields like
/// `health`, `ai`, `movement` are valid here and silently dropped at runtime.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct PrefabComponents {
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Dimension and appearance overrides for `kind: "primitive"` prefabs.
///
/// Field semantics by shape:
/// - `size`       → Cuboid (x, y, z)
/// - `radius`     → Sphere radius | Cylinder/Capsule/Cone radius | Torus outer radius | ConicalFrustum bottom radius
/// - `radius_top` → ConicalFrustum top radius | Torus inner radius
/// - `height`     → Cylinder height | Capsule half_length | Cone height | ConicalFrustum height
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
}

fn one_vec3_child() -> (f32, f32, f32) { (1.0, 1.0, 1.0) }
